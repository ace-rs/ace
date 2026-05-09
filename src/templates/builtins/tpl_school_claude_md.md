# {{ school_name }} — School Repository

This is an ACE school repository. It provides skills, conventions, and session prompts to
projects that subscribe to it via `ace setup`.

Run `ace` to start a coding session for developing this school.

## Structure

- `school.toml` — school configuration (see sections below)
- `skills/` — skill directories, each with a `SKILL.md`

## school.toml Sections

- **`name`** — school display name
- **`[env]`** — shared environment variables (endpoints, feature flags — not secrets)
- **`[[mcp]]`** — remote MCP server endpoints (name, url). The backend handles OAuth.
- **`[[projects]]`** — project catalog (name, repo, description, optional per-project env)
- **`[[imports]]`** — provenance tracking for skills imported via `ace import`

## Adding MCP Servers

Append directly to school.toml:

```toml
[[mcp]]
name = "github"
url = "https://api.githubcopilot.com/mcp/"

[[mcp]]
name = "linear"
url = "https://mcp.linear.app/sse"
```

Fields: `name` (unique identifier), `url` (remote MCP endpoint). The backend discovers
OAuth metadata and handles authentication automatically.

## Commit Messages

Every commit to this repo is a policy change — it affects how every
developer using this school writes code. Commit messages must explain the
decision, not just the diff.

Summary line: short, imperative, what changed.

Body: a policy memo answering three questions:
1. **What prompted this** — the pattern or problem observed
2. **What's changing** — the new rule, stated clearly
3. **Why this is better** — concrete example or rationale

Example:

```
Require error propagation over unwrap()

We've seen repeated panics from unwrap() in non-critical paths across
multiple projects. The cost of a crash far exceeds the cost of writing
a proper error path.

New rule: always propagate with ? or handle explicitly. unwrap() is
banned outside of tests. In tests, use .expect("reason") so failures
have context.

Example of the problem:
  let config = load_config().unwrap();  // panics if file missing

What we want instead:
  let config = load_config()?;  // caller decides how to handle
```

## Useful Commands

- `ace school pull` — re-fetch all imported skills from their sources
- `ace diff` — show uncommitted changes in the school clone
- `ace import <source>` — import a skill from an external repo