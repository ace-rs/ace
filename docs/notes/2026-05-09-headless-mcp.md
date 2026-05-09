_Predates the `docs/notes/` convention; preserved as-is for historical context._

# MCP in Headless/Web-Hosted ACE

Date: 2025-03-21

## Critical Finding

**Claude Code `-p` mode does NOT load HTTP MCP servers.** This is a known issue (GitHub
#34131). MCP servers configured in `~/.claude.json` work in interactive mode but fail silently
in headless `-p` mode. This is a hard blocker for headless Claude Code + MCP.

**OpenCode `serve` mode fully supports MCP** including remote HTTP with OAuth.

## How ACE Handles MCP Today

1. Schools declare MCP servers in `school.toml` as `[[mcp]]` entries with name, url, headers
2. Headers use `{{ placeholder }}` syntax — ACE prompts user for values on first run
3. ACE registers via CLI: `claude mcp add -t http -s user <name> <url> [-H "K: V" ...]`
4. ACE does NOT manage OAuth tokens — backends handle 401 → OAuth discovery → token storage
5. Remote-only model (decision 002) — no stdio/local MCP servers

### Token Storage by Backend

| Backend | OAuth Token Storage | Portable? |
|---------|-------------------|-----------|
| Claude Code | System keychain | No (machine-specific) |
| OpenCode | `~/.local/share/opencode/mcp-auth.json` | Yes (file-based) |
| Codex | `~/.codex/auth.json` or OS keyring | Partially |

## OAuth Headless Problem

MCP OAuth requires browser redirect. In headless/container environments:

- **Option A**: Pre-provision tokens via env vars or mounted secrets
- **Option B**: SSH port forwarding — complete OAuth on local machine, copy tokens
- **Option C**: Use PAT-based servers instead of OAuth (GitHub PAT in headers)
- **Option D**: LiteLLM proxy handles OAuth centrally with client_credentials flow

## Architecture Implications

### Design A: ttyd Wrapper (MVP)

MCP works as-is — full interactive terminal preserved. OAuth flows proceed normally (user
pastes URL in browser). Single-user limitation. Zero ACE changes.

### Design B: Claude Code `-p` Mode

**Blocked.** HTTP MCP servers don't load in `-p` mode (issue #34131). Workarounds:
- Switch to OpenCode `serve` instead
- Wait for Claude Code fix
- Use only non-MCP workflows in headless mode

### Design C: OpenCode `serve` Mode (Recommended for MCP)

Full MCP support via native REST API. OAuth still needs browser for initial auth but tokens
persist in `~/.local/share/opencode/mcp-auth.json` — can be bind-mounted or seeded from
secrets manager.

### Design D: LiteLLM Proxy

LiteLLM includes MCP gateway — acts as central OAuth broker (PKCE for interactive,
client_credentials for machine-to-machine). Handles token refresh automatically. Multi-user
capable. Extra infrastructure cost.

## Comparison

| Design | MCP Support | OAuth Headless | Multi-user | Blocker |
|--------|-------------|---------------|-----------|---------|
| ttyd wrapper | Full | Browser redirect | No | None |
| Claude `-p` | **Broken** | N/A | N/A | Issue #34131 |
| OpenCode serve | Full | Solvable | Per-container | Initial OAuth needs browser |
| LiteLLM proxy | Full | Solved | Yes | Extra infra |

## Multi-User MCP

- **Per-user tokens** (recommended): each user authenticates with own GitHub/Linear identity,
  gets user-scoped data. Requires isolated token storage per user.
- **Shared service account**: single token for team, only viable for read-only non-user-scoped
  resources. Simpler but less secure.

## Placeholder Substitution in Web Context

Currently ACE prompts via CLI (`ace.prompt_text()`). In web-hosted scenario:
- Web form replaces CLI prompt for placeholder values
- API-driven: HTTP POST with credentials, or env vars
- Per-user: store in per-user config
- Per-team: shared service account token in env

## Implementation Checklist

- [ ] Avoid Claude Code `-p` with HTTP MCP until #34131 fixed
- [ ] Prefer OpenCode `serve` for headless MCP workflows
- [ ] Web form for placeholder substitution (replace CLI prompts)
- [ ] Token pre-provisioning: mount secrets or env vars for MCP PAT/API keys
- [ ] OpenCode token persistence: bind-mount `~/.local/share/opencode/`
- [ ] Per-user isolation: separate volumes per user ID
- [ ] Consider LiteLLM proxy for multi-user shared MCP
- [ ] Test: verify MCP servers load before executing user prompts
