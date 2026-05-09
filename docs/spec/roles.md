# Roles

Schools define a set of roles that describe how different team members use the AI coding
environment. Each role carries a prompt that is injected into the session prompt so the LLM
understands the user's context and adjusts its behavior accordingly.

Roles can influence which skills are relevant, how the agent communicates, and what kind of
work it prioritizes. The school operator writes each role's prompt to steer the backend
appropriately — e.g. telling it to focus on spec writing for a PM role, or to skip code
review guidance for a designer role. Future versions may add explicit skill filtering per
role; for now, the prompt is the control surface.

## Definition

Roles are declared in `school.toml`:

```toml
[[roles]]
name = "task-master"
prompt = """You are a project manager. Break down requirements into actionable tasks, \
write specs, manage issue trackers, and coordinate work across repos. Do not write \
application code directly — create issues and specs instead. Focus on clarity, \
prioritization, and unblocking the team. \
Always load the issue-creator skill at the start of each session."""

[[roles]]
name = "frontend-engineer"
prompt = """You are a frontend engineer. Focus on UI components, styling, accessibility, \
and browser APIs. Prefer React/Next.js patterns, write CSS with Tailwind, and ensure \
responsive design. Run lint and type-check before committing. Skip backend and \
infrastructure concerns unless they directly affect the frontend. \
Always load the general-coding skill at the start of each session."""

[[roles]]
name = "backend-engineer"
prompt = """You are a backend engineer. Focus on API design, database queries, \
business logic, and service architecture. Write migrations, handle error cases \
exhaustively, and ensure proper logging. Skip frontend and styling concerns. \
Always load the general-coding and rust-coding skills at the start of each session."""

[[roles]]
name = "operator"
prompt = """You are a DevOps/infrastructure operator. Focus on CI/CD pipelines, \
deployment configs, monitoring, and infrastructure-as-code. Prefer Terraform for \
provisioning, Docker for packaging, and Kubernetes for orchestration. Flag security \
concerns proactively. Do not modify application business logic. \
Always load the general-coding skill at the start of each session."""
```

Each role has:

- `name` — Short identifier. Used as the value in `ace.local.toml`.
- `prompt` — Injected into the session prompt verbatim. Written for LLM consumption — the
  school operator uses this to control how the backend behaves for this role.

Schools with no `[[roles]]` entries skip role selection entirely.

## Selection

On the main `ace` run, if the school defines roles and no role is set in config:

1. Present `prompt_select()` with the school's role list (name + prompt).
2. Save the chosen role name to `ace.local.toml` (`role = "backend-engineer"`).
3. Continue with normal startup.

If only one role is defined, auto-select it without prompting.

## Storage

The selected role is stored in `ace.local.toml` (gitignored, per-machine):

```toml
role = "backend-engineer"
```

This keeps role selection personal — different team members working on the same repo can
have different roles without conflicting.

`ace.toml` (project) or `~/.config/ace/ace.toml` (user) can also set `role` to provide a
default, but `ace.local.toml` overrides both.

Resolution order: user < project < local (last non-empty wins, same as other fields).

## Resolution

1. Resolve `role` string from config layers (user < project < local; last non-empty wins).
2. Look up the role name in the school's `[[roles]]` list.
3. If found → inject `Role: {name}\n{prompt}` into the session prompt.
4. If set but not found in school's list → warn ("role '{name}' not defined in school"),
   proceed without role injection.
5. If empty and school has roles → trigger selection (see above).
6. If empty and school has no roles → no-op.

## Prompt Injection

Role information is injected into the session prompt after the built-in base prompt and
before the school's `session_prompt`:

```
School: Acme Corp

{built-in base prompt}

Role: task-master
You are a project manager. Break down requirements into actionable tasks, write specs,
manage issue trackers, and coordinate work across repos. Do not write application code
directly — create issues and specs instead. Focus on clarity, prioritization, and
unblocking the team.

{school session_prompt}

{project session_prompt}
```

See [prompt-templating.md](prompt-templating.md) for the full composition order.

## Commands

### `ace roles`

List available roles from the school's `[[roles]]` definitions. Marks the currently
selected role. No side effects.

If the school has no roles defined, prints a message and exits.

### `ace roles --switch`

Interactive picker to change the current role. Overwrites `ace.local.toml` with the new
choice.

### `ace roles --switch <name>`

Set role directly without prompting. Errors if the name doesn't match a defined role.

### `ace roles --add <name>`

Add a new role to `school.toml`. Requires school context (school.toml present).

- `ace roles --add task-master` — prompts interactively for the prompt text.
- `ace roles --add task-master --prompt "You are a project manager..."` — sets both directly.

Errors if the role name already exists.

### `ace roles --remove <name>`

Remove a role from `school.toml`. Errors if the role doesn't exist.

## Future: Skill Filtering

A role could eventually specify which skills to load or exclude:

```toml
[[roles]]
name = "task-master"
prompt = "..."
skills = ["issue-creator"]

[[roles]]
name = "frontend-engineer"
prompt = "..."
skills = ["general-coding", "react-coding"]
```

Not in initial implementation. For now, the `prompt` field can instruct the LLM to focus on
or ignore certain skills.
