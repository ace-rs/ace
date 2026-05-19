# School Overview

A school is a git-cloneable source repository containing skills, conventions, agent configs, and
other shared resources for an organization. ACE maintains a local clone in
`~/.local/share/ace/{owner/repo}/` (XDG_DATA_HOME).

This spec covers the **school-authoring** mode — the maintainer curating a school's
content. The other mode — a project consuming a school — is covered in
[setup.md](../setup.md).

The two modes are distinguished by **which command was invoked**, not by any marker file:

- **Project mode** — bare `ace` and `ace setup` / `ace pull` / etc. Reads `ace.toml`,
  resolves the specifier, syncs school content into the project. Actions live in
  `src/actions/project/`.
- **School-authoring mode** — `ace school <subcmd>`. Operates on the cwd as the school
  root. The cwd's `school.toml` is the file being edited. Actions live in
  `src/actions/school/`.

`school.toml` is school-side metadata. Its presence in a directory means "this directory
is a school"; it does *not* mean "this is the school in use." Which school is in use is
determined exclusively by `ace.toml`'s specifier.

A school repo that wants to dogfood itself gets an `ace.toml` with `school = "."` from
`ace school init`; bare `ace` from that workdir then resolves the embedded school via
the specifier like any other consumer.

## Specifier

The `school` field in `ace.toml` uses a multi-mode specifier:

```
<source>:<path>
```

- **`source`** — GitHub `owner/repo` shorthand, or `.` for embedded (current repo).
- **`path`** — (optional) subfolder within the repo containing `school.toml`. Separated by `:`.

If `:<path>` is omitted, the repo root is assumed.

| Specifier             | Meaning                              |
|-----------------------|--------------------------------------|
| `ace-rs/school`       | Remote repo, root                    |
| `sith/temple:school`  | Remote repo, `school/` subfolder     |
| `.:/school`           | Embedded in current repo at `school/`|

Embedded schools (`.`) skip clone/fetch — they read directly from the working tree.

Examples in this spec and elsewhere use `jedi/` and `sith/` as placeholder
owners; pick the convention `<owner>/school` for real repos so the specifier
reads obviously.

## Context Resolution

`Ace::require_school()` (`src/ace/mod.rs`) resolves the school's on-disk location
exclusively from `ace.toml`'s specifier. The workdir's `school.toml` (if any) is *not*
consulted as a location signal — a school repo that wants to dogfood itself uses
`school = "."` in its own `ace.toml`. Inputs:

- **A** — `ace.toml` present in workdir
- **S** — `ace.toml` declares `school = ...`
- **K** — specifier kind: local (`.` / `.:path`) vs remote (`owner/repo[:path]`)
- **R** — `school.toml` present at the resolved school root

| # | A   | S   | K      | R   | Outcome                                         | Meaning                                          |
|---|-----|-----|--------|-----|-------------------------------------------------|--------------------------------------------------|
| 1 | no  | n/a | n/a    | n/a | `Err(TreeLoad(NoConfig))`                       | empty dir — intent unknowable                    |
| 2 | yes | no  | n/a    | n/a | `Err(NoSpecifier)` — "run `ace setup`"          | project-repo, specifier missing                  |
| 3 | yes | yes | local  | yes | `Ok` resolved paths                             | embedded / dogfood / sibling school              |
| 4 | yes | yes | local  | no  | `Err(NotInitialized)` — "run `ace school init`" | local specifier points at uninitialized dir      |
| 5 | yes | yes | remote | yes | `Ok` clone paths                                | project consumer, clone present and initialized  |
| 6 | yes | yes | remote | no  | `Err(NotInitialized)` — "run `ace school init`" | clone exists but lacks `school.toml`             |
| 7 | yes | yes | remote | —   | `Ok` (clone dir absent)                         | first-run; `cmd/pull.rs` self-heals via clone    |

**Detection rule for cases 4 and 6.** After `school_paths::resolve`, if the resolved
root *exists as a directory* but does not contain `school.toml`, return
`SchoolError::NotInitialized`. The `is_dir()` guard preserves case 7 — when the
clone dir is absent entirely, return `Ok` so `cmd/pull.rs` can self-heal by cloning.

**Why case 1 is left as `NoConfig`.** Without an `ace.toml`, ACE has no signal of
intent (project setup vs. school authoring). The generic "no config found" message
stands; either `ace setup` or `ace school init` is the right next step depending on
what the user means to do. `ace school <subcmd>` itself is a separate path that
requires a `school.toml` in the cwd; see [school-commands.md](school-commands.md).

## Purpose

The school is the single source of truth for how an organization's AI coding environment
behaves. It centralizes shared knowledge so that every developer on the team gets the same
skills, conventions, and agent configurations — regardless of which project they're working on.

## Structure

```
school.toml              # School metadata and configuration (see school-toml.md)
skills/
  <name>/
    SKILL.md             # Skill definition (standard Claude Code skill format)
rules/
  <name>.md              # Convention/rule files
commands/
  <name>.md              # Slash commands for the backend
agents/
  <name>.md              # Agent configurations
```

All four folders are optional. Only folders present in the school are linked into projects.

Beyond on-disk folders, `school.toml` itself ships first-class declarations that ACE applies
to subscriber projects: top-level `session_prompt` (unconditional injection into every
session), `[[mcp]]` server registrations, `[[backends]]` custom-backend entries,
`[[roles]]` prompts (role-conditional), `[[projects]]` catalog, and `[[imports]]` skill
provenance (including wholesale-imports of another school via `skill = "*"`). See
[school-toml.md](school-toml.md) for the full reference.

Every school created by `ace school init` is seeded with a `[[imports]]`
entry for `ace-rs/school` (the standard school) — see
[standard-imports.md](standard-imports.md).

## Relationship to Projects

A school is independent of any single project. Multiple projects (e.g. frontend and backend
repos) share the same school. ACE syncs the school into each project via symlinks, so all
projects see identical skill versions from a single local clone.

A user can have multiple schools configured (e.g. `acme`, `personal`), each pointing to a
different source repository.

## Commit Messages

A school repo is not application code — it is **policy**. Every commit changes how an entire
team writes code. School changes propagate to every developer on the team, so commits must
carry enough context for someone reading `git log` months later to understand the decision
without asking around. A diff alone doesn't convey intent — the commit body is the
institutional memory.

Format and examples are in `src/templates/tpl_school_claude_md.md`.
