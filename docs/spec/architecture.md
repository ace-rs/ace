# Architecture

## Layers

Five layers, demand-driven. Each binding loads on first request and caches.
See `docs/decisions/2026-04-27-config-resolution-redesign.md` for the rationale.

```
disk → Tree → Resolved → Bindings (Backend / School / Skills) → Ace → Actions / Cmd
       parse  merge       lookup / I/O                          orchestrate
```

### Config (`src/config/`)

Dumb I/O. Parses TOML, writes back. No merging, no resolution.

- `AceToml` — shape of `ace.toml` / `ace.local.toml` / `~/.config/ace/ace.toml`.
- `SchoolToml` — shape of `school.toml`.
- `IndexToml` — shape of `~/.local/share/ace/index.toml` (downloaded schools).
- `AcePaths`, `SchoolPaths` — resolve config / clone locations from a project dir.
- `Tree` — `Option<AceToml>` for user/project/local plus `Option<SchoolToml>`. Built by
  `Tree::load(&AcePaths)` followed by `Tree::load_school(&Path)`. `None` means "no file
  on disk" — distinct from "present but empty," which matters for diagnostics.
- `ConfigError` — parse / I/O failures only. Binding-level failures live elsewhere.

### Resolver (`src/resolver/`)

Pure logic. Given `Tree` + an `AceToml`-shaped overrides layer, produce a merged view
with per-field provenance. Infallible past parse.

- `merge(tree, overrides) -> Resolved` — fold the four layers (user → project → local →
  overrides) plus the school layer per the rules in `docs/spec/configuration.md`.
- `Resolved` — the merged scalars: `school_specifier`, `backend_name`, `backend_decls`,
  `session_prompt`, `env`, `trust`, `resume`, `skip_update`. Each value is `Sourced<T>`
  carrying a `Source { User, Project, Local, School, Override, Default }`.
- `resolve_skills(...) -> Resolution` — the skills-specific resolver (lives here for
  shared `Source` vocabulary; consumed by `skills/`).

The resolver does not look up the backend, read school.toml beyond what
`Tree::load_school` already loaded, or touch the filesystem.

### Bindings — `Backend`, `School`, `Skills`

Each binding is independent and fallible. No shared trait — operations differ too much
(pure lookup vs filesystem I/O vs typestate transitions).

- `src/backend/` — `Kind`, `Backend`, `Registry`, `BackendError`.
  `registry::bind(resolved)` walks `[[backends]]` declarations into a `Registry` seeded
  with built-ins, then looks up `resolved.backend_name`. Errors: `Unknown` /
  `Unresolvable` / `KindMismatch`.
- `src/school.rs` — `School` domain object built by `From<SchoolToml>`.
  `SchoolError::NoSpecifier` when ace.toml lacks `school = ...`;
  `SchoolError::NotInitialized` when the resolved root has no `school.toml`
  (see `docs/spec/school/overview.md` Context Resolution).
- `src/skills/` — `Skills<Discovered>` / `Skills<Decided>` typestate. `Skills::discover`
  walks `<school>/skills/`; `.resolve(&Tree)` produces the resolved set with diagnostics.
  `SkillError` wraps discovery I/O plus upstream `ConfigError` / `SchoolError`.

Each binding's error type carries `#[from] ConfigError` so tree-load failures bubble
through without forced double-handling.

### Ace (`src/ace/`)

The session orchestrator. A single `Ace` instance is created in `main()` and threaded
through every command. It owns the project dir, output sink, runtime overrides, and a
lazy cache cell per layer (`tree`, `resolved`, `backend`, `school`, `skills`).

Commands declare what they need by calling accessors on the existing instance:

| Method                 | Returns                                 | What it does                                        |
| ---------------------- | --------------------------------------- | --------------------------------------------------- |
| `require_tree()`       | `Result<&Tree, ConfigError>`            | Parse the four config files; load school.toml.      |
| `require_resolved()`   | `Result<&Resolved, ConfigError>`        | Run the merge over `Tree` + overrides.              |
| `backend()`            | `Result<&Backend, BackendError>`        | Build the registry; look up the selected name.      |
| `require_school()`     | `Result<&SchoolPaths, SchoolError>`     | Resolve school clone path (dual-context aware).     |
| `school()`             | `Result<Option<&School>, SchoolError>`  | Build the `School` domain object from school.toml.  |
| `skills()`             | `Result<&Skills<Decided>, SkillError>`  | Discover `<school>/skills/` and resolve.            |
| `override_backend`     | —                                       | Push a runtime override; invalidates resolved.      |
| `reload_tree`          | `Result<&Resolved, ConfigError>`        | Re-read school.toml + invalidate downstream caches. |

Failures stay local. `ace config show` calling `resolved()` is unaffected by an unknown
backend selector. `cmd::main` matches `BackendError::Unknown` directly to drive the
recovery picker (see `docs/decisions/2026-04-27-config-resolution-redesign` §"Recovery UX").

Commands fall into three tiers:

1. **No state** — `paths`, `fmt`, `school init`. `Ace` is purely an output sink.
2. **Partial bindings** — `config get/set/show`, `diff`, `import`, `school pull`. Call
   only the accessors they need.
3. **Full orchestration** — bare `ace`, `ace auto`, `ace yolo`. Run Prepare → register
   MCP → build session prompt → exec the backend.

Never create new `Ace` instances inside commands. Extend the single instance with lazy
loading rather than bypassing it.

### Actions (`src/actions/`)

Peer to bindings, not nested inside them. Actions are operations *on* `Ace` and the
filesystem. Grouped by user role (see `docs/decisions/2026-04-22-action-layout.md`):

- **`actions/project/`** — consumer-side. User is in their own repo that consumes a
  school. Covers setup, prepare, clone, link, register/remove MCP, update gitignore,
  list/explain skills.
- **`actions/school/`** — maintainer-side. User is in a school repo, curating skills.
  Covers init, add_import, pull_imports.

Each action has its own scoped error type (`SetupError`, `PrepareError`, etc.); see
`CLAUDE.md` for the convention.

## Data Flow

```
disk → Tree → Resolved → Backend / School / Skills → action.run(&mut Ace) → disk
```

Each arrow is demand-driven. `Tree` is parsed only when something asks for it; `Resolved`
is merged only after `Tree` exists; bindings are built only when a command reaches for
them. Cache invalidation is explicit (`reload_state`, `invalidate_*`) and called at the
small set of write sites (after `ace config set`, after `ace setup`, after `ace school
pull`).

## Dependency Direction

```
config ← resolver ← backend, school, skills ← ace ← actions, cmd
```

- `config` imports nothing from the project.
- `resolver` imports `config` (raw types) only.
- Bindings import `config` and `resolver`.
- `ace` imports bindings, threads them through accessors.
- `actions` and `cmd` consume `ace`.
- No layer imports a layer to its right. `config` never imports `resolver`; bindings
  never import `ace`.

## Standalone Modules

Helper modules independent of the binding pipeline live at the `src/` top level:

- `src/git.rs` — git subprocess helpers (with `GIT_TERMINAL_PROMPT=0` baked in).
- `src/glob.rs` — simple glob matching.
- `src/fsutil.rs` — recursive copy, symlink helpers.
- `src/paths.rs`, `src/platform.rs` — XDG / OS-specific path handling.
- `src/upgrade/` — self-update: version check, binary download, self-replacement.
- `src/templates/` — session-prompt template engine.

These modules may be called from `main()`, `cmd/`, `Ace`, or any binding, but they do
not import bindings, `Ace`, or actions. They receive only the values they need.
