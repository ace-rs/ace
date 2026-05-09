# Decision: Resume Graceful Fallback (2026-04-09)

Status: **decided** — hint before exec.

## Problem

When `resume = true` (the default), ACE passes `--continue` to Claude, which fails with
"No conversation found to continue" on a fresh clone or when the user has no prior session.
ACE uses `exec()` to replace itself with the backend process, so it cannot catch the failure
and retry.

## Findings

### Claude `--continue` behavior

| Scenario | Interactive | `--print` |
|---|---|---|
| `--continue` (has session) | Resumes ✓ | Resumes ✓ |
| `--continue` (no session) | **Fails hard**: "No conversation found to continue" | Silently starts new session |
| `--continue --system-prompt "..."` (no session) | **Fails hard**: same error | Starts new session with prompt |

Key insight: `--continue` is checked and rejected before `--system-prompt` is considered in
interactive mode. Passing both flags does not help.

### Claude exit codes

| Scenario | Exit code |
|---|---|
| `--continue` with no session | 1 |
| Normal exit (`/exit`) | 0 |
| Ctrl+C | 0 |

### Codex `resume --last`

**No issue.** When no prior session exists, Codex shows an empty picker. Pressing ESC creates
a new session. Resume-by-default is safe.

## Decision

Print a hint before exec when `resume = true`:

```
Resuming previous session. If this fails, run: ace new
```

No automatic retry. The user sees the hint, and if `--continue` fails, they know the recovery
path. Simple, transparent, no process model changes.

## Options Rejected

### Shell wrapper exec

`sh -c 'claude --continue ... || claude --system-prompt ...'` — ACE execs into a shell
one-liner that retries on failure.

Exit codes make `||` safe for the "no session" case (exit 1 vs exit 0), but the wrapper
**silently falls back on all errors**, not just "no session". Auth failures, crashes, and
other real errors would be masked — the user lands in a fresh session without realizing their
resumed session failed for a different reason. Silent fallback is strictly worse than a
visible error.

### Marker file

ACE-owned breadcrumb written after first launch, checked before passing `--continue`.
Fails when user deletes sessions out-of-band (marker says "session exists" but it doesn't).
The failure is visible and recoverable (`ace new`), but adds state ACE must manage and can
still be wrong.

### Spawn + wait + retry

ACE stays alive as parent process. Changes the process model — signal forwarding, TTY
handling, cleanup become ACE's responsibility. Too heavy for this problem.

### Check backend session storage

Couples ACE to Claude's internal storage format. Rejected.

### Combined `--continue --system-prompt`

Claude rejects `--continue` before considering `--system-prompt` in interactive mode.
Does not work.
