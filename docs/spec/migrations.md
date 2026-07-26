# Storage Migrations

## Overview

ACE owns several on-disk areas and their shapes change across releases. A **store** is
one such area; each store carries a **layout version**, and a **migration** moves a store
from one version to the next. Migrations are first-class: a declared registry, a single
execution point, a mandatory log line, and a hard rule that nothing from the old layout
survives a successful run.

Two properties drive the design:

- **Tidiness.** A migration that leaves the previous layout on disk has not finished.
  The user's filesystem is left clean whenever ACE can do so safely.
- **Visibility.** Anything that moves or deletes user-visible files says so, once, in
  plain language. Silent mutation of on-disk state is never acceptable, however
  disposable the data.

## Stores

| Store     | Kind  | Root                       | Contents                          |
|-----------|-------|----------------------------|-----------------------------------|
| `imports` | cache | `<user_cache>/ace/imports` | Clones of upstream import sources |
| `schools` | data  | `<user_data>/ace`          | School clones, `index.toml`       |

**Kind** decides policy, not size:

- **cache** — every byte is re-derivable from a remote. A migration may delete and let
  the next command re-fetch.
- **data** — may carry work that exists nowhere else (a dirty school clone, the school
  index). Nothing is deleted on a guess.

## Layout version

Each store root holds `layout.toml`:

```toml
version = 2
```

The version is recorded, never inferred from directory shape. Recording it is what makes
the store readable by a binary that does not understand it: a store whose version is
**newer** than the running ACE is not touched at all — the command fails with an
upgrade hint rather than migrating downward or corrupting the tree. Shape-sniffing cannot
express that, because an unrecognized shape and a future shape look identical.

A missing `layout.toml` means version 0: the pre-versioning era. Version 0 is the only
version a migration may detect by shape, and only to identify what it must clean up.

## Registry

Migrations are declared per store as an ordered list of steps, each with a `from` and
`to` version exactly one apart. There is no branching and no skipping: upgrading from 0
to 3 runs three steps in order, each logging its own line. A step is a unit of work
(`struct` + `run(&self, ace)`, per the action pattern) living in `src/actions/migrate/`.

Execution happens once at startup, before command dispatch, and costs one small file read
per store. A store root that does not exist yet is not migrated — it is created at the
current version, and `layout.toml` is written when the store is first populated.

## Policy by kind

**cache** — migrate automatically, including delete-and-re-derive. The old tree is
removed in the same step that establishes the new one; a cache store never accumulates
generations. Failure is not fatal: warn and continue, since the next command can retry.

**data** — migrate automatically only when the transform is provably lossless (a move, a
rewrite with the same information content). The pre-migration state is preserved as a
sibling `.bak`, reported in the log line, and deleted on the next successful startup —
so the backup is a safety net with an expiry, not permanent residue. A transform that
could lose information is not run: it is detected and reported with instructions, and the
store stays at its old version. Failure is fatal for the command that needed the store.

## Log lines

Every migration that changes anything on disk emits exactly **one** line naming the store,
the version transition, and what actually happened to the files:

```
Migrated imports cache v1 → v2 (host-scoped paths; removed 3 stale clones)
Migrated schools v0 → v1 (index.toml moved to ~/.local/share/ace; backup at index.toml.bak)
Removed stale backup ~/.local/share/ace/index.toml.bak
```

Rules:

- A no-op migration prints nothing. Silence means "already current" — the absence of
  output is the signal, so no line may be emitted speculatively.
- Deletions always name the path and the count. "Cleaned up" without a path is not an
  acceptable line.
- The line is emitted after the change succeeds, never before.

## Out of scope

- **Downgrade.** A store is never moved backward; an older binary refuses instead.
- **Config-file schema changes** (`ace.toml`, `school.toml`). Those are handled at load
  time by serde defaults and normalize-on-load, not by this mechanism — see
  [configuration.md](configuration.md).
- **Cross-machine or shared-cache coordination.** Migrations assume a single local
  filesystem and a single ACE process at a time.
