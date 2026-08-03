# Process Exit Codes

ACE classifies every error exit into one of four codes via an `ExitCode` enum on
`CmdError`. `exit_on_err` dispatches on `err.exit_code().code()`. This file is the
contract; scripts and CI may branch on these values.

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

Two bypasses: the one-shot child exit code (`ace -p`) passes the backend child's
code through directly (`one_shot_exit_code_propagates`), and SIGINT keeps its
direct `exit(130)` in the Ctrl+C handler.

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

## No un-classified errors

There is no catch-all variant. Ad-hoc errors carry their exit class **mandatorily
at construction** through classifying constructors:

```rust
CmdError::usage("cannot combine --user and --project")     // 1
CmdError::unavailable("no schools cached, run ace setup")  // 2
CmdError::failed(format!("download failed: {e}"))          // 3
CmdError::unavailable("...").with_hint("try: ace setup")   // hints, any class
```

A new leaf-error variant must be routed in the matching `*_exit_code` classifier;
an un-classified fallback must not be (re)introduced — the mandatory code is the
forcing function that keeps the map below correct.

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
| `SchoolError::NoSpecifier/NotInitialized/NoSchool` | `Unavailable`                  |
| `BackendError::Unknown`             | `Unavailable` (borderline, see above)         |
| `BackendError::Unresolvable/KindMismatch` | `Usage`                                 |
| `SetupError::NotInGitRepo`          | `Unavailable`                                 |
| `SetupError::AlreadySetUp` / `InitError::AlreadyExists` | `Usage`                    |
| `GitError::*`                       | `Operational`                                 |
| `PrepareError::Clone/Write`         | `Operational`                                 |
| `AddImport::NoSkills/SkillNotFound` | `Usage`                                       |
| `AddImport::Clone` / `PullImports::Git` | `Operational`                             |
| `PullImports::InvalidDecl`          | `Usage`                                       |
| `*::RejectedImports` (inadmissible skills) | `Operational`                          |
| `SkillError::Discovery`             | `Operational`                                 |
| `IoError::Cancelled`                | `Cancelled`                                   |

\* Traversal is a fail-closed rejection of untrusted config content — the user
authored a bad path, so `Usage`.

The scheme is deliberately small: enough for a script to branch on, not a
per-error catalogue that ossifies into a compatibility burden. `mcp check` stays
informational at exit `0`; a strict mode would be a separate CLI surface change.
