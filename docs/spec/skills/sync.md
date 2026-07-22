# Skill Sync

Consumer-side workflow — what happens when a project consuming a school runs `ace`,
`ace pull`, or `ace setup`. Covers fetching the school clone, syncing folder symlinks into
the project, and reconciling per-skill links.

The skill *model* (discovery, identity, frontmatter, sanitization) lives in
[model.md](model.md). *Selection* (match handles, `[[imports]]`, cross-source merge) lives
in [selection.md](selection.md). *Emit* (school storage, backend emit rule, loser-drop)
lives in [emit.md](emit.md).

## Linkable Folders

ACE links four folder types from the school into the project:

| Folder      | Purpose                                |
| ----------- | -------------------------------------- |
| `skills/`   | Skill definitions (SKILL.md per skill) |
| `rules/`    | Convention / rule files                |
| `commands/` | Slash commands for the backend         |
| `agents/`   | Agent configurations                   |

Linking strategy differs by folder:

- `skills/` — `<backend>/skills/` is a real directory containing per-skill symlinks (one
  per Included skill in the resolution; see [Skill selection](#skill-selection)).
- `rules/`, `commands/`, `agents/` — single whole-dir symlink to the school's folder.

Only folders that exist in the school are linked — absent folders are silently skipped.

### Backend support matrix

Not all backends natively support every folder. ACE links regardless and warns for
unsupported combos:

| Folder      | Claude | Codex | OpenCode |
| ----------- | :----: | :---: | :------: |
| `skills/`   |   ✓    |   ✓   |    ✓     |
| `rules/`    |   ✓    |   ✗   |    ✗     |
| `commands/` |   ✓    |   ✗   |    ✓     |
| `agents/`   |   ✓    |   ✗   |    ✓     |

Linking still happens for unsupported combos — the warning is informational only (linked
for future compatibility).

## Fetch and sync

On every run:

1. `git fetch` the school
2. Compare local HEAD SHA against remote HEAD SHA
3. If changed, `git pull` and reconcile school folders into the project
4. If unchanged, skip — cached state is current

Always sync. No user prompt, no opt-out. Consistency across the team is more important
than saving a few seconds.

The fetch step honors a cooldown so back-to-back invocations don't re-hammer the remote.
See [`actions/project/pull.rs`](../../../src/actions/project/pull.rs) for the guard.

## Skill selection

Per-repo skill selection runs through the three fields documented in
[configuration.md § Skills Selection](../configuration.md#skills-selection): `skills`
(whitelist, last-wins), `include_skills` (additive, union), and `exclude_skills`
(subtractive, union). Selection resolution stamps each skill Included or Excluded with a
provenance trace; only Included skills get linked into `<backend>/skills/`.

When all three fields are unset across all scopes, every validated skill is linked
(implicit-all base). This is the default for fresh setups.

`ace skills` lists the resolved set with provenance;
`ace skills include / exclude / reset` edit the union-merge fields; `ace explain <name>`
prints the per-step trace for a single skill. See
[configuration.md → CLI](../configuration.md#cli).

Match patterns in any of these fields obey the [match handle](selection.md#match-handle)
rules.

## Symlinks over copies

Sync into projects using symlinks, not file copies. Multiple projects sharing the same
school (e.g. frontend and backend repos in the same org) all point to the same local
clone. Avoids redundant data; ensures all projects see the same skill versions immediately
after a pull.

**Two link shapes:**

- **Per-skill symlinks for `skills/`.** `<backend>/skills/` is a real directory; each
  Included skill gets its own symlink inside, pointing at the discovered skill path in the
  school clone:

  ```text
  project/.claude/skills/                   (real directory)
  project/.claude/skills/rust-coding        → ~/.local/share/ace/{school}/skills/rust-coding/
  project/.claude/skills/coding             → ~/.local/share/ace/{school}/skills/typescript/coding/
  ```

  The link **name** is the backend-emit `skillName` (see
  [emit.md § Backend emit rule](emit.md#backend-emit-rule)). The link **target** is the
  skill's path inside the school clone — flat or nested per the school's storage layout
  (see [emit.md § School storage layout](emit.md#school-storage-layout)).

- **Whole-dir symlinks for the rest.** `rules/`, `commands/`, `agents/` are single
  symlinks to the school's corresponding directory:

  ```text
  project/.claude/rules/     → ~/.local/share/ace/{school}/rules/
  project/.claude/commands/  → ~/.local/share/ace/{school}/commands/
  project/.claude/agents/    → ~/.local/share/ace/{school}/agents/
  ```

To change a skill, edit through the symlink and propose the change back to the school.

## Reconciliation

Each `ace` / `ace pull` / `ace setup` run reconciles `<backend>/skills/` against the
resolved Included set:

- Add a symlink for any new skill.
- Re-point a managed symlink that targets a stale path (skill moved within the school).
- Remove managed symlinks for skills no longer in the resolved set.
- Skip and warn when an entry's name collides with a non-managed file or symlink.
- Drop the loser on backend-emit `skillName` collisions per
  [emit.md § Loser-drop on collision](emit.md#loser-drop-on-collision-flatten-branch-only),
  with a loud warning. Applies only on the flatten branch; nested-capable backends emit
  verbatim per [emit.md § Backend emit rule](emit.md#backend-emit-rule).

**ACE-managed predicate:** a symlink whose target path resolves textually inside either
the current school clone OR the ACE data root (`~/.local/share/ace/`, parent of all
school clones). No marker files. The data-root branch catches symlinks left over from a
previous `school = "..."` value pointing into a sibling clone, so switching schools via
`ace.toml` prunes those leftovers on the next link/setup. Anything else (real files, real
subdirs, symlinks pointing outside every managed root) is treated as user content and
left alone — except when its name collides with a desired skill, in which case the link
is skipped with a warning so the user can resolve the conflict.

### Removal visibility (admission-evicted vs config-orphaned)

A managed symlink is removed whenever its skill is absent from the resolved Included set — the
reconciler works by *absence*, never by consulting a rejected list. Two distinct causes put a
skill in that absence, and the summary names which:

- **admission-evicted** — the skill was rejected by name admission this run (a newer ACE
  tightened the Unicode table, or the skill's identity changed). Pruning its stale link is
  **self-healing on upgrade**, not a regression, and there is no fail-open override — see
  [admission eviction is non-overridable](../../decisions/2026-06-04-admission-eviction-non-overridable.md).
- **config-orphaned** — the skill is admissible but the user's `skills` / `exclude_skills` no
  longer select it.

Two surfaces keep the removal legible rather than surprising:

- **Dry-run / preview** *(intended; not implemented)* — surface the pending removes (and the
  rejected set) *before* acting,
  so the user can rename a bad path, regenerate the predicate, or step outside ACE first.
- **Reconcile summary** — report completed removes split by cause: admission-evicted rows carry
  their rejection reason, config-orphaned rows do not. Undifferentiated deletion reads as a
  bug; a named eviction with its reason does not.

### First-time adoption (rules / commands / agents only)

For `rules/`, `commands/`, and `agents/`, an existing real directory at the link path is
renamed to `previous-{name}/` before the symlink is created. This is a one-time bulk
migration so pre-ACE content is preserved; the session prompt may nudge the LLM to help
merge `previous-{name}/` back into the school.

The skills folder no longer triggers this adoption — its per-skill reconciler handles a
mix of managed and foreign entries directly. A `previous-skills/` directory only exists on
projects upgraded from a pre-2026-04-23 ACE that performed the bulk rename before the
per-skill layout shipped; the legacy directory is left in place for the user to
consolidate manually.

### Migrating from the legacy whole-dir symlink

ACE versions before 2026-04-23 created `<backend>/skills` as a single symlink to
`<school>/skills/`. The reconciler detects that legacy symlink, removes it, and rebuilds
`<backend>/skills/` as a real directory with per-skill symlinks inside. No user action
required.

## Storage

- **School clones**: `~/.local/share/ace/{owner/repo}/` (XDG_DATA_HOME). Schools are user
  data — `PullOutcome::Dirty` / `AheadOfOrigin` states can carry in-progress work that
  must survive OS cache hygiene.
- **Import source cache**: `~/.cache/ace/imports/{owner/repo}/` (XDG_CACHE_HOME).
  Read-only upstream snapshots used during `ace import` and `ace school pull`; safe to
  delete.
- **Index**: `~/.local/share/ace/index.toml` (XDG_DATA_HOME) — tracks downloaded schools.
  See [index.toml lives in the data dir](../../decisions/2026-04-22-index-toml-data-dir.md);
  the `~/.cache/ace/` path is legacy.
- **Cache key**: remote HEAD SHA. On SHA match: no-op. On SHA mismatch: pull + sync. First
  run: full clone + index entry.

### Import source cache (`git::ensure_source_cache`)

Both `ace import <source>` and `ace school pull` pull skills from upstream repositories.
Rather than re-cloning each source into a fresh `tempfile::tempdir()` on every invocation,
ACE maintains a persistent cache at `~/.cache/ace/imports/{owner/repo}/` and uses
`git::ensure_source_cache(source)`:

- **First call** — `git clone https://github.com/{owner/repo}.git` into the cache path.
  Returns the on-disk path.
- **Subsequent calls** — `git fetch origin` + `git merge --ff-only origin/<branch>` on the
  existing clone. Returns the same path.

The cache is ACE-managed — users should not edit it. Unlike the school clone (in
XDG_DATA_HOME), the import cache is safe to sweep; next invocation re-clones. Parent
callers resolve the cache root via `config::paths::ace_import_cache_dir()`.

### `index.toml`

```toml
[[school]]
specifier = "ace-rs/school"
repo      = "ace-rs/school"
path      = ""

[[school]]
specifier = "jedi/academy:school"
repo      = "jedi/academy"
path      = "school"
```

- `specifier` — full specifier as written in `ace.toml`
- `repo` — `owner/repo` portion (git clone target)
- `path` — subfolder within the repo containing `school.toml` (empty string if root)

`list_cached_schools` reads `index.toml`, not the filesystem.
