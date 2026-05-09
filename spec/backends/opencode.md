# Backend: OpenCode

Binary: `opencode` | Dir: `.opencode` | Instructions: `AGENTS.md`

## Readiness

`~/.local/share/opencode/auth.json` exists and is non-empty. Stores provider
auth tokens; missing or empty `{}` means no providers authenticated.

`OPENCODE_HOME` overrides `~/.local/share/opencode`. The DB file
(`opencode.db`) is created on first command run, but auth is the meaningful
readiness signal.

## Session Prompt

**No `--system-prompt` CLI flag.** OpenCode uses an agent system for custom
instructions.

### Agent-based injection

ACE writes `.opencode/agents/ace.md` during setup — a markdown file with
YAML frontmatter and the session prompt as the body:

```markdown
---
description: "ACE-provisioned coding session"
mode: all
---

<session prompt here>
```

`mode: all` makes the agent available as both primary (selectable in TUI,
usable via `--agent`) and subagent. Agents are stored in
`.opencode/agents/` (project) or `~/.config/opencode/agents/` (global).
Filename minus `.md` = agent name. The markdown body **replaces** the
default system prompt entirely.

All frontmatter fields are optional. Available fields: `description`,
`mode` (`subagent` | `primary` | `all`), `model` (`provider/model`),
`variant`, `temperature`, `top_p`, `color`, `steps`, `hidden`, `disable`,
`permission` (per-tool: `allow` | `deny` | `ask`).

### Interactive mode

`opencode --agent ace` launches interactive mode with the "ace" agent
selected, carrying the full session prompt.

### One-shot mode

`opencode run [message..]` — sends a single prompt, streams output to
stdout, exits when idle. Supports piped stdin (combined with positional
message).

- `--agent <name>` — select agent (carries system prompt). Use `--agent ace`.
- `--format json` — streams raw JSON events.
- `--continue/-c` — continue last session.
- `--session/-s <id>` — resume specific session.
- `--file/-f <path>` — attach file(s).
- `--model/-m <provider/model>` — e.g. `anthropic/claude-3-5-sonnet`.

## Yolo Mode

`--dangerously-skip-permissions` — auto-approves all permission requests.
Available on `opencode run` only (interactive mode auto-rejects by default).

## MCP Registration

**Method: Direct config write** — `opencode mcp add` is an interactive
wizard, unusable for non-interactive ACE setup.

Config file: `opencode.json` or `opencode.jsonc` in project root (JSONC
format).

ACE writes the config file directly — merging into existing content
(preserving manually-added entries) rather than overwriting.

## Linked Folders

| Folder      | Supported |
|-------------|-----------|
| `skills/`   | ✓         |
| `rules/`    | ✗         |
| `commands/` | ✓         |
| `agents/`   | ✓         |
