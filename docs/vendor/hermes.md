<!-- derived from: hermes-agent.nousresearch.com/docs @ 2026-07-30 -->

# Hermes Agent — CLI surface

**Not** an ACE design document. The upstream surface ACE would need to target to add
Hermes as a backend (`docs/spec/backend.md` §Backend Contract).

Upstream: <https://hermes-agent.nousresearch.com/docs/reference/cli-commands> ·
repo `NousResearch/hermes-agent`.

## Identity

| Field       | Value                                       |
| ----------- | ------------------------------------------- |
| Binary      | `hermes`                                    |
| Config      | `~/.hermes/config.yaml`                     |
| Credentials | `~/.hermes/.env`                            |

## Invocation

| Intent           | Command                                              |
| ---------------- | ---------------------------------------------------- |
| Interactive      | `hermes chat`                                        |
| One-shot         | `hermes -z "<prompt>"` — final response text only     |
| Resume, by name  | `hermes --continue [name]`                           |
| Resume, by id    | `hermes --resume <session>`                          |
| ACP server       | `hermes acp`                                         |
| MCP server       | `hermes mcp serve`                                   |

MCP management: `hermes mcp install | list | add | remove | test | configure`.
Auth: `hermes auth [list | add <provider>]`. Model selection: `hermes model`, or `/model`
mid-session. First-run wizard: `hermes setup`.

## Gaps for a backend implementation

Unread upstream, needed before `Kind::Hermes` can be spec'd: the instructions-file name
and per-project agent/skill directory (ACE's `backend_dir()` and `instructions_file()`),
whether a system prompt can be injected at launch, the approval/sandbox flag that maps to
ACE's trust levels, and whether `hermes mcp add` is non-interactive enough for the
CLI-first registration strategy.
