# {{ school_name }}

ACE school repository — shared skills, conventions, and session prompts for
your team.

## For Developers

Subscribe a project to this school:

```sh
ace setup {school_specifier}
```

This clones the school, symlinks skills into your project, and configures
your AI coding session. Run `ace` to start.

## Structure

```
school.toml       # School configuration
skills/           # Skill directories (each has a SKILL.md)
  ace-school/     # Imported from ace-rs/school
CLAUDE.md         # AI session instructions (or AGENTS.md for Codex/OpenCode)
```

`school.toml` defines: env vars, MCP server endpoints, project catalog, and
skill imports. See the instructions file (`CLAUDE.md` for Claude, `AGENTS.md`
for Codex/OpenCode) for section details.

## Managing Skills

| Task               | Command                   |
|--------------------|---------------------------|
| Import a skill     | `ace import <owner/repo>` |
| Re-fetch imports   | `ace school pull`         |
| Review local edits | `ace diff`                |

Skills are the primary content of this repo. Each skill is a directory
under `skills/` with a `SKILL.md` that the AI backend reads during coding
sessions.
