# ACE Overview

ACE (Accelerated Coding Environment) is a CLI gateway into Claude Code or Codex. It ensures the
development environment is properly configured and up-to-date before handing off to the
underlying AI coding tool.

## Table of Contents

- [index.md](index.md) — This file. Philosophy, school concept, lifecycle.
- [configuration.md](configuration.md) — Config file locations, layering, format.
- [architecture.md](architecture.md) — Layers, data flow, dependency direction.
- [setup.md](setup.md) — `ace setup` first-run flow.
- [roles.md](roles.md) — Role definitions, selection, prompt injection.
- [skills-sync.md](skills-sync.md) — School folder sync (skills, rules, commands, agents).
- [prompt-templating.md](prompt-templating.md) — Session prompt composition and template rendering.
- [mcp.md](mcp.md) — MCP server design (remote-only, OAuth delegation).
- [authentication.md](authentication.md) — Authentication (MCP OAuth, school repo access).
- [backend.md](backend.md) — Backend abstraction contract.
- [backend-install.md](backend-install.md) — Backend installation and readiness.
- [backends/claude.md](backends/claude.md) — Claude backend: permission modes, MCP, linked folders.
- [backends/codex.md](backends/codex.md) — Codex backend specifics.
- [school/overview.md](school/overview.md) — School repository structure.
- [school/school-toml.md](school/school-toml.md) — `school.toml` format reference.
- [school/school-commands.md](school/school-commands.md) — `ace school` subcommands.
- [testing.md](testing.md) — Integration test strategy, TestEnv pattern.
- [upgrade.md](upgrade.md) — Self-update: version check, background upgrade, `ace upgrade`.
- [learn.md](learn.md) — `ace learn`: project study, instructions-file edit, `ace.toml` skills filter narrowing.

## Philosophy

ACE is strictly a development tool. It optimizes for developer ergonomics over production
security concerns. Sharing credentials in config is acceptable since no production secrets
should ever be managed through ACE.

Convention over configuration. Do the obvious right thing automatically. Never assume
non-obvious defaults — ask instead.

GitHub is the assumed default host. `owner/repo` shorthand maps to
`https://github.com/owner/repo`.

Get the user into coding as fast as possible. Never block on operations that can be deferred.

## Versioning Philosophy

Skills always track latest main — projects never pin to a specific version.

Skills model how teams actually work. When a team agrees on a new convention, that decision
applies immediately to all ongoing work — nobody files a ticket to "upgrade" each project.
A school change propagates to every project on next sync, no per-project ceremony required.

Schools evolve independently of projects. Skills should work across any project at any point in
its history — when compatibility issues arise, the LLM resolves the gap itself.

Version-pinning assumes a dumb consumer that breaks on interface changes. LLMs are not dumb
consumers — they read the skill, adapt, and resolve compatibility gaps themselves. The execution
engine is non-deterministic at every level (model versions, prompt evolution, run-to-run variance),
so pinning cannot make a non-reproducible pipeline reproducible. Skills with companion scripts make
it worse: new prompts against old code, old tools against new code. The combinatorial matrix is
unwinnable.

This is a deliberate departure from lockfile-and-pin paradigms. The skills folder captures intent
and preferences, not reproducible builds. Changes are still tracked — schools are git repositories
with full commit history. What ACE avoids is per-project pinning to a specific school revision.

Wildcard imports (`skill = "*"`, `skill = "frontend-*"`) follow the same principle: always pull
latest, always overwrite. This is how the parent school pattern works
(see `school/school-commands.md`).

## School

A school is a git-cloneable source repository containing skills, conventions, agent configs, and
other shared resources for an organization. See [school/overview.md](school/overview.md) for
full details on specifiers, structure, and relationship to projects.

## Lifecycle

1. **Discover config files** — find user-global, project-local, project-committed
2. **Setup check** — if no config found, error and tell the user to run `ace setup` (see [setup.md](setup.md))
3. **Parse and merge** — layer configs together
4. **Register MCP servers** — register `[[mcp]]` entries into the backend
5. **Fetch school** — `git fetch` the school's repo (clone on first run)
6. **Sync school folders** — pull latest and link school folders (skills, rules, commands, agents) into the project
7. **Select role** — if school defines `[[roles]]` and no role set, prompt user to pick (see [roles.md](roles.md))
8. **Check tooling** — required CLI tools, language runtimes, etc.
9. **Check project setup** — CLAUDE.md, MCP configs, project-specific requirements from source
10. **Select backend** — Claude Code or Codex
11. **Inject prompt** — prepend system context about skills, role, and school workflow
12. **Version check** — read cache marker, run `git ls-remote` if stale, print hint and spawn background upgrade if newer version available. Skipped for `ace upgrade`, `ace --version`, `--porcelain`, `skip_update`, `ACE_SKIP_UPDATE=1`. See [upgrade.md](upgrade.md).
13. **Exec** — replace process with the chosen tool
