# Skill Emit

Where skills land on disk: how a school stores imports, how a project consumer emits
to a backend's flat skills dir, and what gets sanitized at each boundary. Companion to
[model.md](model.md) (what a skill IS) and [selection.md](selection.md) (which skills
are picked).

## School storage layout

Schools store imported skills under `<school>/skills/<identity-path>/`. The outer
`skills/` is the school's category root (sibling to `rules/`, `commands/`, `agents/`
per [school/overview.md § Structure](../school/overview.md#structure)). The inner
segments are the **identity path** — already prefix-stripped at the source side, so no
`skills/skills/…` stutter.

```text
<school>/skills/foo/SKILL.md                  # from skills/foo/  OR  .claude/skills/foo/
<school>/skills/typescript/coding/SKILL.md    # from skills/typescript/coding/
<school>/skills/bar/SKILL.md                  # from skills/.curated/bar/
```

### Writes are additive / overwriting

`ace school pull-imports` only adds or overwrites under `<school>/skills/`. ACE never
deletes anything. Stale imports (skills dropped from `[[imports]]` resolution) persist
in the working tree until the school author cleans them up manually (`git rm`,
`rm -rf`). No manifest, no scan-and-diff, intentionally dumb.

Rationale: per
[index.md § Versioning Philosophy](../index.md#versioning-philosophy), schools track
latest main with full git history. Auto-deletion would mean ACE owns the school's
working tree; instead the school author owns it, ACE just lays down imports and lets
git track the rest.

### Downstream skills.sh compatibility (P2)

The school is a *valid* skills.sh source, **not equivalent**. Consumers running
`npx skills add <school>` experience skills.sh's silent first-wins dedup — same UX
they would get from any nested-layout repo. ACE-internal consumers get the better
behavior (loud warnings, per-import `exclude_skills`). The "compatible source" promise
is met without lobotomizing ACE's internal model.

## Backend emit rule

Backends (Claude Code, OpenCode, Codex, Droid) expect a **flat** layout —
`<backend>/skills/<name>/`. ACE's internal model is nested path identity. Flattening
at the emit boundary forces a naming rule and a collision policy.

For each discovered skill at backend emit time:

```text
skillName = skill.name || basename(skill.identity)
skillName = sanitize(skillName)
// write to <backend>/skills/<skillName>/
```

The `skillName` rule matches `vercel-labs/skills` `src/installer.ts:247`. ACE's
collision policy diverges from skills.sh:

- **Loud warning** at every `skillName` collision — skills.sh drops silently.
- **Deterministic tiebreaker** — alphabetical by source path. skills.sh's effective
  tiebreaker is first-encountered, which can churn as source repos reorder.

Both implementations drop the loser; see [Loser-drop on collision](#loser-drop-on-collision).

### Loser-drop on collision

When two skills resolve to the same `skillName` at emit:

- **Tiebreaker** — alphabetical by source path. Winner emits.
- **Loser** — omitted from the backend entirely.
- **Warning** — identifies both source paths and provides remediation hints:
  - Rename frontmatter `name` upstream to disambiguate (or `basename` if no explicit
    name).
  - Use `[[imports]]` `exclude_skills` to express disjoint sets per source.

No path-prefix disambiguation. No segment expansion. No separator design. ACE applies
the rule, drops the loser, and warns. It does not synthesize new names.

Cost: the loser is absent from the backend until authoring fix. It remains reachable
in ACE's internal model (discoverable, globbable via match handles per
[selection.md](selection.md#match-handle)).

### Why universal flat emit

Codex
([codex-rs/core-skills/src/loader.rs:455+](https://github.com/openai/codex/tree/main/codex-rs/core-skills/src/loader.rs),
BFS walk with `MAX_SCAN_DEPTH`) and OpenCode
([packages/opencode/src/skill/index.ts:23-25](https://github.com/sst/opencode),
`**/SKILL.md` glob) both support nested emit today. Claude Code is the holdout: its
loader is flat-only ([anthropics/claude-code#18192](https://github.com/anthropics/claude-code/issues/18192)
is the active issue tracking nested support).

ACE adopts universal flat emit across all backends rather than per-backend special
casing — one rule, one collision policy, one set of warnings. Claude Code is the
lowest common denominator; the others get a uniform shape.

### Contingency

If Claude Code lands nested discovery, ACE's nested-aware internal model means we can
switch to per-backend nested emit without re-architecting — only the emit rule
changes. Loser-drop + warn becomes vestigial for backends that no longer collide.
Reassess at that point.

## Frontmatter passthrough

Per [index.md](../index.md#versioning-philosophy) ("LLMs are not dumb consumers — they
read the skill, adapt, and resolve compatibility gaps themselves"):

- **All frontmatter passes through verbatim** to every backend. No stripping, no
  translation, no per-backend variant logic.
- Other backends ignore unknown fields by spec convention; Claude Code reads its
  extensions. ACE does not intervene.
- **`compatibility` is not a gate.** Skills with
  `compatibility: Designed for Claude Code only` still emit to OpenCode and elsewhere.
- **Launch-time heuristic warning** at bare `ace` (or `ace new`) backend launch: scan
  loaded skills, best-effort case-insensitive substring match between `compatibility`
  prose and the launching backend's name, warn on mismatches. Never blocks.

## Sanitization at write boundaries

Per [model.md § Sanitization](model.md#sanitization), the boundary policy is:

- **School-storage writes** (`ace school pull-imports`) — preserve verbatim.
- **Backend-emit writes** — sanitize into the written frontmatter.
- **ACE's own display** — sanitize on render.

The Unicode-class whitelist (allow `L*`, `M*`, `N*`, `P*`, `S*`, `Zs`; drop `C*` plus
bidi-override) applies at the backend-emit boundary. Path components are imported
as-is with a warning; identity is a path, and post-hoc segment rewriting would break
refs.

### Type-safety invariant

Writes under `<school>/skills/…` and `<backend>/skills/…` accept only:

- Validated identities (from the discovery layer per
  [model.md](model.md#type-safety-invariant)), and
- Sanitized frontmatter (carrying the sanitization marker per
  [model.md](model.md#type-safety-invariant-1)) — except at school storage, which
  takes raw bytes by design (passthrough preserves the school author's responsibility
  and protects ACE consumers downstream).

The boundary type carries the proof. Code cannot write an unverified path or an
unsanitized string to the backend by construction.

## Out of scope

- **Plugin systems** — see [model.md](model.md#out-of-scope). `pluginName` not read,
  `plugin-name:skill-name` not honored.
- **Frontmatter translation** between backend variants — see [model.md](model.md#honored-fields-pass-through).
- **Lockfile / pinning** — see
  [index.md § Versioning Philosophy](../index.md#versioning-philosophy).
