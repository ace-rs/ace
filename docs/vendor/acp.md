<!-- derived from: agentclientprotocol.com + per-agent docs @ 2026-07-30 -->

# Agent Client Protocol (ACP) — reference snapshot

**Not** an ACE design document. A frozen crib of what ACP specifies and which harnesses
speak it, captured while evaluating whether ACP could replace the hand-rolled per-backend
bridges under `.claude/skills/ace-connect/scripts/`.

## Sources

| Source                                                  | What it is                                  |
| ------------------------------------------------------- | ------------------------------------------- |
| <https://agentclientprotocol.com/protocol/overview>     | Canonical spec. Authoritative               |
| <https://agentclientprotocol.com/llms-full.txt>         | Full spec as one text file — best for greps |
| <https://opencode.ai/docs/acp/>                         | OpenCode's native `opencode acp`            |
| <https://github.com/agentclientprotocol/codex-acp>      | Codex bridge (not shipped by OpenAI)        |
| <https://hermes-agent.nousresearch.com/docs/user-guide/features/acp> | Hermes' native `hermes acp`    |

## What ACP is

A JSON-RPC protocol standardizing **editor ↔ coding agent** communication, so one editor
plugin can drive any agent. It is the mirror of MCP: MCP gives an agent its tools, ACP
gives an agent a front-end.

- **Transport is stdio.** "All communication happens over stdin/stdout." Remote transport
  (WebSocket, HTTP) is explicitly a work in progress, not shippable today.
- **The client spawns the agent.** "Agents … typically run as subprocesses of the Client";
  "the editor boots the agent sub-process on demand."
- **The agent is headless in this mode.** Running an agent under ACP replaces its own TUI —
  the client renders the conversation, permission prompts, diffs, and tool calls.
- Core methods: `initialize`, `session/new`, `session/prompt`, `session/update`
  (notification), `session/load`, `session/resume`, `session/list`, `session/close`,
  `session/delete`.
- `session/load` replays the whole conversation as `session/update` notifications;
  `session/resume` reconnects without replay.

### What the spec does not say

Session ownership and concurrency are **unspecified**. There is no documented way for a
second client to attach to, observe, or inject into a session another client is currently
driving. `session/load` / `session/resume` are serial reconnection of one client to a
persisted session, not concurrent multi-subscriber fan-out.

## Harness support

| Harness   | ACP server                | Notes                                        |
| --------- | ------------------------- | -------------------------------------------- |
| OpenCode  | native — `opencode acp`   | All features; `/undo` and `/redo` unsupported |
| Hermes    | native — `hermes acp`     | Also ships an ACP *client* (Copilot CLI only) |
| Codex     | third-party bridge        | `codex-acp` wraps the codex app-server        |
| Claude    | third-party bridge        | Zed's `claude-code-acp`; Anthropic ships none |

Client side: Zed and JetBrains are native; Neovim, Emacs, VS Code have community plugins.

## Bearing on ACE

Facts above; the reading below is ours and no ruling has been made on it.

**ACP does not address what ace-connect addresses.** ace-connect's problem is delivering a
peer message into a session *a human is actively driving in their own TUI* — which needs a
second, non-owning connection to a live session. That is precisely the capability ACP
leaves unspecified, and the capability codex's `--listen` app-server and opencode's
`serve` + `attach` do provide natively. Adopting ACP for the bridge would mean ACE renders
the session UI itself and the human gives up their harness's native TUI.

**Where ACP does fit** is the surface that is already headless and already 1:1: `ace -p`
one-shot, and any future `ace serve` (Roadmap "Later", 245). There, ACP would collapse
four hand-written argv dialects (`docs/spec/backend.md` §Per-Backend Argv) into one
protocol, and give structured events where ACE currently captures raw stdout.

**Coverage is the catch either way** — two of four backends need a third-party bridge
binary on `$PATH`, which is a dependency ACE does not currently impose.
