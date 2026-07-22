# Test suite audit — speedup opportunities

Captured 2026-05-27 after `[profile.test] debug = "line-tables-only"` landed
(commit on top of 8d3e375). Baseline at audit time:

- Cold `cargo test --no-run` after clean: 17.14s
- Warm `cargo test` harness time: ~9.4s wall, 729 passed in 21 suites
- 20 integration test binaries under `tests/` (~4.5k LOC)
- Slowest tests dominated by `school_init_test.rs` (7×~1.18s) and the
  `setup_remote_school`-backed suites (`pull/import/update/diff/link/network`)

User constraint (2026-05-27): full e2e coverage is not required for every
test. In-process action-struct tests are the preferred default; subprocess
`ace` spawns must be justified per case.

## Per-file classification

`spawn` = number of `ace()`/`cmd()` invocations.
`remote` = number of `setup_remote_school`/`setup_tiered_origin` calls.
`class` = recommended target shape.

| file                 | tests | spawn | remote | class                          |
|----------------------|------:|------:|-------:|--------------------------------|
| config_test.rs       |    44 |    45 |      0 | in-process (config parser)     |
| import_test.rs       |    24 |    27 |     10 | mixed: in-process + shared fix |
| exec_test.rs         |    16 |    16 |      0 | in-process (action dispatch)   |
| mcp_test.rs          |    14 |    14 |      0 | in-process                     |
| setup_test.rs        |    13 |    11 |      0 | in-process                     |
| learn_test.rs        |    12 |    12 |      0 | in-process                     |
| skills_test.rs       |    11 |    19 |      0 | in-process                     |
| school_init_test.rs  |     9 |     9 |      0 | in-process (canary)            |
| update_test.rs       |     9 |     9 |     10 | mixed: in-process + shared fix |
| school_update_test.rs|     7 |     7 |      0 | in-process                     |
| startup_test.rs      |     7 |     1 |      0 | keep e2e (tiny smoke layer)    |
| upgrade_test.rs      |     7 |     7 |      0 | in-process                     |
| pull_test.rs         |     6 |     7 |      5 | mixed: in-process + shared fix |
| explain_test.rs      |     4 |     6 |      0 | in-process                     |
| fmt_test.rs          |     4 |     4 |      0 | in-process                     |
| link_test.rs         |     4 |     7 |      3 | in-process + shared fix        |
| paths_test.rs        |     4 |     4 |      0 | keep e2e (CLI wiring smoke)    |
| yolo_test.rs         |     4 |     4 |      0 | in-process                     |
| diff_test.rs         |     3 |     3 |      2 | in-process + shared fix        |
| network_test.rs      |     2 |     3 |      1 | keep e2e (true network path)   |

Totals: 204 tests in `tests/`, 215 spawns, 31 remote-fixture setups. Of those,
~190 tests are in-process candidates; ~14 stay e2e as the smoke layer
(`startup`, `paths`, `network`).

The remaining `cargo test` count of 729 lives in `#[cfg(test)]` modules in
`src/`. Those are already cheap and out of scope here.

## Hotspot deep-dive: school_init_test.rs

All 9 tests follow the same shape:

```rust
let env = TestEnv::new();
env.git_init();
env.ace().args([...]).assert().success(); // or .failure()
env.assert_exists(...);
```

- 2 failure-path tests error before `PullImports` runs — fast (~0.05s each).
- 7 success-path tests reach `Init::run` → `PullImports { school_root }.run()`,
  which goes through `ensure_source_cache_in` and clones `ace-rs/school`
  (real network or warm cache). Hence the ~1.18s per-test cost.

In-process conversion needs **`PullImports` source isolation**. Three viable
shapes; choose at Slice B:

1. **Pre-seeded cache + insteadOf redirect.** Reuse the pattern from
   `setup_remote_school`: stand up a bare origin at a known path, redirect
   `https://github.com/ace-rs/school.git` via `.gitconfig` in the tempdir,
   pre-clone into the cache path. Tests then call `Init { ... }.run(&mut ace)`
   in-process; pull resolves to the fake origin.

2. **Inject a no-op imports resolver.** Add a test-only seam: e.g.
   `Init::with_resolver(Box<dyn ImportResolver>)`, defaulting to the real
   `PullImports`. Tests pass a stub that no-ops. Avoids any git/network work.

3. **`ace.toml` opt-out.** Add a `[test] skip_imports = true` or env var the
   `Init` action honors. Cheapest but adds production surface for test-only
   reasons — least preferred.

**Recommendation**: try option 1 first because it exercises the same cache
machinery the rest of the suite already uses (so we can share the fixture
helper later in Slice F). Fall back to option 2 only if option 1 doesn't
sufficiently drop the per-test cost.

**Expected payoff**: 7 × ~1.18s ≈ 8.3s → estimated 7 × <0.1s ≈ <1s for the
suite. Largest single win in the audit.

## setup_remote_school call-site survey

31 invocations across 6 files. The helper performs **6 git subprocesses per
call** (init bare, clone, add, commit, push, clone-to-cache). At ~50–100ms
per subprocess that's ~300–600ms of fixed cost per call, before any test
logic runs.

**Sharing strategy (Slice F)**: build the bare origin + populated cache once
per integration binary via `OnceLock<PathBuf>`, store it under a
process-wide tmpdir, then `cp -r` (or hardlink) the directory contents into
each test's `TestEnv` tempdir. Tests gain an isolated copy; setup cost drops
to one-time per binary.

