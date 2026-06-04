# Skill model rearchitect — design notes (2026-06-02)

Status: **design decided (2026-06-04).** The model is now ruled in two decisions —
[skill lifecycle typestate](../decisions/2026-06-04-skill-lifecycle-typestate.md) and
[admission eviction is non-overridable](../decisions/2026-06-04-admission-eviction-non-overridable.md).
This note is retained for the defect catalogue (§ Why) and the still-open forks (2–4); its
lifecycle table and fork 1 are superseded by the decisions and annotated inline below. Spec
edits + the implementation series are the remaining steps.

## Resume — next session (speccing)

Cold-start order for the fresh speccing session:

1. **Ratify naming first (fork 4).** The decisions use provisional markers `Raw` /
   `Validated` / `Judged` and the `Vetted` trait, plus the unresolved path-identity rename
   (away from `SkillId` / `id`). Pin these before specs or impl hard-code them.
2. **Specs.** Rewrite `spec/skills/model.md` § Type-safety invariant to describe the
   `Raw → Validated → Judged` lattice + `Vetted` gate honestly — today it claims an
   end-to-end-typed identity the code never delivered. Update `selection.md` and `emit.md`
   for the `validate`-partition + `(collection, action)` framing, and add the
   eviction-visibility surfaces (dry-run; reconcile summary splitting *admission-evicted*
   vs *config-orphaned*) from the eviction decision. Keep `model.md` behavioral; concrete
   type names stay in decisions/impl (house spec-voice).
3. **Implementation series.** After specs, not piecemeal. Forks 2 (`MatchHandle` keep/cut)
   and 3 (package placement) get decided during impl.

**Uncommitted at save (2026-06-04):** the two new decisions, this note, and the 2026-05-30
supersession edit are on disk but not committed — branch is 34 ahead of `gh/main`, unpushed.
Commit/push in the resume session.

## Why

A type-audit of the skills subsystem (`src/skills/`, `src/resolver/`,
`src/actions/project/link_skills.rs`) found an **incoherent data model**: typed-domain
wrappers were introduced but never threaded through, so the system actually runs on raw
`String` while the typed layer floats unused.

Concrete defects:

- **`SkillId` dies at the door.** Minted by discovery (`DiscoveredSkill.id`, private
  constructors, "the only path into existence" invariant), then discarded one hop later —
  `from_discovered_inner` does `name: d.id.to_string()`, and the imports resolver
  stringifies it immediately. It never reaches the model (`Skill.name: String`) or emit.
- **`MatchHandle` is 100% dead.** A complete, 11-test newtype (validate-once invariant,
  classifiers, `matches`) with `#[allow(dead_code)]` on the type *and* impl; the only
  non-test reference is the `pub use`. Both resolvers bypass it and call the free
  `pattern_matches(&str, &str)` directly.
- **Resolvers are stringly-typed.** `resolve_skills(&[String])`,
  `ResolvedImport.identity: String`, `Collision.skill: String`, `MatchedSkill.identity:
  String`. `identity.rs` admits it: *"resolvers hold patterns/identities as `String` for
  historical reasons."*
- **`DiscoveredSkill` and `Skill<Discovered>` are parallel structures** over the same
  domain — same five fields, mirrored `admission()` / `frontmatter_warning()`. The
  `Discovered` marker is empty, so `DiscoveredSkill` is essentially `Skill<Discovered>`
  minus the `SkillId` erasure.

`SkillId`-dies-early and `MatchHandle`-floats are the **same defect wearing two hats**:
the typed-domain layer (identity, pattern) was added but never threaded. So neither can be
keep/cut in isolation — they belong to a model that needs designing.

This also closes a real **spec-vs-code divergence**: `docs/spec/skills/model.md`
§ Type-safety invariant *claims* identities are typed end-to-end. They are not. Fixing the
model is compliance, not gold-plating.

## Lifecycle (the design substrate)

> **Superseded (2026-06-04).** The linear Found → Sourced → Judged → Emitted framing below
> was re-derived clean-slate into a `(collection, action)` algebra over a three-state marker
> lattice **`Raw → Validated → Judged`**: Sourced collapses into a Raw-stage field
> (provenance), Emitted is `DesiredLink` (not a `Skill` state), and admission folds into a
> `validate` partition rather than a stage. See
> [the lifecycle decision](../decisions/2026-06-04-skill-lifecycle-typestate.md). The table
> below is kept for its intent (design from the flow, not the current types), not its stages.

Where a skill actually flows through the system. Design the data/metadata from this, not
from the current types.

