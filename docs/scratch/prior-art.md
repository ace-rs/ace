# Prior art digest

Not spec/decision because: this is a condensed record of exploration that never became
either — kept so the same ground isn't re-surveyed, not because any of it is current.

One section per absorbed note. Each states what was explored, what still holds, what
went stale, and where the live answer lives now. The absorbed originals are in git
history (`git log --diff-filter=D -- docs/notes docs/scratch`).

Everything below dates from the 2026-05-09 dump of pre-`docs/` session research (much of
it authored 2025-03-21). Treat every code path, file name, and version claim as
historical: `src/state/actions/` no longer exists (actions moved to `src/actions/`
per [architecture § actions](../spec/architecture.md)), and the backend matrix
has changed since.

---

## Web-hosted / headless ACE (3 notes)

**Absorbed:** headless-backend, headless-mcp, headless-auth-credentials.

The question: could ACE run as a server, driving backends non-interactively behind a web
UI? Three notes surveyed the execution model, MCP behavior, and credential handling.

**Findings that still hold**

- ACE hands off with `exec()` — process replacement, no pipes, no subprocess
  communication. Any server mode requires `spawn()` + pipe capture instead. That
  handoff model is still the design; see [`spec/backend.md`](../spec/backend.md).
- Backend headless surfaces differ sharply: Claude Code `-p` (one-shot, JSON/NDJSON
  output), OpenCode `serve` (persistent REST), Codex `exec --json` (JSONL events).
