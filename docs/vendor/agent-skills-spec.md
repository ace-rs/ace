# skills.sh / Agent Skills spec — reference snapshot

Captured 2026-05-25 from primary sources. **Not** an ACE design document — this is a
frozen copy of what the upstream ecosystem currently specifies, for use as a reference
when we design ACE's compatibility layer. If/when ACE chooses to diverge from any of this,
that decision goes under `decisions/` and supersedes the relevant section here.

## Sources

| Source                                                    | What it is                                                     |
| --------------------------------------------------------- | -------------------------------------------------------------- |
| <https://agentskills.io/specification>                    | The canonical open spec for the SKILL.md format. Authoritative |
| <https://github.com/vercel-labs/skills> (`src/skills.ts`) | The `npx skills` CLI — reference consumer implementation       |
| <https://code.claude.com/docs/en/skills>                  | Claude Code's skill behavior — the most-extended consumer      |

## SKILL.md format (agentskills.io spec)

A skill is a directory containing a `SKILL.md` file with YAML frontmatter + markdown body.

```
skill-name/
├── SKILL.md       # required
├── scripts/       # optional
├── references/    # optional
├── assets/        # optional
└── ...
```

### Frontmatter fields

| Field           | Required | Constraints                                                                 |
| --------------- | -------- | --------------------------------------------------------------------------- |
| `name`          | Yes      | 1–64 chars. `[a-z0-9-]` only. No leading/trailing/consecutive hyphens.      |
| `description`   | Yes      | 1–1024 chars. Non-empty.                                                    |
| `license`       | No       | License name or path to bundled license file                                |
| `compatibility` | No       | ≤500 chars. Environment requirements (target product, system pkgs, network) |
| `metadata`      | No       | Free-form string → string map                                               |
| `allowed-tools` | No       | Space-separated tool list, pre-approved. **Experimental**                   |

### The `name` invariant (verbatim from spec)

> Must be 1-64 characters May only contain unicode lowercase alphanumeric characters
> (`a-z`, `0-9`) and hyphens (`-`) Must not start or end with a hyphen (`-`) Must not
> contain consecutive hyphens (`--`) **Must match the parent directory name**

The directory-name == frontmatter-name equality is part of the spec, not a convention. A
skill in `foo/SKILL.md` must have `name: foo` in frontmatter, or it's invalid.

### Progressive disclosure (3 stages)

1. **Discovery**: agent loads only `name` + `description` for every skill at startup.
2. **Activation**: when a task matches, full `SKILL.md` body is loaded.
3. **Execution**: bundled scripts/references/assets are loaded only when referenced.

Recommendation: keep SKILL.md under 500 lines; large content in separate referenced files.

## skills.sh discovery algorithm (`src/skills.ts`)

Discovery is a **3-stage priority cascade**, not a flat `**/SKILL.md` walk:

### Stage 1 — direct skill

If `<searchPath>/SKILL.md` exists, treat `<searchPath>` as a single-skill target and
return. (Unless `fullDepth: true` is set, then continue to stage 2.)

### Stage 2 — priority directory list

For each well-known dir, scan one level deep for child dirs containing `SKILL.md`.
First-seen-by-frontmatter-name wins.

```
<searchPath>/                       (top-level subdirs)
<searchPath>/skills/
<searchPath>/skills/.curated/
<searchPath>/skills/.experimental/
<searchPath>/skills/.system/
<searchPath>/.agents/skills/        (Codex, universal-agent install target)
<searchPath>/.claude/skills/
<searchPath>/.cline/skills/
<searchPath>/.codebuddy/skills/
<searchPath>/.codex/skills/
<searchPath>/.commandcode/skills/
<searchPath>/.continue/skills/
<searchPath>/.github/skills/
<searchPath>/.goose/skills/
<searchPath>/.iflow/skills/
<searchPath>/.junie/skills/
<searchPath>/.kilocode/skills/
<searchPath>/.kiro/skills/
<searchPath>/.mux/skills/
<searchPath>/.neovate/skills/
<searchPath>/.opencode/skills/
<searchPath>/.openhands/skills/
<searchPath>/.pi/skills/
<searchPath>/.qoder/skills/
<searchPath>/.roo/skills/
<searchPath>/.trae/skills/
<searchPath>/.windsurf/skills/
<searchPath>/.zencoder/skills/
+ plugin-manifest-declared paths
```

Note: `.curated` /`.experimental`/`.system` are recognized at this layer. skills.sh treats
them as priority dirs to scan; there is no tier semantic attached beyond their position in
the priority list.

