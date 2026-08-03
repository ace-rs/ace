# `ace connect` — Cross-Backend Agent Bridge

**Designed, not yet implemented.** No `connect` subcommand exists in `src/cmd/`; the
`ace-connect` skill and its shell scripts (`scripts/*.sh` in the skill) carry the
behavior until the binary lands, at which point the skill collapses to a thin pointer.
Work is tracked in the Outline ACE collection.

The bridge is part of the `ace` binary — not a separate binary or repo. The hard part
is the inbound **listen → inject → wake** path, which diverges per backend, and ace
already owns exactly that abstraction: the `Kind` registry and `dispatch!` macro in
`src/backend/mod.rs`, the capability-mask pattern (`Kind::features()` — wake-idle
becomes a new `FEATURE_WAKE_IDLE` bit; branch on feature bits, never backend name),
`exec_session`/`exec_one_shot` launch wrapping, and the authoritative slug computation
(`project_dir` + `Kind` + school). A separate repo would rebuild the backend registry
and guarantee drift — the worst outcome for a backend-uniformity feature.

## North star

Let local (later remote) agents talk quickly, token-efficiently, with minimal extras —
"solo senior engineer in front of a tmux with 100s of panes." Machinery lives in the
binary, not in agent context: the model sees a message only when it must act.

## Architecture: shared core + per-backend adapter

- **Shared core** (port of the proven shell implementation): socket dir
  `${XDG_RUNTIME_DIR:-$HOME/.ace/run}/messages/` mode 0700; slug
  `<parent>.<workdir>.<backend>`; `<slug>.sock`+`.pid`; discover (sweep dead pids);
  send (one line, strip tab/CR/LF, single attempt); listen.
- **Sync, no tokio** (matches ace): listener = blocking
  `std::os::unix::net::UnixListener` accept loop; REST inject = `ureq`; codex bridge =
  blocking `tungstenite` (not tokio-tungstenite) JSON-RPC. Threads if concurrency is
  ever needed.

## Per-backend adapter contracts

| Backend  | Wake-idle | Mechanism |
| -------- | --------- | --------- |
| Claude   | always    | Monitor surface — the ONLY way to push into an idle session. MCP/hooks are pull-only, can't wake. So integration is NOT an MCP server. Agent runs `ace connect listen` under Monitor; binary owns parse/log/filter/dispatch; SKILL.md collapses to ~1 line. Preserve control/autonomous + `.inbox.log`. |
| OpenCode | always    | Server holds session. Inject via `POST /session/<sid>/message` (`ureq`, verify path vs `GET /doc`, honor `OPENCODE_SERVER_PASSWORD`). In-process variant is a TS plugin ace emits during `ace setup` (can't live in the Rust binary). Start sidecar+REST; promote to plugin only if needed. |
| Codex    | mode-dependent | Dual-mode, user picks at launch. Plain `codex` CANNOT be woken mid-idle (only surfaces on next `write_stdin` poll). Mode 1 `ace connect codex`: orchestrate `codex app-server` + JSON-RPC bridge + exec into `codex --remote`; wake via `turn/start`/`turn/steer`, `ThreadStatusChangedNotification` = idle. Mode 2 plain: next-turn delivery, native TUI, re-armed each turn. |

## Wire format

`from=<slug>\tto=<slug>\tbody=<text>`, one line, <~500 chars (Claude truncates;
larger → tmp file path in body). Verbs: ACK WAIT DONE ASK STUCK FILE CTX NACK
(extensible). Caveman rules; report bodies = dash steps ≤5 words. Control-mode inbox:
append `<ISO8601-UTC>\tfrom=<slug>\t<body>` to gitignored `.inbox.log`.

## Out of scope

Auth, encryption, cross-machine (design the wire so it stays *possible* later),
persistence, threading, acks/retries/delivery guarantees. Single-user trust,
fire-and-forget.

## Open questions (implementer)

1. Shared backend crate? Fold-in reuses `src/backend/` directly; extraction would need
   a workspace crate — bigger refactor.
2. Codex wake-idle modeling: runtime field on launch request vs mode enum.
3. Codex app-server: identify the attached TUI thread among many; TUI+bridge
   co-injection safety; `turn/steer` vs `turn/start`.
4. Autonomy mode (control/autonomous) config home: ace.toml? per-invoke flag? default
   control.
5. `ace connect` subcommand tree shape vs `src/cmd/` conventions
   (`listen`/`send`/`discover`/`codex`).
