# Backend: Droid (Factory.ai)

Binary: `droid` | Dir: `.factory` | Instructions: `AGENTS.md`

## Readiness

`~/.factory/settings.json` exists. Created on first browser-based sign-in.

## Session Prompt

**No `--system-prompt` flag for interactive mode.**

### Interactive mode — managed AGENTS.md block

ACE injects the session prompt into `AGENTS.md` using a managed block with
markers, same pattern as `.gitignore`:

```markdown
<!-- ACE-managed — do not edit this block. -->
<session prompt here>
<!-- end ACE -->
```

User content outside the managed block is preserved. The block is
idempotently replaced on each `ace setup` / `ace` run. Droid loads
`AGENTS.md` automatically at task start in both interactive and exec modes.

### Exec mode (one-shot)

`droid exec` supports `--append-system-prompt-file <path>` — appends the
contents of a file to the built-in system prompt. ACE writes the session
prompt to a tmpfile and passes this flag. This bypasses `AGENTS.md` for
one-shot runs.

Other flags: `--append-system-prompt <text>` passes inline text (less
suitable for long session prompts).

### Exec mode flags

- `-f, --file <path>` — read prompt from file.
- `--auto <low|medium|high>` — autonomy level.
- `-m, --model <id>` — model selection.
- `-s, --session-id <id>` — continue existing session.
- `-o, --output-format <format>` — output format (default: text).
- `--cwd <path>` — working directory.
- `--enabled-tools` / `--disabled-tools` — tool filtering.

## Yolo Mode

`--skip-permissions-unsafe`

Also supports tiered autonomy via `--auto <low|medium|high>` on `droid exec`.

## MCP Registration

**Method: CLI** — similar to Claude.

```sh
droid mcp add <name> <url> --type http [--header "K: V" ...]
```

Examples:

```sh
# OAuth server (no headers)
droid mcp add linear https://mcp.linear.app/mcp --type http

# PAT server (with header)
droid mcp add github https://api.githubcopilot.com/mcp/ --type http \
  --header "Authorization: Bearer ghp_xxxxx"
```

Key differences from Claude's CLI:
- `--type http` instead of `-t http`
- `--header` instead of `-H`
- No scope flag — user-level by default

**MCP config files:**
- User-level: `~/.factory/mcp.json`
- Project-level: `.factory/mcp.json`

User config takes priority. Project-defined servers cannot be removed via
CLI.

**MCP list**: parsed from `~/.factory/mcp.json`.

## Linked Folders

| Folder      | Supported |
|-------------|-----------|
| `skills/`   | ✓         |
| `rules/`    | ✗         |
| `commands/` | ✗         |
| `agents/`   | ✗         |

Note: Droid uses `droids/` for custom subagents (not `agents/`). Agent
support deferred — requires folder name mapping (`agents/` → `droids/`)
that the current link system doesn't handle.
