# ACE skill lifecycle — design summary

Status: **design draft**, 2026-06-01. Consolidates the supply-chain / shadowing thread
into the decided direction: **a skill's name is its path**. Companion to the rendered
note `2026-06-01-skill-lifecycle.html` (same content, browser-openable).

Companions:

- `docs/notes/2026-06-01-supply-chain-skill-shadowing.md` — the threat analysis this
  supersedes the framing of.
- `docs/decisions/2026-05-26-skill-discovery-identity-storage.md`,
  `docs/decisions/2026-05-26-skill-emit-and-match.md` — the ratified base.
- `docs/spec/skills/{model,selection,emit,sync}.md` — the per-stage specs.

## 1 — The lifecycle

A skill's path from source repo to backend, with the collision-prone stages flagged.

| # | Stage                      | On disk                          | Collision risk                                                       |
| - | -------------------------- | -------------------------------- | ------------------------------------------------------------------- |
| 1 | Source repo                | `skills/foo`, `skills/ts/coding` | none — just a `SKILL.md` dir, flat or nested                         |
| 2 | Import + select (ACE)      | discovery · admission · imports  | bad-char name (reject); dead selector; selected-but-rejected        |
| 3 | Save to school `skills/`   | `<school>/skills/<identity>`     | **dup path** — two sources, same identity → first-wins, git-visible |
| 4 | Consumer pull / setup      | symlinks into the project        | minor — dead `ace.toml` selector; foreign file name clash           |
| 5 | Backend emit               | nested verbatim, or flat collapse| **Claude collapse** — distinct nested paths share a leaf on flat    |

Stages 1 and 4 are inert. Stages 2 and 3 are author-side conflicts ACE warns on. Stage 5
holds the only emit-time collision, and only on flat-only backends.

## 2 — What warns, and where

| Conflict / collision                                          | Surfaces at                 | Action                          | Domain  |
| ------------------------------------------------------------- | --------------------------- | ------------------------------- | ------- |
| Bad-char name (bidi / control)                                | discovery (every cmd); import | reject + warn; import hard-refuses | ACE  |
| Dead selector — `skills`/`exclude_skills` matches nothing     | `school validate`, `pull`   | informational                   | ACE     |
| Selected skill was admission-rejected                         | `validate` / `pull`         | warn (asked for, refused)       | ACE     |
| Same identity path from two sources                           | school tree at `pull` (git) | first-declared wins + warn      | ACE     |
| **Claude collapse** — nested paths share a leaf on flat-only  | flat-emit sim at `validate`/`pull`; emit | warn + drop loser  | ACE     |
| `frontmatter.name` ≠ `basename(identity)`                     | `school validate`           | warn — spec hygiene, not security | ACE   |
| Backend keys/invokes on `frontmatter.name` (OpenCode, skills.sh, Claude slash) | —          | none — verbatim passthrough     | Backend |
| Folder unsupported (e.g. `rules/` on Codex)                   | emit / sync                 | informational                   | ACE     |