Constraints to verify before sharing:

- Tests must not mutate the shared origin/cache template. Inspect every
  `setup_remote_school` caller; flag any that push commits back to origin or
  rewrite the cache. Suspect callers: `pull_test.rs` (`fetches_new_changes`,
  `diverged_warns`), `update_test.rs` (anything testing the fetch path).
- The `.gitconfig` `insteadOf` block writes the **origin's actual path** into
  the redirect. Sharing the origin path across tests is fine; sharing the
  gitconfig file is not (each test's gitconfig points at its own copy).
- The shared template must be built lazily and torn down at process exit
  (test binaries don't guarantee a clean shutdown — accept the tempdir
  may leak on panic).

**Expected payoff**: ~31 sites × ~400ms saved ≈ 12s of sequential work, but
since tests run in parallel within a binary, wall-clock savings will be less
— rough estimate ~3–5s off warm `cargo test` after the in-process moves
land.

## canonicalize() audit

`TestEnv::new()` eagerly canonicalizes the tempdir root because
`src/actions/project/link_skills.rs` canonicalizes school/data roots when
classifying symlinks (line 465–466, 480, 526). Tests that read
`ace paths`-style output or assert against `read_link` results compare
against the canonicalized form, so dropping the eager canonicalize would
break path equality assertions on macOS (`/var` vs `/private/var`).

`canonicalize()` on a fresh tempdir is a single syscall — sub-millisecond.
Across 200+ tests it adds up to maybe 100ms wall, not a real hotspot.

**Drop this from Phase 2.** Not worth the risk of breaking symlink-equality
tests for negligible wall-clock gain. Slice A is reassigned (see below).

## Integration-binary consolidation map (Slice G)

Once in-process moves land, most of `tests/*.rs` will be thin and not need
their own binary. Proposed regrouping:

| target binary               | source files                                       |
|-----------------------------|----------------------------------------------------|
| `tests/config.rs`           | config_test, paths_test (paths is CLI-smoke)       |
| `tests/skills.rs`           | skills_test, link_test, learn_test, import_test    |
| `tests/school.rs`           | school_init_test, school_update_test               |
| `tests/project.rs`          | setup_test, update_test, pull_test, upgrade_test   |
| `tests/cmd.rs`              | exec_test, fmt_test, explain_test, mcp_test,       |
|                             | diff_test, yolo_test                               |
| `tests/e2e.rs` (subprocess) | startup_test, network_test, plus 1–2 smoke tests   |
|                             | lifted from each above grouping                    |

6 binaries instead of 20. Cuts ~14 link rounds off cold build. Order this
last in Phase 2 — until the in-process moves land the file split still
maps to "what gets exercised by this suite" and is useful for narrow
`cargo test --test <name>` runs.

## Phase 2 progress

Landed 2026-05-27 (commits dc85d18, 8dafe6f):

- **Shared `ace-rs/school` import-cache template.** Process-wide OnceLock
  template seeded into each `school_init` test's XDG_CACHE_HOME. ace's
  PullImports finds the cache locally; no real clone. school_init suite
  ~10s → ~1.5s wall (6×).
- **Shared `setup_remote_school` per-specifier template.** Origin+cache
  built once per binary per specifier; per-test cost is a pair of
  `cp -R`s + one `git remote set-url`. Heavy fixture suites
  (pull/update/import/diff/link) modestly faster.
- **Local-redirect for clone-failure tests.** `redirect_to_invalid()`
  helper points `https://github.com/<source>.git` at a local nonexistent
  path so `git clone` fails in ~20ms instead of network-timeout. Applied
  to 8 tests across `import_test`/`school_update_test`.

**Warm `cargo test` wall-clock**: 9.4s → 6.8s (~28%). 729 tests pass.

## Open slices not pursued

Re-evaluated mid-phase. The remaining ideas need bigger refactors or
hit diminishing returns versus the user's "minimal and elegant"
constraint:

- **In-process action-struct tests** (`Init`, `Config`, etc.). Requires
  threading paths through `Ace` instead of reading `XDG_*` env, since
  parallel in-process tests share global env. Substantial production
  refactor — not done here.
- **Per-binary parallelism.** Cargo runs integration binaries serially.
  `cargo-nextest` would parallelize but the user ruled it out.
- **Binary consolidation** (20 → ~6 binaries). Mostly worth doing
  alongside an in-process conversion; not on its own.
- **Lazy `canonicalize()` in `TestEnv::new()`.** Dropped at audit time —
  symlink-equality tests on macOS depend on it; cost is sub-millisecond.

## Floor

Remaining slow tests (0.5–0.8s) are doing legitimate multi-step ace
flows. The structural floor is ~50–150ms per `ace` subprocess invocation
× ~200 invocations across the suite, scheduled in parallel within each
binary and serially across binaries. Further wins require either an
in-process path or fewer subprocess hops.

## Out of scope (re-stated)

- Reviving e2e for the converted tests — `network_test`, `startup_test`,
  `paths_test`, plus a deliberate smoke layer in the consolidated `e2e.rs`
  is the full e2e surface going forward.
- cargo-nextest, `[profile.release]` changes, mold linker — separate menu
  items in `2026-05-09-build-test-speedups.md`.
- New test framework crate (`rstest`, `test-case`, etc.). Not justified.