### Stage 3 — recursive fallback

If stage 2 found nothing (or `fullDepth: true`), walk recursively from `<searchPath>` up
to `maxDepth = 5`, skipping:

```
SKIP_DIRS = ['node_modules', '.git', 'dist', 'build', '__pycache__']
```

This handles repos with custom layouts (nested category dirs, monorepos) that aren't
covered by the priority list.

## Skill predicate (skills.sh `parseSkillMd`)

A directory containing `SKILL.md` becomes a skill iff:

1. The file parses as YAML frontmatter + markdown.
2. `data.name` is present and is a string.
3. `data.description` is present and is a string.
4. Unless overridden, `data.metadata?.internal !== true` (or `INSTALL_INTERNAL_SKILLS=1`,
   or the skill is requested by explicit name).

Failures return `null`, silently skipping the directory. This means example / template /
fixture `SKILL.md` files without proper frontmatter are auto-filtered.

## Sanitization

`sanitizeMetadata` (in `src/sanitize.ts`) strips terminal escape sequences and control
chars from `name` and `description` strings before display. Defends against CWE-150
(terminal escape injection). Whitespace is collapsed: internal newlines → single space,
surrounding whitespace trimmed.

Note this is a **display defense**, not a slug normalization. Spec-conformant names
already only contain `[a-z0-9-]`, which is escape-free; sanitization is defense-in-depth
against malformed frontmatter.

## Internal skills (`metadata.internal`)

Frontmatter convention skills.sh honors:

```yaml
---
name: find-skills
description: ...
metadata:
  internal: true
---
```

Behavior:

- Skipped from discovery by default.
- Included if `INSTALL_INTERNAL_SKILLS=1` environment variable is set.
- Included if `options.includeInternal: true` is passed (e.g. CLI explicit name).

vercel-labs/skills uses this for its own bootstrap skill (`find-skills`).

## Subpath safety

skills.sh validates that any user-supplied subpath doesn't escape the cloned repo via `..`
traversal (`isSubpathSafe` in `src/skills.ts`). Important if ACE exposes per-import
subpath anchoring in future.

## Invocation forms (consumer-defined, not spec)

The spec is silent on invocation; that's consumer territory. Observed patterns:

| Consumer         | Invocation              | Notes                                                                      |
| ---------------- | ----------------------- | -------------------------------------------------------------------------- |
| Claude Code      | `/<name>` slash command | Frontmatter `name` is both display name and slash token                    |
| Claude Code      | Auto, by description    | LLM decides from `description` whether to load skill                       |
| Codex / OpenCode | Implementation-defined  | Per agentskills.io listing, both adopt the format; invocation not surveyed |
| skills.sh CLI    | `--skill <name>`        | Filter, not invocation                                                     |

Because spec mandates name = parent dir = `[a-z0-9-]`, the slash form `/<name>` is
unambiguous across consumers. ACE can rely on this.

## Claude Code extensions (not in spec)

Claude Code's frontmatter accepts fields beyond the spec — `when_to_use`, `argument-hint`,
`arguments`, `disable-model-invocation`, `user-invocable`, `allowed-tools` (Claude
variant), `model`, `effort`, `context`, `agent`, `hooks`, `paths`, `shell`. Other
consumers ignore them. Per spec these belong under `metadata:` for forward-compat but the
docs show them at top level.

The spec's `allowed-tools` (experimental) and Claude's `allowed-tools` overlap in name and
intent; the spec one is space-separated string, Claude's accepts both string and YAML
list.

## Plugin grouping

skills.sh recognizes `plugin-manifest` -declared skill paths (see `src/plugin-manifest.ts`).
When a skill is part of a plugin, it gets a `pluginName` tag. Plugins are a way to bundle
multiple related skills with shared config. Claude Code uses `plugin-name:skill-name`
namespacing on top of this for invocation.

## What's *not* in the spec

For context — recurring patterns that look load-bearing but aren't:

- **Tier dirs** (`.curated/.experimental/.system`): a community folder pattern that
  skills.sh bakes into its priority list, not part of the agentskills.io spec.
- **Slug normalization**: spec assumes `name` is already a valid slug. No "we'll slugify
  your messy name" behavior anywhere.
- **Display name separate from identity**: name is both. No display-only alias mechanism.
- **Versioning**: spec has no version field; consumers (skills.sh, etc.) handle update
  semantics out of band (lockfiles, git refs).
