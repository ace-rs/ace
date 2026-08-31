# ACE Overview

ACE (Accelerated Coding Environment) is a CLI gateway into Claude Code, Codex, or
OpenCode. It ensures the development environment is properly configured and up-to-date
before handing off to the underlying AI coding tool.

## Philosophy

ACE is strictly a development tool. It optimizes for developer ergonomics over production
security concerns. Sharing credentials in config is acceptable since no production secrets
should ever be managed through ACE.

Convention over configuration. Do the obvious right thing automatically. Never assume
non-obvious defaults — ask instead.

GitHub is the assumed default host. `owner/repo` shorthand maps to
`https://github.com/owner/repo`.

Get the user into coding as fast as possible. Never block on operations that can be
deferred.

## Versioning Philosophy

Skills always track latest main — projects never pin to a specific version.

Skills model how teams actually work. When a team agrees on a new convention, that
decision applies immediately to all ongoing work — nobody files a ticket to "upgrade" each
project. A linked-school change propagates to every consuming project on next sync, no
per-project ceremony required.

Schools evolve independently of projects. Skills should work across any project at any
point in its history — when compatibility issues arise, the LLM resolves the gap itself.

Version-pinning assumes a dumb consumer that breaks on interface changes. LLMs are not
dumb consumers — they read the skill, adapt, and resolve compatibility gaps themselves.
The execution engine is non-deterministic at every level (model versions, prompt
evolution, run-to-run variance), so pinning cannot make a non-reproducible pipeline
reproducible. Skills with companion scripts make it worse: new prompts against old code,
old tools against new code. The combinatorial matrix is unwinnable.

This is a deliberate departure from lockfile-and-pin paradigms. The skills folder captures
intent and preferences, not reproducible builds. Changes are still tracked — schools are
git repositories with full commit history. What ACE avoids is per-project pinning to a
specific school revision.

Wildcard imports (`skill = "*"`, `skill = "frontend-*"`) follow the same principle:
always pull latest, always overwrite. This is authoring-side — how an authored school
inherits from a parent import source (see `school/school-commands.md` § Parent school
pattern).

## School

A school is a git-cloneable source repository containing skills, conventions, agent
configs, and other shared resources for an organization. See
[school/overview.md](school/overview.md) for full details on specifiers, structure, and
relationship to projects.

## Lifecycle

1. **Discover entry** — check the current directory for `workspace.toml`; otherwise find
   user-global, project-local, and project-committed ACE config.
2. **Setup check** — if neither a workspace manifest nor ACE config resolves, error and
   tell the user to run `ace setup` (see [setup.md](setup.md)).
3. **Parse and merge** — expand workspace members when present, then resolve each
   instance's configuration independently.
4. **Register MCP servers** — register `[[mcp]]` entries into the backend
5. **Fetch school** — `git fetch` the linked school's repo (clone on first run)
6. **Sync school folders** — pull latest and link the linked school's folders (skills,
   rules, commands, agents) into the project
7. **Check tooling** — required CLI tools, language runtimes, etc.
8. **Check project setup** — CLAUDE.md, MCP configs, project-specific requirements from
   source
9. **Select backend** — Claude Code, Codex, or OpenCode
10. **Inject prompt** — prepend system context about skills and school workflow
11. **Version check** — read cache marker, `GET https://ace-rs.dev/latest` if stale, print
    hint and spawn background upgrade if newer version available. Skipped for `ace upgrade`,
    `ace --version`, `--porcelain`, `skip_update`, `ACE_SKIP_UPDATE=1`. See
    [upgrade.md](upgrade.md).
12. **Configure instances** — construct one `Ace` per project, apply its resolved
    configuration, and choose a structurally valid `StartMode`; workspace mode may
    produce several independently configured instances.
13. **Build session process** — translate each instance into its backend's native
    `SessionProcess`, including the configured command, arguments, environment, and
    working directory.
14. **Execute** — supervise the foreground process and attach the backend's own terminal
    UI.

The later managed path extends these stages with feature requirements, backend process
topology, feature decoration, and mux execution. Controlled multi-process startup,
[managed sessions](session.md), [connect](connect.md), and [workspaces](workspace.md)
land only with their endpoint, readiness, backend-handle, and ownership contracts.
