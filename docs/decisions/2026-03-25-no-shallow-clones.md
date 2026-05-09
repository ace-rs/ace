# Decision: No Shallow Clones for School Cache (2026-03-25)

ACE uses full git clones for school repositories. No `--depth` flag on clone or fetch.

## Context

The install action used `git clone --depth 1` and the update action used `git fetch --depth 1`.
This broke fast-forward merges: `--depth 1` fetch disconnects the fetched commit from local
history, causing `git merge --ff-only` to fail with "refusing to merge unrelated histories."

The update action caught this as a "diverged" state and warned the user — meaning school updates
silently never applied.

## Why Full Clones

- School repos are small (markdown, TOML, small configs). Shallow saves negligible time/space.
- Full history enables correct `git merge --ff-only` after fetch.
- Removes the need for `--depth` management and shallow-to-unshallow transitions.
- `git diff` for skill change detection works reliably with connected history.
- Simplifies git operations to standard clone/fetch/merge — no special flags.

## Changes

- `git::clone_shallow` → `git::clone_repo` (drop `--depth 1 --single-branch`)
- `Git::fetch_shallow` → `Git::fetch` (drop `--depth 1`)
- `--no-tags` retained on both — school repos have no useful tags.
