# Cancelled / superseded

Source: [Outline][source], revision 10.
Status reconciled against repository records at `9df624a` on 2026-09-05.

[source]: https://outline.prodigy9.co/doc/cancelled-superseded-ryHk1rP9yA

Kept as records so the rejection trail stays queryable instead of re-litigated.

* **122** complete Droid (Factory.ai) backend — dropped 2026-07-22; Factory is not a
  target. Hermes takes its first-tier slot (A)
* **9** investigate Cursor/Continue/Cline — superseded by custom-backends (129)
* **26** Homebrew tap — Homebrew shipped via 194; done-by-other
* **22** `ace switch` (duplicate) — superseded by 55 + the live 69
* **38** global CLAUDE.md for cross-backend preferences — duplicate
* **227** `ace template` renders builtin prompt templates to stdout — killed 2026-07-26,
  not deferred. Dumping the 9 builtins in `src/templates/builtins/` to stdout is
  inspection of a constant, and its scope was already ambiguous over the 3 `tpl_*`
  scaffolding files vs the 6 `prompt_*` ones. Superseded by the `ace config expand` idea
  in H, which only makes sense once prompt-override lands (H)
* **32** `ace tunnel` and the unnumbered `ace remote` — superseded 2026-08-26 by
  tmux-hosted session attachment over ordinary SSH. ACE owns no remote transport; the
  managed-session work lives in
  [M — Managed sessions, connect & workspaces](m-sessions.md).
