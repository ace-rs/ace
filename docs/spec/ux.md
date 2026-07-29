# UX Philosophy

The principles that govern how `ace` talks to its user. Listed in order of
importance — when two principles pull in opposite directions, the earlier one
wins.

## 1. The backend is the product; `ace` is the doorway

When the user types `ace`, they want to be coding with their agent — not
configuring `ace`, not reading `ace` output, not learning `ace` syntax.
Every command should optimize for getting them through the door and out of
the user's way.

This shapes several defaults:

- Bare `ace` launches a session — no subcommand required for the common case.
- Unknown arguments forward to the backend rather than erroring.
- Setup, MCP registration, skill provisioning happen in service of the
  session, not as ends in themselves.
- `ace`'s own surface stays small and deliberate — every flag and subcommand
  is friction the user has to learn.

`ace` succeeds when the user forgets it is there.

## 2. `ace` is debuggable

`ace` does a lot under the hood: resolves config across four layers, picks
backends, manages symlinks, registers MCPs, mutates caches. The user trusts
that not because `ace` does little, but because every effect is inspectable
without reading source.

- `ace paths` lists every directory `ace` knows about.
- `ace config show` displays the resolved view with source layer per value
  (user / project / local / school / override).
- `ace diff` shows what `ace` would change before changing it.
- Diagnostics name the responsible layer or file when something is wrong.

Adding a new `ace` capability without a corresponding read surface is
incomplete work. A feature the user can't inspect is a trust hole.

## 3. Errors come with hints when recovery is possible

A recoverable failure must emit a `hint` alongside the `error`. The error
states what is wrong; the hint states what to do about it.

```
✗ no school configured for this project
  → run `ace setup` to choose a school
```

Unrecoverable errors (panics, internal invariant violations) do not get
hints — there is nothing the user can do. Conversely, a hint without an
error is fine: a successful operation may still hint at next steps.

The hint is not optional flair. A bare recoverable error is a bug.

## 4. Don't bug the user about decisions they already made

If the user declined an offer or chose an option once, `ace` does not ask
again on the next invocation. The product of N consecutive runs should not
include N copies of the same yes/no.

This applies to: opt-in offers, discoverability nudges, upgrade prompts, MCP-registration suggestions, and
any other "want me to also…" surface.

Concretely:

- A decline is a decision and must persist across runs.
- The persistence mechanism (ace.toml field vs. `ace.local.toml` vs. a
  `.ace/` state file) is a separate design question.
- A state change *can* re-open a previously declined prompt. If the user
  declines "register this MCP" and then the school adds a new MCP entry,
  re-asking is fine. The rule guards against asking about *the same thing*,
  not against asking about new things.

## 5. Borrow vocabulary from the agents being wrapped

`ace` wraps Claude Code, Codex, OpenCode — each with established CLI
conventions. New `ace` flags and subcommands borrow names from those
conventions rather than inventing ace-specific terms. The user types what
their agent-CLI muscle memory expects.

- A flag with a standard name in the wrapped backends keeps that name in
  `ace` (`--model`, `--continue`, `--resume`, etc.).
- New verbs get checked against the agent ecosystem before being introduced.
- When backends disagree, follow Claude Code (the primary backend) and
  document the divergence.

The wrapper should feel like an extension of the agent, not a separate tool
with its own dialect.

## 6. Channels have meanings, not just shapes

Four output primitives, each with a defined semantic role:

| Primitive | Meaning                                              |
| --------- | ---------------------------------------------------- |
| `info`    | Status, progress milestones, neutral information.    |
| `warn`    | Something is off but the operation continued.        |
| `error`   | Something failed; the operation did not complete.    |
| `hint`    | How to fix or proceed. Pairs with `warn` or `error`. |

Channel choice is about *what kind of message this is*, not about how
attention-grabbing it should look. Decoration follows from channel; channel
does not follow from desired loudness.

## 7. Re-running is safe and informative

Every command must be safe to re-run. If the desired state already holds,
the command reports that — it does not error, does not no-op silently, and
does not redo work that has no effect.

- `ace setup` on an already-configured project says so and exits 0.
- `ace pull` with nothing to pull says so and exits 0.
- `ace mcp register` for an already-registered server says so and exits 0.

Idempotency is a property of the *result*, not the *implementation*. The
command may still touch disk to verify state; what matters is that the
user can re-run it without fear and learn something from the output.

## 8. Presentation and interaction are independent

How output *looks* and whether ACE may *ask* are separate questions, decided
by separate inputs. Nothing derives one from the other.

| question | decided by |
|--------------------|--------------------------------------------------|
| Should it colorize? | a terminal is attached, and `--porcelain` is off |
| Should it emit?     | the run has someone to report to |
| May it ask?         | a terminal is attached, and neither `--yes` nor a set `CI` variable waived the question |

Consequences worth stating outright:

- `--porcelain` selects machine-readable output. It does **not** suppress
  prompts — a script that must not be asked passes `--yes`.
- `--yes` waives being asked. It does **not** downgrade output — a run with a
  terminal still gets colors and spinners.
- A set `CI` (or `CONTINUOUS_INTEGRATION`) variable implies `--yes`. Nobody is
  watching an unattended run, so nothing may block on an answer.

When ACE may not ask, each prompt resolves by what it can defend:

- **Checklists** take their default, all or none. Every option is visible in
  the declaration, so the default is a real answer.
- **Free-form and single-choice prompts** fail. There is no defensible answer
  to invent, and inventing one puts words in the user's mouth.

A refusal names the cause the caller can act on: the waiver when one exists
(`--yes` or CI), otherwise the missing terminal.
