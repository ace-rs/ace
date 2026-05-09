# Backend Implementation Plan: OpenCode + Droid

Date: 2025-05-09
Linear: PROD9-17 (OpenCode), PROD9-122 (Droid), PROD9-119 (gitignore)

## Specs (done)

Updated `spec/backends/opencode.md` and `spec/backends/droid.md` with
confirmed CLI behavior. WIP banners removed. Decisions locked:

- **OpenCode session prompt**: agent file `.opencode/agents/ace.md`
  (`mode: all`), launched via `--agent ace` for both interactive and
  one-shot.
- **OpenCode one-shot**: `opencode run --agent ace [message..]`
- **OpenCode yolo**: `--dangerously-skip-permissions`
- **OpenCode MCP**: direct config write (interactive wizard unusable)
- **Droid session prompt (interactive)**: ACE-managed block in
  `AGENTS.md` with HTML comment markers, idempotent replace.
- **Droid session prompt (one-shot)**: `droid exec
  --append-system-prompt-file <tmpfile>`
- **Droid MCP**: CLI `droid mcp add <name> <url> --type http
  [--header ...]`
- **Droid linked folders**: skills only (no commands, no agents).
  `droids/` mapping deferred.

## Code changes needed

### New files

1. **`src/backend/opencode.rs`** — all 7 functions:
   - `is_ready`: `~/.local/share/opencode/auth.json` (respect
     `OPENCODE_HOME`)
   - `exec_session`: `opencode --agent ace` (agent file carries
     session prompt)
   - `exec_one_shot`: `opencode run --agent ace <prompt>`
   - `mcp_list`: parse `opencode.json`/`opencode.jsonc` in project
     root
   - `mcp_add`: direct JSONC config write with merge
   - `mcp_remove`: direct config write, remove entry
   - `mcp_check`: agent-probe pattern

2. **`src/backend/droid.rs`** — all 7 functions:
   - `is_ready`: `~/.factory/settings.json`
   - `exec_session`: write managed block to `AGENTS.md`, then exec
     `droid` (interactive)
   - `exec_one_shot`: `droid exec --append-system-prompt-file
     <tmpfile> <prompt>`
   - `mcp_list`: parse `~/.factory/mcp.json`
   - `mcp_add`: `droid mcp add <name> <url> --type http [--header
     ...]`
   - `mcp_remove`: `droid mcp remove <name>`
   - `mcp_check`: agent-probe via `droid exec`

### Modified files

3. **`src/backend/mod.rs`** — add `OpenCode` and `Droid` to `Kind`
   enum, `dispatch!` macro, `ALL`, `name()`, `backend_dir()`,
   `instructions_file()`, `from_name()`.

4. **`src/actions/project/prepare.rs`** — update `is_supported()`
   matrix:
   - OpenCode: skills ✓, rules ✗, commands ✓, agents ✓
   - Droid: skills ✓, rules ✗, commands ✗, agents ✗

5. **`src/actions/project/update_gitignore.rs`** — `.opencode` and
   `.factory` now auto-included via `Kind::ALL` iteration (no code
   change needed if `backend_dir()` returns the right values).

### New: agent file writing (OpenCode only)

ACE needs a new action (or extension of prepare) that writes
`.opencode/agents/ace.md` with the session prompt during setup. Must
run AFTER linking (to avoid clobbering — linked `agents/` is a
whole-dir symlink; the `ace.md` file must coexist).

**Conflict risk**: if school has `agents/` folder, it's linked as a
whole-dir symlink to `.opencode/agents/`. Writing `ace.md` there
would put it inside the school clone (not the project). Options:
- Write to `~/.config/opencode/agents/ace.md` (global, avoids
  conflict)
- Switch `agents/` to per-entry symlinks for OpenCode (like skills)
- Write before linking and accept that the symlink replaces it

Recommended: global path `~/.config/opencode/agents/ace.md` — it's
the agent for the user, not the project.

### Droid AGENTS.md managed block

New helper (likely in `src/backend/droid.rs` or a shared module):
- Managed block with `<!-- ACE-managed -->` / `<!-- end ACE -->`
  HTML comment markers
- Same replace/append pattern as gitignore
- Called during `exec_session` before exec-replacing into `droid`

## Open items from spec audit

- OpenCode agent file + linked agents/ folder conflict (see above)
- Droid exec mode: both AGENTS.md managed block AND
  `--append-system-prompt-file` fire → duplicate session prompt.
  Accept duplication or skip managed block for exec mode.
- Session resume behavior for both backends needs documenting.
