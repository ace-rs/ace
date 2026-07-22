# Skill lifecycle: typestate proves actions, gated by a `Vetted` trait

- **Date:** 2026-06-04
- **PR:** manual
- **Status:** revised

> **Revised (2026-06-05):** fork-4 naming ratified. The provisional markers
> `Raw → Validated → Judged` are now **`Discovered → Validated → Decided`**; the `Vetted`
> trait name stands; the identity type is **`Locator`** (field `locator`), replacing
> `SkillId` / `id`, and the `Skill.name` field is **dropped** (callsites resolve to `locator`
> or `frontmatter_name`). Names updated throughout; substance unchanged. Rationale in
> § Open / downstream, fork 4. **Forks 2 (`MatchHandle`) and 3 (package placement) also
> resolved 2026-06-05** — see § Open / downstream.

## Decision

The skill model is a `(collection, action)` algebra over a three-state marker lattice
**`Discovered → Validated → Decided`**. A typestate marker proves **an action ran in this
process's call graph** — it stores no verdict and persists nothing. `validate` is a
**partition** (`Skills<Discovered> → (Skills<Validated>, Vec<Rejected>)`), and a sealed
`Vetted` capability trait — implemented by every state at or past `Validated`, never by
`Discovered` — gates the boundaries that must not see an un-vetted skill (disk write, backend
emit).

This supersedes the *in-memory mechanism* of
[name admission policy](2026-05-30-skill-name-admission-policy.md) (select-over-everything
+ `rejected()` as a view); the admission predicate and its discovery-gate placement are
unchanged. Resolves fork 1 (metadata placement) of
[the rearchitect note](../scratch/2026-06-02-skill-model-rearchitect.md); forks 2 (MatchHandle),
3 (packages), and 4 (naming) resolved 2026-06-05 (§ Open / downstream).

## Context

A type-audit (note above) found the skills subsystem running on raw `String` while a typed
layer (`SkillId`, `MatchHandle`) floated unused — `SkillId` minted by discovery and discarded
one hop later (`name: d.id.to_string()`), `MatchHandle` 100% dead, both resolvers
stringly-typed. The defect isn't "thread the existing types harder"; it's that the
**lifecycle was never modelled**, so there was no structure to thread them through. This
decision fixes the model; the type-threading falls out of it.

## The reframe: a marker proves an action ran, not a stored verdict

The blocker to using typestate here was a false objection. ACE's no-version philosophy
forbids *persisting* an admission verdict — a stored verdict goes stale the instant the
rules tighten (see
[the eviction decision](2026-06-04-admission-eviction-non-overridable.md)) — so "we have
nowhere to store the verdict" looked like an argument against the marker.

It isn't. **A typestate marker writes nothing to disk.** `Skill<Validated>` is a
compile-time token meaning "`validate()` sat on the path that produced this value." It is
rebuilt from scratch every process and proves only *call-graph ordering*. The versioning
philosophy constrains *persistence*, not *type-level proof of in-process ordering* — the
two are unrelated. Once that conflation is dropped, typestate is not merely permissible but
the precise tool: it makes "you must validate before you persist" a compiler-enforced
obligation that costs no storage and re-runs freely.

## The lattice

```
discover  ∅ ──▶ Skills<Discovered>
validate        ──▶ Skills<Validated>   (partition: also yields Vec<Rejected>)
resolve         ──▶ Skills<Decided>
emit            ──▶ Vec<DesiredLink>    (from Decided.included) ──▶ links

persist<S: Vetted>        gates disk write — Validated and Decided impl Vetted; Discovered does not
Compose: import-resolve ──▶ copy_into     value-level; re-enters the lattice through the Vetted gate
```

Actions: `discover` (∅ → Discovered), `validate` (Discovered → Validated, splitting off
Rejected), `resolve` (Validated → Decided), `emit` (Decided.included → links), and the
Compose-side `import-resolve` + `merge` + `copy_into`. Each marker's job — a marker with no
job gets cut:

| Marker       | Carries                                 | Job — the gate it powers                                                                                                    |
| ------------ | --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `Discovered` | identity + intrinsic facts + provenance | *Negative capability*: not `Vetted`, not `Decided`, so the compiler forbids persisting or emitting it. Input to `validate`. |
| `Validated`  | nothing (unit marker)                   | Only constructible via `validate`. `impl Vetted` → gates `persist`/`copy_into`. Precondition for the resolvers.             |
| `Decided`    | per-atom decision + trace               | `impl Vetted`. Gates `emit`. Carries the trace `ace explain` / `ace skills` read per skill.                                 |

