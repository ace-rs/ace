_Predates the `docs/notes/` convention; preserved as-is for historical context._

# Future Directions

Explored but parked. Each section captures the key finding and why it was shelved.
Sourced from research session 2026-03-21.

## Cloud Deployment Integration

ACE could generate deployment config files (render.yaml, fly.toml, etc.) from school
definitions. Evaluated Render, Railway, Fly.io, Coolify, Kamal.

**Conclusion:** Out of scope for ACE core. ACE is an environment-setup tool, not a deployment
orchestrator. Config-file generation is the only in-scope integration point — and even that
has low demand since most teams already have deployment pipelines.

## Web Hosting via Dagger + ttyd

Containerize ACE with ttyd (WebSocket terminal) for browser-based access. Three options
evaluated:

| Option | Effort | Multi-user | ACE Changes |
|--------|--------|------------|-------------|
| ttyd wrapper | ~130 LOC | No | None |
| Custom Axum API | ~1000 LOC | Yes | Major |
| Dagger build + K8s runtime | ~130 LOC | Yes | None |

**Conclusion:** ttyd wrapper is the simplest MVP (zero ACE changes). Worth revisiting if
browser-based ACE becomes a product requirement. Blocked by headless MCP issues (see
`2026-05-09-headless-mcp.md`).

## Self-Hosted Backend via Claude Agent SDK

Evaluated replacing CLI exec with Agent SDK (Python/TypeScript) to host the agentic loop
directly.

**Conclusion:** Not recommended. Agent SDK is Python-only (no Rust SDK). Claude Code `-p`
mode already provides headless access with all tools, MCP, and permissions built in.
Self-hosting would mean a Python subprocess bridge from Rust — significant complexity for
marginal gain. Estimated 2-3 weeks MVP, 2-3 months for CLI parity.

## Team Session Sharing

Current model: schools share config, credentials stay per-user. Five options evaluated
ranging from status quo to a full ACE web server.

**Conclusion:** Current model is sufficient. Phased approach if needed:
1. Short-term: enhance placeholder substitution in school.toml
2. Medium-term: document LiteLLM proxy pattern for shared budget/credentials
3. Long-term: separate ACE Server product (out of scope for CLI)

## Codebase Audit (2026-03-21)

Point-in-time audit against general-coding and rust-coding skills. Key findings at time
of audit:

- Zero `.unwrap()` in production code
- Error enums follow one-per-module convention
- Option<String> vs String usage consistent with serde-default pattern
- Integration test gaps identified: `ace school diff`, backend install flow, session prompt
  composition (some have since been addressed)
- Three-act grouping and deep nesting fixed in separate branches

**Status:** Snapshot only. Codebase has moved significantly since this audit.
