# Skill collision & emission analysis

Working analysis for ACE's skill discovery/import/emit boundaries under the skills.sh /
agentskills.io compat decision. Captures the collision matrix, harness behavior
comparison, emission boundaries, options at each, and real-world reports from upstream
repos. Companion to:

- [skills.sh spec reference](2026-05-25-skills-sh-spec-reference.md) — frozen spec
  snapshot
- [open questions list](2026-05-25-skills-sh-import-questions.md) — Q1, Q1b, etc.

This is *notes*, not policy. Decisions land in `docs/decisions/` once ruled.

## Identity model (working assumption)

Per Q1 working decisions:

- **Identity = source-relative path.** A skill is uniquely identified by where it lives in
  the source. Two skills can share a frontmatter name iff their paths differ.
- **Frontmatter `name` ** = display/match key, may legally collide across paths under the
  liberal-intake policy.
- **No flatten** at school storage emit: school preserves the source-relative layout
  verbatim.
- **`SkillName` Rust type** owns the dual representation (path + display name).
- **Robustness** is the guiding principle (liberal intake, conservative emit).

## Collision matrix (simplified, two axes)

After collapsing scope+dir-name into a single `Path` axis:

| #   | Path      | Frontmatter name | Scenario                                                           | Possible?         | Collision class                                                 |
| --- | --------- | ---------------- | ------------------------------------------------------------------ | ----------------- | --------------------------------------------------------------- |
| 1   | same      | same             | Two sources with identical layout (one source filesystem-prevents) | Cross-source only | **Emit collision** at school storage (both want same dest path) |
| 2   | same      | different        | Same relative path, divergent frontmatter names                    | Cross-source only | **Emit collision** at school storage + frontmatter divergence   |
| 3   | different | same             | Two skills at distinct paths share a display name                  | Always            | **Display ambiguity** (globs, `--skill foo`, listing)           |
| 4   | different | different        | Trivially distinct                                                 | Always            | None                                                            |

**What can go wrong, per row:**

### Row 1 — same path, same name (cross-source)

Two import sources both ship `skills/foo/SKILL.md` with `name: foo`. ACE pulls both into
`<school>/skills/foo/`. Without a policy, the second copy overwrites the first, or vice
versa, depending on merge order.

What goes wrong:
- Silent data loss if no warning
- Non-deterministic outcome if merge order isn't fixed
- School author may not know which version they got

Mitigation (current code, `Skills::merge` in `src/skills/mod.rs:157`): last-wins per
`[[imports]]` declaration order in `school.toml`. Documented in `docs/spec/skills-sync.md`
§ Import Merge Strategy. Behavior is deterministic given a stable school.toml.

Gap: no warning currently emitted when overwrite happens. User intent often is exactly
this (override an upstream version), so warning by default may be noisy. Configurable warn
level? Maintain a "intentional overrides" list in school.toml?

### Row 2 — same path, different name (cross-source)

Source A ships `skills/foo/SKILL.md` with `name: foo`. Source B ships
`skills/foo/SKILL.md` with `name: foo-but-renamed`. Same on-disk path, divergent
frontmatter.

What goes wrong:
- All of Row 1's issues, plus:
- Display name flips depending on which source wins
- School author may have been referring to `foo` in matchers, suddenly gets
  `foo-but-renamed` after the merge
- Indicates the upstream sources are spec-noncompliant on at least one side (since spec
  mandates dir-name == frontmatter-name)

Mitigation: same as Row 1 + warn loudly because the frontmatter divergence is a
near-certain sign of trouble (not deliberate override of "same skill").

### Row 3 — different paths, same name (any scope)

Most common collision class once nested layouts are accepted. `<school>/skills/foo/` and
`<school>/typescript/foo/` both exist after import.

What goes wrong:
- `--skill foo` is ambiguous — which one does the user mean?
- Glob `*` matches both; school author may or may not want both
- Listing shows two `foo` entries; user may think it's a duplicate bug
- **Backend emission collision** — both want to symlink as `.claude/skills/foo/`

Mitigation options (interact with emission policy below):
- Match-by-path when ambiguous (`--skill typescript/foo`)
- Refuse ambiguous glob matches with diagnostic
- Path-prefix flatten at backend emit
- Warn at school-level if two skills share a frontmatter name

### Row 4 — no collision

Trivial. No action.

## Cross-harness behavior comparison

How upstream consumers handle these collisions today:

