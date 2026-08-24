<!-- not spec/decision because: architecture research is complete,
but no product ruling has been made -->

# First-class agent communication and workspace mode

Research snapshot for the ACE maintainer deciding how ACE should launch, supervise, and
connect coding agents across repositories.

## Executive conclusion

ACE should become a local **workspace supervisor** with three explicit boundaries:

1. **Session control** starts and observes one backend agent in one repository.
2. **Agent messaging** routes typed tasks and messages between those sessions.
3. **Presentation** renders all session events and lets the user change focus.

Codex app-server and OpenCode serve already provide sanctioned session-control surfaces.
Claude still needs its native receive surface. ACE should hide those differences behind
one backend session adapter and own the lifecycle above them.

A2A 1.0 is mature enough to adopt as the semantic model for agent-to-agent tasks,
messages, artifacts, status, and capability discovery. It should not replace the local
transport in the first release: use A2A-shaped domain types over an ACE-owned local IPC
transport, then add the standard HTTP binding when remote interoperability becomes real.

ACP 1 is mature enough to inform—and potentially become—the session-control boundary
between ACE's UI and coding agents. It is a substantially closer fit than inventing a
generic ACE session protocol: sessions, prompts, streamed updates, permissions, plans,
terminal operations, modes, cancellation, and Rust libraries already exist.

AG-UI fits the presentation boundary, not agent-to-agent communication. Its event model
is useful if ACE grows a graphical or remote UI; adopting it inside the initial terminal
implementation would add a second abstraction before a second UI exists.

MCP remains the tool and data boundary. Its 2026 direction explicitly moves away from
hidden transport sessions, roots, sampling, and logging. It must not be stretched into
ACE's session supervisor or agent bus.

## Evidence recovered from this repository

### `ace-connect` is complete prior art, not a product boundary

The entire skill, its four references, and all seven scripts were reviewed. The current
system proves these facts:

- A deterministic endpoint identifies one backend session in one workdir.
- Local discovery, send, and receive work across Claude, Codex, and OpenCode.
- Codex can be launched as app-server + bridge + attached TUI from one command.
- OpenCode can be launched as server + owned session + bridge + attached TUI from one
  command.
- The receiving model, not the transport, applies authority and repository policy.
- Plain interactive Codex cannot be retrofitted with an external receive endpoint; the
  attachable server must be selected at launch.

The shell bridge also exposes the limits ACE must remove:

- Unix-socket delivery is fire-and-forget and capped by a one-line dialect.
- There is no message identity, task identity, correlation, acknowledgement, retry,
  persistence, multi-message thread, or structured artifact.
- The listener rebinds after every message, creating a deliberate delivery gap.
- Session identity is encoded in a predictable slug rather than stored as typed state.
- Backend lifecycle and bus lifecycle are fused inside shell wrappers.
- The Codex bridge chooses the first loaded thread and drives only `turn/start`.
- The user's view and the transport log are reconstructed by model instructions rather
  than emitted as first-class events.

These are acceptable properties for a skill prototype and poor foundations for a
workspace product.

### Existing ACE design already selected the ownership boundary

[`docs/spec/connect.md`](../spec/connect.md) says the bridge belongs in the `ace` binary
because ACE already owns backend binding, launch, project identity, and capability
dispatch. The accepted historical decision says the same thing and records a former
`ace-rs/connect` sibling checkout as its source.

The historical sibling-repository pointer is non-authoritative and contributes nothing
to this analysis. The current Rust path supports the ownership decision:

- `cmd::main` prepares one project and builds one `SessionRequest`.
- `Backend::exec_session` maps that request to a backend and replaces ACE with the
  backend process.
- `Kind` owns backend capabilities, while custom backend instances inherit their kind.
- `Ace` owns one `project_dir` and lazily resolves one config tree and backend binding.

Workspace mode therefore cannot be another flag threaded through `exec_session`. It
changes ACE from an exec-style launcher into a process supervisor that owns several
project-scoped session runtimes.

### Workspace patch recovery

No workspace-mode patch was found inside this repository. The search covered:

- tracked and hidden files;
- all local and remote refs;
- branch and stash inventories;
- reflogs;
- unreachable commits and blobs;
- filenames and content matching workspace, thread, connect, and start-mode terms.

The only stash is an unrelated gitlink/scrub attempt. The workspace concept below is
therefore a reconstruction from the user's description and current code, not a recovered
patch. The former sibling `ace-rs/connect` repository is explicitly excluded as a source;
only the loaded `ace-connect` skill, references, and scripts describe the prototype.

## Protocol assessment

- **A2A 1.0:** independent agent-to-agent work; stable 1.0, governed project,
  official Rust SDK. Adopt its semantic model and defer its HTTP binding.
