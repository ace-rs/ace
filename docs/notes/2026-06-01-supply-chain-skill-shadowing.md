# Supply-chain skill shadowing — provenance via namespaced storage

Status: **research / design draft**, not ratified. Resume target for a fresh session.

This is the **single, consolidated note** for the skill-shadowing / collision-spoof /
name-takeover thread. On 2026-06-01 it absorbed and replaced four predecessor notes:
`2026-05-26-skill-collision-analysis.md` (collision matrix + upstream field evidence),
`2026-05-26-consumer-side-collision-followup.md` (consumer-side warning gap),
`2026-05-30-skill-name-admission-plan.md` (name-admission — *shipped*, see State of play),
and the two resolved skills.sh-compat checkpoints
(`2026-05-25-skills-sh-import-questions.md`, `2026-05-26-skill-refactor-session-state.md`,
whose rulings live in the 2026-05-26 decision docs). The only sibling note still standing
is `2026-05-25-skills-sh-spec-reference.md` — a frozen upstream spec snapshot, kept as a
reference.

Supersedes and reframes the memory note `pending-collision-spoof` (2026-05-27), which
framed this as an emit-time alphabetical-tiebreak bug. That framing was **wrong**; see
"The threat" and "Why emit cannot defend it" below.

---

## State of play (read this first)

A fresh session must not re-open work that already shipped. Three threads ran in parallel;
only one is still open.

### Shipped — do NOT redesign

- **skills.sh / agentskills.io compatibility refactor.** 2-stage discovery cascade
  (direct skill → priority dirs, no recursive fallback); **identity = post-strip
  source-relative path** (discovery-prefix dirs like `.claude/skills/` are stripped, not
  part of identity); school storage lands skills under `<school>/skills/<identity>/`;
  backend emit = **flatten with loser-drop + loud warning** (not path-prefix); resolver
  with first-wins + collision warnings + a provenance field.
  - Decisions: `docs/decisions/2026-05-26-skill-discovery-identity-storage.md`,
    `docs/decisions/2026-05-26-skill-emit-and-match.md`.
  - Code: `src/skills/{discover,identity,mod}.rs`,
    `src/actions/project/link_skills.rs`, `src/skills/resolver/project.rs`.
  - Commits: `7039c2d` (2-stage cascade) → `f7ba781` (SkillId + MatchHandle newtypes) →
    `4e92bce` (bare-name leaf-match) → `edc7b7a` (emit rule + sanitization + pull.rs
    walk-up) → `9bf2d44` (resolver: first-wins + collision warnings + **provenance**) →
    `958f3a6` (capability-driven emit, `FEATURE_NESTED_SKILLS`).

- **Name-admission policy** (the *other* security concern — see the distinction below).
  Whitelist / default-deny / **fail-closed** over Unicode general categories
  (`L/M/N/P/S/Zs` admissible; all `C*`, `Zl`, `Zp` denied, including unassigned `Cn`);
  `SanitizedString` newtype for display boundaries (U+FFFD transform); import
  hard-refuse (skip + warn; `ace school pull-imports` exits non-zero on skip); rejection
  modelled as a terminal state on the discovery decision enum, backend-independent.
  - Decision: `docs/decisions/2026-05-30-skill-name-admission-policy.md`.
  - Code: `src/skills/name/` (absorbed the old `sanitize.rs`); `enum Status` in
    `src/skills/mod.rs` (admission × selection collapsed into one axis).
  - Commits: `abfc402` (whitelist, fail-closed) → `b2d1ace` (admission orthogonal to
    selection) → `7402042` (comment softening) → `2c4ad97` (Status enum).
  - Tracking: PROD9-226 ("static detection, ≥1 class blocks by default" facet).
  - **Done.** Do not relitigate the whitelist (see memory `feedback-whitelist-failclosed`
    for the recurring denylist-drift failure mode).

### Open — this note

The **supply-chain name-takeover / shadowing** defense via **namespaced import storage**.
Not ratified, not implemented. Everything below "TL;DR" is about *this*.

### Admission vs. shadowing (don't conflate — the most common reasoning error here)

Two orthogonal security concerns, two separate fixes:

| | Admission (shipped) | Shadowing (this note) |
| --- | --- | --- |
| Attack | malicious **characters** in a name (bidi, control, escape) | a **legit-looking name** taken over by a compromised source |
| Defense | char-class whitelist at discovery (fail-closed) | provenance via namespaced storage (containment) |
| Layer | per-name predicate | per-source storage path |

Admission stops a name from containing dangerous codepoints. It does **nothing** against a
clean, spec-valid name like `general-coding` being *served by the wrong source*. That is
the gap this note closes. The two compose; neither subsumes the other.

