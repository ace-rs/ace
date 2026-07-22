# Releasing ACE

End-to-end runbook for cutting an ACE release. One script does the work
(`release.sh`, with `build-all.sh` as its cross-build primitive); this doc
explains the prereqs, the moving pieces, and what each step is doing.

## 1. Prerequisites

One-time host setup:

- `cargo install cargo-edit` — provides `cargo set-version` (used by
  `release.sh`).
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
./release.sh 0.7.2     # bump, build, patch formula, commit, tag, push, publish
```

Then author the GitHub release notes (§7), notify the website agent (§8),
and post the Discord announcement (§9).

## 3. What each script does

**`release.sh <version>`** — refuses to run with a dirty tree. In one linear
flow: calls `cargo set-version` to update `Cargo.toml` + `Cargo.lock`, writes
`v<version>` to `./latest`, runs `./build-all.sh`, computes the sha256 of
`target/dist/ace-aarch64-apple-darwin`, sed-patches
`homebrew-tap/Formula/ace.rb` (version, download URL, sha), commits and tags
as `v<version>`, pushes `main` and the tag to `gh`, runs `gh release create
v<ver> --generate-notes <binaries>`, re-downloads the published macOS arm64
artifact and verifies its sha matches the formula (aborts if not), then
pushes the formula via `git subtree push --prefix=homebrew-tap gh-tap main`.

The build happens **before** the version-bump commit on purpose: `build.rs`
embeds `ACE_GIT_HASH` from `git rev-parse --short HEAD` and uses
`cargo:rerun-if-changed=.git/HEAD`, so any HEAD movement between the build
and the upload would invalidate the formula sha. Building first means the
shipped binary embeds the pre-bump commit's hash (one behind the tag), but
the formula sha and the uploaded artifact agree — which is what users hit
when `brew install` validates the download.

**`build-all.sh`** — invoked by `release.sh`. Cross-builds all seven targets
into `target/dist/ace-<triple>` (`ace-<triple>.exe` for Windows). Builds
`*-apple-darwin` with plain `cargo build` + `SDKROOT` (Zig 0.14 can't
resolve Apple frameworks); builds the rest with `cargo zigbuild`. Builds
each target group in a single multi-target invocation; on group failure,
retries per-target to isolate which one broke. Also usable standalone for
local cross-build smoke tests.

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
e.g. `v0.6.0`). `release.sh` writes it; the commit on `main` is the source
of truth.

`https://ace-rs.dev/latest` redirects to the raw `./latest` file on `main`,
which is what both installers fetch.

`ace upgrade` reads the same marker — it fetches `https://ace-rs.dev/latest`
and compares against the running version. So a stale `./latest` on `main`
holds every channel back, installers and `ace upgrade` alike. See
`docs/spec/upgrade.md`.

## 6. Homebrew

Formula lives at `homebrew-tap/Formula/ace.rb`, kept in this repo as a git
subtree. `release.sh` sed-patches three lines after the macOS aarch64
binary is built (and before the version-bump commit is created):

- `version "<x.y.z>"`
- `url "https://github.com/ace-rs/ace/releases/download/v<x.y.z>/ace-aarch64-apple-darwin"`
- `sha256 "<sha of the macOS aarch64 binary>"`

After publishing the GitHub release, `release.sh` re-downloads the macOS
arm64 artifact and re-hashes it as a safety net — if the published sha
doesn't match the formula, the script aborts before pushing the subtree to
`gh-tap`. End users install with:

```sh
brew install ace-rs/tap/ace
```

The formula currently only carries the macOS aarch64 binary + sha. Other
platforms are served by `install.sh` / `install.ps1`.

## 7. Author the GitHub release notes

`release.sh` publishes with `gh release create --generate-notes`, which yields
only a bare "Full Changelog" link. After publishing, replace it with a real
summary:

```sh
gh release edit v<ver> --notes-file /tmp/ace-<ver>-ghnotes.md
```

Lead with the **most significant change, not the most user-visible one**. A
core-subsystem rearchitecture headlines even when its surface impact is
indirect — ACE *is* a skill-provisioning tool, so a skill-model overhaul is the
story, not a "plus" line under smaller features. Watch for the inverse trap too:
features that are actually downstream *manifestations* of the headline change
(e.g. an admission-policy or validation tweak that the rearchitecture produced)
belong under it, not promoted alongside it.

The same headline summary feeds the website notify (§8) and the Discord post
(§9) — author it once here, then adapt tone per surface. Write the body to
`/tmp/ace-<ver>-ghnotes.md` (so the harness doesn't mangle
backticks/angle-brackets) and keep the `Full Changelog` compare link at the
bottom.

## 8. Notify the website agent

After every published GitHub release, send an `ace-connect` bridge message
to the `ace-rs.www.claude` peer so the website (schools, commands,
configuration pages) can be regenerated. Include:

- the version tag (e.g. `v0.7.0`)
- a short summary of user-visible changes (new commands, flags, config keys,
  removed behavior)

See the `ace-connect` skill for the send/receive flow.

The bridge truncates lines past ~500 chars. For a release announcement that
lists more than a couple of changes, write the full notes to a tmp file
(`/tmp/ace-<ver>-www.md`) and send a short body that links to it, rather
than stuffing the whole changelog into one line.

## 9. Discord announcement

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

## 10. Open gaps

- **Checksums / signing** — only the Homebrew sha256 is computed. Publishing
  a `SHA256SUMS` file alongside release assets and verifying it from
  `install.sh` / `install.ps1` would be a nice add.
