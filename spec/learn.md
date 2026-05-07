# Learn

`ace learn` actively studies the project at hand — reading the source tree,
configs, and existing instructions — and produces two edits:

1. The active backend's instructions file (`CLAUDE.md`, `AGENTS.md`, …) —
   edited in-place by the agent itself during a one-shot. Two things go in:
   (a) what it learned about the project (conventions, layout, gotchas), and
   (b) explicit guidance to load the skills it selected, so future sessions
   reading the instructions file know which skills are relevant and pull
   them in.
2. The project `ace.toml` `skills` filter — rewritten by ACE from a list the
   agent returns on stdout, narrowed to the skills relevant to this project.

This is project study, not session capture. The agent has no transcript of
prior work; it forms its understanding from the project's state alone.

Review is done via `git diff` after the run. ACE does not gate writes
post-hoc.

## User Journey

A concrete example end-to-end:

1. User clones a random Rust project and runs `ace setup`.
2. Setup wires the school. The school exposes ~100 skills.
3. Skill-count check fires (count > 10). Setup prompts inline: "this school
   has 100 skills — run `ace learn` now to narrow to what this project
   actually needs? [y/N]".
4. User answers `y`. Setup invokes `LearnAction` directly — the
   setup-time y/n is the confirm and `LearnAction` itself does no
   prompting. ACE one-shots the backend with the full skill list and a
   prompt to study the project.
5. The agent reads source, `Cargo.toml`, existing `CLAUDE.md`, etc. It edits
   the instructions file in place with project-specific notes and prints,
   say, 7 skill names on stdout.
6. ACE writes those 7 names to `ace.toml`'s `skills` key.
7. User reviews via `git diff`, keeps or rolls back per file.

Net effect: future `ace` invocations load 7 relevant skills instead of 100,
and the instructions file reflects what the agent learned about the project.

## Invocation

`ace learn` — project-repo only. Errors with `School` if no school resolves.

Confirm semantics by mode:

- **Human mode** — `cmd/learn` prompts y/N (default N) before running. On
  N, exit cleanly with no work done.
- **Porcelain / non-TTY** — explicit `ace learn` invocation IS consent.
  Run directly, no prompt.

Contrast with auto-trigger callers (setup, pull-imports, `ace` startup):
the user's intent there is setup/pull/startup, not learn. Spending tokens
must be opt-in via the inline offer prompt — and that prompt only fires
in Human mode, so auto-trigger never spends in porcelain. See "Auto-trigger"
below.

A global yes/auto-confirm flag is tracked separately (no such flag exists in
ACE today). When it lands, the Human-mode `cmd/learn` prompt will honor it
like any other prompting command.

### Auto-trigger

On `ace setup`, `ace school pull-imports`, and `ace` startup, ACE counts
the total skills from the resolved school. When the count exceeds **10**,
each site prompts inline (y/N): "school has N skills — run `ace learn`
now to narrow to what this project needs?" On `y`, `LearnAction` runs
immediately — the inline prompt is the confirm; the action itself does
no prompting.

All three sites share one helper (`school::skill_count::maybe_offer_learn`)
so the prompt phrasing and skip conditions stay in lockstep. Don't make
the user run a second command when ACE can just do it.

### Change-driven re-run hint

Once a project has an explicit `skills` filter (the user already ran learn
or hand-pinned), the inline prompt path goes silent — the user opted in to
manual control. But the school's skill set keeps moving as imports change.
A second helper covers that case:

`school::skill_count::maybe_hint_relearn(ace, changes)` emits a single
soft hint when **all** of these are true:

- `changes` is non-empty (skills were just added/removed/modified).
- Resolved school skill count > 10.
- Project `ace.toml` has `skills` set explicitly.
- Output mode is Human.

The hint reads `school skills changed (3+/1-) — consider rerunning ace
learn`. No prompt, no block — the user pinned their filter deliberately,
this is just a nudge that the world moved.

Wired at:

- `cmd/main::prepare_school` — every `ace` startup runs Prepare, which
  pulls and surfaces a `SkillChange` list. Same hint covers `ace setup`
  for free since it shares prepare.
- `cmd/pull` — explicit `ace pull` (project-side school refresh).

`ace school pull-imports` already uses `maybe_offer_learn` for the
no-explicit-skills case; once the user pins, the next pull-imports
session gets the soft hint via the startup path on next `ace` run.

Skip conditions:

- Project `ace.toml` has the `skills` key set explicitly (any value, including
  `skills = []`) — user opted into manual control, ACE stays out of it. ACE
  raw-parses `ace.toml` to distinguish the explicit `skills = []` opt-out from
  a missing key, since serde collapses both to `Vec::is_empty()`.
- Non-Human output mode (`--porcelain`, non-TTY) — no inline prompt is
  possible, so the auto-trigger silently skips. The user can still run
  `ace learn` explicitly.

The threshold is a hardcoded constant. No config knob (convention over
configuration; see `index.md`).

## Flow

`LearnAction::run` is pure work — no prompting, no mode checks. Callers
own the user-confirm step.

