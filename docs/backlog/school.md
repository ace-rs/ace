# School

Source: [Outline][source], revision 7.
Status reconciled against repository records at `9df624a` on 2026-09-05.

[source]: https://outline.prodigy9.co/doc/school-0LU8NLqTfG

Reference record for **ACE Home** (`ace-rs/school`) — the base school other schools
import. These records concern a separate repository; their presence here does not
authorize changes there.

Scope is the school itself: skills under `skills/`, `school.toml`, `docs/`, and repo
conventions. Work on the ACE tool belongs in the epic docs (A–M) instead.

Conventions match the rest of this collection: `- [x]` shipped · `- [ ]` open.

## ACE Home source status

The Outline School record contains no open tasks; the pantry notes below come from the
local session trail and have an unresolved owner.

## Shipped

- [x] `ace-connect` opencode backend — bridge owns exactly one session it creates, never
      scans for the TUI's.
- [x] `discover.sh` sweeps ownerless `.sock` / `.codex-app.url` sidecars; `send.sh` drops
      the `-1` sentinel that could reach `kill -0 -1`.
- [x] Relay smoke-test method + probe script — `docs/guides/connect-relay-smoke-test.md`,
      run live in both directions.
- [x] Commit-prefix convention: prefix with the skill name, not `skills:`.

## Recorded pantry follow-ups

- [ ] **lowfat-diff-hunks** · agent:inferred, recorded in `.ace/save.md`. The git compact
      filter mis-associated hunks for this command:

      ```sh
      lowfat git diff -- docs/spec/configuration.md src/config/mod.rs \
        src/cmd/config.rs tests/config_test.rs
      ```

- [ ] **lowfat-opencode-dispatch** · agent:inferred, same source.
      `lowfat opencode --version` selects lowfat's filter subcommand instead of invoking
      the executable.

The owning school/pantry checkout is not established by these notes. Keep the reproduction
evidence here until that owner is identified; no external investigation or edit is
implied.
