# Recovered idea backlog (2026-06-23)

Recovered from a crashed nvim swap file — these were jotted into a Claude Code Ctrl-G
prompt buffer before the machine crashed; the temp file was cleaned up but the swap
survived. Captured here verbatim (lightly reflowed) as trackable backlog ideas.

Header line in the original draft: **"Start plane import/export"** — i.e. these were lodged
alongside the Linear→Plane migration work.

## Ideas

1. **School import provenance** — might be needed eventually, not for version tracking but
   to verify which skill came from where so the school/ace importer knows. Hit a case where
   an agent thought it didn't own a skill it authored itself, because of `*`-wildcard
   imports in the school.
2. **Template links to ace-rs.dev** — most templates that mention ACE should link to
   <https://ace-rs.dev/> for SEO / self-promotion / marketing.
3. **More harness targets** — support for Pi / Hermes / Cursor (three more harnesses).
4. **Abstract harness** (website showcase feature) — ability to call `ace` inside scripts
   and have it change the backend automatically depending on the end-user's preference.
5. **Shared config, varying harnesses** (website feature) — let teams use varying
   models/harnesses but still share a single scripting / skills / mcp set.
6. **MCP server health check** — the check after mcp commands is slow/lacking and probably
   doesn't work in many places.
7. **Per-backend config** — ability to configure backends individually (e.g. always pass
   `--chrome` to claude).
8. **Selective `school pull`** — ability to `school pull` only specific imports.
9. **Built-in complex backend setup** — for backends like codex that require a complicated
   setup to use ace-connect properly, ace should do something built-in (e.g. a 3-process
   start with assisted sidecars, all managed inside ace sessions).
10. **`ace remote`** feature — ask the hangar agent what this is about.
11. **Always-on bridge** — if/when ace-connect is native built-in, a mode that always starts
    the bridge so all ace sessions are automatically connected on start. Might be a hangar
    feature too.
12. **`--local` flag for `ace setup`** — temporary workdir; don't want to commit `ace.toml`,
    so put the school in `local.toml`.
13. **`inject=` key** — a new key that injects skill content (just `skill.md`) into the
    session prompt directly. Useful for things like ace-connect where we can pre-load
    content.