- **ACP 1:** coding agent-to-client sessions; stable v1, Rust runtime and schema,
  broad editor and agent support. Prefer it for the session-control seam.
- **AG-UI:** agent-to-user application events; working ecosystem and broad event
  surface. Reserve it for a presentation adapter.
- **MCP:** agent tools, resources, and external APIs; mature, but becoming more
  stateless. Keep it in its existing role.

### A2A: the right agent-task vocabulary

A2A 1.0 models opaque independent agents. Its core concepts—Agent Card, Message, Task,
Artifact, context identifier, task lifecycle, cancellation, polling, streaming, and
push delivery—cover almost every concept the socket dialect currently leaves implicit.
It has a stable 1.0 specification and official SDKs including Rust.

The fit is not exact. A2A explicitly does not define an agent's internal subagent
protocol, and its primary deployment shape is independently addressable agent services.
ACE workspaces begin as several local child processes under one supervisor. Implementing
the entire HTTP server, webhooks, authentication, and discovery surface on day one would
solve a deployment problem ACE does not yet have.

Adopt the stable semantics first:

- `AgentId` and capability description;
- `TaskId` and `ContextId`;
- typed message parts and artifacts;
- task states including input-required and terminal outcomes;
- ordered task events;
- cancellation and explicit delivery results.

Keep the first transport local and private. When ACE needs cross-host or third-party
interoperability, expose the same domain through A2A's HTTP+JSON binding and Agent Card.

Sources: [A2A 1.0 specification], [A2A overview], and [agent discovery].

### ACP: the right coding-session seam

ACP standardizes a client driving a coding agent. Stable v1 includes initialization,
session creation/load/list/delete, prompt turns, streamed updates, permissions, file and
terminal requests, plans, session modes, configuration options, cancellation, and
extensibility. It ships official Rust schema and runtime crates.

That is nearly the exact boundary ACE needs between a workspace UI and each backend
runtime. A first-class ACP adapter would let ACE consume any conforming coding agent
without teaching its supervisor a new event dialect for each one.

The caveat is direction: Codex app-server and OpenCode expose their own control APIs;
Claude's supported integration may not speak ACP directly. ACE still needs backend
adapters, but an adapter should translate a backend's sanctioned API into ACP-shaped
session events rather than into a private grab bag.

ACP remote support remains work in progress, so it does not eliminate the later need for
A2A or a remote transport. ACP owns client ↔ coding session; A2A owns agent ↔ independent
agent. These are adjacent layers, not competing standards.

Sources: [ACP introduction], [ACP v1 overview], and [ACP repository].

### AG-UI: useful above the supervisor

AG-UI standardizes a bidirectional event stream between an agent runtime and a
user-facing application. Its standardized message, tool-call, state, activity, run,
interrupt, and steering events could support a future browser or native ACE workspace
UI.

It is not an agent bus. Using it for peer work would erase task ownership and capability
discovery behind UI events. The terminal UI should initially consume ACE's typed session
events directly; introduce an AG-UI presentation adapter only when a second UI or remote
frontend creates the second concrete use.

Sources: [AG-UI overview] and [AG-UI architecture].

### MCP: deliberately not the answer

MCP is an agent-to-tool and data protocol. Its latest 2026 release removes
protocol-level sessions, moves tasks into an extension, replaces server-initiated
round-trips, and deprecates roots, sampling, and logging. That evolution reinforces the
boundary: explicit application handles should own session state.

An ACE MCP server could expose tools such as `agents.list` or `messages.send`, but that
would make agents discover the supervisor as a tool; it would not provide wake-idle,
session attachment, UI event streaming, or lifecycle ownership. MCP may be an optional
agent-facing façade later, never the internal architecture.

Sources: [MCP 2026-07-28 release] and [MCP authorization].

## Proposed architecture

```text
                              ace workspace UI
                         focus / status / approvals
                                      │
                           typed session event stream
                                      │
                         ┌────────────┴────────────┐
                         │   WorkspaceSupervisor   │
                         │ registry + lifecycle +  │
                         │ routing + durable log   │
                         └──────┬──────────┬───────┘
                                │          │
                     SessionControl     AgentRouter
                       (ACP-shaped)     (A2A-shaped)
                    ┌───────┼──────┐          │
                 Codex   Claude  OpenCode   local IPC
               app-server native   serve      first
```

### Domain objects

The design should make these states explicit:

```text
Workspace
  id, root, display_name, config_path, session

Session
  id, workspace_id, backend, native_session_id, state, capabilities

Agent
  id, session_id, role, capabilities, endpoint

Task
  id, context_id, sender, recipient, state, messages, artifacts

Event
  sequence, time, workspace_id, session_id, task_id?, payload
```