---

## TL;DR (the stand we landed on)

The "collision-spoof" is a **supply-chain** problem, not an emit bug. It **cannot** be
defended at the backend-emit boundary. The only sound fix is to give each import source
its own namespaced storage in the school, so **provenance is encoded in the path**:

```text
<school>/skills/                         # authored skills — highest priority, always win
<school>/imports/<owner>/<repo>/skills/  # per-source — contained to its own dir
```

A compromised source can only ever write under its own `imports/<owner>/<repo>/`. It can
never take over an authored name or another source's name. Containment is structural
(fail-closed by layout), not a runtime check that can be forged.

## The threat (correct framing)

A school legitimately imports a third-party skill repo via `[[imports]]`. Later that repo
is **compromised upstream** (maintainer account taken over, malicious PR merged, a
transitive import typosquatted). Because ACE's versioning philosophy is **"don't version"**
(the LLM-text × backend × tool-version matrix is unwinnable — see
`docs/spec/index.md` § Versioning Philosophy), there is no pin: the compromised version
auto-propagates on the next `ace school pull`.

Attack payload: the compromised skill sets `frontmatter.name` (or its own folder name) to
the name of an **authoritative** skill the school trusts (`general-coding`, the `ace*`
family, etc.). On a flat backend it then wins the emit tiebreak and **silently replaces**
the trusted skill — the agent runs attacker instructions under a trusted name.

This is NOT "the author imported a bad skill." The import was trustworthy when added; the
compromise happened upstream afterward. Blaming the author is like blaming a 0-day server
breach on the operator. The no-version stance is a deliberate trade, and this is its
exposed flank.

Why shadowing is worse than "a malicious new skill": a *new* bad skill is visible and
auditable; a *takeover* of a known-good name is camouflaged, and it lets a **low-value**
compromised import escalate to hijack a **high-value** name it never legitimately owned.
That cross-source escalation is the thing worth stopping.

## Where this sits in the collision space

The full collision space (from the absorbed collision-analysis note), two axes —
on-disk **path** × **frontmatter name**:

| # | Path | Name | Scenario | Class |
| - | ---- | ---- | -------- | ----- |
| 1 | same | same | two sources, identical layout | emit collision (same dest) |
| 2 | same | different | same path, divergent frontmatter | emit collision + name divergence |
| 3 | different | same | distinct paths, shared display name | display/emit ambiguity |
| 4 | different | different | trivially distinct | none |

Rows 1–3 are *benign collisions* and are already handled by the shipped resolver
(first-wins + warn) and emit (loser-drop + warn). **Shadowing is the adversarial
weaponization of row 3**: a compromised source deliberately ships a *different* path whose
emitted name equals an authoritative skill's, so it competes for — and can win — that
name's backend slot. The benign machinery treats it as just another collision; nothing in
that machinery can tell the attacker's `general-coding` from the real one. That blindness
is the vulnerability.

## Why emit cannot defend it (dead ends — don't revisit)

- **Provenance is erased at emit.** `Skill.source: Option<String>` (`owner/repo` for
  imports, `None` for school-own) is populated only during `ace school pull` accumulation
  (`from_discovered_with_source` in `pull_imports`). The consumer path
  (`link_skills::prepare` → `Skills::discover` → `from_discovered`) tags **everything**
  `source: None`. By the time `build_desired` runs the collision tiebreak, the authoritative
  skill and the impostor are indistinguishable.
  - *Re-verify before building:* commit `9bf2d44` added a provenance field to the
    **resolver** (`src/skills/resolver/project.rs`). Confirm what provenance actually
    survives to the emit boundary in *current* code before assuming the erasure above is
    still total — the fix may have a shorter path than the note's original framing implies.
- **Every signal at emit is forgeable.** Both the folder basename and the `frontmatter.name`
  come from the (possibly hostile) import source. A "folder-owner-wins" rule fails: the
  spoofer just names its leaf folder to match, becoming a co-owner, and the tiebreak falls
  back to alphabetical — which it can win.
- **ACE can't win at backend resolution either**, and shouldn't try: backends key
  differently (see table) and ACE passes frontmatter through verbatim (ratified). Mutating
  frontmatter to disambiguate is off the table.

## The two questions that force the conclusion

1. **"How do you define school-authored?"** — Undefinable without persistent provenance.
   Imports are currently copied *into* `<school>/skills/` next to hand-written skills, no
   origin marker. A stateless derivation ("names no current import provides = authored")
   breaks: drop an import and its stale skill (ACE never deletes — additive writes) gets
   promoted to "authored"; and an authored skill can't be told apart from a *different*
   import's skill. No stateless trick exists.
