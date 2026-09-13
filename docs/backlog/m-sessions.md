# M — Managed sessions, connect & workspaces

Source: [Outline][source], revision 12.
Status reconciled against repository records at `9df624a` on 2026-09-05.

[source]: https://outline.prodigy9.co/doc/m-managed-sessions-connect-workspaces-FdFXj4qMEO

Implementation tracker and triage guide only. Behavioral contracts remain authoritative in
the repository:

* `docs/spec/session.md`
* `docs/spec/connect.md`
* `docs/spec/workspace.md`
* `docs/spec/backend.md`
* `docs/spec/backends/{claude,codex,opencode}.md`

Current implementation at `9df624a` supervises one native `SessionProcess`. Earlier typed
component lists and controlled backend graphs were removed from production until
endpoints, readiness, primary handles, and a second owned process can enter together. This
backlog tracks ordering and completion; the specifications own behavior.

## Product model

The low-level primitive is one named ACE instance: one repository, resolved config,
backend session, primary thread, terminal runtime, and optional relay identity. `Ace` is
the instance; there is no separate instance-plan wrapper.

ACE owns one built-in launch pipeline:

```text
workspace expansion
  → configured Ace instances
  → feature requirements
  → backend component materialization
  → feature component decoration
  → local or mux execution
```

tmux owns process persistence, panes, windows, attachment, and switching. ACE-connect is
only the local fire-and-forget relay. Workspace mode composes several connected instances.
There is no task model, transcript UI, generic supervisor, remote protocol, or public
plugin ABI in the initial implementation.

## Now — establish one managed connected session

- [x] **start-pipeline** route preparation and launch through `Ace::start(StartMode)`;
      landed in `7ae516e`.
- [x] **native-session-supervision** supervise one native `SessionProcess`; `65dc1bf`
      established foreground ownership and `9df624a` reduced the singleton supervisor to
      direct waiting.
- [ ] **runtime-endpoints** introduce endpoint allocation, controlled backend components,
      protocol readiness, primary backend handles, and multi-process ownership together;
      this absorbs the local ledger's controlled-startup item. `component-foundation` and
      `backend-component-graphs` are historical slice names, superseded by `9df624a`, not
      currently shipped components.
- [ ] **component-supervision** extend native supervision to readiness-aware cohorts with
      owner-classified cascades and coordinated shutdown. Regular threads and channels
      enter with concurrent workloads; the singleton already waits directly.
- [ ] **mux-runtime** execute component lists in tmux and add
      `ace session {start,list,inspect,attach,stop}`; tmux remains the terminal UI.
- [ ] **connect-core** add `[connect] enabled = true`, relay identity, Unix-socket
      discovery/send/monitor/status, and component decoration; preserve fire-and-forget
      semantics.
- [ ] **connect-codex** inject incoming messages into the Codex primary thread created by
      the managed component graph; never address native child threads.
- [ ] **connect-opencode** inject incoming messages into the OpenCode primary session
      created by the managed component graph.
- [ ] **connect-claude** move the proven monitor receive path into the binary and report
      unsupported idle injection honestly; never emulate control with tmux keystrokes.

## First Codex managed-runtime implementation plan

**Proposed; implementation approval pending.** This plan serves the implementer and
reviewer of **runtime-endpoints**, **component-supervision**, and the single-instance
portion of **mux-runtime**. It delivers a usable, inspectable managed Codex session before
**connect-core** and **connect-codex** depend on it. Existing specs govern behavior; the
implementation choices below remain proposed.

Planning authority: “sharpen the draft a bit and then start planning plz”, retained in
`.ace/save.ledger.md`. On 2026-09-13 Chakrit replied “approve” to the specific fresh-start
order: server ready → native terminal creates primary → retain its initial ID → publish
recipient readiness. That ordering is now in the session, connect, and Codex specs;
approval of the ordering does not authorize application code or dependency changes.

### Scope and acceptance

Deliver the existing `ace session start [path] [--name <name>] [--detach]`, `list`,
`inspect`, `attach`, and `stop` surface for a single managed Codex instance on Unix.
tmux owns persistence and terminal attachment. Starting the same live named instance
attaches to it unless `--detach` requests returning its identity without attaching;
conflicting identity or configuration must be reported without replacing its processes.
Bare native startup, `ace new`, and one-shot execution retain their current
behavior until their documented managed/connected activation is implemented.

