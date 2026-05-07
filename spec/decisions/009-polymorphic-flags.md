# Decision: Polymorphic Backend Flags via Intent Split (2026-05-07)

Status: **decided** — two transport methods, one argv builder per backend.

## Problem

ACE-internal consumers need a programmatic backend entrypoint that returns captured
stdout/stderr. The existing `Backend::exec_session(SessionOpts)` exec-replaces the current
process — fine for `ace` (the user's terminal hands off to claude/codex), useless for
`ace learn` and any other consumer that wants to parse the backend's reply.

The pre-split `SessionOpts.one_shot_prompt: Option<String>` smuggled a non-interactive prompt
through the interactive transport: `exec_session` would translate it to `claude -p <text>` or
`codex exec <text>` and exec-replace anyway. That worked for `ace -p "..."` from the CLI but
exposed no programmatic API — there was no way to capture the response.

PROD9-151 (`ace learn`) blocks on this. It spawns the backend with a structured prompt and
reads two diffs out of the response.

## Decision

Split the backend contract into two transport methods backed by a shared per-backend argv
builder:

- `exec_session(SessionRequest) -> io::Error` — exec-replace, never returns on success.
  Carries interactive fields: trust, session_prompt, resume, env, project_dir, extra_args.
- `exec_one_shot(OneShotRequest) -> io::Result<std::process::Output>` — spawn subprocess,
  capture stdout/stderr, return the raw `Output`. Carries: prompt source (`PromptInput`),
  env, project_dir, extra_args. No resume/trust/session_prompt — non-interactive doesn't
  need approval modes or system-prompt injection.

`PromptInput` is `Inline(String)` (passed as argv) or `Stdin` (child inherits parent stdin).

`SessionOpts.one_shot_prompt` is removed. The user-facing `ace -p "..."` path routes through
`exec_one_shot` instead.

## Why two methods, not one

Considered `Backend::exec(Intent) -> ?`. Rejected: return types differ fundamentally
(never-returns vs `Output`). A unified signature lies about semantics, forces callers to
match on intent to know what to do with the return value, and complicates the trait. Two
methods make the contract explicit at the type level.

The polymorphic core is the **argv builder**, not the transport. Each backend has two
private argv functions (`build_session_args`, `build_one_shot_args`); the two transport
methods are thin shells around them.

An earlier sketch threaded an `Intent { Session(&SessionRequest), OneShot(&OneShotRequest) }`
enum through a single per-backend `build_args(Intent)` dispatcher. Dropped — the dispatcher
just matched and called one of the two argv functions, and the enum existed only to be
matched on at the only callsite. Two free functions per backend communicates the same
"argv-per-intent" structure without the indirection.

## Why no dedicated error type

`exec_one_shot` returns `io::Result<std::process::Output>`. Spawn failures land in
`io::Error`; non-zero exits are visible in `Output.status` with `stderr` already captured.
Callers decide whether non-zero is fatal.

A `OneShotError { Spawn, NonZeroExit { stdout, stderr, status } }` enum was considered.
Rejected as YAGNI: callers can already inspect the fields they need on `Output`. If a
common pattern emerges, wrap it in a helper later.

## Output buffering for `ace -p`

The pre-split `ace -p` path streamed claude/codex stdout in real time via exec-replace.
The new `exec_one_shot` captures-then-prints — output buffers until the child exits. For
short prompts this is invisible; for long ones the user sees nothing until completion.

Accepted as a known regression. The `ace learn` use case requires capture; preserving
streaming for the user-facing CLI while also exposing capture for consumers would require a
third transport method (spawn + tee stdout) and isn't worth the surface area today. Revisit
if user feedback flags it.

## Per-backend argv mapping

See `spec/backend.md` for the full table. Summary:

| Intent          | claude                          | codex             |
|-----------------|---------------------------------|-------------------|
| Session         | `--system-prompt <prompt>` etc. | `-c developer_instructions=<prompt>` etc. |
| OneShot Inline  | `-p <text>`                     | `exec <text>`     |
| OneShot Stdin   | `-p` + piped stdin              | `exec -` + piped stdin |

Codex stdin syntax (`exec -`) verified against
`github.com/openai/codex/codex-rs/exec/src/cli.rs` — the `-` sentinel forces stdin reading.

## Out of scope

- `--model`, `--resume`, `--system-prompt` polymorphic flags — separate Linear issues. The
  abstraction supports them, but each needs its own design call (cross-backend model name
  normalization, resume semantics under capture).
- `BackendError::UnsupportedOperation` — both built-ins support both intents. Add when a
  backend genuinely can't.
- `ace learn` itself — PROD9-151. This decision unblocks it.