2. **"What if two imports collide?"** — No un-forgeable winner; both names are equally
   source-controlled. The only principled tiebreak is `[[imports]]` declaration order,
   which defends nothing by itself.

Both questions converge on the same result: **a sound defense needs provenance.** The
cleanest place to keep provenance — consistent with ACE's path-as-identity model and its
"no manifest, intentionally dumb" stance (which was scoped to *versioning*, not security) —
is the **storage path itself**, not a separate ledger file.

## The decision (namespaced storage)

Stop flattening imports into `skills/`. Write each import under
`<school>/imports/<owner>/<repo>/skills/<identity-path>/`. Then:

- **Authored** = lives under `<school>/skills/`. Defined by location, not inference.
- **Two-import collision** = two distinct on-disk paths; neither can overwrite the other,
  neither can touch authored space. They collide only on the *emitted* name, resolved by
  declaration order + a loud warning — same determinism as today, but now it **cannot reach
  across** namespaces.
- **Containment** = a compromised `owner/repo` can only write under its own dir. Takeover
  of authored or other-source names is structurally impossible.

### It reuses existing machinery (not a big new system)

Discovery already runs a priority cascade with first-found-wins (`seen: HashSet` in
`src/skills/discover.rs`, canonical priority order: `skills/.curated`, `skills/`,
`skills/.experimental`, `skills/.system`, then backend-fallback dirs). Add `skills/` as top
priority and the `imports/<source>/` dirs in declaration order; **author-wins and
first-import-wins fall out of the existing first-found-wins set for free**. The real change
is *where `ace school pull` writes* (and the discovery priority list).

### Honest limit (don't oversell)

This defends **name takeover / shadowing** (containment). It does **not** defend a source
shipping malicious content under a name it *legitimately owns* — undefendable under the
no-version philosophy, accepted the same way you accept a trusted distro pushing a bad
package. Content-trust of your chosen sources stays yours.

## Two orthogonal sub-decisions (came up here; NOT the security fix)

These are independent of the supply-chain fix and were left open:

1. **Honoring `frontmatter.name` on the flatten branch.** Keep it — but only for skills.sh
   installer parity and **spec-valid output**, not as a defense. Reasoning: ACE emits
   symlinks with verbatim frontmatter, so it controls the *dir* name but not the
   *frontmatter* name. Naming the dir from `frontmatter.name` yields **dir == name**
   (spec-compliant: agentskills.io mandates name == parent-dir-name). Naming from
   `basename(identity)` yields **dir ≠ name** for non-compliant skills — ACE would emit a
   spec violation. So honoring the name is the conservative-emit choice.
   - Open: drop the override entirely (`basename`-always, simpler, one identity model) vs.
     honor it. Would be settled by whether Claude Code *enforces* dir == name or merely
     invokes by frontmatter name — **unverified** (Claude loader is closed-source).
   - For compliant skills (the spec norm), both choices are byte-identical; only
     spec-violating skills differ.

2. **Over-depth flatten (`MAX_SKILL_DEPTH = 5`).** Retire it as an emit-flatten *trigger*.
   Today an over-deep skill on a nested-capable backend gets *relocated* to the top level
   (`a/b/c/d/e/f` → `f`), where it can collide — a surprising, undocumented wart.
   Reframe `5` as a **discovery/scan cap** (matches skills.sh stage-3 `maxDepth = 5` and
   Codex `MAX_SCAN_DEPTH`): skills past the cap are skipped + warned, not relocated.
   Consequence: the flatten branch collapses to one meaning — "this backend can't nest" —
   and nested-capable backends never touch it.
   - Open: skip-at-discovery (simplest; invisible to `ace skills`) vs. discover-but-
     don't-emit (keeps the internal model complete). Note: ACE discovery is currently
     **unbounded** depth (`walk_priority_dir` has no counter); the cap lives only at emit.

## Verified facts

Backend dedup keying — confirmed against live sources, not just a stale snapshot:

| Backend     | Dedup key             | Collision behavior              | Source (re-fetchable)                                              |
| ----------- | --------------------- | ------------------------------- | ----------------------------------------------------------------- |
| skills.sh   | frontmatter `name`    | first-wins, **silent** drop     | `vercel-labs/skills` `src/skills.ts:200-220` (`seenNames.has`)     |
| OpenCode    | frontmatter `name`    | last-wins + warn                | `sst/opencode` `packages/opencode/src/skill/index.ts:126-135`     |
| Codex       | path (`AbsolutePath`) | different paths → both kept     | `openai/codex` `codex-rs/core-skills/src/loader.rs:196-225`        |
| Claude Code | dir name + scope tier | flat-only load; source closed   | leaked v2.1.88 `yasasbanukaofficial/claude-code` `src/skills/loadSkillsDir.ts:415` + docs |

