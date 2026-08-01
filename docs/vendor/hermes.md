<!-- derived from: hermes-agent.nousresearch.com/docs @ 2026-07-30; source read at
     NousResearch/hermes-agent e444d165 @ 2026-08-01 -->

# Hermes Agent — CLI surface

**Not** an ACE design document. The upstream surface ACE would need to target to add
Hermes as a backend (`docs/spec/backend.md` §Backend Contract).

Upstream: <https://hermes-agent.nousresearch.com/docs/reference/cli-commands> ·
repo `NousResearch/hermes-agent`.

## Identity

| Field          | Value                                   |
| -------------- | --------------------------------------- |
| Binary         | `hermes`                                |
| Config         | `~/.hermes/config.yaml`                 |
| Credentials    | `~/.hermes/.env`                        |
| Install        | git checkout — see `backend-install.md` |
| Agent identity | `$HERMES_HOME/SOUL.md`                  |
| Skills         | `~/.hermes/skills/`                     |

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

## Project context files

Hermes loads **all** of these, in this order, rather than stopping at the first match
(`agent/prompt_builder.py`):

| File                       | Discovery                                    |
| -------------------------- | -------------------------------------------- |
| `$HERMES_HOME/SOUL.md`     | agent identity, slot #1 of the system prompt |
| `.hermes.md` / `HERMES.md` | nearest one, walking up to the git root      |
| `AGENTS.md` / `agents.md`  | cwd only, no walk                            |
| `CLAUDE.md` / `claude.md`  | cwd only                                     |
| `.cursorrules`             | cwd only                                     |

Sub-directory `AGENTS.md`/`CLAUDE.md`/`.cursorrules` files are discovered progressively as
the agent navigates into folders (`agent/subdirectory_hints.py`). Every context file is
scanned for prompt injection before loading, and truncated head/tail past a size cap.
`--ignore-rules` skips this injection; `--safe-mode` skips it plus user config, plugins,
and MCP servers.

`hermes /init` generates or updates `AGENTS.md` from a project scan — Hermes writes
`AGENTS.md`, and has no file of its own that it authors.

## Skills

Global only: `~/.hermes/skills/`, seeded from the repo's bundled `skills/` on install,
plus any directory listed under `skills.external_dirs` in `config.yaml`
(`agent/skill_utils.py::get_external_skills_dirs`). **There is no per-project skill
directory** — a skill is either installed for the user or reachable through an external
dir. `-s/--skills <name>` preloads named skills into a session; `hermes skills` toggles
them (disabled sets live in `config.yaml` under `skills.disabled` /
`skills.platform_disabled`).

## Trust and approvals

One switch, no levels: `--yolo` bypasses every dangerous-command approval prompt.
`-z/--oneshot` auto-bypasses approvals on its own. `--accept-hooks` separately
auto-approves unseen shell hooks declared in `config.yaml`. `chat --checkpoints` takes
filesystem snapshots before destructive file operations.

## System prompt at launch

No flag injects prompt text. The launch-time levers are `-s/--skills` (preload),
`--pass-session-id` (adds the session ID to the system prompt), and the context files
above; everything else the agent reads it discovers from the cwd.

## `hermes mcp add` is interactive

It cannot be driven from a script. `hermes mcp add <name> --url|--command|--preset` takes
its transport from flags, but the flow then connects, lists the discovered tools, and
blocks on `Enable all N tools? [Y/n/select]` — an unconditional `input()` with no `--yes`
or `--all-tools` escape (`hermes_cli/mcp_config.py::cmd_mcp_add`). A `--url` server also
asks whether it needs authentication, and prompts for the token. Under EOF the command
cancels without saving.

Servers land in `config.yaml` under `mcp_servers.<name>` as `{url|command, args, env,
headers, tools.include, enabled}`. A CLI-first registration strategy does not work here;
writing that config directly does.
