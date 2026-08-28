# Backend Configuration

## Backend Enum

| Value      | Binary     | Backend Dir | Instructions File | Details                                      |
| ---------- | ---------- | ----------- | ----------------- | -------------------------------------------- |
| `claude`   | `claude`   | `.claude`   | `CLAUDE.md`       | [backends/claude.md](backends/claude.md)     |
| `codex`    | `codex`    | `.agents`   | `AGENTS.md`       | [backends/codex.md](backends/codex.md)       |
| `opencode` | `opencode` | `.opencode` | `AGENTS.md`       | [backends/opencode.md](backends/opencode.md) |
| `flaude`   | `flaude`   | `.claude`   | `CLAUDE.md`       | Test-only fixture backend (see testing.md)   |

### Project path surface

The backend root is only the first component of ACE's project path surface:

| Backend kind | Root | ACE-linked project folders | Backend-native folders ACE does not link |
| ------------ | ---- | -------------------------- | ----------------------------------------- |
| `claude` | `.claude` | `skills/`, `rules/`, `commands/`, `agents/` | — |
| `codex` | `.agents` | canonical school folders for compatibility; Codex natively consumes `skills/` | user/config rules, prompts, and agents |
| `opencode` | `.opencode` | `skills/`, `commands/`, `agents/` | `rules/` |
| `flaude` | `.claude` | same project paths as Claude | — |

These are on-disk names, not capability labels. A future backend-specific rename must be
exposed by the backend path surface and consumed by linking, discovery, and gitignore
generation; those consumers must not grow backend-name conditionals or duplicate mappings.

## TOML Syntax

```toml
backend = "claude"
```

Valid in `~/.config/ace/ace.toml` (user), `ace.toml` (project), `ace.local.toml` (local),
and `school.toml` (top-level field, not a `[school]` table).

## Resolution Order

First `Some` wins in this priority order (highest to lowest):

1. CLI override — `ace --backend <name>`, `ace -b <name>`, or convenience flags such as
   `ace --claude` / `ace --codex`
2. Local — `ace.local.toml`
3. Project — `ace.toml`
4. User — `~/.config/ace/ace.toml`
5. `school.toml` — the linked school's default

Fallback if no layer specifies backend: `claude`.

The CLI override is runtime-only. It does not write any config file and applies to
backend-dependent commands generally, including bare `ace`, `ace mcp`, `ace config`,
`ace setup`, and `ace pull`.

## Backend Contract

Each backend must provide:

- **`binary()`** — executable name on `$PATH`, used for exec.
- **`backend_dir()`** — project directory where school folders are linked.
- **`instructions_file()`** — markdown file generated per-project during setup.
- **`is_ready()`** — heuristic check that the backend is authenticated/configured.
- **`supports_trust(trust)`** — whether the backend can honour the given trust level.
  Independent of whether it emits flags for it (flaude honours every level yet emits
  none). Before launching a session, ACE checks this and announces an unsupported
  level to the user — the backend runs with its own default permissions; the level is
  never dropped silently.
