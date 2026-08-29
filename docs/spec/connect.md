# `ace connect` — local agent relay

**Designed, not yet implemented.** The `ace-connect` skill and its shell scripts remain
the working prototype until this contract lands in the binary.

## Purpose

ACE-connect is a local message relay between independently running ACE instances. It
provides peer identity, discovery, send, receive, and backend-specific injection. The
receiving model applies repository policy and authority; transport identity never grants
permission.

ACE-connect is not a task system. It does not own tasks, artifacts, acknowledgements,
retries, transcripts, durable workflow state, process supervision, or workspace
membership.

## Activation

A project opts into connected startup through its existing configuration:

```toml
[connect]
enabled = true
```

Bare `ace` resolves this setting before launch. Connect selects
`BackendMode::WithServer`, the backend materializes its control and terminal-session
components, and the built-in connect decorator inserts its relay immediately before the
terminal session. There is no separate `ace connect start` path.

Connected startup cannot generally be retrofitted onto an arbitrary backend process.
Codex and OpenCode must be born through their server/control surfaces so ACE has the
primary-session handle required for injection. An already-running session is attachable
only when its backend exposes a sanctioned attachment surface.

## Commands

```text
ace connect discover
ace connect send <target> <message>
ace connect monitor
ace connect status
```

`discover` lists live peers. `send` performs one delivery attempt. `monitor` runs the
receive surface used by Claude-style integrations and is also the human debugging view.
`status` explains the current instance's relay identity, endpoint, backend receive mode,
and capability gaps.

## Identity and discovery

One relay identity names one running ACE instance. Its default is derived from the
project directory and resolved backend instance; workspace configuration supplies an
explicit stable member name.

The local runtime directory is:

```text
${XDG_RUNTIME_DIR:-$HOME/.ace/run}/messages/
```

It is mode `0700`. Each live identity publishes a Unix socket and process marker.
Discovery sweeps dead markers before returning peers. Runtime paths and process IDs are
ephemeral and never committed to a repository.

## Delivery

The first implementation preserves the prototype's fire-and-forget semantics:

- local Unix-domain sockets;
- one message per delivery;
- one delivery attempt;
- a small plain-text envelope carrying sender, recipient, and body;
- explicit socket-write success, unavailable-recipient, or local transport failure at
  the CLI.

The envelope is transport data, not an agent-task protocol. Message bodies may retain the
prototype's terse conventions, but ACE does not parse verbs such as `ACK`, `DONE`, or
`STUCK` into state transitions.

Cross-machine transport, authentication, encryption, retry, acknowledgement, message
history, and structured artifacts are outside this contract.

## Backend receive adapters

### Codex

Connected Codex uses its documented app-server surface. The configured `Ace` starts the
server, establishes the primary thread, starts the relay adapter, and finally attaches
the native client UI. Incoming messages target the primary thread through the sanctioned
thread/turn API.

ACE may list backend-native child threads for inspection, but the relay does not address
them. Plain interactive Codex has no external receive endpoint and is therefore not a
connected session.

### OpenCode

Connected OpenCode uses `opencode serve` and its documented session API. The instance
component list starts the server first. Its backend controller waits for readiness and
establishes the primary session before starting the relay adapter and, finally, the
client. Incoming messages target that primary session.

### Claude

Claude uses the strongest sanctioned receive surface available to the installed client.
The current prototype uses a monitor process. `ace connect monitor` preserves the visible
control/autonomous behavior and debugging log without pretending Claude exposes Codex-
style thread control.

If Claude cannot inject into an idle session through a sanctioned surface, `status`
reports that capability gap. ACE does not emulate a control API with terminal keystrokes.

## Process relationship

Connect decorates a session plan; it does not execute the plan. The local or mux executor
places the backend and relay components. Their lifecycles are coordinated because they
belong to one ACE instance, not because the relay became a supervisor.

Every decorated component is essential. Connect owns relay readiness and exit semantics;
the backend owns its native cascade classification. The cohort is reconciled before its
outcome is classified, so a successful user exit remains normal even when ACE observes a
cascaded backend or relay exit first. Connect classifies whether a relay exit belongs to
that normal cascade or represents independent component failure.

Workspace mode enables the same decorator for every member and supplies their peer names.
The transport itself remains unaware of workspace configuration.

## Prototype migration

The Rust implementation ports the proven shell behavior in narrow slices:

1. local identity, discovery, send, and monitor;
2. connect configuration and instance-plan decoration;
3. Codex primary-thread injection;
4. OpenCode primary-session injection;
5. Claude monitor integration.

The skill collapses to usage guidance only after the binary implements each documented
backend adapter and reports capability gaps honestly.
