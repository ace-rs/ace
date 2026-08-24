# Skill Selection

How ACE picks which skills are active at each surface: how users address a single skill
(match handle), how `ace.toml` 's three skill fields combine, how `[[imports]]` in
`school.toml` selects from each source, and how cross-source collisions are resolved.

Companion to [model.md](model.md) (what a skill IS) and [emit.md](emit.md) (where
skills land).

## Match handle

Wherever a user names a skill — `--skill <arg>`, an `[[imports]]` `skills` pattern, an
`ace.toml` `skills` / `include_skills` / `exclude_skills` pattern — the argument is
matched as a **glob against the identity path**.

### Bare names

A pattern with no `*` and no `/` matches as either:

- The full identity path (exact), **or**
- The trailing segment after a `/` (leaf match).

That is, `rust-coding` matches identity `rust-coding` *and* `typescript/rust-coding`.
Bare names preserve the pre-nested-identity UX: users typing `rust-coding` continue to
mean "the skill called rust-coding" without having to learn paths.

### Path-anchored patterns

A pattern containing `/` matches as exact-path (no leaf-fallback). `typescript/coding`
matches only `typescript/coding`.

### Glob patterns

A pattern containing `*` is an explicit glob, multi-match by design:

| Pattern        | Matches                                                       |
| -------------- | ------------------------------------------------------------- |
| `*`            | every discovered identity                                     |
| `*/coding`     | every identity ending in `/coding` (multi-segment paths only) |
| `rust-*`       | every identity matching `rust-*` (as a single path segment)   |
| `**`           | same as `*` — accepted, not special                           |

Multi-match is **not** an ambiguity error. If the user wants single-target semantics,
they type a bare name or a path-anchored pattern.

`?` and character classes are not supported. At the CLI entry (`ace skills include` /
`ace skills exclude`) an unsupported pattern is a hard error. At the resolver boundary — `ace.toml` selection and
`[[imports]]` patterns, which may be third-party authored — it is instead warned and
skipped, keeping resolution infallible.

### Errors echo user input verbatim

The user sees their own pattern back when a match fails or yields nothing — verbatim,
never a normalized or internal rendering of it.

### Type-safety invariant

Identity and selection pattern live in distinct slots. Identity is a typed value
constructed only by discovery; a selection pattern stays a plain string, validated at the
resolver boundary. The resolver cannot accept a raw string where an identity is required,
so a pattern can never be mistaken for — or minted into — an identity. The separation is
structural (the signatures), not a dedicated handle newtype.

## `ace.toml` fields

