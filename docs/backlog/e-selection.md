# E — Skill selection & learn

Source: [Outline][source], revision 7.
Status reconciled against repository records at `9df624a` on 2026-09-05.

[source]: https://outline.prodigy9.co/doc/e-skill-selection-learn-ICDLuwjpx3

Everything converging on the `ace.toml` `skills=` write path.

> `ace learn` **was removed from the binary on 2026-07-22** — commit `8b40432`, ruling in
> `docs/decisions/2026-07-22-learn-leaves-the-binary.md`. Narrowing `skills` is a school
> skill now. Every learn-prompt item below is moot as filed; kept for the record.

- [ ] **120** per-repo skill selection to limit token usage — still live, the `skills=`
      filter itself stays

## Closed by the learn removal

* **151** backend-driven two-diff capture — moot, action deleted
* **236** skill-count 3-way menu — collapsed; the manual TUI-select arm shipped as the
  multi-select picker, the auto-learn arm no longer exists
* **215** stop re-prompting after a no — moot, no prompt left
* **244** don't prompt learn on `ace school pull` — moot, auto-trigger deleted

The old "learn re-run threshold" note (only prompt on substantial school deltas) dies with
them.

## Ideas / later

* **134** skill filter: token-compress skill content at link time
* 🆕 `inject=` key — inject skill content (just `skill.md`) into the session prompt
  directly; useful for pre-loading e.g. ace-connect
* 🆕 **startup nudge, if it returns** — "school has N skills, consider narrowing" was
  deleted outright, not reimplemented. Should come back as a session-prompt layer or the
  school skill's own trigger, never a Rust-side y/N. Needs a decision before build.
