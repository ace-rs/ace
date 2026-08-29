# Managed sessions

**Partially implemented.** ACE prepares one project and materializes its backend launch
as a validated ordered `session::Components` startup list. Normal sessions execute their
single component; server-backed lists await endpoint allocation, protocol control, and
multi-component execution.

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
  -> feature requirement resolution
  -> backend component materialization
  -> feature component decoration
  -> local or mux execution
```

`Ace` is the instance and `StartMode` makes launch intent structurally valid:

```rust
enum StartMode {
    OneShot { prompt: String },
    Session { resume: ResumeMode, backend: BackendMode },
}

enum ResumeMode {
    Fresh,
    Latest,
}

enum BackendMode {
    Normal,
    WithServer,
}
```

`Fresh` starts a new native session. `Latest` asks the backend to resume its most recent
session for the project, subject to the resolved personal resume preference.

Callers construct and configure `Ace`, then call `ace.start(mode)`. `Normal` requests the
backend's standard native chat harness. `WithServer` requires a server-capable launch;
the controlled component path lands in the next implementation phase.

Only one workspace expander and one executor may be active. Decorators compose in a
declared order and add typed requirements. Workspace composition validates duplicate
instance names, incompatible requirements, and missing backend capabilities before any
process starts.

The initial implementation is built-in and single-binary. The stage boundaries are
internal Rust interfaces, not a public plugin ABI. An installable subprocess protocol is
justified only when an independently shipped extension needs the same boundary.

## Components

A `Component` is the executable process boundary. It owns a typed role, program and
arguments, environment, and working directory. Only its execution edge converts that
data into `std::process::Command`; component construction can compose process data before
execution.

Normal Claude, Codex, and OpenCode sessions each construct one `session` component and
exec-replace ACE with it. This preserves the existing foreground behavior while giving
multi-process planning a real component type to compose.

`session::Components` is a non-empty ordered list with exactly one terminal `session`
component. Every included component is essential by construction: ACE never starts an
inessential process, and the session is useful only while every listed component is
healthy. A backend constructs its portion because only the backend knows its process
topology. Feature decorators then insert their own components before the terminal session;
`session::Components` owns validation of the final neutral list. Roles describe purpose
rather than backend-specific executable names:

- `server` — backend control server, included only when another component requires it;
- `relay` — connected-session message adapter, included only when connect is enabled;
- `session` — the terminal primary backend session or native client.

List order governs startup only. The executor starts each component after the preceding
component's owner reports it ready. A backend controller owns protocol readiness and
establishes the primary backend-session handle before any consumer of that handle starts;
connect owns relay readiness. The local execution edge exec-replaces ACE only when the
list contains one foreground component. A multi-component list returns an explicit
unsupported error until the component executor lands. The mux executor keeps each
component's stdout and stderr directly inspectable.

Codex materializes `server -> session` with a concrete Unix-socket endpoint:
`codex app-server --listen unix://...` followed by `codex --remote unix://...`.
OpenCode materializes the same roles with a concrete loopback HTTP endpoint:
`opencode serve --hostname 127.0.0.1 --port ...` followed by `opencode attach ...`.
Endpoint allocation is a runtime responsibility and is not performed during component
construction.

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

## Lifecycle

The terminal `session` component is the main user-facing process. Exit observation order
does not decide the outcome. The runtime reconciles the cohort with every component owner
before classifying it because a user exit may cascade through backend and relay processes
before ACE observes the successful terminal-session exit.

A successful terminal-session exit and every owner-classified cascade complete the
managed session normally. An abnormal terminal-session exit or component loss outside
such a cascade fails it because every component is essential. Once failure is recorded,
exits caused by ACE cleanup cannot replace that outcome with normal completion.

After completion or failure, ACE stops whatever owned components remain. Cleanup may
traverse reverse startup order for deterministic behavior, but that order is not a
correctness requirement: a backend may cascade one process exit into others. Every stop
operation is idempotent and treats an already-dead process as cleaned up.

The managed-session contract has no automatic restart. A later start may use the
backend's native resume behavior, but it creates a new managed session.

## Mux execution

The mux executor uses tmux as a sanctioned process and terminal host. A standalone
instance occupies a named tmux session; a workspace occupies one tmux session with one
window per member and as many panes as that member's component list requires.

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

Automatic restart, durable event history, generic wake-idle behavior, transcript storage,
and cross-host process management are outside the contract. Backend-native resume may
still be used when the user starts a session, as specified in
[backend.md](backend.md).

## Implementation sequence

1. Route preparation and launch through `Ace::start(StartMode)`.
2. Represent each normal backend launch as one typed process component while preserving
   exec-replace behavior.
3. Materialize controlled component lists for Codex app-server and OpenCode serve,
   using only their sanctioned control surfaces.
4. Allocate runtime endpoints, establish primary backend handles, and add the component
   executor plus `ace session` inspection, attachment, and lifecycle commands.
5. Add the connect decorator and backend receive adapters described in
   [connect.md](connect.md).
6. Add workspace expansion and group lifecycle from [workspace.md](workspace.md).
7. Add suspend, wake, reconnect, or richer status only when a concrete workflow requires
   each capability.

Every phase preserves bare `ace` as the common entry point and ships with a corresponding
read surface before another layer depends on it.
