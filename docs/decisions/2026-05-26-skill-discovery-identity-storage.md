# Decision: Skill Discovery, Identity, and School Storage (2026-05-26)

Status: **decided** — skills.sh-compatible 2-stage cascade (direct skill → priority dirs,
no recursive fallback); identity is the post-strip path (discovery-prefix dirs are not
identity); school storage drops the source's discovery prefix and lands skills under
`<school>/skills/<identity>/`.

## Problem

ACE's existing discovery only walks `<repo>/skills/<name>/`. This breaks for nested-layout
repos common in the ecosystem (e.g. mattpocock/skills, vercel-labs/skills, repos that ship
skills under `.claude/skills/`, `.cursor/skills/`, tier dirs like `.curated/`, plugin
manifests).

Fixing discovery cascades through identity (what *is* a skill, uniquely?), match handles
(how does the user refer to one?), school storage layout (how do imports land?), and
cross-source merge policy.

skills.sh + agentskills.io are the de-facto contract for this ecosystem. ACE must be
compatible with both — skills.sh is the looser superset, so "compat with both" effectively
means "compat with skills.sh's looser predicate."

Companion notes capturing the full analysis:

- `docs/vendor/agent-skills-spec.md` — frozen upstream spec snapshot.
- `docs/decisions/2026-06-01-skill-name-is-path.md` — resolves the shadowing / collision
  thread (name = path; shadowing is source-trust). It consolidated and replaced the earlier
  research note, which was removed once its rulings landed in that decision.

## Decision

### Discovery cascade

**2-stage** cascade:

1. **Direct skill** — repo root is itself a skill dir (`<root>/SKILL.md` exists). Returns
   immediately.
2. **Priority dirs (recursive within)** — canonical `skills/` + tier subdirs (`.curated/`,
   `.experimental/`, `.system/`) first; if empty, fall back to the full backend-specific
   priority list (`.claude/skills`, `.codex/skills`, `.opencode/skills`, `.cursor/skills`,
   etc. — skills.sh's full list). Within each priority dir, walk recursively at any depth
   for `SKILL.md` files. Nested categorization like `skills/typescript/coding/SKILL.md` is
   found here.

**No whole-repo recursive fallback.** skills.sh's stage 3 (`maxDepth=5` walk over the
entire repo, finding skills anywhere) is explicitly dropped: skills outside the
priority-dir set are off-spec and out of ACE's import surface. Repos must land skills at
root (stage 1) or under a priority dir (stage 2).

ACE divergence from skills.sh:

- **skills.sh stage 2** scans priority dirs *one level deep* and relies on stage 3 to pick
  up nested skills. ACE walks priority dirs recursively within stage 2 instead, so nested
  layouts work without enabling whole-repo walks.
- **skills.sh stage 2** walks all priority dirs in one pass. ACE splits into
  canonical-first / backend-fallback: when a repo provides both `skills/foo/` (curated)
  and `.claude/skills/foo/` (possibly an installed copy), only the curated one is
  discovered. If a repo only ships in backend dirs, the fallback still finds them.

Tier dirs (`.curated`, `.experimental`, `.system`) are a community convention that
skills.sh recognizes — **not** an ACE-owned identity layer. ACE specs that previously
described tier dirs as ACE-owned (`school-commands.md:103`, `school-toml.md:158`) need
rewrites to clarify ownership.

`metadata.internal` is honored as a discovery-time filter. Explicit-name imports bypass
the filter (mirrors skills.sh). `include_internal` joins the existing
`include_experimental` / `include_system` family in school.toml `[[imports]]` declarations
and matching CLI flags. **No `INSTALL_INTERNAL_SKILLS` env var passthrough** — flag +
per-decl config only.

### Discovery-prefix stripping (identity)

Discovery-location dirs describe *how the source organized itself for discovery*; they are
not part of skill identity. On the way into ACE's internal model, the longest matching
known prefix is stripped from the source-relative path. The remainder is the **identity**.

Known prefixes (longest-match wins):

- `skills/.curated/`, `skills/.experimental/`, `skills/.system/`
- `skills/`
- Every backend-specific dir from skills.sh's priority list (`.claude/skills/`,
  `.codex/skills/`, `.agents/skills/`, `.opencode/skills/`, `.cursor/skills/`, …)

Resulting identity examples:

| Source path                         | Identity                                               |
| ----------------------------------- | ------------------------------------------------------ |
| `skills/foo/SKILL.md`               | `foo`                                                  |
| `skills/typescript/coding/SKILL.md` | `typescript/coding`                                    |
| `skills/.curated/bar/SKILL.md`      | `bar`                                                  |
| `.claude/skills/foo/SKILL.md`       | `foo`                                                  |
| `.codex/skills/typescript/coding/…` | `typescript/coding`                                    |
| Root `SKILL.md` (stage 1)           | `<repo-basename>` (or `[[imports]]` key when supplied) |

Two skills with the same identity collide. That is intentional and surfaced by the
collision policy below; it is not a bug to be hidden behind a longer key.

