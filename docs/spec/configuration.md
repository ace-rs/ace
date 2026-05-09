# Configuration Management

## Format

TOML.

## Layers

Resolved by merging (later overrides earlier):

1. **User** `~/.config/ace/ace.toml` (or `$XDG_CONFIG_HOME/ace/ace.toml`) — personal defaults across all projects
2. **Project** `ace.toml` (checked into git, shared with team)
3. **Local** `ace.local.toml` (gitignored, per-machine overrides)

### Fields

Each layer can set:

- `school` — school specifier (last non-empty wins)
- `backend` — backend name (highest-priority `Some` wins: local → project → user
  → school's `school.toml` → fallback `claude`). Built-ins: `"claude"`,
  `"codex"`, `"flaude"`. Custom names are valid when declared in `[[backends]]`
  (see [Custom backends](#custom-backends)). See [backend.md](backend.md).
- `role` — *(planned, PROD9-19)* selected role name (last non-empty wins). Will match a `[[roles]]` entry in the school's `school.toml`. Typically set in `ace.local.toml` via interactive selection. See [roles.md](roles.md). Not yet implemented.
- `session_prompt` — additional prompt text (last non-empty wins)
- `env` — environment variables (additive merge, later keys override)
- `skip_update` — disable automatic version check and background upgrade. Default: `false`.
  See [upgrade.md](upgrade.md). Also overridden by `ACE_SKIP_UPDATE=1` env var.
- `skills` — per-project skill whitelist. Last-wins replace across scopes; empty
  at all scopes = "all skills". See [Skills selection](#skills-selection).
- `include_skills` — always-add skill patterns. **Union across all scopes**
  (exception to last-wins). See [Skills selection](#skills-selection).
- `exclude_skills` — always-remove skill patterns. **Union across all scopes**
  (exception to last-wins). See [Skills selection](#skills-selection).

### Personal-only fields

These fields are resolved from the **user** and **local** layers only — never from
project-committed `ace.toml` or `school.toml`. They are personal workflow preferences.

- `trust` — permission mode: `"default"`, `"auto"`, or `"yolo"`. Default: `"default"`.
- `resume` — auto-resume previous session on `ace` launch. Default: `true`.
  When `true`, `ace` passes resume flags to the backend if a previous session exists for
  the current project directory. `ace --new` (or `ace -n`) forces a fresh session regardless.
  Backends that don't support resume start fresh silently.

Resolution for personal-only fields: local wins over user. Project layer is skipped entirely.

## Custom backends

`[[backends]]` declarations seed or augment the backend registry. Built-ins
(`claude`, `codex`, `flaude`) are pre-registered; declarations override their
`env`/`cmd` or introduce new names that reuse an existing built-in's behavior
(its *kind*).

### Fields

- `name` — registry key. Required. May match a built-in (override) or be new.
- `kind` — built-in name whose behavior to reuse (`"claude"`, `"codex"`,
  `"flaude"`). Optional.
- `cmd` — argv for launching the binary. `cmd[0]` is the program; `cmd[1..]`
  are prepended to runtime args. Optional.
- `env` — environment variables merged into the launched process. Optional.

### Layer order

`[[backends]]` may appear in `school.toml` and in any `ace.toml` /
`ace.local.toml` layer. Resolution walks **built-ins → school → user → project
→ local**, applying each declaration in order.

### Resolution rules

For each declaration:

- **Name already registered** (built-in or earlier-layer custom) — partial
  override:
  - `env` per-key last-wins.
  - `cmd` last-wins-non-empty (empty `cmd` does not clobber a prior value).
  - `kind`, if specified, must match the existing entry's kind. Mismatch
    errors with `BackendKindMismatch`.
- **New name** — kind is resolved by trying:
  1. Explicit `kind` field.
  2. `name` matching a built-in name.
  3. `cmd[0]` basename matching a built-in name.
  4. Otherwise: error `UnresolvableBackendKind`.

  Then `cmd` defaults to `[kind.name()]` if not given.

### Selecting a custom backend

Once registered, a custom name is selectable like a built-in:

- `backend = "bailer"` in any `ace.toml` layer.
- `--backend bailer` on the CLI (or `ace config set backend bailer`).

Unknown names error with `UnknownBackend` at resolve time.

### Examples

```toml
# Tweak built-in claude's env
[[backends]]
name = "claude"
env = { ANTHROPIC_LOG = "debug" }

# Custom backend reusing claude's binary
[[backends]]
name = "bailer"
kind = "claude"
env = { ANTHROPIC_BASE_URL = "..." }

# Custom backend with a forked binary; kind inferred from cmd[0] basename
[[backends]]
name = "bedrock-claude"
cmd = ["claude-bedrock"]
env = { AWS_REGION = "..." }

# Local layer adds an env var to a school-declared custom backend
[[backends]]
name = "bailer"
env = { API_TOKEN = "..." }
```

## Skills Selection

Three fields control which of the school's skills the backend loads at session start.
Resolution is two-stage: each field merges across the three scopes per its own rule,
then the merged values combine via `(skills − exclude_skills) ∪ include_skills`.

### Per-field merge

- `skills` — last-wins replace (local > project > user, first non-empty). Empty at
  every scope leaves the base set as "all discovered skills."
- `include_skills` — union of all three scopes, dedup, order preserved
  (user → project → local on first occurrence).
- `exclude_skills` — same merge as `include_skills`.

`skills` follows the standard last-wins rule. `include_skills` and `exclude_skills`
are the documented exceptions: they exist precisely to add or remove guarantees
on top of whichever `skills` value won, so unioning across scopes is the point.

### Resolution

```
effective = (skills_base − exclude_skills) ∪ include_skills
```

Exclude is applied before include, so include is authoritative when an item appears
in both: a skill explicitly named in `include_skills` will be loaded even if a
matching pattern in `exclude_skills` would have removed it.

### Use cases

| Want | Where | What |
|---|---|---|
| Always add `issue-*` globally | user | `include_skills = ["issue-*"]` |
| This repo only needs rust-coding | project | `skills = ["rust-coding"]` |
| Replace project's choice on this machine | local | `skills = ["debug-*"]` |
| Skip a global include in this repo | local | `exclude_skills = ["issue-*"]` |
| Add a skill on this machine only | local | `include_skills = ["debug-tools"]` |

### Empty vs missing

`skills = []` and an absent `skills` key are equivalent — both mean "this scope
contributes nothing." Same for `include_skills` and `exclude_skills`.

### Warnings

Detected during resolution against the discovered skill set:

- **Same-scope `include_skills` ∩ `exclude_skills` collision** — both fields in
  the same file end up matching the same resolved skill. Almost always a typo.
  Distinct from cross-scope collision (user includes, local excludes), which is
  the feature.
- **Unknown skill patterns** — a pattern in any of the three fields matches no
  discovered skill. Likely typo or stale config.
- **`skills` filter active without project contribution** — effective `skills`
  base is non-empty but the project scope contributed nothing to it. User or
  local scope narrowed what the school would have shipped. Suppressed when
  project also contributes (the project author's curation is intentional).

### CLI

```
ace skills [--all] [--names]              # list resolved skills (default: hide excluded)
ace skills include <pattern>...           # append to include_skills
ace skills exclude <pattern>...           # append to exclude_skills
ace skills reset [--include] [--exclude]  # set list(s) back to empty; bare = both
ace explain <name>                        # provenance + per-step trace for one skill
```

`include`/`exclude`/`reset` write to project scope by default; pass the global
`--user` or `--local` flag to target another layer. Patterns are validated up
front (`*` only — `**`, `?`, and `[...]` are rejected).

There is intentionally no `ace skills set <pattern>...` verb. The `skills`
field (the last-wins whitelist) is config-only — edit `ace.toml` directly.
The CLI exposes the union-merge fields (`include_skills`, `exclude_skills`)
because those are the ones a user typically tweaks per session.

## Scope Flags

Write commands (`ace config set`, `ace auto`, `ace yolo`) accept a scope flag to choose
which layer to write to:

- `--user` (alias: `--global`) — write to user-level `~/.config/ace/ace.toml`
- `--project` — write to project `ace.toml`
- `--local` — write to `ace.local.toml`

When no scope flag is given, the default is inferred from the key:

- Personal-only fields (`trust`, `resume`) → `--local`
- Shared fields (`school`, `backend`, `session_prompt`, `env.*`, `skip_update`) → `--project`

An explicit scope flag always overrides inference.

## Config Commands

### `ace config`

Bare `ace config` prints the effective resolved configuration (all layers merged).

### `ace config get <key>`

Print the effective resolved value for a single key. Outputs the raw value, one line.

Keys: `school`, `backend`, `trust`, `resume`, `session_prompt`, `skip_update`, `env.KEY`.

### `ace config explain [key]`

Print provenance per layer for one or all keys. Bare form lists every key; pass a key
name (e.g. `backend`, `trust`, `env.FOO`) to filter to one block.

Each block shows the resolved winner with its source label, then a per-layer breakdown
(`user`/`project`/`local`/`school`/`override`). The winning layer is marked `← winner`.
When no layer contributes a value (winner is `default`), the block collapses to a single
line.

```
backend = "bailer"  [project]
  user:     (unset)
  project:  "bailer"  ← winner
  local:    (unset)
  school:   (unset)
  override: (unset)

trust = "default"  [default]
```

The breakdown shows the raw value present in each file. For personal-only keys
(`trust`, `resume`), the merge ignores the project layer — a value listed under
`project:` for those keys is informational only and does not influence the winner.

### `ace config set <key> <value> [--user|--project|--local]`

Write a single field to the appropriate layer. Loads the target file, modifies the field,
saves back. Other fields in that file are preserved.

Key syntax:
- Simple fields: `backend`, `school`, `trust`, `resume`, `session_prompt`, `skip_update`
- Env map entries: `env.KEY` — dot-path into the `[env]` table (e.g. `ace config set env.ANTHROPIC_API_KEY sk-...`)

## Loading vs Validation

Config loading and config validation are separate concerns.

### Loading

Serde handles deserialization only. All config structs use `#[derive(Default)]` and
`#[serde(default)]` at the struct level. Every field parses successfully regardless of what's
present in the TOML — missing keys get their type's `Default` value (empty string, empty vec,
`None`, etc.).

This means a TOML with no `name` key produces `SchoolToml { name: "".into(), .. }`
rather than a serde error. Partial or empty files always parse.

### Validation

After loading, a separate validation pass checks invariants and produces clear, actionable errors.
Validation runs on the merged config (after all three layers are combined), not on individual files.

Rules are expressed in code, not via serde attributes. Examples:

- `name` (top-level) must be non-empty.
- No duplicate `mcp[].name` entries.
- `projects[].repo` must be a valid specifier.

Validation errors reference the offending key path: e.g. `name: must not be empty`,
`mcp[0].name: duplicate entry`.

### Why

- Serde's "missing field" errors are opaque and unhelpful to users.
- Required-vs-optional is a validation concern, not a parsing concern.
- Validation on the merged config catches cross-layer issues (e.g. a layer overrides a field to
  an invalid value).
- Richer checks (URL format, uniqueness, non-empty) cannot be expressed through serde alone.

## Placeholder Substitution

Config string values may contain `{{ name }}` placeholders that are resolved at runtime by
prompting the user. This is a general-purpose mechanism — currently used by MCP header values
(see [mcp.md](mcp.md)) but available wherever user-specific values are needed.

### Syntax

- `{{ name }}` — placeholder, resolved by prompting the user.
- Whitespace inside braces is flexible: `{{name}}`, `{{ name }}`, `{{  name  }}` all match.
- Name must be `[a-zA-Z0-9_]+`.
- Literal `{{` that should not be treated as placeholders: not supported yet (no escaping).

### Engine

Hand-rolled 4-state parser (Text → MaybeOpen → Name → MaybeClose). Two pure functions:

- `extract_placeholders(input) -> Vec<String>` — returns unique placeholder names in
  order of first appearance.
- `substitute(input, values) -> String` — replaces each `{{ name }}` with the corresponding
  value from the map. Missing keys resolve to empty string.

No regex dependency. Lives in its own module (`src/template.rs` or similar), independent of
config or MCP logic.

### Future

Current engine is intentionally minimal. May be replaced with a mature template engine
(Jinja-compatible, Go template-compatible, etc.) if more complex substitution needs arise.
