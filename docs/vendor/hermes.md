<!-- derived from: hermes-agent.nousresearch.com/docs @ 2026-07-30; source read at
     NousResearch/hermes-agent e444d165 @ 2026-08-01 -->

# Hermes Agent

**Not** an ACE design document. The upstream surface ACE would need to target to add
Hermes as a backend (`docs/spec/backend.md` §Backend Contract), mapped far enough that a
design question can be answered from this file instead of from the source.

Upstream: <https://hermes-agent.nousresearch.com/docs/reference/cli-commands> ·
repo `NousResearch/hermes-agent`.

Hermes is much larger than the other backends ACE dispatches to — ~70 top-level
subcommands covering messaging gateways, a web dashboard, a kanban board, and a desktop
app. Most of it is irrelevant to ACE. The sections below cover what a backend
implementation touches; §Full command map is the index for everything else.

## Identity

| Field          | Value                                   |
| -------------- | --------------------------------------- |
| Binary         | `hermes`                                |
| Home           | `$HERMES_HOME`, default `~/.hermes`     |
| Config         | `$HERMES_HOME/config.yaml`              |
| Credentials    | `$HERMES_HOME/.env`                     |
| Agent identity | `$HERMES_HOME/SOUL.md`                  |
| Skills         | `$HERMES_HOME/skills/`                  |
| Install        | git checkout — see `backend-install.md` |

## Invocation

| Intent          | Command                                          |
| --------------- | ------------------------------------------------ |
| Interactive     | `hermes` or `hermes chat`                        |
| Modern TUI      | `hermes --tui` (`--cli` forces the classic REPL) |
| One-shot        | `hermes -z "<prompt>"` — final response text only |
| Single query    | `hermes chat -q "<prompt>"`                      |
| Resume, by name | `hermes --continue [name]`                       |
| Resume, by id   | `hermes --resume <session>`                      |
| ACP server      | `hermes acp`                                     |
| MCP server      | `hermes mcp serve`                               |
| Backend server  | `hermes serve` — headless, powers the desktop app |

Launch-time flags that matter to a backend: `-m/--model`, `--provider`, `-t/--toolsets`,
`-s/--skills` (preload), `--yolo`, `--accept-hooks`, `--pass-session-id`, `-w/--worktree`
(run in an isolated git worktree), `--ignore-user-config`, `--ignore-rules`, and
`--safe-mode`.
`-z` also takes `--usage-file PATH` for a JSON cost/token report, written even when the
run fails.

## Isolation: a profile is just a `HERMES_HOME`

This is the feature with no counterpart in the other backends. Everything per-project —
skills, MCP servers, config, identity, memories, sessions — lives inside one directory,
and that directory is selectable per invocation.

`hermes -p <name>` and `hermes profile use <name>` both resolve to setting `HERMES_HOME`:
`resolve_profile_env()` (`hermes_cli/profiles.py`) maps a profile name to a path and the
CLI exports it before any hermes module is imported. `HERMES_HOME` set directly in the
environment does the same job with no named profile involved.

A profile directory is bootstrapped with `memories/ sessions/ skills/ skins/ logs/ plans/
workspace/ cron/ home/`, plus `config.yaml`, `.env`, and `SOUL.md`.

Two consequences worth holding onto:

- **Credentials are per-home.** `$HERMES_HOME/.env` is the credential file, so a home
  that is not `~/.hermes` does not inherit the user's provider keys. A project `.env`
  fills in missing values as a dev fallback (`hermes_cli/env_loader.py`).
- **`HERMES_HOME` must be propagated to subprocesses.** Hermes warns loudly when a
  non-default profile is sticky-active but `HERMES_HOME` is unset, because the process
  then writes into the *default* profile.

Named profiles add a registry on top: `~/.hermes/profiles/<name>/`, a `create` step, an
`active_profile` sticky-default file, and wrapper-script aliases so `<name> chat` works
as a command. The registry is global mutable state; the `HERMES_HOME` mechanism under it
is not.

### Profile distributions

`hermes profile install <git-url>[#<ref>]` installs a profile published as a git repo,
`hermes profile update <name>` re-pulls it, `hermes profile info <name>` shows the
manifest. A local directory containing `distribution.yaml` also works, for authoring
before the first push.

`distribution.yaml` at the profile root declares `name`, `version`, `description`,
`hermes_requires`, `env_requires` (named variables with descriptions, required flags, and
defaults), and `distribution_owned` paths. On update, distribution-owned paths
(`SOUL.md`, `skills/`, `cron/`, `mcp.json`, the manifest) are replaced, `config.yaml` is
preserved unless `--force-config`, and user-owned paths (`memories/`, `sessions/`,
`state.db`, `auth.json`, `.env`, `logs/`, `workspace/`, `home/`, `plans/`, `*_cache/`,
`local/`) are never touched.