The frontmatter `name` is pure metadata — display label, emit-time fallback (see
[emit decision](2026-05-26-skill-emit-and-match.md)), never an ACE-internal match key.
Rationale: the ecosystem disagrees on what frontmatter `name` *means*. agentskills.io
mandates `name == parent dir`; skills.sh doesn't enforce it; Claude Code uses `name` as
the slash-command token; other backends ignore it or treat it as display-only. Using it as
identity would couple ACE's internal model to whichever backend's semantics it picked.
Path-based identity is the one shape every consumer agrees on (it's where the `SKILL.md`
actually lives on disk), so ACE keys off that and lets `name` serve whatever per-backend
purpose each backend assigns it.

### Identity types

Required Rust types (sketched here, designed at impl time):

- **`SkillName`** — encapsulates identity + display + glob-target forms. Carries:
  - Identity path (the post-strip path; the unique key).
  - Frontmatter display name (verbatim from authoring).
  - Glob-target form: the identity path itself. Patterns like `rust/*`, `*/coding`, `**`
    operate against this path. Exact separator + render details TBD at implementation.
- **`SkillMeta`** — sibling type carrying the non-identity skill fields: `description`,
  `compatibility`, `internal`, and all Claude-Code-extended frontmatter (`when_to_use`,
  `argument-hint`, `disable-model-invocation`, `allowed-tools`, `model`, `effort`,
  `context`, `agent`, `hooks`, `paths`, `shell`). Pass-through container; ACE does not
  interpret these fields.

Frontmatter intake is **liberal**: skills.sh predicate (string `name` + string
`description`). No slug enforcement, no dir-name-match enforcement at parse time. Spec
violations (including `name != basename(identity)`) are warned, never rejected. See
companion [emit & match decision](2026-05-26-skill-emit-and-match.md) for the sanitization
model.

### Match handle

`--skill <arg>` is a glob against the identity path.

- `rust-coding` → matches any skill whose identity path is exactly `rust-coding` OR ends
  in `/rust-coding`
- `typescript/coding` → path-anchored multi-segment match
- `*/coding` / `**` → multi-match, intended behavior, not "ambiguity"
- `"Convex Best Practices"` → matches the skill whose identity leaf equals that exact
  string (rare; identities are normally slug-shaped)

Rationale for the bare-name rule (exact-or-end-match only, **not** prefix/middle):

- **Exact match preserves prior UX.** Before this decision, identity was a flat name;
  `--skill rust-coding` meant "the skill called rust-coding." Existing users and scripts
  expect bare names to resolve to the obvious skill, not to fuzzy-match.
- **End match is the minimum extension** required by the new path-based identity. A skill
  might live at `typescript/rust-coding`; forcing users to type the full path to address
  it would punish them for a model change they didn't ask for. Matching the leaf segment
  is the natural generalization.
- **Glob behavior is opt-in.** `*`, `**`, and explicit `/` segments are the user's signal
  that they want multi-match or path-anchored semantics. Bare strings shouldn't silently
  behave like globs.
- **Prefix and middle matches are deliberately excluded.** `rust` matching `rust-coding`
  (prefix) or `coding` matching `rust-coding-extra` (middle) would surprise users and
  create accidental multi-match. If you want fuzzy semantics, type the glob.

Errors echo user input verbatim. The slugified form (used at emit time per the companion
decision) is internal; users never see it.

### School storage

Schools store skills at `<school>/skills/<identity-path>/`. The outer `skills/` is the
school's category root (sibling to `rules/`, `commands/`, `agents/` per
`docs/spec/school/overview.md`). The inner segments are the identity path — already
prefix-stripped, so no `skills/skills/…` stutter.

Examples (within a single school):

```
<school>/skills/foo/SKILL.md                  # from skills/foo/ OR .claude/skills/foo/
<school>/skills/typescript/coding/SKILL.md    # from skills/typescript/coding/
<school>/skills/bar/SKILL.md                  # from skills/.curated/bar/
```

`ace school pull-imports` is **purely additive/overwriting**. ACE never deletes anything
from the school. Stale imports (skills dropped from `[[imports]]` resolution) persist
until the school author cleans them up manually (`git rm`, `rm -rf`). No manifest file, no
scan-and-diff, no detection logic. Intentionally dumb.

