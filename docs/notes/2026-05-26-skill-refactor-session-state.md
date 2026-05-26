# Skill Discovery & Naming Refactor — Session Checkpoint

Captured 2026-05-26 to resume in a fresh session. Self-contained checkpoint of the design
conversation around making ACE skills.sh + agentskills.io compatible while preserving
authored skill structure.

## Status (2026-05-26 resume session)

**All open questions resolved.** Rulings live in two decision docs:

- [`docs/decisions/2026-05-26-skill-discovery-identity-storage.md`](../decisions/2026-05-26-skill-discovery-identity-storage.md)
- [`docs/decisions/2026-05-26-skill-emit-and-match.md`](../decisions/2026-05-26-skill-emit-and-match.md)

**Corrections to this checkpoint** (resolved during the resume session):

- **Q1d** — the original ratification (path-prefix disambiguation + alphabetical
  tiebreaker) was **superseded by loser-drop**. Winner emits alphabetically; loser is
  omitted from the backend with a loud warning. No path-prefix expansion, no separator
  design. See emit decision § "Collision handling (loser-drop)".
- **Q1f** — the framing here ("today's `pull-imports` flow knows where it wrote and can
  diff old-vs-new to clean up removed skills") was a wrong premise. ACE's `pull-imports`
  is purely additive/overwriting; there is no diff/cleanup logic. The decision generalizes
  this: no manifest, no scan-and-diff, intentionally dumb. School author handles cleanup
  manually.
- **Q3** (plugin namespacing) and **Q10** (subpath import) — removed from scope. ACE
  handles skills only.
- **Q11** (lockfile) — closed by existing spec (`index.md:60-73`).

**Pending action items not enacted this session:**

- Linear PROD9-123 edit (skill-spec diagnostic checks). Deferred to a follow-up.
- Tier-dir ownership rewrites in `school-commands.md:103` and `school-toml.md:158`.
  Deferred.
- Cross-source merge policy spec edit (likely `school-toml.md` or `school-commands.md` —
  not `skills-sync.md`, which covers project-side materialization). Deferred.

Remainder of checkpoint preserved for traceability.

## Origin

Pending task from prior session: import from mattpocock/skills failed (memory
`project_pending_import_mattpocock.md`). Repo uses nested skill layout
(`<category>/<skill>/SKILL.md`); ACE's discovery only looks at `<repo>/skills/<name>/`.
Investigation expanded to full skills.sh + agentskills.io compatibility.

## Companion notes (read these first when resuming)

- `docs/notes/2026-05-25-skills-sh-spec-reference.md` — frozen snapshot of the
  agentskills.io spec + skills.sh implementation (discovery cascade, SKILL.md predicate,
  sanitization, internal flag, etc.). Includes verbatim spec quotes and URLs to the
  authoritative sources.
- `docs/notes/2026-05-26-skill-collision-analysis.md` — collision matrix (4 rows, path ×
  frontmatter-name), cross-harness behavior comparison table (skills.sh / OpenCode / Codex
  / Claude Code), emission-boundary analysis with 6 named options, real-world evidence
  from GitHub issues (Claude Code #43003, #43297, #59423, etc., agentskills.io #115,
  #137).
- `docs/notes/2026-05-25-skills-sh-import-questions.md` — running questions list. Subsumed
  by this checkpoint for resolved questions; remaining questions still tracked there too.

## Decisions ratified this session

### Discovery model

- **3-stage cascade** matching skills.sh: direct skill → priority dir list → recursive
  fallback with `maxDepth = 5`
- **SKIP_DIRS**: skills.sh's defaults (`node_modules`, `.git`, `dist`, `build`,
  `__pycache__`) **extended** for ACE ecosystem (`target`, `.venv`, `.next`, `.turbo`,
  `out`, `vendor`, etc.). Hardcoded; no CLI/config flag yet — defer until concrete need.
- **Priority list includes tier dirs** (`.curated`, `.experimental`, `.system`) as a
  community convention skills.sh recognizes — not as ACE-owned identity layer.
- **`metadata.internal`** honored as discovery-time filter; explicit-name imports bypass
  the filter (mirrors skills.sh).
- **`include_internal`** joins the existing `include_experimental` / `include_system`
  family in school.toml `[[imports]]` and CLI flags.
- **No `INSTALL_INTERNAL_SKILLS` env var passthrough.** Flag + per-decl config only.

### Identity model

- **Identity = source-relative path.** Ratified. Two skills can share a frontmatter `name`
  if their paths differ.
- **Frontmatter `name` is pure metadata** — never used as a match key for ACE-internal
  operations. Shown in listings as secondary label when it differs from the path leaf.
- **Match handle = leaf of path** by default; ambiguous-leaf cases require a path
  qualifier.
- **`SkillName` Rust type** to be designed carefully — normalization layer encapsulating
  identity + display + glob-target forms. Replaces bare `String` representing skill
  identity across the codebase.
- **`SkillName` glob-target form**: joined `<source-path> + '/' + <frontmatter-name>`.
  Patterns like `rust/*`, `*/foo-coding`, `**` operate against this joined string. Exact
  separator + render details TBD at implementation time.

### School storage layout

- **Preserve nested source paths verbatim.** No flatten. No disambiguation at school-write
  time.
- **P2 — school is a valid skills.sh source, not equivalent.** Downstream consumers
  running `npx skills add <school>` experience skills.sh's de-facto silent first-wins
  dedup (no regression — same UX they'd get from any nested-layout repo). ACE-internal
  consumers get the better behavior. The "compatible source" promise is met; we don't
  lobotomize ACE's internal model.

### Cross-source override warnings (Q1c)

- **Warn-by-default** on every path collision across `[[imports]]` sources.
- **Add `include` / `exclude` patterns per `[[imports]]` ** — mirrors ace.toml's
  `include_skills` / `exclude_skills` shape. School author expresses disjoint sets
  explicitly to suppress warnings:
  ```toml
  [[imports]]
  source = "ace-rs/school"
  skill = "*"
  exclude = ["rust-coding"]

  [[imports]]
  source = "my/customizations"
  skill = "rust-coding"
  ```
- **Row 2** (path collision + frontmatter divergence) → same warn policy PLUS extra
  warning flagging the frontmatter mismatch as likely upstream spec violation.
- **No new consumer-side suppression mechanism.** Existing `exclude_skills` in ace.toml is
  the escape hatch — drop the noisy skill, lose the warning. Keeps pressure on school
  maintainer to fix upstream.
- **Warnings fire at two boundaries:**
  - `ace school pull-imports` (school maintainer's machine, during their own
    materialization)
  - Consumer discovery time (`ace pull` / `ace setup` of a downstream project, only if
    school maintainer ignored their warnings)
- **Warning messages attribute the problem to the school**, not the consumer ("the school
  you're consuming has...").

### Backend emit rule (Q1d)

- **Match skills.sh's installer** (`vercel-labs/skills` `src/installer.ts:247`):
  ```
  rawSkillName = skill.name || basename(skill.path)
  skillName = sanitizeName(rawSkillName)
  // land at <backend>/skills/<skillName>/
  ```
- **ACE diverges from skills.sh by warning on collisions** rather than silently
  first-wins-dropping.
- **Collision tiebreaker** (TBD detail at implementation): alphabetical by source path;
  loser gets path-prefix disambiguation. Deterministic, no churn across reruns given
  stable inputs.
- **No "never flatten" at backend** — Claude Code's flat-only discovery (verified via
  leaked v2.1.88 source at `yasasbanukaofficial/claude-code`
  `src/skills/loadSkillsDir.ts:415`, plus open issues #28266, #39138, #40640, #18192,
  #20805, #16438) forces flatten regardless of preference.
- **Codex and OpenCode support nested-within-skills-root discovery** (Codex
  `codex-rs/core-skills/src/loader.rs:455+` BFS walk with `MAX_SCAN_DEPTH`; OpenCode
  `packages/opencode/src/skill/index.ts:23-25` glob `**/SKILL.md`). But ACE adopts
  universal flat emit across all backends for consistency.

### Frontmatter-name vs dir-name compliance warning

- **Separate warning channel** from collision warnings.
- **Fires only at `ace school pull-imports` ** time — never at consumer discovery. This is
  school-authoring quality, not something downstream users should see noise about.
- Spec violations (kebab-case, length, leading/trailing/consecutive hyphens) are also
  school-authoring concerns — same channel.

### Frontmatter handling

- **Liberal intake**: skills.sh predicate (string `name` + string `description`). No slug
  enforcement, no dir-name-match enforcement at parse time.
- **Preserve verbatim** through internal model. No slugification.
- **Sanitization at write boundaries**: path-traversal defense + terminal escape stripping
  (CWE-150, per skills.sh's `sanitizeMetadata`).
- **Spec violations warned, not rejected.**

### Robustness principle (Postel's law)

- **Liberal in what we accept** (intake)
- **Conservative in what we emit** (sanitize, warn, disambiguate deterministically)
- **No silent drops** — every discovered skill reaches the backend in *some* form
  (possibly disambiguated with a warning)

### Q1b absorbed

Original Q1b "dir-name vs frontmatter-name as identity" — superseded by Q1's
path-as-identity ruling. Not re-litigated.

## Captured action items (pending — not yet enacted)

The 1-by-1 protocol was interrupted before step 6 (batch execution). These action items
are queued:

1. **Edit Linear PROD9-123** ("ace doctor: general environment health check command") to
   add skill-spec diagnostic checks:
   - Frontmatter `name` ≠ leaf dir name (spec violation)
   - Frontmatter `name` violates spec constraints (kebab-case, length 1-64, no
     leading/trailing/consecutive hyphens)
   - Frontmatter `name` or `description` missing or non-string
   - Path collisions across `[[imports]]` sources
   - Leaf-name display ambiguity across paths in the same school
   - Path segments hitting SKIP_DIRS (`node_modules`, `.git`, `dist`, etc.)

   PROD9-195 was considered but rejected — that's school-declared custom checks (different
   concern). PROD9-123 is the right home.

2. **Update questions list** (`docs/notes/2026-05-25-skills-sh-import-questions.md`) to
   mark Q1, Q1b, Q1c, Q1d as resolved with refs to this checkpoint.

3. **Draft decision doc(s)** once all questions resolved. Likely two decisions:
   - SkillName + identity + school storage model (Q1, Q1c, plus Q1e/f/g)
   - Backend emit + glob/match design (Q1d, plus Q3/Q4/Q5/Q9)

## Open questions (queue for next session)

### Carried from Q1's tail

- **Q1e** — School storage: keep top-level `skills/` segment, or store imports directly
  under school root? Affects backcompat with existing schools (everything under `skills/`
  today).

- **Q1f** — `ace school pull-imports` diff/cleanup with nested layouts: today's flow knows
  where it wrote (under `<school>/skills/`) and can diff old-vs-new to clean up removed
  skills. With nested layouts the write surface grows. Track imported paths explicitly
  (manifest? index file?) or scan-and-diff the whole school?

- **Q1g** (same as Q2 in original list) — `pull.rs:237-260` path-shape regex
  generalization. Today's regex assumes `skills/[.tier/]<name>/...`. Two replacements
  proposed: walk-up to find SKILL.md, or manifest lookup.

### Original numbered list

- **Q3** — Plugin namespacing. skills.sh tags `pluginName`; Claude Code uses
  `plugin-name:skill-name` (extension, not in spec). ACE: ignore, surface, or enforce?

- **Q4** — Which backend-specific dirs in skills.sh's priority list does ACE adopt?
  Verbatim (~30 dirs incl. `.cursor/skills`, `.windsurf/skills`, `.kiro/skills`), only
  ACE-targeted (`.claude`, `.agents`, `.opencode`), or only canonical `skills/`?

- **Q5** — Claude-Code-extended frontmatter fields. Pass through verbatim, strip on copy,
  or translate per backend? Fields: `when_to_use`, `disable-model-invocation`,
  `user-invocable`, `argument-hint`, `arguments`, `model`, `effort`, `context`, `agent`,
  `hooks`, `paths`, `shell`. Also: spec's experimental `allowed-tools` vs Claude Code's
  variant.

- **Q6** — `compatibility` field. Should ACE filter on it when syncing to non-target
  backends?

- **Q7** — Migration check for existing schools. Audit prod9/school and ace-rs/school for
  spec violations; fix upstream, grace period, or rely on the PROD9-123 doctor checks?

- **Q8** — Name collision across sources. Substantially answered by Q1c+Q1d but needs
  explicit confirmation in the decision doc.

- **Q9** — Sanitization adoption. Adopt skills.sh's `sanitizeMetadata` (terminal-escape
  stripping for CWE-150) at display boundaries?

- **Q10** — Subpath import. skills.sh supports
  `add https://github.com/owner/repo/tree/main/skills/foo`. Defer to "future capability"
  but track.

- **Q11** — Lockfile. skills.sh ships `skills-lock.json`. Worth surveying whether a lock
  fits ACE's model?

- **Q12** — skills.sh `--skill "Multi Word"` wart (from skills.sh README). Document that
  ACE follows agentskills.io spec (kebab-case only) and treats this as a skills.sh doc
  wart, not authoritative.

### Sub-questions for SkillName design (raised, not yet ruled on)

When implementing `SkillName`:

- Slug derivation rule (if any) — probably none under liberal intake, but the type might
  still expose a `slugify()` for emit-time sanitization?
- When dir-name and frontmatter `name` disagree on intake: which gets used where? Path
  identity uses dir-name (since it's the path). Display uses frontmatter `name`. Backend
  emit uses skills.sh's `name || basename(path)` rule.
- Exact separator for joined glob-target form (path-leaf-name join)
- Whether to expose a path-qualified canonical form for school.toml refs (e.g.
  `typescript/foo` to disambiguate from `python/foo`)

These should be captured in the decision doc when finalizing.

## How to resume

1. Read the three companion notes (refs above) for full background.
2. Read this checkpoint for ratified decisions + pending action items.
3. Walk remaining questions Q1e → Q12 via 1-by-1 protocol. Recommended batching:
   - Quick: Q1e, Q4, Q6, Q9, Q12, Q8-confirm
   - Substantive: Q1f, Q1g, Q3, Q5
   - Likely-defer: Q10, Q11
4. Once all open questions resolved, draft decision doc(s) under `docs/decisions/`
   (probably two — identity model + glob/emit model).
5. Enact captured action items:
   - Edit PROD9-123 with diagnostic check list
   - Update questions list with resolved markers
6. Then move to implementation work.

## File index (working set)

- `docs/notes/2026-05-25-skills-sh-spec-reference.md`
- `docs/notes/2026-05-25-skills-sh-import-questions.md`
- `docs/notes/2026-05-26-skill-collision-analysis.md`
- `docs/notes/2026-05-26-skill-refactor-session-state.md` ← this file

No code changes yet. No Linear edits yet. No decision docs yet. All ratified decisions
live in this checkpoint until the decision doc is drafted.
