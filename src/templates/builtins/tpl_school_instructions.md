# {{ school_name }} — School Repository

This is an ACE school repository. It provides skills, conventions, and session
prompts to projects that subscribe to it via `ace setup`.

Run `ace` to start a coding session for developing this school. ACE writes this
instructions file under the name your backend expects (`CLAUDE.md` for Claude,
`AGENTS.md` for Codex/OpenCode) — same content either way.

## Structure

- `school.toml` — school configuration
- `skills/` — skill directories, each with a `SKILL.md`

Run `ace llm-help` for what ACE can do, which command or `school.toml` section
handles each feature, and what is out of scope. It ships with the `ace` binary,
so it always matches the installed version.

## Session Prompt

`session_prompt = "..."` at the top of `school.toml` injects custom instructions
into every ACE session that uses this school.

Use it for school-wide directives that should always apply: "always load skill
X first", "all output in English", repo-specific reminders. Keep it short;
anything long belongs in a skill instead.