| Harness         | Identity key             | Dedup behavior                                                                  | Collision response                                                                          | Source                                                        |
| --------------- | ------------------------ | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| **skills.sh**   | Frontmatter `name`       | Map keyed on `name`; first-found wins guarded by `seenNames.has(name)`          | First-found-wins, silently drops the rest                                                   | `vercel-labs/skills` `src/skills.ts:200-220`                  |
| **OpenCode**    | Frontmatter `name`       | Map keyed on `name`; warn-then-overwrite                                        | Last-found-wins; `log.warn("duplicate skill name", {name, existing, dup})`                  | `sst/opencode` `packages/opencode/src/skill/index.ts:126-135` |
| **Codex**       | Path (`AbsolutePathBuf`) | `HashSet<AbsolutePathBuf>` retain pattern, sorted by `scope_rank` before retain | Scope-precedence-wins on path collision; different paths = both kept                        | `openai/codex` `codex-rs/core-skills/src/loader.rs:196-225`   |
| **Claude Code** | Dir name + scope tier    | Cross-scope: enterprise > personal > project; plugins namespaced `plugin:skill` | Filesystem-prevented within scope; cross-scope precedence; within-scope-nested undocumented | docs only — source closed                                     |

**Three different identity keys across the four consumers.** No standard exists beyond the
spec's mandate that `name == dir-name` (which, if everyone followed it, would collapse
path vs. name into one thing). ACE picks its own; Codex's path-as-identity is closest to
what's been ratified here.

## Real-world evidence from upstream issues

Searching the relevant repos surfaced these — confirms the collision tension is real and
unsolved in practice:

### Claude Code

- **#43003** *Local personal skills don't suppress anthropic-skills: duplicates in skill
  list* (closed). Skills created by skill-creator land in a deep
  `~/Library/.../skills-plugin/...` path AND in `~/.claude/skills/<name>/` after manual
  copy. Both appear in the listing. Invocation works (precedence works at invocation
  time), but listing is cosmetically broken. **Evidence: scope-precedence is not
  consistently applied across all surfaces.**

- **#43297** *Marketplace plugin skill gets silently mapped to official plugin skill
  instead of registering separately* (closed). Two plugins (`frontend-design` official +
  `interface-design` marketplace) ship skills under their respective
  `skills/<plugin-name>/` dirs. Even with plugin namespacing, the marketplace skill gets
  *silently mapped* to the official one. **Evidence: even with explicit namespacing,
  silent collisions happen at consumer layer.** The intended
  `interface-design:interface-design` slot never registers.

