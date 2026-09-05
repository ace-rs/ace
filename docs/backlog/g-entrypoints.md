# G — Entrypoints & headless

Source: [Outline][source], revision 6.
Status reconciled against repository records at `9df624a` on 2026-09-05.

[source]: https://outline.prodigy9.co/doc/g-entrypoints-headless-UcoFNgcTmT

Alternate ways to invoke ACE — headless serving, transparent shims, and input automation.

## Ideas / later

* **245** `ace serve` — normalize headless/serve across claude/codex/opencode. It must
  consume the managed-session and backend-component boundaries from
  [M — Managed sessions, connect & workspaces](m-sessions.md), not create a second
  runtime.
* **246** transparent replacement of `claude` / `codex` (shim mode).
* **159** polymorphic flags for common backend operations (one-shot prompt, etc.).
  Partial history: `4185348` already implements one-shot transport under PROD9-159;
  revalidate the remaining normalization scope before treating the whole item as open.
* **156** multi-backend fork/compare runs. The old `ace mux` / `split` names describe
  comparison UX, not the tmux executor owned by M.
* **160** `ace --bare` — start a backend with no skills or school. This is distinct from
  M's bare workspace entry.
* 🆕 abstract harness — call `ace` inside scripts and select the backend from the end
  user's preference.
* 🆕 idle/on-pause command injection — detect an idle interactive session and invoke a
  configured command, such as `ace-save`.
* 🆕 ACE macros — replay user-defined input chords into an interactive harness.
* 🆕 advance ACE `/loop` — re-submit a prompt when a backend exposes an explicit
  waiting-for-input signal.

## Relationship to managed sessions

Epic G owns alternate invocation and input-automation ideas. It does not own process
lifecycle, relay transport, remote access, or workspace composition; those boundaries are
tracked in [M — Managed sessions, connect & workspaces](m-sessions.md).

The old always-on bridge is superseded by connected bare startup through
`[connect] enabled = true`. The old `ace remote` and **32** `ace tunnel` ideas are
superseded by ordinary SSH plus `ace session attach`. Auto-pause belongs to M's later
**advanced-session-lifecycle** task. Idle injection, macros, and loop continuation remain
separate input-automation proposals until a backend exposes enough control to design them
honestly.
