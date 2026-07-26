# Decision: Storage Migrations Are First-Class (2026-07-26)

Status: **decided** — spec at [../spec/migrations.md](../spec/migrations.md).

Baseline: ACE v0.8.1.

## Problem

On-disk layout changes have been handled one at a time, by hand, in `main.rs`:
`migrate_legacy_index_toml` moves the pre-PROD9-76 `index.toml` and
`warn_stray_cache_dirs` nudges about the old flat cache tree. Both work, and both leave
the old copy on disk forever — the legacy `index.toml` is re-detected and re-warned on
every startup until the user removes it manually. There is no record of which layout a
store is on, so each new change means another bespoke shape-sniffing startup function,
and an older binary meeting a newer tree cannot tell that is what happened.

The import-source rework (path-traversal fix) changes the import cache from flat
`owner/repo` to host-scoped `host/path…`, which would have been the third such
hand-rolled case.

## Decision

Migrations become a declared mechanism rather than a pattern re-implemented per change:

- A **store** (`imports`, `schools`) records its layout version in `layout.toml` at its
  root. Version is recorded, never inferred; a store newer than the running binary is
  refused, not touched.
- Steps are units of work in `src/actions/migrate/`, registered per store as an ordered
  `from`→`to` list, run once at startup.
- **Policy splits by store kind.** Cache stores auto-migrate and may delete-and-re-derive.
  Data stores auto-migrate only when the transform is lossless, leave a reported `.bak`,
  and sweep that backup on the next successful startup. Lossy transforms stay
  detect-and-hint.
- **Every migration that touches disk logs exactly one line** naming the store, the
  version transition, and what happened to the files, with paths and counts for
  deletions. No-ops print nothing.
- No store keeps a previous generation after a successful migration. The only permitted
  residue is a data-store `.bak`, and it has an expiry.

`migrate_legacy_index_toml` and `warn_stray_cache_dirs` are retired into registry steps.

## Relation to the Backcompat Policy

CLAUDE.md says storage migrations are detect-and-hint, not silent auto-migration. That
rule was written against user data, where a wrong guess destroys work. It is refined
here rather than overturned: the guarantee it protects is "ACE never silently destroys
something the user cannot get back", and that guarantee is untouched for data stores.
A cache clone is re-fetchable by definition, so refusing to clean it buys no safety and
costs a stale tree that ACE itself created. The mandatory log line closes the remaining
gap — the migration is automatic, but never invisible.

## Alternatives considered

- **Version as a directory component** (`imports/v2/…`). Self-describing and isolating,
  and it makes a wrong-version read impossible rather than merely detectable. Rejected:
  it strands every previous generation on disk by construction, which is exactly the
  untidiness this decision exists to end, and cleanup would need a sweeper anyway.
- **Keep shape-sniffing, no metadata file.** Cheapest for one more change. Rejected: it
  cannot distinguish "layout I have never seen" from "layout from the future", so an old
  binary meeting a new tree does the wrong thing confidently.
- **Sidecar metadata per cache entry** (origin URL, fetch time). Unnecessary — the clone
  already records its origin in `.git/config`; duplicating it invites the two to disagree.
- **A user-facing `ace cache clear` instead of migrations.** Useful independently, but it
  makes correctness the user's job: an unmigrated store is wrong until they think to run
  it.
