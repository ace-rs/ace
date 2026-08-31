# Spec & architecture

**Current-understanding durable artifacts** — the design of the project and how it
actually fits together: design specs, RFCs, interface contracts, architecture / "how it
works" overviews, *and our own exact surface* (our CLI flags, config keys, API, schemas).
Prose you read to understand the system, plus the lookup facts about our own thing.
Updated in place; always reflects present design, not history.

A ruling on a question is a decision — `../decisions/`. A *third-party* surface (a
framework's API, another product's flags) is `../vendor/`. Research, a survey, or a draft
is `../scratch/`.

## Index

### System

- [`overview.md`](overview.md) — product philosophy, school concept, and lifecycle.
- [`architecture.md`](architecture.md) — layers, data flow, and dependency direction.
- [`platforms.md`](platforms.md) — supported compilation targets and platform limits.
- [`configuration.md`](configuration.md) — config locations, layering, and format.
- [`authentication.md`](authentication.md) — MCP OAuth and school repository access.
- [`migrations.md`](migrations.md) — on-disk stores, layout versions, and migrations.
- [`testing.md`](testing.md) — integration-test strategy and the `TestEnv` pattern.

### User surface

- [`setup.md`](setup.md) — first-run setup.
- [`ux.md`](ux.md) — terminal interaction and output contracts.
- [`exit-codes.md`](exit-codes.md) — process exit-code contract.
- [`upgrade.md`](upgrade.md) — version checks and self-update.
- [`session.md`](session.md) — managed ACE instances, startup planning, threads, and
  tmux.
- [`connect.md`](connect.md) — local agent bridge.
- [`workspace.md`](workspace.md) — multi-repository session composition.
- [`prompt-templating.md`](prompt-templating.md) — session prompt composition.
- [`mcp.md`](mcp.md) — MCP server configuration and lifecycle.

### Backends

- [`backend.md`](backend.md) — shared backend abstraction.
- [`backends/claude.md`](backends/claude.md) — Claude-specific behavior.
- [`backends/codex.md`](backends/codex.md) — Codex-specific behavior.
- [`backends/opencode.md`](backends/opencode.md) — OpenCode-specific behavior.

### Schools and skills

- [`school/overview.md`](school/overview.md) — school repository model.
- [`school/school-toml.md`](school/school-toml.md) — `school.toml` surface.
- [`school/school-commands.md`](school/school-commands.md) — school commands.
- [`school/standard-imports.md`](school/standard-imports.md) — seeded standard imports.
- [`skills/model.md`](skills/model.md) — skill identity and discovery.
- [`skills/selection.md`](skills/selection.md) — matching, imports, and merge behavior.
- [`skills/emit.md`](skills/emit.md) — backend materialization.
- [`skills/sync.md`](skills/sync.md) — fetch, link, and reconciliation.
- [`skills/lifecycle.md`](skills/lifecycle.md) — validated skill typestate.

## Format

One file per subject: `<slug>.md` (no date prefix — describes a thing, not the moment it
was written). Every file describes the current accepted system; unsettled or historical
material belongs elsewhere.
