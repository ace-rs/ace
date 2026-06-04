# Skill lifecycle: typestate proves actions, gated by a `Vetted` trait

- **Date:** 2026-06-04
- **PR:** manual
- **Status:** accepted

## Decision

The skill model is a `(collection, action)` algebra over a three-state marker lattice
**`Raw → Validated → Judged`**. A typestate marker proves **an action ran in this
process's call graph** — it stores no verdict and persists nothing. `validate` is a
**partition** (`Skills<Raw> → (Skills<Validated>, Vec<Rejected>)`), and a sealed `Vetted`
capability trait — implemented by every state at or past `Validated`, never by `Raw` —
gates the boundaries that must not see an un-vetted skill (disk write, backend emit).

This supersedes the *in-memory mechanism* of
[name admission policy](2026-05-30-skill-name-admission-policy.md) (select-over-everything
+ `rejected()` as a view); the admission predicate and its discovery-gate placement are
unchanged. Resolves fork 1 (metadata placement) of
[the rearchitect note](../notes/2026-06-02-skill-model-rearchitect.md); forks 2–4
(MatchHandle, packages, naming) stay open.

## Context

A type-audit (note above) found the skills subsystem running on raw `String` while a typed
layer (`SkillId`, `MatchHandle`) floated unused — `SkillId` minted by discovery and
discarded one hop later (`name: d.id.to_string()`), `MatchHandle` 100% dead, both resolvers
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
 discover            validate                  resolve
   ∅ ─▶ Skills<Raw> ─▶ Skills<Validated> ─▶ Skills<Judged> ─▶ emit ─▶ links
                       │  (+ Vec<Rejected>)          ▲
                       └── import-resolve ─┐         │   persist<S: Vetted>
                           (value-level)   ▼         │
                                       copy_into ────┘
```

Actions: `discover` (∅ → Raw), `validate` (Raw → Validated, splitting off Rejected),
`resolve` (Validated → Judged), `emit` (Judged.included → links), and the Compose-side
`import-resolve` + `merge` + `copy_into`. Each marker's job — a marker with no job gets cut:

| Marker      | Carries                                  | Job — the gate it powers                                                                                  |
| ----------- | ---------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| `Raw`       | identity + intrinsic facts + provenance  | *Negative capability*: not `Vetted`, not `Judged`, so the compiler forbids persisting or emitting it. Input to `validate`. |
| `Validated` | nothing (unit marker)                    | Only constructible via `validate`. `impl Vetted` → gates `persist`/`copy_into`. Precondition for the resolvers. |
| `Judged`    | per-atom decision + trace                | `impl Vetted`. Gates `emit`. Carries the trace `ace explain` / `ace skills` read per skill.                |

## The `Vetted` trait — the "or" as a bound, not a union

A naïve encoding of two proof axes (validated, selected) as independent phantom params
forces a 2-D product `Skill<V, S>` with mostly-dead corners, or a sum type for functions
that accept "validated OR judged." Both are wrong. The functional answer is a **sealed
capability trait**:

```rust
trait Vetted {}                  // sealed: external code cannot impl it
impl Vetted for Validated {}
impl Vetted for Judged {}
// Raw deliberately does NOT impl Vetted

fn persist<S: Vetted>(skills: &Skills<S>) -> io::Result<()>;   // no un-vetted skill to disk
fn emit(skills: &Skills<Judged>) -> Vec<DesiredLink>;          // only resolved skills emit
```

`Vetted` *is* the "or" — "any state at or past `validate`" — without enumerating a union or
multiplying type parameters. Markers are nodes; the trait is a gate that accepts a family of
nodes. Sealing it keeps `Raw` (and any future pre-validate state) permanently out.

## `validate` partitions; it does not annotate

For `Vetted` to *mean* "contains only admissible skills," `validate` must **remove** the
inadmissible ones, not tag them:

```rust
fn validate(raw: Skills<Raw>) -> (Skills<Validated>, Vec<Rejected>);
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

