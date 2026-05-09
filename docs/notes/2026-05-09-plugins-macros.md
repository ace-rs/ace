_Predates the `docs/notes/` convention; preserved as-is for historical context._

# Plugins & Macros

## Problem

ACE orchestrates the environment but offers no extension points between "prepare school" and
"exec backend." Schools provide static assets (skills, rules, commands, agents) and config
(env, MCP, session_prompt) — but cannot run code, define lifecycle hooks, or provide shorthand
for common workflows.

Two gaps:

1. **Plugins** — executable behavior at ACE lifecycle points (setup, prepare, pre-exec).
   Example: a school wants to run `npm install` after linking, verify tool versions, or
   seed a database before launching the backend.

2. **Macros** — named shortcuts that expand to backend invocations with pre-filled arguments,
   env overrides, or session prompt fragments. Example: `ace --macro review` expands to
   `ace -- --continue` with a "code review" session_prompt prepended.

## Design Principles

- **Schools own both.** Plugins and macros live in the school repo alongside skills/rules.
  No new distribution mechanism — same git clone + symlink model.
- **Shell scripts, not Rust.** Plugins are executable files. ACE invokes them with a defined
  environment. No dynamic linking, no trait objects, no WASM.
- **Opt-in, not mandatory.** A school with no `plugins/` or `macros/` works exactly as today.
- **Fail-open by default, fail-closed on request.** Hooks log warnings on failure unless the
  plugin is marked `required`.

## Plugins

### Location

```
school/
  plugins/
    pre-setup.sh        # runs before ace setup writes config
    post-setup.sh       # runs after ace setup completes
    post-prepare.sh     # runs after install/update/link, before exec
    pre-exec.sh         # runs immediately before exec'ing the backend
```

Filenames are fixed — one script per hook point. Only executable files are invoked.
Non-executable files are silently skipped (allows README, etc.).

### Lifecycle Hooks

| Hook              | When                                              | Blocking | Can fail setup |
|-------------------|---------------------------------------------------|----------|----------------|
| `pre-setup`       | After `ace setup` validates git, before writing    | Yes      | Yes            |
| `post-setup`      | After `ace setup` finishes all steps               | Yes      | No (warn only) |
| `post-prepare`    | After install/update + link + MCP register         | Yes      | Configurable   |
| `pre-exec`        | After session prompt is built, before `exec()`     | Yes      | Configurable   |

All hooks run synchronously. ACE waits for the process to exit before continuing.

### Execution Environment

Plugins receive context via environment variables — no stdin, no arguments.

| Variable              | Value                                          |
|-----------------------|------------------------------------------------|
| `ACE_PROJECT_DIR`     | Absolute path to the project root              |
| `ACE_SCHOOL_ROOT`     | Absolute path to the school (cache or local)   |
| `ACE_BACKEND`         | `claude`, `opencode`, or `codex`               |
| `ACE_BACKEND_DIR`     | Backend config dir name (e.g. `.claude`)       |
| `ACE_SPECIFIER`       | School specifier (e.g. `org/school`)           |
| `ACE_HOOK`            | Hook name (e.g. `post-prepare`)                |

Plus any `[env]` vars from the merged config.

Working directory is set to `ACE_PROJECT_DIR`.

### Failure Handling

- Exit 0 → success, continue.
- Exit non-zero → depends on the hook:
  - `pre-setup`: always fatal (abort setup).
  - `post-setup`: always warn-only.
  - `post-prepare`, `pre-exec`: warn by default. Fatal if the script's filename is
    listed in `school.toml` under `required_plugins`.

```toml
# school.toml
required_plugins = ["post-prepare"]
```

### Timeout

Plugins get 30 seconds by default. Configurable per-school:

```toml
# school.toml
[plugins]
timeout = 60  # seconds
```

ACE kills the process and warns (or errors, if required) on timeout.

### Output

Plugin stdout/stderr are forwarded to ACE's output. In porcelain mode, stdout is prefixed
with `plugin:` for machine parseability.

### Symlinking

`plugins/` is NOT symlinked into the project (unlike skills/rules/commands/agents). Plugins
run from the school clone directly. This keeps the project clean — plugins are school
infrastructure, not project content.

## Macros

### Concept

A macro is a named shortcut that pre-fills backend arguments, environment variables, and/or
session prompt text. Macros are defined in TOML and invoked via `ace run <name>` (or
`ace -m <name>` shorthand).

