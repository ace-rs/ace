---
name: ace-school
description: >
  ACE school management — proposing skill changes, creating PRs to the school
  repo, and understanding school structure. TRIGGER when: user wants to propose
  changes to skills, create a school PR, run `ace diff`, or asks about school
  structure/workflow. DO NOT TRIGGER for: normal coding tasks or project-specific
  work.
---

# ACE School Management

## What is an ACE school?

A school is a git repo containing skills, conventions, and session prompts shared across
projects. Structure:

- `school.toml` — school metadata, session prompt, imports, services, MCP servers
- `skills/` — skill directories, each with a `SKILL.md`

Projects subscribe via `ace setup`, which clones the school into a local data
directory (`~/.local/share/ace/…`) and symlinks `skills/` into the project.

## `ace learn`

`ace learn` one-shots the backend to study the current project and narrow `ace.toml`'s
`skills` filter to just what's relevant. The agent edits the project's instructions file
in-place (project-study notes plus a "load these skills" section) and emits the desired
skill names/globs on stdout, which ACE writes back to `ace.toml`.

When to run it:

- After `ace setup` on a new project where the school has many skills (ACE auto-prompts).
- After importing new skills into the school (`ace school pull-imports`) when the school
  has grown past what fits a single project.
- Any time the project's needs shift enough that the active skill set should change.

The user reviews the result via `git diff` (both `ace.toml` and the instructions file)
and accepts or rolls back per file.

## Editing skills

Skill files in the project are symlinks into the school clone. Edits go directly to the clone —
this is intentional. The school clone is a real git working copy.

## Proposing changes

When skill edits need to go upstream:

1. Run `ace diff` to review changes.
2. `git -C $(ace paths school) checkout -b ace/{short-description}` — create a feature branch.
3. Stage and commit with a descriptive message. Use `git -C $(ace paths school)` for all git commands — do not `cd` out of the project directory.
4. `git -C $(ace paths school) push -u origin {branch}` — push to the school remote.
5. Create a PR to the school repo. Use GitHub MCP if available.
6. Do **NOT** reset the clone to main — that destroys uncommitted work across all branches.

## Good school PR guidelines

- **One skill or one coherent theme per PR.** Don't mix unrelated skill changes.
- **Title**: imperative, scoped (e.g. "Add self-audit checklist to general-coding").
- **Body**: what changed, why, which sessions revealed the need.
- **Keep skills generic** — no project-specific content. Skills must work across all projects
  that subscribe to the school.
- **Watch for conflicts** — skill instructions can interact with project `CLAUDE.md` and with
  each other. If a skill contradicts another skill or common project conventions, call it out
  in the PR description.
- **Honor existing conventions** — if issue-creator, PR-creator, or similar skills are
  available in the session, follow their format and guidelines when creating issues or PRs.