| Data                                                | Home                                       | Why                                                                                          |
| --------------------------------------------------- | ------------------------------------------ | -------------------------------------------------------------------------------------------- |
| identity, tier, `internal`, display name            | struct fields on `Skill<S>`, every stage   | intrinsic facts, present from discovery on                                                    |
| provenance (own vs import source)                   | struct field on the atom (Raw onward)      | metadata some actions attach (`import`) and others ignore (`emit`); dead weight in Provision, fine |
| admission verdict                                   | **nowhere — recomputed on demand**         | a cheap pure function of identity; carrying it risks staleness and buys nothing               |
| selection decision + trace                          | the `Judged` marker payload (per atom)     | genuine new state: config-derived, set-relative, expensive; `ace explain` reads it per skill   |
| set-level diagnostics (unknown patterns, collisions)| the `Skills<Judged>` collection            | no per-atom home; produced by the resolution run                                              |

**Admission is never carried.** It is a derived predicate over identity, recomputed wherever
needed — like `leaf()` or `is_nested()`. Identity is the only durable state; admissibility
falls out of it. This is both versioning-correct (nothing to go stale) and the honest shape
(admission needs no config and no set context, unlike selection).

The marker therefore lives **on the atom** for `Judged` (decision + trace is real per-skill
payload, so `Skill<Raw>` has no decision and `Skill<Judged>` has one — a type fact, not an
`Option`), and the collection rides the same parameter so `Skills<Judged>` can own the
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
`Judged`. Compose's import resolution feeds **aggregate** collision warnings plus a **path
list** for `copy_into` → no per-atom carrier, so it stays value-level (`ResolvedImport`
records as diagnostics). There is no `ImportJudged` typestate. Import resolution consumes
`Skills<Validated>`, emits warnings + paths, and `copy_into` re-enters the lattice through
the `Vetted` gate. One fewer marker, justified by the principle rather than convenience.

## Versioning and typestate compose with zero redundancy

Because `validate` re-partitions from scratch every process, versioning-safety (self-healing
on upgrade) is satisfied at the `validate` step. Within a run, a `Judged` skill is admissible
*by lineage* (`Judged: Vetted`, and the sealed trait means nothing fabricates a `Judged` that
skipped `validate`). So **emit needs no admission re-check** — the safety property falls out
of construction, not a guard we must remember to write. The type model and the versioning
philosophy reinforce rather than duplicate each other.

## Rejected alternatives

| Approach                                          | Why not                                                                                          |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Carry the admission verdict on the skill          | Stale on rule-tightening; defeats self-healing. Admission is a pure function of identity — recompute. |
| Product `Skill<Validated, Selected>`              | Two phantom params everywhere, mostly-dead corners; noisy for no precision gain.                  |
| Sum / or-type for "validated or judged"           | A union where a sealed trait bound (`Vetted`) is the idiomatic, open answer.                       |
| `validate` annotates instead of partitions        | Leaves inadmissible skills in `Skills<Validated>`; `persist` can't trust the type. Breaks the gate. |
| `ImportJudged` typestate for Compose              | No per-atom consumer of the payload; verdict is aggregate diagnostics + a path list. Marker with no job. |
| Re-check admission at emit (belt-and-suspenders)  | Redundant: per-run re-validate + sealed `Vetted` lineage already guarantee it.                    |

## Open / downstream

- **Fork 2 — `MatchHandle` keep/cut.** `select` is where user patterns meet identities; the
  lattice implies a typed handle-vs-identity boundary there, but whether to thread
  `MatchHandle` config→resolver or delete it for raw `pattern_matches` is a separate impl
  decision.
- **Fork 3 — package placement.** Who owns atom / collection / resolver across `skills/` and
  `resolver/`. The de-stringified seam (resolver returns verdicts keyed by identity,
  `Skills::resolve` stamps the atoms) collapses the current `Carry`/`by_name` round-trip, but
  module relocation is undecided.
- **Fork 4 — naming.** Marker names `Raw` / `Validated` / `Judged` and the `Vetted` trait are
  **provisional**; the path-identity rename (away from `SkillId`/`id`) is unsettled.

## References

- [the rearchitect note](../notes/2026-06-02-skill-model-rearchitect.md) — defect catalogue +
  the four forks.
- Partially supersedes [name admission policy](2026-05-30-skill-name-admission-policy.md)
  (in-memory mechanism only).
- Builds on [name = path](2026-06-01-skill-name-is-path.md) (identity is the spine; admission
  keys on identity) and
  [discovery & identity](2026-05-26-skill-discovery-identity-storage.md).
- Companion: [admission eviction is non-overridable](2026-06-04-admission-eviction-non-overridable.md).
- Specs (`model.md`, `selection.md`, `emit.md`) updated in the following step.
