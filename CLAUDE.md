If you do not see any ACE context in this conversation, tell the user to start their
session through the `ace` command instead of running the backend directly.

# ACE Project

**ACE** (Accelerated Coding Environment) — entrypoint to Claude Code / Codex / etc. that
keeps skills, agents, conventions, and credentials provisioned per-project.

Read `docs/spec/architecture.md` first; load specs for the feature area you're touching
(`ls docs/spec/`). Decisions live under `docs/decisions/`.

## Durable artifacts

`docs/` — file by the routing gate in `docs/README.md`: a ruling → `decisions/`;
third-party lookup → `vendor/`; a how-to → `guides/`; our own design/surface →
`spec/`; unsettled exploration → `scratch/` (last resort, opened with a
"not spec/decision because ___" line). Nothing defaults to `scratch/`.

## Load these skills

This repo IS the ACE tool, so the full `ace*` family applies (`ace`, `ace-afk`,
`ace-audit`, `ace-connect`, `ace-docs`, `ace-init`, `ace-realign`, `ace-save`,
`ace-school`) — those come from the user-level config, not `ace.toml`.

Project-level additions live in `ace.toml`: `general-coding`, `rust-coding`,
`skill-creator`, `skill-reviewer`.

## Coding Style

- Load `simplify`, `general-coding`, `rust-coding` before proposing changes.
  Simplification that violates a coding principle is a regression.
- **Formatting**: the toolchain is pinned (`rust-toolchain.toml`, stable 1.96),
  so `cargo fmt` is deterministic and idempotent across the tree — just run it.
  No per-file `rustfmt`, no `--edition`/`--style-edition` flags (those were a
  workaround for the old unpinned-nightly drift; the pin retired it). If you
  remove or bump the pin, re-verify `cargo fmt --check` is clean before relying
  on it again.
- Error enums by layer: `ConfigError` (`src/config/`), action-scoped errors
  (`SetupError`/`PrepareError`/`InitError`/etc. in `src/actions/`), `CmdError`
  (`src/cmd/`). Pure-I/O actions return `std::io::Error` directly.
- `CmdError` carries a process exit class via `ExitCode` (`exit_code()`,
  dispatched by `exit_on_err`). Build ad-hoc errors with `CmdError::usage`/
  `unavailable`/`failed` — the exit class is mandatory at construction; there is
  no catch-all `Other`. New leaf-error variants must be routed in the matching
  `*_exit_code` classifier. Contract: `docs/spec/exit-codes.md`.

## Project-Repo vs School-Repo Context

Two distinct user contexts. Confusing them is the most common reasoning error here.

The two modes are distinguished by which *command* runs, not by any marker file:

- **Project mode** — bare `ace` / `ace setup` / `ace pull`. Workdir is the user's
  codebase consuming its **linked school** via `ace.toml`'s `school = "<specifier>"`.
  Actions in `src/actions/project/`. See `docs/spec/setup.md`.
- **School-authoring mode** — `ace school <subcmd>` and `ace import`. Workdir IS the
  **authored school**; `school.toml` is the file being edited. Resolution is cwd-first
  with an announced fallback to the linked school — never `require_linked_school`
  directly. Actions in `src/actions/school/`. See `docs/spec/school/`.

Glossary (canonical, `docs/spec/school/overview.md`): **linked school** = consumed via
the specifier; **authored school** = under edit in cwd. Retired synonyms: "active
school", "school in use", "local school".

`ace setup .` is project-repo with an embedded school (monorepo). It does NOT bootstrap
`school.toml`; a same-repo authored school is a separate, undesigned feature.

**Default school: `ace-rs/school`.** That's the base linked school for ACE consumers.
This repo's own `ace.toml` links `prod9/school` by design — not a leftover from the
ace-rs.dev migration. Do not "fix" it.

Detection: `Ace::require_linked_school()` (`src/ace/mod.rs`) resolves the **linked
school** exclusively via the `ace.toml` specifier; `school.toml` is read as content
from the resolved root, never used to detect location. Authoring commands never call it
directly. A dogfooding school uses `school = "."` in its own `ace.toml` (written by
`ace school init`) — the only case where authored and linked coincide. Errors split by
cause: `SchoolError::NoSpecifier` ("run `ace setup`") when ace.toml lacks
`school = ...`; `SchoolError::NotInitialized` ("run `ace school init`") when the
resolved root exists but has no `school.toml`. Full case matrix in
`docs/spec/school/overview.md` (Linked-School Resolution).

## Conventions

- **Action pattern**: `run(&self, ace: &mut Ace)` in `src/actions/`. Split by role
  (`project/` vs `school/`) — see `docs/spec/architecture.md` § `actions/`.
- **Testing**: `cargo test`, `cargo test --test <name>`. Pure-logic in `#[cfg(test)]`;
  fs/git/symlinks in `tests/` with `TestEnv`. See `docs/spec/testing.md`.
- **TUI**: `term_ui::Tui` + `Workflow` enum dispatch (no traits). `inquire` for prompts.
  See `docs/decisions/2026-03-15-no-crossterm.md`.
- **CLI**: `ace paths` is `key\tvalue`, prints regardless of on-disk existence. Help
  text lives in clap doc comments; keep `--help` aligned with behavior.
- **Storage**: see `docs/spec/skills/sync.md` § Storage and `docs/spec/migrations.md`.
  Git via
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

See [docs/guides/release.md](docs/guides/release.md). It is the only place release
steps live — do not duplicate them here or in any other doc.

**Distribution.** Primary channel is Homebrew via the
[`ace-rs/homebrew-tap`](https://github.com/ace-rs/homebrew-tap) tap: end users
install with `brew install ace-rs/tap/ace`. The formula source lives at
`homebrew-tap/Formula/ace.rb` in this repo as a git subtree; `release.sh` patches
it and pushes the subtree to the tap repo. A sha mismatch between the published
binary and the formula sha means the release is broken — verify after publishing.

## Roadmap & backlog

**Outline is the only tracker.** All tasks, epics, phasing, and planning artifacts go
there — never into local files, never into Linear. File new work as a checklist item on
the matching epic doc (or the `Roadmap` doc for ordering); nothing gets a local
`TODO.md`, backlog note, or plan file.

Lives in the **ACE** collection on Outline (self-hosted, via the `outline` MCP):
<https://outline.prodigy9.co/collection/ace-hbmmUqagR9> — one doc per epic (A–L),
each with its own checklist, plus a `Roadmap` doc for suggested ordering. The collection
home page indexes every doc. There is no local roadmap/backlog file — the local
consolidation and catalog notes were deleted once Outline took over (2026-07-22); recover
them from git history if ever needed.

Legacy: Linear project ACE (team PRODIGY9, key PROD9) — scope queries to
`project:"ACE"`. Superseded by Outline; issue numbers survive as references only.