1. Resolve backend + school. Bail with `School` error if no school.
2. **One-shot.** `Backend::exec_one_shot` with the rendered `LEARN` template.
   The agent:
   - Edits its own instructions file in place. ACE doesn't pass the path —
     each backend already knows its own instructions file. The edit must
     include both project-study notes *and* a section guiding future
     readers (i.e. the same backend on next launch) to load the selected
     skills.
   - Returns on stdout a list of skill names *or glob patterns*, one per
     line, nothing else. This is the desired final `skills` set. Globs
     (e.g. `frontend-*`) are valid because `ace.toml`'s `skills` key
     already supports them — the prompt tells the agent so it can collapse
     related skills.
4. **Parse stdout — forgiving.** LLMs hallucinate and stray; ACE tolerates it
   and warns rather than aborting. Per line:
   - Trim whitespace. Skip blanks.
   - Strip common stray decoration: leading `- `, `* `, backticks, fence
     markers (` ``` `), trailing punctuation.
   - If the residue is a glob pattern → keep it (validated structurally,
     not against the index — globs may match nothing today and something
     later).
   - If the residue is a literal name that exists in the school's skill
     index → keep it.
   - Otherwise → drop it and emit a `warn:` line to stderr naming the
     offending line and the reason (`unknown skill`, `looks like prose`,
     `fence marker`, etc.).

   Empty result is acceptable — small school, novel project, nothing
   matched yet. ACE writes the empty list (or whatever survived) and
   prints a summary: `kept N entries, dropped M lines (see warnings
   above)`. No `EmptySkillsList` abort.
5. **Apply to `ace.toml`.** Replace the `skills` array with the parsed list.
   Preserve other keys and formatting via the existing toml-edit plumbing.
6. **Done.** User runs `git diff` / `git status` to review and `git restore` /
   `git add` / `git commit` to accept or reject. ACE prints no further prompts.

## Prompt Template

`src/templates/builtins/prompt_learn.md`, registered as `LEARN` in
`src/templates/builtins.rs`.

Variables:

- `{available_skills}` — newline-separated skill names from the resolved
  school's index. Defines the name space the agent picks from (and the
  pool that glob patterns match against).

The template tells the agent:

- Edit your own instructions file in place. You know which file that is.
  Include project-study notes *and* a "load these skills" section pointing
  at the names you'll return below.
- Output to stdout: skill names or glob patterns (e.g. `frontend-*`),
  one per line, nothing else. Globs are supported in `ace.toml`'s
  `skills` key — use them when a related cluster fits the project.
- Strict: no prose, no fences, no headers on stdout.

ACE parses forgivingly anyway (see Flow step 4) so an imperfect response
still produces a useful filter.

## Response Schema

Stdout, one skill name or glob pattern per line. Nothing else. Final
desired set (replacement, not toggles).

Example:

```
general-coding
rust-coding
simplify
markdown-writing
frontend-*
```

That's the entire contract. No headers, no fences, no empty lines, no
explanations. ACE's parser is forgiving (see Flow step 4) but the prompt
asks for the strict form.

Stderr is ignored on success and printed verbatim on non-zero exit for
debugging.

## Errors

`LearnError` lives in `src/actions/project/learn.rs`:

- `School(SchoolError)` — no school resolved.
- `Skill(SkillError)` — skill discovery failed.
- `Config(ConfigError)` — ace.toml load/save failed.
- `Backend(BackendError)` — backend binding failed.
- `Prompt(IoError)` — prompt I/O failed (only via callers; `LearnAction`
  itself never prompts).
- `BackendSpawn(io::Error)` — `exec_one_shot` failed to launch.
- `BackendNonZero { status, stderr }` — backend exited non-zero.
- `TomlWrite(io::Error)` — failed to write `ace.toml`.

Note: by the time the backend has exited, the instructions file may already be
edited. Errors after step 3 leave the instructions-file edits in place; the
user can `git restore` them. This is intentional — git is the rollback
mechanism.

## Documentation Surfaces

`ace learn` must be documented in two places that ship with ACE:

- **`tpl_ace_school_skill.md`** (the `ace-school` skill scaffolded into every
  school) — short section explaining what `ace learn` does, when to run it
  (project bootstrap, after schools grow), and how it interacts with
  `ace.toml` `skills` and the instructions file.
- **`prompt_session.md`** (the session prompt injected into every backend
  session) — one short paragraph noting that `ace learn` exists, that the
  agent's instructions file may include "load these skills" guidance left by
  a prior `ace learn` run, and that those should be honored.

Both surfaces are template files under `src/templates/builtins/`. Updates to
them land in the same PR as `ace learn` itself.

## Module Layout

- `src/actions/project/learn.rs` — `LearnAction`, `LearnError`.
- `src/school/skill_count.rs` — shared skill-count check + auto-hint emitter.
  Called from `setup`, `school::pull_imports`, and `Ace::startup` (or
  equivalent startup hook).
- `src/templates/builtins/prompt_learn.md` — prompt template.

## Out of Scope

- School-side `ace school learn` (would edit school skills, not project
  filter).
- Multi-lesson batching.
- `--dry-run` flag.
- Partial accept of the skills list (git handles partial accept).
- Retry on malformed output.
- Session transcript handoff. `ace learn` is project study, not session
  recall — the agent works from project state (instructions file, `ace.toml`,
  source tree) and forms its own understanding.
- Polymorphic flags beyond one-shot (`--model`, `--resume`, etc.) — tracked
  separately under PROD9-159.
