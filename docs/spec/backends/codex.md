# Backend: Codex

Binary: `codex` | Dir: `.agents` | Instructions: `AGENTS.md`

Verified against codex 0.145.0 and the vendored
[Codex manual](../../vendor/codex-manual.md) (2026-08-03).

## Model and Effort

ACE translates a resolved `model` to `--model <value>` and `effort` to the native
`-c model_reasoning_effort=<value>` configuration override. Both apply to interactive and
`exec` invocations. Values remain opaque to ACE; Codex validates them.

## Readiness

`~/.codex/auth.json` exists, **or** `OPENAI_API_KEY`/`CODEX_API_KEY` env var is set.

`CODEX_HOME` overrides `~/.codex`.

Accepted heuristic gaps:

- `cli_auth_credentials_store = "keyring"` (or `"auto"` resolving to the OS keychain)
  stores credentials outside `auth.json` — a logged-in user reads as not-ready.
- `CODEX_API_KEY` is honored by `codex exec` only, so it over-reports readiness for
  interactive sessions; `OPENAI_API_KEY` authenticates via `codex login --with-api-key`,
  not ambiently.

## Session Prompt

Do not pass ACE's session prompt as Codex's initial positional prompt in interactive mode.
That positional prompt is a user message and triggers a reply, which is not the intended
behavior for ACE's ambient session instructions.

For interactive Codex runs, ACE should pass the session prompt through Codex's native config
override surface as `-c developer_instructions=...`. Codex does not support a
`--system-prompt` flag.

## Trust Modes

- `trust = "auto"` → `--ask-for-approval on-request --sandbox danger-full-access`.
  Mirrors codex's Auto preset (`--sandbox workspace-write -a on-request`) with the
  sandbox raised to `danger-full-access`: ACE typically runs inside an
  externally-sandboxed environment, so codex's internal sandbox fights the outer one
  instead of adding protection.
- `trust = "yolo"` → `--dangerously-bypass-approvals-and-sandbox` (upstream alias
  `--yolo`).

Upstream deprecated `--full-auto` (still accepted, prints a warning); ACE never
passes it.

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

## Managed and connected sessions

Codex advertises controlled startup, primary-thread input, native resume, and thread
listing through its documented app-server surface. A connect-compatible component graph
starts app-server on its sanctioned Unix-socket transport, establishes or resumes the
primary thread, attaches the native client UI, and runs the local relay adapter.

The primary thread is the only ACE-connect delivery target. Parent/child relationships
and loaded native threads may be exposed by `ace session inspect`, but ACE does not own
Codex subagent orchestration or route peer messages to child threads.

Plain interactive Codex remains valid for an ordinary unmanaged launch. It cannot be
retrofitted with the external receive handle required by connected mode; that requirement
must be present when the instance plan is materialized.

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

After registration, MCP auth and ongoing management happen inside Codex — via `/mcp` in a
session, or `codex mcp login <name>` / `codex mcp logout <name>` from the CLI (OAuth,
streamable-HTTP servers only).

ACE should not run a separate external OAuth flow for Codex. It registers the server and
leaves authentication to those native surfaces.

## MCP Operations

All four operations are implemented:

- `mcp_add()` — `codex mcp add <name> --url <url>` when the declaration has no static
  headers. The CLI has no static-header flag (only `--bearer-token-env-var`), so header
  declarations fall back to a merge into `config.toml`'s `[mcp_servers.<name>]` with
  `http_headers`.
- `mcp_list()` — `codex mcp list --json` (top-level array of `{name, ...}` entries),
  falling back to parsing `mcp_servers` from `config.toml`.
- `mcp_check()` — `codex exec --output-schema <schema> -o <file> <prompt>` asking the
  model to probe each server; "registered" does not imply "working". Runs with
  `--skip-git-repo-check`: the probe does no repo work, and codex refuses to `exec`
  outside a git repository otherwise.
- `mcp_remove()` — `codex mcp remove <name>`, config-merge fallback.

Automatic post-registration health checks in ACE's shared main flow are a separate
cross-backend product decision. ACE does not introduce Codex-only auto-check behavior
through the shared registration path.

## Project paths

Root: `.agents/`. ACE links the canonical school folders beneath it for compatibility;
Codex natively consumes `.agents/skills/` only. Its rules, prompts, and config-defined
agents live in user/config surfaces and are not ACE project links.

## Linked Folders

| Folder      | Supported |
|-------------|-----------|
| `skills/`   | ✓         |
| `rules/`    | ✗         |
| `commands/` | ✗         |
| `agents/`   | ✗         |

Codex natively discovers skills: it scans `.agents/skills` in every directory from cwd up
to the repo root, plus `$HOME/.agents/skills` and `/etc/codex/skills`, follows symlinked
skill folders, and progressively discloses them (name + description list capped at ~2% of
context, full `SKILL.md` loaded on selection). ACE's nested symlink emit into
`<project>/.agents/skills/` lands directly on this surface — no `AGENTS.md` skill listing
is needed.

The unsupported rows have codex-side analogs with different semantics — execpolicy rules
under `~/.codex/rules`, custom prompts under `~/.codex/prompts`, config-defined agents in
`config.toml` — so a school-folder mapping for them is new design work, not a linking
gap.
