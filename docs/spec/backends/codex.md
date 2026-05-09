# Backend: Codex

Binary: `codex` | Dir: `.agents` | Instructions: `AGENTS.md`

## Readiness

`~/.codex/auth.json` exists, **or** `OPENAI_API_KEY`/`CODEX_API_KEY` env var is set.

`CODEX_HOME` overrides `~/.codex`.

## Session Prompt

Do not pass ACE's session prompt as Codex's initial positional prompt in interactive mode.
That positional prompt is a user message and triggers a reply, which is not the intended
behavior for ACE's ambient session instructions.

For interactive Codex runs, ACE should pass the session prompt through Codex's native config
override surface as `-c developer_instructions=...`. Codex does not support a
`--system-prompt` flag.

## Trust Modes

- `trust = "auto"` → `--full-auto`
- `trust = "yolo"` → `--dangerously-bypass-approvals-and-sandbox`

## Session Resume

`codex resume --last` resumes the most recent session scoped to the current working directory.
The `--all` flag disables cwd filtering to show sessions from any directory.

`codex resume <SESSION_ID>` resumes a specific session by UUID. Session IDs are visible in
the picker, `/status`, or files under `~/.codex/sessions/`.

`codex resume` (bare) launches an interactive picker of recent sessions, filtered to cwd by
default.

Note: `resume` is a subcommand, not a flag — so ACE must build a different command for resume
vs new session (unlike Claude where `--continue` is just a flag on the same command).

**No prior session:** `codex resume --last` in a directory with no previous sessions shows an
empty picker. Pressing ESC creates a new session. This means resume-by-default is safe — no
error or crash on first run.

## MCP Registration

**Method: CLI-first.** Prefer `codex mcp add` for registration.

Fallback: edit `~/.codex/config.toml` directly only if the CLI cannot express the needed
configuration cleanly. Prefer the CLI because it remains aligned with Codex's evolving config
model.

Config file: `~/.codex/config.toml` (TOML format). Codex also supports project-level
`.codex/config.toml`, but ACE registers school MCP servers at user scope.

ACE should merge into existing config when using the fallback path. Never overwrite unrelated
user config.

## MCP Auth And Management

After registration, MCP auth and ongoing management happen inside Codex via `/mcp`.

ACE should not run a separate external OAuth flow for Codex. Once inside the backend session,
the user manages MCP connectivity there.

## Implementation Priorities

Codex support should be completed in this order:

1. `mcp_add()` — required to register school MCP servers through the native Codex CLI.
2. `mcp_list()` — required so ACE can avoid repeatedly offering or re-adding already
   registered servers.
3. `mcp_check()` — required in the same Codex pass because "registered" does not imply
   "working". MCP can be configured but unusable due to expired auth, invalid tokens, or
   backend-side state drift.
4. `mcp_remove()` — required because `ace mcp reset` is already part of ACE's user-facing
   command surface.

Automatic post-registration health checks in ACE's shared main flow are a separate
cross-backend product decision. Codex should implement `mcp_check()` now, but ACE should not
quietly introduce Codex-only auto-check behavior through the shared registration path.

## Linked Folders

| Folder      | Supported |
|-------------|-----------|
| `skills/`   | ✓         |
| `rules/`    | ✗         |
| `commands/` | ✗         |
| `agents/`   | ✗         |

Codex uses ACE school skills through the current `AGENTS.md` plus linked-folder workflow.
Unsupported entries here mean ACE should not assume richer native folder primitives for those
other school directories.