## The `Vetted` trait — the "or" as a bound, not a union

A naïve encoding of two proof axes (validated, selected) as independent phantom params
forces a 2-D product `Skill<V, S>` with mostly-dead corners, or a sum type for functions
that accept "validated OR decided." Both are wrong. The functional answer is a **sealed
capability trait**:

```rust
trait Vetted {}                  // sealed: external code cannot impl it
impl Vetted for Validated {}
impl Vetted for Decided {}
// Discovered deliberately does NOT impl Vetted

fn persist<S: Vetted>(skills: &Skills<S>) -> io::Result<()>;   // no un-vetted skill to disk
fn emit(skills: &Skills<Decided>) -> Vec<DesiredLink>;          // only resolved skills emit
```

`Vetted` *is* the "or" — "any state at or past `validate`" — without enumerating a union or
multiplying type parameters. Markers are nodes; the trait is a gate that accepts a family of
nodes. Sealing it keeps `Discovered` (and any future pre-validate state) permanently out.

## `validate` partitions; it does not annotate

For `Vetted` to *mean* "contains only admissible skills," `validate` must **remove** the
inadmissible ones, not tag them:

```rust
fn validate(discovered: Skills<Discovered>) -> (Skills<Validated>, Vec<Rejected>);
```

The `Rejected` half carries each rejection reason and feeds warnings / doctor — so the
"internal model sees violations so doctor can report them" invariant is **relocated, not
lost**: rejects are an explicit second output, not inadmissible skills lurking in the main
collection wearing a `Status::Rejected`.

