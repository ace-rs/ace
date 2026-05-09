# 007: Config Resolution Redesign

## Context

Today's `State::resolve` is one eager pass that conflates two distinct concerns into a
single fallible operation:

1. **Merged config** — the layered config files (user / project / local / school) folded
   into one effective view per the project's resolution rules.
2. **Operational bindings** — looking up the selected backend in the registry, loading the
   school clone from disk, discovering on-disk skills against the merged skill config.

When any one binding fails (e.g. `UnknownBackend("bailer")` because the user-scope
selector points at a school-local custom backend in a different repo), the entire
`State::resolve` call fails. Read-only and write-only commands that have no relationship
to the failing binding (`ace config set school X`, `ace config show`, `ace setup`) get
blocked anyway because they all go through the same fail-fast pipeline.

PROD9-146 ("scope-aware backend selector validation") is the visible symptom. The
underlying issue is the design.

## Decision

Restructure config resolution as a four-layer demand-driven pipeline:

```
Disk → Tree → Resolved → Bindings (Backend / School / SkillSet)
       parse  merge       lookup / I/O
```

Each layer is computed only when something asks for it. Each binding has its own error
type and its own recovery surface. Failures stay local.

## Layer 1: `Tree` — raw layered config

Pure parsed data from disk. No derivation, no path resolution, no validation beyond parse.

```rust
pub struct Tree {
    pub user: Option<AceToml>,
    pub project: Option<AceToml>,
    pub local: Option<AceToml>,
    pub school: Option<SchoolToml>,
}
```

`Option<_>` for all four sources. `None` means "no file on disk." Distinguishable from
"present but empty," which matters for diagnostics ("school.toml not found" vs
"school.toml present, no school configured"). Today's asymmetry — `AceToml::default()` for
missing ace.tomls but `Option<SchoolToml>` for school — was incidental; we standardise.

`Tree::load(&Paths) -> Result<Tree, ConfigError>`. Fails only on parse / I/O. Today's
derived fields on `Tree` (`school_backend`, `school_paths`) come out — they're computed
downstream, not raw input.

## Layer 2: `Resolved` — merged scalars with provenance

The single merged-config view. Replaces today's public `State` (operational mush) and
private `Resolved` (interim).

```rust
pub struct Resolved {
    pub school_specifier: Sourced<Option<String>>,
    pub backend_name:     Sourced<String>,
    pub backend_decls:    Vec<Sourced<BackendDecl>>,
    pub session_prompt:   Sourced<String>,
    pub env:              HashMap<String, Sourced<String>>,
    pub trust:            Sourced<Trust>,
    pub resume:           Sourced<bool>,
    pub skip_update:      Sourced<bool>,
}

pub struct Sourced<T> { pub value: T, pub from: Source }

pub enum Source { User, Project, Local, School, Override, Default }
```

Skills resolution lives on its own typestate (`Skills<Discovered>` →
`Skills<Decided>` in `src/skills/`), not on `Resolved`. Folding it in would
either drop trace information or force `Resolved::merge` to take a discovered
school clone as input — which defeats PROD9-146 (`ace config show` must
survive without a school clone). `Ace::skills()` runs the resolver lazily
against `tree()` when something asks.

`Resolved::merge(tree: &Tree, overrides: &AceToml) -> Resolved` — pure transform.
Infallible past parse. Tracks which layer contributed each field.

Provenance is a deliberate addition: it's the engine for `ace config explain`. Computing
it later by re-walking layers per key duplicates merge logic; folding it into the merge is
cheap and keeps source-of-truth single. The skills resolver already does this for skills
(Decision/Op/Scope/Field types in `state/resolver.rs`); we generalise.

For additive fields (`env`, `include_skills`, `exclude_skills`), provenance is per-entry —
`HashMap<String, Sourced<String>>` for env, etc.

### Parity with skills resolution

The skills resolver in today's `state/resolver.rs` already tracks provenance, but with its
own vocabulary (`Decision`, `Op`, `Scope`, `Field`, `Entry`). It needs a richer shape than
`Sourced<T>` because each skill's outcome is the result of multiple layer contributions
(base set + per-layer include/exclude ops), not a single winning layer.