Inspection must work during startup and teardown, exposing the instance and project,
backend instance, component roles and process/pane identities, server endpoint, startup
phase, fixed primary ID once known, and any failure. A starting instance can be inspected
without being an available message recipient. No connect recipient is published in this
runtime slice; the later connect decorator consumes the ready endpoint/primary value.

Reuse the configured `Ace`, `StartMode`, `ResumeMode`, and `Backend` resolution boundary.
Keep preparation and session instructions in `src/ace/start.rs`; preserve command
wrappers, backend-over-global environment merging, model/effort, trust, cwd, and
passthrough ordering.
Use typed native/controlled startup variants so a controlled request always carries its
allocated endpoint. Do not add a second instance-plan abstraction.

### Endpoint, attachment, and ownership

Acquire the persistent instance lock before claiming its socket, keep the lock for the
whole lifetime, and reclaim only a stale socket whose lock has been acquired. Validate
explicit names as path components and keep runtime sockets private to the user. Extend
`src/paths.rs` for the existing session-root convention; allocate before backend process
materialization. Socket-path collisions or excessive path lengths produce an explicit
error rather than a changed storage convention.

The Codex backend owns app-server initialization and native client construction. Fresh
startup uses the approved sequence and the recorded empty dedicated-server baseline:
launch the native client, discover its sole initial loaded thread through public metadata,
and retain that ID. No ID means still starting; multiple candidates are an explicit
discovery failure, never a guess. Later thread creation or terminal selection cannot
replace the retained primary. Model completion is not a readiness condition.

Latest startup preserves the native cwd-scoped resume choice and resolved resume
preference. The native client selects/resumes its conversation; ACE captures the initial
loaded primary through the same dedicated-server boundary. Keep fresh instruction/trust
configuration separate from resumed attachment: recorded remote resume rejects permission
overrides. Map configuration only to sanctioned surfaces that accept it, and report a
mapping failure rather than silently dropping a requested setting. Empty persisted-thread
resume limitations remain backend-owned; do not manufacture a turn or inspect rollout
files to make attachment succeed.

Extend `SessionProcess` for owned components instead of duplicating process construction.
Use regular threads and channels for the actual concurrent workloads. Put Codex protocol
control in a responsibility-named module under the existing backend, separating it from
MCP provisioning before `src/backend/codex.rs` exceeds 1,000 lines. Keep all process and
socket mutations behind their owning operations; `cmd` remains composition.

The runtime owner holds the resolved instance and current component state in memory.
Proposed inspection transport: a private sibling `<slug>.control.sock` under the existing
session runtime root, queried by `list`, `inspect`, `stop`, and component launchers.
This ACE-local control socket is distinct from the Codex socket and the later message
discovery directory. It avoids per-instance TOML records and a new durable database.
The mux executor uses the documented `session component` boundary and obtains the owner's
resolved component command; it does not re-read changing config independently in each
pane. Capture tmux IDs from creation results and target them explicitly for every
operation.

Extend `src/platform.rs` for owned-process signalling and integrate cancellation with the
existing handler in `src/ace/io.rs`. Reconcile owner-classified cascades before deciding
normal exit versus failure. Preserve the first classified failure through cleanup,
stop every remaining owned component, share the specified five-second grace period,
reap children, withdraw runtime availability, and release the lock last. Delay process
exit propagation until cleanup finishes; no automatic restart or unrelated tmux targeting.

### Implementation and validation sequence

First resolve the concrete dependency gate below. Then implement the runtime boundary as
one coherent slice: identity/endpoint ownership, controlled Codex startup and fixed
primary, cohort shutdown, and the single-instance tmux command/read surfaces. Update
CLI help and
route new leaf errors through `src/cmd/error.rs` and its exit-code classifiers.