The Claude-collapse row **can't be a git-diff fact** — the colliding paths are distinct on
disk. ACE computes it by simulating flat emit over the identity set ("do any two
identities share a leaf?") and warns at `validate` / `pull`, shifted left from the
consumer's emit so the author who can fix it actually sees it.

**Warning principle.** ACE *warns* only on what it decides silently and automatically —
admission rejecting a name, or a requested skill dropped by that rejection. Anything the
author typed explicitly (including a selector that happens to match nothing) is at most
*informational*: they said it; a vacuous match is their business.

## 3 — The decision, and what it kills

**Emit name = `basename(identity)`, always.** Drop the `frontmatter.name ||` channel.
Identity is the path; the path is the only naming axis. Frontmatter is display-only — the
backend's to interpret. A name takeover can then only be a *path* collision, which is
visible in the author's own git tree. Settles the shadowing note's open sub-decision #1
toward basename-always.

Rejected, and why:

| Approach                    | Why not                                                                          |
| --------------------------- | -------------------------------------------------------------------------------- |
| Namespaced storage          | Defends the name as a trust boundary — it isn't one. Heavy machinery, non-threat. |
| Authored / source wins      | Same non-boundary; needs provenance to survive to emit, which it doesn't.        |
| TOFU / change-detect        | Re-introduces the pinning no-version rejected; content changes every pull anyway. |
| Strict agentskills storage  | Backends read ACE's emit, not `<school>/skills/`; storage is decoupled from compat. Buys nothing, costs third-party intake. |
| Honor frontmatter at emit   | The forgeable second channel — source of the shadow vector and divergence. Byte-identical for compliant skills anyway. |

**The boundary.** ACE owns the path and warns on every path-level conflict at author
time; the backend owns the frontmatter. Shadowing collapses into source-trust (the
trusted-distro model ACE already accepts), so nothing structural gets built — the
machinery is git plus a handful of `validate` / `pull` warnings.

## 4 — skills.sh compat and impact on the current corpus

The decision is **consumer-emit-side** (how ACE names dirs under `<backend>/skills/`); it
does not touch school storage, which still passes frontmatter through verbatim. So the
skills.sh-direct compat surface — `npx skills add <our-school>` — is unchanged.

Measured against the three local school clones (read 2026-06-01):

| School             | Skills | Nested | `name ≠ basename` | Non-kebab | Impacted |
| ------------------ | -----: | -----: | ----------------: | --------: | -------: |
| prod9/school       |     31 |      0 |                 0 |         0 |        0 |
| forwardinsight     |     31 |      0 |                 0 |         0 |        0 |
| naxon-ai           |     27 |      0 |                 0 |         0 |        0 |

Every skill is flat (single-segment), `name == basename`, and kebab-clean. So for our
corpus `name = basename(identity)` is a **provable no-op**: emit is byte-identical, no
Claude-collapse is possible (collapse needs two paths sharing a leaf), no divergence
warnings fire, and there is nothing to migrate. Full skills.sh parity — a direct
`npx skills add` sees the same compliant flat layout it does today.

The decision only bites on shapes **absent from our schools**:

- **Nested-category repos** (a `skills/<lang>/<skill>` path layout): on a flat-only
  backend (Claude) two skills sharing a leaf collapse onto one dir. The skill still works;
  its leaf collides with a sibling. Caught by the flat-emit lint.
- **`name ≠ dir` spec-violators**: frontmatter-keyed backends (skills.sh, OpenCode) key
  on the divergent name. Surfaced by the divergence warning. Rare — agentskills.io
  mandates `name == dir`, so only off-spec skills hit this.

### External source repos (cached imports, read 2026-06-01)

Checked against cached clones of popular ecosystem repos — including the canonical
`anthropics/skills` and `vercel-labs/skills` — not just our own schools. (No
`mattpocock/skills` clone is present locally or in memory; these are the popular ones we
do have.)

| Source repo                   | Discovered | Nested | name ≠ basename | Collapse | Notes                                                       |
| ----------------------------- | ---------: | -----: | --------------: | -------: | ----------------------------------------------------------- |
| anthropics/skills             |         16 |      0 |               0 |        0 | `template/` (name `template-skill`) is off-spec at repo root — ACE's no-stage-3 cascade never scans it |
| vercel-labs/skills            |          1 |      0 |               0 |        0 | flat                                                        |
| coreyhaines31/marketingskills |         43 |      0 |               0 |        0 | flat                                                        |
| pproenca/dot-skills           |       ~195 |      0 |               0 |        0 | tiers `.curated`/`.experimental` (stripped → flat); backend dirs + `plugins/` correctly not scanned; one within-source tier collision (`radical-simplification`, curated wins) |
| chakrit/kien-thai             |          2 |      0 |               0 |        0 | flat                                                        |
| bentossell/visualise          |          1 |      0 |               0 |        0 | stage-1 direct skill                                        |
| mattpocock/skills (fetched)    |         29 |     29 |               0 |        0 | category-nested `skills/<cat>/<skill>`; leaves unique so no drop, but categories flatten away on Claude |

`mattpocock/skills` **is** the real nested-category repo: 29 skills under
`skills/{engineering,productivity,misc,personal,deprecated,in-progress}/<skill>`, so the
nested shape does occur in the wild — just not in anything our schools import. Even there
every leaf is unique, so nothing collapse-collides today; the only flat-backend (Claude)
effect is the category segment being dropped (`engineering/tdd` → `tdd`) — the ratified
flatten, not a bug. A future leaf clash across two categories (e.g. adding `personal/review`
next to `in-progress/review`) is exactly what the flat-emit lint guards. `name ==
basename(leaf)` holds throughout, so the divergence axis stays empty — across the whole
~280-skill external corpus the only `name ≠ basename` is `anthropics` `template-skill`,
which sits in an off-spec location ACE never scans. Every other cached repo categorizes
via tier dirs (`.curated`/`.experimental`, stripped to flat) or flat `skills/<name>/`.

`pproenca/dot-skills` is the stress case — ~195 skills across `.curated`, `.experimental`,
`plugins/`, and duplicated `.claude/skills/` + `.agents/skills/` copies — and it confirms
the cascade: backend-dir duplicates and `plugins/` are correctly skipped when canonical
tiers are present, so no cross-prefix collision arises.

Bottom line: `name = basename(identity)` changes **no emitted name anywhere measured** —
schools and ecosystem repos, mattpocock included — because `name == basename` holds
universally. Flattening itself still happens for nested repos (mattpocock's categories drop
on flat backends); collapse and divergence remain real-but-currently-harmless shapes the
flat-emit lint and divergence warning cover.
