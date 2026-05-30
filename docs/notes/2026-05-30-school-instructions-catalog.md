# School instructions file as capability catalog (+ PROD9-13)

Captured 2026-05-30. Draft / research — not yet decided. Full review + decisions
deferred to a fresh session. Do **not** treat anything here as ruled.

## Origin

While auditing builtin templates (N3), confirmed `school.toml`'s `session_prompt`
is fully wired (`src/cmd/main.rs:42-69` → `build_session_prompt` layer 2). That
raised a follow-on realization about `tpl_school_instructions.md`:

> The school instructions file (the `CLAUDE.md`/`AGENTS.md` ACE writes at
> `ace school init`) probably needs to be **very comprehensive**. Agents working on
> the school itself have no baseline knowledge of how to manage a school and can't
> learn enough from `ace --help` alone. The file needs to enumerate every school
> feature so that when the user asks "do X", the agent knows whether X *can* be
> done (or not) and through which feature.

Key correction from the user (rejecting a "thin pointer" approach): a bare
"run `ace llm-help`" pointer fails, because it tells the agent *where to look* but
not *what's possible*. The agent needs the capability surface **in-context** to
judge feasibility — including saying "no" correctly for out-of-scope asks.

This ties into **PROD9-13** — "Add `ace llm-help` command for AI-friendly CLI
guidance" (Backlog, Low). Same underlying problem: `--help` isn't built for LLM
consumption, agents can't discover capabilities. User wants PROD9-13 resolved when
this work lands.

## Proposed design (NOT yet approved)

Three channels already exist; the instruction file's job is the one only it can do.

| Channel                            | Job                                              | Drift risk                       |
| ---------------------------------- | ------------------------------------------------ | -------------------------------- |
| `tpl_school_instructions.md`       | **Capability map** — what exists, whether X is   | low if it names features, not    |
| (always loaded)                    | possible, which feature/section does it          | flags                            |
| `ace-school` skill (already        | Detailed *how* — workflow steps (PR submission,  | already maintained               |
| imported into every school)        | import flows)                                    |                                  |
| `ace <cmd> --help`                 | Exact flag reference                             | zero (generated from clap)       |

The `ace-school` skill is seeded into every school via the standard `ace-rs/school`
import (school-commands.md:65), so the detailed-how channel is already present at
every school. The catalog should **name features + point to `--help`** for exact
flags rather than copying them — that's what stops it re-drifting into a fifth
stale flag-copy.

**Candidate consequence:** this supersedes PROD9-13 — the pushed capability catalog
plus the existing `ace-school` skill solve the discovery problem without a separate
`ace llm-help` command. Close PROD9-13 as superseded on ship. (Decision pending.)

### Open question the design hinges on

Whether the instruction file should stay a feature-naming catalog (defer flags to
`--help`/skill) **or** be fully self-contained including exact flags/workflows
(higher drift risk). User has not chosen.

## Verified capability inventory (checked against the live clap tree)

Source of truth for this list is the actual CLI, **not** the specs (specs drifted —
see below). Verified in `src/cmd/mod.rs` + `src/cmd/school.rs` + `src/cmd/import` on
2026-05-30.

### Commands (school-authoring relevant)

- `ace school init [--name N] [--force]`
- `ace school pull` (alias `update`)
- `ace school skills`
- `ace school validate` (alias `check`)
- `ace import <source> [--skill X] [--all] [--include-experimental] [--include-system]`
  (top-level, not under `ace school`)
- `ace diff` (top-level)
- General: `ace config`, `ace paths`, `ace fmt`/`format`, `ace skills`/`ls`,
  `ace explain <name>`, `ace mcp`

### `school.toml` sections

`name`, `backend` (top-level default backend), `session_prompt`, `[env]`, `[[mcp]]`,
`[[projects]]` (+ optional `projects.env`), `[[backends]]` (custom/override backends),
`[[imports]]`.

`[[imports]]` fields: `source`, `skills` (alias `skill`), `exclude_skills`,
`include_experimental`, `include_system`, `include_internal`. NB: `include_internal`
is a valid **TOML field** but has **no** `--include-internal` CLI flag — the catalog
must keep that distinction (agent can set it by editing `school.toml`, not via a
flag).

### On-disk folders

`skills/`, `rules/`, `commands/`, `agents/` — all optional; only present folders link
into consuming projects.

### Out-of-scope (so the agent says "no" correctly)

Plugins, lockfile/pinning, subpath imports, local-school bootstrap (`ace setup .` does
not bootstrap `school.toml`), roles (removed 2026-05-22). Sources:
[[project_skill_scope]], [[project_roles_removed]], CLAUDE.md.

### Current template gaps

`tpl_school_instructions.md` today is missing: top-level `backend`, the `[[backends]]`
section, the `[[projects]]` env nuance, `rules/`/`commands/`/`agents/`, the full
import surface, `ace school skills`/`validate`, `ace diff`, and the out-of-scope list.

## Spec drift found (specs document phantom commands/flags)

Verified against the live CLI — these spec entries describe things that **do not
exist** in the binary:

