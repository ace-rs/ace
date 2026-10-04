# Testing

## Decision

Use `tempfile` crate for integration test isolation. Dagger/testcontainers deferred — only
revisit if multi-distro or network-dependent testing becomes necessary.

Rationale: ACE's integration surface is filesystem + git + symlinks. A sandboxed temp directory
covers this without container overhead.

## No test reads the host

A test outcome never depends on the machine it runs on. A developer's real
`~/.config/ace/ace.toml`, `~/.gitconfig`, or data directory must not be able to turn a
test green or red.

Integration tests get this from `TestEnv`, which `env_clear()`s and points `HOME` and
every `XDG_*` var at sandbox subdirs. Unit tests get it because ambient paths are not
ambient: `AcePaths` is resolved once at the process edge (`main.rs`) and handed to
`Ace::new`, so a unit test constructs an `Ace` whose every config layer roots inside its
own tempdir.

The check is empirical, not visual — run the suite against a *poisoned* user config
(one that sets `school`, `backend`, `trust`) and against an empty home. Any test whose
result differs between the two is reading the host.

## Test Categories

### Unit tests

- Live in `src/` alongside the code they test (`#[cfg(test)]`)
- Pure logic only — no filesystem, no git, no network
- Follow conventions in the `rust-coding` skill

### Integration tests

- Live in `tests/` directory
- Exercise filesystem, git, symlinks, and cross-module interactions
- Each test gets its own `TestEnv` — no shared state, no `#[serial]`

Backend adapter filesystem tests may run in process through a `#[cfg(test)]`
module that includes a file under `tests/`, as configuration action tests do.
They exercise the real adapter's file loading and publication with supplied,
already verified external inputs; they never invoke an external backend or
replace its executable with a shell script. Pure parsing and command construction
remain unit tests, and CLI orchestration remains Flaude integration coverage.

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

### Real backends live outside the test suite

Real backend binaries are still needed — to read a CLI surface, or to confirm a backend
consumes what ACE emits. They are whatever is installed on the machine
(`docs/vendor/backend-install.md`) and are exercised by hand, never by `cargo test`. No
test may look for a backend binary, skip on its absence, or branch on what it finds; a
suite that passes on a machine with no backend installed is the only passing suite.

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
  `project_dir`, `extra_args`, `cmd`, `model`, `effort`
- **exec_one_shot**: `prompt` (`kind` + `text`), `env`,
  `project_dir`, `extra_args`, `cmd`, `model`, `effort`
- **mcp_add**: `name`, `url`, `headers`
- **mcp_remove**: `name`

One-shot output is controllable via env vars
(`FLAUDE_ONE_SHOT_STDOUT`, `FLAUDE_ONE_SHOT_STDERR`,
`FLAUDE_ONE_SHOT_EXIT_CODE`) for testing exit code propagation
and output passthrough.

### Prerequisite and health failure scenarios

Flaude accepts JSON controls through the resolved invocation environment:

- `FLAUDE_PREFLIGHT`: `"ready"` (default), `{"unavailable":"message"}`,
  `{"failed":"message"}`, or `{"invalid_arguments":"message"}`.
  These exercise the existing prerequisite exit classes and the requirement to
  verify before publishing config, linking, provisioning, or launching.
- `FLAUDE_MCP_LIST`: the same prerequisite responses, applied when MCP inventory
  is consulted. This tests MCP prerequisites independently of launch validation,
  including that empty or fully excluded school server sets skip MCP inventory.
- `FLAUDE_MCP_CHECK`: `"healthy"` (default),
  `{"statuses":{"server-name":false}}`, or `{"failed":"message"}`.
  A statuses map must cover each requested server; missing results fail the
  fixture instead of silently reporting healthy. Health checks never change
  registration records.

Malformed controls fail explicitly. Backend-instance environment values override
ambient controls through the same resolved environment used by other operations.

Prerequisite and MCP operation attempts append to the invocation home's
`.flaude-calls.jsonl`, including action, requested names, command, environment,
project directory, model, effort, and passthrough arguments. Records include failed
attempts; successful prerequisite checks may be cached for an identical invocation.
Tests use these records to verify ordering and resolved intent, not incidental
inventory-read counts.

Pi coverage follows this split: Flaude verifies ACE's prerequisite guards,
school refresh, dispatch, and health-failure handling; Pi units verify package
probe arguments, parsing, manifest compatibility, and health report policy;
in-process adapter tests verify real Pi configuration layers, symlink preservation,
and concurrent-write rejection. A Flaude success is not verification of Pi's CLI.
