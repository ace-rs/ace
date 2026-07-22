# Build & test speedup backlog

Not spec/decision because: it's a menu of unmeasured options, not a ruling on which
to take.

Captured from session memory before repo move. Ranked menu of remaining wins
after the 2026-04-22 build-all.sh rework (commits 7253dd2, 2d89be1: zig 0.14
pin, multi-target groups, sccache opt-in).

**Test-side items are settled** — the 2026-05-27 audit shipped the wins and closed the
rest; see [prior-art](prior-art.md) § Test-suite speedup audit. What survives below is
the **build** menu.

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

## Tests — closed

All test-side items are resolved. `[profile.test] debug = "line-tables-only"` landed
2026-05-27 (cold `cargo test --no-run` 21.45s → 17.14s); the fixture-sharing work landed
the same day (warm `cargo test` 9.4s → 6.8s); `cargo nextest` was **ruled out** by
chakrit; `cargo test --release` was skipped (ACE's integration tests exec `ace` as a
subprocess, so binary mode matters more than harness mode). Details and the
deliberately-unpursued list: [prior-art](prior-art.md) § Test-suite speedup audit.

## Next up

`[profile.release] strip = "symbols"` — still absent from `Cargo.toml`, so this is the
one unclaimed low-effort win. Optionally spend 30 min checking whether `mold` can be
forced through zigbuild.