This follows the versioning philosophy in
[`docs/spec/index.md` §Versioning Philosophy](../spec/index.md#versioning-philosophy):
schools track latest main, the school is a git repo with full commit history, and ACE
deliberately stays out of reproducible-build territory. Auto-deletion would mean ACE owns
the school's working tree; instead the school author owns it, ACE just lays down imports
and lets git track the rest.

**Downstream compatibility (P2).** The school is a valid skills.sh source, not equivalent.
Downstream consumers using `npx skills add <school>` experience skills.sh's silent
first-wins dedup — same UX they'd get from any nested-layout repo. ACE-internal consumers
get the better behavior (see emit & match decision). We meet the "valid source" promise
without lobotomizing ACE's internal model.

### Cross-source merge policy

When two `[[imports]]` resolve skills to the same identity path, ACE picks the
**first-declared import** as the winner (matches skills.sh's stage-2 first-wins) and
**warns loudly** at every collision boundary:

- `ace school pull-imports` time (school maintainer's machine).
- Consumer discovery time (`ace pull` / `ace setup` of a downstream project, if the school
  maintainer ignored their warnings).

Warning messages attribute the problem to the **school**, not the consumer ("the school
you're consuming has...").

Within a single source, stage order is the tiebreaker (canonical `skills/` beats backend
dirs; tier order follows skills.sh).

Schools express disjoint sets explicitly to suppress warnings via `include_skills` /
`exclude_skills` patterns per `[[imports]]` declaration, matching the names already in use
in ace.toml (`docs/spec/configuration.md` §`include_skills`/`exclude_skills`):

```toml
[[imports]]
source = "ace-rs/school"
skills = ["*"]
exclude_skills = ["rust-coding"]

[[imports]]
source = "my/customizations"
skills = ["rust-coding"]
```

Schema refactor (consistency pass): `[[imports]]` adopts the plural `skills = []` array as
canonical, mirroring ace.toml's `skills` / `include_skills` / `exclude_skills` triple. The
existing singular `skill = "<string-or-glob>"` is **retained as a backcompat alias** (per
[CLAUDE.md §Backcompat](../../CLAUDE.md): `school.toml` keys are a public contract, no
removal in minor/patch). Liberal accept, conservative emit (per [[robustness-principle]]
memory): parser accepts both shapes; writers emit the plural form. Tier flags
(`include_experimental`, `include_system`, `include_internal`) are orthogonal and
unchanged — they gate discovery expansion, not skill-set membership.

Identity collision with frontmatter divergence (different `name` strings at the same
identity across sources) triggers an additional warning flagging the frontmatter mismatch
as a likely upstream spec violation.

**No new consumer-side suppression.** Existing `exclude_skills` in `ace.toml` remains the
consumer escape hatch. Keeps pressure on the school maintainer to fix upstream.

Spec edit pending: the school-side collision policy needs to land in the appropriate
school spec (likely `docs/spec/school/school-toml.md` or
`docs/spec/school/school-commands.md`). `docs/spec/skills-sync.md` is not the right target
— it covers project-side `ace pull` materialization, not school-side import resolution.

### `pull.rs` diff path extraction

The existing `skill_name_from_path` (`src/actions/project/pull.rs:237-252`) peels
`skills/[.tier/]<name>/` and returns a leaf string. Wrong shape under post-strip identity.

Replacement: walk up each diff path until a `SKILL.md` sibling is found; that dir's path
relative to `<school>/skills/` is the identity. Since school storage already lands skills
at `<school>/skills/<identity>/`, no further stripping is needed at the project-pull
boundary — the prefix-strip was applied at school write time.

### Collision taxonomy (Q8)

Under post-strip identity, "name collision" decomposes into three distinct concerns:

1. **Identity collision across `[[imports]]` sources** — same identity path in two
   declarations. Handled by the cross-source merge policy above (first-wins + warn,
   include/exclude to suppress).
2. **Backend-emit dirname collision** — different skill identities resolving to the same
   `.claude/skills/<dir>/` after the skills.sh emit rule (e.g. `typescript/coding` and
   `python/coding` both emit as `coding/`). See companion
   [emit & match decision](2026-05-26-skill-emit-and-match.md).
3. **Leaf invocation "ambiguity"** — `python/coding/` and `rust/coding/` coexist; user
   types `--skill coding`. Not an error: `--skill` is a glob, multi-match is intended.

Frontmatter-name "collisions" are not a collision class — under post-strip identity, two
skills sharing a frontmatter `name` field is fine as long as their identity paths differ.

## Out of scope

- **Plugin systems.** ACE does not handle Claude's plugin system or skills.sh's
  `pluginName` tagging. Skills from plugin-shaped repos are just skills.
  `plugin-name:skill-name` invocation grammar is not honored.
- **Subpath import** (skills.sh's `add https://github.com/owner/repo/tree/main/skills/foo`).
  Rejected; not tracked.
- **Lockfile.** `index.md:60-73` already rejects the lockfile-and-pin paradigm. ACE
  schools track latest main; reproducibility is not a goal.
- **Whole-repo recursive discovery (skills.sh stage 3).** Skills outside stage 1 or stage
  2 priority dirs are not discoverable by ACE. Off-spec layouts are the source repo's
  problem to fix. (Recursive walking *within* priority dirs is in scope — that's how stage
  2 finds nested skills.)

## Open

- Decision doc(s) need to enumerate the SkillName/SkillMeta type APIs at implementation
  time. This decision sketches roles only.
- Doctor checks (Linear PROD9-123) to be edited with the skill-spec diagnostic list
  (frontmatter mismatch, kebab/length violations, missing fields, identity collisions,
  bidi/control chars in path components).

Spec edits landed under `docs/spec/skills/` (model, selection, emit, sync) on 2026-05-26,
along with tier-ownership wording fixes in `docs/spec/school/school-toml.md` and
`docs/spec/school/school-commands.md`. Cross-source merge policy is in
`docs/spec/skills/selection.md`.
