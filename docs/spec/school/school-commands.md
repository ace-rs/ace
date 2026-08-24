# School Commands

The `ace school` subcommand manages school repositories. This file is authoring-side:
every `<school>` below means the **authored school** (see the glossary in
[overview.md](overview.md)) — except § `ace diff` and § Skill Modification Workflow,
which are consumer-side and mean the **linked school**.

Authoring commands (`ace school pull` / `skills` / `validate`, and `ace import`)
resolve the school they operate on with one rule, cwd-first:

1. `cwd/school.toml` exists → the authored school is the cwd. Primary mode.
2. Otherwise → fall back to the linked school, and **announce it** with a warning
   naming the school being touched ("no school.toml in current directory — using the
   linked school"). The fallback is never silent.
3. Neither a cwd `school.toml` nor a resolvable specifier → hard error with a hint
   naming both `ace school init` and `ace setup`.

`ace school init` is the exception: it *creates* the authored school, so it only
requires cwd to be a git repo and never resolves.

| Command               | Primary (cwd/school.toml)  | Fallback (specifier only) | Neither    |
| --------------------- | -------------------------- | ------------------------- | ---------- |
| `ace import`          | edits cwd school           | warn, edit linked school  | hard error |
| `ace school pull`     | pulls cwd school's imports | warn, pull linked school  | hard error |
| `ace school skills`   | lists cwd school's skills  | warn, list linked school  | hard error |
| `ace school validate` | validates cwd school.toml  | warn, validate linked     | hard error |
| `ace school init`     | creates in cwd (git repo required); no resolution involved |            |

No other command crosses the line: consumer-side commands (bare `ace`, `setup`, `pull`,
`link`, `diff`, `skills`, `explain`, `mcp`) touch only the linked school, and none of
them ever falls back to the authored school. `Ace::require_linked_school()` resolves the
linked school only; authoring commands reach it exclusively through the announced
fallback above.

This is school-authoring mode (see [overview.md](overview.md)). The complementary mode —
consuming a school from a project — is reached through bare `ace` and `ace setup` /
`ace pull` (see [setup.md](../setup.md)).

## `ace school init`

Initialize a new school repository. Must be run inside a git repo.

Steps:

1. Check cwd is a git repo.
2. Ask for school display name (or accept via `--name` arg).
3. Write `school.toml` with the standard import seeded:
   ```toml
   name = "<name>"

   [[imports]]
   source = "ace-rs/school"
   skills = ["*"]
   ```
   The `ace-rs/school` import is the canonical source of `ace-school` and any other base
   skills. See `docs/spec/school/standard-imports.md`. Users may remove the entry for a
   fully standalone school.
4. If `ace.toml` does not already exist in cwd, create one containing `school = "."` so
   the school can dogfood itself — the authored and linked school become the same
   directory, the only case where the two coincide (see the glossary in
   [overview.md](overview.md)). Existing `ace.toml` is preserved.
5. Create `CLAUDE.md` and `README.md` if missing.
6. Create `.gitignore` if missing.
7. Run `PullImports` to fetch the standard skills into `skills/`.
8. Done. User commits and pushes to their school repo.

Prerequisites: create and clone a git repo first (e.g.
`gh repo create org/school --private`).

## Update and Edit Safety

The linked school clone is a live working copy. Users may have uncommitted edits (skills
modified through symlinks). The **Update** action must check for dirty state before
pulling:

1. `git status --porcelain` — if dirty, warn and abort. Tell user to propose changes when
   ready.
2. `git fetch origin`
3. Fast-forward to `origin/main` (only when the cache is confirmed clean).

The dirty guard in step 1 ensures user edits are never silently discarded.

## Skill Modification Workflow

When ACE execs into the backend (the final lifecycle step in
[index.md](../index.md)), it injects a session prompt that:

1. Tells the AI that skills are loaded from the linked school and are editable.
2. Instructs it to propose changes back to the linked school's repo when skills are
   modified.

The AI backend handles the full PR workflow: `ace diff` to review, branch in the linked
school clone, commit, push, create PR via GitHub MCP. No dedicated `ace` command needed — the AI
has all the tools (git + GitHub MCP).

The `ace-school` skill (provided by the `ace-rs/school` standard import, seeded by
`ace school init`) provides detailed instructions for this workflow.

## `ace import <source> [--skill <name>] [--all]`

Import a skill from an external repository into the **authored school**. A top-level
command for convenience only — its resolution is authoring-side (cwd-first rule above),
not consumer-side.

- **source** — GitHub `owner/repo` shorthand or full URL (same convention as school
  specifiers).
- **--skill** — Specific skill name or glob pattern (e.g. `"frontend-*"`).
- **--all** — Import all skills from the source. Shorthand for `--skill "*"`.
- **--include-experimental** — With `--all`: also expand into `skills/.experimental/`.
  Fails if used without `--all`.
- **--include-system** — With `--all`: also expand into `skills/.system/`. Fails if used
  without `--all`.
- **--include-internal** *(intended; not yet on the CLI)* — With `--all`: admit skills with
  `internal: true` via glob matches. Fails if used without `--all`. `include_internal` is
  wired end-to-end in config/resolve and settable as an `[[imports]]` field; only the flag
  is missing.

### Parity with skills.sh

The `skills` CLI (https://skills.sh, `npx skills`) supports `--skill '*'` and `--all`
for bulk import, but only as a point-in-time snapshot — `skills update` only refreshes
what's in the lock file. New skills added to the source require another `add`.

ACE's wildcard imports go further: glob patterns in `[[imports]]` re-discover matching
skills on every `ace school pull`. New skills added to the source are picked up
automatically.

The `skills` CLI only supports literal `*` (all-or-nothing) and exact names for
`--skill` values — no prefix/suffix patterns, no `?`, no character classes. ACE
supports the match-handle grammar in
[skills/selection.md → Match handle](../skills/selection.md#match-handle): bare names
match exact-or-leaf, paths anchored at `/`, `*` anywhere in the pattern, `**` accepted
but not special, no `?` or character classes.

### Flow

1. Resolve the authored school root — cwd-first with announced fallback, per the rule at
   the top of this file. Never via `Ace::require_linked_school` directly.
2. Clone source repo into the import cache (`~/.cache/ace/imports/`) — a full clone, no
   `--depth`; see [no shallow clones](../../decisions/2026-03-25-no-shallow-clones.md).
3. Discover skills via the 2-stage cascade in
   [skills/model.md → Discovery Cascade](../skills/model.md#discovery-cascade).
   `.curated/`, `.experimental/`, `.system/` are community conventions skills.sh
   recognizes — not ACE-owned categories.
4. Select skill:
   - `--skill` given → find by name.
   - Single skill in repo → auto-import.
   - Multiple skills → interactive `inquire::Select` prompt.
5. Copy skill folder into `{authored_school}/skills/{identity_path}/`. For top-level skills
   (most common) the identity is the leaf name; nested skills preserve their source path
   (e.g. `typescript/coding`).
6. Append `[[imports]]` entry to `school.toml` (upsert — replace if skill name already
   exists).
7. Print confirmation to stderr.

### Important

- Skills are copied as real files — the school owns and commits them.
- Re-importing the same skill overwrites files and updates (not duplicates) the
  `[[imports]]` entry.
- When multiple skills are found and no `--skill` or `--all` is given, prompts for
  selection.
- Glob patterns (`--skill "frontend-*"` or `--all`) record an `[[imports]]` entry and
  print a hint to run `ace school update`. No skills are copied immediately — resolution
  happens during update.
- **Tier gating**: explicit `--skill <name>` resolves across all tiers (Curated,
  Experimental, System) and bypasses the `internal: true` filter (mirrors skills.sh).
  Glob matching and `--all` default to Curated only and exclude `internal: true` skills.
  Use `--include-experimental`, `--include-system`, and/or `--include-internal` to widen
  the match — all require `--all`.

### Parent school pattern

To inherit all skills from a company-wide school:

```sh
ace import company/school --all
ace school update
```

This adds `skills = ["*"]` to `[[imports]]` and fetches all skills on update. New
skills added to the parent are picked up automatically on subsequent updates.

## `ace school pull` (alias: `ace school update`)

Re-fetch all imported skills from their sources. `update` is a visible alias retained for
muscle-memory; `pull` is the canonical verb.

### Flow

1. Resolve the authored school root (cwd-first rule above); read `[[imports]]` from its
   `school.toml`.
2. If empty, print "no imports to pull" and return.
3. Group imports by source (avoid cloning same repo twice).
4. For each source group: clone to temp dir, discover skills, resolve `[[imports]]` per
   [skills/selection.md → `[[imports]]` schema](../skills/selection.md#imports-schema).
   Tier expansion, internal-flag handling, and cross-source merge are documented there.
5. Report which skills were updated to stderr.

### Important

- Exact imports update only the named skill. If not found in the source, warns and skips.
- Wildcard imports re-discover on every pull — new skills matching the pattern are picked
  up automatically. Existing skills are overwritten with the latest from the source,
  consistent with ACE's always-latest versioning philosophy (see `docs/spec/index.md`).

## `ace school skills`

List the skills currently in the authored school's `skills/` directory (cwd-first rule
above). Read-only.

For each skill: name (from `SKILL.md` frontmatter when available, else identity-path
leaf), word count across all files in the skill folder, and description. The footer
prints the skill total, aggregate word count, and a token estimate (~1.33 tokens/word).

Porcelain output is `<identity-path><TAB>words`, one per line, no footer.

Skill discovery here walks the authored school's `skills/` recursively for `SKILL.md`
files. Each found skill is keyed by its identity path (the location relative to
`skills/`). Tier dirs (`.curated/` / `.experimental/` / `.system/`) are not honored at
the school boundary — those are upstream-source conventions; this listing reflects what
the authored school itself stores after import.

## `ace school validate` (alias: `ace school check`)

Typo-check `{{ ... }}` placeholders in `[backends.<name>].cmd[]` and
`[backends.<name>].env` values against the closed set
`{school_dir, project_dir, home, backend_dir}` (defined in
[backend.md → Custom Backends](../backend.md#custom-backends)).

### Flow

1. Resolve the authored school root (cwd-first rule above).
2. Load its `school.toml`.
3. For each `[backends.<name>]` table, parse every `cmd[i]` and every `env[key]` value
   as a template. Any placeholder name not in the closed set is reported as an issue.
4. Each issue is paired with a Levenshtein-≤2 did-you-mean suggestion when one of the
   allowed names is close.

### Output

One line per issue, written to the data stream:

```
backends[<name>].cmd[<index>]: unknown placeholder '<name>', did you mean '<suggestion>'?
backends[<name>].env[<key>]: unknown placeholder '<name>'
```

Suggestion is omitted when no close match exists.

### Exit code

- `0` — clean. A success message (`school.toml looks good`) is emitted.
- `3` (operational) — one or more issues reported. The error line `N validation issue(s)
  found` follows the issue list. See [exit codes](../exit-codes.md).

### Scope (v1)

Only `[backends.<name>]` placeholders. Other shapes (`[[imports]]`, `[[mcp]]`, etc.) are
not validated — see `docs/decisions/2026-05-09-school-validate-scope.md` for rationale.
`ace school validate` is not auto-run by `ace school pull` or `ace setup`; users invoke it
explicitly.

## `ace diff`

Consumer-side: `ace diff` always operates on the **linked school**, never the authored
one. Shows uncommitted changes in the linked school clone, including untracked files.

- Runs `git add -N .` (intent-to-add) before diffing so new files appear in the output.
- Prints `# school-clone\t<path>` as the first line (metadata, tab-separated).
- Resolves school specifier from `ace.toml`.
- Errors if no school configured or school is embedded (no clone directory).
- Passes raw diff output through to stdout (human-readable, not tab-separated).
- Prints metadata line even if the cache is clean (diff output may be empty).
- Output is a valid unified diff (patch-compatible).