- **`exec_session(options)`** — launch an interactive backend session via exec-replace.
  Builds one typed `Component` from `SessionOptions` (trust, session prompt, project dir,
  env, extra args, typed resume mode, backend mode), then converts that component to a
  process command at the execution edge. Returns `io::Error` on spawn failure; never
  returns on success (terminal hands off to the child). In `Latest` mode, some backends
  may fail if no prior session exists (Claude) while others handle it gracefully (Codex).
  ACE prints a hint before exec so the user knows to run `ace new` on failure. See
  [backends/claude.md → Session Resume](backends/claude.md#session-resume).
- **`exec_one_shot(options)`** — spawn the backend non-interactively and capture
  stdout/stderr. Builds its Command from `OneShotOptions` (prompt source, project dir,
  env, extra args; no resume, trust, or session prompt — the non-interactive entry point
  doesn't take approval modes or system-prompt injection). Returns
  `io::Result<std::process::Output>` — caller inspects `status.success()` and `stderr` for
  non-zero exits. `ace -p` uses this captured form; model-driven backend operations that
  require live diagnostics must use an execution path that preserves terminal output.
  See § Intent Mapping below.
- **`mcp_list()`** — list currently registered MCP server names.
- **`mcp_add(entry)`** — register a remote MCP server.
- **`mcp_remove(name)`** — unregister a remote MCP server by name.
- **`mcp_check(names)`** — runtime usability check for registered MCP servers. This is not
  a static config parse — the backend executes a one-shot prompt that exercises each
  server from inside the backend's own environment (auth state, token storage, MCP
  client). Returns a list of name/ok pairs. Best-effort: returns empty on failure or if
  unsupported.

See per-backend specs for implementation details.

### Managed-session contract

`exec_session` is the implemented single-component transport. `Ace::start` supplies a
`BackendMode`; managed sessions compose the same component type into a backend-owned
graph.

The graph contains named process roles, dependencies, environment, working directories,
and the handles a backend can expose for its native session and primary thread. The local
executor may preserve exec-replace for a one-component foreground graph; mux executes a
multi-component graph without learning backend names.

Connect selects `BackendMode::WithServer` before materialization.
Each backend either produces a sanctioned receive component and primary-session target or
reports the requirement unsupported. No caller branches on `Kind` to construct Codex or
OpenCode process topology.

Backend capabilities describe facts, including controlled startup, primary-session
input, thread listing, and native resume. They do not promise task tracking, generic
wake-idle behavior, or a shared subagent model. See [session.md](session.md) and
[connect.md](connect.md).

## Intent Mapping

`exec_session` and `exec_one_shot` are the two transport methods — deliberately two, not
one `exec(Intent)`: the return types differ fundamentally (never-returns vs captured
`Output`), and a unified signature would lie about that at the type level. Each backend
builds its argv from the matching options type. The argv builder is the polymorphic core;
the session path carries it in a component before exec-replace, while one-shot builds and
captures its subprocess directly.

`ace -p` routes through `exec_one_shot`, which captures then prints after the child
exits. That buffering belongs to the user-requested one-shot surface only. Model-driven
internal operations may not reuse it when doing so would hide warnings, progress, or
prompts; they must preserve live backend output through the resolved `Backend` boundary.

### Per-Backend Argv

| Intent          | claude                                                 | codex                                                     |
| --------------- | ------------------------------------------------------ | --------------------------------------------------------- |
| Session         | `--system-prompt <prompt>` (or `--continue` if resume) | `-c developer_instructions=<prompt>` (or `resume --last`) |
| OneShot, Inline | `-p <text>`                                            | `exec <text>`                                             |
| OneShot, Stdin  | `-p` + piped child stdin                               | `exec -` + piped child stdin                              |

Trust flags (`--permission-mode` / `--ask-for-approval` / sandbox) attach to Session only.
OneShot is non-interactive — approval modes don't apply.

### Prompt Source

`OneShotOptions.prompt: PromptInput` is `Inline(String)` for argv-passed prompts, `Stdin`
for piped stdin. Backends translate per the table above. When `Stdin`, the spawned child
inherits the parent's stdin (`Stdio::inherit()`); the caller must arrange the piped data
themselves.

Launch-domain values use `Mode` or `Options`; `Request` is network-protocol terminology
and is not used for in-process launch configuration.

## MCP Server Registration

ACE registers `[[mcp]]` entries from `school.toml` into the active backend. All entries
are remote MCP endpoints — see [mcp.md](mcp.md) for the remote-only design rationale.

**Strategy: CLI-first.** Prefer invoking the backend's CLI to add MCP servers. Only fall
back to writing config files when the CLI cannot express the needed configuration cleanly.

ACE owns registration into the backend. Backend-native auth and MCP management should
remain in the backend wherever possible.

## Linked Folders

ACE links school folders (`skills/`, `rules/`, `commands/`, `agents/`) into the project's
backend directory. Not all backends support every folder — see per-backend specs for the
support matrix.

Some backends may use different directory names for linked folders. The Link action
handles remapping when needed.

## Session Prompt

Backends receive the session prompt via their native invocation surface. For some backends
this is a CLI flag such as `--system-prompt`; for others it is an initial positional
prompt. See per-backend specs for the exact delivery mechanism.

## Readiness Check

Backends may expose an `is_ready()` heuristic so ACE can warn or gate execution when the
backend is clearly not initialized. Whether ACE should enforce readiness before exec is a
product decision and may vary by backend or evolve over time.

## Custom Backends

The `[backends.<name>]` table lets a school, user, or project configure a built-in or
register an additional backend name. A custom backend is **not** a new `Kind` — it is a
named instance that aliases a built-in `Kind`. The backend contract (MCP, readiness,
instructions file, linked-folder layout) is inherited from the aliased `Kind`.

### TOML Syntax

```toml
[backends.bailer]           # key is selectable via `backend = "bailer"` or `-b bailer`
kind = "claude"             # optional — see kind resolution below
cmd = ["claude"]            # optional — argv for launch; defaults to [kind.name()]
env = { ANTHROPIC_BASE_URL = "https://proxy.example.com" }
model = "opus"              # optional opaque backend-native value
effort = "high"             # optional opaque backend-native value
```

Valid in `school.toml`, user, project, and local config. The legacy `[[backends]]` array
shape is invalid; backend identity lives only in the table key.

### Kind Resolution

For a new name, `kind` is resolved in order:

1. Explicit `kind = "..."` field (must be a built-in name).
2. The table key matches a built-in name → that kind.
3. `cmd[0]` basename matches a built-in name → that kind.
4. Otherwise → `BackendError::Unresolvable`.

For a name that already exists (built-in or earlier-layer custom), the decl partially
overrides the existing entry: `env` merges per-key; `cmd`, `model`, and `effort` are
last-wins when present; and a declared `kind` must match the existing kind (otherwise
`BackendError::KindMismatch`).

### Layer Merge

Declarations are folded into a registry seeded with built-ins, in layer order: school →
user → project → local. Later layers may add new entries or partially override earlier
ones. The selected backend name (resolved per the [Resolution Order](#resolution-order)
above) is then looked up in the final registry; an unknown name is `BackendError::Unknown`.

Session-start sites recover from `Unknown` interactively: on a TTY, a picker over the
registry names, then re-resolve with the pick as a runtime override and print
`to make permanent: ace config set backend <pick>`; without a TTY, hard fail with the
same hint inlined. Never a silent fallback — a wrong-backend run could route prompts to
the wrong vendor, which is worse than failing.

### Path Templating

`cmd[]` entries and `env` values may use `{{ ... }}` placeholders, rendered at bind time.
Shell-style `$VAR` and `~` are **not** expanded — use a placeholder or a literal absolute
path.

| Placeholder         | Resolves to                                                 |
| ------------------- | ----------------------------------------------------------- |
| `{{ school_dir }}`  | The linked school root (the school this project consumes).  |
| `{{ project_dir }}` | Project working directory.                                  |
| `{{ home }}`        | `$HOME`.                                                    |
| `{{ backend_dir }}` | `<project_dir>/<kind.backend_dir()>` for the resolved kind. |

The placeholder set is closed by design: shell-style expansion is open-ended (every env
var in scope is reachable) and context-sensitive at spawn time, while named placeholders
are auditable and stable across machines. An env var is routed through `env = {...}`
plus a `{{ ... }}` reference, or written as a literal path. Rendering happens in
`registry::bind` after `kind` is resolved (so `{{ backend_dir }}` knows its kind);
`Backend.cmd` and `Backend.env` carry fully rendered strings — `exec_session` /
`exec_one_shot` never see template syntax. Unknown names render to empty;
`ace school validate` surfaces typos
([school-commands.md](school/school-commands.md)).

### Use Cases

- **Override a built-in** — e.g. `[backends.claude]` can set `env`, `cmd`, `model`, or
  `effort` for every session selecting `backend = "claude"` in that resolved tree.
- **Multiple instances of the same kind** — register `bailer` and `bedrock-claude` as
  separate names, each with its own env, both backed by `Kind::Claude`. Users select via
  `backend = "..."`.
- **Wrap a built-in binary** — set `cmd = ["wrapper", "claude"]` to launch the backend
  through a process wrapper while keeping the rest of the contract (MCP, instructions
  file, linked folders) intact.

A custom backend cannot introduce new behavior beyond what its aliased `Kind` provides.
Adding a genuinely new backend requires extending the `Kind` enum in source.

## Model and Effort

`model` and `effort` are optional opaque strings on each backend instance. ACE does not
maintain a shared model catalogue or normalize effort values across vendors. The resolved
`Backend` owns one pair, and its `Kind` translates each configured value through the
backend's native launch surface.

The pair governs every model process ACE launches through that backend: interactive
sessions, `ace -p`, and model-driven operations such as MCP health checks. There is no
secondary pair. A missing field leaves that choice to the backend's own default.

Backend-native translation is documented in the per-backend specs. Runtime passthrough
arguments remain opaque and come after ACE-owned arguments, preserving their existing
ability to override a configured choice for one invocation.