`Workspace`, `Session`, `Agent`, and `Task` are different identities. A repository may
restart a session; a session may host several agent threads; a task may outlive the
connection that submitted it. The socket slug prototype collapses all four and cannot
support reliable UI focus or reconnection.

### Supervisor ownership

`WorkspaceSupervisor` owns:

- discovery and validation of workspace declarations;
- one child runtime per enabled workspace;
- restart and shutdown policy;
- the in-memory registry of workspace, session, agent, and task identities;
- append-only event persistence;
- message routing and delivery outcomes;
- the stream consumed by the terminal UI;
- backend-independent focus, status, cancel, and send operations.

It does not own:

- repository policy or authority decisions;
- backend protocol details;
- model prompts or agent reasoning;
- MCP tool registration;
- cross-repository filesystem access.

Each child is launched with its subrepository as cwd and receives only that repository's
config, instructions, and permissions. Communication crosses repositories as messages or
artifacts, never by widening filesystem roots.

### Session adapter contract

The backend abstraction needs a second responsibility beside argument translation. It
should expose session control as a capability-driven contract:

```text
start(request) -> session
resume(native_id, request) -> session
send_input(session, input) -> turn
steer(session, turn, input) -> result
interrupt(session, turn) -> result
subscribe(session) -> ordered events
close(session) -> result
```

Do not add backend-name branches in the supervisor. Each adapter declares capabilities
such as persistent session, wake-idle, steer-active, stream-events, list-threads, and
multiple threads.

Codex uses app-server's stable thread/turn APIs. Current upstream documentation supports
Unix-socket app-server transport, thread start/resume/fork/list/read, loaded-thread
inventory, ordered notifications, status changes, and cancellation. ACE should prefer
the documented Unix-socket endpoint over the prototype's experimental WebSocket and
external `websocat` dependency.

Source: [Codex app-server README].

OpenCode uses its server/session API. Claude uses the strongest sanctioned interactive
surface it exposes; if that surface cannot provide structured subscription or wake-idle,
the adapter advertises the missing capability instead of emulating it through terminal
keystrokes.

### Agent router contract

The router should use typed envelopes rather than interpreting a body prefix:

```text
SendTask { from, to, context_id?, parts, requested_capability }
SendMessage { from, to, task_id, parts }
CancelTask { requester, task_id }
PublishArtifact { sender, task_id, artifact }
```

Delivery returns an admission result, not mere socket success:

```text
Accepted { task_id }
Rejected { reason }
Unavailable { recipient }
Unsupported { capability }
```

Authority remains evaluated by the receiving agent and user-facing client. The protocol
must preserve sender identity and cited provenance, but it must never turn peer identity
into permission. This keeps the strongest rule from `ace-connect` while removing its
model-parsed wire format.

### Persistence

Persist an append-only event log under ACE's user data directory, not inside each
repository. Repository `.ace/connect.log` is useful human provenance but cannot be the
supervisor's source of truth: workspaces need one ordered view across repositories, and
repos should not acquire runtime bookkeeping.

The minimal durable facts are workspace declarations, stable workspace IDs, native
session IDs needed for resume, task/message/artifact events, and terminal task states.
Current status is derived from the log. Volatile process IDs and socket paths remain
runtime-only.

The exact storage format remains a design decision. A single append-only file is enough
for the first use; a normalized embedded database becomes justified only when indexed
history and concurrent readers are concrete requirements.

## Product surface

### Single-project native connect

Bare `ace` should remain the one command. When the selected backend supports supervised
sessions, it prepares the project, starts the backend control server, starts the session
adapter and router endpoint, then attaches the terminal UI. There should be no separate
`ace connect codex` startup path.

Operational commands may still exist for inspection and explicit messaging:

```text
ace agents
ace send <agent> <message>
ace tasks
```

Those commands consume the running supervisor; they do not create a second bridge.

### Workspace mode

Workspace mode should be explicit configuration at the monorepo root. Recursive
auto-detection of every `ace.toml` is attractive but ambiguous: fixtures, examples,
vendored repos, nested schools, and inactive packages can all contain configuration.

Proposed root configuration:

```toml
[[workspaces]]
path = "services/api"

[[workspaces]]
path = "apps/web"

[[workspaces]]
path = "infra"
enabled = false
```

Each child path must resolve inside the root and contain its own `ace.toml`. The child
file remains authoritative for school, backend, trust, environment, and session prompt.
The root owns only membership and presentation metadata. Facts are not duplicated across
root and child configs.

Bare `ace` at that root detects the declared workspace set, prepares each enabled child,
starts one supervised session per child, connects every session to the common router,
and opens the workspace UI. Bare `ace` inside a child keeps today's single-project
behavior.

### Terminal UI

The first UI needs only four concepts:

- workspace list with backend and session state;
- focused session transcript;
- task/message activity badges;
- focus cycling and explicit task/message composition.

The UI must not pretend several sessions are one conversation. Switching focus changes
which session receives human input; agent-to-agent tasks remain separately identified
and visible in both sender and recipient activity.

## Threads support

ACE needs thread support only at the session-adapter boundary and registry, not a generic
promise that every backend exposes Codex-style subagents.

For Codex, distinguish:

- the primary workspace thread attached to the UI;
- child agent threads created by Codex itself;
- peer tasks injected into the primary thread;
- independent ACE workspace sessions.

The app-server already exposes parent thread identity, loaded-thread inventory, status,
forking, resume, and filtering. ACE can represent these without owning Codex's internal
subagent orchestration. A workspace is not a Codex subagent; it is an independently
configured ACE session that may itself contain backend-native subagents.

Initial scope should track and display backend-native threads but route cross-workspace
tasks to the workspace's primary agent. Addressing a particular child thread is a later
capability, added only when two backends can support a coherent meaning.

## Delivery sequence

### Phase 1 — supervised single project

- Replace the exec-only launch boundary with a session supervisor.
- Implement the backend-neutral session event model.
- Move Codex startup to app-server over its documented Unix-socket transport.
- Port local discovery/routing from shell to Rust with typed identities and delivery
  results.
- Preserve the current model-side authority contract.
- Keep the existing terminal experience focused on one session.

This phase retires the three-process shell wrapper without requiring workspace UI or
standard HTTP protocols.

### Phase 2 — workspace registry and UI

- Add explicit root workspace membership.
- Start one isolated session per child repository.
- Add focus cycling, status, task activity, and shutdown handling.
- Persist session/task events and resume identifiers.
- Route typed peer tasks between workspace agents.

### Phase 3 — ACP compatibility

- Make the internal session event contract ACP-compatible where semantics match.
- Add an ACP agent adapter for conforming coding agents.
- Add an ACP client surface only if third-party clients need to drive ACE sessions.

ACP compatibility may move into Phase 1 if using its Rust runtime materially reduces
the amount of session protocol ACE must own without forcing backend workarounds.

### Phase 4 — A2A interoperability

- Publish Agent Cards for explicitly exported ACE agents.
- Expose A2A HTTP+JSON task operations.
- Add authentication and explicit export policy.
- Map local tasks and artifacts losslessly to standard A2A objects.

Remote transport, cross-machine discovery, and external agents belong here—not in the
local workspace MVP.

### Phase 5 — presentation adapters

- Add AG-UI only when a browser, native, or remote frontend is a real second consumer.
- Keep the terminal UI on the same typed event source.

## Decisions required before implementation

1. Whether ACP becomes the internal session contract in Phase 1 or remains an adapter
   added after the native supervisor works.
2. Whether workspace membership lives in root `ace.toml` under `[[workspaces]]` or in a
   dedicated root manifest; normalized ownership favors `ace.toml` unless root and child
   project config need fundamentally different lifecycles.
3. Whether the first durable event store is an append-only file or a database.
4. Which Claude sanctioned surface can provide wake-idle and structured events today;
   capability absence must remain explicit if none can.
5. Whether first-class communication ships for one supervised project before workspace
   mode, or both land as one release boundary.

## Recommended ruling

Adopt this layered direction:

- **ACP-shaped session control** for ACE UI ↔ coding session.
- **A2A-shaped task communication** for independent ACE agents.
- **ACE-owned local IPC and supervision** for the first implementation.
- **AG-UI presentation adapter** only with a second UI.
- **MCP unchanged** as the tool and data boundary.

Build supervised single-project Codex first. It exercises lifecycle, event streaming,
typed routing, and the user-visible session boundary using a sanctioned upstream API.
Then add explicit workspace membership and multiple isolated sessions. This sequence
tests the hard abstraction before multiplying it, while every Phase 1 type remains the
same type workspace mode needs.

[A2A 1.0 specification]: https://a2a-protocol.org/latest/specification/
[A2A overview]: https://a2a-protocol.org/latest/
[agent discovery]: https://a2a-protocol.org/latest/topics/agent-discovery/
[ACP introduction]: https://agentclientprotocol.com/get-started/introduction
[ACP v1 overview]: https://agentclientprotocol.com/protocol/v1/overview
[ACP repository]: https://github.com/agentclientprotocol/agent-client-protocol
[AG-UI overview]: https://docs.ag-ui.com/introduction
[AG-UI architecture]: https://docs.ag-ui.com/concepts/architecture
[MCP 2026-07-28 release]: https://blog.modelcontextprotocol.io/posts/2026-07-28/
[MCP authorization]: https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization
[Codex app-server README]: https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md
