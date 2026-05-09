# Backend Configuration

## Backend Enum

| Value      | Binary     | Backend Dir | Instructions File | Details                          |
|------------|------------|-------------|-------------------|----------------------------------|
| `claude`   | `claude`   | `.claude`   | `CLAUDE.md`       | [backends/claude.md](backends/claude.md)     |
| `codex`    | `codex`    | `.agents`   | `AGENTS.md`       | [backends/codex.md](backends/codex.md)       |
| `opencode` | `opencode` | `.opencode` | `AGENTS.md`       | [backends/opencode.md](backends/opencode.md) |

## TOML Syntax

```toml
backend = "claude"
```

Valid in `~/.config/ace/ace.toml` (user), `ace.toml` (project), `ace.local.toml` (local),
and `school.toml` (`[school]` section).

## Resolution Order

First `Some` wins in this priority order (highest to lowest):

1. CLI override — `ace --backend <name>`, `ace -b <name>`, or convenience flags such as
   `ace --claude` / `ace --codex`
2. Local — `ace.local.toml`
3. Project — `ace.toml`
4. User — `~/.config/ace/ace.toml`
5. `school.toml` — school-level default

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
- **`supports_trust(trust)`** — validate whether the backend supports the given trust level.
- **`exec_session(req)`** — launch an interactive backend session via exec-replace. Builds its
  Command from `SessionRequest` (trust, session prompt, project dir, env, extra args, resume).
  Returns `io::Error` on spawn failure; never returns on success (terminal hands off to the
  child). When `resume = true`, some backends may fail if no prior session exists (Claude)
  while others handle it gracefully (Codex). ACE prints a hint before exec so the user knows
  to run `ace new` on failure. See `docs/decisions/2026-04-09-resume-fallback.md`.
- **`exec_one_shot(req)`** — spawn the backend non-interactively and capture stdout/stderr.
  Builds its Command from `OneShotRequest` (prompt source, project dir, env, extra args; no
  resume, trust, or session prompt — the non-interactive entry point doesn't take approval
  modes or system-prompt injection). Returns `io::Result<std::process::Output>` — caller
  inspects `status.success()` and `stderr` for non-zero exits. Used by `ace -p` (CLI) and
  ACE-internal consumers (e.g. `ace learn`) that need a programmatic backend invocation.
  See `docs/decisions/2026-05-07-polymorphic-flags.md`.
- **`mcp_list()`** — list currently registered MCP server names.
- **`mcp_add(entry)`** — register a remote MCP server.
- **`mcp_remove(name)`** — unregister a remote MCP server by name.
- **`mcp_check(names)`** — runtime usability check for registered MCP servers. This is not a
  static config parse — the backend executes a one-shot prompt that exercises each server from
  inside the backend's own environment (auth state, token storage, MCP client). Returns a list
  of name/ok pairs. Best-effort: returns empty on failure or if unsupported.

See per-backend specs for implementation details.

## Intent Mapping

`exec_session` and `exec_one_shot` are the two transport methods. Each backend builds its
argv from the matching request type. The argv builder is the polymorphic core; transport
just decides whether to exec-replace or spawn-and-capture.

### Per-Backend Argv

| Intent           | claude                                                | codex                                  |
|------------------|-------------------------------------------------------|----------------------------------------|
| Session          | `--system-prompt <prompt>` (or `--continue` if resume) | `-c developer_instructions=<prompt>` (or `resume --last`) |
| OneShot, Inline  | `-p <text>`                                           | `exec <text>`                          |
| OneShot, Stdin   | `-p` + piped child stdin                              | `exec -` + piped child stdin           |

Trust flags (`--permission-mode` / `--ask-for-approval` / sandbox) attach to Session only.
OneShot is non-interactive — approval modes don't apply.

### Prompt Source

`OneShotRequest.prompt: PromptInput` is `Inline(String)` for argv-passed prompts, `Stdin` for
piped stdin. Backends translate per the table above. When `Stdin`, the spawned child inherits
the parent's stdin (`Stdio::inherit()`); the caller must arrange the piped data themselves.

## MCP Server Registration

ACE registers `[[mcp]]` entries from `school.toml` into the active backend. All entries are
remote MCP endpoints — see [mcp.md](mcp.md) for the remote-only design rationale.