**Three different identity keys across four consumers.** No standard beyond the spec's
`name == dir-name` mandate. Codex's path-as-identity is closest to what ACE ratified.

agentskills.io spec (`/specification`, fetched live): `name` is 1–64 chars, lowercase
`a-z0-9` + hyphens, no leading/trailing/consecutive hyphens, and **"Must match the parent
directory name."** The spec is **silent** on resolving two skills that share a `name` at
different paths — explicitly consumer territory.

Code (current `main`):

- `build_desired` (`src/actions/project/link_skills.rs`): flatten link name =
  `frontmatter_name || basename(identity)`; candidates sorted by identity; first-in-order
  wins per link name; loser dropped + warned. Structural checks on the synthesized name
  (slash, backslash, dot-segment, leading-dot, NUL, > 255 bytes).
- Nested branch (`FEATURE_NESTED_SKILLS`, depth ≤ `MAX_SKILL_DEPTH = 5`): emits verbatim at
  identity path, **ignores `frontmatter.name`**, no collision check. Backend masks:
  Claude = 0, Flaude = 0, Codex/OpenCode = `FEATURE_NESTED_SKILLS`.
- `merge()` during pull (`src/skills/mod.rs`): last-wins by identity (`s.name`); spec says
  imports converge to the latest regardless of declaration order. `copy_into` writes by
  identity path; imports land flat under `<school>/skills/<identity>/`.

## Upstream field evidence (why collision is unsolved everywhere)

From searching the relevant repos — confirms the tension is real and unsolved in practice,
which is *why* ACE has to pick its own model rather than inherit one.

**Claude Code** (collision handling has multiple dedup pipelines that disagree on which
copy "won"):

- **#43003** (closed) — local personal skills don't suppress anthropic-skills; duplicates
  in the skill list. Invocation precedence works; listing is cosmetically broken.
  *Scope-precedence is not consistently applied across surfaces.*
- **#43297** (closed) — a marketplace plugin skill gets *silently mapped* to the official
  plugin skill instead of registering separately. *Even with explicit namespacing, silent
  collisions happen at the consumer layer.*
- **#59423** (open) — "N skill descriptions dropped" for duplicate `SKILL.md` discovered
  via two paths (marketplace cache vs install cache); breaks description-budget bookkeeping.
- **#42384** (closed) — duplicate skills in slash-command autocomplete.
- **#29520** (closed) — plugin skills duplicated in `/context` report + system prompt.
- **#25994** (closed) — skills loaded twice after context compaction (111 instead of ~63).
- Flat-only loader corroboration (for the "must flatten at backend" ruling): #28266,
  #39138, #40640, #18192, #20805, #16438.

