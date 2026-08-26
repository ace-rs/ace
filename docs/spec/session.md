# Managed sessions

**Designed, not yet implemented.** ACE currently prepares one project and exec-replaces
itself with one backend. This specification defines the compositional session boundary
that replaces that launch path.

## Primitive

The lowest-level ACE primitive is a named **ACE instance**:

```text
AceInstance
  name
  project_dir
  resolved_config
  backend
  native_session
  primary_thread
  relay_identity?
  runtime
```

One instance represents one repository, one resolved `ace.toml`, one backend session,
one terminal runtime, and optionally one ACE-connect identity. It does not represent a
workspace, a backend-native subagent, or an agent-to-agent task.

The user-facing noun remains `session`, matching the wrapped backends:

```text
ace session start [path] [--name <name>] [--detach]
ace session list
ace session inspect <name>
ace session attach <name>
ace session stop <name>
```

Bare `ace` remains the normal entry point. In a project it prepares the project, starts
or resumes its default session, and attaches the terminal. The explicit commands expose
the same primitive for composition and inspection.

## Ownership

ACE owns:

- project preparation and resolved configuration;
- stable ACE-instance identity;
- backend component planning;
- coordinated startup and shutdown;
- runtime health and component identity;
- the primary backend-session and thread handles needed by enabled features.

tmux owns:

- process and terminal persistence;
- detach and reattach;
- panes, windows, and interactive switching;
- terminal viewing locally or through SSH.

The backend owns its protocol, thread semantics, transcript, permissions, and internal
subagent orchestration. ACE does not render a replacement terminal UI or reconstruct a
transcript from pane output.

## Launch pipeline

Startup is one typed pipeline inside the `ace` binary:

```text
project discovery
  -> workspace expansion
  -> instance decoration
  -> backend component materialization
  -> local or mux execution
```

`Ace` is the instance and `StartMode` makes launch intent structurally valid:

```rust
enum StartMode {
    OneShot { prompt: String },
    Session { resume: bool, backend: BackendMode },
}

enum BackendMode {
    Normal,
    WithServer,
}
```

Callers construct and configure `Ace`, then call `ace.start(mode)`. `Normal` requests the
backend's standard native chat harness. `WithServer` requires a server-capable launch;
the controlled component path lands in the next implementation phase.

Only one workspace expander and one executor may be active. Decorators compose in a
declared order and add typed requirements. Workspace composition validates duplicate
instance names, incompatible requirements, missing backend capabilities, and component
cycles before any process starts.

The initial implementation is built-in and single-binary. The stage boundaries are
internal Rust interfaces, not a public plugin ABI. An installable subprocess protocol is
justified only when an independently shipped extension needs the same boundary.

## Components

A backend materializes an instance into a graph of named process roles. Roles describe
purpose rather than backend-specific executable names:

- `server` — optional backend control server;
- `session` — the primary backend session or client;
- `terminal` — optional attached backend UI;
- feature components added by decorators, such as `relay`.

The default executor may exec-replace ACE when the graph contains one foreground
component. The mux executor starts graph nodes in tmux panes in dependency order and
keeps their stdout and stderr directly inspectable.

The diagnostic component surface is:

```text
ace session component <role> --instance <name>
```

It exists so the mux executor and maintainers can run one planned component in one pane.
It is not a second configuration language: component argv and environment always come
from the configured `Ace` instance.

## Thread model

ACE models only the thread identities required to drive a backend correctly:

- the primary thread attached to the user-facing session;
- backend-native child threads reported by the backend, when available.

Connect delivery targets the primary thread. Native child threads may be listed for
inspection, but ACE does not address them, route peer messages to them, or promise a
cross-backend subagent model. A workspace member is an independent ACE instance, not a
child thread.

Codex and OpenCode require controlled startup to obtain the server and primary-session
handles. Claude may expose less structured identity; the backend advertises what it can
provide instead of ACE manufacturing a false common model.

## Mux execution

The mux executor uses tmux as a sanctioned process and terminal host. A standalone
instance occupies a named tmux session; a workspace occupies one tmux session with one
window per member and as many panes as that member's component graph requires.

ACE records enough runtime metadata to map an instance name to its tmux socket, session,
window, panes, process roles, backend session, and relay identity. `ace session inspect`
prints that mapping. Runtime metadata is operational identity, not a task store or
transcript.

Remote use requires no ACE network protocol:

```console
ssh -t gz44 ace session attach <name>
```

SSH transports the terminal; tmux performs attachment on the host where the processes
run.

## Lifecycle boundary

The first implementation supports start, inspect, attach, detach through tmux, and
coordinated stop. State exists only for the lifetime of the managed processes and the
small runtime record needed to find them.

Restart policy, durable event history, generic wake-idle behavior, transcript storage,
and cross-host process management are not prerequisites. Backend-native resume may still
be used when the user starts a session, as specified in [backend.md](backend.md).

## Implementation sequence

1. Move the existing preparation and launch sequence behind `Ace::start(StartMode)` while
   preserving the current single-process behavior.
2. Add controlled component graphs for Codex app-server and OpenCode serve, using only
   their sanctioned control surfaces.
3. Add the tmux executor and `ace session` inspection, attachment, and lifecycle commands.
4. Add the connect decorator and backend receive adapters described in
   [connect.md](connect.md).
5. Add workspace expansion and group lifecycle from [workspace.md](workspace.md).
6. Add suspend, wake, reconnect, or richer status only when a concrete workflow requires
   each capability.

Every phase preserves bare `ace` as the common entry point and ships with a corresponding
read surface before another layer depends on it.
