# Testing

## Decision

Use `tempfile` crate for integration test isolation. Dagger/testcontainers deferred — only
revisit if multi-distro or network-dependent testing becomes necessary.

Rationale: ACE's integration surface is filesystem + git + symlinks. A sandboxed temp directory
covers this without container overhead.

## Test Categories

### Unit tests

- Live in `src/` alongside the code they test (`#[cfg(test)]`)
- Pure logic only — no filesystem, no git, no network
- Follow conventions in the `rust-coding` skill

### Integration tests

- Live in `tests/` directory
- Exercise filesystem, git, symlinks, and cross-module interactions
- Each test gets its own `TestEnv` — no shared state, no `#[serial]`

## TestEnv

Sandboxed filesystem root backed by `tempfile::TempDir`. RAII cleanup on drop.

See `tests/common/mod.rs` for the full API. Key design points:

- **Sandbox isolation**: `ace()` returns a `Command` with `env_clear()`, `HOME`/`XDG_CONFIG_HOME`/`XDG_CACHE_HOME` pointed at sandbox subdirs. Tests never touch real user files.
- **Escape prevention**: all path methods go through `path()`, which panics on absolute paths.
- **Remote school fixture**: `setup_remote_school()` creates a bare origin repo, cache clone, index entry, and ace.toml — everything needed to test Update/Pull without network access.

## File Layout

```
tests/
  common/
    mod.rs              # TestEnv, RemoteSchool, FlaudeRecord, helpers
  <command>_test.rs     # one file per CLI command / action area
```

## Conventions

- One `TestEnv` per test function — no sharing between tests
- `unwrap()` / `expect()` are fine in test code
- Test file names: `tests/*_test.rs`
- Helper functions go in `tests/common/mod.rs`

## Backend Testing Strategy

Integration tests verify ACE's behavior, not backend CLI syntax.
Real backends (`claude`, `codex`) are external binaries whose CLI
interface can change between versions — coupling tests to their
argument shapes creates false failures and maintenance burden.

### Flaude is the integration test backend

All integration tests that exercise exec, MCP, or session dispatch
MUST use the Flaude fixture backend. Flaude records the *intent*
ACE passes (trust, resume, session_prompt, env, cmd, prompt) as
JSONL — tests assert on those fields.

**Never** shell out to a real backend binary (or a fake shell script
impersonating one) in integration tests. If a test needs a binary on
`$PATH` it is testing the wrong layer.

### Unit tests own backend-specific behavior

Backend-specific logic — argument construction, MCP CLI syntax,
config file formats, trust flag mapping — belongs in unit tests
inside `src/backend/<name>.rs`. These tests call the backend
module's free functions directly with known inputs and assert on
the produced `Command` args or output.

This split gives us:

| Layer                      | What it tests                                                    | Backend used         |
|----------------------------|------------------------------------------------------------------|----------------------|
| Unit (`src/backend/*.rs`)  | Arg building, parsing, flag mapping                              | Direct function calls |
| Integration (`tests/`)     | Dispatch routing, env merging, trust/resume flow, MCP registration | Flaude only          |

### What Flaude records

Flaude writes JSONL to `$HOME/.flaude-exec-records.jsonl` and
`$HOME/.flaude-mcp-records.jsonl`. Fields available for assertion:

- **exec_session**: `trust`, `resume`, `session_prompt`, `env`,
  `project_dir`, `extra_args`, `cmd`
- **exec_one_shot**: `prompt` (`kind` + `text`), `env`,
  `project_dir`, `extra_args`, `cmd`
- **mcp_add**: `name`, `url`, `headers`
- **mcp_remove**: `name`

One-shot output is controllable via env vars
(`FLAUDE_ONE_SHOT_STDOUT`, `FLAUDE_ONE_SHOT_STDERR`,
`FLAUDE_ONE_SHOT_EXIT_CODE`) for testing exit code propagation
and output passthrough.
