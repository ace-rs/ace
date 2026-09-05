# A — Backends

Source: [Outline][source], revision 39.
Status reconciled against repository records at `9df624a` on 2026-09-05.

[source]: https://outline.prodigy9.co/doc/a-backends-bgUEggvFml

Every backend a first-class peer — completion, per-backend config, normalization.

- [x] **backend-config-set** extend `ace config set` with `backends.<instance>.model` and
      `backends.<instance>.effort` dot paths. The instance name may identify a built-in or
      custom backend. Validate only the terminal field; model and effort values are
      backend-owned opaque strings and must pass through unchanged. The existing
      `--backend` flag remains a runtime selector, never a config mutation target. Shipped
      `97ba9e2`.
- [x] **backend-runtime-config** keyed `[backends.<name>]` schema plus one opaque
      model/effort pair on every resolved backend invocation — shipped `1ebb8d0`.
- [x] **17** complete OpenCode backend · *High* — shipped `eeb140e`; trust capability
      validation shipped `4b8c35a`; current implementation matches
      `docs/spec/backends/opencode.md` (agent-based prompt injection, JSON MCP
      registration, session/one-shot argv, readiness, and explicit unsupported MCP health
      check).
- [ ] **hermes** first-tier Hermes harness support — peer of claude/codex/opencode, not a
      long-tail target · *Deferred 2026-08-26* — *ruled 2026-08-03:* ships as a
      **built-in** `Kind`, not a `[backends.<name>]` table. The two are not alternatives:
      a built-in `Kind` is the behavior (launch, MCP registration, instructions file) and
      only a release adds one; a declaration must bind to an existing Kind and can only
      patch `env`/`cmd` or register a new name for it (`docs/spec/backend.md` §Custom
      Backends). Surface fully mapped in `docs/vendor/hermes.md` — the two facts that
      shape the implementation: **skills are per-`HERMES_HOME`, never per-project** (a
      hermes "profile" *is* a `HERMES_HOME` directory, so ACE sets that env var at a
      repo-local home and skips the named-profile registry), and `hermes mcp add` **cannot
      be scripted** (always blocks on `Enable all N tools?`, no `--yes`), so MCP
      registration must write `mcp_servers.<name>` into `config.yaml` directly. Also worth
      reading before designing the school shape: `hermes profile install <git-url>` +
      `distribution.yaml` is a close analogue (git source, `#ref` pin, distribution-owned
      vs user-owned paths on update).

**start-mode** moved to [M — Managed sessions, connect & workspaces](m-sessions.md) and is
owned by **start-pipeline**, **native-session-supervision**, **runtime-endpoints**, and
**component-supervision**. Backend-specific component materialization remains owned by A's
backend boundary.

## Ideas / later

* **127** `ace upgrade` also upgrades configured backends · **161** support rollback to an
  earlier version
* **149** survey Ollama's `launch` integration list (spike)
* **214** session name + color — **doc task only, do not build.** Every backend has its
  own native "name and color this session" feature (Claude Code calls it a session;
  opencode has the same thing under different naming). Moved here from H on 2026-07-26
  because the deliverable is a backend-capability survey, not CLI ergonomics. **The only
  task: study the shape across all backends and write it up in** `docs/vendor/` **first.**
  Nothing else is decided — not which backends expose it, not whether ACE defaults the
  session name to the project name, not where config would live. Same normalization family
  as per-backend config below and **159** polymorphic flags.
* 🆕 per-backend config — always pass flags like `--chrome` to a backend (overlaps 128)
* 🆕 backend-agnostic `--chrome` flag — a uniform ACE flag that ACE translates to each
  backend's equivalent (flip side of per-backend config: runtime passthrough vs
  config-time always-pass; relates to 159 polymorphic flags)
* 🆕 more harness targets — Pi / Cursor (note: Cursor was Cancelled-9, revisit). Hermes
  was promoted out of this list to first-tier support.
* 🆕 shared config, varying harnesses — teams share one skills/mcp/scripting set while
  using different models/harnesses

## Shipped

18, 128, 129, 54, 55, 157, 47, 35, 48, 60 (self-update).

## Roadmap items awaiting status verification

- [ ] **146** scope-aware backend selector validation — open in the Outline roadmap but
      absent from A's checklist; no closure evidence in the local ledger.
- [ ] **147** `ANTHROPIC_API_KEY` versus claude.ai login conflict — same source gap as
      146; verify before implementing.

The Cursor portion of the more-targets idea conflicts with cancelled **9**; it remains a
proposal requiring a new decision, not an approved revival.

**74** Windows verification remains hardware-gated and unverified; use
[the platform contract](../spec/platforms.md) for the supported target boundary. The local
session trail records a cross-check blocked by missing `x86_64-w64-mingw32-gcc`, not proof
of success or a request to change this machine.
