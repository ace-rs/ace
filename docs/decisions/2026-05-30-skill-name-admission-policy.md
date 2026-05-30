# Skill Name Admission & Sanitization Policy

- **Date:** 2026-05-30
- **PR:** manual
- **Status:** accepted

> **Supersedes** the *Sanitization (Q9)* section of
> [`2026-05-26-skill-emit-and-match.md`](2026-05-26-skill-emit-and-match.md) and
> corrects the corrupted *§ Approach* / boundary table in
> [`../spec/skills/model.md`](../spec/skills/model.md). The whitelist intent in the
> 2026-05-26 ruling stands; the spec's later "aspirational denylist" rewrite was an
> error and is reverted. This entry additionally moves the *gate* from emit to
> discovery and reframes sanitization as admission.

## Decision

Skill **names are admitted or rejected** by a predicate at **discovery** — the
universal chokepoint every skill-touching command runs through. Rejected skills are
**excluded from the in-memory model + warned**, never mutated and never deleted from
disk. Sanitization-as-character-mutation survives in exactly one place: **ACE's own
terminal rendering**. ACE does not sanitize backend output, because ACE does not
produce backend output — it symlinks.

## Rationale

### Why not emit-time name-sanitization (the thing being replaced)

The prior model sanitized the link name at backend-emit time. Three defects:

1. **Conflates two operations.** "Strip bad chars so a string is safe to *render*"
   (a transform, output: a different string) is not "decide whether a skill is
   *allowed*" (a predicate, output: yes/no). Emit-time sanitization implemented the
   predicate as a transform, at the last inch of the pipeline.
2. **Not version-safe.** The malicious skill stays **resident in the school**. Safety
   depends on every consumer's ACE version re-applying a mutation on every link. A
   rule tightened in a later version does not reliably evict a skill an older version
   already materialized. This is the concrete failure that motivated this decision.
3. **Identity drift.** Mutating the emitted name desyncs it from the stored identity.

### Why discovery is the gate of record

`discover_skills(school_root)` runs on **every** operation that touches skills —
`ace import`, `ace school pull-imports`, `ace setup`, `ace pull`, `ace link`,
`ace skills`. Making admission a predicate *there* means:

- A bad skill in **any** school — ACE-authored or not, fresh or years old, honest or
  malicious — is re-evaluated against the **current** ACE version's rules **every
  time** it is discovered.
- Tighten the rule, and the next `ace setup` / `ace pull` re-scans the cached school
  and now excludes what the old version admitted; the stale symlink becomes an orphan
  and reconcile removes it. **Self-healing on upgrade.**
- "Check at import" and "check the cloned school at setup" are not two mechanisms —
  both are discovery running. Setup-time coverage comes for free and is the
  load-bearing boundary, because schools cannot be assumed to have been authored
  through ACE.

### Boundary model

| Boundary                        | Operation                                  | Why                                                                 |
| ------------------------------- | ------------------------------------------ | ------------------------------------------------------------------- |
| **Discovery** (every op)        | Admit-predicate: exclude bad name + warn   | Gate of record. Version-current, universal, self-healing.           |
| **Import** (`import`/`pull-imports`) | *Additionally* hard-refuse: don't copy in | Keeps ACE-authored schools clean *at rest*. Not load-bearing alone. |
| **Terminal display**            | Transform (whitelist)                      | The only place mutation is correct — ACE rendering to its terminal. |
| **Emit / symlink name**         | Structural only (traversal / NUL / length) | ACE's own filesystem safety. Not a content gate.                    |
| **Backend file content**        | Nothing — symlink, verbatim                | Out of scope. Backends protect themselves.                          |

### ACE does not write backend frontmatter

The replaced boundary table had a row "Backend emit write → sanitize into written
frontmatter." **That operation does not exist.** Emit is a symlink
(`link_skills.rs` `create_dir_symlink`) into the school clone; the emitted SKILL.md
*is* the school's original, byte-for-byte. `for_emit`'s only real job was deriving the
symlink filename. Protecting the backend's terminal from its own skill files is the
backend's responsibility — and is consistent with the frontmatter-passthrough ruling
("pass all frontmatter verbatim, ACE does not intervene").

### Exclude, don't delete

A rejected skill in a cloned school stays on disk — it is a git checkout, and deleting
fights the next `git pull`. Discovery refuses to *admit* it into the `Skills` set, with
a warning. This also preserves the "internal model sees violations so doctor can
report them" invariant: model rejection as a decision with a reason, not a silent
vanish.

### Supply-chain stance

Skills are a primary AI-era supply-chain attack surface: third-party instruction text
the backend LLM reads and acts on. ACE chooses to be strict at the authoring boundary
(import hard-refuse) *and* defensive at every consumer boundary (discovery admission),
rather than tolerant-with-a-warning. Pioneering a real admission gate here is
deliberate, not over-engineering.

## The predicate

The admission ruleset is the Unicode-class whitelist from the 2026-05-26 ruling, used
as a **predicate** (not a transform): a name is admissible iff every character is in
`L*` (letters), `M*` (marks), `N*` (numbers), `P*` (punctuation), `S*` (symbols), or
`Zs` (space) — i.e. nothing in `C*` (control, incl. `Cf` format and the bidi-override
block, `Cn` unassigned, `Co` private-use) — **and** it is a structurally valid path
component (no traversal, no `NUL`, within the length cap). Requires the
`unicode-general-category` crate; the spec's "this needs a dependency for ~0 safety"
claim was the corruption (`Cn`/`Co` *are* valid Rust `char`s and leak through the
denylist today).

The same whitelist, applied as a **transform**, is what `for_terminal` uses to render
untrusted text (rejected-skill warnings, descriptions, any non-name frontmatter ACE
displays).

## Open (in active discussion — not yet settled)

- **G — identity-segment supersession.** The 2026-05-26 *"Path components from foreign
  repos: import as-is, warn"* rule tolerated bad chars in identity segments ("can't
  rename without breaking refs"). The admission gate **rejects** such skills instead.
  Confirm we supersede that tolerance rather than carve an exception for multi-segment
  identities.
- **B — import hard-refuse granularity.** Skip-the-offending-skill + loud warning
  (import the rest) vs fail-the-whole-operation. Leaning skip-the-skill.
- **D — `SanitizedString` newtype scope.** With emit no longer sanitizing, the newtype
  guards only display output. Still in this pass, or split out once the admission gate
  lands?
- **F — emit structural backstop.** Does `unsafe_flatten_name` stay at emit as
  defense-in-depth, or does it move wholesale into the discovery predicate and emit
  trusts admitted names? Leaning: predicate owns the rules; emit keeps a thin
  structural backstop.
- **Replacement char for the display transform** — drop (empty) vs `U+FFFD`. Current
  code drops; carried over unless changed.

## Out of scope

- Backend file-content sanitization — symlink, verbatim, backend's problem.
- Frontmatter translation / stripping between backends — rejected per `index.md`.
- Skill *body* scanning (prompt-injection payloads, `curl | sh`) and the LLM-audit
  command — separate facets of PROD9-226, not this decision.