Use meaningful assertion failures before implementation for ACE-owned behavior. Reuse
`tests/exec_test.rs`, `TestEnv`, and Flaude for dispatch and resolved configuration;
extend that fixture for managed intent instead of impersonating a real backend binary.
Use isolated runtime tests for locks, stale sockets, premature publication, ambiguous
primary discovery, partial startup failure, reordered exit observations, cleanup failure,
and the shared grace deadline. Pure Codex construction/parsing tests belong with the
backend; test protocol decisions with recording in-process fixtures. Exercise tmux command
composition without touching the user's windows; any live attachment check requires its
own explicitly owned target and appropriate execution authority.

Run focused tests, the full suite, `cargo fmt --check`, all-target/all-feature Clippy,
documentation links, and whitespace checks after code approval. Retain existing deadlines;
record compilation time and any unavailable platform check. Audit every changed file
against the request, architecture, specs, and skills before a coherent local commit.
No new live model turns are prerequisites: the Codex spec already preserves the
integration evidence, and backend approvals, sender completion lifetime, and
terminal-selection tracking are outside these implementation tests.

### Dependency gate and following slice

The current manifest has no WebSocket client. The verified Unix app-server transport needs
WebSocket handshake/framing, so select a blocking client with Unix-stream support and
present its exact version, features, and transitive changes for separate approval.
Likewise check safe Unix signalling and advisory locking against the pinned standard
library and existing dependencies before proposing any additional crate or feature.
Do not add an async runtime, hand-roll WebSocket transport, use private transitive APIs,
or change manifests/lockfiles before approval. Resource-intensive checks need their own
execution approval under the repository rules.

After the runtime slice is accepted and implemented, **connect-core** and
**connect-codex** add configuration/decorating, recipient publication, discovery/status,
and the common
`ace connect send` translation to the fixed primary. Sender-side translation remains the
only backend message translation boundary. Workspace composition, other backend adapters,
durable runtime history, and automatic restart remain outside the first runtime slice.

## Next — compose workspaces

- [ ] **workspace-manifest** implement `ace workspace init` and validate `workspace.toml`:
      unique member names, in-root paths, independent child config, and no root config
      overrides.
- [ ] **workspace-expansion** expand enabled members into independently configured `Ace`
      instances and require connect decoration without mutating child config.
- [ ] **workspace-mux** create one tmux session with one window per member, expose
      `workspace {start,list,status,attach,stop}`, and verify relay discovery across
      members.
- [ ] **bare-workspace-entry** make bare `ace` at a manifest root start or attach the
      workspace; bare `ace` inside a member remains single-project startup.

## Later — only after concrete demand

- [ ] **ace-mutation-surface** audit and consolidate the scattered setters, overrides, and
      cache invalidation paths in `src/ace/mod.rs` into one coherent mutation surface with
      explicit invariants.
- [ ] **advanced-session-lifecycle** separately justify and design suspend, wake,
      reconnect, restart policy, or richer status. Workspace mode does not depend on them.
- [ ] **external-launch-hooks** extract the internal expand/decorate/materialize/execute
      phases into a versioned subprocess protocol only after an independently shipped
      extension needs them.

## Regrouped work

* A's **start-mode** belongs here: **start-pipeline** and **native-session-supervision**
  are complete; **runtime-endpoints** owns the future controlled component boundary.
* G's **always-on bridge** is superseded by connected bare startup through
  `[connect] enabled = true`.
* G's `ace remote` and **32** `ace tunnel` are superseded by running the same
  `ace session attach` command over SSH; ACE owns no remote transport.
* G's idle injection, macros, and loop continuation remain separate input-automation
  ideas. They do not define the session primitive.
* G's auto-pause idea is folded into **advanced-session-lifecycle**.
* G's **156** compare runs and H/L's **126** editor side pane remain separate product
  ideas; neither defines mux execution.

## Deferred transport choice

**claude-mcp-transport** · deferred · user:verbatim. Keep the normal Claude session plus
monitor path; an MCP receive adapter is a later option, not a prerequisite for
**connect-claude** or a revival of a broader MCP server product.

> ace connect could be integrated via mcp tool, it might've been sipler that way but we'll
> note that later.

Source: `.ace/save.ledger.md`, recorded before 2026-09-05; the quote is preserved
verbatim.
