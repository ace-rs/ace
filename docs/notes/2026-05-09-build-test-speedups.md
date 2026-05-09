# Build & test speedup backlog

Captured from session memory before repo move. Ranked menu of remaining wins
after the 2026-04-22 build-all.sh rework (commits 7253dd2, 2d89be1: zig 0.14
pin, multi-target groups, sccache opt-in).

Baseline:
- `build-all.sh` clean: ~1m 55s for all 7 targets
- `build-all.sh` no-op: ~3.3s

Pick from this menu later without re-deriving. Don't bundle into one PR — each
is independently measurable.

## Builds

### Tier 1 — low effort, real impact

1. **Add `[profile.release]` to `Cargo.toml`** — currently absent, all 7
   targets build with stock defaults:
   - `strip = "symbols"` — smaller binaries, slightly faster link
   - Try `opt-level = 2` and measure — for a CLI like ACE, ~20% compile
     speedup with no measurable runtime hit
   - Add `[profile.release-fast]` inheriting from release with
     `lto = false, opt-level = 2` for iteration
2. **`cargo build --timings`** on a clean build to identify dep bottlenecks.
   Common offenders: `serde_derive`, `clap_derive`, `regex`. ACE already
   disables default features on `ureq`/`inquire`/`indicatif`/`gif`. Worth
   auditing `console`'s default features.
3. **CI registry caching** if/when build-all moves to CI: `actions/cache` on
   `~/.cargo/registry` and `~/.cargo/git`.

### Tier 2 — more effort, more payoff

4. **`mold` linker for linux targets** — 5×–10× link time cuts. zigbuild uses
   zig as linker, so needs `--linker=mold` flag or env override; may not work.
   ~30 min to check feasibility before committing.
5. ~~Per-target `CARGO_TARGET_DIR`~~ — defeats artifact sharing. Skip.

### Tier 3 — not worth it

- `cranelift` codegen — nightly only.
- Drop musl targets — would lose static linux binaries.

## Tests

### Tier 1

1. **`cargo nextest`** — drop-in for `cargo test`, ~2-3× faster, cleaner
   output, surfaces ordering flakes more clearly. Worth adopting just for the
   output, particularly given the existing `git::tests::ls_remote_tags_local_repo`
   ordering flake.
2. **Add `[profile.test]`** to `Cargo.toml`:
   ```toml
   [profile.test]
   debug = "line-tables-only"
   ```
   Real link-time savings on integration test binaries (each is its own crate).

### Tier 2

3. **Audit `tests/common/` setup cost** — integration tests do `git init`,
   write fixture school repos via `setup_remote_school`. If measurable, share
   fixtures via lazy_static. Risks test coupling — only if measured as
   bottleneck.

### Skip

- `cargo test --release` — slower compile, only worth it for hot-path runtime
  tests. ACE integration tests exec ace as subprocess; binary mode matters
  more than harness mode.

## Recommended single PR

`[profile.release] strip = "symbols"` + `[profile.test] debug = "line-tables-only"`
+ document `cargo nextest` as recommended runner. Optional: spend 30 min
checking `mold` via zigbuild. Estimated combined: 15–25% off clean build time
+ faster test cycles.
