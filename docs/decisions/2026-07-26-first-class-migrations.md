# Decision: Storage Migrations Are First-Class (2026-07-26)

Status: **decided** — spec at [../spec/migrations.md](../spec/migrations.md).

Baseline: ACE v0.8.1.

## Problem

On-disk layout changes have been handled one at a time, by hand, in `main.rs`:
`migrate_legacy_index_toml` moves the pre-PROD9-76 `index.toml` and
`warn_stray_cache_dirs` nudges about the old flat cache tree. Both work, and both leave
the old copy on disk forever — the legacy `index.toml` is re-detected and re-warned on
every startup until the user removes it manually. There is no record of which layout the
state is on, so each new change means another bespoke shape-sniffing startup function,
and an older binary meeting a newer tree cannot tell that is what happened.

The import-source rework (path-traversal fix) changes the import cache from flat
`owner/repo` to host-scoped `host/path…`, which would have been the third such
hand-rolled case.

## Decision

Migrations become a declared mechanism rather than a pattern re-implemented per change:

- **One `layout_version` key in `index.toml`** — the single internal metadata file,
  already ACE-owned and already read at startup. No new dotfile, no per-store metadata.
  User config (`ace.toml`, `school.toml`) never carries bookkeeping.
- **The value is the ISO date the layout change landed**, not the ACE release version and
  not semver. On-disk shape changes on its own schedule; dating it avoids expressing
  steps as version ranges and rewriting the key for releases that changed nothing. Dates
  sort lexicographically and line up with the dated decision doc behind each step.
- Version is recorded, never inferred. State newer than the running binary is refused
  with an upgrade hint, not migrated.
- Steps are units of work in `src/actions/migrate/`, registered as one date-ordered list,
  run once at startup.
- **Tear and rebuild is the default.** Re-derivable state is deleted so the next command
  re-fetches it; in-place transforms are not written for data a re-clone reproduces. The
  narrow exception is state that exists nowhere else — a dirty or ahead-of-origin school
  clone, and `index.toml`'s own contents — which a step never deletes on a guess. No
  backup files either way.
- **Every migration that touches disk logs exactly one line** naming the version
  transition and what happened, with paths and counts for deletions. No-ops print
  nothing.

`migrate_legacy_index_toml` and `warn_stray_cache_dirs` are retired into registry steps.

## Relation to the Backcompat Policy

CLAUDE.md says storage migrations are detect-and-hint, not silent auto-migration. That
rule was written against user data, where a wrong guess destroys work. It is refined
here rather than overturned: the guarantee it protects is "ACE never silently destroys
something the user cannot get back", and that guarantee is what the tear-and-rebuild
exception list encodes. A cache clone is re-fetchable by definition, so refusing to clean
it buys no safety and costs a stale tree ACE itself created. The mandatory log line
closes the remaining gap — automatic, but never invisible.

## Alternatives considered

- **A dedicated `layout.toml` per store.** Rejected on sight: a metadata file per concern
  is how a tool accumulates ten settings files with no authoritative one. `index.toml`
  already exists for exactly this.
- **Version as a directory component** (`imports/v2/…`). Self-describing and isolating,
  and it makes a wrong-version read impossible rather than merely detectable. Rejected:
  it strands every previous generation on disk by construction, which is exactly the
  untidiness this decision exists to end.
- **A monotonic counter (`layout = 2`) or the ACE release version.** The counter works but
  carries no meaning at a glance; the release version drags semver ranges into the
  registry and churns on releases that touch nothing. A date does both jobs — ordered and
  self-explaining.
- **Keep shape-sniffing, no recorded version.** Cheapest for one more change. Rejected:
  it cannot distinguish "layout I have never seen" from "layout from the future", so an
  old binary meeting a new tree does the wrong thing confidently.
- **Lossless in-place transforms with `.bak` safety nets.** Rejected as over-engineering
  for re-derivable data: the backup is residue with extra steps, and the transform is
  strictly more code and more failure modes than deleting and re-cloning.
- **A user-facing `ace cache clear` instead of migrations.** Useful independently, but it
  makes correctness the user's job: unmigrated state is wrong until they think to run it.
