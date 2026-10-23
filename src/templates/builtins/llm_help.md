# ACE capability catalog

ACE (Accelerated Coding Environment) launches a coding agent backend (Claude Code,
Codex, OpenCode, Pi) for a project and provisions it from a **school**: a shared git
repository of skills, session prompts, environment variables, and MCP servers.

This catalog ships with the `ace` binary and matches its version exactly. Use it to
decide whether ACE can do something and which command or config section does it. Run
`ace <command> --help` for exact flags. If a feature is not listed here, ACE does not
have it.

## Concepts

- **Project** — the repository where `ace` runs; configured by `ace.toml`.
- **Linked school** — the school a project consumes, named by `school = "owner/repo"`
  in `ace.toml` (`owner/repo:subdir` for a school in a subdirectory, `.` for a school
  embedded in the project). ACE clones it under its data directory (`ace paths school`)
  and updates the clone on `ace` and `ace pull`.
- **Authored school** — a school repository opened as the current directory. The
  `ace school` commands and `ace import` edit it.
- **Skills** — directories with a `SKILL.md`. ACE links the selected school skills into
  the backend's skills directory; projects narrow the selection in config.

## Commands

{{ commands }}

## Global flags

{{ flags }}

## Project configuration: `ace.toml`

Layers, later overriding earlier: school `school.toml` defaults, user `ace.toml`
(`ace paths config.user`), project `ace.toml`, then `ace.local.toml` (gitignored).
Read with `ace config` / `ace config get`; write with `ace config set`.

- `school` — linked school specifier.
- `backend` — backend name: built-in or declared under `[backends.<name>]`.
- `session_prompt` — extra text appended to the injected session prompt.
- `[env]` — environment variables for the backend process.
- `skills`, `include_skills`, `exclude_skills` — skill selection by name or glob.
- `exclude_mcp` — school MCP servers not to register.
- `trust` — `"default"`, `"auto"`, or `"yolo"` (user and local layers only).
- `resume` — auto-resume the previous session (user and local layers only).
- `skip_update` — disable automatic update checks.
- `[backends.<name>]` — custom or overridden backend: `kind`, `cmd`, `env`, `model`,
  `effort`.

## School configuration: `school.toml`

- `name` — display name.
- `backend` — default backend for projects using this school.
- `session_prompt` — text injected into every session using this school.
- `[env]` — shared environment variables (not secrets).
- `[[mcp]]` — remote MCP servers: `name`, `url`, optional `headers`, `instructions`.
- `[[projects]]` — project catalog: `name`, `repo`, `description`, optional `env`.
- `[[imports]]` — skills imported from other repositories: `source`, `skills`,
  `exclude_skills`, `include_experimental`, `include_system`, `include_internal`.
  Written by `ace import`; refreshed by `ace school pull`.
- `[backends.<name>]` — same shape as in `ace.toml`.

## Out of scope

- Plugin systems: plugin manifests and `plugin:skill` invocation are not honored.
- Lockfiles or version pinning: imports and schools always track the latest `main`.
- Per-skill import provenance: ACE does not record which import wrote a skill.
- Translating skill frontmatter between backends.
- Editing the linked school clone: ACE manages it. School skill changes go through the
  `ace-school` skill.