This is the one place we amend a ratified boundary. The 2026-05-30 policy's *Abstraction
Boundaries* described selection running over everything with `rejected()` as a *view* on a
single collection. Partition-first replaces that, deliberately: an annotate-and-keep
`validate` would leave inadmissible skills inside `Skills<Validated>`, so `persist` could
not trust the type and would have to re-filter — which defeats the gate. Partition is
*forced* the moment `Vetted` must guarantee the safety property rather than merely assert
it. It also matches the 2026-05-30 *intent* ("`included()` / `excluded()` require
admissibility") better than its described mechanism did.

## Metadata placement (resolves fork 1)

Where each piece of per-skill data lives, and why:

| Data                                                 | Home                                         | Why                                                                                                |
| ---------------------------------------------------- | -------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| identity, tier, `internal`, display name             | struct fields on `Skill<S>`, every stage     | intrinsic facts, present from discovery on                                                          |
| provenance (own vs import source)                    | struct field on the atom (Discovered onward) | metadata some actions attach (`import`) and others ignore (`emit`); dead weight in Provision, fine  |
| admission verdict                                    | **nowhere — recomputed on demand**           | a cheap pure function of identity; carrying it risks staleness and buys nothing                     |
| selection decision + trace                           | the `Decided` marker payload (per atom)      | genuine new state: config-derived, set-relative, expensive; `ace explain` reads it per skill        |
| set-level diagnostics (unknown patterns, collisions) | the `Skills<Decided>` collection             | no per-atom home; produced by the resolution run                                                    |

**Admission is never carried.** It is a derived predicate over identity, recomputed wherever
needed — like `leaf()` or `is_nested()`. Identity is the only durable state; admissibility
falls out of it. This is both versioning-correct (nothing to go stale) and the honest shape
(admission needs no config and no set context, unlike selection).

The marker therefore lives **on the atom** for `Decided` (decision + trace is real per-skill
payload, so `Skill<Discovered>` has no decision and `Skill<Decided>` has one — a type fact, not
an `Option`), and the collection rides the same parameter so `Skills<Decided>` can own the
set-level diagnostics. `Validated` carries no atom payload — its proof is *set membership*
(passed the partition), which is collection-level by construction.

### The accepted asymmetry

A lone `Skill` plucked from a `Skills<Validated>` via `find()` is just `Skill` — it loses
the proof. Validation lives on the *set*; selection's payload lives on the *atom*. A function
needing a single vetted skill takes the collection or re-checks. This is the one real
give-up, and it's honest: a marker carries payload only when there is payload, and
validation's "payload" is membership, not a per-skill verdict.

## Compose stays value-level past `Validated`

**Principle: a typestate is earned only where something downstream reads the carried payload
*per atom*.** Provision's decision + trace feeds `ace explain` one skill at a time → earns
`Decided`. Compose's import resolution feeds **aggregate** collision warnings plus a **path
list** for `copy_into` → no per-atom carrier, so it stays value-level (`ResolvedImport`
records as diagnostics). There is no `ImportDecided` typestate. Import resolution consumes
`Skills<Validated>`, emits warnings + paths, and `copy_into` re-enters the lattice through
the `Vetted` gate. One fewer marker, justified by the principle rather than convenience.

## Versioning and typestate compose with zero redundancy

Because `validate` re-partitions from scratch every process, versioning-safety (self-healing
on upgrade) is satisfied at the `validate` step. Within a run, a `Decided` skill is admissible
*by lineage* (`Decided: Vetted`, and the sealed trait means nothing fabricates a `Decided` that
skipped `validate`). So **emit needs no admission re-check** — the safety property falls out
of construction, not a guard we must remember to write. The type model and the versioning
philosophy reinforce rather than duplicate each other.

## Rejected alternatives

| Approach                                         | Why not                                                                                               |
| ------------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| Carry the admission verdict on the skill         | Stale on rule-tightening; defeats self-healing. Admission is a pure function of identity — recompute. |
| Product `Skill<Validated, Selected>`             | Two phantom params everywhere, mostly-dead corners; noisy for no precision gain.                      |
| Sum / or-type for "validated or decided"         | A union where a sealed trait bound (`Vetted`) is the idiomatic, open answer.                          |
| `validate` annotates instead of partitions       | Leaves inadmissible skills in `Skills<Validated>`; `persist` can't trust the type. Breaks the gate.   |
| `ImportDecided` typestate for Compose            | No per-atom consumer of the payload; verdict is aggregate diagnostics + a path list. Marker with no job. |
| Re-check admission at emit (belt-and-suspenders) | Redundant: per-run re-validate + sealed `Vetted` lineage already guarantee it.                        |

## Open / downstream

- **Fork 2 — `MatchHandle`: cut. Resolved (2026-06-05).** Once `Locator` lands the identity
  side of the handle/identity boundary is already typed (`pattern_matches(&str, &Locator)`),
  so the newtype's justification collapses — and a pattern is selection *input*, never a
  `Skill<S>` state. Replaced by: (1) moving the glob-validation that today runs only inside
  the dead `MatchHandle::new` to the resolver seam as a **warn-diagnostic** (echo verbatim,
  beside the unknown-pattern warning) so `resolve` stays infallible — not a hard error;
  (2) expressing the pattern-`&str` vs `Locator` separation structurally in signatures, with
  no new pattern newtype unless the seam later proves leaky.
- **Fork 3 — package placement: dissolve `src/resolver/`. Resolved (2026-06-05).** Rule:
  resolution lives with the typed data it reads and stamps. Skill resolution stamps
  `Skill<S>` → moves to `src/skills/resolve/` (this removes the stringly `Carry`/`by_name`
  round-trip — the seam ceases to exist, not merely de-stringifies). Config-merge reads only
  `Tree` / `AceToml` → moves to `src/config/resolve/`; `Source` / `Sourced` go with it.
  Recorded as [resolver dissolution](2026-06-05-resolver-dissolution.md), superseding
  decision 007's module layout. 007's load-bearing constraint (`merge` stays infallible and
  skills-free so `ace config show` survives without a school clone) is preserved.
- **Fork 4 — naming. Resolved (2026-06-05).** Markers `Discovered → Validated → Decided`; the
  `Vetted` trait name stands. Identity is a typed **`Locator`** newtype (field `locator`),
  carried end-to-end, replacing `SkillId` / `id`; the `Skill.name` field is **dropped** —
  callsites resolve to `locator` (the path-identity) or `frontmatter_name` (display-only).
  Why: `Decided` mirrors its payload (`decision`) where `Resolved` would only name the action;
  `Locator` matches the entrenched "identity" vocabulary while sidestepping a clash with the
  filesystem `path` field; dropping `name` forces every callsite to declare which concept it
  meant, and aligns with name = `basename(identity)`.

## References

- [the rearchitect note](../scratch/2026-06-02-skill-model-rearchitect.md) — defect catalogue +
  the four forks.
- Partially supersedes [name admission policy](2026-05-30-skill-name-admission-policy.md)
  (in-memory mechanism only).
- Builds on [name = path](2026-06-01-skill-name-is-path.md) (identity is the spine; admission
  keys on identity) and
  [discovery & identity](2026-05-26-skill-discovery-identity-storage.md).
- Companion: [admission eviction is non-overridable](2026-06-04-admission-eviction-non-overridable.md).
- Specs (`model.md`, `selection.md`, `emit.md`, `sync.md`) updated 2026-06-05 — partition
  framing, the vetted-gate type-safety invariant, and the eviction-visibility surfaces.
