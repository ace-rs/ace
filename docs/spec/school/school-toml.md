# school.toml

The `school.toml` file lives at the root of a school repository. It declares metadata,
configuration, MCP servers, environment, and project catalog for the school.

## Example

```toml
name = "Acme Corp"

[env]
NODE_VERSION = "22"
PYTHON_VERSION = "3.12"
LITELLM_BASE_URL = "https://llm.acme.corp/v1"

[[roles]]
name = "task-master"
prompt = """You are a project manager. Break down requirements into actionable tasks, \
write specs, manage issue trackers, and coordinate work across repos."""

[[roles]]
name = "backend-engineer"
prompt = """You are a backend engineer. Focus on API design, database queries, \
business logic, and service architecture."""

[[mcp]]
name = "github"
url = "https://api.githubcopilot.com/mcp/"

[[mcp]]
name = "jira"
url = "https://mcp.atlassian.com/v1/sse"

[[mcp]]
name = "sentry"
url = "https://mcp.sentry.dev/sse"

[[projects]]
name = "backend"
repo = "github.com/acme-corp/backend"
description = "Go API server. Handles auth, billing, and core business logic."

[projects.env]
SERVICE_NAME = "backend"

[[projects]]
name = "frontend"
repo = "github.com/acme-corp/frontend"
description = "Next.js web app. Customer-facing dashboard and admin portal."

[projects.env]
SERVICE_NAME = "frontend"

[[projects]]
name = "infra"
repo = "github.com/acme-corp/infra"
description = "Terraform and Kubernetes configs for AWS deployment."
```

## Sections

### `name`

Display name for the school. Used in logs, UI, and fuzzy search. Not an identifier —
the school is identified by its GitHub `owner/repo` shorthand.

### `[env]`

Key-value pairs of environment variables. Set in the shell before exec-ing the backend.
Use for shared endpoints, API base URLs, feature flags, etc.

These are not secrets — secrets are managed by the backend's own OAuth flow when connecting
to remote MCP servers.

### `[[mcp]]`

Array of MCP server declarations. Each entry defines a remote MCP endpoint. ACE registers
these with the active backend (see [backend.md](../backend.md#mcp-server-registration) and
[mcp.md](../mcp.md) for design rationale).

- `name` — Identifier for the MCP server.
- `url` — Remote MCP endpoint URL. The backend discovers OAuth metadata via `.well-known`.

### `[[projects]]`

Catalog of projects in the organization. Gives the AI context about available repositories and
what they do, enabling better cross-project reasoning and navigation.

- `name` — Short project identifier.
- `repo` — Git-cloneable URL for the project.
- `description` — What the project is and does. Written for AI/LLM consumption — be specific
  about tech stack, domain, and responsibilities.
- `env` — Optional. Project-specific environment variables. Merged with top-level `[env]`
  (project values override).

### `[[roles]]`

Array of role definitions. Each entry describes a team role that affects how the AI behaves
during sessions. Users pick a role on first run; the selected role's prompt is injected
into the session prompt. See [roles.md](../roles.md) for the full workflow.

- `name` — Short identifier (e.g. `"task-master"`, `"backend-engineer"`). Used as the stored
  value in `ace.local.toml`.
- `prompt` — Injected into the session prompt verbatim. The school operator uses this to
  control how the backend behaves for this role.

Schools with no `[[roles]]` entries skip role selection entirely.

```toml
[[roles]]
name = "task-master"
prompt = """You are a project manager. Break down requirements into actionable tasks, \
write specs, manage issue trackers, and coordinate work across repos."""

[[roles]]
name = "backend-engineer"
prompt = """You are a backend engineer. Focus on API design, database queries, \
business logic, and service architecture."""
```

### `[[backends]]`

Array of backend declarations. Each entry registers a custom backend instance or partially
overrides a built-in (`claude`, `codex`). See
[backend.md § Custom Backends](../backend.md#custom-backends) for kind resolution and
layer-merge semantics.

- `name` — Identifier. Becomes selectable via `backend = "<name>"` or `-b <name>`.
- `kind` — Optional. Built-in name (`claude`, `codex`) the backend aliases. When omitted,
  ACE infers from `name` matching a built-in, then from `cmd[0]` basename.
- `cmd` — Optional. Argv for launching the backend. Defaults to `[kind.name()]`.
- `env` — Optional. Environment variables set in the launched process. Merged with the
  top-level `[env]`; per-backend env wins on collision.

```toml
# Override env on the built-in claude backend
[[backends]]
name = "claude"
env = { ANTHROPIC_BASE_URL = "https://proxy.example.com" }

# Custom name aliasing claude, with its own env
[[backends]]
name = "bailer"
kind = "claude"
env = { ANTHROPIC_BASE_URL = "https://bailer.example.com" }

# Wrap the claude binary through a process wrapper
[[backends]]
name = "claude-wrapped"
kind = "claude"
cmd = ["wrapper", "claude"]
```

### `[[imports]]`

Array of imported skill declarations. Each entry tracks a skill that was imported from an
external repository via `ace import`. Used by `ace school update` to re-fetch skills from
their sources.

- `skill` — Skill name or glob pattern. Exact names match the folder name under `skills/`.
  Glob patterns use `*` to match zero or more characters.
- `source` — GitHub `owner/repo` shorthand where the skill was imported from.
- `include_experimental` — When `true`, glob matching also expands into
  `skills/.experimental/`. Defaults to `false`. Only meaningful for wildcard entries —
  explicit skill names resolve across all tiers regardless of this flag.
- `include_system` — When `true`, glob matching also expands into `skills/.system/`.
  Defaults to `false`. Same tier-filter semantics as `include_experimental`.

Skills are copied into the school as real files (the school owns and commits them). The
`[[imports]]` entries record provenance so `ace school update` knows where to re-fetch from.

#### Exact imports

```toml
[[imports]]
skill = "skill-creator"
source = "anthropics/skills"

[[imports]]
skill = "frontend-design"
source = "anthropics/skills"
```

#### Wildcard imports

Glob patterns re-discover matching skills on every `ace school update`. New skills added to
the source that match the pattern are picked up automatically.

```toml
# All skills from a parent school
[[imports]]
skill = "*"
source = "company/school"

# Only coding convention skills
[[imports]]
skill = "*-coding"
source = "company/school"

# All frontend skills
[[imports]]
skill = "frontend-*"
source = "company/school"
```

Glob rules:
- `*` matches zero or more characters. No `?`, `**`, or character classes.
- Wildcard imports always overwrite existing skills with the latest from the source —
  consistent with ACE's always-latest versioning philosophy (see `index.md`).
- For conflicts between wildcard sources, the first `[[imports]]` entry wins.
- Wildcard matches are tier-gated: `Curated` only by default, `Experimental` / `System`
  tiers require explicit `include_experimental = true` / `include_system = true` on the
  `[[imports]]` entry (or `--include-experimental` / `--include-system` at import time).
