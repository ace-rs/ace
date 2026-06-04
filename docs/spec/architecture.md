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
(`reload_tree`, `invalidate_*`) at the small set of write sites — after `ace config set`,
`ace setup`, `ace school pull`. Rationale in
[2026-04-27-config-resolution-redesign.md](../decisions/2026-04-27-config-resolution-redesign.md);
package placement in
[2026-06-05-resolver-dissolution.md](../decisions/2026-06-05-resolver-dissolution.md).

## Dependency law

```
config ← { backend, school, skills } ← ace ← actions, cmd
```

- `config` imports nothing from the project. It owns parse **and** merge: `config/resolve/`
  folds the layers into `Resolved` and is the home of `Source` / `Sourced`.
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

- `AceToml` / `SchoolToml` / `IndexToml` — shapes of `ace.toml` (+ `.local`, user scope),
  `school.toml`, and `~/.local/share/ace/index.toml`.
- `AcePaths` / `SchoolPaths` — resolve config and clone locations from a project dir.
- `Tree` — `Option<AceToml>` per user/project/local plus `Option<SchoolToml>`. `None` means
  "no file on disk," distinct from "present but empty."
- `config/resolve/` — `merge(tree, overrides) -> Resolved`, infallible past parse, with
  per-field `Sourced<T>` provenance (rules in [configuration.md](configuration.md)). Owns
  `Source { User, Project, Local, School, Override, Default }`. Never reads a discovered
  school, so `ace config show` survives without a clone.
- `ConfigError` — parse / I/O only.

### Bindings — `backend/`, `school/`, `skills/`

Independent and fallible. No shared trait — operations differ too much (pure lookup vs
filesystem I/O vs typestate transitions). Each error type carries `#[from] ConfigError` so
tree-load failures bubble without double-handling.

- `backend/` — `Kind`, `Backend`, `Registry`, `BackendError` (`Unknown` / `Unresolvable` /
  `KindMismatch`). Each `Kind` advertises a capability bitmask (see
  [Cross-cuts](#cross-cuts)).
- `school/` — `School` built by `From<SchoolToml>`. `SchoolError::NoSpecifier` when ace.toml
  lacks `school = …`; `NotInitialized` when the resolved root has no `school.toml` (see
  [school/overview.md](school/overview.md)).
- `skills/` — the typestate model `Skill<Discovered> → Skill<Validated> → Skill<Decided>`,
  the sealed `Vetted` gate, and the `Locator` identity type (concrete names in the
  [lifecycle decision](../decisions/2026-06-04-skill-lifecycle-typestate.md)). `discover`
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
| `require_tree()`     | `Result<&Tree, ConfigError>`           | Parse the config files; load school.toml.          |
| `require_resolved()` | `Result<&Resolved, ConfigError>`       | Run the merge over `Tree` + overrides.             |
| `backend()`          | `Result<&Backend, BackendError>`       | Build the registry; look up the selected name.     |
| `require_school()`   | `Result<&SchoolPaths, SchoolError>`    | Resolve school clone path (dual-context aware).    |
| `school()`           | `Result<Option<&School>, SchoolError>` | Build the `School` domain object from school.toml. |
| `skills()`           | `Result<&Skills<Decided>, SkillError>` | Discover `<school>/skills/` and resolve.           |
| `override_backend`   | —                                      | Push a runtime override; invalidates resolved.     |
| `reload_tree`        | `Result<&Resolved, ConfigError>`       | Re-read school.toml + invalidate downstream.       |

Never create new `Ace` instances inside commands — extend the single instance with lazy
loading.

### `actions/` — operations on `Ace` and the filesystem

Peer to bindings, not nested. Grouped by user role (see
[action-layout](../decisions/2026-04-22-action-layout.md)): `actions/project/` (consumer
side — setup, prepare, clone, link, MCP register/remove, list/explain skills) and
`actions/school/` (maintainer side — init, add_import, pull_imports). Each action has its
own scoped error type (`SetupError`, `PrepareError`, …); see `CLAUDE.md`.

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
the [lifecycle decision](../decisions/2026-06-04-skill-lifecycle-typestate.md).

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
