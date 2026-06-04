# Spec reconciliation plan (2026-06-05)

**Status:** **executed 2026-06-05.** Audit complete; batch ratified via `/1-by-1`, then
applied — all 8 files edited plus the new D2 decision
(`decisions/2026-06-05-resolver-dissolution.md`). Docs-only (specs + decisions). Forks 2 & 3
were resolved this session (previously deferred to "decide during impl"). The code
implementation series (§ "Relationship to the code implementation series") is the next step.

This plan is the output of a pre-implementation **spec consistency audit** of the
skill-model rearchitect. The audit swept `docs/spec/skills/` *and* the related specs
(`architecture.md`, `configuration.md`, etc.) and found the system-level `architecture.md`
still documenting the **superseded** admission model. Fix the specs/decisions first so the
code implementation series builds from a coherent source of truth.

## Forks resolved this session

- **Fork 2 — `MatchHandle`: CUT.** Once `Locator` lands, the identity side of the
  handle/identity wall is already typed (`pattern_matches(&str, &Locator)`), so the
  newtype's load-bearing justification collapses; it's also part of the dead speculative
  layer the rearchitect removes, and a pattern is selection *input*, never a `Skill<S>`
  state. **Replace with:** (1) boundary glob-validation — `glob::validate` currently runs
  *only* inside the dead `MatchHandle::new`, so `skills = ["foo?"]` silently matches
  nothing today; move the validate call to the resolver seam as a **warn-diagnostic**
  (echo verbatim, next to the existing unknown-pattern warning, keeps `resolve` infallible)
  — *not* a hard error; (2) express pattern-`&str` vs `Locator` separation structurally in
  signatures. No new pattern type for now ("if not by another type construction"); revisit
  only if the seam proves leaky.

- **Fork 3 — package placement: DISSOLVE `src/resolver/`.** First-principles rule:
  *resolution lives with the typed data it reads/stamps.* Skill resolution stamps
  `Skill<S>` → moves into `src/skills/resolve/` (this is what kills the stringly
  `Carry`/`by_name` round-trip — the seam ceases to exist, not merely de-stringifies).
  Config-merge reads only `Tree`/`AceToml` → moves into `src/config/resolve/`. The
  top-level `resolver/` was a holding pen for two domain-resolutions that each have a
  proper home. `Source`/`Sourced` → `config/resolve/` ("which config layer won" is a config
  concept; `skills/resolve/` imports it leftward — the correct direction). The stringly
  typing was never "historical" — it was a **layering-violation workaround**: decision 007
  placed `resolve_skills` in `resolver/` (left of `skills/`), so it *couldn't* import skill
  types and downgraded to `String`. Fix the placement, the strings vanish. 007's only
  load-bearing constraint (`merge` stays infallible + skills-free so `ace config show`
  works without a school clone) is preserved — `merge` still touches only config types.

## The batch — execute in this order

Decisions first (specs cite them), then specs. Eight files, coupled vocabulary →
**execute sequentially in one context** (not parallel agents), **one commit**.

### Decisions

| ID | File | Edit |
| -- | ---- | ---- |
| D1 | `decisions/2026-06-04-skill-lifecycle-typestate.md` § Open/downstream | Mark **fork 2 = cut `MatchHandle`** and **fork 3 = dissolve `resolver/`** resolved. In-place status edit, mirroring how fork 4 was revised. |
| D2 | new dated decision (2026-06-05) | Rule the resolver dissolution / placement, **superseding `decisions/2026-04-27-config-resolution-redesign.md` § Module layout** (`:219-231`, incl. the `resolver/skills.rs` placement at `:225-226`). |

**D2 open question (last unresolved call):** form — a **new dated decision** (recommended;
house convention freezes decisions and supersedes via new entries) vs. an in-place note in
007. Pick before executing D2.

### Specs

| File | Loc | Edit |
| ---- | --- | ---- |
| `architecture.md` | — | **Restructure, not patch** (see below). |
| `model.md` | `:119-121` | Soften match-handle cross-ref → `Locator`-wall + validated-string-pattern (fork 2). |
| `model.md` | `:122-126` | Split the gate: persist accepts **vetted** (validated-or-later); emit accepts only **decided**. Current "validated-or-later" lump is too loose for emit (reads `Decided.included`). |
| `emit.md` | `:181-189` (`:183`) | De-telescope "validated identities **from the discovery layer**" → identities *constructed* by discovery, *gated* by validation. |
| `selection.md` | `:54-60` | Soften the typed-handle invariant ("transition is the only thing you can compile") → `Locator`-typed identity + boundary-validated string pattern in distinct slots (fork 2). |
| `sync.md` | `:71` | "every **discovered** skill is linked" → **validated**. |
| `sync.md` | `:66-67` | De-concretize `Resolution` / `Decision::Included` → behavioral ("stamps Included/Excluded; only Included links"). |
| `sync.md` | `:64` | Drop the `src/resolver/` path (fork-3 ripple; sync.md is behavioral). |
| `configuration.md` | `:168,:173` | Selection denominator "**discovered** skill set" → **validated** (admissible). |

### `architecture.md` restructure (Item 3 — the biggest)

The drift (admission-carried-as-annotation `:151-152`; select-over-everything +
display-derives-rejected `:153-163`; `Skill<Imported>` parallel typestate `:38-39,:64-65`)
exists because architecture.md **restates skill behavior that belongs in `skills/*.md`** —
violating its own "architectural shape only" promise (`:122-124`). Fix by **deletion**, so
there's no restatement left to drift. Blank-slate structure:

1. **Pipeline** — one diagram (collapse the duplicate at `:8-11` and `:187-189`) + one
   paragraph on demand-driven/lazy/cache.
2. **Dependency law** — promote from `:197` (it's the load-bearing invariant):
   `config ← {backend, school, skills} ← ace ← actions/cmd`; **no standalone resolver**;
   `config/resolve` + `skills/resolve` are in-domain submodules.
3. **Module map** — terse reference on the post-fork-3 tree: `config/` (parse +
   `config/resolve/` merge→`Resolved`), `backend/`, `school/`, `skills/` (typestate model
   `Discovered→Validated→Decided` + `Vetted` + `Locator`, discovery, admission,
   `skills/resolve/` stamping atoms), `ace/` (orchestrator + accessor table — **keep**),
   `actions/`, standalone.
4. **Cross-cuts** — only facts no single module owns: skills span bindings→actions;
   identity constructed **solely by discovery** (the `Locator` wall); **capability-driven
   emit** (`Kind::features()` bitmask — genuinely architectural, **keep** `:166-183`).

**Delete** the behavior narrative (`:126` four-stage list, `:140-164` admission/selection
prose) → replace with ~4 lines of typestate *shape* + pointer to the lifecycle decision.
A/B/C resolve by deletion.

## Relationship to the code implementation series

This batch is **docs-only reconciliation**. The **code** implementation series (carry
`Locator` end-to-end, fold `DiscoveredSkill` into `Skill<Discovered>`, make `validate` a
real partition, add the `Vetted` gate, de-stringify resolvers, drop `Skill.name`, plus the
fork-2/3 code changes now decided above) is the separate, larger next step — see
`docs/notes/2026-06-02-skill-model-rearchitect.md` § Resume. Do the reconciliation first;
the code then builds from coherent specs.
