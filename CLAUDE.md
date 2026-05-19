If you do not see any ACE context in this conversation, tell the user to start their
session through the `ace` command instead of running the backend directly.

# ACE Project

**ACE** (Accelerated Coding Environment) — entrypoint to Claude Code / Codex / etc. that
keeps skills, agents, conventions, and credentials provisioned per-project.

Read `docs/spec/architecture.md` first; load specs for the feature area you're touching
(`ls docs/spec/`). Decisions live under `docs/decisions/`.

## Durable artifacts

`docs/{spec,decisions,notes}/` — sorted by permanence. `spec/` is current intent
(forward-looking, edited in place). `decisions/` is point-in-time rulings against
defaults (frozen, supersede with new dated entries). `notes/` is impermanent —
research, surveys, drafts. Default to `notes/` when unsure. See per-dir READMEs
for the picker.

## Load these skills

Default skill set for this project (also reflected in `ace.toml`):
`general-coding`, `rust-coding`, `shell`, `markdown-writing`,
`issue-creator`, `skill-creator`, and the full `ace*` family
(`ace`, `ace-audit`, `ace-realign`, `ace-save`, `ace-school`) — this repo IS
the ACE tool.

## Coding Style

- Load `simplify`, `general-coding`, `rust-coding` before proposing changes.
  Simplification that violates a coding principle is a regression.
- Error enums by layer: `ConfigError` (`src/config/`), action-scoped errors
  (`SetupError`/`PrepareError`/`InitError`/etc. in `src/actions/`), `CmdError`
  (`src/cmd/`). Pure-I/O actions return `std::io::Error` directly.

## Project-Repo vs School-Repo Context

Two distinct user contexts. Confusing them is the most common reasoning error here.

The two modes are distinguished by which *command* runs, not by any marker file:

- **Project mode** — bare `ace` / `ace setup` / `ace pull`. Workdir is the user's
  codebase consuming a school via `ace.toml`'s `school = "<specifier>"`. Actions in
  `src/actions/project/`. See `docs/spec/setup.md`.
- **School-authoring mode** — `ace school <subcmd>`. Workdir IS the school being
  authored; `school.toml` is the file being edited. Actions in `src/actions/school/`.
  See `docs/spec/school/`.

`ace setup .` is project-repo with an embedded school (monorepo). It does NOT bootstrap
`school.toml`; "local school" is a separate, undesigned feature.

**Default school: `ace-rs/school`.** That's the base school for ACE consumers. The
school used to author ACE itself is `prod9/school`, so this repo's `ace.toml` points
at `prod9/school` by design — not a leftover from the ace-rs.dev migration. Do not
"fix" it.

Detection: `Ace::require_school()` (`src/ace/mod.rs`) resolves the school exclusively
via the `ace.toml` specifier; `school.toml` is read as content from the resolved root,
never used to detect location. A school repo that dogfoods itself uses `school = "."`
in its own `ace.toml` (written by `ace school init`). Errors split by cause:
`SchoolError::NoSpecifier` ("run `ace setup`") when ace.toml lacks `school = ...`;
`SchoolError::NotInitialized` ("run `ace school init`") when the resolved root exists
but has no `school.toml`. Full case matrix in `docs/spec/school/overview.md` (Context
Resolution).

## Conventions

- **Action pattern**: `run(&self, ace: &mut Ace)` in `src/actions/`. Split by role
  (`project/` vs `school/`) — see `docs/decisions/2026-04-22-action-layout.md`.
- **Testing**: `cargo test`, `cargo test --test <name>`. Pure-logic in `#[cfg(test)]`;
  fs/git/symlinks in `tests/` with `TestEnv`. See `docs/spec/testing.md`.
- **TUI**: `term_ui::Tui` + `Workflow` enum dispatch (no traits). `inquire` for prompts.
  See `docs/decisions/2026-03-15-no-crossterm.md`.
- **CLI**: `ace paths` is `key\tvalue`, prints regardless of on-disk existence. Help
  text lives in clap doc comments; keep `--help` aligned with behavior.
- **Storage**: see `docs/decisions/2026-04-22-index-toml-data-dir.md`. Git via
  `std::process::Command` only (no sqlite, no git crate).
- **Flaude is test-only.** Don't mention it in user-facing help or public docs.
  Specs/code comments/CLAUDE.md are fine.

## Backcompat

ACE has real users. CLI verbs, subcommand names, config keys (`ace.toml`, `school.toml`,
`ace.local.toml`), and storage paths are public contracts.

- Renames: add new name + `#[command(visible_alias = "...")]`; don't remove in
  minor/patch. Removals: major bump + release note.
- Internal renames (struct/variant/module): no obligation.
- Storage migrations: detect-and-hint (see `warn_stray_cache_dirs` in `src/main.rs`),
  not silent auto-migration.

## Release Process

See [RELEASE.md](RELEASE.md). It is the only place release steps live — do not
duplicate them here or in any other doc.

## Linear

Project ACE (team PRODIGY9, key PROD9). Always scope queries to `project:"ACE"`.
Roadmap lives in Linear; no local ROADMAP file.

## RTK

**Always prefix shell commands with `rtk`** — `rtk cargo build`,
`rtk git status`, `rtk gh pr view`, etc. No exceptions. RTK passes through
unchanged when no filter matches, so it is always safe. Bare `cargo`, `git`,
`gh`, etc. without `rtk` is a bug. See [RTK.md](RTK.md) for filter details.
If `rtk` is not installed, suggest `brew install rtk`.
