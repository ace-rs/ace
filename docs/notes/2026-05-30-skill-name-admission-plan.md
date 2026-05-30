# Skill-Name Admission — Session Notes & Implementation Plan (2026-05-30)

**Status:** planning complete, **no code written**. Next session: spec review →
fold resolutions into the decision doc → implement. This file is the handoff.

## Pointers

- **Decision (architecture):** `docs/decisions/2026-05-30-skill-name-admission-policy.md`
  — discovery = gate of record, exclude-don't-delete, import hard-refuses, emit
  structural-only, display whitelist-transform. Its **Open section is resolved by
  this file** (fold in next session, task A1 below).
- **Superseded:** `docs/decisions/2026-05-26-skill-emit-and-match.md` § Sanitization
  (banner added).
- **Spec to correct:** `docs/spec/skills/model.md` § Approach + boundary table
  (corrupted denylist framing — task A2 below).
- **Tracking:** PROD9-226 (this is its "static detection, ≥1 class blocks by
  default" facet — comment posted). No separate ticket per owner.
- **Shared coding principle shipped:** `prod9/school` PR #56 — `general-coding`
  "Trust Boundaries" section (whitelist / fail-closed). Owner: school refines wording.
- **Memory:** `feedback-whitelist-failclosed` (the recurring denylist-drift failure
  mode — read before re-opening the whitelist question).

## The whitelist (settled, do not relitigate)

A skill name is admissible iff **every character is in `L/M/N/P/S/Zs`** (deny all
`C*` incl. `Cf`/bidi/`Cn`-unassigned/`Co`, plus `Zl`/`Zp`) **and** it is a
structurally valid path component. Whitelist / default-deny / **fail-closed** is
the policy. The earlier "category-denylist is equivalent" detour was retracted —
it fails open on unknown-future chars; implementation economy never justifies that
flip. Rejecting unassigned (`Cn`) is a free, correct consequence of the whitelist,
not a separate concern.

## 1-by-1 resolutions (the six open items)

| # | Item | Decision |
| - | ---- | -------- |
| G | identity-segment supersession | **Supersede** the "import-as-is + warn" tolerance. Disallowed char in *any* identity segment ⇒ reject the whole skill (exclude + warn). |
| B | import refuse granularity | **Skip-the-skill + loud warning** for both `ace import` and `ace school pull-imports`. Exit codes: `ace import` unchanged; **`ace school pull-imports` exits non-zero when a skill was skipped** (CI-visible). |
| F | emit structural backstop | **Keep** the emit structural check. Factor structural validity into **one shared helper** called by both discovery-admission and emit. Whitelist (char-class) stays discovery-only. |
| D | `SanitizedString` newtype | **In this pass.** Display boundaries take `SanitizedString` via explicit conversion; internal model holds raw. Bounded enforcement (see abstraction #4), not codebase-wide. |
| 5 | replacement char | Display transform emits **`U+FFFD`** (idempotent, `So`-class). Rejection warnings **additionally name exact offending codepoint(s) + position** for forensics. |
| 6 | GC data source | **2b — `ucd-generate` one-shot → committed `tables.rs`**, zero build/runtime deps. (Owner trusts BurntSushi.) |

## Abstraction-boundary decisions

Grounded in the existing code: `skills::identity` already has `SkillId` +
`MatchHandle` newtypes (in-flight typed-identity effort, tied to the same
`model.md` § Type-safety invariant). `Skill.name` is still `String` (migration
incomplete). `Decision` (`resolver/project.rs`) = `Included`/`Excluded`.
`resolve()` in `skills/mod.rs` is the one place identity + `frontmatter_name` +
decision coexist. Name logic is scattered across `skills::sanitize`,
`link_skills::unsafe_flatten_name`, `link_skills::build_desired`.

1. **Consolidate into `skills::name`** (absorbs `sanitize.rs`). Single owner of
   "valid/safe skill name": char predicate + display transform, structural
   validity, composite admissibility, `SanitizedString`, `RejectReason`, committed
   `ucd` table. `unsafe_flatten_name` moves here (decision-F shared helper).
   Internal rename — no backcompat obligation.
2. **Admission = classification, not a construction gate.** `SkillId` stays able
   to hold a raw identity (rejected skills must remain in the model for doctor).
   Admissibility is a separate verdict over the name.
3. **Model rejection as `Decision::Rejected { reason }`** — one enum, three
   terminal states; `included()` excludes it for free; add `rejected()`. Assigned
   in `resolve()` when identity-OR-`frontmatter_name` is inadmissible (overrides
   config). Backend-independent, as "discovery is the gate" requires. Folds in G.
4. **`SanitizedString` bounded enforcement.** Rejected `Untrusted<T>` everywhere
   (blast radius). Chosen: `SanitizedString` (sibling of `SkillId`, `Display`,
   built only via `render()` = the U+FFFD transform). Teeth: untrusted frontmatter
   accessors (`SkillMeta` name/description for `ace skills`; `Skill.name`/
   `frontmatter_name` forensic display) return `SanitizedString`; raw retained for
   internal/doctor. `Ace::warn/hint/error` keep `&str` (dev literals trusted —
   forcing the type there is the discipline-not-enforcement trap). `RejectReason`'s
   `Display` sanitizes offending names internally.
5. **Defer the `SkillId` adoption** (`Skill.name: String → SkillId`). Adjacent
   in-flight scope; admissibility works on `&str`. Design stays consistent so it
   can land later. **(Judgment call — confirm next session.)**

## Plan

### Phase A — lock the design (docs only)

- **A1.** Resolve the Open section of the 2026-05-30 decision doc with the six
  resolutions above + an "Abstraction boundaries" subsection (decisions 1–5).
- **A2.** Correct `docs/spec/skills/model.md`: whitelist framing restored;
  boundary table rewritten (drop fictional "sanitize into written frontmatter"
  row; discovery = admit-predicate; display = transform; emit = structural-only;
  backend content = out of scope); supersede identity-segment tolerance.

### Phase B — implement (TDD, sliced; red→green→refactor→commit each)

1. **`skills::name` foundation.** `ucd-generate` → commit
   `src/skills/name/ucd_tables.rs` (regen command in header). `char_allowed(c)`;
   `SanitizedString` + `render()` (U+FFFD). Port `sanitize.rs` tests; delete it.
2. **Structural helper (F).** `structural_ok(name) -> Result<(), RejectReason>`
   (traversal, `.`/`..`, leading-dot, slash, backslash, length, **+NUL**). Emit
   calls it as backstop; delete `unsafe_flatten_name`.
3. **Admissibility (5).** `admissibility(identity, frontmatter_name)` = char-check
   over every identity segment + `frontmatter_name`, then `structural_ok`.
   `RejectReason` carries rule + `U+XXXX` + position; `Display` sanitizes.
4. **`Decision::Rejected` + resolve (G).** Add variant; assign in `resolve()`;
   add `Skills::rejected()`; route reasons through `emit_warnings`.
5. **`SanitizedString` enforcement (D).** Untrusted accessors → `SanitizedString`;
   update `ace skills` listing (`cmd/school.rs`, `list_skills`), `explain_skill`,
   warning sites.
6. **Import hard-refuse (B).** Filter inadmissible before `copy_into`; warn
   per-skill; `ace school pull-imports` exits non-zero on skip; `ace import`
   exit unchanged.
7. **Cleanup + audit.** Remove dead `for_emit`; emit derives name then calls
   `structural_ok`; re-read all changed files vs decision doc + general-coding;
   full `rtk cargo test`.

### Out of scope / follow-ups

- `SkillId` adoption completion (deferred — abstraction #5).
- Body-scanning + LLM-audit (other PROD9-226 facets).
- **Vet exit codes across the codebase** — candidate Linear ticket.
- **Align tabular CLI output.** `list_skills` / `explain_skill` / `pull` emit raw
  `\t`-separated columns (`NAME\tTIER\tSTATUS\tREASON`) that don't visually align.
  No table lib in tree — only `console` (0.16, has `measure_text_width` for
  unicode-correct widths) and `inquire` (prompts only); the CLAUDE.md `term_ui::Tui`
  is aspirational, doesn't exist. Either compute column widths via
  `console::measure_text_width` (no new dep, must account for `SanitizedString`
  render width) or add a table crate (`tabled`/`comfy-table`). Candidate Linear
  ticket. Tests assert on `\t`-split — they'd need updating.

## Confirm next session before coding

- `sanitize` → `skills::name` rename (abstraction #1).
- Deferring the `SkillId` migration (abstraction #5).