**agentskills.io spec** (ecosystem hasn't agreed either):

- **#115** (open) — proposal for path-based recursive discovery with **deepest-path-wins**
  precedence for same-named skills. This is essentially the model ACE converged on; Codex
  already implements path-as-identity. Same-name-different-path is explicitly addressed but
  not ratified.
- **#137** (open) — clarify whether nested skills are allowed (spec is silent).
- **#30** (open) — `foldername.md` as an alternative discovery pattern.
- **#46** (open) — support versioning/locking. *The ecosystem has no agreed versioning
  mechanism* — which is exactly the gap the no-version stance accepts and this note's
  containment fix works around.

**Conclusion:** every consumer chose its own identity model; multiple have surface-level
bugs from inconsistent dedup; the spec itself has open proposals (#115) that would address
this but aren't accepted. ACE's path-as-identity + flatten-with-loser-drop matches where
the spec is moving and avoids the Claude Code-style "multiple dedup pipelines" trap.

## Open questions / next steps

- **Consumer-side collision warnings (deferred gap).** `docs/spec/skills/selection.md`
  § Warning boundaries mandates two surfaces. (1) Maintainer-side — `ace school
  pull-imports` — **shipped** (`surface_import_diagnostics` in
  `src/actions/school/pull_imports.rs`, plus the resolver's collision warnings, `9bf2d44`).
  (2) Consumer-side — `ace pull` / `ace setup` of a downstream project, "only if the school
  maintainer ignored their own warnings" — **not implemented.** The materialized
  `<school>/skills/` already encodes the first-wins outcome (one copy per identity), so a
  consumer can't observe the collision after the fact. Restoring the surface needs either:
  *replay the resolver* locally (re-clone declared `[[imports]]`, re-run `resolve_imports`
  — heavyweight, network on every `ace pull`), or a *cached warning manifest* (school
  writes e.g. `<school>/.ace/collisions.toml` at pull-imports time; consumer reads +
  surfaces — lightweight but adds a manifest contract ACE deliberately avoids). Deferred:
  ship maintainer-side, document the gap, watch for real reports. **Revisit when** a
  real school ships unresolved cross-source collisions downstream, OR any other
  consumer-side validation surface (doctor checks, health pings) lands and would batch
  naturally, OR plugin/lockfile work (out of scope) brings a manifest contract back.
  *Note:* the namespaced-storage fix in this note changes the cross-source containment
  story — re-evaluate whether the consumer-side gap still matters once imports are
  namespaced, since cross-source name reach is structurally removed.

- **Migration.** Imports already sit flat in `skills/` for existing schools. Per CLAUDE.md
  storage-migration rule: detect-and-hint, not silent auto-migration. How to detect
  pre-namespace layout and what to tell the author.

- **Spec edits.** `docs/spec/skills/emit.md` § "School storage layout" currently says
  imports live under `<school>/skills/<identity-path>/` — must be rewritten for the
  `imports/<source>/` namespace. `selection.md` / `model.md` discovery cascade text too.

- **skills.sh-direct consumers.** skills.sh's stage-2 priority list includes `skills/` but
  not `imports/`; a `npx skills add <ace-school>` may miss namespaced imports. The "valid
  source, not equivalent" stance (emit.md § Downstream skills.sh compatibility, P2) likely
  covers this, but confirm the degradation is acceptable.

- **Import precedence.** Confirm declaration-order **first-wins** (containment) vs. the
  current last-wins merge; the `merge()` last-wins semantics may need to change or be
  re-scoped to within-a-single-source.

- **school.toml round-trip.** Check the namespace doesn't disturb the singular→plural fold
  / `ace fmt` round-trip.

- Settle the two orthogonal sub-decisions (frontmatter.name override; depth cap) — they can
  ship independently of the security work.

- **Scope reminder:** this is a `ace school pull` + storage + discovery change (pull-time
  boundary), **bigger than the emit-only scope** originally set. That scope was why earlier
  emit-side attempts kept failing.

### Implementation starting points (code map for a fresh session)

- **Where pull writes imports today** (the thing that has to change): `Skills::merge` +
  `copy_into` in `src/skills/mod.rs`; `pull_imports` / `from_discovered_with_source` in
  `src/actions/school/pull_imports.rs`. Imports currently land flat under
  `<school>/skills/<identity>/`; target is `<school>/imports/<owner>/<repo>/skills/...`.
- **Discovery priority cascade:** `walk_priority_dir` + the `seen: HashSet` first-found-wins
  in `src/skills/discover.rs`. Add `skills/` (top) + `imports/<source>/` dirs (declaration
  order).
- **Emit / flatten / collision tiebreak:** `build_desired` in
  `src/actions/project/link_skills.rs`.
- **Provenance:** `Skill.source: Option<String>` (`src/skills/mod.rs`), set by
  `from_discovered_with_source`, dropped by `from_discovered`; resolver provenance added in
  `9bf2d44` (`src/skills/resolver/project.rs`) — confirm current reach.
- **Identity types:** `SkillId` + `MatchHandle` newtypes in `src/skills/identity.rs`
  (`Skill.name: String → SkillId` migration is in-flight but incomplete).

## Sources

- **Kept companion reference:** `docs/notes/2026-05-25-skills-sh-spec-reference.md` —
  frozen agentskills.io spec + skills.sh discovery cascade / predicate / sanitization
  snapshot. Re-read for the upstream contract details this note assumes.
- **Ratified decisions:** `docs/decisions/2026-05-26-skill-discovery-identity-storage.md`,
  `docs/decisions/2026-05-26-skill-emit-and-match.md`,
  `docs/decisions/2026-05-30-skill-name-admission-policy.md`.
- **Specs:** `docs/spec/skills/emit.md`, `model.md`, `selection.md`; `docs/spec/index.md`
  § Versioning Philosophy.
- **Code:** `src/skills/{discover,identity,mod}.rs`, `src/skills/name/`,
  `src/skills/resolver/project.rs`, `src/actions/project/link_skills.rs`,
  `src/actions/school/pull_imports.rs`.
- **Live (re-fetch if stale):** `agentskills.io/specification`,
  `github.com/vercel-labs/skills` (`src/skills.ts`).
- **Memory:** `pending-collision-spoof` (this note supersedes its framing),
  `third-party-skills-constraint`, `feedback-whitelist-failclosed`.
