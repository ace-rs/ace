# `ace connect` — local agent relay

ACE-connect requires Unix-domain sockets and is unavailable on the limited Windows GNU
target. See [platform support](platforms.md).

**Designed, not yet implemented.** The `ace-connect` skill and its shell scripts remain
the working prototype until this contract lands in the binary.

## Purpose

ACE-connect lets an agent send a message to another running ACE instance through one
ACE CLI command. ACE discovers the recipient through a Unix-based location convention
and translates the send into the recipient backend's sanctioned call. The sender does
not need to know the recipient's backend protocol.

The receiving model applies repository policy and authority; transport identity never
grants permission.

ACE-connect is not a task system. It does not own tasks, artifacts, acknowledgements,
retries, transcripts, durable workflow state, process supervision, or workspace
membership.

## Communication boundary

Instruct sending agents to invoke `ace connect send <target> <message>`. The command
resolves the recipient and uses its supported receive endpoint. The controlled process
split supplies that endpoint; sending owns the backend-specific translation.

The term mesh means the local convention for finding running instances and their
endpoints. It does not introduce a separate mesh protocol or require messages to traverse
both a sender relay and a recipient relay. A reply is another explicit send command.

ACE communicates with backends only through sanctioned APIs and commands. It does not
interfere with backend internals, scrape terminal output, forward backend events as peer
messages, or install separate incoming/outgoing translation machinery. The native
terminal communicates directly with its backend server.

## Activation

A project opts into connected startup through its existing configuration:

```toml
[connect]
enabled = true
```

Bare `ace` resolves this setting before startup. A connected-session request carries its
control requirement and concrete endpoint as one structurally valid value. The backend
then constructs its control topology, and the built-in connect decorator adds its relay
according to the backend's startup prerequisites. Connect publishes the recipient only
after its endpoint and fixed primary target are known. There is no separate
`ace connect start` path.

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

`discover` lists live peers. `send` resolves the target and performs one delivery attempt
through the target backend's sanctioned receive surface. `monitor` runs the
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

It is mode `0700`. Each live identity publishes the information needed to find its
sanctioned receive endpoint and primary target, along with a process marker. Discovery
sweeps dead markers before returning peers. Runtime paths and process IDs are ephemeral
and never committed to a repository.

## Delivery

The first implementation preserves the prototype's fire-and-forget semantics:

- a local Unix-based discovery convention for recipient endpoints;
- one message per delivery;
- one delivery attempt;
- a small plain-text envelope carrying sender, recipient, and body;
- explicit send success, unavailable-recipient, or transport/backend failure at the CLI.

The recipient backend determines the supported endpoint protocol; the Unix discovery
convention does not replace it. The send implementation owns that translation. Success
reports the result of the send operation, not an acknowledgement from the receiving model.

The envelope is transport data, not an agent-task protocol. Message bodies may retain the
prototype's terse conventions, but ACE does not parse verbs such as `ACK`, `DONE`, or
`STUCK` into state transitions.

Cross-machine transport, authentication, encryption, retry, acknowledgement, message
history, and structured artifacts are outside this contract.

## Backend send translation

### Codex

Connected Codex uses its documented app-server surface. For fresh startup, the configured
`Ace` waits for server readiness, starts the native client to create the primary, and
retains its initial thread ID before publishing the recipient through connect.
Publication makes the backend endpoint and fixed primary target discoverable to senders;
the instance is not an available recipient before both are established. This requires
neither a setup message nor a model response. `ace connect send` targets that primary
thread through the sanctioned thread/turn API. The process split supplies the receive
capability; it does not require a second ACE translation layer on the receiving side.

ACE may list backend-native child threads for inspection, but the relay does not address
them. Plain interactive Codex has no external receive endpoint and is therefore not a
connected session.

[Codex 0.154.0 integration checks](backends/codex.md#verified-managed-session-integration)
verified first delivery to an empty recipient, native fresh startup, populated-thread
attachment, idle delivery, and busy steering. The verified fresh path lets the native
client create the primary and is the selected fresh-start ordering above.

### OpenCode

Connected OpenCode uses `opencode serve` and its documented session API. The instance
component list starts the server first. Its backend controller waits for readiness and
establishes the primary session before starting the relay adapter and, finally, the
client. `ace connect send` targets that primary session through the supported session API.

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

Component startup follows each backend's readiness prerequisites. Fresh Codex starts the
native terminal before publishing its recipient identity because the terminal creates the
primary. The terminal connects directly to its backend server. The communication command
uses the receive surface made available by controlled startup; the sender agent knows
only the ACE CLI command and recipient identity.

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
