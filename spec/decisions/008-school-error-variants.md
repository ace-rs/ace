# Decision: Split `SchoolError::Missing` into Cause-Specific Variants

Status: **accepted**.

Baseline: ACE v0.5.0.

## Problem

`SchoolError::Missing` ("no school configured, run `ace setup`") was raised for two
distinct failure modes:

1. **Project-repo, never set up** — no `school` field in `ace.toml`. The hint
   "run `ace setup`" is correct.
2. **Resolved school root has no `school.toml`** — the workdir IS a school being
   authored but `ace school init` hasn't been run yet, OR a remote clone landed
   without `school.toml`. The hint "run `ace setup`" is *wrong*; the user (or
   the school maintainer) needs `ace school init`.

User-visible symptom: after `ace setup .` in a workdir without `school.toml`, a
bare `ace` run failed with "no school configured, run `ace setup`" — even though
setup had just succeeded. The message was actively misleading.

## Decision

Split `Missing` into two cause-specific variants. Names describe the on-disk
state so the remediation reads off the variant:

- **`SchoolError::NoSpecifier`** — `ace.toml` has no `school = ...` key. Message:
  `no school configured, run \`ace setup\``. Fires from `Ace::require_school()`
  when `tree.specifier()` returns `None`. Project-repo callers (`cmd/main.rs`,
  `cmd/pull.rs`, `cmd/diff.rs`) propagate this when they need the specifier.

- **`SchoolError::NotInitialized`** — specifier resolved successfully, but the
  resolved root *exists as a directory* and contains no `school.toml`. Message:
  `school not initialized, run \`ace school init\``. Fires from
  `Ace::require_school()` only.

The `is_dir()` guard in `require_school()` is essential: when the resolved root
is *absent* (case 8 in `spec/school/overview.md`), `require_school` returns `Ok`
so `cmd/pull.rs` can self-heal by cloning. `NotInitialized` fires only when the
directory is present but uninitialized.

Both variants are unit variants. A single fixed message suffices for each
cause; the matrix in `spec/school/overview.md` covers both
"local-pre-init" (case 5) and "remote-clone-without-school.toml" (case 7) under
the same remediation.

## Call-site audit

| Site                       | Before  | After          |
|----------------------------|---------|----------------|
| `src/ace/mod.rs` (no specifier) | Missing | NoSpecifier   |
| `src/ace/mod.rs` (root has no school.toml) | n/a (silent Ok) | NotInitialized |
| `src/cmd/main.rs:30`       | Missing | NoSpecifier    |
| `src/cmd/main.rs:39`       | Missing | NoSpecifier    |
| `src/cmd/pull.rs:18`       | Missing | NoSpecifier    |
| `src/cmd/diff.rs:11`       | Missing | NoSpecifier    |

Project-repo callers all use `NoSpecifier` because they're unwrapping
`Option<specifier>`. None of them need to distinguish "no specifier" from "root
uninitialized" — `require_school()` does that detection internally and surfaces
`NotInitialized` at the right moment.

## Alternatives considered

- **Keep one variant, branch the message at format-time.** Variants exist to
  carry cause; threading the cause through `Display` reinvents what the type
  system gives us. Rejected.
- **`NotInitialized { root: PathBuf }` payload + adaptive message** (e.g.
  "school.toml not found at <root>; check `school = ...` in ace.toml" for the
  bad-typo case). Speculative — no current call site needs the bad-path
  distinction. Defer until a real caller demands it.
- **Auto-create `school.toml` when `ace setup .` runs in an empty workdir.**
  That's the "local school bootstrap" feature flagged as undesigned in
  `setup.md`. Out of scope.
- **Names `Unconfigured` / `Uninitialized`** — symmetric and verb-form. Lost to
  `NoSpecifier` / `NotInitialized` because the latter describes the literal
  on-disk state, which is easier to reason about when reading call sites.

## Backcompat

`SchoolError` variants are internal — no public CLI surface depends on the
specific variant name. Error messages are user-facing and have changed only
where the previous text was wrong (case 5 / case 7 paths).
