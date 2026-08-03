# Skill Name Admission & Sanitization Policy

- **Date:** 2026-05-30
- **PR:** manual
- **Status:** accepted

> **Partially superseded (2026-06-01)** by
> [name = path](2026-06-01-skill-name-is-path.md): admission now keys on the **identity
> path only**. The "verdict over identity *plus frontmatter name*" below is narrowed —
> frontmatter `name` is the backend's domain (verbatim passthrough), neutralized only by
> the display transform, never an admission axis. Everything else here stands.

> **Partially superseded (2026-06-04)** by
> [the lifecycle spec](../spec/skills/lifecycle.md): the in-memory
> mechanism in *Abstraction Boundaries* — selection running over everything with
> `rejected()` as a *view* on one collection — is replaced by a `validate` **partition**
> that splits admissible from rejected before selection. The admission *predicate*, its
> discovery-gate placement, and the "exclude, don't delete" stance are unchanged; only the
> in-memory representation moves (partition vs. view).

> **Supersedes** the *Sanitization (Q9)* section of
> the emit & match decision (folded into `docs/spec/skills/emit.md`) and corrects
> the corrupted *§ Approach* / boundary table in
> [`../spec/skills/model.md`](../spec/skills/model.md). The whitelist intent in the
> 2026-05-26 ruling stands; the spec's later "aspirational denylist" rewrite was an error
> and is reverted. This entry additionally moves the *gate* from emit to discovery and
> reframes sanitization as admission.

## Decision

Skill **names are admitted or rejected** by a predicate at **discovery** — the universal
chokepoint every skill-touching command runs through. Rejected skills are **excluded from
the in-memory model + warned**, never mutated and never deleted from disk.
Sanitization-as-character-mutation survives in exactly one place: **ACE's own terminal
rendering**. ACE does not sanitize backend output, because ACE does not produce backend
output — it symlinks.

## Rationale

### Why not emit-time name-sanitization (the thing being replaced)

The prior model sanitized the link name at backend-emit time. Three defects:

1. **Conflates two operations.** "Strip bad chars so a string is safe to *render*" (a
   transform, output: a different string) is not "decide whether a skill is *allowed*" (a
   predicate, output: yes/no). Emit-time sanitization implemented the predicate as a
   transform, at the last inch of the pipeline.
2. **Not version-safe.** The malicious skill stays **resident in the school**. Safety
   depends on every consumer's ACE version re-applying a mutation on every link. A rule
   tightened in a later version does not reliably evict a skill an older version already
   materialized. This is the concrete failure that motivated this decision.
3. **Identity drift.** Mutating the emitted name desyncs it from the stored identity.

### Why discovery is the gate of record

`discover_skills(school_root)` runs on **every** operation that touches skills —
`ace import`, `ace school pull`, `ace setup`, `ace pull`, `ace link`, `ace skills`. Making
admission a predicate *there* means:

- A bad skill in **any** school — ACE-authored or not, fresh or years old, honest or
  malicious — is re-evaluated against the **current** ACE version's rules **every time**
  it is discovered.
- Tighten the rule, and the next `ace setup` / `ace pull` re-scans the cached school and
  now excludes what the old version admitted; the stale symlink becomes an orphan and
  reconcile removes it. **Self-healing on upgrade.**
- "Check at import" and "check the cloned school at setup" are not two mechanisms — both
  are discovery running. Setup-time coverage comes for free and is the load-bearing
  boundary, because schools cannot be assumed to have been authored through ACE.

### Boundary model

The per-boundary table (discovery admit-predicate, import hard-refuse, terminal
transform, emit structural checks, backend verbatim) lives in
[model.md § Boundary policy](../spec/skills/model.md); this decision established it.

### ACE does not write backend frontmatter

