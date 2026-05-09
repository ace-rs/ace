_Predates the `docs/notes/` convention; preserved as-is for historical context._

# Headless Backend Mode — Research Notes

Date: 2025-03-21

## Current State

ACE uses `exec()` (process replacement) in `src/state/actions/exec.rs` to hand off to the
backend binary. No subprocess communication — ACE dies, backend takes over the terminal.

## Backend Headless Capabilities

| Backend      | Headless Mode | Mechanism                                                      |
|--------------|---------------|----------------------------------------------------------------|
| Claude Code  | Yes           | `-p` flag, stdout JSON/streaming, `--session-id` for continuity |
| OpenCode     | Yes (best)    | `opencode serve --port 4096` — full REST API, persistent server |
| Codex        | Likely        | CLI args, less documented                                      |

## Claude Code `-p` Mode

- Non-interactive: `claude -p "prompt" --output-format json`
- Streaming: `--output-format stream-json` for NDJSON token-by-token output
- Session continuity: `--resume <session_id>`
- System prompt: `--system-prompt` or `--system-prompt-file`
- Tool control: `--allowedTools "Read,Edit,Bash"`
- Structured output: `--json-schema '{"type":"object",...}'`
- ~800ms startup per invocation

## OpenCode Server Mode

- `opencode serve --port 4096 [--hostname 127.0.0.1] [--cors <origin>]`
- OpenAPI 3.1 REST endpoints: Sessions, Messages, Files, Providers, Config, Tools
- Auth: `OPENCODE_SERVER_PASSWORD` env var (HTTP basic auth)
- Persistent server, sub-100ms per request
- TUI control endpoint for remote driving

## Architecture for Web UI

```
Web UI (browser)
    ↓ HTTP/WebSocket
ACE Server (new Rust component)
    ↓ subprocess spawning
    ├─ Claude Code: claude -p "..." --system-prompt "..."
    ├─ OpenCode: opencode serve OR opencode run
    └─ Codex: codex -p "..."
```

## Key Changes Required in ACE

1. `exec.rs` — new `run_headless()` returning `Child` instead of replacing process
2. New HTTP server layer (routes: `/api/chat`, `/api/sessions`, etc.)
3. WebSocket or SSE for streaming token output
4. Credential pass-through to subprocess env

## Blockers

- ACE's `exec()` model must change to `Command::spawn()` + pipe capture
- Claude Code skills (like `/commit`) only available in interactive mode
- Credential management for containerized environments
- `inquire` TUI prompts need web-based alternatives

## Detailed: How ACE Invokes Backends Today

Invocation in `src/state/actions/exec.rs`:

```rust
pub fn run(&self, _ace: &mut Ace) -> Result<(), std::io::Error> {
    let mut cmd = Command::new(self.backend.binary());
    cmd.current_dir(&self.project_dir);
    for (key, val) in &self.env {
        cmd.env(key, val);
    }
    cmd.arg("--system-prompt").arg(&self.session_prompt);
    cmd.args(&self.backend_args);
    use std::os::unix::process::CommandExt;
    let err = cmd.exec();  // REPLACES current process
    Err(err)
}
```

Flow: Load config → resolve backend → build session prompt → `exec()` replaces ACE process.
No subprocess communication, no pipes, no I/O redirection. Inherits stdin/stdout/stderr.

## Detailed: Claude Code Non-Interactive Examples

```bash
# Single-turn query
claude -p "Find and fix the bug in auth.py" --allowedTools "Read,Edit,Bash"

# Structured JSON output
claude -p "Extract function names" \
  --output-format json \
  --json-schema '{"type":"object","properties":{"functions":{"type":"array","items":{"type":"string"}}}}'

# Streaming NDJSON
claude -p "Refactor this module" --output-format stream-json --verbose

# Session continuity
claude -p "Continue the refactor" --resume <session_id>
```

## Detailed: OpenCode Server Endpoints

- Sessions: create, list, get, delete
- Messages: send, list, stream
- Files: read, write, list
- Providers: list models, switch
- Config: get/set
- Tools: list, invoke
- TUI Control: drive terminal UI remotely (plugin architectures)
- mDNS discovery support
- CORS configurable

## Detailed: Architecture Options

### Option 1: Claude Code `-p` (simplest)

```
Web UI → ACE Server (Rust) → claude -p "..." --output-format json → stdout → parse → return
```

Advantages: single/multi-turn via --session-id, structured output, token streaming, inherits
tools/skills/MCP. Limitation: ~800ms startup per invocation, no bidirectional comms.

### Option 2: OpenCode Server (bidirectional)

```
Web UI → opencode serve (REST on :4096) → full API surface
```

Advantages: persistent server, low latency, rich API. Limitation: less mature ecosystem.

### Option 3: Hybrid (Claude Code SDK)

```
Web UI → ACE Server → Claude Code SDK (Python/TS) → CLI subprocess → NDJSON protocol
```

Richer control: callbacks, structured messages, tool approval handlers.

## Implementation Checklist

1. Create ACE server layer (Axum or similar)
   - HTTP routes: POST `/api/chat`, GET `/api/sessions`, etc.
2. Subprocess execution
   - Replace `exec()` with `Child` spawning
   - Capture stdout/stderr with pipes
   - Parse JSON output from `-p` mode
3. Session management
   - Map web session ID to backend session ID
   - Persist across server restarts
4. Credential handling
   - Accept API keys from environment or web config
   - Inject into subprocess environment
5. Output streaming
   - `--output-format stream-json` for real-time updates
   - Forward NDJSON events via WebSocket
6. System prompt integration
   - Keep existing prompt building logic
   - Pass via `--system-prompt` or `--system-prompt-file`
7. Web frontend
   - Chat interface, session browser, file tree
   - Settings panel for model, tools, effort level
