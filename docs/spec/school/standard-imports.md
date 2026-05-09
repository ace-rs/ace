# Standard School Imports

Every school created by `ace school init` is seeded with one
`[[imports]]` entry pointing at the **standard school** — the canonical
source of skills that every ACE school is expected to inherit.

## The standard school

- Source: `ace-rs/school` (GitHub `owner/repo` shorthand).
- Imported as: `skill = "*"` (every skill in the source).
- Resolved on every `ace school pull`.

The standard school is what provides `ace-school` (the workflow skill that
documents school management, `ace diff`, PR creation, etc.) and any other
base-level skills the ACE project considers universal.

## Why it's an import, not a bundled template

Earlier versions of ACE shipped `tpl_ace_school_skill.md` and scaffolded
`skills/ace-school/SKILL.md` directly into new schools from a hard-coded
template. That coupled skill content to ACE releases and forced a code
change every time the workflow guidance evolved.

Treating it as an import instead means:

- The skill lives, versions, and evolves in `ace-rs/school` like any other
  skill.
- Updates ship via `ace school pull` rather than `cargo install ace`.
- The same mechanism scales to additional base skills without growing
  ACE's template directory.

## What `ace school init` does

1. Writes `school.toml` with the seeded `[[imports]]` entry.
2. Runs `PullImports`, which clones `ace-rs/school` into the source cache
   and copies all matching skills into the new school's `skills/`.
3. Leaves the user with a populated school ready to commit and push.

The skill files copied in are real files in the new school's repo (not
symlinks) — they belong to the new school and can be edited locally. The
`[[imports]]` line stays as provenance and lets `ace school pull` refresh
the skills if/when `ace-rs/school` changes.

## Opting out

A user wanting a fully standalone school can remove the
`source = "ace-rs/school"` entry from `school.toml` and delete any pulled
skills they don't want. ACE does not re-seed the import on subsequent
`ace school init --force` runs if the entry is absent — `ensure_standard_import`
only adds when the entry is missing, but `--force` only rewrites `name` on
an existing `school.toml` (it does not blow away the imports list).

## Network behavior

`ace school init` performs network I/O via the `PullImports` step. This
is intentional and matches the earlier behavior (which scaffolded files
locally but expected `ace import` calls for any non-trivial school
afterward). If the user is offline, init fails after `school.toml` is
written; re-running with network restores correctness.

## Related

- `docs/spec/school/school-commands.md` — `ace school init` step list.
- `docs/decisions/2026-04-22-action-layout.md` — action role split.
