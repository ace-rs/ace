# School Overview

A school is a git-cloneable source repository containing skills, conventions, agent configs, and
other shared resources for an organization. ACE maintains a local clone in
`~/.local/share/ace/{owner/repo}/` (XDG_DATA_HOME).

This spec covers the **school-repo** context — the maintainer authoring a school. The
other context — a project consuming a school — is covered in [setup.md](../setup.md).
The two contexts are distinguished by which marker file exists at the workdir root:
`school.toml` (school) vs `ace.toml` (project). Maintainer-side commands are under
`ace school <subcmd>` and `src/actions/school/`; consumer-side under bare `ace` /
`ace setup` / `ace pull` and `src/actions/project/`.

## Specifier

The `school` field in `ace.toml` uses a multi-mode specifier:

```
<source>:<path>
```

- **`source`** — GitHub `owner/repo` shorthand, or `.` for embedded (current repo).
- **`path`** — (optional) subfolder within the repo containing `school.toml`. Separated by `:`.

If `:<path>` is omitted, the repo root is assumed.

| Specifier | Meaning |
|---|---|
| `prod9/school` | Remote repo, root |
| `prod9/mono:school` | Remote repo, `school/` subfolder |
| `.:/school` | Embedded in current repo at `school/` |

Embedded schools (`.`) skip clone/fetch — they read directly from the working tree.

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
to subscriber projects: `[[mcp]]` server registrations, `[[backends]]` custom-backend
entries, `[[roles]]` prompts, `[[projects]]` catalog, and `[[imports]]` skill provenance
(including wholesale-imports of another school via `skill = "*"`). See
[school-toml.md](school-toml.md) for the full reference.

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
