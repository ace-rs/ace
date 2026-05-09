# Agent Ecosystem Research

Date: 2026-05-09

Research into agent-to-agent communication, extensibility systems,
and their implications for ACE across Claude Code, Codex, OpenCode,
and Droid.

---

## 1. Agent-to-Agent Communication (Local)

### Claude Code Agent Teams

Released with Opus 4.6 (February 2026). Experimental — requires
`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` in settings.json or
environment.

**Architecture.** One session acts as Team Lead; it spawns 2–16
Teammates via `TeamCreate`. Each teammate is an independent Claude
Code process with its own context window and full tool access. All
run in the same local machine.

**Coordination primitives:**

- **TaskCreate / TaskGet / TaskList / TaskUpdate** — file-based work
  queue. Tasks are JSON files under `~/.claude/tasks/{team-name}/`.
  Each task has an ID, subject, description (the prompt the teammate
  executes), status, owner, and dependency edges.
- **SendMessage** — typed peer-to-peer messaging (direct, broadcast,
  shutdown, plan-approval). Any teammate can message any other
  teammate or the team lead.
- **Worktree isolation** — each teammate gets its own git worktree
  for parallel file safety. Merges back to the lead's branch on
  completion.

**Communication is file-based IPC.** No shared memory, no sockets,
no daemon. The task JSON files on disk ARE the coordination protocol.
This is simple but means cross-backend coordination (e.g. a Claude
Code lead orchestrating a Codex teammate) is not possible — the
tools are Claude Code internals.

### Other Backends

No equivalent multi-agent coordination primitives were found for
Codex CLI or OpenCode. Droid has a "PLN Droid Orchestrator" plugin
for structured planning, but it orchestrates within a single session
rather than spawning independent agent processes.

Third-party orchestrators (e.g. `claude-code-workflow-orchestration`,
Shipyard) exist but are Claude Code-specific wrappers.

### Cross-Backend Coordination

No standard protocol exists. A hypothetical ACE-mediated
coordination layer would need to:

1. Define a backend-agnostic task format (superset of Claude Code's
   task JSON).
2. Provide a file-based or socket-based message bus that any backend
   can poll.
3. Handle worktree isolation generically (git worktrees work for
   any backend).

This is a significant engineering effort with unclear demand.


## 2. Claude Code Channels

**Confirmed feature.** Launched March 2026 (v2.1.80+). Channels
connect a running Claude Code session to external messaging
platforms — Telegram, Discord, and iMessage (macOS only, added
March 26).

**How it works.** Channels are implemented as MCP server plugins
that bridge Claude Code to messaging APIs. The plugin polls outward
(no inbound connections). Messages sent from Telegram/Discord are
relayed into the active Claude Code session, which processes them
with full local environment access (filesystem, git, MCP tools) and
replies back through the same channel.

**Security model (three layers):**

1. Plugin allowlist — only Anthropic-approved plugins during
   research preview.
2. Pairing-code authentication — only the paired user ID gets
   through.
3. No inbound connections — the plugin polls outward, no ports
   exposed.

**Requirements:** Bun runtime, claude.ai Pro or Max subscription.

**Relevance to ACE.** Channels are a remote-control interface, not
an inter-agent protocol. They let a human interact with a running
session from a mobile device. Schools could potentially mandate
channel configuration, but the use case is narrow (remote
monitoring of long-running agent sessions).


## 3. Claude Code Monitors

**Confirmed feature.** Shipped April 9, 2026 (v2.1.98). The Monitor
tool lets the agent watch a background process and receive
notifications when something meaningful happens.

**How it works.** The agent starts a subprocess (log tail, build
process, dev server) via the Monitor tool. Each stdout line from the
subprocess becomes a notification injected into the conversation.
The agent continues working on other tasks while monitoring —
interrupt-driven rather than polling.

**Key properties:**

- Each stdout line is a conversation message; lines within 200ms
  are batched.
- `persistent: true` for session-length watches (PR monitoring,
  log tails).
- Non-persistent monitors have a configurable timeout (default
  300s, max 3600s).
- The agent can cancel via `TaskStop`.
- Stderr goes to an output file (readable via Read) but does not
  trigger notifications unless merged with `2>&1`.

**Use cases:** dev server error watching, CI status polling, log
tailing, file change detection (`inotifywait`), PR comment
streaming.

