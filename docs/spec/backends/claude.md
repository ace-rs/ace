# Backend: Claude

Binary: `claude` | Dir: `.claude` | Instructions: `CLAUDE.md`

## Readiness

`~/.claude.json` exists with auth data. Created on first successful login.

## Session Prompt

Passed via `--system-prompt <prompt>` CLI flag.

## Permission Modes

| Trust     | Flag                                      |
|-----------|-------------------------------------------|
| Default   | *(none)*                                  |
| Auto      | `--permission-mode auto`                  |
| Yolo      | `--permission-mode bypassPermissions`     |

**Auto mode availability**: Team, Enterprise, or API plans only — **not available on Pro or
Max**. Requires admin opt-in on Team/Enterprise. Only works with Sonnet 4.6 / Opus 4.6 on
Anthropic API (not Bedrock/Vertex/Foundry). Uses a background Sonnet 4.6 classifier to review
each tool call; safe actions proceed, dangerous ones block. Adds latency and token cost per
check.

## Session Resume

`claude --continue` resumes the most recent session scoped to the current project directory.

No session ID needed — Claude tracks sessions per project internally. Multiple terminals in
different project directories each resume their own session correctly. Multiple terminals in
the same directory is last-writer-wins.

**No prior session:** `claude --continue` in interactive mode fails hard with
"No conversation found to continue" (exit code 1). In `--print` mode it silently starts a new
session. Passing `--continue --system-prompt "..."` together does not help — `--continue` is
rejected before `--system-prompt` is considered.

**ACE strategy:** ACE prints a hint before exec'ing with `--continue`, telling the user to
run `ace new` if resume fails. No automatic retry — the user handles it.
See `docs/decisions/2026-04-09-resume-fallback.md`.

## MCP Registration

**Method: CLI** — non-interactive, user-scoped, handles merging.

```sh
claude mcp add -t http -s user <name> <url> [-H "K: V" ...]
```

Examples:

```sh
# OAuth server (no headers)
claude mcp add -t http -s user linear https://mcp.linear.app/mcp

# PAT server (with header)
claude mcp add -t http -s user github https://api.githubcopilot.com/mcp/ \
  -H "Authorization: Bearer ghp_xxxxx"
```

User scope (`-s user`) makes the server available across all projects. The CLI writes to the
user-level config and merges with existing entries. ACE never touches `.mcp.json` directly.

Before adding, ACE checks `claude mcp get <name>` to detect existing registrations. If the
server is already registered at any scope, ACE warns and skips — it does not overwrite.

**MCP list**: parsed from `~/.claude.json`.

## Linked Folders

| Folder      | Supported |
|-------------|-----------|
| `skills/`   | ✓         |
| `rules/`    | ✓         |
| `commands/` | ✓         |
| `agents/`   | ✓         |