- **`ace school fix`** — fully documented (school-commands.md:227, school-toml.md:193)
  but **absent from the `school::Command` enum** (only Init/Pull/Skills/Validate).
  Either implement it or delete the spec. Memory wrongly recorded it as "landed"
  (corrected — see below). Note: singular-key normalization already happens on any
  write, so no `fix` command is strictly required for migration.
- **`ace school add-import`** — referenced as a writer (school-toml.md:187); not a
  real subcommand.
- **`ace import --include-internal`** — spec'd as a flag (school-commands.md:81); not
  in the `Import` struct. (`include_internal` the TOML field is real.)
- **`pull-imports` → `pull`** — already fixed this session (commit a8551f4) in
  emit.md/model.md/selection.md/learn.md.
- **`tpl_school_claude_md.md`** — overview.md:139 references this filename for the
  commit-message format; the actual builtin is `tpl_school_instructions.md`.

## Decisions still owed (deferred to fresh session)

1. **Channel design** — catalog-names-features (defer flags to `--help`/skill) vs.
   fully self-contained instruction file. + confirm PROD9-13 closes as superseded.
2. **Phantom `ace school fix`** — delete from spec, or implement (track separately)?
3. **Spec-drift cleanup scope** — fix remaining drift in this work (one commit), or
   sweep separately, or just-the-blockers.

## Related to-be-done: session-injecting CLAUDE.md content

(Was a separate session task; folded here because session tasks don't survive
`/clear`. Same push-channel question, one layer broader.)

Investigate whether content ACE normally writes into the instruction file
(`CLAUDE.md` / `AGENTS.md`) would be better delivered via `session_prompt`
injection instead. `session_prompt` is confirmed wired (school.toml `session_prompt`
→ `build_session_prompt` layer 2, `src/cmd/main.rs:42-69`).

Trade-off to think through:

- **Instruction file** — backend-native, persists on disk, editable/inspectable,
  survives outside ACE, committed.
- **Session-injected** — dynamic, role-aware, not committed, present only when
  launched via `ace`.

Determine which categories of instruction-file content (durable project facts vs.
dynamic directives) belong in which channel. This dovetails with the catalog design
above: the capability catalog is durable/on-disk (instruction file); dynamic nudges
may belong in `session_prompt`.

### Full push-channel stack (correction — more channels, distinct ownership)

The "3 channels" table earlier is about *guidance content for school authors*
(catalog vs skill vs `--help`). Orthogonal to that is the **session-prompt layer
stack**, which has multiple push channels with different owners and lifetimes. Per
`build_session_prompt` (`src/templates/session.rs:20-34`):

1. **ACE built-in `prompt_session.md`** — *ACE-owned, ALWAYS injected*, rendered
   with `{{ school_name }}`. Fires regardless of whether the school sets
   `session_prompt`. This is ACE's guaranteed always-present channel.
2. **School `session_prompt` field** — school-owned, injected verbatim if non-empty.
3. **Project `session_prompt` field** (`ace.toml`/`ace.local.toml`) — project-owned,
   verbatim if non-empty.
4. **Conditional built-in layers** — `excluded_skills`, `changes`, `school_changes`
   (+ dirty), `previous_skills` — ACE-owned, fire only when their condition holds.

Plus, entirely separate from the session prompt: the **on-disk instruction file**
(`CLAUDE.md`/`AGENTS.md`) the backend reads natively — persistent, committed.

Design implication: ACE owns a guaranteed always-on injection vehicle
(`prompt_session.md`) independent of any school config. The session-injection
question (#1) is really: which instruction-file content should migrate into *that*
ACE-owned layer (dynamic, ACE-controlled, present every session) vs. stay on disk
(persistent, editable, survives outside ACE). The school `session_prompt` field is a
third option but is school-author-controlled, not ACE-controlled.

## In-scope for this refactor

- **`tpl_school_readme.md`** — the Structure block predated nested skills; the N3 pass
  added a nested example, but the readme should be revisited wholesale alongside
  `tpl_school_instructions.md` so both describe the nested-layout model consistently
  (school storage at `skills/<identity-path>/SKILL.md`, flat-vs-nested emit per backend
  capability). The instruction file is the comprehensive catalog; the readme is the
  human-facing front door — they should agree.

## Pointers

- Templates: `src/templates/builtins/tpl_school_instructions.md`,
  `tpl_school_readme.md`. Composition: `src/templates/session.rs`,
  `src/cmd/main.rs:42-69`.
- Specs: `docs/spec/school/{overview,school-commands,school-toml}.md`,
  `docs/spec/skills/{emit,model,selection,sync}.md`,
  `docs/spec/prompt-templating.md`.
- CLI truth: `src/cmd/mod.rs` (top-level), `src/cmd/school.rs` (school subcmds),
  `src/cmd/import.rs`.
- PROD9-13: https://linear.app/prodigy9/issue/PROD9-13
- Related tickets: PROD9-227 (`ace template` render builtins), PROD9-151
  (`ace learn` two-diff), session task #1 (session-injecting CLAUDE.md content).
