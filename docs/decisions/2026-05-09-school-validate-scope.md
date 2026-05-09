# Decision: `ace school validate` v1 scope (2026-05-09)

Status: **decided** — placeholder typo-detection only; no shape validation,
no auto-hooks into `pull` / `setup`.

## Problem

`docs/decisions/2026-05-09-backend-cmd-templating.md` shipped templated
`{{ ... }}` placeholders in `[[backends]].cmd[]` and `env` values, with
unknown placeholders rendering to empty strings. Typos surface as missing
path segments at exec time — the failure is silent at config-write time
and confusing at exec time.

The follow-up was a `ace school validate` command. The obvious framing
("validate everything in `school.toml`") expands the surface enormously
and overlaps with what serde already covers.

## Decision

V1 ships **placeholder typo detection only**, scoped to
`[[backends]].cmd[]` and `[[backends]].env` values. Output:

```
backends[<name>].cmd[<i>]: unknown placeholder '<name>'[, did you mean '<sug>'?]
backends[<name>].env[<key>]: unknown placeholder '<name>'[, did you mean '<sug>'?]
```

Exit non-zero when any issue is reported. Did-you-mean uses Levenshtein
distance ≤ 2 against the closed set
`{school_dir, project_dir, home, backend_dir}`.

The closed set lives once, as `BackendVars::NAMES` next to the renderer's
substitution map (`BackendVars::into_map`) in `src/backend/registry.rs`.
Adding a placeholder forces both updates at one site — validator and
renderer cannot drift.

## Out of scope (v1)

- **Shape validation of `school.toml`.** Required-field and type checks
  are already enforced by serde at load time. Re-implementing them in a
  validate pass duplicates work and ages poorly as the schema evolves.
- **Hooking validate into `ace school pull` / `ace setup`.** Auto-running
  would couple the new command to two existing flows before users have
  exercised the manual form. Defer until v1 sees real use; the manual
  command stands alone.
- **Validating `[[imports]]`, `[[mcp]]`, `[[roles]]`.** Different domains,
  different placeholder/shape concerns. Separate slices when they're
  needed; no demand yet.
- **Imported-school `[[backends]]` merge concerns.** Tracked as a
  separate spec/decision; today only the active `school.toml`'s
  `[[backends]]` are merged (`resolver::merge::backend_decls`). Validate
  v1 walks only the active school's decls — same scope as the renderer.

## Why a closed-set check (not "render and compare")

We could detect typos by rendering each template and looking for empty
substrings, but that catches only typos that map to empty values, not
the general case (`{{ Schol_Dir }}` vs `{{ school_dir }}`). Comparing
parsed placeholder names against the typed catalogue catches every
typo regardless of context.

## References

- `src/templates/mod.rs` — `check`, `UnknownPlaceholder`.
- `src/backend/registry.rs` — `BackendVars` (typed catalogue + values).
- `src/actions/school/validate.rs` — walk + print.
- `docs/decisions/2026-05-09-backend-cmd-templating.md` — original
  placeholder set + "validation deferred" non-goal.
- `docs/spec/school/school-commands.md` — `ace school validate` user
  docs.
