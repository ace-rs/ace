# Backend: Codex

Binary: `codex` | Dir: `.agents` | Instructions: `AGENTS.md`

Baseline verified against codex 0.145.0 and the vendored
[Codex manual](../../vendor/codex-manual.md) (2026-08-03); managed-session integration
verified against 0.154.0 on 2026-09-13 as recorded below.

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
listing through its documented app-server surface. The backend materializes app-server
on its sanctioned Unix-socket transport followed by the native client UI as the terminal
`session` component. Connect publishes the recipient after its primary is known.
Every listed component is essential.

The planned controlled-session boundary constructs the first two process roles as
`codex app-server --listen unix://...` followed by
`codex --remote unix://...` session. Runtime endpoint allocation and primary-thread
establishment remain part of the later controller/executor boundary.

This order describes startup, not message routing: the terminal connects directly to
app-server. The split exposes the sanctioned receive surface that `ace connect send`
uses to deliver to this instance. The sending agent invokes the ACE command; ACE resolves
the recipient and translates the send into the Codex call without changing Codex internals.

The planned controller distinguishes server readiness from recipient readiness. For fresh
startup it waits for app-server readiness, starts the native terminal to create the
primary, discovers its initial ID on the dedicated server, and retains that fixed target.
Only then may connect publish the endpoint and primary ID as an available recipient.
This follows the
[verified native-client-created primary](#verified-managed-session-integration)
without a setup message or model response. Later terminal conversation selection does
not change the retained target. The controller also classifies Codex-native shutdown
cascades so exit observation order does not decide the outcome.
A successful user exit from the native client and its app-server cascade complete
normally; unrelated app-server loss or an abnormal client exit fails the session.
Connect classifies relay exits and may include
them in the normal user-exit cascade. Cleanup is idempotent, and ACE does not restart the
component list.

The primary thread is the only ACE-connect delivery target. Parent/child relationships
and loaded native threads may be exposed by `ace session inspect`, but ACE does not own
Codex subagent orchestration or route peer messages to child threads.

Plain interactive Codex remains valid for an ordinary unmanaged session. It cannot be
retrofitted with the external receive handle required by connected mode; a connected
request must carry its control endpoint and topology requirement by construction.

## Verified managed-session integration

Recorded 2026-09-13 against installed Codex 0.154.0. These are backend integration
results, not evidence that ACE session or connect commands are implemented. Checks used
public app-server APIs and the native terminal; no backend rollout files were inspected
or manufactured.

### Empty-recipient delivery and fresh startup

An app-server client created a thread with `thread/start` and verified that its returned
`turns` array was empty. A separate WebSocket client sent the first input directly with
`turn/start` to that explicit thread ID. The owner observed the completed turn and
`EMPTY_RECIPIENT_OK` response in 5.848 seconds. No preceding setup message was sent.
Recipient thread: `01a09a2f-ce6c-7091-bc45-62bd5a5572c3`.

A second check started a dedicated app-server with no loaded threads, then opened
`codex --remote unix://<socket>` without a prompt. The terminal created a fresh
conversation and displayed its empty input interface. `thread/loaded/list` returned one
thread ID; `thread/read` with `includeTurns: false` reported it idle, and no terminal
input or turn request had been issued. The sender retained that ID and called
`turn/start` directly. The terminal displayed both the first external input and the
assistant response `FRESH_UI_EMPTY_OK` in 7.282 seconds while remaining alive.
Recipient thread: `01a09a33-cff3-7e42-92e5-7344f0ad2af4`.

**Verified:** an empty recipient can receive its first external message, and the native
terminal can open empty and display that delivery. Neither requires a preceding “hi”.
The second check verifies native-client-created primary → discover its initial ID →
send to that fixed ID. It does not verify controller-created primary → attach an empty
thread, or discovery among multiple candidate threads. Later terminal conversation
selection was not followed.

### Retained attachment and delivery evidence

The preceding integration run verified native terminal attachment after one completed
turn, external idle delivery, and external busy-turn steering to the fixed primary.
Setup, idle, and busy turns completed in 6.928, 3.045, and 12.251 seconds respectively.
The native terminal displayed each input and response. Busy input was accepted and
subsequently displayed `BUSY_OK`; immediate interruption was not established.
App-server continued answering after terminal disconnection.

Observed boundaries and unsuccessful attempts:

- Unix transport required WebSocket handshake and framing; raw JSONL through
  `codex app-server proxy --sock` timed out, while websocat worked on the same socket.
- Resuming an empty thread returned `no rollout found`; resuming after its first
  completed turn succeeded. Direct `turn/start` did not require first calling
  `thread/resume` on the empty recipient.
- Remote terminal resume rejected permission overrides.
- Full history reading in the native fresh-thread check returned
  `list_turns is not supported yet`; the corrected check used public metadata and
  observation of the native terminal response.
- Waiting for completion notifications on the separate sender timed out although the
  terminal displayed the response. The final assertion checked the rendered assistant
  response and terminal liveness, rejecting prompt or test-log matches.

### Test conditions and planning handoff

Completed runs used `gpt-5.6-luna`, standard service tier, and fast mode disabled.
Fresh terminal runs displayed medium effort. Low effort was requested afterward;
scripts now set `model_reasoning_effort="low"` and turn `effort="low"`, but those setting
changes were not used to repeat the successful live checks. Future test sessions must
explicitly use Luna, low effort, and fast mode disabled.

The original attachment/idle/busy scripts were discarded; their retained results are
recorded above. Additional scripts, append-only results, and terminal captures remain
at `/tmp/ace-empty-recipient-check/`. Those temporary files are supplementary; this spec
preserves the evidence needed to resume planning without them. Test-owned processes
were stopped after the runs. No ACE application code or dependencies changed.

Resume implementation planning using these results; do not recreate the backend test
suite merely because a new session starts. The selected fresh-start ordering is specified
under Managed and connected sessions above. ACE-owned endpoint, discovery,
lifecycle, and command behavior still need implementation tests; backend-owned approvals,
sender lifetime semantics, and terminal-selection tracking are not additional
prerequisites. Integration verification does not approve implementation or dependencies.

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
