# ACE Epics & Roadmap — Outline seed (2026-06-24)

Target tool switched from Plane → **Outline** (a wiki, not a task tracker). Outline has no
issue states, no sprint board — just documents. So work is organized as **one document per
epic** (each holding its own task checklist) plus **one Roadmap document** that suggests
ordering.

Grouping + de-dups + verify-done closures are inherited wholesale from the ratified
`2026-06-13-consolidation-plane-seed.md` (which walked all 134 Linear issues 1-by-1). This
note only re-shapes that for a doc-based tool; it does not re-decide anything. The Plane
5-state status model is dropped — in Outline, `- [x]` = shipped, `- [ ]` = open, and an
*Ideas / later* sub-list holds spikes and gated work.

## How to use this (loose by design)

- Epics and the roadmap are **guides, not rules**. Work happens across several epics at any
  one time. Don't expect strict observance.
- **Releases are out-of-band** — decoupled from epic closure and from roadmap bands. Ship
  whenever a coherent slice is green; no milestone gate.
- The roadmap below is a *suggested priority*, nothing more. Reorder freely.

## Outline collection layout

```
Collection: ACE Development
├─ Roadmap                       (the section below)
├─ A — Backends
├─ B — MCP provisioning
├─ C — Skill imports & supply chain
├─ D — Resource sync generalisation   (gated)
├─ E — Skill selection & learn
├─ F — School lifecycle, setup & env health
├─ G — Entrypoints & headless
├─ H — CLI ergonomics & inspection
├─ I — Quality — testing & internals
├─ J — Docs & templates
├─ K — Research spikes
├─ L — Big bets / out-of-core
└─ Cancelled / superseded
```

`🆕` marks the 13 recovered ideas (`2026-06-23-recovered-idea-backlog.md`) — net-new, never
filed in Linear.

---

# Roadmap

A priority guide, not a commitment. Work spans epics; releases ship out-of-band.

## Now — hardening + High-priority gaps

The security fix and the High bugs/capabilities. These sit across epics A/B/C/F — that's
expected; do them as a loose front line, not a sequence.

- [ ] **247** path-traversal in `[[imports]].source` — security, do first (C)
- [ ] **243** `ace import` merges into existing block instead of appending a dup (C)
- [ ] **216** detect ace.toml school edit, stop stale-symlink spam (F)
- [ ] **64** stdio MCP servers in school.toml (B)
- [ ] **17** complete OpenCode backend (A)
- [ ] **122** complete Droid backend — needs prompt-strategy decision up front (A)

## Next — medium follow-ups + quick learn/import fixes

- [ ] **215 + 244** quick learn-prompt fixes (E)
- [ ] **187** `*` import shadows explicit decls (C)
- [ ] **146** scope-aware backend selector validation (A)
- [ ] **147** ANTHROPIC_API_KEY vs claude.ai login conflict (A)
- [ ] **152** `ace pull` misreports tier folder as changed skill (I)
- [ ] **selection-UX** multi-select TUI for import/mcp/skill prompts — `tui-multiselect` (C)
- [ ] **44** pipe `ace diff` through pager (H)
- [ ] **66** document wildcard imports (C/J)
- [ ] **119** gitignore block covers all backends at once (A)
- [ ] **124 (⊇195) + 123** required-CLI-deps declaration + `ace doctor` env health (F)
- [ ] **198** `supports_trust` per-backend validation (A)

## Later — architectural epics (each gated on a decision/spec)

- [ ] **Epic D** resource-sync generalisation — gated on superseding the skills-only scope
      decision (write the new `docs/decisions/` entry first)
- [ ] **236** skill-count 3-way menu (E)
- [ ] **120** per-repo skill selection / token budget (E)
- [ ] **245** `ace serve` — needs a design spike + spec before build (G)
- [ ] **69 + 43** `ace switch` / `ace eject` (F)
- [ ] **226** skill-import supply-chain checks (C)

## Icebox

Research spikes (Epic K — go/no-go gate), big bets (Epic L), low-priority ergonomics
(190, 214, 13, 227, 126, 134, 155, 67, 70, 33, 161, 246, 156, 32, 199, 127), verify ACE on a
real Windows box (74 split-off, hardware-gated), investigate `skills.json` format
(nextlevelbuilder/ui-ux-pro-max-skill).

---

# A — Backends

Every backend a first-class peer — completion, per-backend config, normalization.

- [ ] **17** complete OpenCode backend · *High*
- [ ] **122** complete Droid (Factory.ai) backend — needs prompt-strategy decision · *High*
- [ ] **146** scope-aware backend selector validation (129 follow-up)
- [ ] **147** `[[backends]]` ANTHROPIC_API_KEY env conflicts with claude.ai login
- [ ] **119** gitignore block enumerates all backends from registry
- [ ] **198** `supports_trust` per-backend trust-level validation

*Ideas / later*
- **backend model config** (⊇197+248) — two opaque passthrough slots `model` +
  `model_secondary`, forwarded as `--model <slug>`; no internal model list; internal calls
  use `model_secondary`
