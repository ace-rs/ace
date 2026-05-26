---
name: consumer-side-collision-followup
type: notes
date: 2026-05-26
---

# Follow-up: consumer-side cross-source collision warnings

## What's missing

`docs/spec/skills/selection.md` § Warning boundaries spells out two
surfaces:

1. `ace school pull-imports` — school maintainer's machine.  *(landed in
   this session via `surface_import_diagnostics` in
   `src/actions/school/pull_imports.rs`.)*

2. **Consumer discovery** (`ace pull` / `ace setup` of a downstream
   project) — "only if the school maintainer ignored their own
   warnings." **Not implemented.**

The school's materialized `<school>/skills/` already encodes the
first-wins outcome (one copy per identity), so a consumer can't
directly observe the collision after the fact. To restore the
warning surface the spec mandates, the consumer would need either:

- **Replay the resolver** locally: re-clone the school's declared
  `[[imports]]` sources, run `resolve_imports`, surface the same
  warnings. Heavyweight (network on every `ace pull`).
- **Cached warning manifest**: school write a small file (e.g.
  `<school>/.ace/collisions.toml`) at pull-imports time; consumer
  reads + surfaces it.  Lightweight, but adds a manifest contract
  that the rest of ACE deliberately avoids.

## Why deferred

Neither option is small. The pragmatic envelope: ship the
maintainer-side warnings, document the consumer-side gap, watch for
real reports.

## Trigger for revisiting

Any of:
- A real school in the wild ships with unresolved cross-source
  collisions and downstream consumers don't notice.
- We add any other consumer-side validation surface (doctor checks,
  health pings) — would batch naturally with this.
- Plugin or lockfile work (currently out of scope) brings a manifest
  contract back into the picture.

## Related

- `docs/spec/skills/selection.md` § Warning boundaries (the spec line
  we're leaving unmet).
- `docs/spec/skills/sync.md` (consumer-side workflow).
- `src/actions/school/pull_imports.rs` `surface_import_diagnostics`
  (the present-day warning emit code).