- Claude Code `-p` did not load HTTP MCP servers (upstream issue #34131) — a hard
  blocker for headless Claude + MCP at the time. OpenCode `serve` and `codex exec`
  both did.
- Credentials: injecting a real API key into a subprocess env is unsafe (the agent can
  read its own `/proc/*/environ`). The workable patterns were a proxy holding the real
  key and handing out virtual keys, or short-lived OAuth session tokens.
- MCP OAuth needs a browser redirect, which is the recurring headless blocker;
  workarounds were pre-provisioned tokens, SSH port-forwarding, or PAT-based servers.

**Stale**

- Every code excerpt (`src/state/actions/exec.rs`).
- Backend feature tables and upstream issue status — all unverified since 2025-03-21.
- The LiteLLM-proxy and multi-tenant sketches assume a hosted product ACE never became.

**Where it went.** Nowhere yet — ACE remains a local launcher. MCP scope is ruled by
[remote-only MCP](../decisions/2026-03-04-remote-only-mcp.md); the current credential
posture is [`spec/authentication.md`](../spec/authentication.md) and
[`spec/mcp.md`](../spec/mcp.md). If hosted ACE is ever revisited, the `exec()` →
`spawn()` change and the proxy-credential pattern are the two conclusions worth
carrying forward; re-verify everything else against current backends.

---

## Agent ecosystem survey

**Absorbed:** agent-ecosystem-research.

A wide survey of Claude Code agent teams, channels, monitors, and the plugin/extension
stacks of all four backends, asking two questions: could ACE have been a plugin, and
what could a school mandate?

**The conclusion that stuck: ACE must remain a launcher.** A plugin runs *inside* one
backend and cannot choose which backend to start, set env vars for the host process,
manage school lifecycle, or resolve skills before the backend boots. That verdict is
now baked into [`spec/architecture.md`](../spec/architecture.md).

**Also still true**

- MCP is the one extension point every backend shares — the most portable thing a
  school can mandate.
- ACE's link step is proto-plugin generation: symlinking school content into backend
  directories is what a plugin manifest would formalize.
- Cross-backend agent coordination had no standard protocol, so ACE was well-positioned
  to mediate but building it was premature. That premise was later overtaken — the
  bridge became a first-party feature; see
  [ace connect spec](../spec/connect.md).

**Stale**

- The per-backend parity tables (versions, feature availability) are a snapshot of
  early 2026 and were already drifting when written.
- Droid rows throughout: Droid was dropped as a backend.
- The `[[plugins]]` / `[channels]` / `[agent_teams]` `school.toml` sketches. Plugin
  support is explicitly out of ACE's scope — ACE handles skills.

---

## Plugins & macros design

**Absorbed:** plugins-macros.

A complete design for two extension points: lifecycle **plugins** (shell scripts a
school runs at `pre-setup`/`post-setup`/`post-prepare`/`pre-exec`, receiving context in
`ACE_*` env vars, with per-hook failure policy and a timeout) and **macros** (named
TOML shortcuts expanding to pre-filled session prompt + backend args + env, invoked as
`ace run <name>`).

**Status: not pursued.** Plugins are out of scope — ACE provisions skills, it does not
execute school-supplied code. Macros were never taken up either, though nothing rules
them out; the design is coherent and self-contained if the need returns.

**Worth keeping from it**

- The hook-point set and the fail-open/fail-closed split (warn by default, fatal only
  when the school marks a hook required) is a sane default shape.
- Macro merge semantics: whole-macro replacement by config layer, no field-level merge
  — matches ACE's existing layering.

**Stale.** Its file-by-file implementation plan targets the old `src/state/actions/`
and `src/config/tree.rs` layout.

---

## Codex backend review + OpenCode/Droid implementation plan

**Absorbed:** codex-backend-comprehensive, backend-plan.

A deep review of Codex CLI as a backend, plus the follow-on plan to implement OpenCode
and Droid.

**Findings that shaped the specs**

- Codex is fully headless-capable (`codex exec`, `--json`, `--output-last-message`,
  `codex resume`) with working MCP in headless mode, unlike Claude `-p` at the time.
- Codex MCP is CLI-managed (`codex mcp add/remove/list`) but auth happens *in-session*
  via `/mcp` — so ACE should register cleanly and defer auth to the backend.
- Readiness is `~/.codex/auth.json` or `OPENAI_API_KEY`.
- OpenCode carries the session prompt through an agent file launched with
  `--agent ace`, one-shot via `opencode run --agent ace`, and needs a direct config
  write for MCP because the interactive wizard is unusable.
- The OpenCode agent-file/linked-`agents/`-folder collision (a whole-dir symlink would
  swallow `ace.md`) was real; the recommendation was to write the agent file to a
  global path since it belongs to the user, not the project.

**Where it went.** [`spec/backends/codex.md`](../spec/backends/codex.md) and
[`spec/backends/opencode.md`](../spec/backends/opencode.md) carry the current
contracts. Droid was dropped as a backend; its half of the plan is dead.

**Stale.** Model names, SDK availability, LiteLLM compatibility notes, the
Claude/OpenCode/Codex comparison table, the "gaps in ACE's Codex support" list, and
all Linear (PROD9) issue references — current task ownership lives in
[the repository backlog](../backlog/README.md).

---

## Test-suite speedup audit (2026-05-27)

**Absorbed:** test-audit. Executed; the remaining *build*-side menu still lives in
[build & test speedups](2026-05-09-build-test-speedups.md).

Audited 20 integration binaries (~204 tests, 215 `ace` subprocess spawns, 31
`setup_remote_school` fixtures) and shipped the top three wins (commits `dc85d18`,
`8dafe6f`): a process-wide `OnceLock` cache template so `school_init` tests stop
cloning `ace-rs/school` (~10s → ~1.5s), a per-specifier shared origin+cache template
copied per test, and a local-redirect helper making clone-failure tests fail in ~20ms
instead of on a network timeout. Warm `cargo test` went 9.4s → 6.8s (~28%).

**Deliberately not pursued, with reasons that still stand**

- **In-process action-struct tests.** Blocked on threading paths through `Ace` instead
  of reading `XDG_*` env — parallel in-process tests share global env. Substantial
  production refactor for test-only benefit.
- **`cargo-nextest`.** Ruled out by chakrit. (An earlier note lists it as a Tier-1 win;
  that entry is superseded by this call.)
- **Binary consolidation, 20 → ~6.** Only worth doing alongside an in-process
  conversion, not on its own.
- **Lazy `canonicalize()` in `TestEnv::new()`.** Sub-millisecond cost, and macOS
  symlink-equality assertions depend on the eager form.

**The floor.** What remains slow is legitimate multi-step `ace` flows: ~50–150ms per
subprocess invocation × ~200 invocations, parallel within a binary and serial across
binaries. Further wins need an in-process path or fewer subprocess hops — there is no
cheap win left on the test side.

---

## Skill-model rearchitect: spec reconciliation (2026-06-05)

**Absorbed:** spec-reconciliation-plan. Fully executed — both the docs batch
(2026-06-05) and the code series (slices 1–7, plus two audit passes on 2026-06-08).

A pre-implementation spec-consistency audit found `architecture.md` still teaching the
superseded admission model. The fix was ratified and applied as one batch: eight spec
files edited plus a new decision. Two forks were resolved in the same pass — `MatchHandle`
was **cut** (replaced by boundary glob-validation as a warn-diagnostic at the resolver
seam) and `src/resolver/` was **dissolved** into `config/resolve/` + `skills/resolve/`,
on the rule that *resolution lives with the typed data it reads and stamps*.

**Where it went.** [`spec/architecture.md`](../spec/architecture.md),
[`spec/skills/`](../spec/skills/), and
[resolver dissolution](../decisions/2026-06-05-resolver-dissolution.md). The defect
catalogue that motivated the whole rearchitect survives in
[the rearchitect note](2026-06-02-skill-model-rearchitect.md).

---

## School skill proposals (2026-05-09)

**Absorbed:** pending-school-prs. All landed in `prod9/school` — verified against the
live `rust-coding` skill: Rust 2024 let-chains, the inherent `label()` convention for
short enum strings, and clippy-as-a-done-gate (`cargo clippy --all-targets`, since
`#![deny(warnings)]` covers rustc warnings only). The `ace-audit` skill shipped too.

The note's one durable observation: that 2026-04-14 clippy cleanup fixed 30 errors of
which only let-chains was genuinely post-training-cutoff — the other 29 were
long-standing lints that drifted in because agents treated a clean `cargo build` as
done. That is why the clippy gate exists.