**Relevance to ACE.** Monitors are a Claude Code-internal tool. No
equivalent exists in Codex or OpenCode. Schools cannot configure
monitors directly — they are invoked by the agent at runtime based
on context. However, a skill could instruct the agent to set up
specific monitors (e.g. "always monitor the dev server during
development").


## 4. Backend Plugin Support Parity

### Claude Code

The most mature extensibility stack. Six mechanisms:

| Mechanism     | Description                                |
| ------------- | ------------------------------------------ |
| Skills        | `.claude/skills/<name>/SKILL.md` —         |
|               | markdown files with frontmatter for        |
|               | auto-invocation and slash commands          |
| Hooks         | Shell commands at lifecycle events          |
|               | (pre/post tool use, session start/end)     |
| MCP servers   | Remote (streamable HTTP, OAuth 2.1) or     |
|               | local (stdio) tool/resource providers      |
| Plugins       | Distributable packages bundling skills,    |
|               | agents, hooks, MCP configs, and LSP        |
|               | configs under `.claude-plugin/plugin.json` |
| Agents        | Custom agent definitions in `agents/`      |
| Slash commands | Legacy — unified into skills as of 2026   |

**Plugin format.** A directory with `.claude-plugin/plugin.json`
manifest. Contains `skills/`, `agents/`, `hooks/`, `.mcp.json`,
`.lsp.json`. Installable from marketplaces (official Anthropic
marketplace auto-available). Path variable
`${CLAUDE_PLUGIN_ROOT}` for referencing bundled assets.

**Deferred tool loading.** Claude Code loads only tool names at
startup; full schemas are fetched on demand via `ToolSearch`.
Reduces context overhead for large MCP tool sets.

### Codex (OpenAI)

Rapidly catching up. Plugin system shipped with 90+ plugins in
April 2026.

| Mechanism     | Description                                |
| ------------- | ------------------------------------------ |
| Plugins       | Bundles skills, app integrations, and MCP  |
|               | servers. Installable from marketplaces     |
|               | (GitHub, Git, local).                      |
| MCP servers   | Configured in `config.toml`                |
|               | (`~/.codex/config.toml` or                 |
|               | `.codex/config.toml`). Supports stdio and  |
|               | remote HTTP.                               |
| Skills        | Built-in `@plugin-creator` skill for       |
|               | scaffolding.                               |
| CLI plugin    | `codex mcp` for MCP management. `codex     |
|  management   | plugin` for marketplace browsing/install.  |

**Key difference from Claude Code:** Codex plugins are more
marketplace-centric — MCP servers bundled inside plugins are
auto-configured on install. The plugin format appears to be
converging with Claude Code's (both use MCP as the tool layer),
but the manifest format differs.

### OpenCode

Open-source, Go-based. Flexible plugin system.

| Mechanism     | Description                                |
| ------------- | ------------------------------------------ |
| Plugins       | JS/TS modules exporting plugin functions.  |
|               | Loaded from a plugin directory or npm      |
|               | packages (auto-installed via Bun).         |
| Custom tools  | Plugin-defined tools available alongside   |
|               | built-ins. Same-name overrides built-ins.  |
| Hooks         | 25+ lifecycle events: `chat.message`,      |
|               | `chat.params`, `permission.ask`,           |
|               | `tool.execute.before/after`.               |
| MCP servers   | Supported via configuration.               |

**Key difference:** OpenCode plugins are code-first (JS/TS) rather
than declarative (markdown/JSON). This gives more power (intercept
tool execution, modify runtime behavior, inject env vars) but
requires a JS runtime. No marketplace system — plugins are local
files or npm packages.

### Droid (Factory)

Enterprise-focused. Plugin system mirrors Claude Code closely.

| Mechanism     | Description                                |
| ------------- | ------------------------------------------ |
| Plugins       | Directory with `.factory-plugin/plugin.json`|
|               | manifest. Contains skills, commands,       |
|               | agents, hooks, MCP configs.                |
| MCP servers   | `mcp.json` in `.factory/` or `~/.factory/`.|
|               | 40+ curated servers in built-in registry.  |
|               | OAuth and stdio transports.                |
| Skills        | Markdown-based, same pattern as Claude     |
|               | Code.                                      |
| Interop       | Claims compatibility with Claude Code      |
|               | plugins — can install them directly.       |

**Key difference:** Droid's plugin format is intentionally
interoperable with Claude Code's. If this claim holds, ACE could
potentially produce plugins that work on both backends.

### Parity Summary

| Feature            | Claude | Codex | OpenCode | Droid |
| ------------------ | :----: | :---: | :------: | :---: |
| Plugin packaging   |   Y    |   Y   |    Y     |   Y   |
| MCP servers        |   Y    |   Y   |    Y     |   Y   |
| Skills (markdown)  |   Y    |   Y   |    N     |   Y   |
| Lifecycle hooks    |   Y    |   N   |    Y     |   Y   |
| Marketplace        |   Y    |   Y   |    N     |   Y   |
| Agent teams        |   Y    |   N   |    N     |   N   |
| Channels           |   Y    |   N   |    N     |   N   |
| Monitor tool       |   Y    |   N   |    N     |   N   |
| Plugin interop     |   —    |   N   |    N     |   Y   |


## 5. Could ACE Have Been a Plugin?

### What Maps to Plugin APIs

| ACE Feature             | Plugin-able? | Notes                    |
| ----------------------- | :----------: | ------------------------ |
| Skill provisioning      |      Y       | Skills are native to     |
|                         |              | Claude/Codex/Droid       |
|                         |              | plugins.                 |
| CLAUDE.md injection     |    Partial   | Plugins can ship skills  |
|                         |              | but not overwrite the    |
|                         |              | root instructions file.  |
| MCP server config       |      Y       | `.mcp.json` in plugin    |
|                         |              | format.                  |
| Hooks configuration     |      Y       | `hooks/` in plugin       |
|                         |              | format.                  |
| Environment variables   |      N       | Plugins cannot set env   |
|                         |              | vars for the host        |
|                         |              | process. Requires a      |
|                         |              | launcher.                |
| Session management      |      N       | Resume, trust level,     |
|                         |              | backend selection —      |
|                         |              | these are launch-time    |
|                         |              | concerns outside plugin  |
|                         |              | scope.                   |
| School management       |      N       | Git clone, index, update |
|                         |              | — lifecycle management   |
|                         |              | that plugins cannot do.  |
| Backend selection       |      N       | Fundamental: a plugin    |
|                         |              | runs INSIDE a backend.   |
| Multi-backend support   |      N       | A plugin is              |
|                         |              | backend-specific by      |
|                         |              | definition.              |
| Skill resolution/learn  |      N       | Cross-cutting concern    |
|                         |              | that examines the        |
|                         |              | project and edits config.|

### Verdict: No, ACE Cannot Be Just a Plugin

The core value proposition of ACE — provisioning a consistent
environment ACROSS backends from a shared school — is fundamentally
a launcher concern. A plugin runs inside one backend; ACE runs
before any backend starts.

**What a plugin cannot do that ACE does:**

1. **Choose which backend to launch.** A Claude Code plugin cannot
   decide to launch Codex instead.
2. **Set environment variables.** `ANTHROPIC_BASE_URL`, custom env
   from school config — these must be set before the backend
   process starts.
3. **Manage school lifecycle.** Clone, pull, index — these are
   pre-session operations.
4. **Resolve skills across scopes.** The four-layer merge
   (user/project/local/school) with include/exclude patterns
   happens before any backend sees the skills.
5. **Register MCP servers.** ACE registers at user scope via
   backend CLI before the session starts. Plugins ship MCP configs
   but cannot call `claude mcp add` on the host.

### Could ACE Be a Hybrid?

**Yes, and this is worth considering.** A hybrid approach:

- **Launcher (Rust binary, current ACE):** School management,
  backend selection, env setup, MCP registration, skill resolution,
  session management. Runs before the backend.
- **Plugin (per-backend):** Skills, hooks, agents, commands that
  the school provides. Installed/updated by the launcher as part of
  `ace setup` / `ace` prepare.

The launcher would generate or symlink a plugin directory that the
backend discovers natively. This is essentially what ACE already
does with its Link action (symlinking `skills/`, `commands/`,
`agents/` into `.claude/`), but formalizing it as a plugin would
give ACE access to hooks, `.mcp.json`, `.lsp.json`, and agent
definitions through the plugin manifest.

**Tradeoffs of hybrid approach:**

| Pro                                 | Con                         |
| ----------------------------------- | --------------------------- |
| Native plugin discovery — no        | Must maintain plugin        |
| symlink hacks                       | manifests per backend       |
| Access to hooks, LSP, agents        | Plugin format may diverge   |
| through plugin API                  | across backends             |
| Marketplace distribution possible   | ACE's value is the          |
| for school content                  | cross-backend story;        |
|                                     | plugins are per-backend     |
| Backend treats school content as    | Tight coupling to plugin    |
| first-class                         | format versioning           |


## 6. School-Mandated Backend Configuration

### What Schools Can Mandate Today

Via `school.toml`:

- Backend selection (`backend = "claude"`)
- Custom backend declarations (`[[backends]]`)
- MCP server registration (`[[mcp]]`)
- Skills (bundled in `skills/`)
- Rules, commands, agents (linked folders)
- Environment variables (`[env]`)
- Session prompt

### What Schools Could Mandate With New Features

**Plugins.** If ACE generates a plugin manifest during Link, schools
could mandate:

```toml
# Hypothetical school.toml additions

# Backend plugins to install from marketplaces
[[plugins]]
name = "code-review"
source = "claude-plugins-official"
backends = ["claude", "droid"]  # only install on these backends

[[plugins]]
name = "linear-integration"
source = "https://github.com/org/linear-plugin"
```

**Channels.** Possible but narrow use case:

```toml
# Hypothetical — channel config is user-specific
[channels]
telegram = { enabled = true }  # school suggests, user opts in
```

Channel configuration is inherently personal (pairing codes, user
IDs). Schools can suggest but not mandate.

**Monitors.** Cannot be configured statically — monitors are
runtime tools invoked by the agent. Schools can influence monitoring
behavior through skills:

```markdown
# skills/dev-server-watch/SKILL.md
---
trigger: auto
---
When starting a dev server, use the Monitor tool to watch for
compilation errors and test failures.
```

**Agent Teams.** Could be school-mandated:

```toml
# Hypothetical
[agent_teams]
enabled = true
max_teammates = 4
```

But agent teams are Claude Code-only and experimental. Premature
to build school support.

### Limitations

1. **Backend-specific features cannot be mandated cross-backend.**
   Channels, monitors, and agent teams are Claude Code-only. A
   school mandating them would need backend-conditional config.
2. **Plugin format divergence.** Even if Droid claims Claude Code
   plugin interop, Codex and OpenCode use different formats. ACE
   would need to generate per-backend plugin manifests.
3. **Auth is user-specific.** MCP servers, channels, and plugins
   that require authentication cannot be fully provisioned by a
   school — the user must complete auth interactively.
4. **Experimental features are unstable.** Agent teams, channels,
   and monitors are all recent additions. Building school-level
   configuration around them risks churn.


## Implications for ACE

### Confirmed Decisions

1. **ACE must remain a launcher.** The plugin analysis confirms
   that ACE's core value (cross-backend provisioning, school
   lifecycle, env setup) cannot be delivered as a plugin. The
   current architecture is correct.

