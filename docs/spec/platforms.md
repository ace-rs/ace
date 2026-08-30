# Platform support

ACE publishes binaries for a fixed set of Rust compilation targets. A target is supported
only while the Rust toolchain pinned in `rust-toolchain.toml` can compile ACE and its full
dependency graph for that target.

## Targets

| Target                         | ACE support |
| ------------------------------ | ----------- |
| `aarch64-apple-darwin`         | Full        |
| `x86_64-apple-darwin`          | Full        |
| `aarch64-unknown-linux-gnu`    | Full        |
| `x86_64-unknown-linux-gnu`     | Full        |
| `aarch64-unknown-linux-musl`   | Full        |
| `x86_64-unknown-linux-musl`    | Full        |
| `x86_64-pc-windows-gnu`        | Limited     |

The release target list is closed. Supporting another target requires adding it here and
to the release build; Rust supporting or compiling a target does not add it automatically.

## Full support

Full support covers the documented ACE surface unless a feature spec states a narrower
platform requirement. External programs remain separate dependencies: a feature that
requires Git, tmux, or a backend CLI also requires a compatible version of that program.

## Limited Windows GNU support

`x86_64-pc-windows-gnu` is the only supported Windows target. ACE does not support MSVC
targets or imply general Windows compatibility.

Windows GNU support is opt-in per feature. A feature is supported on Windows only when its
own spec explicitly includes `x86_64-pc-windows-gnu`; silence means unsupported. This
keeps a Windows-specific implementation detail or a successful cross-build from widening
the platform contract accidentally.

The release artifact, PowerShell installer, Windows path resolution, direct backend
process dispatch, and Windows self-update path are included. Managed sessions,
ACE-connect, and workspaces are excluded because their contracts require Unix process
and socket primitives and tmux.

## Verification

Every release builds all seven targets with the pinned Rust toolchain. A toolchain or
dependency change that stops compiling any target is a platform-support regression; the
change must restore compilation or explicitly revise this spec and the release matrix.