- **127** `ace upgrade` also upgrades configured backends · **161** support rollback to an
  earlier version
- **149** survey Ollama's `launch` integration list (spike)
- 🆕 per-backend config — always pass flags like `--chrome` to a backend (overlaps 128)
- 🆕 backend-agnostic `--chrome` flag — a uniform ACE flag that ACE translates to each
  backend's equivalent (flip side of per-backend config: runtime passthrough vs config-time
  always-pass; relates to 159 polymorphic flags)
- 🆕 more harness targets — Pi / Hermes / Cursor (note: Cursor was Cancelled-9, revisit)
- 🆕 shared config, varying harnesses — teams share one skills/mcp/scripting set while
  using different models/harnesses
- 🆕 built-in complex backend setup — e.g. codex's 3-process ace-connect, managed inside ACE

*Shipped:* 18, 128, 129, 54, 55, 157, 47, 35, 48, 60 (self-update).

# B — MCP provisioning

Provision MCP servers per-school, mirroring the `[[backends]]` story.

- [ ] **64** support stdio MCP servers in school.toml · *High*
- [ ] **225** `ace mcp list` — lightweight, side-effect-free (decouple from health/register)

*Ideas / later*
- **199** support `[[mcp]]` decls at project/user/local layers (needs merge-semantics design)
- **237** school ships a Dockerfile, ACE builds + boots it as an MCP server (spike)
- **34** ACE as an MCP server inside the backend (spike)
- 🆕 MCP health check is slow / probably broken in many places — the post-`mcp` check
  (overlaps 225)

*Shipped:* 48, 53, 42.

# C — Skill imports & supply chain

Importing skills from external repos + keeping that path safe.

- [ ] **247** path traversal via `[[imports]].source` in `ensure_source_cache` · *High,
      security* (roadmap: Now)
- [ ] **243** `ace import` merges into existing `[[imports]] skills=` instead of appending a
      duplicate block · *High*
- [ ] **187** `ace school pull-imports` silently shadows skills when a `*` import collides
      with explicit decls
- [ ] **121** parallelize import-source fetches in `ace school pull`
- [ ] **66** document wildcard imports + parent-school pattern on the website
- [ ] **selection-UX** multi-select TUI picker (⊇242 + 253) — tag `tui-multiselect`

*Ideas / later*
- **226** supply-chain safety checks (static scan + LLM audit)
- **155** rethink skill-import propagation across nested schools
- **67** explore git-based skill import instead of file copy
- **70** handle deleted upstream skills on school update
- 🆕 school import provenance — track which skill came from where, so the importer knows
  ownership (hit a case where an agent disowned a skill it authored, due to `*` imports)
- 🆕 selective `school pull` — pull only specific imports
- 🆕 **rethink the import model — no skill copy in the school** *(think later, hard)*. Since
  there are no lockfiles and provenance isn't tracked anyway, the school may not need any
  copied skill content at all (or a different structure) — resolve imports only at `ace pull`
  time, letting the user pick overrides then. Could delete a swath of copy-handling code.
  Needs a long, careful design pass before any move. Relates to the provenance idea above and
  to Epic D (sync generalisation).

*Shipped:* 65, 75, 76, 62.

# D — Resource sync generalisation  *(gated)*

The biggest latent epic. All four members circle one decision: does ACE sync only skills, or
all four backend resource folders (skills, agents, commands, rules)?

**Gating step:** write a new dated `docs/decisions/` entry superseding the skills-only scope
ruling (`project_skill_scope`). Do not start any member until that lands.

- [ ] **234** first-class `agents/` sync
- [ ] **68** extend imports to rules, commands, and agents folders
- [ ] **235** first-class `plugins/` sync (supersedes skills-only scope decision)
- [ ] **228** unified backup strategy for pre-ACE content across all backend folders

# E — Skill selection & learn

Everything converging on the `ace.toml` `skills=` write path + the learn prompt.

- [ ] **120** per-repo skill selection to limit token usage
- [ ] **151** `ace learn` backend-driven two-diff capture (CLAUDE.md + ace.toml skills)
- [ ] **236** skill-count 3-way menu — manual TUI-select / auto-learn / skip (absorbs 215, 244)
- [ ] **215** stop re-prompting `ace learn` after a no
- [ ] **244** don't prompt learn on `ace school pull` — only on session start

*Ideas / later*
- **134** skill filter: token-compress skill content at link time
- 🆕 `inject=` key — inject skill content (just `skill.md`) into the session prompt directly;
  useful for pre-loading e.g. ace-connect

Ties to the pending "learn re-run threshold" note (`project_pending_learn_threshold`): only
prompt on substantial school deltas, not single-skill edits.

# F — School lifecycle, setup & env health

Setup, school switching, and environment diagnostics.

- [ ] **216** detect ace.toml school edit, stop spamming stale-symlink warnings · *High*
- [ ] **69** `ace switch` — change project school
- [ ] **43** `ace eject` — unlink a school (building block for 69)
- [ ] **123** `ace doctor` — general environment health check
- [ ] **124 (⊇195)** school.toml declares required CLI commands + AI-guided install flow;
      195's pluggable env checks fold in as the "recommendations" arm, hosted by 123

