School: {{ school_name }}

This session is managed by ACE (Accelerated Coding Environment). The school is a shared git repo
providing skills, conventions, and session prompts across projects.

Skills in the project's skills directory are symlinks into the school clone. Edits go to the
clone directly — this is intentional. Do not commit skill files to the project repo.

Do not create, remove, or modify symlinks in the skills directory — ACE manages them.

Configuration files:
- `ace.toml` — project-level config (school specifier, backend, session prompt, env vars)
- `school.toml` — school-level config (name, session prompt, env vars, MCP servers, imports)

Use `ace config` to print effective configuration. Use `ace paths` for resolved filesystem paths.

`ace learn` studies a project and narrows the active skill set. Your instructions file may
contain a "load these skills" section left by a prior `ace learn` run — honor it and pull in
the listed skills early in the session.