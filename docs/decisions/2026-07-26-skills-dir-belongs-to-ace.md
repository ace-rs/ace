# The skills directory belongs to ACE

**2026-07-26** — supersedes the leave-alone half of the ACE-managed predicate in
[`spec/skills/sync.md`](../spec/skills/sync.md).

## Context

Switching a project off a school whose root sat outside the ACE data root — an embedded
`school = "."` or a path specifier — left every per-skill symlink pointing into that old
root. The managed predicate only recognized the *current* school root and the data root,
so ACE read its own links as user content: a warning on every command, and the new
school's skill of that name never linked. The user's `ace.toml` edit silently did nothing.

Remote-to-remote switches never hit this, because all clones live under the data root and
that branch of the predicate catches them.

## The question

An outside-root symlink has two possible authors: ACE, from a school root nothing records
anymore, or the user, hand-placed. Telling them apart requires knowing where the previous
root was, and nothing stores that.

## Ruling

**The user is not one of the authors.** A project that has adopted ACE receives skills
through the school or through `ace.toml` imports; hand-maintaining a symlink inside a
directory ACE reconciles is not a supported workflow. So a live symlink outside every
managed root is ACE's own leftover, and the second author was a persona, not a user.

That removes the need to identify the link — but not the need to decide what replaces it.
**ACE does not guess: the run fails, and `ace link --force` is the user's answer.** Every
command that links shares one action and so stops the same way.

Two entries are deliberately not covered:

- **A dangling symlink** points at nothing, so nobody maintains it on purpose. It is
  managed, and repaired in silence.
- **A real file or directory** is repo content, plausibly checked in, and was never a link
  of ours. It keeps skip-and-warn.

## Rejected: record the previous school root

`index.toml` could store each project's last-resolved school root, making a mismatch the
detection signal. Rejected because it grows ACE's one internal metadata file with
per-project state to answer a question the user can answer in one command — and because
the automatic path would then be deciding, on the user's behalf, to destroy a link it
inferred the provenance of.

## Consequences

- Breaking: a project with such a leftover now fails until forced. That is the point —
  the previous behavior was a silent no-op wearing a warning.
- `ace link --force` is new public CLI surface; additive per
  [`CLAUDE.md` § Backcompat](../../CLAUDE.md).
- `PrepareError::BlockedLinks` exits `Unavailable`: the tree is intact and waiting on a
  decision, not broken.
- `ace pull` updates the clone without linking, so it does not participate. Whether it
  should is open and unfiled.
