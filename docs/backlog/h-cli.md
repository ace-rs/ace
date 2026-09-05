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
