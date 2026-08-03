# Skill Lifecycle — Typestate Contract

The skill model is a `(collection, action)` algebra over a three-state marker lattice
**`Discovered → Validated → Decided`**. A typestate marker proves **an action ran in
this process's call graph** — it stores no verdict and persists nothing.
`Skill<Validated>` is a compile-time token meaning "`validate()` sat on the path that
produced this value": rebuilt from scratch every process, proving only call-graph
ordering. ACE's no-version philosophy constrains *persistence*, not type-level proof of
in-process ordering — nothing here writes a verdict to disk, so the marker re-runs
freely and can never go stale.

Companion to [model.md](model.md) (what a skill IS), [selection.md](selection.md)
(which skills are picked), and [emit.md](emit.md) (where they land).

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
Compose-side `import-resolve` + `merge` + `copy_into`. Each marker's job — a marker
with no job gets cut:

| Marker       | Carries                                 | Job — the gate it powers                                                                                                    |
| ------------ | --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `Discovered` | identity + intrinsic facts + provenance | *Negative capability*: not `Vetted`, not `Decided`, so the compiler forbids persisting or emitting it. Input to `validate`. |
| `Validated`  | nothing (unit marker)                   | Only constructible via `validate`. `impl Vetted` → gates `persist`/`copy_into`. Precondition for the resolvers.             |
| `Decided`    | per-atom decision + trace               | `impl Vetted`. Gates `emit`. Carries the trace `ace explain` / `ace skills` read per skill.                                 |

## The `Vetted` trait — the "or" as a bound, not a union

```rust
trait Vetted {}                  // sealed: external code cannot impl it
impl Vetted for Validated {}
impl Vetted for Decided {}
// Discovered deliberately does NOT impl Vetted

fn persist<S: Vetted>(skills: &Skills<S>) -> io::Result<()>;   // no un-vetted skill to disk
fn emit(skills: &Skills<Decided>) -> Vec<DesiredLink>;          // only resolved skills emit
```

`Vetted` *is* "any state at or past `validate`" — without enumerating a union or
multiplying type parameters (a 2-D product `Skill<V, S>` would carry mostly-dead
corners). Sealing it keeps `Discovered` (and any future pre-validate state)
permanently out.

## `validate` partitions; it does not annotate

For `Vetted` to *mean* "contains only admissible skills," `validate` **removes** the
inadmissible ones, not tags them:

```rust
fn validate(discovered: Skills<Discovered>) -> (Skills<Validated>, Vec<Rejected>);
```

The `Rejected` half carries each rejection reason and feeds warnings / doctor — rejects
are an explicit second output, never inadmissible skills lurking in the main collection
wearing a status flag. An annotate-and-keep `validate` would leave inadmissible skills
inside `Skills<Validated>`, so `persist` could not trust the type and would have to
re-filter — which defeats the gate. Partition is *forced* the moment `Vetted` must
guarantee the safety property rather than merely assert it.

## Metadata placement

| Data                                                 | Home                                         | Why                                                                                                |
| ---------------------------------------------------- | -------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| identity, tier, `internal`, display name             | struct fields on `Skill<S>`, every stage     | intrinsic facts, present from discovery on                                                          |
| provenance (own vs import source)                    | struct field on the atom (Discovered onward) | metadata some actions attach (`import`) and others ignore (`emit`); dead weight in Provision, fine  |
| admission verdict                                    | **nowhere — recomputed on demand**           | a cheap pure function of identity; carrying it risks staleness and buys nothing                     |
| selection decision + trace                           | the `Decided` marker payload (per atom)      | genuine new state: config-derived, set-relative, expensive; `ace explain` reads it per skill        |
| set-level diagnostics (unknown patterns, collisions) | the `Skills<Decided>` collection             | no per-atom home; produced by the resolution run                                                    |

**Admission is never carried.** It is a derived predicate over identity, recomputed
wherever needed — like `leaf()` or `is_nested()`. Identity is the only durable state;
admissibility falls out of it. This is both versioning-correct (nothing to go stale)
and the honest shape (admission needs no config and no set context, unlike selection).

The marker lives **on the atom** for `Decided` (decision + trace is real per-skill
payload — a type fact, not an `Option`), and the collection rides the same parameter so
`Skills<Decided>` can own the set-level diagnostics. `Validated` carries no atom
payload — its proof is *set membership* (passed the partition), which is
collection-level by construction.

### The accepted asymmetry

A lone `Skill` plucked from a `Skills<Validated>` via `find()` is just `Skill` — it
loses the proof. Validation lives on the *set*; selection's payload lives on the
*atom*. A function needing a single vetted skill takes the collection or re-checks.
This is the one real give-up, and it's honest: a marker carries payload only when there
is payload, and validation's "payload" is membership, not a per-skill verdict.

## Compose stays value-level past `Validated`

**Principle: a typestate is earned only where something downstream reads the carried
payload *per atom*.** Provision's decision + trace feeds `ace explain` one skill at a
time → earns `Decided`. Compose's import resolution feeds **aggregate** collision
warnings plus a **path list** for `copy_into` → no per-atom carrier, so it stays
value-level (`ResolvedImport` records as diagnostics). There is no `ImportDecided`
typestate. Import resolution consumes `Skills<Validated>`, emits warnings + paths, and
`copy_into` re-enters the lattice through the `Vetted` gate.

## Versioning and typestate compose with zero redundancy

Because `validate` re-partitions from scratch every process, versioning-safety
(self-healing on upgrade) is satisfied at the `validate` step. Within a run, a
`Decided` skill is admissible *by lineage* (`Decided: Vetted`, and the sealed trait
means nothing fabricates a `Decided` that skipped `validate`). So **emit needs no
admission re-check** — the safety property falls out of construction, not a guard
someone must remember to write. The type model and the versioning philosophy reinforce
rather than duplicate each other.
