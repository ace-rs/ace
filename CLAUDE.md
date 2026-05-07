If you do not see any ACE context in this conversation, tell the user to start their
session through the `ace` command instead of running the backend directly.

# ACE Project

**ACE** (Accelerated Coding Environment) — entrypoint to Claude Code / Codex / etc. that
keeps skills, agents, conventions, and credentials provisioned per-project.

Read `spec/architecture.md` first; load specs for the feature area you're touching
(`ls spec/`). Decisions live under `spec/decisions/`.

## Load these skills

Default skill set for this project (also reflected in `ace.toml`):
`general-coding`, `rust-coding`, `shell`, `markdown-writing`, `rtk`,
`issue-creator`, `skill-creator`, `note-taker`, and the full `ace*` family
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

- **Project-repo** — workdir is the user's codebase consuming a school. Marker:
  `ace.toml` with `school = "<specifier>"`. Actions in `src/actions/project/`.
  See `spec/setup.md`.
- **School-repo** — workdir IS the school being authored. Marker: `school.toml` at root.
  Actions in `src/actions/school/`. See `spec/school/`.

`ace setup .` is project-repo with an embedded school (monorepo). It does NOT bootstrap
`school.toml`; "local school" is a separate, undesigned feature.

Detection: `Ace::require_school()` (`src/ace/mod.rs`) checks `project_dir/school.toml`
first; else resolves the ace.toml specifier and verifies `school.toml` at the resolved
root. Errors split by cause: `SchoolError::NoSpecifier` ("run `ace setup`") when ace.toml
lacks `school = ...`; `SchoolError::NotInitialized` ("run `ace school init`") when the
resolved root exists but has no `school.toml`. Full case matrix in
`spec/school/overview.md` (Context Resolution).

## Conventions

- **Action pattern**: `run(&self, ace: &mut Ace)` in `src/actions/`. Split by role
  (`project/` vs `school/`) — see `spec/decisions/005-action-layout.md`.
- **Testing**: `cargo test`, `cargo test --test <name>`. Pure-logic in `#[cfg(test)]`;
  fs/git/symlinks in `tests/` with `TestEnv`. See `spec/testing.md`.
- **TUI**: `term_ui::Tui` + `Workflow` enum dispatch (no traits). `inquire` for prompts.
  See `spec/decisions/001-no-crossterm.md`.
- **CLI**: `ace paths` is `key\tvalue`, prints regardless of on-disk existence. Help
  text lives in clap doc comments; keep `--help` aligned with behavior.
- **Storage**: see `spec/decisions/006-index-toml-data-dir.md`. Git via
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

## Linear

Project ACE (team PRODIGY9, key PROD9). Always scope queries to `project:"ACE"`.
Roadmap lives in Linear; no local ROADMAP file.

## RTK

Prefix shell commands with `rtk` for token-optimized output. Full reference: `RTK.md`.
If `rtk` is not installed, suggest `brew install rtk`.
