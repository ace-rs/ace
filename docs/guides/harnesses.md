# Backend harness pen

Real backend CLIs, built from source into `.harnesses/` — a gitignored, disposable
directory inside this repo. Use it when a question can only be answered by running the
actual binary: reading a CLI surface, checking that a backend consumes what ACE emits,
or filling a gap in `docs/vendor/`.

The pen is not part of `cargo test`. Integration tests use Flaude and nothing else —
`docs/spec/testing.md` §Backend Testing Strategy.

## Using it

```sh
./scripts/harnesses.sh            # status of every known backend
./scripts/harnesses.sh codex      # clone-or-pull, build, wrap as .harnesses/bin/codex
./scripts/harnesses.sh clean      # remove the pen
```

Each backend is provisioned on demand — naming one clones and builds only that one.
Builds run under `nice -n 19`; `codex` and `opencode` are heavy, so expect minutes.

Run a provisioned binary through `.harnesses/bin/`, never through whatever is on `$PATH`:

```sh
.harnesses/bin/codex --version
```

That distinction is the point. A global install drifts with your machine and updates
itself when it feels like it — see `../vendor/backend-install.md` — while the pen is a
shallow clone of upstream `main` that changes only when you re-provision. Delete it
whenever it gets stale and provision again.

## Keeping the writes inside

A backend run against your real home defeats the pen: one `hermes --version` was enough
to create `~/.hermes`. So `.harnesses/bin/<name>` is a generated wrapper, not a symlink —
it points the XDG variables and the backend's own state variable at the clone before
exec'ing the binary. Invoking through the wrapper is the contained path; invoking the
built binary directly is not.

State goes in the clone rather than a directory of its own because a backend handed an
empty state dir tries to install itself into it, and the clone is already a populated
install. Nothing in the pen survives a `clean`, so a dirtied checkout costs nothing.

This is soft containment. A backend that hardcodes `~` or shells out to something that
does will still reach the host; the wrapper covers the well-behaved majority and nothing
more. If you find state landing outside the pen, add the variable that governs it to the
table in `scripts/harnesses.sh` rather than working around it at the callsite.

## Registry

| backend    | source           | build         | binary                           |
|------------|------------------|---------------|----------------------------------|
| `codex`    | `openai/codex`   | `cargo build` | `codex-rs/target/release/codex`  |
| `opencode` | `sst/opencode`   | `bun install` | `packages/opencode/bin/opencode` |

`opencode` requires [bun](https://bun.sh/). Both rows are transcribed from their
repositories and neither has been provisioned here — if a build command or binary path is
wrong, fix the table in `scripts/harnesses.sh` and this one together.

`claude` has no public source and is absent by design. `hermes` was provisioned here and
then removed: a `uv tool install` copy on `$PATH` serves the same purpose without a
per-run build (`../vendor/backend-install.md`). `flaude` is ours and lives in
`src/backend/flaude.rs`.
