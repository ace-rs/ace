# Decision: Action Layout by User Role (2026-04-22)

Status: **decided** — actions grouped by user role (consumer vs maintainer), not by
mutation subject.

## Problem

The action module tree had drifted through several axes:

1. Initially a flat list in `src/state/actions/*.rs` with verb-first names
   (`InitSchool`, `UpdateSchool`, `ImportSkill`, etc.).
2. Grouped into submodules by mutation subject: `school/` for actions touching the
   school clone, `imports/` for actions touching the `[[imports]]` config, `mcp/`
   for MCP registration, `project/` for project-dir mutations.
3. Actions lived under `src/state/actions/` — four levels deep — even though
   actions aren't part of State (the data layer); they operate *on* it.

The subject-based grouping produced counterintuitive placements:

- `school::Pull` was invoked by `ace pull` (a consumer running in their own repo)
  but lived under `school::` because it mutates the school clone directory.
- `school::Clone` and `school::Link` were similarly consumer-side workflow
  operations classified by what directory they write to.
- `imports::Refresh` was a school-maintainer operation (`ace school update` inside
  a school repo) buried under a config-section name.

## Decision

Two changes:

### 1. Promote `actions/` to `src/actions/`

Actions aren't part of State — they're operations *on* State and the filesystem.
Treating `actions` as a peer module to `state`, `config`, `cmd`, etc. drops
nesting from 4 levels to 3 and makes the layering honest: State is pure data,
Actions consume it alongside other callers.

### 2. Group actions by user role, not mutation subject

Two distinct user roles drive ACE invocations:

- **Consumer** — a developer working in their own repository that uses a school
  as a dependency. Runs `ace setup`, `ace pull`, bare `ace`. Never edits school
  internals.
- **Maintainer** — a developer working inside a school repository, authoring or
  curating skills. Runs `ace school init`, `ace school pull` (new), `ace import`.

This axis matches the CLI structure (`ace <verb>` for consumer, `ace school
<verb>` for maintainer) and the invariants each role carries (consumer has an
`ace.toml` + project dir; maintainer has a `school.toml` + skills dir).

Final layout:

```
src/
├── actions/
│   ├── project/          consumer-side — user is in their repo
│   │   ├── setup
│   │   ├── update_gitignore
│   │   ├── prepare       (orchestrator)
│   │   ├── clone         (was school::Clone)
│   │   ├── link          (was school::Link)
│   │   ├── pull          (was school::Pull)
│   │   ├── register_mcp  (was mcp::Register)
│   │   └── remove_mcp    (was mcp::Remove)
│   └── school/           maintainer-side — user is in a school repo
│       ├── init
│       ├── add_import    (was imports::Add)
│       └── pull_imports  (was imports::Refresh)
└── state/
    ├── school.rs
    ├── skill_set.rs
    └── discover.rs       (was actions/school/discover.rs)
```

### 3. "Pull" verb symmetry

Both `project::Pull` and `school::PullImports` are pull-shaped: fetch upstream,
update local. The old `Refresh` name for the maintainer-side operation hid this
symmetry. Renaming unifies the vocabulary:

- `project::Pull` — consumer pulls their school clone from its git origin.
- `school::PullImports` — maintainer pulls imported skills from upstream sources
  into the school's `skills/` dir.

Scope (`project::` vs `school::`) distinguishes which side is pulling; verb
(`Pull`) describes the shape of the operation.

### 4. `ace school pull` CLI alias

Matches the `PullImports` struct name and the verb symmetry above. `ace school
update` remains as a backcompat alias — we have real users; breaking existing
command invocations is not acceptable without a major version.

### 5. `discover` belongs in `state/`

`discover_skills` is a pure read that produces `DiscoveredSkill` structs. The
output is consumed by `SkillSet::from_discovered` in `state/skill_set.rs`.
Co-locating the producer and the primary consumer is clearer than isolating
`discover` under `actions/` where it doesn't fit the "action = mutation" pattern
of its neighbors.

### 6. Flat within small scopes

`school/` has 3 actions, `project/` has 8. Neither is large enough to benefit
from sub-submodules like `school/imports/`. Keep one level of nesting under
`actions/` and let file names disambiguate (`add_import.rs`, `pull_imports.rs`).

## Options Rejected

### Group by mutation subject (previous design)

Produced the counterintuitive placements described above. The subject a function
writes to is an implementation detail; the role of the user invoking it is what
defines the CLI command tree and the environmental invariants.

### Keep `actions/` under `state/`

Preserves the original (4-level) nesting for no gain. State is data; actions are
behavior. Layering them as peers is cleaner and call sites shorten.

### Separate `imports/` and `mcp/` submodules

With 2–3 actions each, the submodule name adds a level of nesting without
earning it. Flat with descriptive file names (`add_import`, `register_mcp`)
reads as well and keeps the tree shallow.

## Backcompat Implications

- `ace school update` must continue to work. Clap alias on the `Pull` subcommand
  or a separate `Update` variant that dispatches to the same handler.
- No config-file renames; `school.toml` and `ace.toml` field names are
  unchanged by this refactor.
- Error-type renames (`RefreshError` → `PullImportsError`, etc.) are internal —
  no public API.
