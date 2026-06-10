# ACE Roadmap Consolidation — 2026-06-09

Snapshot analysis of the full PROD9 / project ACE backlog. Source of truth stays in
Linear; this is a one-time grouping + dedup pass to make 72 unlabelled backlog items
reasonable to plan against. Local cache of the raw pull lives in `.roadmap-cache/`
(gitignored).

**Counts at snapshot:** 72 Backlog · 53 On Production · 2 Canceled · 2 Duplicate.
No labels in use anywhere — the backlog is a flat list, which is why it reads as noise.

---

## 1. Act-on-these-first (the actionable output)

### Close / verify-done

| Issue   | Why                                                                          |
| ------- | ---------------------------------------------------------------------------- |
| PROD9-74 | Body says "Code work complete," pipeline committed `197765f`; sibling PROD9-84 (Windows runtime) already On Production. Verify the release path actually ships a Windows binary, then close. Currently mis-filed as open High. |

Already handled (no action): PROD9-22 and PROD9-38 are marked **Duplicate**; PROD9-22
was the dup of the live PROD9-69 (`ace switch`).

### Merge candidates (collapse before working)

| Merge                    | Recommendation                                                                 |
| ------------------------ | ------------------------------------------------------------------------------ |
| **124 ⊇ 195**            | Near-duplicate. Both = "school declares checks / required commands, ACE runs them at setup-or-doctor with an install hint." 124 frames it as `[[requires]]`, 195 as `[[checks]]`. One feature. Keep one, fold the other's notes in; host the runner in **123** (`ace doctor`). |
| **236 ⊇ 215 + 244**      | All three mutate the same `maybe_offer_learn` skill-count prompt. 236 (3-way menu) is the redesign; 215 (stop re-prompting after a no) and 244 (don't fire on `ace school pull`) are corrections that the redesign should absorb. Make 236 the parent and fold 215/244 as acceptance criteria — or ship 215/244 as quick bug-fixes *now* and let 236 supersede. Ties to the pending learn-threshold note. |
| **234 + 68 (+235, 228)** | "Generalise sync beyond skills." 68 (extend imports to rules/commands/agents) and 234 (first-class `agents/` sync) overlap directly; 235 (`plugins/`) and 228 (unified backup across backend folders) are the same generalisation. Promote to one epic; the skills-only scope decision is the gating doc to supersede (235 already calls this out). |
| **56 + 15**              | Both are "fill integration-test coverage." Consolidate into one test-strategy item rather than two parallel audits. |

### Newly unblocked / re-scoped by shipped work

- **PROD9-65 (wildcard imports) shipped** → **66** (document wildcard + parent-school
  pattern on the website) is now actionable, and **187** (`*` import silently shadows
  explicit decls) is a *follow-up bug* on the shipped feature, not net-new design.
- **PROD9-18 (Codex backend) shipped** → backend-completion track is down to **17**
  (OpenCode) and **122** (Droid).
- **PROD9-129 (custom `[[backends]]`) shipped** → **146** (scope-aware selector
  validation) is the follow-up bug it created.
- **PROD9-128 (per-backend env) shipped** → **147** (API-key/login conflict) and **197**
  (model tiers) build directly on it.
- **PROD9-76 (cache imported repos) shipped** → reduces urgency of **121** (parallelize
  fetches) and reframes **67** (git-based import).

### Real High-priority work after the above

243 (import merge bug) · 64 (stdio MCP) · 216 (stale-symlink spam) · 17 (OpenCode) ·
122 (Droid). PROD9-74 drops off once verified-closed.

---

## 2. Logical groups

Fifteen clusters. `[Pn]` = priority (P2 High … P4 Low; P0 = no-priority/research).
`†` marks an open research spike rather than committed work.

### A. Backends — completion & normalization

The core "make every backend a first-class peer" track.

| Issue | [P] | Note                                                          |
| ----- | --- | ------------------------------------------------------------ |
| 17    | P2  | Complete OpenCode backend (MCP reg, readiness, server list)  |
| 122   | P2  | Complete Droid backend (no `--system-prompt`; design blocker)|
| 198   | P3  | `supports_trust` per-backend trust-level validation          |
| 146   | P3  | Scope-aware backend selector validation (follow-up to 129)   |
| 147   | P3  | `ANTHROPIC_API_KEY` vs claude.ai login conflict              |
| 197   | P3  | Backend model tiers (primary vs cheap/fast)                  |
| 119   | P3  | gitignore block should enumerate *all* backends from registry|
| 127   | P3  | `ace upgrade` should also upgrade configured backends        |
| 149†  | P4  | Survey Ollama `launch` integration list as input             |

### B. MCP

| Issue | [P] | Note                                                          |
| ----- | --- | ------------------------------------------------------------ |
| 64    | P2  | Support stdio MCP servers in school.toml (today HTTP-only)   |
| 199   | P0  | `[[mcp]]` decls at project/user/local layers (mirror backends)|
| 225   | P3  | `ace mcp list` — lightweight, side-effect-free               |
| 237†  | P0  | School ships a Dockerfile, ACE builds + boots it as MCP      |
| 34†   | P3  | ACE itself as an MCP server inside the backend (hot-reload)  |

### C. Skill imports & supply chain

| Issue | [P] | Note                                                          |
| ----- | --- | ------------------------------------------------------------ |
| 243   | P2  | `ace import` merges into existing `[[imports]]` skills= (bug)|
| 242   | P0  | `ace import` picker should be multi-select                   |
| 187   | P3  | `pull-imports` `*` silently shadows explicit decls (65 bug)  |
| 226   | P3  | Supply-chain safety checks (static scan + LLM audit)         |
| 121   | P3  | Parallelize import-source fetches in `ace school pull`       |
| 155†  | P4  | Nested-school propagation — rethink double-upstream path     |
| 67†   | P4  | Git-based import (subtree/merge) instead of file copy        |
| 70    | P4  | Handle deleted upstream skills (dangling symlinks)           |
| 66    | P3  | Document wildcard imports + parent-school pattern (unblocked)|

### D. Resource sync beyond skills  *(merge into one epic — see §1)*

| Issue | [P] | Note                                                          |
| ----- | --- | ------------------------------------------------------------ |
| 234   | P3  | First-class `agents/` sync (partial wiring exists)           |
| 68†   | P4  | Extend imports to rules/commands/agents folders              |
| 235   | P4  | First-class `plugins/` sync (supersede skills-only decision) |
| 228   | P0  | Unified backup for pre-ACE content across backend folders    |

### E. Skill selection & token budget

| Issue | [P] | Note                                                          |
| ----- | --- | ------------------------------------------------------------ |
| 120   | P3  | Per-repo skill selection to limit token usage                |
| 134   | P4  | Token-compress kept skill content at link time (builds on 120)|

### F. `ace learn` & skill-count prompting  *(215+244 fold into 236 — see §1)*

| Issue | [P] | Note                                                          |
| ----- | --- | ------------------------------------------------------------ |
| 151   | P3  | Backend-driven two-diff capture (CLAUDE.md + ace.toml skills)|
| 236   | P0  | Skill-count offer: 3-way menu (manual / auto-learn / skip)   |
| 215   | P3  | Stop re-prompting `ace learn` after a no                     |
| 244   | P0  | Don't prompt on `ace school pull` — only on session start    |

### G. School lifecycle: switch / eject / edit-detection

43 is the building block for 69 — sequence eject → switch.

| Issue | [P] | Note                                                          |
| ----- | --- | ------------------------------------------------------------ |
| 216   | P2  | Detect ace.toml school edit, stop spamming stale-symlink warn|
| 69    | P3  | `ace switch` — change project school                         |
| 43    | P0  | `ace eject` — unlink a school (building block for 69)        |
| 33    | P3  | Treat dirty school cache as the default working state        |

### H. Environment health, setup & onboarding  *(124⊇195 — see §1)*

| Issue | [P] | Note                                                          |
| ----- | --- | ------------------------------------------------------------ |
| 123   | P3  | `ace doctor` / `ace check` — env health command (hosts checks)|
| 195   | P3  | School-pluggable env checks + recommendations                |
| 124   | P3  | school.toml declares required CLI commands w/ AI install flow|
| 72    | P3  | Initial-setup module for non-technical users / junior devs   |
| 10†   | P4  | School scripts for machine/software setup                    |

### I. Upgrade / self-update

| Issue | [P] | Note                                                          |
| ----- | --- | ------------------------------------------------------------ |
| 60    | P3  | Check for new versions on startup; hint, don't auto-update   |
| 127   | P3  | `ace upgrade` upgrades backends too (also listed in §A)      |
| 161   | P4  | `ace upgrade` rollback to an earlier version                 |

### J. Programmatic / headless / multi-run

The "ACE as more than an interactive entrypoint" theme.

| Issue | [P] | Note                                                          |
| ----- | --- | ------------------------------------------------------------ |
| 245   | P0  | `ace serve` — normalize headless/serve mode across backends  |
| 159   | P3  | Polymorphic flags for backend ops (one-shot prompt, model…)  |
| 156†  | P4  | Multi-backend fork/compare runs (`ace mux` / `split`)        |
| 32†   | P4  | `ace tunnel` — SSH tunnel for remote terminal access         |

### K. CLI ergonomics & inspection

| Issue | [P] | Note                                                          |
| ----- | --- | ------------------------------------------------------------ |
| 44    | P0  | Pipe `ace diff` through pager on TTY                         |
| 190   | P4  | Global `--yes` / `ACE_YES=1` auto-confirm                    |
| 160   | P4  | `ace --bare` — start backend with no skills/school           |
| 227   | P4  | `ace template` — render builtin prompt templates to stdout   |
| 13    | P4  | `ace llm-help` — AI-friendly CLI guidance                    |
| 225   | P3  | `ace mcp list` (also listed in §B)                           |

### L. Testing & internal quality  *(56+15 consolidate — see §1)*

| Issue | [P] | Note                                                          |
| ----- | --- | ------------------------------------------------------------ |
| 56    | P3  | Design + plan live backend integration tests                 |
| 15    | P3  | Fill integration-test coverage gaps                          |
| 36    | P3  | Simplify flaude: diagnostics to stdout, not JSONL file       |
| 150   | P3  | Hide flaude from user-facing help/docs (feature-flag)        |
| 152   | P3  | `ace pull` misreports tier folder as the changed skill (bug) |
| 154   | P4  | Reconsider per-binding error variant naming (Config→TreeLoad)|
| 241   | P4  | Surface discovery structural prunes in read-only paths       |

### M. Docs & templates

| Issue | [P] | Note                                                          |
| ----- | --- | ------------------------------------------------------------ |
| 200   | P3  | Expand school CLAUDE.md template with skill-authoring guide  |
| 66    | P3  | Document wildcard imports (also §C; unblocked by 65)         |

### N. Research spikes (decide go/no-go before building)

A natural "research track" — none committed, all P0/no-priority. Batch a spike session.

| Issue | Question                                                       |
| ----- | ------------------------------------------------------------- |
| 240   | Are backend exit codes reliable enough to drive error hints?  |
| 239   | Should ACE synchronize backend memory files?                  |
| 238   | Provision ACE skills into Claude Desktop?                     |
| 237   | School Dockerfile → ACE-built MCP container? (also §B)        |
| 34    | ACE as MCP server inside the backend? (also §B)               |
| 149   | Survey Ollama `launch` (also §A)                              |

### O. Big bets / out-of-core scope

| Issue | [P] | Note                                                          |
| ----- | --- | ------------------------------------------------------------ |
| 77†   | P4  | Convert ACE to a Tauri desktop app (ties to 72)              |
| 158†  | P4  | Hangar/Tower — ancillary coding-velocity tooling             |
| 126   | P4  | Auto-spawn tmux side pane with editor/diff on session start  |
| 19    | P3  | School-defined roles — spec removed; redesign before reviving|

---

## 3. Observations for planning

- **The High tier is small and clean.** After verifying 74 closed, real Highs are 243,
  64, 216, 17, 122 — two bug-fixes (243, 216), two backend completions (17, 122), one
  capability (64). That's a coherent next-milestone slice.
- **No labels = no slicing in Linear.** The fifteen clusters above map naturally to
  Linear labels (`backends`, `mcp`, `imports`, `learn`, `school-lifecycle`, `env-setup`,
  `upgrade`, `headless`, `cli-ux`, `testing`, `docs`, `spike`, `big-bet`). Applying them
  would make the backlog filterable without any further consolidation.
- **Research spikes (§N) are blocking nothing but clutter the priority view.** Either
  schedule one spike-batch session or move them to a separate "research" state so they
  stop reading as un-triaged work.
- **The "generalise beyond skills" pull (§D) is the biggest latent epic** — four issues
  circling the same architectural decision (does ACE sync only skills, or all four
  backend resource folders?). Worth a decision doc before any of 234/68/235/228 is picked
  up, since they share one design.
