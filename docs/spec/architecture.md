# Architecture

## Pipeline

Five stages, demand-driven. Each is computed on first request and cached; nothing loads
until a command reaches for it.

```
disk → Tree → Resolved → Bindings (Backend / School / Skills) → Ace → Actions / Cmd
       parse  merge       lookup / I/O                          orchestrate
```

`Tree` is parsed only when something asks; `Resolved` is merged only after `Tree` exists;
bindings are built only when a command reaches for them. Cache invalidation is explicit
(`invalidate_school_caches`, `invalidate_*`) at the small set of write sites — after
`ace config set`, `ace setup`, `ace school pull`. Layer contracts and provenance rules in
[configuration.md](configuration.md); package placement in
[2026-06-05-resolver-dissolution.md](../decisions/2026-06-05-resolver-dissolution.md).

## Dependency law

```
config ← { backend, school, skills } ← ace ← actions, cmd
```

- `config` imports nothing from the project, with one type-only exception: the merge
  takes the school's parsed shape (`school::toml::SchoolToml`) as an input parameter —
  data crossing leftward, no I/O and no behavior imported. It owns parse **and** merge:
  `config/resolve/` folds the layers into `Resolved` and is the home of `Source` /
  `Sourced`.
- Bindings (`backend`, `school`, `skills`) import `config` only. They do not import `ace`.
- `skills/resolve/` imports `Source` from `config/resolve/` — a leftward import
  (binding → config), the correct direction.
- `ace` imports the bindings and threads them through accessors; `actions` and `cmd` consume
  `ace`.
- **No standalone resolver.** Resolution lives with the typed data it stamps — config merge
  in `config/resolve/`, skill resolution in `skills/resolve/`. No layer imports a layer to
  its right.

## Module map

### `config/` — parse + merge

Dumb I/O plus the pure merge; no filesystem beyond reading the config files.

- `AceToml` / `IndexToml` — shapes of `ace.toml` (+ `.local`, user scope) and
  `~/.local/share/ace/index.toml`.
- `AcePaths` — resolve config file locations from a project dir.
- `Tree` — `Option<AceToml>` per user/project/local. `None` means "no file on disk,"
  distinct from "present but empty." School content is not a layer here; `school/` owns
  it, and the merge receives it as a separate input.
- `config/resolve/` — `merge(tree, school, overrides) -> Resolved`, infallible past parse,
  with per-field `Sourced<T>` provenance (rules in [configuration.md](configuration.md)).
  Owns `Source { User, Project, Local, School, Override, Default }`. Never reads a
  discovered school itself, so `ace config show` survives without a clone.
- `ConfigError` — parse / I/O only.

### Bindings — `backend/`, `school/`, `skills/`

Independent and fallible. No shared trait — operations differ too much (pure lookup vs
filesystem I/O vs typestate transitions). Each error type carries `#[from] ConfigError` so
tree-load failures bubble without double-handling.

