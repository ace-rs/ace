# Decision: Move `index.toml` to Data Dir (2026-04-22)

Status: **decided** — moved to `~/.local/share/ace/`, legacy read-migrated once.

Baseline: ACE v0.4.1.

## Problem

`index.toml` — the manifest of installed schools (specifier → repo/path) — lived at
`~/.cache/ace/index.toml`. Schools themselves moved to `~/.local/share/ace/{owner}/{repo}/`
in PROD9-76 because schools are user data, not cache: a dirty cache can carry in-progress
work (`UpdateOutcome::Dirty` / `AheadOfOrigin`). Losing the index to OS cache hygiene
leaves the clones in place but silently forgets them — `ace setup` (no-arg) stops listing
anything, and `Clone::update_index` re-creates a fresh index from whichever school was
most recently touched, losing the rest of the history.

The index belongs alongside the data it indexes.

## Decision

- Primary path moves to `~/.local/share/ace/index.toml` (`ace_data_dir()?.join("index.toml")`).
- Legacy path `~/.cache/ace/index.toml` is read once via `index_toml::load_or_migrate`:
  if the new path doesn't exist and the legacy path does, load legacy, write to new,
  return the data. New path wins if both exist.
- The legacy file is **left on disk** — not auto-deleted. `warn_stray_cache_dirs` surfaces
  it on subsequent startups so the user sees it and removes it manually, consistent with
  the PROD9-76 detect-and-hint policy. `detect_stray_cache_dirs` was extended to flag
  `index.toml` (now legacy) and to skip non-directory files other than `index.toml` so
  the self-update `latest-version` cache file is not incorrectly flagged.

## Removal path

Keep `legacy_index_path()` and the migration call in `load_or_migrate` until enough
versions have elapsed that users who skipped several releases are unlikely to still
have only the legacy file. Candidate removal: **v0.6.0 or later** (at least two minor
versions past v0.4.x). At that point:

1. Delete `legacy_index_path()` and collapse `load_or_migrate` into plain `load`.
2. Keep `detect_stray_cache_dirs` flagging `index.toml` — the hint is cheap and helps
   stragglers on very old installs.
3. Note removal in the release CHANGELOG under a "Breaking (unlikely)" heading.

## Alternatives considered

- **Drop `index.toml` entirely**, derive the list by scanning `~/.local/share/ace/*/*/.git`.
  Works for `owner/repo` specifiers but loses the `owner/repo:subpath` case — disk gives
  no way to recover which subpath of `prod9/mono` was the school. Would need a marker
  file inside each clone. Deferred; may revisit if maintaining the index becomes costly.
- **Auto-delete the legacy file.** Rejected per the Backcompat Policy: silent state
  mutation on startup is a foot-gun. Detect-and-hint leaves the user in control.
