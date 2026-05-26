# school.toml

The `school.toml` file lives at the root of a school repository. It declares metadata,
configuration, MCP servers, environment, and project catalog for the school.

## Example

```toml
name = "Acme Corp"
session_prompt = """Always load the `acme-conventions` skill first. \
This org uses Postgres for storage and gRPC for service-to-service calls."""

[env]
NODE_VERSION = "22"
PYTHON_VERSION = "3.12"
LITELLM_BASE_URL = "https://llm.acme.corp/v1"

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

Display name for the school. Used in logs, UI, and fuzzy search. Not an identifier — the
school is identified by its GitHub `owner/repo` shorthand.

### `backend`

Optional top-level string. Default backend name for projects that consume this school,
used when no `ace.toml` / `ace.local.toml` / user config / CLI override sets one. Value is
a built-in name (`claude`, `codex`, `opencode`, `droid`) or a custom name declared in
`[[backends]]` below. See [backend.md → Resolution Order](../backend.md#resolution-order).

```toml
backend = "claude"
```

### `session_prompt`

Top-level string. Injected verbatim into every ACE session's prompt for projects that
consume this school. Use it for school-wide instructions that always apply —
load-this-skill nudges, org-wide conventions, coding-style reminders, etc.

```toml
session_prompt = """Always load the `acme-conventions` skill first.
This org uses Postgres for storage and gRPC for service-to-service calls.
Prefer `rtk`-prefixed shell commands."""
```

Layering order with the project-level `session_prompt` (in `ace.toml` / `ace.local.toml`)
is documented in [prompt-templating.md](../prompt-templating.md#composition). The school
layer renders before the project layer; both are injected verbatim with no template
substitution.

### `[env]`

Key-value pairs of environment variables. Set in the shell before exec-ing the backend.
Use for shared endpoints, API base URLs, feature flags, etc.

These are not secrets — secrets are managed by the backend's own OAuth flow when
connecting to remote MCP servers.

### `[[mcp]]`

Array of MCP server declarations. Each entry defines a remote MCP endpoint. ACE registers
these with the active backend (see [backend.md](../backend.md#mcp-server-registration) and
[mcp.md](../mcp.md) for design rationale).

- `name` — Identifier for the MCP server.
- `url` — Remote MCP endpoint URL. The backend discovers OAuth metadata via `.well-known`.

### `[[projects]]`

Catalog of projects in the organization. Gives the AI context about available repositories
and what they do, enabling better cross-project reasoning and navigation.

- `name` — Short project identifier.
- `repo` — Git-cloneable URL for the project.
- `description` — What the project is and does. Written for AI/LLM consumption — be
  specific about tech stack, domain, and responsibilities.
- `env` — Optional. Project-specific environment variables. Merged with top-level `[env]`
  (project values override).

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

Array of import declarations. Each entry tracks an upstream skills repository and which
skills from it are pulled into this school. Re-run by `ace school pull` (alias
`ace school update`).

The behavioral spec — match-handle semantics, tier expansion, cross-source merge — is
in [skills/selection.md → `[[imports]]` schema](../skills/selection.md#imports-schema).
This section documents the file shape.

#### Fields

- `source`               — GitHub `owner/repo` shorthand for the upstream skills repo.
- `skills`               — list of match handles. Required (or its backcompat alias
                           `skill`).
- `exclude_skills`       — list of match handles subtracted from `skills`. Optional.
- `include_experimental` — widen discovery to include `skills/.experimental/`.
                           Default `false`.
- `include_system`       — widen discovery to include `skills/.system/`. Default `false`.
- `include_internal`     — admit skills with `internal: true` via glob matches.
                           Default `false`.

`.curated/`, `.experimental/`, `.system/` are
[community conventions skills.sh recognizes](../skills/model.md#discovery-cascade) —
not ACE-owned categories.

#### Backcompat alias

The singular `skill = "<pattern>"` is accepted as an alias for
`skills = ["<pattern>"]`. Writers (`ace import`, `ace school *`) emit the plural form.
Per [CLAUDE.md § Backcompat](../../../CLAUDE.md), the singular form is not removed in
any minor / patch release.

#### Migration

ACE never proactively rewrites a `school.toml` that the user did not cause to be
saved. Singular `skill = ...` keys persist on disk until a write happens.

Writes happen during `ace import`, `ace school pull`, `ace school add-import`,
and any other `ace school <subcmd>` that updates `school.toml`. The writer
always emits the canonical plural form, so the whole file normalizes on the
next save — incremental modernization for free as authors continue using their
school.

`ace school fix` is the explicit one-shot — see
[school-commands.md → `ace school fix`](school-commands.md#ace-school-fix).
Schema-only re-serialize: read `school.toml`, write it back in canonical form.
No network, no import resolution. Idempotent.

ACE may emit a non-blocking hint when it reads singular keys, so the author
knows the next write will normalize.

#### Examples

Single skill:

```toml
[[imports]]
source = "anthropics/skills"
skills = ["skill-creator"]
```

Whole upstream:

```toml
[[imports]]
source = "company/school"
skills = ["*"]
```

Subset with subtraction (cross-source disjoint sets):

```toml
[[imports]]
source         = "ace-rs/school"
skills         = ["*"]
exclude_skills = ["rust-coding"]

[[imports]]
source = "my/customizations"
skills = ["rust-coding"]
```

Skills are copied into the school as real files (the school owns and commits them).
The `[[imports]]` entries record provenance so `ace school pull` knows where to
re-fetch from.
