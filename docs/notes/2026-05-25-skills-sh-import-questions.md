# Open questions — skills.sh-compatible import

Running list of unresolved design questions for adopting the agentskills.io/skills.sh
discovery contract in ACE. To be walked 1-by-1 in a later session; each question gets a
final ruling that goes into the spec/decisions docs.

## Status (2026-05-26)

**All questions resolved.** Rulings recorded in two decision docs:

- [`docs/decisions/2026-05-26-skill-discovery-identity-storage.md`](../decisions/2026-05-26-skill-discovery-identity-storage.md)
  — Q1, Q1b, Q1c, Q1e, Q1f, Q1g (= Q2), Q4, Q7, Q8
- [`docs/decisions/2026-05-26-skill-emit-and-match.md`](../decisions/2026-05-26-skill-emit-and-match.md)
  — Q1d, Q5, Q6, Q9, Q12

Out of scope (no decision needed):

- **Q3** (plugin namespacing) — ACE doesn't handle Claude's plugin system. Skills only.
- **Q10** (subpath import) — rejected.

Closed by existing spec:

- **Q11** (lockfile) — `docs/spec/index.md:60-73` already rejects lockfile/pinning
  paradigm.

Questions list below is preserved as-is for traceability.

**Companion docs:**
- [skills.sh spec reference](2026-05-25-skills-sh-spec-reference.md) — frozen copy of
  upstream contracts
- Authoritative sources (re-fetch if local refs go stale):
  - <https://agentskills.io/specification> — canonical SKILL.md format spec
  - <https://github.com/vercel-labs/skills/blob/main/src/skills.ts> — reference consumer
    impl (discovery cascade + parsing)
  - <https://github.com/vercel-labs/skills/blob/main/src/sanitize.ts> — display
    sanitization (CWE-150)
  - <https://code.claude.com/docs/en/skills> — Claude Code's skill behavior (most-extended
    consumer)

## Resolved going in

For traceability — these are settled, not for re-litigation:

- **Discovery model**: skills.sh-compatible 3-stage cascade (direct skill → priority dirs
  → recursive fallback with `maxDepth=5` + SKIP_DIRS). Not flat `**/SKILL.md`.
- **Tier dirs** (`.curated`/`.experimental`/`.system`): community folder pattern, **not**
  ACE convention and **not** in agentskills.io spec. skills.sh includes them in its
  priority list; ACE does too. Existing ACE specs that describe these as ACE-owned
  (`school-commands.md:103`, `school-toml.md:158`) need to be rewritten to clarify
  ownership.
- **Skill identity**: directory name = frontmatter `name`. This is mandated by
  agentskills.io spec, not chosen by ACE. ACE's existing model is already spec-compliant;
  we enforce, not redesign.
- **`metadata.internal`**: honor as discovery-time filter. Add `include_internal` flag to
  the family alongside `include_experimental` / `include_system`. Explicit-name imports
  bypass the filter (mirrors skills.sh).
- **SKIP_DIRS**: hardcoded extended list (skills.sh's + ACE-ecosystem additions like
  `target`, `.venv`, `.next`, etc.). No CLI/config flag exposed yet — defer until concrete
  need.
- **No env var passthrough**: `INSTALL_INTERNAL_SKILLS=1` not honored. Flag + per-decl
  config only.

## Open — to be walked 1-by-1

### Q1 — Spec enforcement strictness (compat-with-both reframe)

**Goal: be compatible with both skills.sh's actual behavior AND agentskills.io spec.**
Since skills.sh is the looser superset (its `parseSkillMd` only requires `name` and
`description` to be non-empty strings; no slug enforcement, no dir-name-match check),
"compat with both" effectively means "compat with skills.sh's looser predicate."

That forces a decision on what ACE does with a skills.sh-acceptable but spec-noncompliant
skill (e.g. `name: "Convex Best Practices"`):