- **#59423** *"N skill descriptions dropped" — empty descriptions for duplicate SKILL.md
  between plugins/marketplaces/ and plugins/cache/* (open). Same skill discovered via two
  distinct paths (marketplace cache vs install cache) causes downstream description-budget
  bookkeeping to break.

- **#42384** *Duplicate skills in slash command autocomplete menu* (closed).

- **#29520** *Plugin skills duplicated in /context report and system prompt* (closed).

- **#25994** *Skills loaded twice after context compaction (111 instead of ~63)* (closed).

Pattern: Claude Code's collision handling has had multiple bugs around different surfaces
(listing, autocomplete, context, descriptions) disagreeing on which copy "won." Indicates
the implementation has multiple dedup pipelines that don't share state.

### agentskills.io spec

Active open spec issues — the ecosystem hasn't agreed on these either:

- **#137** *Clarify whether nested skills are allowed* (open). Spec is silent on whether
  one skill can invoke another. Tangentially relevant — points at unresolved ambiguity in
  how skills compose.

- **#115** *Proposal: add path-based, recursive skill discovery* (open). Cline contributor
  proposing monorepo-style nested `.agents/skills/` discovery with **deepest-path-wins**
  precedence for same-named skills at multiple levels. This is essentially the model ACE
  is converging on. **Same-name-different-path is explicitly addressed.** Not yet ratified
  by the spec.

- **#30** *Propose having foldername.md as an alternative discovery pattern* (open).

- **#46** *support versioning/locking* (open). Indicates the ecosystem has no agreed-upon
  mechanism for skill versioning either.

### Conclusion from field evidence

The collision problem is unsolved upstream. Every consumer has chosen its own identity
model; multiple consumers (Claude Code) have surface-level bugs because their dedup logic
is inconsistent across surfaces; the spec itself has open proposals for path-based
discovery (#115) that would address this but haven't been accepted.

ACE inheriting **path-as-identity + deepest-wins / leaf-name-with-disambiguation** matches
the direction the spec is moving (#115), matches Codex's existing implementation, and
avoids the Claude Code-style "multiple dedup pipelines" inconsistency.

## Emission boundaries

ACE has **two** emit points with different constraints:

### Emit 1: Source → school storage (`ace school pull-imports`)

ACE controls the format. Working decision: **no flatten** — school preserves
source-relative paths verbatim.

```
upstream://typescript/foo/SKILL.md  →  <school>/typescript/foo/SKILL.md
upstream://skills/.curated/bar/...   →  <school>/skills/.curated/bar/...
```

Implications:
- School's own structure becomes nested (today it's flat under `skills/`)
- Tier dirs (`.curated`, `.experimental`, `.system`) are preserved as path segments if the
  source uses them
- Collision row 1 + 2 still possible cross-source; merge policy unchanged
  (declaration-order, last-wins)

Open sub-questions:
- Do we still need a top-level `skills/` segment in school storage, or do we store
  directly under school root?
- How does `ace school pull-imports` 's existing diff/cleanup logic (which compares
  against the school's `skills/` dir) generalize?
- `pull.rs:237-260` git-diff name extraction still breaks here.

### Emit 2: School → backend (`ace setup`, `ace pull`)

Backends expect flat layout (`.claude/skills/<name>/`, `.agents/skills/<name>/`,
`.opencode/skills/<name>/` — verified via docs and source). **Flatten is required at this
boundary**; the only question is the rule.

Backend emit options:

- **(a) Leaf-name only**: symlink `.claude/skills/foo/` for any skill whose dir name is
  `foo`. Last-wins on collision, silent. **Bad** — Claude Code's #43297-class issue.

- **(b) Path-prefix flatten**: `.claude/skills/typescript-foo/`. Deterministic and
  collision-free. Ugly names; breaks `/foo` slash invocation; partially destroys author
  intent at backend boundary.

- **(c) Leaf-name with collision detection**: prefer leaf-name; on collision with another
  in same school, warn and path-prefix the loser. Clean common case; mechanical
  disambiguation only when needed. Deterministic disambiguation order (alphabetical or
  import-declaration-order).

- **(d) Frontmatter-name**: use the `name:` field. Spec-conformant cases produce clean
  names; noncompliant (spaces, uppercase) inherits noncompliance into backend layout and
  breaks slash invocation. Couples backend output to upstream frontmatter quality.

- **(e) Refuse to emit collisions**: hard error, force author to disambiguate. Strict;
  preserves invariant; bad UX (no path forward without authoring change).

- **(f) Hybrid — frontmatter-name when compliant+unique, else leaf, else path-prefix**:
  layered fallback. Maximum author-intent preservation when authors followed the spec;
  mechanical disambiguation otherwise.

**Leaning option (c).** Reasoning:

- Robustness: clean output by default, mechanical disambiguation when needed.
- Doesn't couple backend output to upstream spec compliance (avoids option d's
  brittleness).
- Doesn't fail the user (avoids e).
- Avoids silent collisions (avoids a).
- Avoids unnecessary noise (avoids b).
- Option f is similar but more layers = more rules = more surprises.

Worth verifying: does deterministic disambiguation order matter beyond "any stable order"?
Probably alphabetical by path, with a warning emitted to the user explaining which skill
got renamed and what they need to do to silence the warning (rename upstream, or change
school.toml to import one side under an alias).

**What can go wrong with (c):**

- Symlink churn if the disambiguation order isn't truly deterministic across runs.
  Mitigation: sort by path, deterministic.
- User confusion when `/foo` works on day 1 but becomes `/typescript-foo` on day 2 after a
  new import adds a colliding `python/foo`. Mitigation: clear warning + the disambiguated
  symlink retains the leaf-name as an alias if filesystem allows? (Probably not — too
  magic.)
- Path-prefix may itself collide (`typescript/foo` and `typescript-foo` both exist).
  Mitigation: pick a separator unlikely to appear (`--`?) or fall through to
  full-path-flatten.

### Cross-emit invariant

Whatever rule we adopt at emit 2, ACE should never silently drop a skill. Every discovered
skill that survives intake should reach the backend in *some* form, possibly under a
disambiguated name with a warning. Silent drops are the single most common upstream bug
(Claude Code #43297, skills.sh's first-wins-silently).

## Open sub-questions raised by this analysis

To add to the question list (separate file):

- **Q1c**: Warning policy for Row 1 cross-source overrides — silent (intentional),
  warn-by-default, configurable?
- **Q1d**: Backend emit rule — option (c) draft, needs ratification.
- **Q1e**: Should school storage have a top-level `skills/` segment, or store imports
  under school root directly?
- **Q1f**: How does `ace school pull-imports` diff/cleanup work with nested layouts in
  school storage?
- **Q1g**: `pull.rs:237-260` path-shape regex — generalize to "walk up until SKILL.md,
  take that dir name as the leaf identity component"?
