# School Commands

The `ace school` subcommand manages school repositories. It operates in two contexts
depending on where it is invoked:

- **School repo context** — `school.toml` exists at cwd root. Commands operate on the
  school directly.
- **App repo context** — no `school.toml` at cwd root, but `ace.toml` links to a school.
  Commands operate against the linked school's local clone.

Detection: if `school.toml` exists in the current working directory root, treat as school
repo context. Otherwise, resolve the linked school from `ace.toml`.

## `ace school init`

Initialize a new school repository. Must be run inside a git repo.

Steps:

1. Check cwd is a git repo.
2. Ask for school display name (or accept via `--name` arg).
3. Write `school.toml` with the standard import seeded:
   ```toml
   name = "<name>"

   [[imports]]
   skill = "*"
   source = "ace-rs/school"
   ```
   The `ace-rs/school` import is the canonical source of `ace-school` and
   any other base skills. See `docs/spec/school/standard-imports.md`. Users
   may remove the entry for a fully standalone school.
4. Create `CLAUDE.md` and `README.md` if missing.
5. Create `.gitignore` if missing.
6. Run `PullImports` to fetch the standard skills into `skills/`.
7. Done. User commits and pushes to their school repo.

Prerequisites: create and clone a git repo first (e.g. `gh repo create org/school --private`).

## Update and Edit Safety

The school clone is a live working copy. Users may have uncommitted edits (skills modified
through symlinks). The **Update** action must check for dirty state before pulling:

1. `git status --porcelain` — if dirty, warn and abort. Tell user to propose changes when
   ready.
2. `git fetch origin`
3. Fast-forward to `origin/main` (only when the cache is confirmed clean).

The dirty guard in step 1 ensures user edits are never silently discarded.

## Skill Modification Workflow

When ACE execs into the backend (lifecycle step 13), it injects a session prompt that:

1. Tells the AI that skills are loaded from the linked school and are editable.
2. Instructs it to propose changes back to the school repo when skills are modified.

The AI backend handles the full PR workflow: `ace diff` to review, branch in the school
cache, commit, push, create PR via GitHub MCP. No dedicated `ace` command needed — the AI
has all the tools (git + GitHub MCP).

The `ace-school` skill (provided by the `ace-rs/school` standard import,
seeded by `ace school init`) provides detailed instructions for this
workflow.

## `ace import <source> [--skill <name>] [--all]`

Import a skill from an external repository into the school. Top-level command (not under
`ace school`) for convenience.

- **source** — GitHub `owner/repo` shorthand or full URL (same convention as school specifiers).
- **--skill** — Specific skill name or glob pattern (e.g. `"frontend-*"`).
- **--all** — Import all skills from the source. Shorthand for `--skill "*"`.
- **--include-experimental** — With `--all`: also expand into `skills/.experimental/`.
  Fails if used without `--all`.
- **--include-system** — With `--all`: also expand into `skills/.system/`. Fails if used
  without `--all`.

### Parity with skills.sh

The `skills` CLI (https://skills.sh, `npx skills`) supports `--skill '*'` and `--all` for
bulk import, but only as a point-in-time snapshot — `skills update` only refreshes what's in
the lock file. New skills added to the source require another `add`.

ACE's wildcard imports go further: glob patterns in `[[imports]]` re-discover matching skills
on every `ace school update`. New skills added to the source are picked up automatically.

The `skills` CLI only supports literal `*` (all-or-nothing). ACE supports `*` anywhere in the
pattern (`frontend-*`, `*-coding`, `*-design-*`). The `skills` CLI uses exact name matching
for `--skill` values — no glob, no `?`, no character classes. ACE matches this constraint
(no `?` or character classes) but adds prefix/suffix/infix `*` matching.

### Flow

1. Resolve school context: if `school.toml` in cwd → school dir. Otherwise resolve linked
   school from `ace.toml` → school clone path.
2. Clone source repo to temp dir (`git clone --depth 1`).
3. Discover `SKILL.md` files under `skills/` (priority: `skills/.curated/` > `skills/` >
   `skills/.experimental/` > `skills/.system/`, first hit per name wins). Each skill is
   tagged with its tier — `Curated` (top-level or `.curated/`), `Experimental`, or `System`.
4. Select skill:
   - `--skill` given → find by name.
   - Single skill in repo → auto-import.
   - Multiple skills → interactive `inquire::Select` prompt.
5. Copy skill folder into `{school_root}/skills/{skill_name}/`.
6. Append `[[imports]]` entry to `school.toml` (upsert — replace if skill name already exists).
7. Print confirmation to stderr.

### Important

- Skills are copied as real files — the school owns and commits them.
- Re-importing the same skill overwrites files and updates (not duplicates) the `[[imports]]`
  entry.
- When multiple skills are found and no `--skill` or `--all` is given, prompts for selection.
- Glob patterns (`--skill "frontend-*"` or `--all`) record an `[[imports]]` entry and print
  a hint to run `ace school update`. No skills are copied immediately — resolution happens
  during update.
- **Tier gating**: explicit `--skill <name>` resolves across all tiers (Curated, Experimental,
  System). Glob matching and `--all` default to Curated only. Use `--include-experimental`
  and/or `--include-system` to widen the match — both require `--all`.

### Parent school pattern

To inherit all skills from a company-wide school:

```sh
ace import company/school --all
ace school update
```

This adds `skill = "*"` to `[[imports]]` and fetches all skills on update. New skills added
to the parent are picked up automatically on subsequent updates.

## `ace school update`

Re-fetch all imported skills from their sources.

### Flow

1. Read `[[imports]]` from `school.toml`.
2. If empty, print "no imports to update" and return.
3. Group imports by source (avoid cloning same repo twice).
4. For each source group: clone to temp dir, discover skills.
   - **Exact imports**: copy the named skill over existing. Resolves across all tiers.
   - **Wildcard imports**: filter discovered skills to the tiers allowed by the
     `[[imports]]` entry (`Curated` always; `Experimental` if `include_experimental = true`;
     `System` if `include_system = true`), then match against the glob pattern.
5. Report which skills were updated to stderr.

### Important

- Exact imports update only the named skill. If not found in the source, warns and skips.
- Wildcard imports re-discover on every update — new skills matching the pattern are picked up
  automatically. Existing skills are overwritten with the latest from the source, consistent
  with ACE's always-latest versioning philosophy (see `docs/spec/index.md`).

## `ace diff`

Show uncommitted changes in the school clone, including untracked files.

- Runs `git add -N .` (intent-to-add) before diffing so new files appear in the output.
- Prints `# school-clone\t<path>` as the first line (metadata, tab-separated).
- Resolves school specifier from `ace.toml`.
- Errors if no school configured or school is embedded (no clone directory).
- Passes raw diff output through to stdout (human-readable, not tab-separated).
- Prints metadata line even if the cache is clean (diff output may be empty).
- Output is a valid unified diff (patch-compatible).
