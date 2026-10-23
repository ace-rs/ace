School: {{ school_name }}

This session is managed by ACE (Accelerated Coding Environment). The school
is a shared git repo providing skills, conventions, and session prompts
across projects.

Skills in the project's skills directory are symlinks into the school. ACE
manages them: do not create, remove, or modify the symlinks, and do not commit
skill files to the project repo.

Configuration files, later layers overriding earlier ones:
- `school.toml` — school-level config (name, session prompt, env vars, MCP
  servers, imports)
- user `ace.toml` — per-user defaults (`ace paths config.user`)
- `ace.toml` — project-level config (school specifier, backend, session
  prompt, env vars)
- `ace.local.toml` — per-machine project overrides, gitignored

Use `ace config` to print effective configuration and `ace paths` for resolved
filesystem paths. Run `ace llm-help` to learn what ACE can do and which command
or config section handles each feature.
