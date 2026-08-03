# Admission eviction is self-healing and non-overridable

- **Date:** 2026-06-04
- **PR:** manual
- **Status:** accepted

## Decision

When a newer ACE tightens admission and a previously-linked skill becomes inadmissible, the
skill is **evicted by construction**: it falls out of the `Vetted` set, is absent from the
emitted desired-links, and reconcile removes its now-orphaned symlink. This is the designed
**self-healing-on-upgrade** behavior, not a regression to soften. ACE provides **no
fail-open per-skill override**. The answer to "don't surprise me" is **visibility** — a
dry-run and a reconcile summary that distinguishes *admission-evicted* from *config-orphaned*
removes. The only escape hatches are: fix the identity, fix the predicate, or leave ACE's
protection envelope.

## Context

Re-running a newer ACE over an already-provisioned project re-discovers and re-validates the
school from scratch (per [the lifecycle spec](../spec/skills/lifecycle.md),
`validate` runs every process). A skill admissible under the old rules but rejected under the
new ones lands in the `Rejected` partition instead of `Skills<Validated>`. The question:
should the user be able to override that and keep the link?

## The dataflow correction

The eviction is **not** driven by the `Rejected` bucket. It is driven by the skill's
**absence from the desired-links set**:

```
validate(rediscovered) → (Validated, Rejected)
emit(resolve(Validated)) → desired          // rejected skills are simply not in here
reconcile(desired, current) → plan          // their live links are orphans → Remove
```

`reconcile` never consults `Rejected`; it removes the links by the orphan rule (a managed
entry with no matching desired row). Two consequences:

- **"Ignore the `Rejected` return" does not keep the links.** Those skills are already absent
  from the `Validated` set being emitted. Ignoring the bucket only drops the *warning* — the
  link is still deleted, now **silently**. The opposite of the intent.
- **The only lever that keeps a link is upstream**: re-inject the skill into `desired` — i.e.
  re-admit it into the `Vetted` set. That is the move this decision refuses.

## Why no override

1. **It is fail-open.** Re-admitting a rejected skill punches a per-skill hole in the
   whitelist / fail-closed boundary — the denylist-drift the project explicitly resists.
2. **It defeats the reason the design exists.** Per the admission policy: *"Tighten the rule,
   and the next `ace pull` re-scans… the stale symlink becomes an orphan and reconcile removes
   it. Self-healing on upgrade."* A keep-the-rejected knob *is* the anti-self-healing knob.
3. **No safe shape exists.** The threat (bidi / terminal-escape payload) lives in the SKILL.md
   *content* the backend reads, which ACE symlinks **verbatim** and refuses to sanitize.
   Re-admitting puts that raw payload in front of the backend. There is no "render it safe and
   keep it" — keeping it *is* exposing it.

## Escape hatches (none fail-open)

| Situation                                     | Hatch                                                                                          |
| --------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Rejection is a bad path character you control | **Fix the identity** — rename the dir. The path is the rejection and the path is yours.        |
| The *rule itself* is wrong                    | **Fix the predicate** — regenerate ACE's committed Unicode table. A uniform policy change, not a per-skill exception. |
| You need the skill now, on your own risk      | **Leave ACE's envelope** — run the backend tool directly against the school. Already framed by `model.md` § Caveat as "outside ACE's protection envelope by their own choice." |

The pressure-relief valve correctly lives *outside* ACE, not as an ACE fail-open flag.

## Visibility over override

The legitimate need under the override request — "don't silently nuke my links on upgrade" —
is met without a hole:

- **Dry-run / preview.** Surface the `Rejected` set and the pending removes *before* acting,
  so the user can rename / fix / step outside first.
- **Legible reconcile summary.** Distinguish *admission-evicted* removes (a skill rejected
  this run) from *config-orphaned* removes (a skill the user deselected). Silent or
  undifferentiated deletion is what reads as a bug; a named eviction with its reason does not.

## Rejected alternatives

| Approach                                               | Why not                                                                                       |
| ------------------------------------------------------ | --------------------------------------------------------------------------------------------- |
| Per-skill "trust anyway" flag                          | Fail-open; exposes the backend to the unsanitized payload.                                     |
| Suppress admission-evicted removes (keep orphan links) | Stops self-healing; the tightened rule never evicts the bad skill — the exact failure the discovery-gate was built to prevent. |
| Global `--accept-unvalidated`                          | Uniform fail-open; loud but still hands the backend the payload.                               |
| "Ignore the `Rejected` return"                         | A no-op for the stated goal — deletion is absence-driven, so this only loses the warning.       |

## References

- Depends on [the lifecycle spec](../spec/skills/lifecycle.md) (the `validate`
  partition + `Vetted` gate that make eviction by-construction).
- Applies [name admission policy](2026-05-30-skill-name-admission-policy.md) (self-healing
  rationale, discovery as gate of record) and
  [name = path](2026-06-01-skill-name-is-path.md) (admission keys on identity).
- The dry-run + legible-summary surfaces are spec work for the following step.
