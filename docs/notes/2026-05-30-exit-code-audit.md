# Exit Code Audit

2026-05-30 — comprehensive scan of every exit path in the ACE binary.

## Current Mechanisms

| Mechanism               | Location           | Exit Code    | Scope                    |
| ----------------------- | ------------------ | ------------ | ------------------------ |
| `exit_on_err()`        | `cmd/mod.rs:451`  | `1`          | All `Result<(),CmdError>`|
| `std::process::exit(N)` | `cmd/upgrade.rs:10`| `1`          | Upgrade errors           |
| `std::process::exit(N)` | `cmd/main.rs:94`   | child code   | One-shot child exit      |
| `exec_replace()`        | `platform.rs`      | child code   | Session (all backends)   |
| SIGINT handler          | `ace/io.rs:37`     | `130`        | Ctrl+C during any phase  |
| `fn main() -> ()`       | `main.rs:23`       | `0` implicit | Happy path               |

`std::process::ExitCode` is not used anywhere. `main()` returns `()` with
direct `exit()` calls for all error paths.

## Gaps

### 1. User cancellation is inconsistent

`IoError::Cancelled` (from `inquire` prompt abort) surfaces as
`CmdError::Prompt(Cancelled)` → `exit(1)`.

But some callers swallow cancellation and return `Ok(())`:

- `cmd/learn.rs:17` — user declines the proceed prompt → silent `Ok(())`, exits
  `0`. Arguably correct (explicit consent gate, user said no).
- `cmd/main.rs` → `recover_backend()` → `ace.prompt_select()` cancels →
  propagated as error → exits `1` with "cancelled". Harsh for a user-initiated
  abort from the backend picker.

These two paths disagree on whether cancellation is an error.

### 2. `fn main()` panics on `current_dir()` failure

`main.rs:33` — `std::env::current_dir().expect(...)`. If the cwd is deleted
between process start and this call, you get a panic (exit code 101).
Unreachable in normal use but inconsistent with the error-message-driven
approach.

### 3. No distinction between usage errors and operational errors

All errors exit `1`. Invalid flags (`--user --project` combo), missing school,
git clone failure, malformed config — all produce the same exit code.
Downstream tooling (shell scripts, CI, makefiles) can't distinguish "you
invoked me wrong" from "the operation failed". Biggest semantic gap.

### 4. MCP health-check failures always exit 0

`cmd/mcp.rs:72-78` — `mcp_check` failure is downgraded to `warn()` and
returns `Ok(())`. The `mcp check` subcommand always exits `0` even when all
servers are unhealthy. Informational mode is reasonable, but a strict
health-check in CI would want non-zero.

### 5. One-shot exit code passthrough has no upper-bound clamp

`cmd/main.rs:94` — `exit(output.status.code().unwrap_or(1))`. Child code is
passed through verbatim. Intentional and tested (`one_shot_exit_code_propagates`
with code 42). On Unix, `waitpid` truncates to 8 bits, so this is fine in
practice. Not a real gap but worth noting.

## Items explicitly left alone

- `exec_replace` — correct, kernel handles exit codes.
- SIGINT at `130` — standard, correct.
- Flaude's `synthesize_status` — test-only, correct.
- `main()` returning `()` — switching to `-> ExitCode` would require
  restructuring every subcommand to return a value instead of calling
  `exit_on_err` and returning. Not worth it for cosmetic benefit; the
  `CmdError`-level abstraction is cleaner.

## Proposed Abstraction

An `ExitCode` enum on `CmdError`, with `exit_on_err` dispatching on it:

| Variant        | Code  | When                                            |
| -------------- | ----- | ----------------------------------------------- |
| `Ok`           | `0`   | Success                                         |
| `Usage`        | `1`   | Invalid flags, unknown subcommand, arg          |
|                |       | validation                                      |
| `Unavailable`  | `2`   | Missing school, no specifier, backend           |
|                |       | not found                                       |
| `Operational`  | `3`   | Git clone failed, config write failed,          |
|                |       | download failed                                 |
| `Cancelled`    | `130` | User-initiated abort (Ctrl+C, prompt            |
|                |       | cancel)                                         |
| `McpUnhealthy` | `4`   | (optional) MCP health-check failure,            |
|                |       | strict mode                                     |

One-shot child exit code (`1..255`) bypasses the enum entirely — it already
has its own `exit(child_code)` path that is tested.

Routing:

- `CmdError` gains `fn exit_code(&self) -> ExitCode`.
- `exit_on_err` calls `exit(err.exit_code().code())`.
- `IoError::Cancelled` maps to `130` explicitly.
- Session path stays as-is (exec_replace, never returns).
