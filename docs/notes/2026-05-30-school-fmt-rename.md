# `ace school fix` → `ace school fmt` — Scope and Change List

2026-05-30 — research only, no code written.

## Background

`ace school fix` was spec'd as a `school.toml` canonical re-serializer (singular→plural
`skill`→`skills`, drop deprecated shapes). Never implemented — phantom command.

`ace fmt` already round-trips both `ace.toml` and `school.toml` through `load()`→`save()`,
and `school_toml::load()` already calls `ImportDecl::normalize()` (singular→plural fold).
So `ace fmt` from a school root already accomplishes the `ace school fix` design.

## Design question: does `ace school fmt` add anything over `ace fmt`?

Three options:

- **(a) Pure alias** — `ace school fmt` just calls `fmt::run(ace)`. Simple, but formats
  `ace.toml` too if present (fine in a school repo that has `ace.toml` with
  `school = "."`).
- **(b) School-scoped only** — `ace school fmt` only touches `school.toml` (and possibly
  `skills/*/SKILL.md` frontmatter). Stricter semantics, avoids surprising `ace.toml`
  rewrites.
- **(c) School-scoped + extras** — `school.toml` round-trip **plus** skill-directory
  normalization (sorting, renaming to canonical identity paths). Genuinely distinct from
  `ace fmt`, justifies its existence as a separate subcommand.

**Decision pending.**

## Changes needed

### Code (`src/`)

1. **`src/cmd/school.rs`** — Add `Fmt` variant to `school::Command` enum. Wire it in
   the `run()` match arm.
2. **`src/cmd/fmt.rs`** — Depending on the design decision:
   - (a) No change — `school fmt` delegates to `fmt::run(ace)`.
   - (b/c) Extract school-specific formatting into a separate fn (or new action) that
     only touches `school.toml` and/or skill-directory normalization.

### Specs (`docs/`)

3. **`docs/spec/school/school-commands.md` L227–236** — Rename `## ace school fix`
   section to `## ace school fmt`. Update description. Note equivalence to `ace fmt`
   (or describe the extra scope if (b/c)).
4. **`docs/spec/school/school-toml.md` L193–194** — Change `ace school fix` references
   to `ace school fmt`.
5. **`docs/notes/2026-05-30-school-instructions-catalog.md` L109, L127** — Update the
   phantom-command finding (remove `ace school fix` from phantom list; note it was
   renamed and implemented).
6. **`docs/spec/school/overview.md`** — Add `fmt` to the subcommand inventory if it
   lists them.
7. **`docs/spec/school/standard-imports.md`** — Scan and update any `fix` references.

### Templates

8. **`src/templates/builtins/tpl_school_instructions.md`** — Add `ace school fmt` to the
   command inventory if it lists available commands.

### Tests

9. **New test** — `ace school fmt` round-trips a `school.toml` with singular `skill`
   keys and produces canonical plural output. Normalization already exists in
   `school_toml::load`/`save`; this proves the subcommand works.

### Backcompat

No backcompat concern — `ace school fix` was never implemented. No `visible_alias`
needed. Clean addition.

## Pointers

- Existing `ace fmt` implementation: `src/cmd/fmt.rs`
- Singular→plural normalization: `src/config/school_toml.rs` `ImportDecl::normalize()`
- School subcommand enum: `src/cmd/school.rs`
- Spec for phantom `ace school fix`: `docs/spec/school/school-commands.md` L227
