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