We unify what *can* be unified and accept divergence where the shapes genuinely differ:

- **`Source` enum is shared.** Skills resolution uses the same `Source { User, Project,
  Local, School, Override, Default }` enum as scalar resolution. Today's `resolver::Scope`
  type folds into this — one vocabulary across the codebase for "which layer."
- **Trace shape stays skill-specific.** Each `ResolvedSkill` keeps its multi-entry
  trace (`Vec<Entry>` of layer + op contributions). Forcing it into `Sourced<Skill>`
  would either drop information (only the "winning" layer) or require `Sourced` to
  grow a multi-entry variant that scalars don't need.
- **Naming aligns.** Today's `Decision` / `Op` / `Field` types stay; they're
  skill-specific vocabulary. But anywhere a "which layer" appears, it's `Source`, not
  a skills-only type.

Result: `ace config explain backend` and `ace config explain skills` share the layer
vocabulary in their output ("project", "local", "override") even though the per-key
rendering differs (scalar = one winner, skills = trace of contributions).

## Layer 3: Bindings — independent, demand-driven, fallible

Each binding is its own type with its own constructor and error. Not unified behind a
trait — operations differ too much (pure lookup vs filesystem I/O).

```rust
impl Backend {
    pub fn lookup(r: &Resolved) -> Result<Backend, BackendError>;
}

impl School {
    pub fn load(r: &Resolved, paths: &Paths) -> Result<Option<School>, SchoolError>;
}

impl SkillSet {
    pub fn discover(r: &Resolved, school: &School) -> Result<SkillSet, SkillError>;
}
```

Per-binding error types replace today's umbrella `ConfigError::UnknownBackend` /
`BackendKindMismatch` / etc.:

```rust
pub enum BackendError { Unknown(String), KindMismatch { ... }, Unresolvable(String) }
pub enum SchoolError  { CloneMissing, SchoolTomlInvalid(...), ... }
pub enum SkillError   { DiscoveryFailed(...), CollisionsBlocked(...), ... }
```

Recovery code pattern-matches the small focused enum at the call site that asked for the
binding. State layer doesn't know about TTY, picker UX, or hint generation — that's
cmd-layer concern.

## Layer 4: `Ace` — session orchestrator with lazy caches

`Ace` becomes thin. Paths, mode, output sink, runtime overrides, plus a `OnceCell` per
layer.

```rust
pub struct Ace {
    pub paths: Paths,
    pub project_dir: PathBuf,
    pub overrides: AceToml,            // CLI flag layer (see "Overrides" below)
    pub mode: TerminalMode,
    output: Sink,

    tree:         OnceCell<Tree>,
    resolved:     OnceCell<Resolved>,
    backend:      OnceCell<Backend>,
    school_paths: OnceCell<SchoolPaths>,
    school:       OnceCell<Option<School>>,
    skills:       OnceCell<SkillSet>,
}

impl Ace {
    pub fn tree(&self)           -> Result<&Tree, ConfigError>;
    pub fn resolved(&self)       -> Result<&Resolved, ConfigError>;
    pub fn backend(&self)        -> Result<&Backend, BackendError>;
    pub fn require_school(&self) -> Result<&SchoolPaths, SchoolError>;
    pub fn school(&self)         -> Result<Option<&School>, SchoolError>;
    pub fn skills(&self)         -> Result<&SkillSet, SkillError>;

    pub fn invalidate_resolved(&mut self);
    pub fn invalidate_school(&mut self);
    pub fn invalidate_skills(&mut self);
    // tree invalidation is rarer — after writes it's the resolved cache that matters
}
```

The cells hold `T`, not `Result<T, E>`. `OnceCell::get_or_try_init` returns the
error and leaves the cell empty on failure, so failed loads retry on the next call.
Memoizing the error would force `Clone` on every error variant (notably the
`std::io::Error` carriers in `ConfigError` / `SkillError`), and the recovery surface
already expects retry-after-fix semantics.