- `backend/` — `Kind`, `Backend`, `Registry`, `BackendError` (`Unknown` / `Unresolvable` /
  `KindMismatch`). Each `Kind` advertises a capability bitmask (see
  [Cross-cuts](#cross-cuts)).
- `school/` — owns both school roles. `school/toml.rs` parses `school.toml`
  (`SchoolToml`); `school/linked.rs` resolves the linked school's location
  (`LinkedSchool::resolve`, specifier parse, traversal checks); `School` is the domain
  view built by `From<SchoolToml>`. `SchoolError::NoSpecifier` when ace.toml lacks
  `school = …`; `NotInitialized` when the resolved root has no `school.toml`; `NoSchool`
  when an authoring command finds neither role (see
  [school/overview.md](school/overview.md)).
- `skills/` — the typestate model `Skill<Discovered> → Skill<Validated> → Skill<Decided>`,
  the sealed `Vetted` gate, and the `Locator` identity type (concrete names in the
  [lifecycle spec](skills/lifecycle.md)). `discover`
  walks the cascade in [model.md](skills/model.md#discovery-cascade); `skills/resolve/`
  stamps the decided set with diagnostics. `SkillError` wraps discovery I/O plus upstream
  `ConfigError` / `SchoolError`.

### `ace/` — session orchestrator

A single `Ace` instance is created in `main()` and threaded through every command. It owns
the project dir, output sink, runtime overrides, and a lazy cache cell per stage. Commands
declare what they need by calling accessors; failures stay local (`ace config show` is
unaffected by an unknown backend selector, which `cmd::main` matches directly to drive the
recovery picker).

| Method               | Returns                                | What it does                                       |
| -------------------- | -------------------------------------- | -------------------------------------------------- |
| `require_tree()`     | `Result<&Tree, ConfigError>`           | Parse the `ace.toml` layers.                       |
| `require_config()`   | `Result<&Resolved, ConfigError>`       | Merge `Tree` + school.toml + overrides into the effective config. |
| `backend()`          | `Result<&Backend, BackendError>`       | Build the registry; look up the selected name.     |
| `require_linked_school()` | `Result<&LinkedSchool, SchoolError>` | Resolve the linked school's location from `ace.toml`. Linked school only — not authoring-aware. |
| `require_authoring_school()` | `Result<PathBuf, SchoolError>`  | Cwd-first school for authoring commands; announced fallback to linked. |
| `school_toml()`      | `Result<&SchoolToml, SchoolError>`     | Raw school.toml content — merge input and config introspection. Absence is an error variant; tolerant callers check `SchoolError::is_absent()`. |
| `school()`           | `Result<&School, SchoolError>`         | Build the `School` domain object from school.toml. Absence as above. |
| `skills()`           | `Result<&Skills<Decided>, SkillError>` | Discover `<school>/skills/` and resolve.           |
| `override_backend`   | —                                      | Push a runtime override; invalidates resolved.     |
| `invalidate_school_caches` | —                                | Drop school-derived caches after clone-on-first-run. |

Never create new `Ace` instances inside commands — extend the single instance with lazy
loading.

### `actions/` — operations on `Ace` and the filesystem

Peer to bindings, not nested. Grouped by the **role of the invoking user**, never by the
subject a function writes to — the written-to directory is an implementation detail; the
role defines the CLI tree and the invariants each command can assume. `actions/project/`
(consumer side — setup, prepare, clone, link, MCP register/remove, list/explain skills)
and `actions/school/` (maintainer side — init, add_import, pull_imports). The two
pull-shaped operations share the verb deliberately: `project::Pull` pulls the linked
school clone from its git origin; `school::PullImports` pulls imported skills from
upstream sources into the authored school's `skills/` — scope names the side, verb names
the shape. Scopes stay flat: with a handful of actions each, file names disambiguate
(`add_import.rs`, `pull_imports.rs`); no sub-submodules. Each action has its own scoped
error type (`SetupError`, `PrepareError`, …); see `CLAUDE.md`.

### Standalone modules

Helpers independent of the pipeline, at the `src/` top level: `git.rs`, `glob.rs`,
`fsutil.rs`, `paths.rs`, `platform.rs`, `upgrade/`, `templates/`. They may be called from
`main()`, `cmd/`, `Ace`, or any binding, but import none of them — they receive only the
values they need.

## Cross-cuts

Facts no single module owns.

### Skills span bindings → actions

The skill lifecycle crosses layers: discovery and admission live in `skills/`, selection in
`skills/resolve/`, emit in a project-side action. A skill flows
`discover → validate → resolve → emit`. The behavioral spec is under [`skills/`](skills/)
(`model.md`, `selection.md`, `emit.md`); the typestate shape and its `Vetted` gate are in
the [lifecycle spec](skills/lifecycle.md).

### Identity is constructed solely by discovery

A skill's identity (`Locator`) is the discovery path with the longest matching discovery
prefix stripped (`skills/typescript/coding/` → `typescript/coding`). Discovery is the
**only** layer that mints identities — downstream boundaries cannot synthesize one from a
raw string, and a user selection pattern stays a plain validated string, never an identity.
Frontmatter `name` is deliberately not identity; ACE keys off the path and leaves `name` for
per-backend display.

### Capability-driven emit

Backends disagree on whether their loader walks nested skill dirs. Rather than branch on
backend name, each `Kind` advertises a feature bitmask (`Kind::features()`); emit and link
code branch on feature bits. `FEATURE_NESTED_SKILLS` is the one that matters today:

- **Set** — the loader handles nested `<backend>/skills/<identity-path>/` layouts (Codex,
  OpenCode). ACE emits verbatim at the identity path; nested emit cannot collide (identity
  paths are unique by construction).
- **Clear** — the loader sees only the top level (Claude Code). ACE flattens: emit name is
  `basename(identity)`, structurally checked, with loser-drop on collision
  (alphabetical-by-source tiebreaker, warn, drop the loser).

A global `MAX_SKILL_DEPTH` cap routes over-deep skills to the flatten branch even on
nested-capable backends. Custom `[[backends]]` entries inherit their kind's features. The
bitmask is the extension point: add a `FEATURE_*` constant, set the bit, branch on the bit —
never on the name.
