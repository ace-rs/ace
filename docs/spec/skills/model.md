# Skill Model

What a skill IS in ACE: how it's discovered, what makes it unique, what its frontmatter
means, and what sanitization rules apply at its edges. Companion specs cover which skills
get picked ([selection.md](selection.md)) and where they land ([emit.md](emit.md)).

Grounded in two decisions:

- [2026-05-26 — Skill discovery, identity, storage](../../decisions/2026-05-26-skill-discovery-identity-storage.md)
- [2026-05-26 — Skill emit, sanitization, frontmatter](../../decisions/2026-05-26-skill-emit-and-match.md)

ACE is compatible with [agentskills.io](https://agentskills.io) and skills.sh. skills.sh
is the looser superset, so "compatible with both" effectively means "compatible with
skills.sh's predicate."

## Discovery Cascade

Given a source directory (a repo clone, a school root, an import cache), discovery yields
a list of skills via a **2-stage** cascade. Each stage returns immediately if it finds
anything; later stages do not run.

### Stage 1 — direct skill

If `<root>/SKILL.md` exists, the root itself is a skill. Identity defaults to the basename
of the source repo (or the `[[imports]]` declaration's key when one is given).

### Stage 2 — priority dirs (recursive within)

Walk priority dirs in order. Within each dir, walk **recursively at any depth** looking
for `SKILL.md` files. First-found wins on collisions across stages.

1. `skills/.curated/` — curated tier
2. `skills/` — canonical (curated, default)
3. `skills/.experimental/` — experimental tier
4. `skills/.system/` — system tier
5. Backend-specific dirs (**fallback only** — used when 1–4 yield nothing):
   `.claude/skills/`, `.codex/skills/`, `.agents/skills/`, `.opencode/skills/`,
   `.cursor/skills/`, … (skills.sh's full priority list)

Backend-specific dirs are a fallback so a repo that ships only in `.claude/skills/` still
imports cleanly. A repo that ships both `skills/foo/` (curated) and `.claude/skills/foo/`
(an installed copy) yields only the curated one.

### Tier dirs are a community convention

`.curated/`, `.experimental/`, `.system/` are conventions skills.sh recognizes and ACE
honors. **ACE does not own them** — they describe how the source repo organized itself,
not ACE-internal categorization.

### What's deliberately excluded

skills.sh's stage 3 (whole-repo recursive `maxDepth=5` walk) is **not adopted**. Skills
outside stage 1 or stage 2 priority dirs are off-spec and out of ACE's import surface. A
source repo that wants its skills picked up must land them at the root (stage 1) or under
a priority dir (stage 2).

Recursive walking *within* a priority dir is in scope — that's how nested layouts like
`skills/typescript/coding/SKILL.md` are discovered.

### `internal` filter

Skills with `internal: true` in frontmatter are filtered out at discovery time, unless an
explicit-name import names them (mirrors skills.sh). The per-`[[imports]]` flag
`include_internal` widens the filter for glob matches — see
[selection.md](selection.md#imports-schema).

## Identity

A skill's identity is the path at which it was discovered, with the **longest matching
discovery prefix stripped**. Discovery-location dirs describe how the source organized
itself; they are not part of identity.

Known prefixes (longest-match wins):

- `skills/.curated/`, `skills/.experimental/`, `skills/.system/`
- `skills/`
- Every backend-specific dir from the priority list (`.claude/skills/`, `.codex/skills/`,
  …)

| Source path                         | Identity                                               |
| ----------------------------------- | ------------------------------------------------------ |
| `skills/foo/SKILL.md`               | `foo`                                                  |
| `skills/typescript/coding/SKILL.md` | `typescript/coding`                                    |
| `skills/.curated/bar/SKILL.md`      | `bar`                                                  |
| `.claude/skills/foo/SKILL.md`       | `foo`                                                  |
| `.codex/skills/typescript/coding/…` | `typescript/coding`                                    |
| Stage-1 `SKILL.md` at root          | `<repo-basename>` (or `[[imports]]` key when supplied) |

Identity is what ACE keys off internally. Two skills with the same identity collide;
collision handling lives in [selection.md](selection.md) (cross-source) and
[emit.md](emit.md) (backend emit).

### Frontmatter `name` is not identity

Path-based identity is the one shape every consumer agrees on. The ecosystem disagrees on
what frontmatter `name` *means*:

- agentskills.io mandates `name == basename(parent dir)`.
- skills.sh does not enforce that.
- Claude Code treats `name` as the slash-command token.
- Other backends ignore it or treat it as display-only.

Using `name` as identity would couple ACE's internal model to whichever backend's
semantics it picked. ACE keys off the path, and the emitted directory name is
`basename(identity)` too (see [emit.md](emit.md#backend-emit-rule)) — so `name` is **never
an emit or match key**. It is display metadata, passed through verbatim for whatever
per-backend purpose the backend assigns it (e.g. Claude's slash-command token).

### Type-safety invariant

A skill moves through three lifecycle stages — **discovered**, **validated**, and **decided**
(its selection resolved) — and the type system encodes that ordering so a wrong-stage
operation cannot be written.

- **Identity is typed, structurally sound by construction, and carried end-to-end.** Identity
  is a distinct newtype, not a string: there is no implicit `String`→identity conversion, so a
  raw value cannot slip into an identity slot, and the typed identity rides every stage unchanged
  rather than collapsing back to a string. Construction goes through *fallible* doors that
  structurally validate every segment (non-empty, no NUL/slash/backslash, not a dot-segment or
  leading-dot, within length) — so holding an identity proves it is a sound path-component
  sequence. Structural failure is pruned at the door; it is not a skill verdict. Discovery is the
  sole production minter by convention (the prefix-strip rule is the only path that mints one);
  this is a layering convention, not a visibility wall. Character *admissibility* is a separate
  axis, not enforced at construction (the partition must be able to represent inadmissible
  identities to report them) — see **Validation** below. The dual holds for the other direction:
  a user-supplied selection pattern stays a plain string — validated at the resolver boundary,
  never minted into an identity (see [selection.md → Match handle](selection.md#match-handle)).
- **Validation is a gate, not a flag.** Validation *partitions* the discovered set into the
  admissible and the rejected (see [Name Admission](#name-admission)); only the admissible
  half advances. The boundaries then accept narrower proofs: **persist** takes any
  validated-or-later (vetted) skill, while **emit** takes only a *decided* one (it reads the
  resolved inclusion). An un-validated skill is **unrepresentable** at the moment of writing
  to disk, and an un-decided skill is unrepresentable at emit — "validate before you persist"
  is compiler-enforced, not a guard to remember.
  Validation re-runs from scratch every process, so this proves in-process ordering and stores
  nothing.

Concrete marker, trait, and identity-type names live in the
[lifecycle decision](../../decisions/2026-06-04-skill-lifecycle-typestate.md).

## Frontmatter

ACE reads SKILL.md frontmatter to extract metadata. Intake is **liberal**: the skills.sh
predicate (a string `name` plus a string `description`) is sufficient.

### Required fields

- `name` — string, non-empty.
- `description` — string. Single line or YAML block scalar (`>` or `|`).

### Honored fields (pass-through)

ACE preserves but does not interpret the rest of the frontmatter:

- `compatibility` — declarative metadata. ACE does not read or interpret it; it passes
  through verbatim to backends like any other field. See
  [emit.md § Frontmatter passthrough](emit.md#frontmatter-passthrough).
- `internal` — discovery-time filter. See [Discovery Cascade](#discovery-cascade).
- Claude-Code-extended fields: `when_to_use`, `argument-hint`, `disable-model-invocation`,
  `allowed-tools`, `model`, `effort`, `context`, `agent`, `hooks`, `paths`, `shell`.

Backends that don't recognize a field ignore it by spec convention. ACE does not translate
between variants.

### Spec violations are warnings, never errors

Liberal intake means ACE accepts skills that violate the agentskills.io spec — e.g.
`name != basename(identity)`, non-kebab-case names, length over 64, missing fields,
non-string types. These are surfaced as warnings, not rejected at
intake. (A doctor-check follow-up is tracked in the Outline **ACE** collection.)

Rationale: rejecting on parse forces the school author to upstream-fix every imported
violation before the skill can be used. Warning lets the import succeed; the violation
gets attention via doctor without blocking the school.

## Name Admission

Threat model: malformed or malicious frontmatter may carry terminal escape sequences
(CWE-150), bidi-override chars (U+202A–U+202E, U+2066–U+2069), or other display-spoofing
payloads. Rendering raw `name` / `description` to a terminal compromises ACE's display
surface. Admitting malicious names into the skill model also lets stale symlinks and
backend-specific materializations outlive later rule tightening.

### Boundary policy

| Boundary                                        | Action                                                 |
| ----------------------------------------------- | ------------------------------------------------------ |
| Discovery (every skill-touching command)        | Admit-predicate: reject bad name + warn                |
| Import (`ace import` / `ace school pull`)       | Additionally hard-refuse: skip bad skill + warn        |
| ACE's own display (prompts, listings, warnings) | Transform on render                                    |
| Emit / symlink name                             | Structural validation only (traversal / NUL / length)  |
| Backend file content                            | Nothing — symlink target content remains byte-for-byte |
| Internal model                                  | Validation partitions: admissible skills advance; rejects split off with their reason |

### Approach

**Unicode-class whitelist, fail closed.** A skill name is admissible iff every character
is in `L*` (letters), `M*` (marks), `N*` (numbers), `P*` (punctuation), `S*` (symbols), or
`Zs` (space separator), and the name is structurally valid as a path component. Everything
in `C*` is rejected, including `Cf` format characters, bidi controls, `Cn` unassigned
characters, and `Co` private-use characters. `Zl` and `Zp` are rejected too.

This is a whitelist, not an equivalent-looking denylist. Unknown future Unicode characters
must fail closed until ACE's committed Unicode table is regenerated.

### Identity coverage

Admission checks every identity segment. A bad character in any segment rejects the whole
skill; the old "import as-is + warn" tolerance for foreign path segments is superseded.
Identity is the path ACE owns and emits from (name = `basename(identity)`), so it is the
only admission axis. Frontmatter `name` is **not** an admission key: ACE passes it through
verbatim and never emits or matches on it, so it is the backend's domain — hostile
characters there are neutralized when ACE itself renders the field (see Display transform).
A spoofable frontmatter `name` does not reject the skill, but the authoring boundaries
(`ace import`, `ace school pull`) raise a non-fatal `warn` + `hint` so the importer sees it.
See [name = path decision](../../decisions/2026-06-01-skill-name-is-path.md). Admission is a
separate axis from config selection, settled at discovery: validation **partitions** the
discovered set, so rejected skills never enter the set selection and emit operate on. They
remain on disk (ACE deletes nothing) and surface as an explicit rejected set carrying each
rejection reason for diagnostics, but cannot be included or emitted regardless of what the
`skills`/`include`/`exclude` rules would have picked.

### Display transform

The same whitelist applies as a transform only when ACE renders untrusted text to its own
terminal. Each disallowed character renders as `U+FFFD`; rejection diagnostics also name
the offending codepoint and position. Backend SKILL.md content is not rewritten.

### Type-safety invariant

Discovery constructs identities; validation then **partitions** them — the admissible advance
to resolution, the rejected split off carrying their reason — so resolution and emit only ever
see admissible skills. Strings that cross ACE display boundaries carry a `SanitizedString`
marker built through the render transform. Internal model fields remain raw; rejection reasons
and display accessors sanitize internally before formatting untrusted content.

### Caveat

Consumers that bypass ACE (running skills.sh directly against an ACE-authored school) get
the unsanitized payload. They are outside ACE's protection envelope by their own choice.

## Out of scope

- **Plugin systems.** ACE does not parse Claude's plugin manifest or skills.sh's
  `pluginName` tagging. Skills from plugin-shaped repos are just skills.
  `plugin-name:skill-name` invocation grammar is not honored.
- **Subpath import.** skills.sh's `add https://github.com/owner/repo/tree/main/skills/foo`
  shape is rejected; not tracked.
- **Lockfile / pinning.** See
  [index.md § Versioning Philosophy](../index.md#versioning-philosophy).
- **Whole-repo recursive discovery** (skills.sh stage 3). See
  [What's deliberately excluded](#whats-deliberately-excluded).
- **Frontmatter translation** between backend variants. LLMs adapt; ACE doesn't.
