# Decision: A skill's name is its path; shadowing is source-trust (2026-06-01)

Status: **decided** — emit name = `basename(identity)`; frontmatter `name` is display-only,
never an emit or identity key. Supply-chain "shadowing" is **not** defended structurally — it
reduces to source-trust plus path-collision visibility and a fixed set of author-time
warnings. Resolves the open sub-decision #1 of
emit & match (folded into `docs/spec/skills/emit.md`) toward basename-always, and supersedes the
namespaced-storage proposal of the supply-chain skill-shadowing note (deleted; see git
history).

## Problem

Two threads converged.

1. **The emit naming rule was dual-keyed.** `skillName = frontmatter.name || basename(identity)`
   (emit & match) gave a skill two naming channels, and the frontmatter one is
   upstream-forgeable. That dual-keying is the root of the shadow vector, the
   frontmatter-divergence warning class, and the name-vs-identity confusion. Sub-decision #1
   left "honor `frontmatter.name` vs basename-always" open.

2. **Supply-chain shadowing.** The research note framed a compromised import taking over an
   authoritative name (`general-coding`, the `ace*` family) as a distinct vulnerability
   needing a structural fix — namespaced import storage, provenance. That framing was wrong.

## Decision

### 1. Name = `basename(identity)`, always

Drop the `frontmatter.name ||` channel. Identity is the path (per
discovery & identity (folded into `docs/spec/skills/model.md`)); the path is the only
naming axis. The emitted directory name on every backend is `basename(identity)`. Frontmatter
`name` is display-only — passed through verbatim, never read as an emit or match key.

For spec-compliant skills (agentskills.io mandates `name == parent-dir`) this is byte-identical
to the old rule. Only spec-violating skills (`name != basename`) differ — and there ACE emits
the dir it controls (the basename), not the forgeable field. Resolves emit & match
sub-decision #1.

### 2. Responsibility boundary — ACE owns the path, the backend owns the frontmatter

ACE controls only the directory/path it lays down; it passes frontmatter through verbatim. A
backend that keys or invokes on `frontmatter.name` is keying on a field ACE deliberately does
not touch. That is **not a gap ACE must patch — it is a boundary.** "Emit cannot defend
shadowing" is the wrong framing; emit is simply not ACE's layer for frontmatter.

Backend keying (verified against live sources, 2026-05/06):

| Backend     | Identity key                          | Collision behavior            |
| ----------- | ------------------------------------- | ----------------------------- |
| Codex       | path (`AbsolutePath`)                 | distinct paths → both kept    |
| Claude Code | dir name (load); `frontmatter.name` (slash/invoke) | flat-only load   |
| OpenCode    | `frontmatter.name`                    | last-wins + warn              |
| skills.sh   | `frontmatter.name`                    | first-wins, silent drop       |