`SchoolPaths` is a sixth binding the redesign keeps — it resolves the dual
project/school context (school.toml in cwd vs cached clone of the configured
specifier) and is independent of the parsed `School` domain object.

`State` is gone. It was the bag operational fields got swept into; once `Resolved` exists
and bindings are independent, State adds no value.

Cache invalidation is explicit (`invalidate_*`). Fewer surprises than mtime-based or
compare-and-swap; the call sites are few (writes after `ace config set`, after `ace
setup`, after `ace school pull`).

## Overrides — runtime layer with the same shape

Today's `RuntimeOverrides { backend: Option<String> }` was hand-shaped to one need. If
overrides participate in merge as another layer, they *are* a layer — same shape as
`AceToml`, not "a partial Resolved with extra Options."

`Ace::overrides: AceToml` (default-empty). CLI flags populate fields on it.
`Resolved::merge` treats it as the 5th, highest-priority layer with `Source::Override`.
Today's hand-coded `overrides.backend.or_else(...)` chains collapse into one uniform fold.

Result: `--backend X`, `--trust auto`, `--no-resume`, `--env KEY=val`, `--session-prompt
...` all become free without per-flag plumbing. Some fields aren't naturally CLI-driven
(e.g. `school`); that's a CLI policy choice, not a shape concern.

## Module layout

- `src/config/` — `Tree`, `AceToml`, `SchoolToml`, `BackendDecl`, `ConfigError`. Disk
  parse.
- `src/resolver/` — `Resolved`, `Sourced`, `Source`, merge engine. (Replaces today's
  `src/state/`.)
- `src/resolver/skills.rs` — skill-specific resolution, folded from today's
  `state/resolver.rs`.
- `src/backend/`, `src/school/`, `src/skills/` — bindings with their own error types.
- `src/ace/` — orchestrator, lazy caches.

`src/state/` directory is fully retired. Anything that was there either moves to
`resolver/` (merge logic) or to one of the binding modules (operational types).

## Commands by dependency

Each command asks for exactly what it needs.

| Command                                   | Needs                                             | Notes                                                               |
| ----------------------------------------- | ------------------------------------------------- | ------------------------------------------------------------------- |
| `ace config get` / `set`                  | `tree()`, `resolved()`                            | Set writes to a Tree layer; get reads merged Resolved. No bindings. |
| `ace config show`                         | `resolved()`                                      | Print merged values. Stale backend selector doesn't block.          |
| `ace config explain [key]`                | `resolved()`                                      | New. Prints `Sourced` provenance per field.                         |
| `ace setup`                               | `resolved()`, `school()`, `backend()`             | Needs `backend()` for instructions-file path + per-backend dir.     |
| `ace school pull` / `init` / `add-import` | `school()`, `tree()`                              | Backend irrelevant.                                                 |
| `ace import`                              | `tree()`                                          | Writes to school skills/.                                           |
| `ace doctor`                              | `resolved()`, `backend()`, `school()`, `skills()` | All bindings, but each failure is reported independently.           |
| `ace upgrade`                             | `resolved()`                                      | Just needs `skip_update`.                                           |
| `ace paths`                               | `paths` directly, `resolved()`                    | Needs `resolved()` for the school specifier when printing paths.    |
| `ace` / `ace auto`                        | full pipeline                                     | On `BackendError::Unknown`, run recovery picker.                    |

## `ace config explain`

Output shape:

```
backend = "bailer"           [project]
  user:    (unset)
  project: "bailer"          ← winner
  local:   (unset)
  school:  (unset)
  override:(unset)

env.ANTHROPIC_BASE_URL = "https://api.bailer.io/anthropic"  [local]
  user:    (unset)
  project: "https://example.com"
  local:   "https://api.bailer.io/anthropic"               ← winner
  school:  (unset)
  override:(unset)

trust = "default"  [default]
```

Bare `ace config explain` prints all keys. `ace config explain backend` filters to one.