The replaced boundary table had a row "Backend emit write → sanitize into written
frontmatter." **That operation does not exist.** Emit is a symlink (`link_skills.rs`
`create_dir_symlink`) into the school clone; the emitted SKILL.md *is* the school's
original, byte-for-byte. The old emit helper's only real job was deriving the symlink
filename. Protecting the backend's terminal from its own skill files is the backend's
responsibility — and is consistent with the frontmatter-passthrough ruling ("pass all
frontmatter verbatim, ACE does not intervene").

### Exclude, don't delete

A rejected skill in a cloned school stays on disk — it is a git checkout, and deleting
fights the next `git pull`. Discovery refuses to *admit* it into the `Skills` set, with a
warning. This also preserves the "internal model sees violations so doctor can report
them" invariant: model rejection as a decision with a reason, not a silent vanish.

### Supply-chain stance

Skills are a primary AI-era supply-chain attack surface: third-party instruction text the
backend LLM reads and acts on. ACE chooses to be strict at the authoring boundary (import
hard-refuse) *and* defensive at every consumer boundary (discovery admission), rather than
tolerant-with-a-warning. Pioneering a real admission gate here is deliberate, not
over-engineering.

## The predicate

The Unicode-class whitelist predicate itself is specced in
[model.md § Name Admission](../spec/skills/model.md). The load-bearing choices ruled
here: it is a **predicate**, never a transform (except at terminal display, where the
same whitelist renders untrusted text via `skills::name::render`), and the
implementation is a one-shot generated Unicode table committed to source — no
build-time or runtime dependency.

## Resolved Follow-Ups

- **Identity segments are not exempt.** The 2026-05-26 *"Path components from foreign
  repos: import as-is, warn"* tolerance is superseded. A disallowed character in any
  identity segment rejects the whole skill (excluded + warned).
- **Import hard-refuses per skill.** `ace import` and `ace school pull` skip the offending
  skill and warn loudly, while continuing with other admissible skills. Both exit non-zero
  with the same code when any selected skill was skipped. For `ace school pull` this lets CI
  catch dirty imports; `ace import` matches mainly for consistency (no CI use case is
  expected for it).
- **Emit keeps a structural backstop.** Discovery owns the whitelist and full
  admissibility predicate. Emit still calls the shared structural helper before creating
  symlinks, so traversal / dotfile / length mistakes fail closed at the filesystem edge.
- **`SanitizedString` lands in this pass.** It is a bounded display-boundary type, built
  only through explicit rendering. Internal model fields remain raw for diagnostics and
  doctor-style checks.
- **Display replacement uses `U+FFFD`.** Rendering is idempotent and replaces each
  disallowed character with the replacement character. Rejection warnings additionally
  name the offending codepoint and position for forensics.

## Abstraction Boundaries

- **`skills::name` owns name safety.** It absorbs the old `skills::sanitize` module and
  contains the character predicate, display transform, structural validation, composite
  admissibility, `SanitizedString`, `RejectReason`, and the generated Unicode table.
- **Admission is classification, not construction.** `SkillId` may continue to hold raw
  identities so rejected skills can be represented and reported. Admissibility is a
  separate verdict over identity plus frontmatter name.
- **Admission is orthogonal to selection, settled at discovery.** The verdict is derived
  once at the discovery boundary (`DiscoveredSkill::admission`) and carried on the skill,
  not folded into the resolver. The project resolver's `Decision` stays pure selection
  (`Included`/`Excluded`) — there is no `Decision::Rejected` variant. `included()` /
  `excluded()` require admissibility; a separate `rejected()` view derives from the
  admission verdict and exposes inadmissible skills for warnings and diagnostics. This
  keeps the `skills/` layer from reaching into and overwriting the resolver's verdict.
- **Display enforcement is bounded.** Untrusted skill frontmatter display accessors and
  rejection diagnostics return `SanitizedString`. `Ace::warn` / `hint` / `error` keep
  `&str` because developer literals are trusted.
- **`SkillId` migration is deferred.** `Skill.name: String -> SkillId` remains adjacent
  work. This policy is implemented over `&str` so the migration can happen later without
  changing the admission model.

## Out of scope

- Backend file-content sanitization — symlink, verbatim, backend's problem.
- Frontmatter translation / stripping between backends — rejected per `index.md`.
- Skill *body* scanning (prompt-injection payloads, `curl | sh`) and the LLM-audit command
  — separate facets of PROD9-226, not this decision.