ACE controls the key for the path/dir-keyed surfaces (Codex, Claude load). The
frontmatter-keyed surfaces (skills.sh, OpenCode, Claude's slash token) are the backend's
domain.

**Consequence for admission.** This narrows the [name admission policy](2026-05-30-skill-name-admission-policy.md),
whose predicate was "a verdict over identity *plus frontmatter name*." Admission now keys on
**identity segments only**. Frontmatter `name` is not an admission axis: ACE never emits or
matches on it, so a hostile char there is the backend's domain, neutralized solely by the
display transform (`render` / `SanitizedString`) when ACE itself prints the field. Checking
it at admission would have ACE fail-closed on — and drop a selected skill because of — a
field it deliberately does not touch, which is a boundary violation, not defense in depth.
The whitelist/fail-closed posture is undiminished: it still governs the identity path, the
one name ACE owns. Implemented as `name::admissible_skill(identity)` (single arg).
Admission no longer drops a skill for its frontmatter, but the hygiene signal is not lost:
the two authoring boundaries (`ace import`, `ace school pull`) emit a non-fatal
`warn` + `hint` — `DiscoveredSkill::frontmatter_warning` — when an admitted skill carries a
spoofable frontmatter `name`. `NameContext::FrontmatterName` survives solely to phrase that
warning; it is no longer an admission axis.

### 3. Shadowing is source-trust, not a name defense

The name is a **label, not a capability**: the model reads the skill's *content* and
adapts; being called `general-coding` grants no authority. A compromised import already
has a live malicious channel — malice inside the skills it legitimately owns,
auto-propagating under the
[no-version philosophy](../spec/overview.md#versioning-philosophy) — which is
undefendable and *accepted*, the "trusted distro ships a bad package" case. "Name
takeover" therefore escalates to nothing. Selection (`skills` / `exclude_skills`) already
scopes the attack surface to what the author chose to trust.

The one ACE-domain takeover surface is a **path collision**: two sources resolving to the same
identity path. That lands as a literal file clash in the school's git tree, visible at `pull`,
reconciled by the human who owns the repo. No provenance, no namespace, no ledger — git is the
mechanism, and it is already there.

### 4. Collision classes, and where each warns

The full warning map lives in
[selection.md § Collision and warning map](../spec/skills/selection.md); this decision
established it, including the warning principle (warn only on what ACE decides silently;
explicit author input is at most informational).

### 5. Rejected, and why

| Approach                    | Why not                                                                          |
| --------------------------- | -------------------------------------------------------------------------------- |
| Namespaced import storage   | Defends the name as a trust boundary — it isn't one. Heavy machinery, non-threat. |
| Authored / source wins      | Same non-boundary; needs provenance to survive to emit, which it doesn't.        |
| TOFU / change-detection     | Re-introduces the pinning the no-version stance rejected; content changes every pull anyway. |
| Strict agentskills storage  | Backends read ACE's emit, not `<school>/skills/`; storage format is decoupled from compat. Buys nothing, costs third-party intake (which [[third-party-skills-constraint]] forbids rejecting). |
| Honor `frontmatter.name` at emit | The forgeable second channel — source of the shadow vector and the divergence convolution. Byte-identical for compliant skills anyway. |

## Empirical impact

Measured 2026-06-01 against the three local school clones, the cached external import repos,
and a fresh `mattpocock/skills` clone (~280 external skills total):

- Every discovered skill has `name == basename` → **name=basename changes no emitted name
  anywhere**. The lone `name != basename` (`anthropics` `template-skill`) sits in an off-spec
  location ACE's no-stage-3 cascade never scans.
- `mattpocock/skills` is the one real nested-category repo (`skills/<cat>/<skill>`, 29 skills,
  2-segment identity). Its leaves are all unique → no collapse drop today; on a flat backend
  the *category* is flattened away (`engineering/tdd` → `tdd`), which is the ratified flatten.
- Every other repo categorizes via tier dirs (`.curated`/`.experimental`, stripped to flat) or
  flat `skills/<name>/`. `pproenca/dot-skills` (~195 skills, tiers + `plugins/` + duplicated
  `.claude`/`.agents` copies) confirms the cascade: backend-dir duplicates and `plugins/` are
  skipped when canonical tiers are present.

Collapse and divergence are real but currently-unobserved shapes; the flat-emit lint and the
divergence warning cover them when a future import uses them.

## Out of scope

- Namespaced storage, provenance ledger, TOFU, strict-agentskills storage — see Rejected.
- Lockfile / pinning, plugin systems, frontmatter translation — unchanged from the 2026-05-26
  decisions.

## Open / to implement

- The `ace school validate` skill-lint slice: flat-emit collision sim, dead-selector
  (informational), requested-but-rejected (warn), `name != basename` divergence (warn). The
  [validate v1 scope decision](2026-05-09-school-validate-scope.md) anticipated "separate
  slices when needed" and deliberately did not auto-hook `pull`/`setup`; honor that — manual
  command, with the authoring agent (`ace-school` skill) instructed to run it.
- Whether `pull` emits a one-line "run `ace school validate`" nudge when same-leaf identities
  are present.

## References

- Consolidates and replaces the prior research note `supply-chain-skill-shadowing.md` (threat
  analysis, backend keying facts, upstream collision evidence) — removed in this commit; see
  git history.
- Companion visual summary: `docs/scratch/2026-06-01-skill-lifecycle.html`.
- Resolves sub-decision #1 of emit & match (folded into `docs/spec/skills/emit.md`); builds on
  discovery & identity (folded into `docs/spec/skills/model.md`) and
  [name admission](2026-05-30-skill-name-admission-policy.md).
- Specs: `emit.md` + `model.md` updated alongside this decision — the `frontmatter.name ||`
  emit rule is dropped and the path-as-name boundary recorded. The warning-set wording lands
  with the `ace school validate` task.
- Memory: supersedes `project_pending_collision_spoof` (its namespaced-storage framing is
  rejected here).
