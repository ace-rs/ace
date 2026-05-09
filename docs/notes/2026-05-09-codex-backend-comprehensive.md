_Predates the `docs/notes/` convention; preserved as-is for historical context._

# Codex CLI as ACE Backend — Comprehensive Review

Date: 2025-03-21

## Status

Codex CLI is **actively maintained** (Apache-2.0, Rust-based, open source). Latest models:
GPT-5.3-Codex, GPT-5.2-Codex. Not deprecated — distinct from the old Codex API.

## ACE's Current Codex Support: Minimal

| Dimension | Status |
|-----------|--------|
| Binary name | `codex` |
| Backend dir | `.agents` |
| Instructions file | `AGENTS.md` |
| Installation | GitHub tarball download (implemented) |
| MCP registration | **Not implemented** (deferred) |
| Readiness check | **Not implemented** |
| Folder support | Skills only (rules, commands, agents unsupported) |
| Integration tests | **None** |

## Headless Mode

Codex has **full headless support** via `codex exec`:

- `codex exec "prompt"` — non-interactive, streams events to stderr, prints final to stdout
- `--json` flag — JSONL output (thread.started, turn.started, turn.completed, item.*, error)
- `--output-last-message <path>` — writes final message to file
- Session continuity: `codex resume <SESSION_ID>` or `codex resume --last`
- Approval defaults to "Never" in non-interactive mode — no TTY needed

## Authentication

| Method | Support |
|--------|---------|
| `OPENAI_API_KEY` env var | Full |
| OAuth (ChatGPT login) | Full (browser-based) |
| Headless OAuth | SSH port forwarding to localhost:1455 |
| Token storage | `~/.codex/auth.json` or OS keyring |
| Token portability | Yes — copy `~/.codex/auth.json` to container |
| LiteLLM proxy | Partial — `OPENAI_BASE_URL` works for basic use, tool call issues with gpt-5-codex |

## MCP Support

| Feature | Status |
|---------|--------|
| HTTP servers | Full |
| STDIO servers | Full |
| CLI management | `codex mcp add/remove/list` |
| OAuth for MCP | Full (managed in-session via `/mcp`) |
| Token storage | `~/.codex/auth.json` or OS keyring |
| Codex as MCP server | Full (can run Codex itself as MCP server over stdio) |

Key difference from Claude/OpenCode: Codex MCP auth and management are handled in-session via
`/mcp`. ACE should register servers cleanly, then defer auth and ongoing MCP management to
the backend.

## SDK / Programmatic Use

| SDK | Status |
|-----|--------|
| TypeScript | `@openai/codex-sdk` — spawns CLI, JSONL over stdin/stdout |
| Native Rust | `@codex-native/sdk` (napi-rs bindings, no child process) |
| Python | None |

OpenAI Agents SDK can orchestrate Codex via MCP or subprocess.

## Tool Execution & Permissions

Three approval modes:
- **Suggest** (default): read-only, must approve writes/commands
- **Auto Edit**: auto-edit files, approve shell commands
- **Full Auto**: fully autonomous (sandboxed, network-disabled by default)

Sandboxing: Landlock/seccomp on Linux, Seatbelt on macOS. May need
`--sandbox danger-full-access` in some container configs.

## System Prompt

- Global: `~/.codex/instructions.md`
- Project: `codex.md` or `AGENTS.md`
- Skills system: reusable instruction bundles (same SKILL.md format as Claude Code)
- `--system-prompt` flag: **needs verification** — ACE spec assumes it exists but not confirmed

## Team Sharing

- `.codex/config.toml` can be checked into repo for team defaults
- No native org/team scope for MCP (must be repo-local)
- Portkey integration for virtual keys, budget limits
- ChatGPT Teams accounts supported for org billing

## Comparison: All Three Backends

| Dimension | Claude Code | OpenCode | Codex |
|-----------|------------|----------|-------|
| Headless mode | `-p` flag | `opencode serve` (REST) | `codex exec --json` |
| MCP in headless | **Broken** (#34131) | Full | Full |
| Auth method | OAuth + API key | OAuth + API key | OAuth + API key |
| MCP auth | Auto on 401 | Auto on 401 | In-session `/mcp` |
| Token portability | No (keychain) | Yes (JSON file) | Yes (JSON file) |
| SDK languages | Python, TypeScript | Go | TypeScript, Rust |
| LiteLLM compat | Good | Good | Partial (tool call issues) |
| Open source | No | Yes | Yes |
| Sandbox | Yes | No | Yes |
| Session resume | `--resume <id>` | API sessions | `codex resume <id>` |

## Gaps in ACE's Codex Support

1. **MCP registration not implemented** — should prefer `codex mcp add`, with direct
   `config.toml` editing only as fallback if the CLI cannot express the needed config
2. **Readiness check missing** — should verify `~/.codex/auth.json` or `OPENAI_API_KEY`
3. **`--system-prompt` flag unverified** — ACE assumes it; needs confirmation
4. **No integration tests** for any Codex flow
5. **LiteLLM proxy issues** not documented — tool calls and MCP format mismatch
6. **Post-registration Codex MCP guidance** not implemented (should direct users to `/mcp`
   inside Codex)

## Viability as Headless Backend

**Viable with caveats.** Codex has better headless support than Claude Code (MCP works in
`codex exec`, unlike Claude's `-p` mode). But LiteLLM compatibility issues and backend-specific
MCP behavior still add friction.

### Recommendation

For web-hosted ACE headless backend:
1. **OpenCode `serve`** — best REST API, full MCP, lowest latency
2. **Codex `exec --json`** — good headless, full MCP, open source
3. **Claude Code `-p`** — mature CLI but MCP broken in headless (#34131)

ACE should prioritize completing Codex support (MCP registration, readiness check, system
prompt verification) since Codex offers a viable alternative when Claude Code's headless
MCP is blocked.
