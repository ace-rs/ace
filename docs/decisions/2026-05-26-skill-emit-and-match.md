# Decision: Skill Backend Emit, Sanitization, and Frontmatter Handling (2026-05-26)

Status: **decided** — emits match skills.sh's installer rule with loser-drop on collision;
sanitization at display + backend-emit boundaries only, using a Unicode-class whitelist;
frontmatter passes through verbatim.

Companion to the
[discovery, identity, and storage decision](2026-05-26-skill-discovery-identity-storage.md).
Reading order: identity first, then this.

## Problem

Backends (Claude Code, OpenCode, Codex, Droid) expect a flat `<backend>/skills/<name>/`
layout. ACE's internal model is nested-path identity. Flattening at the emit boundary
forces a naming rule and a collision policy.

Separately: foreign skills may carry malformed or malicious metadata (terminal escapes,
bidi-override chars). Where does ACE sanitize, and how?

And: Claude Code extends agentskills.io's frontmatter with fields other backends don't
recognize (`when_to_use`, `disable-model-invocation`, `allowed-tools` variant, etc.). What
does ACE do with them when emitting to non-Claude backends?

## Decision

### Backend emit rule

For each discovered skill at backend emit time:

```
skillName = skill.name || basename(skill.identity)
skillName = sanitizeName(skillName)
// write to <backend>/skills/<skillName>/
```

Matches `vercel-labs/skills` `src/installer.ts:247`. ACE diverges from skills.sh in two
ways:

1. **ACE warns loudly on collisions** rather than silently first-wins-dropping.
2. **ACE drops the loser** on `skillName` collision rather than overwriting.

### Collision handling (loser-drop)

When two skills resolve to the same `skillName` at the emit boundary:

- Tiebreaker: **alphabetical by source path**. Winner emits.
- Loser is **omitted from the backend** entirely.
- Loud warning at emit time identifies both source paths and provides remediation hints:
  - Rename frontmatter upstream to change `name` (or `basename` if no explicit name).
  - Use school.toml `[[imports]]` `exclude` to express disjoint sets.

**No path-prefix disambiguation.** No segment expansion. No separator design. The author
already has the tools to disambiguate (frontmatter name, exclude); ACE applies the rule,
drops the loser, and warns. It does not synthesize new names.

Cost: the "no silent drops" invariant from the collision analysis is relaxed. Mitigated by
loud warnings — the loser remains reachable in ACE's internal model (discoverable,
globbable via `SkillName`) but is absent from the backend until authoring fix.

### Frontmatter passthrough (Q5, Q6)

Per `docs/spec/index.md:60-73` ("LLMs are not dumb consumers — they read the skill, adapt,
and resolve compatibility gaps themselves"):

- **Pass all frontmatter through verbatim** to every backend. No stripping, no
  translation, no per-backend variant logic.
- Other backends ignore unknown fields by spec convention; Claude Code reads its
  extensions. ACE does not intervene.
- **ACE does not read the `compatibility` field.** It passes through verbatim like any
  other frontmatter. No gate at emit, no launch-time warning, no inspection anywhere.
  LLMs read it and adapt; ACE stays out. (Amended 2026-05-26 from an earlier draft that
  spec'd a launch-time heuristic warning.)

### Sanitization (Q9)

Threat model: malformed or malicious frontmatter containing terminal escape sequences
(CWE-150), bidi-override chars (U+202A–U+202E, U+2066–U+2069), and other display-spoofing
payloads. If ACE displays raw `name` /`description` unsanitized, ACE's terminal is
compromised. If ACE writes raw bytes to backend SKILL.md files, the backend's terminal
display is compromised.

**Boundary policy:**

| Boundary                                                    | Action                                                                                                |
| ----------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| ACE's own display (prompts, list, warns)                    | **Sanitize on render**                                                                                |
| School storage write (`pull-imports`)                       | **Preserve verbatim** (school author's responsibility; ACE consumers protected at consumer-side emit) |
| Backend emit write (SKILL.md to `.claude/skills/...`, etc.) | **Sanitize into written frontmatter**                                                                 |
| Internal in-memory model                                    | **Raw, preserved** (doctor checks need to see violations; debugging needs truth)                      |

**Approach: Unicode-class whitelist**, not denylist.

Allowed general categories: `L*` (letters), `M*` (marks), `N*` (numbers), `P*`
(punctuation), `S*` (symbols), `Zs` (space separator). Drop everything in `C*` (control)
plus the bidi-override block. Replacement char TBD at impl (likely U+FFFD or empty).

Rationale for whitelist over denylist: denylisting known-bad terminal escapes is a losing
race against novel exploits. Unicode-class allow gives a defensible, principled boundary.

**Path components from foreign repos** (e.g. a segment of a multi-segment identity that
itself contains bidi-override chars): import as-is, warn. Identity is a path (per the
discovery decision); we cannot post-hoc rename segments without breaking refs. Trust
filesystem display tools for the path layer. Consistent with the preservation rule.

**Type sketch (designed at impl time):** introduce a `SanitizedString` newtype to enforce
sanitization-at-boundary via the Rust type system. Sits alongside `SkillName` /
`SkillMeta`. Internal model holds raw `String`; boundary code that emits to display or
backend SKILL.md goes through `SanitizedString` conversion.

### Caveat (accepted)

Consumers who bypass ACE (run skills.sh directly against an ACE-authored school) get the
un-sanitized payload. They're outside ACE's protection envelope by their own choice.

## Contingency: flat emit is not permanent

Universal flat emit is forced by Claude Code, the lowest common denominator. Codex
(`codex-rs/core-skills/src/loader.rs:455+`, BFS walk with `MAX_SCAN_DEPTH`) and OpenCode
(`packages/opencode/src/skill/index.ts:23-25`, `**/SKILL.md` glob) both support nested
emit today. Claude Code is the holdout: leaked v2.1.88 source shows flat-only loading, and
there is active, unresolved user demand for nested support
([anthropics/claude-code#18192](https://github.com/anthropics/claude-code/issues/18192) —
OPEN, 52 👍, 34 comments; companion docs bug
[#40640](https://github.com/anthropics/claude-code/issues/40640) — OPEN; multiple related
issues closed as duplicate or stale).

If Claude Code lands nested discovery, ACE's nested-aware internal model (post-strip
identity, see [discovery decision](2026-05-26-skill-discovery-identity-storage.md)) means
we can switch to per-backend nested emit without re-architecting — only the emit rule
changes. Loser-drop + warn becomes vestigial for backends that no longer collide. Reassess
at that point.

## Out of scope

- **Plugin systems** — see discovery decision. `pluginName` not read,
  `plugin-name:skill-name` not honored.
- **Frontmatter translation** between backend variants — explicitly rejected per
  `index.md:60-73`. LLMs adapt; ACE doesn't.
- **Lockfile / pinning** — rejected by `index.md:60-73`.

## Open

- `SanitizedString` API (Rust crate choice for Unicode general categories, e.g.
  `unicode-general-category`) — design at impl time.
- Match cascade implementation details for `SkillName` joined-form globbing: exact
  separator render, multi-match ordering, glob library choice.
- Doctor checks (Linear PROD9-123) to be edited with the skill-spec diagnostic list — see
  discovery decision.