| Stage        | What exists                                           | Metadata gained            |
| ------------ | ----------------------------------------------------- | -------------------------- |
| 1. **Found** | path + frontmatter facts (tier, internal, disp. name) | intrinsic data only        |
| 2. **Sourced** | same, pulled from an import                          | origin label (own = none)  |
| 3. **Judged** | admission verdict + selection decision               | two orthogonal verdicts + *why*-trace |
| 4. **Emitted** | included skills → backend links (leaf name)          | reconcile state            |

Three mechanisms map onto this:

- **atom** = stage-1 data: the path-identity + frontmatter facts.
- **collection** = the set + resolution-wide diagnostics.
- **metadata layering** = stages 2–3 stacked on the atom — the real job of `Skill<S>`,
  currently half-done.

## Direction (converged, not finalized)

- **Identity is a locator, not an `id`.** `SkillId` / `id:` signals a DB surrogate key we
  don't have. It's the *path the skill lives at*, which **is** its identity. The atom
  carries its own path; rename away from `Id`/`id` (candidate: a path-identity type, name
  TBD).
- **Carry the typed identity end-to-end.** `Skill<S>` holds the path-identity, not
  `name: String`. Survives discovery → resolution → emit. `basename` becomes `leaf()`
  (already on the type).
- **Fold `DiscoveredSkill` into `Skill<Found>`.** `discover_skills` returns the atom type;
  `admission` / `frontmatter_warning` live once, not mirrored across two types.
- **De-stringify the resolvers.** `ResolvedImport.identity`, `Collision.skill`, etc. carry
  the path-identity; the "historical reasons" comment goes away.
- **Provenance is a first-class metadata layer**, not a bolted-on `Option<String>`.
  Decide its shape alongside the typestate.

## Open forks (decide in the design session)

1. **Metadata placement** — **resolved (2026-06-04):** intrinsic facts are struct fields;
   selection decision + trace is the `Judged` marker payload (per atom); admission is never
   carried (recomputed from identity); provenance is a Raw-stage field; set-level diagnostics
   live on the collection. See the lifecycle decision § Metadata placement.
2. **`MatchHandle` keep vs. cut** — now a *sub-decision* of the model, not standalone.
   Either thread it config → resolver (validate-once becomes real, type justified) **or**
   rule patterns stay raw `String` + `pattern_matches` and delete `MatchHandle`. The model
   decision picks one.
3. **Package placement** — who owns atom / collection / resolver. Likely a rearrangement
   across `skills/` and `resolver/`.
4. **Naming** — still open. Marker names `Raw` / `Validated` / `Judged` and the `Vetted`
   trait are **provisional** (used by the decisions pending ratification); the path-identity
   rename away from `SkillId`/`id` is unsettled. The atom (`Skill`) vs. collection (`Skills`)
   names stand.

## Approach

Large change: touches `skills/mod.rs`, `discover.rs`, `identity.rs`,
`resolver/project.rs`, `resolver/imports.rs`, `link_skills.rs`, and rewrites
`model.md` § Type-safety invariant. Design-doc-first, then an implementation series — not
piecemeal edits. **Next session is a dedicated skill-rearchitect.**

## Leftover audit items (fold in where they fit)

Minor findings from the same audit, not load-bearing but cheap to fix while in here:

- **Two frontmatter-hygiene paths now coexist** — `ImportCollision.frontmatter_mismatch`
  (cross-source `name:` divergence) and the new `DiscoveredSkill::frontmatter_warning`
  (bad-char / non-token). Cross-reference them, or unify under the deferred
  `ace school validate` lint.
- **`SkillId` conversion sprawl** — `Deref` + `AsRef` + `Borrow` + 3×`PartialEq` + 2×`From`
  exist mostly to paper over downstream holding `String`. Carrying the typed identity
  retires several.
- **`Skill.source: Option<String>`** — judged fine (absence = "school's own skill" is a
  distinct state), but revisit once provenance becomes a first-class layer.
- **Intersects the deferred `validate` slice** — the 2026-06-01 decision's "Open / to
  implement" (flat-collapse sim, dead-selector, divergence warning) overlaps the
  frontmatter-hygiene path above; sequence the two so they don't collide.

## References

- [name = path decision](../decisions/2026-06-01-skill-name-is-path.md) — identity = path;
  the boundary this builds on.
- [name admission policy](../decisions/2026-05-30-skill-name-admission-policy.md)
  § Abstraction Boundaries — already flags `Skill.name: String -> SkillId` as deferred.
- [discovery & identity](../decisions/2026-05-26-skill-discovery-identity-storage.md).
- `docs/spec/skills/model.md` § Type-safety invariant — the claim the code must be made to
  honor.
