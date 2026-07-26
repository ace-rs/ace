# Storage Migrations

## Overview

ACE owns several on-disk areas and their shapes change across releases. A **migration**
moves that on-disk state from one layout version to the next. Migrations are first-class:
one recorded version, a declared registry of steps, a single execution point, and a
mandatory log line.

Three properties drive the design:

- **One internal metadata file.** All of ACE's own bookkeeping lives in `index.toml`.
  A settings file per concern is how a tool ends up with ten of them, none authoritative.
- **Tidiness.** A migration that leaves the previous layout on disk has not finished.
- **Visibility.** Anything that moves or deletes files says so, once, in plain language.

## Where the version lives

`index.toml` in the data dir (`<user_data>/ace/index.toml`) is ACE's single internal
metadata file. It gains one top-level key:

```toml
layout_version = "2026-07-26"
```

It is already read at startup and already ACE-owned, so the version costs no new file.
`ace.toml`, `ace.local.toml`, and `school.toml` are user-edited config and never carry
bookkeeping — the split is by *who writes it*, not by what it describes.

The value is the ISO date the layout change landed, **not** the ACE release version and
not semver. It describes the shape of the state on disk, which changes on its own
schedule: tying it to releases would mean expressing every step as a version range,
rewriting the key on releases that changed nothing, and remembering which releases
happened to alter layout. ISO dates sort lexicographically, so "am I current" is a string
compare, and each step lines up with the dated decision doc that introduced it. One step
per date — if two layout changes land the same day, they are one step.

The version is recorded, never inferred from directory shape. Recording it is what makes
the state readable by a binary that does not understand it: a `layout_version` **newer**
than the running ACE is not migrated at all — the command fails with an upgrade hint
rather than migrating downward or rewriting a tree it cannot model. Shape-sniffing cannot
express that, because an unrecognized shape and a future shape look identical.

A missing `layout_version` key means the pre-versioning era — the only state a step may
detect by shape, and only to identify what to clean up. A missing `index.toml` entirely
means a fresh install: nothing to migrate, write the current version.

## Registry

Steps are declared as one list ordered by date. No branching, no skipping: state three
steps behind runs all three in order, oldest first, each logging its own line. A step is
a unit of work (`struct` + `run(&self, ace)`, per the action pattern) in
`src/actions/migrate/`, named for its date, and it owns whatever paths it touches. The
current layout version is the date of the last step in the list.

Execution happens once at startup, before command dispatch, and costs one file read that
already happens.

## Tear and rebuild by default

Most of what ACE writes is re-derivable from a remote — import-source clones, school
clones, emitted skill trees. **The default migration is to delete the old thing and let
the next command rebuild it.** Transforming a tree in place is more code, more failure
modes, and buys nothing when a re-clone produces a provably correct result.

The exception is narrow and concrete: state that exists nowhere else. A school clone with
uncommitted work or commits ahead of origin (`UpdateOutcome::Dirty` / `AheadOfOrigin`),
and the contents of `index.toml` itself. A step never deletes those on a guess; if a
layout change would require it, the step reports what it found and leaves the state alone.

No backup files. A `.bak` for re-derivable data is residue with extra steps, and for
non-derivable data the answer is not to touch it in the first place.

Failure of a rebuild step is not fatal — warn and continue, since the next command
retries. Failure to migrate `index.toml` itself is fatal for the command that needed it.

## Log lines

Every migration that changes anything on disk emits exactly **one** line naming the
version transition and what actually happened to the files:

```
Migrated layout to 2026-07-26 (host-scoped import paths; removed 3 stale clones from ~/.cache/ace/imports)
Migrated layout to 2026-04-22 (moved index.toml to ~/.local/share/ace)
```

Rules:

- A no-op migration prints nothing. Silence means "already current" — the absence of
  output is the signal, so no line may be emitted speculatively.
- Deletions always name the path and the count. "Cleaned up" without a path is not an
  acceptable line.
- The line is emitted after the change succeeds, never before.

## Out of scope

- **Downgrade.** State is never moved backward; an older binary refuses instead.
- **Config-file schema changes** (`ace.toml`, `school.toml`). Handled at load time by
  serde defaults and normalize-on-load — see [configuration.md](configuration.md).
- **Cross-machine or shared-cache coordination.** Migrations assume a single local
  filesystem and a single ACE process at a time.
