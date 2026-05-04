# Self-Update

## Overview

ACE checks for newer releases on startup and silently upgrades in the background.
Users can also run `ace upgrade` to upgrade explicitly.

## Version Check

On every `ace` invocation (after state resolution, before exec):

1. Read cache marker `~/.cache/ace/latest_version`.
2. If marker mtime is < 4 hours old, use cached value. Otherwise run
   `git ls-remote --tags https://github.com/ace-rs/ace.git 'v{MAJOR}.*'`
   to discover tags matching the current major version. Parse each tag as
   `semver::Version` (stripping `v` prefix at this boundary only), pick the
   highest, write the canonical semver string to the marker.
3. Parse marker value and current version (`CARGO_PKG_VERSION`) as
   `semver::Version`. All comparisons use semver — no raw string comparison.
4. If latest > current: print hint via `ace.hint()`, then spawn background upgrade.

Network failures are silent — leave the marker untouched, skip the hint.

### Major version gating

Auto-upgrade stays within the current major version. `git ls-remote` is filtered
to `v{MAJOR}.*` so only same-major tags are considered. Crossing a major version
boundary requires explicit `ace upgrade --force X.Y.Z`.

### Skip conditions

The check is skipped entirely when any of these hold:

- `ace upgrade` or `ace --version` is the current command (avoids recursion / noise).
- `--porcelain` flag is set.
- `skip_update = true` in resolved config.
- `ACE_SKIP_UPDATE=1` environment variable is set.

### Source builds

Developers running `cargo install --path .` get the same `CARGO_PKG_VERSION` as
release binaries. Without intervention, the upgrade check would silently replace
the source build with a release binary. Source-build developers should set
`ACE_SKIP_UPDATE=1` in their shell profile or `skip_update = true` in their
user-level config (`~/.config/ace/ace.toml`).

## Background Upgrade

When the check finds a newer version:

1. Print: `ace X.Y.Z available — upgrading in background`
2. Spawn `current_exe() upgrade --silent` as a detached child process with
   stdin/stdout/stderr redirected to null.
3. Main command proceeds immediately — no blocking.

The child process downloads and replaces the binary. Next invocation runs the
new version. Failures are silent.

## `ace upgrade` Command

```
ace upgrade [--silent] [--force [VERSION]]
```

- Default: check latest within current major, download, replace current binary,
  print result.
- `--silent`: suppress all output (used by background spawn). Exit 0 on success,
  1 on failure.
- `--force`: reinstall even if already at latest version. Accepts an optional
  VERSION argument to install a specific version (e.g. `ace upgrade --force 0.3.1`).
  Useful for downgrading, crossing major versions, or recovering from a bad release.

### Steps

1. If `--force VERSION` given, use that version directly (no git ls-remote).
   Otherwise run `git ls-remote --tags` filtered to current major, pick highest.
2. Compare against current version using `semver::Version`. If equal and
   `--force` not set, print "Already at latest version (X.Y.Z)" and exit 0.
3. Determine platform target triple (same mapping as `install.sh`).
4. Download binary from `https://github.com/ace-rs/ace/releases/download/v{version}/ace-{target}`
   (the `v` prefix is added back at this boundary only).
5. Atomic self-replacement (see below).
6. Update cache marker with the new version.

### Self-replacement

Uses `std::env::current_exe()` to locate the running binary — works regardless
of install path (`~/.local/bin`, `/usr/local/bin`, or any custom location).

#### Unix

1. Write new binary to `{current_exe}.new`.
2. Set executable permissions (0o755).
3. `fs::rename()` over `{current_exe}` — atomic on same filesystem.

Safe with running process: the kernel keeps the old inode mapped, new
invocations get the new file.

#### Windows

Running `.exe` files cannot be overwritten but can be renamed.

1. If `{current_exe}.old` exists from a prior upgrade, delete it.
2. Rename running `{current_exe}` → `{current_exe}.old`.
3. Write new binary to `{current_exe}`.
4. Cleanup: `{current_exe}.old` is deleted on next startup or next upgrade.

## Configuration

### `skip_update`

Standard three-layer chain resolution (user → project → local, last wins).

- Type: `bool`
- Default: `false` (absent = auto-update enabled)
- When `true`: disables both the startup version check and silent background upgrade.

### `ACE_SKIP_UPDATE`

Environment variable. When set to `1`, behaves the same as `skip_update = true`.
Intended for CI environments and ephemeral sessions.

## Cache

Marker file: `~/.cache/ace/latest_version`

- Plain text, single line: canonical semver string (e.g. `0.3.2`). No `v` prefix.
- Freshness determined by file mtime.
- TTL: 4 hours.
- Missing file triggers a fresh check.

## Dependencies

- `ureq` (already in deps) for binary download.
- `semver` crate for version parsing and comparison.
- `git ls-remote` for version discovery (no GitHub API dependency).

## Module layout

`src/upgrade/` — standalone helper module, not an action:

- `mod.rs` — public API: `check_for_update()`, `run_upgrade()`, `target_triple()`.
- `check.rs` — cache read/write, git ls-remote, version comparison.
- `download.rs` — binary download for current platform.
- `replace.rs` — atomic self-replacement (platform-specific).

## Out of scope

- Checksum or signature verification.
- Authenticated GitHub API requests.
- Version pinning / locking.
