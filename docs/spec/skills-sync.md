# School Folder Sync

Covers lifecycle steps 6–7: fetching the school and syncing school folders into the project.

## Linkable Folders

ACE links four folder types from the school into the project:

| Folder     | Purpose                                    |
|------------|--------------------------------------------|
| `skills/`  | Skill definitions (SKILL.md per skill)     |
| `rules/`   | Convention/rule files                      |
| `commands/`| Slash commands for the backend             |
| `agents/`  | Agent configurations                       |

Linking strategy differs by folder:

- `skills/` — `<backend>/skills/` is a real directory containing per-skill symlinks
  (one per Included skill in the resolution; see [Skills Selection](#skill-selection)).
- `rules/`, `commands/`, `agents/` — single whole-dir symlink to the school's folder.

Only folders that exist in the school are linked — absent folders are silently skipped.

### Backend support matrix

Not all backends natively support every folder. ACE links regardless and warns for unsupported
combos:

| Folder     | Claude | Codex | OpenCode | Droid |
|------------|--------|-------|----------|-------|
| `skills/`  | ✓      | ✓     | ✓        | ✓     |
| `rules/`   | ✓      | ✗     | ✗        | ✗     |
| `commands/`| ✓      | ✗     | ✓        | ✗     |
| `agents/`  | ✓      | ✗     | ✓        | ✗     |

Linking still happens for unsupported combos — the warning is informational only (linked for
future compatibility).

## Import Merge Strategy

When `ace pull` / `ace school update` processes `[[imports]]` in
`school.toml`, skills from multiple sources may share the same name. The
merge rule is **last-wins in declaration order** — sources are processed
top-to-bottom as declared in `school.toml`, and a later source's skill
silently replaces an earlier one with the same name. No warning, no
tracking.

This is intentional: imported skills are upstream snapshots, not
locally-authored content. A school that imports `skill-creator` from
`anthropics/skills` gets that skill copied into its own `skills/` dir. If
the school is later used as a source by another import (or by itself via a
`*` glob), the same skill is re-discovered. Last-wins ensures every pull
converges to the latest version regardless of declaration order — the
final state is the same because all sources eventually carry the same
upstream content.

No provenance is tracked. If a school author wants to fork and diverge a
skill, they stop importing it and maintain it directly.

## Fetch and Sync

On every run:

1. `git fetch` the school
2. Compare local HEAD SHA against remote HEAD SHA
3. If changed, `git pull` and symlink school folders into the project
4. If unchanged, skip — cached state is current

Always sync. No user prompt, no opt-out. Consistency across the team is more important than
saving a few seconds.

## Skill Selection

Per-repo skill selection runs through the three fields documented in
[`configuration.md`](configuration.md#skills-selection): `skills` (whitelist, last-wins),
`include_skills` (additive, union), and `exclude_skills` (subtractive, union). The resolver
in `state::resolver` produces a per-skill `Resolution` with provenance traces; only skills
with `Decision::Included` get linked into `<backend>/skills/`.

When all three fields are unset across all scopes, every discovered skill is linked
(implicit-all base). This is the default for fresh setups.

`ace skills` lists the resolved set with provenance; `ace skills include/exclude/reset`
edit the union-merge fields; `ace explain <name>` prints the per-step trace for a
single skill. See [configuration.md → CLI](configuration.md#cli).

## Symlinks Over Copies

Sync into projects using symlinks, not file copies. Multiple projects sharing the same school
(e.g. frontend and backend repos in the same org) all point to the same local clone. This avoids
redundant data and ensures all projects see the same skill versions immediately after a pull.

**Two link shapes:**

- **Per-skill symlinks for `skills/`.** `<backend>/skills/` is a real directory; each Included
  skill gets its own symlink inside, pointing at the discovered skill path in the school clone:
  ```
  project/.claude/skills/                    (real directory)
  project/.claude/skills/rust-coding         → ~/.local/share/ace/{school}/skills/rust-coding/
  project/.claude/skills/issue-tracker       → ~/.local/share/ace/{school}/skills/issue-tracker/
  ```
- **Whole-dir symlinks for the rest.** `rules/`, `commands/`, `agents/` are single symlinks
  to the school's corresponding directory:
  ```
  project/.claude/rules/    → ~/.local/share/ace/{school}/rules/
  project/.claude/commands/ → ~/.local/share/ace/{school}/commands/
  project/.claude/agents/   → ~/.local/share/ace/{school}/agents/
  ```

To change a skill, edit through the symlink and propose changes back to the school.

### Reconciliation

Each `ace` / `ace pull` / `ace setup` run reconciles `<backend>/skills/` against the resolved
Included set:

- Add a symlink for any new skill.
- Re-point a managed symlink that targets a stale path (skill moved within the school).
- Remove managed symlinks for skills no longer in the resolved set.
- Skip + warn when an entry name collides with a non-managed file or symlink.

**ACE-managed predicate**: a symlink whose target path resolves textually inside the school
clone's `skills/` subtree. No marker files. Anything else (real files, real subdirs, symlinks
pointing outside the school) is treated as user content and left alone — except when its name
collides with a desired skill, in which case the link is skipped with a warning so the user
can resolve the conflict.

### First-time adoption (rules/commands/agents only)

For `rules/`, `commands/`, and `agents/`, an existing real directory at the link path is
renamed to `previous-{name}/` before the symlink is created. This is a one-time bulk
migration so pre-ACE content is preserved, and the session prompt may nudge the LLM to help
merge `previous-{name}/` back into the school.

The skills folder no longer triggers this adoption — its per-skill reconciler handles a
mix of managed and foreign entries directly. A `previous-skills/` directory only exists on
projects upgraded from a pre-2026-04-23 ACE that performed the bulk rename before the
per-skill layout shipped; the legacy directory is left in place for the user to consolidate
manually.

### Migrating from the legacy whole-dir symlink

ACE versions before 2026-04-23 created `<backend>/skills` as a single symlink to
`<school>/skills/`. The reconciler detects that legacy symlink, removes it, and rebuilds
`<backend>/skills/` as a real directory with per-skill symlinks inside. No user action required.

## Storage

- School clones: `~/.local/share/ace/{owner/repo}/` (XDG_DATA_HOME). Schools are
  user data — `PullOutcome::Dirty` / `AheadOfOrigin` states can carry in-progress
  work that must survive OS cache hygiene.
- Import source cache: `~/.cache/ace/imports/{owner/repo}/` (XDG_CACHE_HOME).
  Read-only upstream snapshots used during `ace import` and `ace school update`;
  safe to delete.
- Index: `~/.cache/ace/index.toml` — tracks downloaded schools
- Cache key: remote HEAD SHA
- On SHA match: no-op
- On SHA mismatch: pull + sync
- First run: full clone + index entry

### Import source cache (`git::ensure_source_cache`)

Both `ace import <source>` and `ace school update` pull skills from upstream
repositories. Rather than re-cloning each source into a fresh `tempfile::tempdir()`
on every invocation, ACE maintains a persistent cache at
`~/.cache/ace/imports/{owner/repo}/` and uses `git::ensure_source_cache(source)`:

- **First call:** `git clone https://github.com/{owner/repo}.git` into the cache
  path. Returns the on-disk path.
- **Subsequent calls:** `git fetch origin` + `git merge --ff-only origin/<branch>`
  on the existing clone. Returns the same path.

The cache is ACE-managed — users should not edit it. Unlike the school clone
(in XDG_DATA_HOME), the import cache is safe to sweep; next invocation re-clones.
Parent callers resolve the cache root via `config::paths::ace_import_cache_dir()`.

### index.toml

```toml
[[school]]
specifier = "ace-rs/school"
repo = "ace-rs/school"
path = ""

[[school]]
specifier = "jedi/academy:school"
repo = "jedi/academy"
path = "school"
```

- `specifier` — full specifier as written in `ace.toml`
- `repo` — `owner/repo` portion (git clone target)
- `path` — subfolder within the repo containing `school.toml` (empty string if root)

`list_cached_schools` reads index.toml, not the filesystem.
