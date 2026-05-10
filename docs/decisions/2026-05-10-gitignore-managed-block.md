# Gitignore management lives in Prepare; Pull is pure fetch
- **Date:** 2026-05-10
- **PR:** manual
- **Status:** accepted

## Decision

`UpdateGitignore` runs as a step inside `Prepare` . `ace pull` no longer re-links or
refreshes anything — it fetches the school clone and stops. School and project repos share
one `UpdateGitignore` codepath with identical block content; no scope specialization.

## Rationale

**Why `ace pull` no longer re-links.** The original design assumed a successful pull
should land skills in the repo immediately. From the user's perspective this doesn't
matter: getting into a coding session means running `ace` , which runs Prepare, which runs
Link — the skills land then anyway. The only case the change breaks is `ace pull` issued
against a long-running session expecting the skill list to refresh live. The fix is one
command: `ace link` , available standalone since v0.7.0. Before v0.7.0 there was no
recovery path; now there is, so the old auto-link is no longer pulling its weight.
Dropping it also leaves `Pull` as a single straightforward action — fetch and stop — and
removes the awkward Pull→Link coupling that was the only reason the gitignore-refresh hook
was hard to place in the first place.

**Why one codepath for school and project, not specialized.** The school context has
basically no ignore lines of its own — and ACE is the tool used to edit the school itself,
which means the school repo IS a project context in practice. Specializing per context
would save a handful of gitignore lines while forking the codepath, with no real
divergence between what the two contexts need.
