# Decision: Path Templating in `[[backends]]` cmd and env (2026-05-09)

Status: **decided** — `{{ ... }}` placeholders rendered at bind time; no shell expansion.

## Problem

A school that ships a custom backend in its own `school.toml` cannot point `cmd[]` at a
file under the school clone, because clone paths differ per machine. Today `cmd[]` only
accepts literal absolute paths, so a base school cannot pre-roll a useful
`[[backends]]` block for downstream consumers.

Concretely, ACE's `ace-connect` skill ships
`skills/ace-connect/scripts/codex.sh` as a Codex launcher wrapper. To pre-roll a
`codex-ace` backend in the school, the decl needs a path that resolves against the
caller's school clone:

```toml
[[backends]]
name = "codex-ace"
kind = "codex"
cmd  = ["{{ school_dir }}/skills/ace-connect/scripts/codex.sh"]
```

## Decision

Render `{{ ... }}` placeholders in `[[backends]].cmd[]` **and** `env` values at bind
time, using the in-house `templates::Template`. Do **not** expand shell-style `$VAR`
or `~` — keep the substitution surface a closed set of named placeholders.

### Placeholders

| Name             | Value                                                            |
|------------------|------------------------------------------------------------------|
| `{{ school_dir }}` | Active school root (`SchoolPaths.root`).                       |
| `{{ project_dir }}`| Project working directory (`Ace::project_dir`).                |
| `{{ home }}`       | `$HOME`.                                                       |
| `{{ backend_dir }}`| `<project_dir>/<kind.backend_dir()>` for the resolved kind.    |

Unknown placeholder → empty string (current `Template::substitute` semantics). A
future `ace school validate` will catch typos before runtime; until that lands,
typos surface as missing path segments at exec time.

### Render site

`registry::bind` takes a render context and applies it inside `merge_decl`, after
`kind` is resolved (so `{{ backend_dir }}` knows which kind it is). `Backend.cmd`
and `Backend.env` carry **fully rendered** strings — downstream consumers
(`exec_session`, `exec_one_shot`) never see template syntax.

### Why not `$VAR` / `~`

Shell-style expansion is open-ended (every env var in scope is reachable) and
context-sensitive (depends on the parent shell environment at spawn time). A
closed set of named placeholders is auditable and stable across machines. Users
who need an env var route it through `env = {...}` and a `{{ ... }}` reference,
or write a literal path.

### Why both `cmd` and `env`

`cmd[]` is the primary case (school-relative launcher paths). `env` values often
need the same paths (e.g. `XDG_CONFIG_HOME = "{{ project_dir }}/.config"`). Same
renderer, same closed placeholder set — no extra surface.

## Non-goals

- **Shell expansion** — `$VAR`, `~`, command substitution. Out, see above.
- **Validation** — typo detection for placeholder names. Belongs in a separate
  `ace school validate` command; tracked separately.
- **Imported-school `[[backends]]` inheritance** — separate slice, separate spec.
  This decision only governs how a decl's strings are rendered; merge order is
  unchanged.

## References

- `src/backend/registry.rs` — `bind` and `merge_decl`.
- `src/templates/mod.rs` — `Template`.
- `spec/backend.md` — user-facing docs.
