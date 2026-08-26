# Configuration Management

## Format

TOML.

## Layers

Resolved by merging (later overrides earlier):

1. **User** `~/.config/ace/ace.toml` (or `$XDG_CONFIG_HOME/ace/ace.toml`) — personal
   defaults across all projects
2. **Project** `ace.toml` (checked into git, shared with team)
3. **Local** `ace.local.toml` (gitignored, per-machine overrides)

Any one layer is enough. A user-level `ace.toml` alone resolves a workdir that has no
`ace.toml` and no `ace.local.toml` of its own — the user layer is the default for every
project that does not override it. Only the absence of all three is "no config found".

### Fields

Each layer can set:

- `school` — school specifier (last non-empty wins)
- `backend` — backend name (highest-priority `Some` wins: local → project → user →
  school's `school.toml` → fallback `claude`). Built-ins: `"claude"`, `"codex"`,
  `"opencode"` (plus the debug-only `"flaude"` test fixture, absent from release builds).
  Custom names are valid when declared in `[backends.<name>]` (see
  [Custom backends](#custom-backends)). See [backend.md](backend.md).
- `session_prompt` — additional prompt text (last non-empty wins)
- `env` — environment variables (additive merge, later keys override)
- `skip_update` — disable automatic version check and background upgrade. Default: `false`.
  See [upgrade.md](upgrade.md). Also overridden by `ACE_SKIP_UPDATE=1` env var.
- `skills` — per-project skill whitelist. Last-wins replace across scopes; empty at all
  scopes = "all skills". See [Skills selection](#skills-selection).
- `include_skills` — always-add skill patterns. **Union across all scopes** (exception to
  last-wins). See [Skills selection](#skills-selection).
- `exclude_skills` — always-remove skill patterns. **Union across all scopes** (exception
  to last-wins). See [Skills selection](#skills-selection).
- `exclude_mcp` — MCP server names to skip registering. **Union across all scopes**, same
  exception as the skill patterns. Written by answering "no" to a registration prompt;
  cleared by `ace mcp register <name>`. See [mcp.md](mcp.md).
- `[connect]` — connected-session settings. `enabled` is last-wins across user, project,
  and local layers; default `false`. See [connect.md](connect.md).

### Personal-only fields

These fields are resolved from the **user** and **local** layers only — never from
project-committed `ace.toml` or `school.toml`. They are personal workflow preferences.

- `trust` — permission mode: `"default"`, `"auto"`, or `"yolo"`. Default: `"default"`.
- `resume` — auto-resume previous session on `ace` launch. Default: `true`. When `true`,
  `ace` passes resume flags to the backend if a previous session exists for the current
  project directory. The `ace new` subcommand forces a fresh session regardless. Backends
  that don't support resume start fresh silently.

Resolution for personal-only fields: local wins over user. Project layer is skipped
entirely.

## Connected sessions

```toml
[connect]
enabled = true
```

`enabled` requests connect-compatible startup through bare `ace`; it does not select a
different command. Local wins over project, which wins over user. Workspace mode may add
the same requirement to its member plans at runtime without writing the child configs.

The table is intentionally narrow. Relay identity defaults from the ACE instance and is
supplied by workspace membership when applicable; transport paths, backend session IDs,
tmux names, and process IDs are runtime state rather than user configuration.

## Custom backends

`[backends.<name>]` tables seed or augment the backend registry. Built-ins (`claude`,
`codex`, `opencode`, and the debug-only `flaude`) are pre-registered; matching keys
override their settings, while new keys reuse an existing built-in's behavior (its
*kind*). The legacy `[[backends]]` array shape is not accepted.

### Fields

- Table key — registry key. May match a built-in (override) or be new. Quote names that
  are not bare TOML keys, for example `[backends."bedrock-claude"]`.
- `kind` — built-in name whose behavior to reuse (`"claude"`, `"codex"`, `"opencode"`).
  Optional.
- `cmd` — argv for launching the binary. `cmd[0]` is the program; `cmd[1..]` are prepended
  to runtime args. Optional.
- `env` — environment variables merged into the launched process. Optional.
- `model` — opaque backend-native model name applied to every ACE-owned model invocation.
- `effort` — opaque backend-native effort value applied to every ACE-owned model
  invocation.

### Layer order

`[backends.<name>]` may appear in `school.toml` and in any `ace.toml` /
`ace.local.toml` layer. Resolution walks **built-ins → school → user → project → local**,
applying each keyed patch in order.

### Resolution rules

For each keyed patch:

- **Name already registered** (built-in or earlier-layer custom) — partial override:
  - `env` per-key last-wins.
  - `cmd` last-wins-non-empty (empty `cmd` does not clobber a prior value).
  - `model` and `effort` each last-win when present.
  - `kind`, if specified, must match the existing entry's kind. Mismatch errors with
    `BackendError::KindMismatch`.
- **New name** — kind is resolved by trying:
  1. Explicit `kind` field.
  2. Table key matching a built-in name.
  3. `cmd[0]` basename matching a built-in name.
  4. Otherwise: error `BackendError::Unresolvable`.

  Then `cmd` defaults to `[kind.name()]` if not given.

### Selecting a custom backend

Once registered, a custom name is selectable like a built-in:

- `backend = "bailer"` in any `ace.toml` layer.
- `--backend bailer` on the CLI (or `ace config set backend bailer`).

Unknown names error with `BackendError::Unknown` at resolve time.

### Examples

```toml
# Tweak built-in claude's env
[backends.claude]
env = { ANTHROPIC_LOG = "debug" }
model = "opus"
effort = "high"

# Custom backend reusing claude's binary
[backends.bailer]
kind = "claude"
env = { ANTHROPIC_BASE_URL = "..." }

# Custom backend with a forked binary; kind inferred from cmd[0] basename
[backends."bedrock-claude"]
cmd = ["claude-bedrock"]
env = { AWS_REGION = "..." }

# Local layer adds an env var to a school-declared custom backend
[backends.bailer]
env = { API_TOKEN = "..." }
```

## Skills Selection

Three fields control which of the school's skills the backend loads at session start.
Resolution is two-stage: each field merges across the three scopes per its own rule, then
the merged values combine via `(skills − exclude_skills) ∪ include_skills`.

### Per-field merge

- `skills` — last-wins replace (local > project > user, first non-empty). Empty at every
  scope leaves the base set as "all discovered skills."
- `include_skills` — union of all three scopes, dedup, order preserved (user → project →
  local on first occurrence).
- `exclude_skills` — same merge as `include_skills`.

`skills` follows the standard last-wins rule. `include_skills` and `exclude_skills` are
the documented exceptions: they exist precisely to add or remove guarantees on top of
whichever `skills` value won, so unioning across scopes is the point.

### Resolution

```
effective = (skills_base − exclude_skills) ∪ include_skills
```

Exclude is applied before include, so include is authoritative when an item appears in
both: a skill explicitly named in `include_skills` will be loaded even if a matching
pattern in `exclude_skills` would have removed it.

### Use cases

| Want                                     | Where   | What                               |
| ---------------------------------------- | ------- | ---------------------------------- |
| Always add `issue-*` globally            | user    | `include_skills = ["issue-*"]`     |
| This repo only needs rust-coding         | project | `skills = ["rust-coding"]`         |
| Replace project's choice on this machine | local   | `skills = ["debug-*"]`             |
| Skip a global include in this repo       | local   | `exclude_skills = ["issue-*"]`     |
| Add a skill on this machine only         | local   | `include_skills = ["debug-tools"]` |

### Empty vs missing

`skills = []` and an absent `skills` key are equivalent — both mean "this scope
contributes nothing." Same for `include_skills` and `exclude_skills`.

### Warnings

Detected during resolution against the validated skill set:

- **Same-scope `include_skills` ∩ `exclude_skills` collision** — both fields in the same
  file end up matching the same resolved skill. Almost always a typo. Distinct from
  cross-scope collision (user includes, local excludes), which is the feature.
- **Unknown skill patterns** — a pattern in any of the three fields matches no validated
  skill. Likely typo or stale config.
- **`skills` filter active without project contribution** — effective `skills` base is
  non-empty but the project scope contributed nothing to it. User or local scope narrowed
  what the school would have shipped. Suppressed when project also contributes (the
  project author's curation is intentional).

### CLI

```
ace skills [--all] [--names]              # list resolved skills (default: hide excluded)
ace skills include <pattern>...           # append to include_skills
ace skills exclude <pattern>...           # append to exclude_skills
ace skills reset [--include] [--exclude]  # set list(s) back to empty; bare = both
ace explain <name>                        # provenance + per-step trace for one skill
```

`include` /`exclude`/`reset` write to project scope by default; pass the global `--user`
or `--local` flag to target another layer. Pattern semantics are documented in
[skills/selection.md → Match handle](skills/selection.md#match-handle): bare names match
exact-or-leaf, paths anchored at `/`, `*` matches any chars, `**` is accepted but
behaves like `*`, `?` and character classes are not supported.

There is intentionally no `ace skills set <pattern>...` verb. The `skills` field (the
last-wins whitelist) is config-only — edit `ace.toml` directly. The CLI exposes the
union-merge fields (`include_skills`, `exclude_skills`) because those are the ones a user
typically tweaks per session.

## Scope Flags

Write commands (`ace config set`, `ace auto`, `ace yolo`) accept a scope flag to choose
which layer to write to:

- `--user` (alias: `--global`) — write to user-level `~/.config/ace/ace.toml`
- `--project` — write to project `ace.toml`
- `--local` — write to `ace.local.toml`

When no scope flag is given, the default is inferred from the key:

- Personal-only fields (`trust`, `resume`) → `--local`
- Shared fields (`school`, `backend`, `session_prompt`, `env.*`, `skip_update`,
  `connect.enabled`) → `--project`

An explicit scope flag always overrides inference.

## Config Commands

### `ace config`

Bare `ace config` prints the effective resolved configuration (all layers merged).

### `ace config get <key>`

Print the effective resolved value for a single key. Outputs the raw value, one line.

Keys: `school`, `backend`, `trust`, `resume`, `session_prompt`, `skip_update`,
`connect.enabled`, `env.KEY`.

### `ace config explain [key]`

Print provenance per layer for one or all keys. Bare form lists every key; pass a key name
(e.g. `backend`, `trust`, `env.FOO`) to filter to one block.

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

The breakdown shows the raw value present in each file. For personal-only keys (`trust`,
`resume`), the merge ignores the project layer — a value listed under `project:` for those
keys is informational only and does not influence the winner.

### `ace config set <key> <value> [--user|--project|--local]`

Write a single field to the appropriate layer. Loads the target file, modifies the field,
saves back. Other fields in that file are preserved.

Key syntax:
- Simple fields: `backend`, `school`, `trust`, `resume`, `session_prompt`, `skip_update`
- Connected-session field: `connect.enabled`
- Env map entries: `env.KEY` — dot-path into the `[env]` table (e.g.
  `ace config set env.ANTHROPIC_API_KEY sk-...`)
- Backend instance fields: `backends.<instance>.model` and
  `backends.<instance>.effort`. The instance may name a built-in or custom backend and
  may itself contain dots; parsing is anchored by the `backends.` prefix and terminal
  field. Values are opaque strings and are written unchanged. These shared fields default
  to project scope, and explicit scope flags retain their usual precedence. No other
  `[backends.<instance>]` field is writable through `ace config set`.

## Loading vs Validation

Config loading and config validation are separate concerns.

### Loading

Serde handles deserialization only. All config structs use `#[derive(Default)]` and
`#[serde(default)]` at the struct level. Every field parses successfully regardless of
what's present in the TOML — missing keys get their type's `Default` value (empty string,
empty vec, `None`, etc.).

This means a TOML with no `name` key produces `SchoolToml { name: "".into(), .. }` rather
than a serde error. Partial or empty files always parse.

### Validation

After loading, a separate validation pass checks invariants and produces clear, actionable
errors. Validation runs on the merged config (after all three layers are combined), not on
individual files.

Rules are expressed in code, not via serde attributes. Examples:

- `name` (top-level) must be non-empty.
- No duplicate `mcp[].name` entries.
- `projects[].repo` must be a valid specifier.

Validation errors reference the offending key path: e.g. `name: must not be empty`,
`mcp[0].name: duplicate entry`.

### Why

- Serde's "missing field" errors are opaque and unhelpful to users.
- Required-vs-optional is a validation concern, not a parsing concern.
- Validation on the merged config catches cross-layer issues (e.g. a layer overrides a
  field to an invalid value).
- Richer checks (URL format, uniqueness, non-empty) cannot be expressed through serde
  alone.

## Placeholder Substitution

Config string values may contain `{{ name }}` placeholders that are resolved at runtime by
prompting the user. This is a general-purpose mechanism — currently used by MCP header
values (see [mcp.md](mcp.md)) but available wherever user-specific values are needed.

### Syntax

- `{{ name }}` — placeholder, resolved by prompting the user.
- Whitespace inside braces is flexible: `{{name}}`, `{{ name }}`, `{{ name }}` all match.
- Name must be `[a-zA-Z0-9_]+`.
- Literal `{{` that should not be treated as placeholders: not supported yet (no
  escaping).

### Engine

Hand-rolled 4-state parser (Text → MaybeOpen → Name → MaybeClose), exposed as a parsed
template value with two operations:

- **placeholders** — the unique placeholder names, in order of first appearance.
- **substitute** — replaces each `{{ name }}` with the corresponding value from the
  supplied map. Missing keys resolve to empty string.

No regex dependency. Lives in its own module, independent of config or MCP logic.

### Future

Current engine is intentionally minimal. May be replaced with a mature template engine
(Jinja-compatible, Go template-compatible, etc.) if more complex substitution needs arise.
