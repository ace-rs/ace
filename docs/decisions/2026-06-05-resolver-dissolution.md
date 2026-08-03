# Dissolve `src/resolver/`: resolution lives with the data it stamps

- **Date:** 2026-06-05
- **PR:** manual
- **Status:** accepted

## Decision

There is no standalone `src/resolver/` module. Resolution lives with the typed data each
resolver reads and stamps:

- **Skill resolution** stamps `Skill<S>` → `src/skills/resolve/`.
- **Config merge** reads only `Tree` / `AceToml` → `src/config/resolve/`. `Source` and
  `Sourced` live here too ("which config layer won" is a config concept); `skills/resolve/`
  imports them leftward.

This **supersedes § Module layout of**
007 (Config Resolution Redesign, folded into `docs/spec/architecture.md` + `docs/spec/configuration.md`) — specifically
its `src/resolver/` package and the `src/resolver/skills.rs` placement. Every other part of
007 (the demand-driven four-layer pipeline, lazy `OnceCell` bindings, per-binding error
types, the override layer) stands unchanged.

## Rationale

007 placed both resolutions in a shared `resolver/` module for one reason: a single `Source`
vocabulary. That co-location forced skill resolution to sit *left* of `skills/` in the
dependency order, so it could not import the skill types and downgraded to `String` — the
stringly `Carry` / `by_name` round-trip the
[the lifecycle spec](../spec/skills/lifecycle.md) set out to remove. The strings
were never historical debt; they were a layering-violation workaround.

Moving each resolution into the module that owns its types fixes both at once: skill
resolution stamps `Skill<S>` directly (the seam disappears rather than de-stringifying), and
config merge keeps touching only config types. The shared `Source` enum still lives in one
place (`config/resolve/`) and `skills/resolve/` imports it — a leftward import, the correct
direction.

The one constraint 007 actually load-bears — `merge` stays infallible and skills-free so
`ace config show` works without a school clone — is preserved: `config/resolve` touches only
`Tree` / `AceToml`, never a discovered school.

## Why not keep `resolver/`

A standalone resolver only earns its place if it owns a domain. It doesn't: it was a holding
pen for two resolutions that each belong with their data. Keeping it to host the `Source`
enum is backwards — one shared enum is an import, not a module.

## References

- Supersedes § Module layout of 007 (folded into the specs above).
- Implements fork 3 of
  [the lifecycle spec](../spec/skills/lifecycle.md).
- Origin: [skill-model rearchitect note](../scratch/2026-06-02-skill-model-rearchitect.md).
