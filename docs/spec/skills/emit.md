# Skill Emit

Where skills land on disk: how a school stores imports, how a project consumer emits to a
backend's skills dir (flat or nested per backend capability), and what gets sanitized at
each boundary. Companion to [model.md](model.md) (what a skill IS) and
[selection.md](selection.md) (which skills are picked).

## School storage layout

Schools store imported skills under `<school>/skills/<identity-path>/`. The outer
`skills/` is the school's category root (sibling to `rules/`, `commands/`, `agents/` per
[school/overview.md § Structure](../school/overview.md#structure)). The inner segments are
the **identity path** — already prefix-stripped at the source side, so no
`skills/skills/…` stutter.

```text
<school>/skills/foo/SKILL.md                  # from skills/foo/  OR  .claude/skills/foo/
<school>/skills/typescript/coding/SKILL.md    # from skills/typescript/coding/
<school>/skills/bar/SKILL.md                  # from skills/.curated/bar/
```

### Writes are additive / overwriting

`ace school pull` only adds or overwrites under `<school>/skills/`. ACE never deletes
anything. Stale imports (skills dropped from `[[imports]]` resolution) persist in the
working tree until the school author cleans them up manually (`git rm`, `rm -rf`). No
manifest, no scan-and-diff, intentionally dumb.

Rationale, two prongs:

- **No version tracking by design.** The support matrix between non-deterministic LLMs,
  project versions, tool versions, and ACE versions is unwinnable; ACE deliberately
  doesn't try. See [index.md § Versioning Philosophy](../index.md#versioning-philosophy).
- **Upstream deletions don't propagate downstream.** Preservation principle: an upstream
  source removing a skill should not silently remove it from schools that already imported
  it. The school author decides when (or whether) to follow upstream's removal, by hand.
  ACE never gives upstream a destructive channel into downstream working trees.

A future major version may revisit this with git-level tricks — `git subtree`, a different
school layout, or another mechanism — to track upstream history more tightly without
giving up the preservation guarantee. Out of scope for current versions.

### Downstream skills.sh compatibility (P2)

The school is a *valid* skills.sh source, **not equivalent**. Consumers running
`npx skills add <school>` experience skills.sh's silent first-wins dedup — same UX they
would get from any nested-layout repo. ACE-internal consumers get the better behavior
(loud warnings, per-import `exclude_skills`). The "compatible source" promise is met
without lobotomizing ACE's internal model.

## Backend emit rule

ACE's internal model is nested path identity. Backends disagree on whether their loader
walks nested skill dirs: Claude Code is flat-only
([anthropics/claude-code#18192](https://github.com/anthropics/claude-code/issues/18192)),
while Codex
([codex-rs/core-skills/src/loader.rs:455+](https://github.com/openai/codex/tree/main/codex-rs/core-skills/src/loader.rs),
BFS walk with `MAX_SCAN_DEPTH`) and OpenCode
([packages/opencode/src/skill/index.ts:23-25](https://github.com/sst/opencode),
`**/SKILL.md` glob) walk nested layouts directly.

Emit is **capability-driven**, not per-backend-coded. Each backend kind advertises a
bitmask of features; the emit code branches on the feature, never on the backend's name.
Two surfaces participate:

- `FEATURE_NESTED_SKILLS` — when set, the backend's loader handles nested
  `<backend>/skills/<identity-path>/` layouts. When clear, the loader sees only the top
  level and ACE must flatten.
- `MAX_SKILL_DEPTH` — global cap (5) on identity segments emitted nested. Skills deeper
  than the cap fall through to the flatten path even on nested-capable backends.

For each included skill, given the backend's `features`:

```text
if (features & FEATURE_NESTED_SKILLS) && segments(identity) <= MAX_SKILL_DEPTH:
    # nested emit
    write to <backend>/skills/<identity>/        # verbatim, no flatten, no collision check
else:
    # flatten emit
    skillName = skill.name || basename(skill.identity)
    structural_check(skillName)
    write to <backend>/skills/<skillName>/
```

Structural validation applies to every emitted path segment regardless of branch — that's
a filesystem concern, not a flatten concern. Character admission already happened during
discovery / resolution.

On the flatten branch, the resolved `skillName` is rejected (warn-and-drop) when it would
escape the skills dir, shadow a dotfile, or exceed filesystem limits:

- contains `/` (would synthesize a fake nested layout on a flat backend) or `\` (path
  separator on Windows)
- equals `.` or `..` (refers to the skills dir or its parent)
- starts with `.` (would shadow a real dotfile like `.gitignore` /`.env`)
- exceeds 255 bytes (per-component filesystem cap)

Imported skills aren't user-controlled, so ACE handles hostile frontmatter at the emit
boundary rather than asking for an upstream rename. Identity-path slashes are legitimate
on the nested branch and never reach this check.

The `skillName` rule for the flatten branch matches `vercel-labs/skills`
`src/installer.ts:247`. The collision policy below applies only on the flatten branch;
nested emit cannot collide because identity paths are unique by construction in school
storage.

### Loser-drop on collision (flatten branch only)

When two skills on the flatten branch resolve to the same `skillName`:

- **Tiebreaker** — alphabetical by source path. Winner emits.
- **Loser** — omitted from the backend entirely.
- **Warning** — identifies both source paths and provides remediation hints:
  - Rename frontmatter `name` upstream to disambiguate (or `basename` if no explicit
    name).
  - Use `[[imports]]` `exclude_skills` to express disjoint sets per source.

No path-prefix disambiguation. No segment expansion. No separator design. ACE applies the
rule, drops the loser, and warns. It does not synthesize new names.

Cost: the loser is absent from the backend until authoring fix. It remains reachable in
ACE's internal model (discoverable, globbable via match handles per
[selection.md](selection.md#match-handle)).

ACE's collision policy diverges from skills.sh on this branch:

- **Loud warning** at every `skillName` collision — skills.sh drops silently.
- **Deterministic tiebreaker** — alphabetical by source path. skills.sh's effective
  tiebreaker is first-encountered, which can churn as source repos reorder.

### Mixed-depth schools

A single school can contain skills at varying depths. Each skill is routed independently:
a Codex emit with skills at `foo/`, `typescript/foo/`, and
`langs/web/frameworks/react/foo/` (depth 5) emits all three nested; the same school
emitting to Claude flattens all three, and the latter two collide on `foo` (loser dropped,
warning emitted).

### When Claude Code lands nested discovery

Flip `FEATURE_NESTED_SKILLS` on for Claude in the registry. Loser-drop + warn becomes
vestigial for Claude; no other code changes.

## Frontmatter passthrough

Per [index.md](../index.md#versioning-philosophy) ("LLMs are not dumb consumers — they
read the skill, adapt, and resolve compatibility gaps themselves"):

- **All frontmatter passes through verbatim** to every backend. No stripping, no
  translation, no per-backend variant logic.
- Other backends ignore unknown fields by spec convention; Claude Code reads its
  extensions. ACE does not intervene.
- **ACE does not read the `compatibility` field.** It passes through like any other
  frontmatter; no gating, no warnings, no inspection. LLMs read it and adapt; ACE stays
  out.

## Admission at write boundaries

Per [model.md § Name Admission](model.md#name-admission), the boundary policy is:

- **School-storage writes** (`ace school pull`) — preserve verbatim.
- **Backend-emit writes** — structurally validate the link name. ACE emits per-skill
  symlinks rather than materialized SKILL.md copies (see
  [sync.md § Symlinks over copies](sync.md#symlinks-over-copies)), so the only string ACE
  synthesizes at the backend boundary is the directory name of each symlink. Discovery has
  already rejected inadmissible character content; emit keeps only the filesystem
  backstop.
- **ACE's own display** — render untrusted text through `SanitizedString`.

The Unicode-class whitelist (allow `L*`, `M*`, `N*`, `P*`, `S*`, `Zs`; reject `C*`, `Zl`,
and `Zp`) applies at discovery admission. Emit does not mutate characters. It only rejects
structurally unsafe path components (slash on the flatten branch, backslash, dot-segments,
leading-dot names, NUL, and overlong components).

### Type-safety invariant

Writes under `<school>/skills/…` and link names under `<backend>/skills/…` accept only:

- Validated identities (from the discovery layer per
  [model.md](model.md#type-safety-invariant)), and
- Structurally checked path components for any value ACE synthesizes at the backend
  boundary — currently the link directory name. School storage takes raw bytes by design
  (passthrough preserves the school author's responsibility and protects ACE consumers
  downstream through discovery admission).

The boundary type carries the proof. Code cannot write an unverified path or a
structurally unsafe link name to the backend by construction.

## Out of scope

- **Plugin systems** — see [model.md](model.md#out-of-scope). `pluginName` not read,
  `plugin-name:skill-name` not honored.
- **Frontmatter translation** between backend variants — see
  [model.md](model.md#honored-fields-pass-through).
- **Lockfile / pinning** — see
  [index.md § Versioning Philosophy](../index.md#versioning-philosophy).