*Ideas / later*
- **33** treat dirty school cache as the default working state
- **72** initial-setup module for non-technical users / junior devs
- **10** investigate school scripts for machine/software setup
- **52** log setup/sync failures for upstream reporting
- **252** `ace setup` seeds CLAUDE.md with an `/ace-init` (repo-bootstrap) hint
- 🆕 `--local` flag for `ace setup` — temporary workdir, don't commit `ace.toml`; put the
  school in `local.toml`

*Shipped:* 6, 30, 57, 71, 7, 14, 49, 73, 125 (init writes ace.toml).

# G — Entrypoints & headless

Alternate ways to invoke ACE — serve mode, transparent shim, remote.

*Ideas / later* (none committed yet)
- **245** `ace serve` — normalize headless/serve across claude/codex/opencode (epic; needs a
  design spike + spec before build)
- **246** transparent replacement of `claude` / `codex` (shim mode)
- **159** polymorphic flags for common backend operations (one-shot prompt, etc.)
- **156** multi-backend fork/compare runs (`ace mux` / `split`)
- **32** `ace tunnel` — SSH tunnel for remote terminal access
- **160** `ace --bare` — start backend with no skills/school
- 🆕 abstract harness — call `ace` inside scripts and have it switch backend automatically
  based on the end-user's preference (website showcase feature)
- 🆕 `ace remote` — ask the hangar agent what this is about
- 🆕 always-on bridge — when ace-connect is native built-in, a mode that always starts the
  bridge so all sessions auto-connect (possibly a hangar feature)
- 🆕 idle/on-pause command injection — detect a long pause in an active session (watch
  keypresses or message scroll) and fire a configurable command on idle, e.g. auto-run
  `ace-save`. Needs a lever to set the trigger + the command. (Runtime cousin of E's
  `inject=`, which is config-time content injection.)
- 🆕 ACE macros — record keybinds that play a chord or series of inputs into the harness.
  (Same session-input plumbing as idle-injection above, but user-triggered rather than
  idle-triggered.)

# H — CLI ergonomics & inspection

- [ ] **44** pipe `ace diff` through a pager for long output

*Ideas / later*
- **190** global `--yes` / auto-confirm flag for prompting commands
- **214** configurable session name + tab color for the terminal/backend
- **126** auto-spawn a tmux side pane with editor / diff view on session start (borderline
  big-bet)
- **13** `ace llm-help` — AI-friendly CLI guidance
- **227** `ace template` renders builtin prompt templates to stdout (inspection/debug)
- 🆕 `ace explain` (or `show` — alias?) should also surface a skill's frontmatter info
  (relates to 227 inspection + 241 read-only discovery output)

# I — Quality — testing & internals

- [ ] **152** `ace pull` misreports tier folder name as the changed skill
- [ ] **36** simplify flaude: print diagnostics to stdout instead of a JSONL file
- [ ] **150** hide flaude backend from user-facing help + docs (feature-flag)

*Ideas / later*
- **56 (+15)** design + fill live backend integration-test coverage (one test-strategy item)
- **154** reconsider per-binding error variant naming (Config → TreeLoad?)
- **241** surface discovery structural prunes in read-only paths (`ace skills` / skill_count)

*Shipped:* 37, 131.

# J — Docs & templates

- [ ] **200** expand school CLAUDE.md template with skill-authoring guidance
- [ ] **66** document wildcard imports + parent-school pattern (cross-listed in C)

*Ideas / later*
- 🆕 template links to ace-rs.dev — most templates mentioning ACE should link to
  <https://ace-rs.dev/> for SEO / self-promotion

*Shipped:* 25, 28, 58, 59, 31, 191.

# K — Research spikes  *(go/no-go gate — batch one session)*

None committed. Either schedule one spike session or keep them parked so they stop reading as
un-triaged work.

- **240** are backend exit codes reliable enough to drive ACE error hints?
- **239** should ACE synchronize backend memory files?
- **238** evaluate provisioning ACE skills into Claude Desktop
- **237** school Dockerfile → MCP server (cross-listed B)
- **34** ACE as an MCP server inside the backend (cross-listed B)
- **149** survey Ollama's `launch` integration list (cross-listed A)

# L — Big bets / out-of-core

- **77** convert ACE to a Tauri desktop app
- **158** Hangar/Tower — ancillary coding-velocity tooling (repos, CI/CD)
- **126** tmux side pane (cross-listed H)
- **19** school-defined roles — spec removed, redesign before reviving
  (`project_roles_removed`)

# Cancelled / superseded

Kept as records so the rejection trail stays queryable instead of re-litigated.

- **9** investigate Cursor/Continue/Cline — superseded by custom-backends (129)
- **26** Homebrew tap — Homebrew shipped via 194; done-by-other
- **22** `ace switch` (duplicate) — superseded by 55 + the live 69
- **38** global CLAUDE.md for cross-backend preferences — duplicate
