# Unified `.gitignore` managed block
- **Date:** 2026-05-10
- **PR:** manual
- **Status:** accepted

## Decision

One `UpdateGitignore` action serves both project and school contexts.
Static OS/editor cruft lives **inside** the ACE-managed marker block.
`.env*` and language-specific patterns are not seeded.

## Rationale

**Why not keep the two surfaces (template + dynamic block).** The
template was a write-once seed; the dynamic block was always-managed.
Two mechanisms for one concern means a future change touches both or
silently drifts. The template also froze schools at first-init forever —
a later ACE upgrade that adjusted the pattern set could never reach
existing schools.

**Why static cruft inside the marker block, not at the top of the file.**
The marker block is the only region ACE re-syncs. Patterns outside it
become permanent on first write — a later version of ACE that wants to
add or remove a pattern has no path. Putting `.DS_Store` etc. inside the
block costs nothing on first install and gives every future ACE a path
to update them.

**Why no `.env` (against the convention every gitignore template
follows).** Ignoring `.env` is harmful for projects that intentionally
commit env samples, fixtures, or templates. The cost asymmetry favours
not seeding it: a user who wants `.env` ignored adds one line; a user
who finds an ACE-added ignore for a file they meant to commit has to
hunt down the source and either edit the marker block (which says "do
not edit") or work around it. Tools should not pre-judge which files in
the user's repo are secret.

**Why no language patterns (against keeping the existing Python list).**
ACE doesn't know the project's language. The previous template hardcoded
Python — arbitrary, since nothing about ACE is Python-specific. Picking
any one language is wrong; picking all of them bloats every project.
The principled rule is "no language-specific patterns ever, users own
that surface."

**Why school becomes always-managed (vs. the previous write-if-absent).**
Same argument as the marker block above: write-once means version bumps
can't propagate. Same semantics in both contexts also means one rule
for users to learn.

## Deferred

- **Auto-refresh on version bump.** Today the block only updates on
  `ace setup` and `ace school init`. If ACE adds a new backend or
  folder, existing projects stay stale until the next setup. Surfaced
  during this work; not solved here. Tied to the smell below.
- **`ace pull` re-links after fetch** (`cmd/pull.rs`). Pull's "fetch
  upstream" verb does double duty by re-linking, which is what made
  placing the gitignore-update hook awkward. Separate concern.
