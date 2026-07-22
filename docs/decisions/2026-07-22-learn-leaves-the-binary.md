# `ace learn` leaves the binary: LLM workflows belong to the school

- **Date:** 2026-07-22
- **PR:** manual
- **Status:** accepted

## Decision

ACE does not orchestrate the LLM. `ace learn` — which one-shot the backend with a baked-in
prompt, parsed the agent's freeform stdout, and rewrote `ace.toml`'s `skills` array — is
removed from the binary. Narrowing a project's skill set is a *school skill*: the agent
reads `ace skills`, decides with full project context, and writes the list.

Dropping a CLI verb is a breaking change, so the next release takes the minor bump
CLAUDE.md § Backcompat requires, plus a release note. `release.sh` owns the version, so no
user-facing string names one.

Removed with it: the auto-trigger that offered learn from `ace setup`, `ace school pull`
and `ace` startup; the `maybe_hint_relearn` soft nudge; `prompt_learn.md`; and
`docs/spec/learn.md`. `Io::prompt_confirm` went too — learn held its last two callers, so
ACE now has no yes/no prompt anywhere (sets are offered as checklists, per
[the selection-prompt ruling](2026-07-22-batch-selection-prompts.md)).

`ace learn` survives as a hidden tombstone that errors with a redirect. Removing the verb
outright would surface only clap's "unexpected argument", which tells a user with muscle
memory nothing.

## Rationale

The action was 431 lines whose center of gravity was a forgiving stdout parser — strip
backticks, drop prose lines, warn on unrecognized names, dedupe globs. That parser exists
only because a Rust program was trying to read an LLM's mind through a pipe. Every failure
mode it defended against is an artifact of the coupling, not of the problem.

Two things follow from ux.md §1 ("the backend is the product; `ace` is the doorway"). ACE
spawning a backend to make a decision *for* the user inverts that: the doorway becomes the
agent. And the prompt that drives the decision was baked into the binary, so the school
maintainer — the one person who actually knows which skills matter for their domain —
could not change a word of it without an ACE release.

As a school skill it gets what it always needed: the agent's full context, the ability to
ask a follow-up, and a prompt that ships and versions with the school.

## Consequences

- **Narrowing now applies from the next session, not the current one.** The old
  auto-trigger ran before the interactive session launched, so a narrowed `skills` list
  took effect immediately. A skill runs *inside* a session already loaded with everything.
  This is the one real capability lost, and it is accepted: the token cost of one wide
  session is smaller than the cost of ACE owning an LLM workflow.
- The startup nudge ("school has 47 skills — consider narrowing") is gone entirely rather
  than reimplemented. If it returns it should be a session-prompt layer or the school
  skill's own trigger, not a Rust-side y/N.
- Backlog **151**, **215**, **244** and the pending learn re-run-threshold note are moot.
  **236** collapses to the multi-select picker already shipped.
- `exec_one_shot` stays on the backend contract but `ace -p` is now its only caller.
