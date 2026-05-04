# Decision: Split `SchoolError::Missing` into Two Variants (proposed)

Status: **proposed** — not yet implemented in code.

Baseline: ACE v0.5.0.

## Problem

`SchoolError::Missing` ("no school configured, run `ace setup`") is currently raised for two
distinct failure modes:

1. **Project-repo, never set up** — no `school` field in `ace.toml`. The hint "run `ace setup`"
   is correct.
2. **School-repo, never initialized** — workdir IS a school being authored, but no
   `school.toml` exists yet. The hint "run `ace setup`" is *wrong* — the user needs
   `ace school init`.

The same conflation also fires when a project-repo specifier resolves to a directory that
lacks `school.toml` (e.g. a typo in `school = "../foo"`), where the right hint is neither.

User-visible symptom: after `ace setup .` in a workdir without `school.toml`, the bare `ace`
run fails with "no school configured, run `ace setup`" — but setup just succeeded. The
message is actively misleading.

## Decision

Split into two variants:

- **`SchoolError::Missing`** — no specifier in any `ace.toml` layer. Hint: `run \`ace setup\``.
  Fires from `cmd/main.rs:30`, `cmd/pull.rs:18`, and `Ace::require_school()` when both
  `school.toml` is absent and `tree.specifier()` returns `None`.

- **`SchoolError::NotInitialized { root: PathBuf }`** — specifier was resolvable, but the
  resolved root has no `school.toml`. Hint depends on root:
  - If `root == project_dir` (workdir is meant to be the school) → `run \`ace school init\``.
  - Otherwise (specifier points at a bad path) → `school.toml not found at <root>; check the
    \`school = ...\` value in ace.toml`.

  Fires from `cmd/main.rs:39` and any other site that requires `tree.school` to be `Some`.

The two-message split inside `NotInitialized` is encoded by inspecting `root` at format-time,
not by adding a third variant. Keeps the error surface small.

## Call-site audit

| Site                       | Today    | After      |
|----------------------------|----------|------------|
| `cmd/main.rs:30`           | Missing  | Missing    |
| `cmd/main.rs:39`           | Missing  | NotInitialized |
| `cmd/pull.rs:18`           | Missing  | Missing    |
| `cmd/diff.rs:11`           | Missing  | NotInitialized (clone_path None on embedded — diff is meaningless) |
| `ace/mod.rs:155`           | Missing  | Missing    |

`cmd/diff.rs` may warrant a separate `Embedded` variant if more sites need to distinguish
"this operation requires a remote clone." Defer until a second call site appears.

## Alternatives considered

- **Keep one variant, change the message at format-time.** Requires plumbing the failure
  reason through `Display`, which is exactly what variants are for. Rejected.
- **Auto-create `school.toml` when `ace setup .` runs in an empty workdir.** That's the
  "local school bootstrap" feature flagged as undesigned in `setup.md`. Out of scope here.
