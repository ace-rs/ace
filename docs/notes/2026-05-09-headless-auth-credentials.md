_Predates the `docs/notes/` convention; preserved as-is for historical context._

# Authentication & Credentials for Web-Hosted ACE

Date: 2025-03-21

## How Claude Code Handles Credentials Today

### Credential Files

- `~/.claude.json` — auth metadata, onboarding state
- `~/.claude/.credentials.json` — OAuth tokens (accessToken, refreshToken, expiresAt)
- macOS also uses Keychain under "Claude Code-credentials"
- Both `.claude.json` and `.credentials.json` required to skip login flow

### Authentication Paths

1. **OAuth Login (default)**: `claude login` → browser OAuth → `sk-ant-oat01-*` tokens
2. **Environment Variable**: `ANTHROPIC_API_KEY` takes precedence over subscription auth
3. **Non-interactive**: `-p` flag, reads env var directly, no prompts

### ACE's Current Model

ACE does not manage credentials. `exec.rs` passes env vars to the backend subprocess, which
inherits the user's home directory config. ACE registers MCP servers but doesn't store their
OAuth tokens — backends handle that internally.

## Architecture Options

### Option 1: Direct Key Injection (Simplest, Not Recommended)

```
Web UI → ACE Server (stores ANTHROPIC_API_KEY in encrypted DB)
  → injects into subprocess env
  → claude -p "..." reads env var
```

Problems:
- Agent can read `/proc/*/environ` and exfiltrate the key
- No per-user budgeting
- Violates Anthropic's Jan 2026 restriction on third-party harnesses

### Option 2: LiteLLM Proxy (Recommended for Self-Hosted)

```
Web UI → ACE Server
  → looks up user's LiteLLM virtual key
  → spawns: claude -p "..." --api-base "http://litellm:4000" --api-key "<virtual_key>"
  → Claude Code calls LiteLLM (not Anthropic directly)
  → LiteLLM validates key, enforces budget, injects real ANTHROPIC_API_KEY
  → forwards to Anthropic API
```

LiteLLM virtual key features:
- Per-user/team keys with budget controls (hard limit + soft warning)
- Rate limiting: max_parallel_requests, tpm_limit, rpm_limit per team
- Cost tracking per key/team/user
- Key rotation (time-based, requires DB)
- RBAC for team members

Advantages: real credentials never visible to subprocess, multi-tenant native, usage accounting.
Disadvantages: adds proxy hop (~50-100ms), requires PostgreSQL + Redis.

### Option 3: OAuth Delegation (For Teams/Enterprise)

```
Web UI → OAuth flow against Claude.ai/teams
  → access_token + refresh_token stored in session
  → ACE Server calls Claude Team API for temporary session_token (5-min TTL)
  → spawns: claude -p "..." --token <session_token>
  → counts against team quota
```

Advantages: official Anthropic auth, team billing built-in, short-lived tokens.
Disadvantages: requires Teams/Enterprise plan, tied to Anthropic OAuth infra.

### Option 4: Claude Code Session Resume (Single-User Only)

```bash
session_id=$(claude -p "Start" --output-format json | jq -r '.session_id')
claude -p "Continue" --resume "$session_id"
```

Sessions persist 24h, 200k token context. Not suitable for multi-tenant web hosting.

## Recommendation Matrix

| Approach | Security | Multi-Tenant | Plan Type | Viability |
|----------|----------|-------------|-----------|-----------|
| Direct Injection | Low | Complex | Console | Not recommended |
| LiteLLM Proxy | High | Native | Console | Recommended (self-hosted) |
| OAuth Delegation | Highest | Built-in | Teams/Ent | Recommended (official) |
| Session Resume | High | None | Any | Single-user only |

## Security Considerations

1. **Phantom Token Pattern**: Never expose real API keys to subprocess. Use proxy URL +
   virtual token. Real credential injected server-side by proxy.

2. **Subprocess Environment Inspection**: Claude Code could read `/proc/*/environ`.
   Mitigate with proxy pattern or short-lived tokens.

3. **Token TTLs**: OAuth access tokens ~1h, session tokens ~5min, LiteLLM virtual keys
   use rotation policy.

4. **Rate Limiting**: Enforce upstream (LiteLLM or OAuth provider).

## Implementation Checklist

### LiteLLM Path:
- Set up LiteLLM with Anthropic provider credentials
- Create PostgreSQL/Redis backend for virtual key storage
- Implement key provisioning: web signup → create LiteLLM virtual key
- Store virtual_key → user_id mapping
- Update exec.rs: `spawn()` instead of `exec()`, pass --api-base and --api-key
- Add budget enforcement webhook
- Implement key rotation policy

### OAuth Path:
- Register web app with Claude OAuth (client_id, client_secret)
- Implement authorization_code flow
- Store tokens in encrypted session store
- Create token refresh background task
- Implement session_token provisioning per request
- Update exec.rs to support --token flag
- Add cleanup: revoke tokens on logout

### Both:
- Update exec.rs from exec() to spawn() + pipe capture
- Parse JSON output, return to HTTP layer
- Session management (web session → backend session)
- Request timeout handling (~800ms startup)
- Ensure credentials never leak to logs or error messages
