# Decision: Unified ACE-managed `.gitignore` block (2026-05-10)

Status: **decided** — single `UpdateGitignore` action serves both project
and school contexts; static OS/editor cruft moves inside the marker block;
`.env` and Python patterns dropped; static `tpl_gitignore.md` template
deleted.

## Problem

Two disjoint `.gitignore` mechanisms coexisted:

1. `src/templates/builtins/tpl_gitignore.md` (`templates::builtins::GITIGNORE`)
   — static OS / Python / editor / env list. Written once by
   `actions/school/init.rs` if `.gitignore` was absent. Never updated
   afterwards.
2. `src/actions/project/update_gitignore.rs` (`UpdateGitignore`) — dynamic
   ACE-managed marker block (symlinked school folders + `ace.local.toml`)
   maintained on every `ace setup`. No OS/editor patterns.

Consequences:

- A fresh project initialised by `ace setup` (no prior `.gitignore`) ended
  up with **only** the ACE block — no `.DS_Store`, no swap files.
- The school template seeded `.env` / `.env.local`, ignoring environment
  files the maintainer might intend to commit (sample envs, fixtures).
- Python-specific patterns leaked into every school regardless of language.
- Two surfaces to maintain for one concern.

## Decision

Single source of truth. `UpdateGitignore` builds the entire managed block
and runs in both contexts; the static template file is removed.

### Block shape

```
# ACE-managed — do not edit this block.
# (intro comment)
.DS_Store
Thumbs.db
*.swp
*.swo
*~
.vscode/
.idea/
                                 ← project scope only, below
.agents/agents
.agents/skills
.claude/agents
.claude/commands
.claude/rules
.claude/skills
ace.local.toml
# end ACE
```

The static prelude (OS + editor cruft) is shared; the per-folder dynamic
section and `ace.local.toml` are project-scope only — schools have no
symlinked school folders to ignore.

### Scope parameter

`UpdateGitignore` gains a `scope: Scope` field (`Project | School`) that
toggles whether the dynamic section is emitted. `cmd/setup.rs` passes
`Project`; `actions/school/init.rs` passes `School` and removes its own
`.gitignore` write path entirely.

### School `.gitignore` becomes always-managed

Previously the school template was written only when `.gitignore` did not
exist. Under the new model, `school init` calls `UpdateGitignore` with the
same append-or-replace marker semantics used for projects. Re-running
`init --force` (or any future caller) refreshes the block.

### Dropped patterns

- **`.env`, `.env.local`** — committing or ignoring env files is a
  per-project decision; ACE should not pre-judge it.
- **Python (`__pycache__/`, `*.pyc`, `*.pyo`)** — language-specific noise
  in a tool that targets any language.

## Out of scope

- Per-language gitignore templates. If users want richer language-aware
  ignores they bring their own; ACE manages only the ACE-relevant block
  plus universal cruft.
- Migration of existing `.gitignore` files that contain the old static
  template. The next `ace setup` will append/replace the marker block;
  any pre-existing `.env` lines stay where the user put them.

## References

- `src/actions/project/update_gitignore.rs` — block builder + scope.
- `src/cmd/setup.rs` — project-scope invocation.
- `src/actions/school/init.rs` — school-scope invocation.
- `docs/decisions/2026-04-22-action-layout.md` — `UpdateGitignore`
  remains under `actions/project/` despite cross-role use; the action is
  consumer-shaped and the school caller is the secondary user.
