# ACE Consolidation — Plane Seed Blueprint (2026-06-13)

**Supersedes** `2026-06-09-roadmap-consolidation.md`. Built on that note's 15 clusters,
then re-checked against the **full descriptions + comments** in
`2026-06-13-linear-ace-catalog.json` (134 issues). This is the structure to seed **Plane**
with — not a flat import of 134 issues.

This is a blueprint. **No edits to Linear** (it's being abandoned). Decisions flagged
🔸 are chakrit's calls — they're made here as defaults so the structure is complete, but
walk them for sign-off before seeding Plane.

---

## 0. Scope decisions (locked with chakrit, 2026-06-13)

Plane uses a **5-state** status model; Linear had 4. The migration remaps:

| Plane status | Definition                    | Source                                              |
| ------------ | ----------------------------- | --------------------------------------------------- |
| BACKLOG      | ideas dump                    | Linear Backlog items that are raw ideas / spikes    |
| PLANNED      | details fleshed out           | Linear Backlog items with settled design + done-when|
| MERGED       | code in-tree on main          | *empty today* — transient lane before a release tag |
| RELEASED     | usable in a released version  | all 53 Linear "On Production"                       |
| CANCELLED    | cancelled / dropped           | 2 Canceled + 2 Duplicate                            |

- **MERGED is transient, starts empty.** Head past `v0.8.1` is docs-only; nothing sits
  merged-but-unreleased. All On Production → RELEASED.
- **Epics (Modules) span every status** — status is an orthogonal axis. Each epic reads as
  a full narrative (shipped + pending + cancelled members).
- The Backlog→{BACKLOG, PLANNED} split is the judgment layer (§2).

---

## 1. Status remap — the BACKLOG vs PLANNED split

53 RELEASED + 4 CANCELLED are mechanical (§5). A further 3 Linear-Backlog items were found
already-done and reclassified RELEASED in the 1-by-1 walk (60, 74-impl, 125 → §5), so the
counts below describe the remaining genuinely-open work. The 77 Linear-Backlog items split as
follows. **PLANNED** = clear problem, settled approach, concrete done-when, ready to pick
up. **BACKLOG** = idea/spike/"explore later", open design questions dominate, or gated on
a decision not yet made.

### PLANNED (30) — fleshed out, ready to work

| ID  | Pri | Title (short)                                      |
| --- | --- | -------------------------------------------------- |
| 247 | High| Path-traversal in `[[imports]].source` (security)  |
| 243 | High| `ace import` merge into existing block (bug)       |
| 216 | High| Detect ace.toml school edit, stop stale-symlink spam|
| 64  | High| stdio MCP servers in school.toml                   |
| 17  | High| Complete OpenCode backend                          |
| 122 | High| Complete Droid backend (decision: prompt strategy) |
| 187 | Med | `*` import shadows explicit decls (65 follow-up bug)|
| 146 | Med | Scope-aware backend selector validation (129 f/u)  |
| 147 | Med | ANTHROPIC_API_KEY vs claude.ai login conflict      |
| 119 | Med | gitignore block enumerates all backends from registry|
| 120 | Med | Per-repo skill selection (token budget)            |
| 121 | Med | Parallelize import-source fetches                  |
| 123 | Med | `ace doctor` env health command                    |
| 124 | Med | school.toml `[[requires]]` declared CLI deps (⊇195)|
| 151 | Med | `ace learn` two-diff capture                       |
| 152 | Med | `ace pull` misreports tier folder as changed skill |
| 195 | Med | School-pluggable env checks → folds into 124       |
| 200 | Med | Expand school CLAUDE.md template (authoring guide) |
| 215 | Med | Stop re-prompting `ace learn` after a no           |
| 225 | Med | `ace mcp list` — lightweight, side-effect-free     |
| 236 | —   | Skill-count offer: 3-way menu (absorbs 215, 244)   |
| 244 | —   | Don't prompt learn on `ace school pull`            |
| —   | —   | Selection UX improvement (merges 242 + 253 + 236's select arm; tag `tui-multiselect`)|
| 44  | —   | Pipe `ace diff` through pager (was borderline → PLANNED)|
| 66  | Med | Document wildcard imports (unblocked by 65)        |
| 36  | Med | Simplify flaude: diagnostics to stdout             |
| 150 | Med | Hide flaude from user-facing help (feature-flag)   |
| 69  | Med | `ace switch` — change project school               |
| 43  | —   | `ace eject` — unlink a school (building block for 69)|
| 198 | Med | `supports_trust` per-backend validation            |

### BACKLOG (44) — ideas / spikes / gated

(PROD9-74 is excluded from both lists — it's a verify-done special case, §5.)

Research spikes: **240, 239, 238, 237, 34, 149** + **246** (idea-stage, "explore later").
Architectural-but-gated (4): **234, 68, 235, 228** (all gated on the skills-only
scope-decision supersede — §3). Big bets (4): **77, 158, 126, 19**. Idea-stage / not-yet-
designed features: **245** (ace serve — epic, needs spike), **199, 159, 156, 32, 160, 13,
227, 190, 214, 127, 161, 134, 155, 67, 70, 33, 226, 72, 10, 52, 154, 241, 252**, plus the
test-strategy pair **56 + 15**, the merged **"backend model config"** ticket (⊇197+248),
**verify ACE on a real Windows box** (74 split-off), and **investigate `skills.json` format**
(nextlevelbuilder/ui-ux-pro-max-skill).
(Removed: 248, 253 → merged tickets; 44 → PLANNED; 60, 125 → RELEASED §5; 197 → merged.)

**Borderline calls — resolved 2026-06-13 (1-by-1 walk):**
- **44** (pager) → **PLANNED** (trivially clear, zero design). Moved to PLANNED table.
- **70** (deleted upstream skills) → **BACKLOG** — needs DX/UX + deletion-detection
  discussion before it's pickup-ready.
- **60** (self-update) → **RELEASED** — verified shipped in `src/upgrade/` (see §5).
- **199** (`[[mcp]]` layers) → **BACKLOG** — needs merge-semantics planning first.
- **33** (dirty cache default) → **BACKLOG**.
- **226** (supply-chain checks) → **BACKLOG** — needs detailing.
- **125** → **RELEASED** (verified, §5).

New BACKLOG item (filed during the walk): **investigate `skills.json` format** seen at
`github.com/nextlevelbuilder/ui-ux-pro-max-skill` — decide whether ACE should support it.

---

## 2. De-dup pass against full bodies — confirmed + new

The 2026-06-09 merge calls **all hold** against the full text. One **new** merge surfaced
from issues filed after that note (242/253), plus one new sibling pair (197/248).

### Confirmed from 2026-06-09

| Merge               | Verdict (full-body check)                                          |
| ------------------- | ------------------------------------------------------------------ |
| **124 ⊇ 195**       | Confirmed. 124 = school declares required CLI cmds; 195 = pluggable env checks; both feed the **123** `ace doctor` runner. One feature: 195 folds in as 124's "recommendations" arm; 123 hosts execution. |
| **236 ⊇ 215 + 244** | Confirmed. 236's body explicitly reasons about the 215 interaction and shares the `maybe_offer_learn` helper with 244. 236 is the redesign; 215/244 are corrections it should absorb — *or* ship 215/244 as quick fixes now and let 236 supersede. |
| **234+68+235+228**  | Confirmed — the biggest latent epic. All four circle one decision: does ACE sync only skills, or all four backend resource folders? Gated on superseding the frozen skills-only scope decision (235's body calls this out explicitly). → **Epic D**. |
| **56 + 15**         | Confirmed. Both = fill integration-test coverage. One test-strategy item. |

### New (from full bodies / post-note issues)

- **242 + 253 + 236's manual-select arm → MERGE** (signed off 2026-06-13). One
  **"selection UX improvement"** ticket covering the multi-select TUI pattern across `ace
  import` / mcp / skill prompts. *Not* parent/child — a single ticket (reverses the earlier
  relate-don't-merge default). 236 itself stays its own ticket (learn 3-way menu); only its
  manual-select arm folds in here. PLANNED (Cycle 2, inheriting 242's shippable status).
  Tag `tui-multiselect`.
- **197 + 248 → MERGE** (signed off 2026-06-13). One **"backend model config"** ticket —
  not parent/child. Design ratified:
  - Two opaque-passthrough slots: `model` + `model_secondary`. ACE forwards each as
    `--model <slug>` and never interprets the value → **no internal model list to maintain**
    (the deciding constraint: vendor model slugs churn too fast to track).
  - Internal/utility invocations (`ace learn` via `exec_one_shot`) forward `model_secondary`;
    interactive sessions forward `model`. "Tier" is just *which slot* a call uses.
  - **Dropped:** the ACE-resolves-tier (`Primary`/`Fast`) enum — that's the only part that
    would rot. ("fast" was also a misleading label: e.g. Claude's Fable 5 is fast *and*
    expensive.) 248's slug-pin = simply filling `model`. BACKLOG, Epic A.
- **246 + 245 + 160** (theme, not merge). 246 (transparent `claude`/`codex` shim), 245 (`ace
  serve`), 160 (`--bare`) are all alternate-entrypoint/invocation modes → **Epic G**.

### Re-scoped by shipped work (notes, not merges)

65→ unblocks 66, makes 187 a follow-up bug. 129→ 146 is its follow-up. 128→ 147 & 197 build
on it. 76 (import caching shipped)→ lowers urgency of 121, reframes 67. 75 (tiered discovery
shipped). 84+74 (Windows) → see §5.

---

## 3. Epics (Plane Modules) — span all statuses

12 modules. Consolidated from the 15 clusters (E+F merged; J+K's entrypoint items pulled
into G; upgrade folded toward A's backend story but kept as its own small module). Each
lists PLANNED/BACKLOG members; RELEASED members noted for narrative.

### A. Backends — completion & normalization
PLANNED: 17, 122, 146, 147, 119, 198 · BACKLOG: "backend model config" (⊇197+248), 127, 161, 149(spike)
RELEASED: 18, 128, 129, 54, 55, 157, 47, 35, 48, **60** (self-update)
→ The "every backend a first-class peer" track. 127/161 are the self-update sub-story
(60 itself now RELEASED).

### B. MCP provisioning
PLANNED: 64, 225 · BACKLOG: 199, 237(spike), 34(spike)
RELEASED: 48, 53, 42
→ 64 (stdio) is the High capability gap; 199 mirrors `[[backends]]` layering for `[[mcp]]`.

### C. Skill imports & supply chain
PLANNED: 243, "selection UX improvement"(⊇242, cross-H), 187, 121, 66 · BACKLOG: 226, 155, 67, 70
RELEASED: 65, 75, 76, 62
→ 247 (security) lives here conceptually but is pulled to Phase 1 (§4).

### D. Resource sync generalisation  *(architectural — gated)*
BACKLOG: 234, 68, 235, 228
→ **Gating step**: a new dated `docs/decisions/` entry superseding the skills-only scope
ruling (`project_skill_scope` memory). Do not start any member until that lands. Biggest
latent epic; one shared design.

### E. Skill selection & learn  *(was E + F)*
PLANNED: 120, 151, 236, 215, 244 · BACKLOG: 134
→ 236 absorbs 215+244. Ties to the pending "learn re-run threshold" note
(`project_pending_learn_threshold`). All converge on the `ace.toml` `skills=` write path.

### F. School lifecycle, setup & env health  *(was G + H)*
PLANNED: 216, 69, 43, 123, 124(⊇195), 195 · BACKLOG: 33, 72, 10, 52, 252, 199(cross)
RELEASED: 6, 30, 57, 71, 7, 14, 49, 73, **125** (init writes ace.toml)
→ 216 is High (stale-symlink spam). 124⊇195 host checks in 123. 72/10/52 = onboarding +
diagnostics. 252 = seed CLAUDE.md with repo-bootstrap hint. 125 now RELEASED (§5).

### G. Entrypoints & headless  *(was J + entrypoint bits of K)*
BACKLOG: 245(serve — epic/spike), 246(transparent shim), 159, 156, 32, 160
→ 245 needs a design spike + spec before build (its body says so). 246/160 are smaller
invocation modes.

### H. CLI ergonomics & inspection
PLANNED: 225(cross-listed B), 44(pager) · BACKLOG: 190, 214, 126, 13, 227
→ 242/253 merged into the "selection UX improvement" ticket (tag `tui-multiselect`, listed
in Epic C). 44 → PLANNED. 126 (tmux pane) borderline big-bet.

### I. Quality — testing & internals
PLANNED: 152, 36, 150, 198(cross-A) · BACKLOG: 56(+15), 154, 241
RELEASED: 37, 131
→ 56+15 = one integration-test-strategy item.

### J. Docs & templates
PLANNED: 200, 66(cross-C) · BACKLOG: 13(cross-H)
RELEASED: 25, 28, 58, 59, 31, 191

### K. Research spikes  *(go/no-go gate — schedule one batch)*
BACKLOG: 240, 239, 238, 237, 34, 149
→ None committed. Either batch one spike session or hold in a dedicated "research" state so
they stop reading as un-triaged work.

### L. Big bets / out-of-core
BACKLOG: 77 (Tauri app), 158 (Hangar/Tower), 126 (tmux), 19 (roles — spec removed, redesign
first; see `project_roles_removed`)

---

## 4. Phases (Plane Cycles)

### Cycle 1 — next milestone: security + High bugs + backend completion
The coherent, mostly-scoped slice. **247 first** (security, fully planned with TDD steps in
its body), then the High tier:

**247** (path-traversal security) · **243** (import-merge bug) · **216** (stale-symlink) ·
**64** (stdio MCP) · **17** (OpenCode) · **122** (Droid — needs the prompt-strategy decision
up front).

### Cycle 2 — medium follow-ups + the quick learn/import fixes
**215 + 244** (ship as quick fixes ahead of 236) · **187** · **146** · **147** · **152** ·
**"selection UX improvement"** (⊇242+253+236-arm) · **44** (pager) · **66** · **119** ·
**124(⊇195)** + **123** (env-health pair) · **198**.

### Cycle 3 — architectural epics (each needs a decision/spec first)
**Epic D** (sync generalisation — supersede scope decision) · **236** (learn 3-way menu) ·
**120** (skill selection) · **245** (ace serve — design spike) · **69 + 43** (switch/eject) ·
**226** (supply-chain checks).

### Unscheduled / icebox
Epic K (spikes — go/no-go gate) · Epic L (big bets) · low-priority ergonomics
(190, 214, 13, 227, 126, 134, 155, 67, 70, 33, 161, 246, 156, 32, 199, 127) · verify ACE
runs on a real Windows box (74 split-off, hardware-gated) · investigate `skills.json` format
(nextlevelbuilder/ui-ux-pro-max-skill).
(Removed from icebox: 44 → Cycle 2; 60, 125 → RELEASED; 242/248/253 → merged tickets.)

---

## 5. Verify-done / close list

- **PROD9-125 → RELEASED** (verified 2026-06-13). `src/actions/school/init.rs:65-71` writes
  `ace.toml` with `school = "."` when none exists, guarded by `if !ace_toml_path.exists()`
  (preserves an existing one). All three done-when boxes met. The "meta-school" note in the
  body was explicitly deferred — not part of done-when, doesn't block closing.
- **PROD9-60 → RELEASED** (verified 2026-06-13). `src/upgrade/` ships the whole self-update
  story: startup `check_for_update` with a TTL cache marker, offline-silent, `ace upgrade`
  command + background upgrade, Homebrew-managed detection (`brew upgrade ace` hint). Every
  done-when box met — and it goes *past* the ticket (auto-upgrades in background, where 60
  said "NOT auto-update"). If that overreach matters, file a separate small ticket.
- **PROD9-74 (Windows) → SPLIT** (signed off 2026-06-13). Implementation is complete +
  committed (`197765f`); pipeline + runtime portability (84) shipped & RELEASED → the
  **implementation half is RELEASED**. Carve out a thin **"verify ACE runs on a real Windows
  box"** task → **BACKLOG** (low pri, hardware-gated). Stop carrying 74 as an open *High*
  feature.
- **PROD9-131, PROD9-75** — already RELEASED with closing comments confirming done. Mechanical.

## 6. CANCELLED (4)

| ID | Title                          | Disposition                                      |
| -- | ------------------------------ | ------------------------------------------------ |
| 9  | Investigate Cursor/Continue/Cline | CANCELLED — superseded by custom-backends (129)  |
| 26 | Homebrew tap                   | CANCELLED — but Homebrew shipped via 194; archive as done-by-other |
| 22 | `ace switch` (dup)             | CANCELLED/duplicate — superseded by 55 + the live 69 |
| 38 | Global CLAUDE.md cross-backend | CANCELLED/duplicate                              |

**Signed off 2026-06-13: import all 4 as CANCELLED records** (preserves the rejection trail
— keeps "why we dropped Cursor support" etc. queryable instead of re-litigated later).

---

## 7. Suggested Plane labels

`backends` · `mcp` · `imports` · `sync-generalisation` · `skill-selection` · `learn` ·
`school-lifecycle` · `env-setup` · `entrypoints` · `cli-ux` · `tui-multiselect` · `testing` ·
`docs` · `spike` · `big-bet` · `security`. These map 1:1 to the modules above and make the
backlog filterable without further consolidation.

---

## 8. 1-by-1 sign-off (2026-06-13)

All 🔸 flags walked and ratified with chakrit. Decisions:

1. **242 + 253 + 236-select-arm → MERGE** → one "selection UX improvement" ticket (not
   parent/child). PLANNED, Cycle 2, tag `tui-multiselect`. §2/§1/§3-C/§4.
2. **197 + 248 → MERGE** → one "backend model config" ticket. Opaque-passthrough slots
   `model` + `model_secondary`; internal calls use `model_secondary`; **no internal model
   list**; tier-enum dropped. BACKLOG, Epic A. §2.
3. **Borderlines:** 44 → PLANNED · 70 → BACKLOG · 60 → RELEASED · 199 → BACKLOG · 33 →
   BACKLOG · 226 → BACKLOG. New BACKLOG item: investigate `skills.json` format.
4. **PROD9-125 → RELEASED** (verified `init.rs:65-71`).
5. **PROD9-74 → SPLIT:** impl RELEASED; thin "verify on a real Windows box" → BACKLOG.
6. **Cancelled 9/26/22/38 → import as CANCELLED records.**

## Housekeeping

- **Revoke the Linear personal API key** — chakrit handling himself (not tracked here).
- Catalog JSON + refetch script stay untracked (migration scratch).
- The parent migration (Linear→Plane) is still parked on the plane.so deploy.
