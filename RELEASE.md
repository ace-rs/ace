# Releasing ACE

End-to-end runbook for cutting an ACE release. Two scripts do the work
(`bump.sh`, `release.sh`); this doc explains the order, the prereqs, and the
moving pieces around them.

## 1. Prerequisites

One-time host setup:

- `cargo install cargo-edit` — provides `cargo set-version` (used by `bump.sh`).
- `cargo install cargo-zigbuild` — cross-compiles the Linux/Windows targets.
- **Zig 0.14.x or 0.15.2** — Zig 0.16 has a known `ar` regression that breaks
  `ring` (rust-cross/cargo-zigbuild#433). `brew install zig` currently pulls
  0.16; install a known-good version manually from
  <https://ziglang.org/download/> if your package manager is too new.
- `gh` CLI, authenticated against `ace-rs/ace`.
- **macOS host** for the full matrix. Linux hosts can build the Linux/Windows
  targets only; the `*-apple-darwin` targets need Apple's toolchain.
- `gh-tap` git remote pointing at the Homebrew tap repo:

  ```sh
  git remote add gh-tap gh:ace-rs/homebrew-tap
  ```

  `release.sh` warns and skips the formula push if this remote is missing.

Optional: `cargo install sccache` to speed up repeat cross-builds.

## 2. Runbook

From a clean working tree on `main`:

```sh
./bump.sh 0.7.0     # bump, build, patch formula, commit, tag (all in one)
./release.sh        # rebuild (cached), push, gh release, subtree push
```

Then notify the website agent (see §7).

## 3. What each script does

**`bump.sh <version>`** — refuses to run with a dirty tree. Calls
`cargo set-version` to update `Cargo.toml` + `Cargo.lock`, writes `v<version>`
to `./latest`, runs `./build-all.sh`, computes the sha256 of
`target/dist/ace-aarch64-apple-darwin`, sed-patches
`homebrew-tap/Formula/ace.rb` (version, download URL, sha), then makes a
single commit `v<version>` containing all of the above and tags it. The
formula update lands in the same commit as the version bump — no follow-up
formula commit, so the source tarball at `v<version>` carries the correct
formula.

**`build-all.sh`** — invoked by both `bump.sh` and `release.sh`. Cross-builds
all seven targets into `target/dist/ace-<triple>` (`ace-<triple>.exe` for
Windows). Builds `*-apple-darwin` with plain `cargo build` + `SDKROOT` (Zig
0.14 can't resolve Apple frameworks); builds the rest with `cargo zigbuild`.
Builds each target group in a single multi-target invocation; on group
failure, retries per-target to isolate which one broke. The `release.sh`
re-run is a cache hit when nothing changed since `bump.sh`.

**`release.sh`** — verifies the current `Cargo.toml` version has a matching
tag on HEAD and the tree is clean, re-runs `build-all.sh` (cached no-op if
`bump.sh` already built), pushes `main` and the tag, runs
`gh release create v<ver> --generate-notes <binaries>`, and pushes the
formula via `git subtree push --prefix=homebrew-tap gh-tap main`.

**`install.sh`** — end-user installer for macOS/Linux. Resolves the latest
tag from `https://ace-rs.dev/latest`, downloads the matching binary from the
GitHub release, and installs to `~/.local/bin/ace`. Run via:

```sh
curl -fsSL https://ace-rs.dev/install.sh | bash
```

**`install.ps1`** — end-user installer for Windows. Same flow as `install.sh`
but installs to `%LOCALAPPDATA%\ace\ace.exe`. Run via:

```powershell
powershell -c "irm https://ace-rs.dev/install.ps1 | iex"
```

## 4. Targets

All seven are built and uploaded to every GitHub release.

| Triple                         | Installer    |
| ------------------------------ | ------------ |
| `aarch64-apple-darwin`         | `install.sh` |
| `x86_64-apple-darwin`          | `install.sh` |
| `aarch64-unknown-linux-gnu`    | `install.sh` |
| `x86_64-unknown-linux-gnu`     | `install.sh` |
| `aarch64-unknown-linux-musl`   | `install.sh` |
| `x86_64-unknown-linux-musl`    | `install.sh` |
| `x86_64-pc-windows-gnu`        | `install.ps1`|

## 5. The `latest` marker

`./latest` at the repo root is the canonical version pointer (plain text,
e.g. `v0.6.0`). `bump.sh` writes it; the commit on `main` is the source of
truth.

`https://ace-rs.dev/latest` redirects to the raw `./latest` file on `main`,
which is what both installers fetch.

`ace upgrade` does not consult `./latest` at all — it discovers versions via
`git ls-remote --tags` against the GitHub repo. See `docs/spec/upgrade.md`.

## 6. Homebrew

Formula lives at `homebrew-tap/Formula/ace.rb`, kept in this repo as a git
subtree. `release.sh` sed-patches three lines after the GitHub release is
live:

- `version "<x.y.z>"`
- `url "https://github.com/ace-rs/ace/releases/download/v<x.y.z>/ace-aarch64-apple-darwin"`
- `sha256 "<sha of the macOS aarch64 binary>"`

It then commits and pushes the subtree to `gh-tap` (which maps to
`ace-rs/homebrew-tap`). End users install with:

```sh
brew install ace-rs/tap/ace
```

The formula currently only carries the macOS aarch64 binary + sha. Other
platforms are served by `install.sh` / `install.ps1`.

## 7. Notify the website agent

After every published GitHub release, send an `ace-connect` bridge message
to the `ace-rs-www.claude` peer so the website (schools, commands,
configuration pages) can be regenerated. Include:

- the version tag (e.g. `v0.7.0`)
- a short summary of user-visible changes (new commands, flags, config keys,
  removed behavior)

See the `ace-connect` skill for the send/receive flow.

The bridge truncates lines past ~500 chars. For a release announcement that
lists more than a couple of changes, write the full notes to a tmp file
(`/tmp/ace-<ver>-www.md`) and send a short body that links to it, rather
than stuffing the whole changelog into one line.

## 8. Discord announcement

After the release is live and the website agent has been notified, draft a brief
Discord message (3–6 lines, casual tone) highlighting the cool new user-visible
features. Lead with the version tag, then bullet the headline changes — skip
internal refactors, doc-only edits, and chores.

Write it to `/tmp/ace-<ver>-discord.md` so it can be copied verbatim without the
harness mangling backticks/angle-brackets/etc.

Discord-flavored markdown template (used for v0.7.0):

```
🎉 **ACE v<ver>** is out — <https://github.com/ace-rs/ace/releases/tag/v<ver>>

- **<headline 1>**
- **<headline 2>**
- **<headline 3>**

Plus: <comma-separated list of smaller user-visible changes>.
```

Notes on the template:
- Wrap the URL in `<...>` so Discord doesn't auto-embed.
- Bold the lead phrase of each bullet; inline-code (`` ` ``) for flag/command
  names inside the bullet body.
- Keep the "Plus:" line to one sentence — anything longer belongs in the
  GitHub release notes, not Discord.

## 9. Open gaps

- **Checksums / signing** — only the Homebrew sha256 is computed. Publishing
  a `SHA256SUMS` file alongside release assets and verifying it from
  `install.sh` / `install.ps1` would be a nice add.
