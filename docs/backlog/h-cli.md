# H — CLI ergonomics & inspection

Source: [Outline][source], revision 17.
Status reconciled against repository records at `9df624a` on 2026-09-05.

[source]: https://outline.prodigy9.co/doc/h-cli-ergonomics-inspection-QD9m16mtOX

- [x] **compact-startup-logo** command-specific terminal wordmarks: the three-line big
      form for session entry, compact `ΠCE` for ACE-owned mutations, and no wordmark for
      read surfaces. Build identity is a separate `version X (commit)` info item;
      terminal-only and non-porcelain gates remain. Shipped `54f3fc9`, `0e925b9`, and
      `328c637`; contract in `docs/spec/ux.md` §6.
- [x] **44** `ace diff` pages through `$PAGER` (default `less -FRX`) when stdout is a
      terminal and `--porcelain` is off — shipped 2026-08-03 (`8cdd6df` + `7c6297e`);
      contract in `docs/spec/ux.md` §8.
- [x] `ace explain` surfaces the skill's frontmatter description — shipped 2026-08-03
      (`0cde8c1`).

## Config command consistency

- [ ] **config-command-consistency** repair explicit trust-default overrides, preserve
      unrelated configuration during writes, and align config inspection with runtime
      resolution. Audit evidence and proposed phases: `.ace/config-command-audit.md`;
      retained regression tests: `tests/config_contract_test.rs`; later-slice candidates
      are retained in `.ace/config-contract-deferred.rs`.
      The first slice is complete: explicit trust overrides retain their presence and
      provenance; targeted writes preserve unrelated fields, comments, and key formatting,
      publish atomically, preserve file permissions and symlinks, and invalidate caches.
      Existing keys are edited through `TableLike::get_mut`; only absent keys are inserted.
      The two-crate choice is accepted at `74e86dd`; the user approved adding toml_edit
      and upgrading all packages on 2026-09-06. Final validation: 831 passed tests, no
      failures, two ignored, clean Clippy, formatting, and full-slice audit.
      `.ace/dependency-upgrade-audit.md` records the dependency assessment and resolved
      comment-preservation finding; no dependency-driven API migration was found.
      Remaining work: inspection/runtime consistency, unknown or misplaced-field
      diagnostics, initial built-in selection, effective-write feedback, typed explanation
      rendering, and compatibility-aware bare output. These need a later approved slice.
      Custom-selector scope policy retains its existing owner
      in [A — Backends](a-backends.md), item 146.
      Ask provenance: "ok $ace-save please we'll restart the work next slice beginning
      w dependency clarification/research first" (2026-09-05); "confirm start"
      (2026-09-06, first implementation slice); "ok do this: add toml_edit, then upgrade
      ALL packages in one go please, then audit for any changes needed and plan for
      those after." (2026-09-06, upgrade and assessment); "approve" (2026-09-06,
      comment-preservation repair, full verification, audit, and local commits).

## Ideas / later

* **126** auto-spawn a tmux side pane with editor / diff view on session start (borderline
  big-bet)
* **13** `ace llm-help` — AI-friendly CLI guidance. **Deferred 2026-08-03**: the
  school-instructions catalog design
  (`docs/scratch/2026-05-30-school-instructions-catalog.md`) proposes closing 13 as
  superseded; hold until that decision lands.
* 🆕 `ace config expand` (name tentative) — once overriding the system prompt is allowed,
  a new verb that writes the session-prompt config into the user's local `ace.toml`, so
  overrides are configured from there. Gated on the prompt-override work. Replaces
  **227**, which was killed 2026-07-26. Unresolved storage choice: H's source records an
  earlier roadmap proposal at `~/.config/ace/prompts/session.md`, while this idea uses
  `ace.toml`; the latest roadmap contains neither choice. Reconcile before building.

## Other completed work

- [x] **190 / global** `--yes` shipped 2026-07-29 (`dcb5c2e`). `--yes`/`-y` waives being
      asked; a set `CI` or `CONTINUOUS_INTEGRATION` variable implies it. Landed with a
      decoupling: `OutputMode` is gone, replaced by `Io::should_colorize` / `should_emit`
      / `can_ask`, so `--porcelain` no longer suppresses prompts and `--yes` no longer
      downgrades output. Contract in `docs/spec/ux.md` §8.
- [x] **maverick** easter egg removed 2026-07-29 (`a866f7f`) — command, the 2.1 MB bundled
      GIF, and the `gif` dependency. Alt-screen plumbing kept for planned full-screen UI.

## Prompt ownership

**prompt-override** owns user-editable session prompt design here; J owns related template
documentation. Neither candidate storage location is settled.