The three skill-selection fields live in user / project / local `ace.toml`. Full
behavior, layering, and CLI surface are documented in
[configuration.md § Skills Selection](../configuration.md#skills-selection). This
section names the cross-cuts that matter for the model.

- `skills`           — last-wins replace across user / project / local. Empty at every
                       scope leaves the base set as "all discovered skills."
- `include_skills`   — union across scopes.
- `exclude_skills`   — union across scopes.

Every pattern in these fields is interpreted as a match handle per the rules above.

Resolution:

```
effective = (skills_base − exclude_skills) ∪ include_skills
```

Include is authoritative when an item appears in both: a skill explicitly named in
`include_skills` will be loaded even if a matching pattern in `exclude_skills` would
have removed it.

Selection runs over the **validated** set. Validation partitions the discovered skills into
admissible and rejected before any matching (see
[model.md § Name Admission](model.md#name-admission)), so an inadmissible skill is never a
selection candidate — `skills_base` is the admissible-and-discovered set.

## `[[imports]]` schema

`[[imports]]` in `school.toml` declares which upstream sources the authored school
pulls skills
from, and which skills it accepts from each source.

### Canonical shape

```toml
[[imports]]
source         = "owner/repo"
skills         = ["pattern", "pattern", ...]
exclude_skills = ["pattern", ...]          # optional
include_experimental = false               # optional
include_system       = false               # optional
include_internal     = false               # optional
```

- `source`               — GitHub `owner/repo` shorthand for the upstream skills repo.
- `skills`               — list of match handles selecting which discovered skills to
                           import. Required (or its backcompat alias `skill`).
- `exclude_skills`       — list of match handles subtracted from `skills`. Optional.
                           Suppresses cross-source collision warnings — see
                           [Cross-source merge](#cross-source-merge).
- `include_experimental` — when `true`, the discovery filter widens to include
                           `skills/.experimental/`. Default `false`.
- `include_system`       — same, for `skills/.system/`. Default `false`.
- `include_internal`     — when `true`, skills with `internal: true` in frontmatter
                           are admitted via glob matches. Explicit-name patterns always
                           bypass the internal filter (mirrors skills.sh). Default
                           `false`.

A declaration with neither `skills` nor `skill` is an error.

### Backcompat alias

The existing singular `skill = "<pattern>"` is accepted as an alias for
`skills = ["<pattern>"]`. Liberal accept; on load it is folded into `skills`
immediately, so any rewrite (incl. `ace fmt` round-trips) emits the plural
form and never the legacy key. Per [CLAUDE.md § Backcompat](../../../CLAUDE.md),
`school.toml` keys are a public contract: the singular form continues to be *accepted*
in any minor / patch release — only its emission is dropped.

### Per-skill resolution

For each `[[imports]]` declaration:

1. Discover skills in the source via the cascade in [model.md](model.md#discovery-cascade).
2. Apply tier expansion: `Curated` is always included; `Experimental` /
   `System` join if their flag is set; `internal: true` skills join if explicit-name or
   `include_internal = true`.
3. Match each pattern in `skills` against the expanded set. Explicit names also match
   skills in `Experimental` / `System` regardless of flag (mirrors skills.sh's
   explicit-bypass).
4. Subtract any matches in `exclude_skills`.

### Examples

Whole upstream:

```toml
[[imports]]
source = "ace-rs/school"
skills = ["*"]
```

Subset with subtraction:

```toml
[[imports]]
source         = "ace-rs/school"
skills         = ["*"]
exclude_skills = ["rust-coding"]

[[imports]]
source = "my/customizations"
skills = ["rust-coding"]
```

Tier widening:

```toml
[[imports]]
source = "owner/repo"
skills = ["*"]
include_experimental = true
```

## Cross-source merge

When two `[[imports]]` declarations resolve skills to the same identity path, ACE picks
the **first-declared import** as the winner and **warns** at every collision boundary.

### Why first-wins + warn

Under nested-path identity, cross-source collisions are common (paths like
`typescript/coding` are likely to recur). The right resolution is for the school
maintainer to express intent with `skills` / `exclude_skills`, not to let
declaration-order silently determine the outcome. First-wins is the deterministic
tiebreaker; the warning is the signal that intent is missing.

This matches skills.sh's stage-2 first-wins ordering — ACE adds the loud warning.

### Within a single source

Stage order is the tiebreaker (canonical `skills/` beats backend dirs; tier order
follows skills.sh). No warning — the source author already expressed precedence by
choosing where to land each skill.

### Warning boundaries

Warnings fire at two surfaces:

1. **`ace school pull`** — school maintainer's machine, during their own
   materialization.
2. **Consumer discovery** (`ace pull` / `ace setup` of a downstream project) — only
   if the school maintainer ignored their own warnings.

Warning text attributes the collision to the **school**, not the consumer ("the school
you're consuming has..."). Pressure stays on the maintainer to fix upstream.

### Frontmatter divergence

Identity collision plus divergent frontmatter `name` strings across the colliding
sources triggers an additional warning flagging the frontmatter mismatch as a likely
upstream spec violation.

### No new consumer-side suppression

Existing `exclude_skills` in `ace.toml` remains the consumer escape hatch — drop the
noisy skill, lose the warning. We deliberately do not add a consumer-side equivalent
of per-import `exclude_skills`; the goal is to keep pressure on the school maintainer
to express intent upstream.

## Provenance

Cross-source merge requires ACE to know, for every resolved skill, which
`[[imports]]` declaration produced it and which other declarations would also
have matched it absent exclusion. Provenance powers:

- **Collision warnings** that name both winner and loser source paths.
- **`exclude_skills` suppression** — when a would-be collider sits in the
  consuming declaration's `exclude_skills`, the warning is suppressed because
  the maintainer signalled intent.
- **Future `ace school explain <skill>`** — once authors ask for it, the
  resolver shape supports a per-skill trace analogous to `ace explain <skill>`
  at the project layer (see [configuration.md → CLI](../configuration.md#cli)).
  It lists each `[[imports]]` declaration considered, what matched, what was
  filtered, and which source won.

### Resolver shape

Import resolution mirrors the project-side resolver: discovered skills go
through pattern matching, filtering, and merge to produce a per-skill verdict
with a trace. The two resolvers are **siblings, not the same resolver**:

- **Project resolver** layers `ace.toml` fields (`skills` / `include_skills` /
  `exclude_skills`) across user / project / local scopes. See
  [configuration.md § Skills Selection](../configuration.md#skills-selection).
- **Imports resolver** merges across `[[imports]]` declarations within one
  `school.toml`. See this spec.

They share infrastructure where the shapes line up: the discovered → validated → decided
typestate progression, the trace-of-steps concept, the per-skill diagnostics
bag, the glob and filter utilities. They diverge where the layers differ — the
project resolver's scope taxonomy (user / project / local / school / override)
does not apply in the imports-resolver scope, where the imports resolver tracks
`[[imports]]` declaration index and source name instead. Verdict variants are
layer-specific: the project resolver decides included or excluded; the imports
resolver also distinguishes "lost to a higher-precedence declaration" to power
collision warnings.

### Type-safety invariant

Provenance is attached at the point a skill is selected from its source's
discovered set. Code outside the import-resolution layer cannot synthesize
provenance, nor consume a resolved skill without it — the invariant is
type-encoded.

Project-resolved and import-resolved skills are distinct kinds. A
project-resolved skill cannot be substituted where an import-resolved skill is
expected, and vice versa — the wrong-layer mix-up is unrepresentable.

## Collision and warning map

Every collision or conflict class, where it surfaces, and what ACE does:

| Conflict / collision                                          | Surfaces at                 | Action                          | Domain  |
| ------------------------------------------------------------- | --------------------------- | ------------------------------- | ------- |
| Bad-char **identity** segment (bidi / control)                | discovery (every cmd); import | reject + warn; import hard-refuses | ACE  |
| Spoofable **frontmatter** `name` (bad char / non-token)       | `import`, `school pull`     | warn + hint, still admitted     | ACE     |
| Dead selector — `skills`/`exclude_skills` matches nothing     | `school validate`, `pull`   | informational                   | ACE     |
| Selected skill was admission-rejected                         | `validate` / `pull`         | warn (asked for, refused)       | ACE     |
| Same identity path from two sources                           | school tree at `pull` (git) | first-declared wins + warn      | ACE     |
| **Flat collapse** — nested paths share a leaf on a flat-only backend | flat-emit sim at `validate`/`pull`; emit | warn + drop loser  | ACE     |
| `frontmatter.name` ≠ `basename(identity)`                     | `school validate`           | warn — spec hygiene, not security | ACE   |
| Backend keys/invokes on `frontmatter.name`                    | —                           | none — verbatim passthrough     | Backend |
| Folder unsupported (e.g. `rules/` on Codex)                   | emit / sync                 | informational                   | ACE     |

The flat-collapse row **cannot** be a git-diff fact — the colliding paths are distinct
on disk. ACE computes it by simulating flat emit over the identity set ("do any two
identities share a leaf?") and warns at `validate` / `pull`, shifted left from the
consumer's emit so the author who can fix it actually sees it.

**Warning principle.** ACE *warns* only on what it decides silently and automatically —
admission rejecting a name, or a requested skill dropped by that rejection. Anything the
author typed explicitly (including a selector that matches nothing) is at most
*informational*.

## Out of scope

- **Lockfile / pinning** — see
  [overview.md § Versioning Philosophy](../overview.md#versioning-philosophy).
- **Plugin namespacing** in match patterns (e.g. `plugin-name:skill-name`) — see
  [model.md § Out of scope](model.md#out-of-scope).
