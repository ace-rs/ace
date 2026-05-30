# `ace connect` — fold cross-backend A2A bridge into ace

2026-05-30 — handoff from peer agent `ace-rs.connect.claude` over the ace-connect bridge.
**Deferred: tackle after 0.8 is released.** Capture only.

**Decision (locked with chakrit):** fold the cross-backend agent-to-agent bridge into
`ace` as an `ace connect` feature. Do **not** ship a separate binary or repo. This
overrides the earlier scoping recommendation (separate monorepo) — see "Why fold" below.

**Full self-contained handoff spec:** `ace-rs/connect` repo at
`docs/spec/ace-connect.md` (sibling checkout: `/Users/chakrit/Documents/ace-rs/connect/`).
That repo may be archived once docs migrate into `ace/docs/` (§ disposition, OPEN). Key
content mirrored below so it survives if that repo goes away.

## North star

Let local (later remote) agents talk quickly, token-efficiently, minimal extras. "Solo
senior engineer in front of a tmux with 100s of panes." Machinery lives in the binary,
not in agent context — the model sees a message only when it must act.

## Why fold into ace (decisive)

The hard part is the inbound **listen → inject → wake** path, which diverges per backend.
ace already owns exactly that abstraction:

- `src/backend/mod.rs` — `Kind { Claude, Codex, Flaude, OpenCode }`, `dispatch!` macro to
  per-backend free fns, `Backend`/`Registry`. connect's 3 adapters become new fns on the
  same `src/backend/{claude,codex,opencode}.rs`.
- Capability-mask pattern already exists: `Kind::features() -> u32` with
  `FEATURE_NESTED_SKILLS`. Wake-idle becomes a new `FEATURE_WAKE_IDLE` bit. Branch on
  feature bits, never backend name (existing discipline).
- `Kind::exec_session` / `exec_one_shot` already launch+wrap agents; codex app-server
  orchestration is the same shape.
- ace already references connect (`src/backend/registry.rs` templates `codex.sh`).
- Distribution solved (homebrew-tap/install.sh/release.sh); a 2nd binary = 2nd pipeline.
- ace computes the slug authoritatively (`project_dir` + `Kind` + school).

A separate repo would rebuild the backend registry and guarantee drift — worst outcome
for a backend-uniformity feature.

## Architecture: shared core + per-backend adapter

- **Shared core (settled, port shell→Rust):** socket dir
  `${XDG_RUNTIME_DIR:-$HOME/.ace/run}/messages/` mode 0700; slug
  `<parent>.<workdir>.<backend>`; `<slug>.sock`+`.pid`; discover (sweep dead pids);
  send (one line, strip tab/CR/LF, single attempt); listen.
- **Sync, no tokio** (matches ace): listener = blocking `std::os::unix::net::UnixListener`
  accept loop (mirrors one-socat-per-message shell loop); REST inject = `ureq`; codex
  bridge = blocking `tungstenite` (NOT tokio-tungstenite) JSON-RPC. Threads if concurrency
  ever needed.

## Per-backend adapter contracts

| Backend  | Wake-idle | Mechanism |
| -------- | --------- | --------- |
| Claude   | always    | Monitor surface — the ONLY way to push into an idle session. MCP/hooks are pull-only, can't wake. So integration is NOT an MCP server. Agent runs `ace connect listen` under Monitor; binary owns parse/log/filter/dispatch; SKILL.md collapses to ~1 line. Preserve control/autonomous + `.inbox.log`. |
| OpenCode | always    | Server holds session. Inject via `POST /session/<sid>/message` (`ureq`, verify path vs `GET /doc`, honor `OPENCODE_SERVER_PASSWORD`). In-process variant is a TS plugin ace emits during `ace setup` (can't live in Rust binary). Start sidecar+REST; promote to plugin only if needed. |
| Codex    | mode-dependent | Dual-mode, user picks at launch. Plain `codex` CANNOT be woken mid-idle (only surfaces on next `write_stdin` poll). Mode 1 `ace connect codex`: orchestrate `codex app-server` + JSON-RPC bridge + exec into `codex --remote`; wake via `turn/start`/`turn/steer`, `ThreadStatusChangedNotification` = idle. Mode 2 plain: next-turn delivery, native TUI, re-armed each turn. |

## Wire format — SETTLED, port verbatim

`from=<slug>\tto=<slug>\tbody=<text>`, one line, <~500 chars (Claude truncates; larger →
tmp file path in body). Verbs: ACK WAIT DONE ASK STUCK FILE CTX NACK (extensible). Caveman
rules; report bodies = dash steps ≤5 words. Control-mode inbox: append
`<ISO8601-UTC>\tfrom=<slug>\t<body>` to gitignored `.inbox.log`.

## Out of scope (carry over)

Auth, encryption, cross-machine (design wire so it's *possible* later), persistence,
threading, acks/retries/delivery guarantees. Single-user trust, fire-and-forget.

## OPEN questions (chakrit / implementer, post-0.8)

1. Shared backend crate? Fold-in reuses `src/backend/` directly; extractable would need a
   workspace crate — bigger refactor, repo owner's call.
2. Codex wake-idle modeling: runtime field on launch request vs mode enum.
3. Codex app-server: identify attached TUI thread among many; TUI+bridge co-injection
   safety; `turn/steer` vs `turn/start`.
4. Autonomy mode (control/autonomous) config home: ace.toml? per-invoke flag? default
   control.
5. `ace connect` subcommand tree shape vs `src/cmd/` conventions (`listen`/`send`/
   `discover`/`codex`).
6. Fate of `ace-rs/connect` repo — design home or archive after migrating docs into
   `ace/docs/`.

## Migration

Existing `ace-connect` skill + `scripts/*.sh` stay functional until the binary lands
(this session uses them). On ship: skill collapses to a thin pointer; shell scripts
superseded by the binary.