## Recovery UX (the PROD9-146 deliverable)

Session-start sites (`cmd::main::run_inner`, `cmd::auto`) call `ace.backend()`. On
`Err(BackendError::Unknown(name))`:

- **TTY**: `term_ui::select` over registry names. User picks. Re-resolve via
  `ace.overrides.backend = Some(pick); ace.invalidate_resolved();
  ace.invalidate_backend();`. Run session. Print hint: `to make permanent: ace config
  set backend <pick>`.
- **Non-TTY**: hard fail with the same hint inlined.

No silent fallback — a wrong-backend run could route prompts to the wrong vendor (bailer →
Anthropic) which is worse than failing.

## Migration steps

The redesign lands as a sequence of small slices. Each ships independently; tests stay
green throughout.

1. **Tree shape** — `Option<AceToml>` for all four layer sources. Mostly rename + None
   handling in load.
2. **Extract `Resolved`** — promote today's private `Resolved` to public; fold `State`'s
   scalar fields in. Keep `State.backend` / `State.school` for now.
3. **`Sourced<T>` + provenance** — extend merge to track origin per field. Unify the
   `Source` enum across config and skills resolution (today's `resolver::Scope` folds in).
   New `ace config explain` command.
4. **Lazy `Backend::lookup`** — drop `State.backend` field; add `Ace::backend()` lazy
   cell. `cmd::main` recovery picker for `BackendError::Unknown`. **This is the
   user-visible PROD9-146 fix.**
5. **Lazy `School::load`** — drop `State.school` field; add `Ace::school()` lazy cell.
   Adjust `require_school` callers.
6. **Lazy `SkillSet::discover`** — drop today's eager skills resolution; add
   `Ace::skills()` lazy cell.
7. **Drop `State`** — `Ace` holds the lazy cells directly; module rename `state/` →
   `resolver/`.
8. **Per-binding error types** — split `ConfigError::UnknownBackend` etc. into
   `BackendError` / `SchoolError` / `SkillError`.
9. **Generalise overrides** — `RuntimeOverrides` becomes `AceToml`-shaped 5th layer.

Steps 1–4 deliver PROD9-146. Steps 5–9 finish the redesign and can land as separate
tickets over multiple sessions.

## Out of scope

- Async. Bindings stay sync; we already dropped `smol`.
- Trait abstraction over bindings. Concrete types per binding read more honestly than a
  uniform `bind` interface.
- Persistence layer. Paths, `index.toml`, school clones, import cache — unchanged.
- CLI flag parsing. Adding override-shaped flags is mechanical once the override layer
  accepts them.

## Tickets

- **PROD9-146** — reframed as "the recovery UX symptom of the resolver redesign."
  Slice 4 above is its deliverable.
- New tickets to file for slices 1–3 (lay the foundation), 5–9 (finish the redesign).
  One ticket per slice.

## Why not alternatives

**Typestate (`ConfigView<S>`).** Considered. Skills uses it (`Skills<Discovered>` /
`Skills<Resolved>`) because the underlying shape is identical and only invariants progress
linearly. Config doesn't fit: shapes differ across stages, and bindings fan out into three
independent axes. Forcing a phantom-typed shell either makes fields optional everywhere
(defeats the type-level guarantees) or makes per-state impls diverge so much the shared
shell is theatre.

**Eager binding with optional fields (`State.backend: Option<Backend>`).** Half-measure.
Keeps the umbrella struct alive, adds an `Option` layer at every call site, and doesn't
generalise to school/skills. Picking the deeper redesign now is cheaper than two passes.

**Validation in `Resolved::merge` to catch bad decls early.** Considered. Means `merge`
becomes fallible and reintroduces "config show fails on unrelated decl." The alternative —
defer all validation to `Backend::lookup` — keeps merge infallible and validation
co-located with the binding that cares. Trade-off: malformed `[[backends]]` decls don't
surface until something asks for the backend. Acceptable; `ace doctor` / `ace config
explain` exercise all bindings on demand for users who want eager checks.