### Location

Macros can be defined at any config layer:

```toml
# school.toml, ace.toml, or ace.local.toml
[[macro]]
name = "review"
session_prompt = "You are doing a code review. Focus on correctness and security."
backend_args = ["--continue"]

[[macro]]
name = "test"
session_prompt = "Run the test suite and fix any failures."
env = { CI = "true" }

[[macro]]
name = "quick"
backend_args = ["--model", "sonnet"]
```

### Schema

```toml
[[macro]]
name = "review"                    # required — unique identifier, [a-z0-9-]
session_prompt = "..."             # optional — prepended to the session prompt
backend_args = ["--continue"]      # optional — appended to backend args
env = { KEY = "value" }            # optional — merged into env (macro wins)
```

### Merge Semantics

Macros follow the same override precedence as other config: user → project → local.
If the same `name` appears in multiple layers, the highest-priority layer wins entirely
(no field-level merge within a single macro — replace semantics).

School-defined macros are available to all projects using that school. Project-level macros
can override or add to them.

### Invocation

```sh
ace run review              # invoke the "review" macro
ace run quick -- --verbose  # macro args + extra backend args (appended after macro's)
ace run --list              # list available macros with descriptions
```

`ace run <name>` is equivalent to running bare `ace` with the macro's session_prompt
prepended, its backend_args appended, and its env merged.

### Interaction with Plugins

Macros set `ACE_MACRO=<name>` in the plugin environment, so plugins can behave differently
per macro if needed.

### No Shell Expansion

Macro values are literal. No template substitution, no shell expansion. This keeps them
predictable and safe.

## Implementation Scope

### New Files

| File                            | Layer   | Purpose                              |
|---------------------------------|---------|--------------------------------------|
| `src/config/macro_decl.rs`      | Config  | `MacroDecl` serde struct             |
| `src/state/actions/plugin.rs`   | Actions | `RunPlugin` action (fork + exec)     |
| `src/cmd/run.rs`                | Cmd     | `ace run <name>` subcommand          |

### Modified Files

| File                            | Change                                         |
|---------------------------------|------------------------------------------------|
| `src/config/school_toml.rs`     | Add `required_plugins`, `plugins` table        |
| `src/config/ace_toml.rs`        | Add `macro` array field                        |
| `src/state/mod.rs`              | Merge macros during resolution                 |
| `src/cmd/mod.rs`                | Add `Run` variant to `Command` enum            |
| `src/cmd/main.rs`               | Call plugin hooks around prepare/exec           |
| `src/state/actions/exec.rs`     | Accept macro overrides                         |
| `src/state/actions/prepare.rs`  | Call post-prepare hook                         |
| `src/state/actions/setup.rs`    | Call pre/post-setup hooks                      |
| `src/templates/session.rs`      | Prepend macro session_prompt                   |

### Not Changed

- `src/config/tree.rs` — macros load via existing `AceToml` / `SchoolToml` parse
- `src/state/actions/link.rs` — `plugins/` is not a linkable folder
- `docs/spec/architecture.md` — no layer boundary changes (plugins are actions, macros are config)

## What This Does NOT Cover

- **Plugin discovery / registry** — no `ace plugin install`. Plugins live in the school.
- **Plugin dependencies** — no ordering between plugins beyond the fixed hook points.
- **Macro composition** — no chaining macros. One macro per invocation.
- **Remote plugins** — no downloading plugins from URLs. Same git-clone model as schools.
- **Per-project plugin overrides** — project can't disable a school plugin (yet). If needed
  later, a `disabled_plugins = ["post-prepare"]` field in `ace.toml` is the obvious path.

## Examples

### School with a post-prepare plugin

```sh
# school/plugins/post-prepare.sh
#!/bin/sh
set -e
cd "$ACE_PROJECT_DIR"
[ -f package.json ] && npm install --silent
[ -f Cargo.toml ] && cargo check --quiet 2>/dev/null || true
```

### Project-local macro

```toml
# ace.local.toml
[[macro]]
name = "fix"
session_prompt = "Find and fix the bug described in the latest Linear issue assigned to me."
backend_args = ["--continue"]
```

### Invoking

```sh
ace run fix          # launches backend with the "fix" prompt prepended
ace run fix -- -p    # same, plus --print mode
ace run --list       # shows: fix, review, test, quick
```