2. **The Link action is proto-plugin generation.** ACE already
   symlinks school content into backend directories. Formalizing
   this as plugin manifest generation is a natural evolution, not
   a redesign.

3. **MCP is the universal extension point.** All four backends
   support MCP servers. ACE's current MCP registration strategy
   (school.toml `[[mcp]]` entries registered via backend CLI) is
   the most portable extensibility mechanism.

### Opportunities

4. **Plugin manifest generation.** ACE could generate a
   `.claude-plugin/plugin.json` (and equivalents for other
   backends) during Link. This would give school content native
   plugin status — hooks, LSP configs, and agent definitions
   would work without additional ACE-side support.

5. **Plugin installation.** Schools could declare third-party
   plugins to install. ACE would call `claude plugin install` /
   `codex plugin install` during Prepare. Low effort, high value
   for teams that standardize on specific plugins.

6. **Monitor-aware skills.** Schools can ship skills that instruct
   agents to use monitors for specific workflows. No ACE code
   changes needed — this is pure skill content.

### Not Worth Pursuing Now

7. **Agent team orchestration.** Claude Code-only, experimental,
   file-based IPC with no cross-backend path. Wait for the feature
   to stabilize and for other backends to adopt multi-agent
   primitives.

8. **Channel configuration.** Too personal, too narrow. Channels
   are a user workflow preference, not a team-level concern.

9. **Cross-backend agent coordination.** No standard protocol
   exists. The demand is speculative. If it emerges, ACE is
   well-positioned as the launcher that could mediate, but
   building it now would be premature.

### Schema Sketch for Plugin Support

If plugin installation is pursued, the minimal `school.toml`
addition would be:

```toml
[[plugins]]
name = "code-review"
source = "claude-plugins-official"

[[plugins]]
name = "custom-lint"
source = "https://github.com/org/custom-lint-plugin"
```

And `ace.toml` could override:

```toml
exclude_plugins = ["code-review"]  # opt out of school plugin
```

This mirrors the existing `skills` / `exclude_skills` pattern.