- **Reject** at import boundary with diagnostic — strict, breaks skills.sh compat
- **Slugify on intake**, keep frontmatter as display name — dual-namespace cost
- **Pass through verbatim**, let downstream fail — surfaces noncompliance lazily
- **Dir-name-as-identity model** (see Q1b) — sidesteps the question for filesystem/CLI;
  spec mismatch becomes warning-only

Subordinate question: warn vs silent for spec violations regardless of chosen handling.

### Q1c — Warning policy for cross-source overrides (Row 1 collisions)

Same path + same name across two `[[imports]]` sources is often intentional (school author
wants to override an upstream version). Today's merge logic silently last-wins. Options:

- Silent: trust the school author knows what they declared
- Warn-by-default: surface every override
- Configurable: school.toml-level `warn_overrides = true` or per-import flag
- "Intentional overrides" list: explicit allowlist in school.toml; warn for any not on the
  list

See [2026-05-26 collision analysis](2026-05-26-skill-collision-analysis.md) Row 1.

### Q1d — Backend emit flattening rule

When school storage is nested but backend expects flat layout (`.claude/skills/<name>/`,
`.agents/skills/<name>/`, `.opencode/skills/<name>/`), what's the rule?

- (a) Leaf-name only — silent collision risk
- (b) Path-prefix flatten — ugly, breaks slash invocation
- (c) Leaf-name with collision detection + path-prefix the loser, warn
- (d) Frontmatter-name — couples backend output to upstream spec compliance
- (e) Refuse — hard error
- (f) Hybrid frontmatter-when-compliant + leaf + path-prefix fallback

Leaning (c). See
[collision analysis Emission boundaries §](2026-05-26-skill-collision-analysis.md).

### Q1e — School storage layout

If school preserves nested source paths, do we still have a top-level `skills/` segment in
the school, or store imports directly under school root?

```
# Option A: keep `skills/` prefix
<school>/skills/typescript/foo/SKILL.md
<school>/skills/skills/.curated/bar/SKILL.md  # awkward when source itself uses `skills/`

# Option B: drop the prefix, store at root
<school>/typescript/foo/SKILL.md
<school>/skills/.curated/bar/SKILL.md          # source path verbatim
```

Affects backcompat — existing schools have everything under `skills/`.

### Q1f — `ace school pull-imports` diff/cleanup with nested layouts

Today's `pull-imports` flow knows where it wrote (under `<school>/skills/`) and can diff
old-vs-new to clean up removed skills. With nested layouts the write surface grows. Track
imported paths explicitly (manifest? index file?) or scan-and-diff the whole school?

### Q1g — `pull.rs:237-260` path-shape regex

Current regex assumes `skills/[.tier/]<name>/...`. Two replacements:

- Walk-up: from any modified path, walk up until `SKILL.md` is found, take the parent dir
  as the leaf identity component (combined with full path for the identity)
- Manifest lookup: maintain a manifest of imported paths; map diff path → skill by
  manifest lookup

### Q1b — Identity model: directory name vs frontmatter name

skills.sh's install behavior appears to use **directory name** as the on-disk identity
(`.agents/skills/<source-dir>/`), with frontmatter `name` serving as a match/display key.
agentskills.io spec mandates they're equal, but skills.sh doesn't enforce that.

ACE today uses directory name everywhere (filesystem, dedup, glob, CLI, school.toml refs).
Three positions:

- **Keep dir-name-as-identity** (matches skills.sh observed behavior + ACE's status quo).
  Frontmatter `name` becomes a display-only field, used in listings and matching.
  Mismatches with spec → warn, don't reject. Smallest change, most compat.
- **Switch to frontmatter-name-as-identity** (matches spec's intent). Requires slug
  enforcement on intake → conflicts with skills.sh compat for noncompliant skills. Biggest
  refactor.
- **Hybrid**: directory name for filesystem/CLI, frontmatter name for match/display. Both
  tracked in `DiscoveredSkill`. Warn on mismatch.

Lean toward dir-name-as-identity; resolves the bulk of the compat-with-both tension
without contorting ACE's existing model.

### Q2 — `pull.rs` git-diff name extraction

`actions/project/pull.rs:237-260` extracts skill name from diff paths shaped like
`skills/[.tier/]<name>/...`. Once discovery accepts nested layouts, this regex breaks for
foreign repos. Two paths:

- Stop relying on path shape — re-discover after pull and diff the discovered set against
  pre-pull state
- Generalize path extraction to "walk up until SKILL.md is found, then take that dir name"

### Q3 — Plugin namespacing

skills.sh tags skills with `pluginName` when discovered via a plugin manifest. Claude Code
uses `plugin-name:skill-name` for invocation (spec-extension, not in agentskills.io). Does
ACE:

- Ignore plugin grouping entirely (current behavior)
- Surface `pluginName` in `DiscoveredSkill` and pass it through to backends
- Enforce namespacing on import (so two plugins can ship `foo` without collision)

### Q4 — Which backend-specific dirs in priority list?

skills.sh's priority list includes ~30 agent-specific dirs (`.claude/skills`,
`.codex/skills`, `.opencode/skills`, `.goose/skills`,...). For ACE importing from a
foreign repo, do we:

- Include all of skills.sh's list verbatim (max compat)
- Include only ACE-targeted ones (`.claude`, `.agents`, `.opencode`)
- Include only the canonical `skills/` + tier subdirs (most conservative)

Tradeoff: foreign repo might ship its skills only under `.cursor/skills/`, and ignoring
that means we silently miss them.

### Q5 — Claude-Code-extended frontmatter fields

Fields like `when_to_use`, `disable-model-invocation`, `user-invocable`, `argument-hint`,
`arguments`, `model`, `effort`, `context`, `agent`, `hooks`, `paths`, `shell` are Claude
Code extensions, not in agentskills.io spec. When ACE syncs to a non-Claude backend:

- Pass through verbatim (Codex/OpenCode ignore unknown fields)
- Strip on copy
- Translate (when there's a semantic equivalent)

Similar question for the spec's experimental `allowed-tools` — Claude Code's variant
differs slightly (accepts list, not just string).

### Q6 — `compatibility` field

Spec allows `compatibility:` to declare target product / system reqs. Should ACE filter on
it? E.g. if a skill declares `compatibility: Designed for Claude Code only`, do we still
sync it to OpenCode? Lenient ignore vs strict gate.

### Q7 — Migration check for existing schools

Existing schools (prod9/school, ace-rs/school) may have skills where directory name and
frontmatter name disagree, or names violate the spec (e.g. leading underscore, uppercase).
Before flipping on strict validation, audit existing schools and either:

- Fix the offenders upstream
- Ship a `ace school doctor` check
- Define a grace period / lenient mode for legacy skills

### Q8 — `name` collision across sources

skills.sh dedups by frontmatter `name`. Today ACE merges per-source last-wins (or
first-wins, depending on accumulator). With the spec mandating name = dir name, collisions
are dir-name collisions. Does the merge strategy stay (sequential, last-wins) or do we
make it explicit?

### Q9 — Sanitization adoption

skills.sh's `sanitizeMetadata` strips terminal escapes from name/desc before display.
Spec-conformant names are already escape-free, so the defense is for malformed
frontmatter. Does ACE adopt the same defense (at display boundaries — `inquire` prompts,
status output, `ace skills list`)?

### Q10 — Subpath import (deferred but tracking)

skills.sh supports
`add https://github.com/owner/repo/tree/main/skills/web-design-guidelines` — direct path
to a single skill. Discovery resolves with `subpath` anchored at that path. ACE doesn't
expose this. Worth specifying as future capability, even if not implemented now.

### Q11 — Lockfile

skills.sh ships `skills-lock.json` for reproducible installs. ACE currently re-discovers
on every pull. Worth surveying whether a lock fits ACE's model.

### Q12 — `--skill "Multi Word"` in skills.sh

skills.sh's own README shows `--skill "Convex Best Practices"` as an example, which would
be **invalid per agentskills.io spec** (uppercase, spaces). This is a wart in skills.sh,
not authoritative. ACE should follow the spec; document the choice so this doesn't bite
later.