**Strategy: CLI-first.** Prefer invoking the backend's CLI to add MCP servers. Only fall back
to writing config files when the CLI cannot express the needed configuration cleanly.

ACE owns registration into the backend. Backend-native auth and MCP management should remain in
the backend wherever possible.

## Linked Folders

ACE links school folders (`skills/`, `rules/`, `commands/`, `agents/`) into the project's
backend directory. Not all backends support every folder — see per-backend specs for the
support matrix.

Some backends may use different directory names for linked folders. The Link action handles
remapping when needed.

## Session Prompt

Backends receive the session prompt via their native invocation surface. For some backends this
is a CLI flag such as `--system-prompt`; for others it is an initial positional prompt. See
per-backend specs for the exact delivery mechanism.

## Readiness Check

Backends may expose an `is_ready()` heuristic so ACE can warn or gate execution when the backend
is clearly not initialized. Whether ACE should enforce readiness before exec is a product
decision and may vary by backend or evolve over time.

## Custom Backends

`[[backends]]` declarations let a school, user, or project register additional backend
entries alongside the built-ins. A custom backend is **not** a new `Kind` — it's a named
instance that aliases a built-in `Kind` and may override its launch `cmd` and `env`. The
backend contract (MCP, readiness, instructions file, linked-folder layout) is inherited
from the aliased `Kind`.

### TOML Syntax

```toml
[[backends]]
name = "bailer"            # required — selectable via `backend = "bailer"` or `-b bailer`
kind = "claude"            # optional — see kind resolution below
cmd  = ["claude"]          # optional — argv for launch; defaults to [kind.name()]
env  = { ANTHROPIC_BASE_URL = "https://proxy.example.com" }  # optional
```

Valid in `school.toml` (`[[backends]]`), user, project, and local config.

### Kind Resolution

For a new name, `kind` is resolved in order:

1. Explicit `kind = "..."` field (must be a built-in name).
2. `name` matches a built-in name → that kind.
3. `cmd[0]` basename matches a built-in name → that kind.
4. Otherwise → `BackendError::Unresolvable`.

For a name that already exists (built-in or earlier-layer custom), the decl partially
overrides the existing entry: `env` merges per-key (last wins), `cmd` last-wins-non-empty,
and a declared `kind` must match the existing kind (otherwise `BackendError::KindMismatch`).

### Layer Merge

Declarations are folded into a registry seeded with built-ins, in layer order:
school → user → project → local. Later layers may add new entries or partially override
earlier ones. The selected backend name (resolved per the [Resolution Order](#resolution-order)
above) is then looked up in the final registry; an unknown name is `BackendError::Unknown`.

### Path Templating

`cmd[]` entries and `env` values may use `{{ ... }}` placeholders, rendered at bind
time. Shell-style `$VAR` and `~` are **not** expanded — use a placeholder or a
literal absolute path.

| Placeholder        | Resolves to                                                  |
|--------------------|--------------------------------------------------------------|
| `{{ school_dir }}` | Active school root.                                          |
| `{{ project_dir }}`| Project working directory.                                   |
| `{{ home }}`       | `$HOME`.                                                     |
| `{{ backend_dir }}`| `<project_dir>/<kind.backend_dir()>` for the resolved kind.  |

Unknown names render to empty (a future `ace school validate` will surface typos).
See [docs/decisions/2026-05-09-backend-cmd-templating.md](docs/decisions/2026-05-09-backend-cmd-templating.md).

### Use Cases

- **Override env or cmd for a built-in** — e.g. point `claude` at a corporate proxy by
  setting `[[backends]] name = "claude" env = { ANTHROPIC_BASE_URL = "..." }`.
- **Multiple instances of the same kind** — register `bailer` and `bedrock-claude` as
  separate names, each with its own env, both backed by `Kind::Claude`. Users select via
  `backend = "..."`.
- **Wrap a built-in binary** — set `cmd = ["wrapper", "claude"]` to launch the backend
  through a process wrapper while keeping the rest of the contract (MCP, instructions file,
  linked folders) intact.

A custom backend cannot introduce new behavior beyond what its aliased `Kind` provides.
Adding a genuinely new backend requires extending the `Kind` enum in source.
