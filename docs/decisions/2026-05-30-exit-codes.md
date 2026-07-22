# Differentiated Process Exit Codes

- **Date:** 2026-05-30
- **PR:** manual
- **Status:** accepted

Baseline: ACE v0.7.1.

## Decision

ACE classifies every error exit into one of four codes via an `ExitCode` enum on
`CmdError`. `exit_on_err` dispatches on `err.exit_code().code()` instead of the
former unconditional `exit(1)`.

| Code  | Class         | Meaning                                                   |
| ----- | ------------- | --------------------------------------------------------- |
| `0`   | `Ok`          | Success                                                   |
| `1`   | `Usage`       | Bad input the user authored — CLI flags/args or config    |
| `2`   | `Unavailable` | A required resource or precondition is absent             |
| `3`   | `Operational` | A valid operation was attempted and failed                |
| `130` | `Cancelled`   | User-initiated abort (Ctrl+C, prompt cancel)              |

`0` is in the table for completeness but is not an `ExitCode` variant — success
exits via the normal `main()` return and never flows through `CmdError`, so the
enum models only the four *error* classes (an error that is "Ok" is
unrepresentable).

The one-shot child exit code (`ace -p`) bypasses the enum — it already has its own
tested `exit(child_code)` passthrough (`one_shot_exit_code_propagates`). SIGINT
keeps its direct `exit(130)` in the Ctrl+C handler.

## The classification rule

The three error classes are not "severity" tiers; they answer *who fixes it and
how*:

- **`Usage` (1)** — the user gave ACE something invalid. CLI flag conflicts
  (`--user --project`), bad `--env`/`--trust`, an unknown `config`/`paths` key, an
  invalid `--skill` glob, naming a skill/source that does not exist. Also bad input
  the user *authored in a config file*: malformed `ace.toml`/`school.toml`
  (`ConfigError::Parse`/`Encode`), a path traversal in config, a misdeclared backend
  (`Unresolvable`/`KindMismatch`). Fix: change what you typed or wrote.

- **`Unavailable` (2)** — an ambient precondition ACE needs is missing. No school
  configured, school not initialized, not in a git repo, no user config/cache/data
  directory, a selected backend name that resolves to nothing. Fix: set it up
  (`ace setup`, `git init`, `ace school init`), not re-type the command.

- **`Operational` (3)** — input was valid and the precondition was met, but the
  operation failed anyway. Clone/fetch, file write, download/install, backend spawn
  or non-zero exit, git exec/exit, MCP registration, skill-discovery I/O. Also the
  "completed but found problems" outcomes: `ace school validate` reporting N issues,
  and import/pull rejecting inadmissible skills. Fix: address the underlying failure
  (network, disk, the reported issues).

- **`Cancelled` (130)** — the user aborted. Prompt cancel (`IoError::Cancelled`) and
  Ctrl+C. 130 is the shell convention for SIGINT-terminated processes.

The dividing line between `Usage` and `Unavailable` is **arg-named vs ambient**:
naming a thing that does not exist *as an argument* (`--backend foo`, `--skill bar`)
is the user's `Usage` error; a precondition that is simply absent from the
environment is `Unavailable`. `BackendError::Unknown` is the one borderline case —
it can arise from either `--backend foo` or a stale `backend = "..."` in config. It
maps to `Unavailable` because the dominant source is stale config (the interactive
`recover_backend` picker exists precisely for that case), and "the backend you asked
for isn't available" reads true for both.

## Why a contract, not the status quo

Before this, every error path returned `1`. A shell script, Makefile, or CI step
driving `ace` could not tell "you invoked me wrong" from "the clone failed" from
"you hit Ctrl+C" — the single most-requested distinction for any user-facing CLI.
The four-code scheme is deliberately small: enough to branch on, not a per-error
catalogue that ossifies into a compatibility burden.

## Killing `CmdError::Other`

The blocking obstacle was `CmdError::Other(String)` (plus its hinted twin
`OtherHinted`): ~30 call sites used it for *both* usage errors and operational
failures, so no variant→code rule could be correct. A bare `Other` is a lazy
escape hatch — it lets an author dodge the one decision that matters here.

`Other`/`OtherHinted` are collapsed into a single ad-hoc variant whose exit class is
**mandatory at construction**:

```rust
CmdError::Adhoc { message: String, hints: Vec<String>, code: ExitCode }
```

reachable only through classifying constructors — there is no un-classified
fallback to grab:

```rust
CmdError::usage("cannot combine --user and --project")     // 1
CmdError::unavailable("no schools cached, run ace setup")  // 2
CmdError::failed(format!("download failed: {e}"))          // 3
CmdError::unavailable("...").with_hint("try: ace setup")   // hints, any class
```

Renaming `Other` to `Unexpected` (the obvious alternative) was rejected: it keeps
the catch-all, just relabeled. The forcing function is the *mandatory code*, not the
name. A side benefit: recovery hints, previously only on `OtherHinted`, now attach
to any ad-hoc error.

## Variant → code map

Wrapper variants (`#[error("{0}")]` around `ConfigError`/`GitError`/`IoError`/…)
**delegate** to the inner error's code rather than fixing one at the outer layer —
e.g. `PrepareError::Config(ConfigError::Parse)` is `Usage`, `PrepareError::Clone` is
`Operational`.

| Leaf error                          | → code                                        |
| ----------------------------------- | --------------------------------------------- |
| `ConfigError::Parse/Encode/Traversal*` | `Usage`                                    |
| `ConfigError::NoConfig/NoConfigDir/NoCacheDir/NoDataDir` | `Unavailable`             |
| `ConfigError::Io`                   | `Operational`                                 |
| `SchoolError::NoSpecifier/NotInitialized` | `Unavailable`                           |
| `BackendError::Unknown`             | `Unavailable` (borderline, see above)         |
| `BackendError::Unresolvable/KindMismatch` | `Usage`                                 |
| `SetupError::NotInGitRepo`          | `Unavailable`                                 |
| `SetupError::AlreadySetUp` / `InitError::AlreadyExists` | `Usage`                    |
| `GitError::*`                       | `Operational`                                 |
| `PrepareError::Clone/Write`         | `Operational`                                 |
| `Learn::BackendSpawn/BackendNonZero/TomlWrite` | `Operational`                      |
| `AddImport::NoSkills/SkillNotFound` | `Usage`                                       |
| `AddImport::Clone` / `PullImports::Git` | `Operational`                             |
| `PullImports::InvalidDecl`          | `Usage`                                       |
| `*::RejectedImports` (inadmissible skills) | `Operational`                          |
| `SkillError::Discovery`             | `Operational`                                 |
| `IoError::Cancelled`                | `Cancelled`                                   |

\* Traversal is a fail-closed rejection of untrusted config content — the user
authored a bad path, so `Usage`.

## Backcompat

None owed. ACE is a user-facing CLI pre-1.0; exit codes were never a published
contract and no consumer depends on "everything is 1". Scripts testing `!= 0` are
unaffected; only the moves `1 → {2, 3, 130}` are observable, and they are
strict refinements. This document *is* the contract from here on.

## Supersedes

`docs/scratch/2026-05-30-exit-code-audit.md` (deleted) — the original scan that
proposed the scheme. It under-counted the `Other` overload, missed the
`ace school validate` exit and the wrapper-delegation requirement, and floated an
optional fifth `McpUnhealthy` code (dropped: `mcp check` stays informational at
exit 0 — adding a strict mode is a separate CLI surface change, not part of this
contract).
