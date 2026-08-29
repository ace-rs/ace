# Workspaces

**Designed, not yet implemented.** Workspace mode composes managed ACE sessions and the
local relay; it does not introduce another agent runtime.

## Purpose

A workspace is a declared set of repository-scoped ACE instances that start, stop, and
remain inspectable as one development environment. Each member keeps its own working
directory, `ace.toml`, school, backend, prompt, environment, permissions, native session,
and ACE-connect identity.

Workspace mode does not merge repository permissions, filesystems, transcripts, or
backend-native threads. Communication crosses member boundaries only through
[ACE-connect](connect.md).

## Configuration

The root uses a dedicated `workspace.toml` because it describes collection membership,
not one project's ACE configuration:

```toml
name = "prodigy9"

[[members]]
name = "api"
path = "services/api"

[[members]]
name = "web"
path = "apps/web"

[[members]]
name = "docs"
path = "docs"
enabled = false
```

The workspace `name` is optional and defaults to the root directory name. Member names
are required and unique relay and tmux-window identities. Paths are relative to the
manifest, must resolve inside the workspace root, and must resolve through the normal ACE
configuration cascade. `enabled` defaults to `true`.

The initial format has no backend, trust, school, environment, or prompt overrides.
Those facts remain normalized in each member's `ace.toml` or `ace.local.toml`. A root
override is added only when a concrete cross-member setting has one authoritative meaning.

## Entry and commands

`ace workspace init` creates the manifest skeleton. Once `workspace.toml` exists, bare
`ace` at that root starts or reattaches the workspace without an extra mode flag.

```text
ace workspace init
ace workspace start [--detach]
ace workspace list
ace workspace status [name]
ace workspace attach [name] [member]
ace workspace stop [name]
```

Bare `ace` inside a member remains the normal single-project path. It does not walk upward
and silently start the parent workspace.

## Expansion

The built-in workspace stage constructs one `Ace` per enabled member. Each instance is
resolved independently from its own working directory and config cascade. Workspace
validation completes for every member before any process starts.

The workspace stage supplies only:

- workspace identity;
- member identity and path;
- enabled membership;
- presentation ordering;
- the requirement that members share one local relay group.

It does not copy resolved child configuration into the root or inspect another member's
repository contents beyond validating its declared path and ACE configuration.

## Mux realization

The mux executor creates one named tmux session for the workspace and one window per
member. Each window runs that member's ordered component list in panes: backend control
server, relay where required, and the terminal primary session or UI.

tmux is the viewing surface. Users inspect or switch agents with normal tmux window and
pane commands; ACE does not implement a screen switcher, transcript renderer, or agent
dashboard.

ACE retains a read-only mapping from workspace and member names to tmux socket, session,
window, panes, component roles, backend handles, and relay identities. `workspace status`
and `session inspect` expose the same underlying runtime facts.

Local and SSH attachment use the same command:

```console
ace workspace attach prodigy9 api
ssh -t gz44 ace workspace attach prodigy9 api
```

ACE does not create SSH tunnels or manage remote hosts.

## Lifecycle

Starting a workspace prepares every enabled project, configures every `Ace`, validates
each member's component list, then starts it through mux. Each member starts in list order
and gates the next component on readiness. A partial startup reports exactly which
components are live and which failed; it never labels the whole workspace healthy.

Stopping coordinates the owned member processes and tmux session. Detaching leaves every
process running. Cleanup is idempotent and tolerates already-dead processes. The first
release does not include suspend, automatic restart, durable event history, wake-idle
policy, or reconnection beyond attaching to the live tmux session.

Backend-native resume remains a member-session behavior. Later suspend or wake support
must enter as explicit backend capabilities and concrete lifecycle operations, not as a
generic promise inferred from workspace mode.

## Relationship to connect

Workspace mode depends on managed sessions, mux execution, and ACE-connect. It enables
connect decoration for each member even when the child project's normal standalone launch
does not enable it; this is a workspace-owned runtime requirement, not a mutation of the
child's `ace.toml`.

The resulting peer set uses member names. Messages target each member's primary backend
thread or session. No task tracking or child-thread routing is introduced.

## Implementation sequence

1. Parse and validate `workspace.toml` into a pure root plan.
2. Resolve each member through the existing project configuration pipeline.
3. Construct and configure one `Ace` per member as specified in [session.md](session.md).
4. Validate names, paths, backend capabilities, and each component list.
5. Execute the component lists through mux and expose list, status, attach, and stop.
6. Enable connect decoration for every member and verify peer discovery across windows.

Workspace implementation starts only after one managed connected session works end to
end; it multiplies the primitive rather than defining it.
