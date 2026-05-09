# Setup Flow

This spec covers **project-repo** setup — bootstrapping a user's codebase to consume a school.
The other context — authoring a school itself — is covered in [school/overview.md](school/overview.md)
and uses `ace school init` instead. The two contexts are distinguished by which marker file
exists at the workdir root: `ace.toml` (project) vs `school.toml` (school). A workdir can be
both, in which case school-context detection takes precedence (see `Ace::require_school` in
`src/ace/mod.rs`).

`ace setup <owner/repo>` is a required first step before using ACE in a project. It must be run
explicitly — ACE does not auto-detect or auto-initialize.

`ace setup .` is a project-repo variant — `school = "."` declares the school is embedded in
the same tree (e.g. monorepo). It does NOT create a `school.toml`; an embedded school must
already exist somewhere addressable by the specifier. Bootstrapping a brand-new local school
is a separate, undesigned feature.

## Guards

Setup fails immediately if:

- **Not in a git repo** — error: `not in git repo, git init?`
- **`ace.toml` already exists** — error: `already set up, use 'ace' to run`

## Specifier Resolution

Before calling the Setup action, the CLI layer resolves which school to use:

- **`ace setup <owner/repo>`** — specifier is the argument.
- **`ace setup`** (no argument) — resolve from cache:
  - **One cached school** — use it automatically.
  - **Multiple cached schools** — TUI picker.
  - **No cached schools** — error: `no schools cached, ace setup <owner/repo>?`

This logic lives in the cmd/TUI layer. The Setup action always receives a resolved specifier.

## Setup Steps

1. Write `ace.toml` with `school = "<owner/repo>"`.
2. Call **Prepare** (see below).
3. **Skill-count check** — if the resolved school exposes more than 10
   skills and the project's `ace.toml` does not have `skills` set
   explicitly (any value, including `skills = []`), prompt inline (y/N)
   to run `ace learn` now. On `y`, invoke `LearnAction` directly — the
   inline prompt is the confirm; the action itself does no prompting.
   Same trigger fires from `ace school pull-imports` and `ace` startup.
   See [learn.md](learn.md).

Setup's only unique responsibilities are writing `ace.toml` and the
post-prepare learn hint. Everything else is delegated to Prepare, which is
shared with the normal `ace` run.

## Prepare

Prepare ensures the school is ready to use. It is called by both `ace setup` and normal `ace`
runs.

1. **Is school cloned?** (check `index.toml` for matching specifier)
   - **No** → **Clone**: `git clone --depth 1` into `~/.local/share/ace/<owner>/<repo>/` (XDG_DATA_HOME), write
     `index.toml` entry, parse `school.toml`, register MCP servers.
   - **Yes** → **Pull**: `git pull --ff-only` on the cached repo.
2. **Link**: sync school folders into `<project>/<backend_dir>/`. Two shapes:
   - `skills/` becomes a real directory with per-skill symlinks (one per Included skill from
     the resolution — see [skills-sync.md](skills-sync.md#skill-selection) and
     [configuration.md](configuration.md#skills-selection)).
   - `rules/`, `commands/`, `agents/` are whole-dir symlinks. An existing real directory at
     the link path is renamed to `previous-<folder>/` first (one-time adoption). Adoption
     does not apply to `skills/` — its reconciler handles a mix of managed and foreign
     entries directly.

   Folders absent from the school are skipped.

## Normal `ace` Run

When the user runs `ace` (no subcommand) in a project that already has `ace.toml`:

1. Load state from `ace.toml`.
2. Call **Prepare** (install-if-needed / update-if-cached, then link).
3. Build system prompt from school config.
4. Detect and exec backend (Claude Code / Codex).

## Actions Summary

All consumer-side actions live in `src/actions/project/` (see
`docs/decisions/2026-04-22-action-layout.md`).

| Action    | Responsibility                                          | When                     |
|-----------|---------------------------------------------------------|--------------------------|
| Setup     | Guard checks, write `ace.toml`, call Prepare            | `ace setup <spec>`       |
| Prepare   | Orchestrate Clone/Pull + Link                           | Setup and normal `ace`   |
| Clone     | `git clone`, index, register MCP                         | School not in cache      |
| Pull      | `git pull --ff-only` on cached repo                     | School already cached    |
| Link      | Symlink school folders from cache into project          | Always (after clone/pull) |
| Learn     | Study project, edit instructions file, narrow `skills`  | `ace learn` / setup hint |

## Error Cases

- **Not in git repo** — hard error.
- **Already set up** — hard error, use `ace` to run.
- **No network** — Clone/Pull fail with clear message.
- **Invalid school** — fail if not git-cloneable or `school.toml` missing/invalid.
- **MCP registration failure** — warn per server, continue. Backend handles auth on first use.
- **No cached schools (no-arg setup)** — error, suggest `ace setup <owner/repo>`.
