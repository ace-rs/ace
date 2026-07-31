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
./scripts/harnesses.sh hermes     # clone-or-pull, build, link .harnesses/bin/hermes
./scripts/harnesses.sh clean      # remove the pen
```

Each backend is provisioned on demand — naming one clones and builds only that one.
Builds run under `nice -n 19`; `codex` and `opencode` are heavy, so expect minutes.

Run a provisioned binary through the link, never through whatever is on `$PATH`:

```sh
.harnesses/bin/hermes --version
```

That distinction is the point. A global install drifts with your machine; the pen is a
fresh shallow clone of upstream `main`, so what you observe is what upstream ships today.
Delete it whenever it gets stale and provision again.

## Registry

| backend    | source                        | build         | binary                              |
|------------|-------------------------------|---------------|-------------------------------------|
| `hermes`   | `NousResearch/hermes-agent`   | `uv sync`     | `.venv/bin/hermes`                  |
| `codex`    | `openai/codex`                | `cargo build` | `codex-rs/target/release/codex`     |
| `opencode` | `sst/opencode`                | `bun install` | `packages/opencode/bin/opencode`    |

`hermes` requires [uv](https://docs.astral.sh/uv/); `opencode` requires
[bun](https://bun.sh/). The `codex` and `opencode` rows are transcribed from their
repositories and have not been provisioned here yet — if a build command or binary path
is wrong, fix the table in `scripts/harnesses.sh` and this one together.

`claude` has no public source and is absent by design; install it the normal way.
`flaude` is ours and lives in `src/backend/flaude.rs`.