`hermes profile export`/`import` is local backup/restore, explicitly *not* a distribution
format.

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

`/init` generates or updates `AGENTS.md` from a project scan — Hermes writes `AGENTS.md`
and has no instructions file it authors for itself.

## Skills

Skills live at `$HERMES_HOME/skills/`, seeded from the repo's bundled `skills/` on
install, plus any directory listed under `skills.external_dirs` in `config.yaml`
(`agent/skill_utils.py::get_external_skills_dirs`). **There is no per-project skill
directory** — per-project isolation comes from pointing `HERMES_HOME` at a different
home, not from a directory inside the repo.

A skill is a `SKILL.md` under a category directory; skills outside the trusted skills
dirs are flagged. Management surface:

| Command          | Purpose                                                          |
| ---------------- | ---------------------------------------------------------------- |
| `hermes skills`  | search, install (incl. from a URL), configure, enable/disable     |
| `hermes bundles` | named aliases for groups of skills, each exposed as a `/slash`    |
| `hermes curator` | background skill maintenance — status, run, pause, pin, prune     |
| `hermes sync`    | sync skills across devices and with a team                       |
| `-s/--skills`    | preload named skills into one session                            |

Disabled sets live in `config.yaml` under `skills.disabled` and
`skills.platform_disabled`.

## Trust and approvals

One switch, no levels: `--yolo` bypasses every dangerous-command approval prompt, and
`-z/--oneshot` auto-bypasses approvals on its own. There is no graded sandbox to map ACE's
trust levels onto. Adjacent controls: `--accept-hooks` auto-approves unseen shell hooks
declared in `config.yaml`; `chat --checkpoints` snapshots the filesystem before
destructive file operations (`/rollback` restores); `hermes approvals` mines approval
history into allowlist proposals; `hermes security` runs an OSV.dev supply-chain audit
over the venv, plugins, and MCP servers.

## MCP

`hermes mcp add` **cannot be driven from a script.** It takes its transport from flags
(`--url`, `--command` + `--args`, or `--preset`, plus `--auth`, `--env`,
`--connect-timeout`), but then connects, lists the discovered tools, and blocks on
`Enable all N tools? [Y/n/select]` — an unconditional `input()` with no `--yes` or
`--all-tools` escape (`hermes_cli/mcp_config.py::cmd_mcp_add`). A `--url` server also asks
whether it needs authentication and prompts for the token. Under EOF it cancels without
saving.

Registration therefore has to write `config.yaml` directly. Servers live under
`mcp_servers.<name>` as `{url | command, args, env, headers, tools.include, enabled}`;
bearer tokens are stored in `$HERMES_HOME/.env` and referenced from `headers` by env-var
interpolation.

Rest of the tree: `hermes mcp list | remove | test | configure | login | reauth | picker |
serve`. `configure` toggles per-tool selection for an existing server.

## Full command map

Grouped from `hermes --help` at `e444d165`. Anything not covered above is listed here so
its existence is known without re-deriving it.

| Area              | Commands                                                          |
| ----------------- | ----------------------------------------------------------------- |
| Session           | `chat` `sessions` `checkpoints` `insights` `prompt-size`          |
| Model & providers | `model` `moa` (mixture-of-agents) `fallback` `proxy` `portal`     |
| Auth              | `login` `logout` `auth` `secrets` `pairing` `egress`              |
| Config            | `config` `setup` `migrate` `profile` `tools` `hooks` `skin` `pets` |
| Skills            | `skills` `bundles` `curator` `sync` `plugins`                     |
| Memory            | `memory` `journey` (aliases `learning`, `memory-graph`)           |
| Protocols         | `mcp` `acp` `lsp` `computer-use` `webhook`                        |
| Messaging gateway | `gateway` `send` `whatsapp` `whatsapp-cloud` `slack` `monitoring` |
| Work management   | `kanban` `project` `cron`                                         |
| Interfaces        | `dashboard` (web UI, port 9119) `desktop`/`gui` `serve` `console` |
| Diagnostics       | `doctor` `status` `logs` `dump` `debug` `security`                |
| Lifecycle         | `version` `update` `uninstall` `backup` `import` `completion`     |
| Migration in      | `import-agent` (Claude Code / Codex CLI setups) `claw` (OpenClaw) |

Two entries deserve a note because their names mislead:

- **`hermes project`** is not a workspace container. A Project is a named, multi-folder
  grouping stored per-profile in `$HERMES_HOME/projects.db`; it anchors desktop session
  grouping (by longest-prefix `cwd` match) and gives kanban tasks a deterministic worktree
  and branch. It carries no skills, config, or context of its own.
- **`hermes import`** restores a Hermes backup zip. Importing another agent's setup is
  `hermes import-agent`, which maps `CLAUDE.md` / `AGENTS.md` into memory entries under
  `$HERMES_HOME/memories/MEMORY.md`.
