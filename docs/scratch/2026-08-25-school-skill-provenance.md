<!-- not spec/decision because: provenance semantics are proposed,
but no storage or deletion policy has been ruled -->

# School-side skill provenance

Research for the ACE maintainer deciding how imported skills retain source ownership,
surface update conflicts, and track upstream deletions without versioning Markdown.

## Conclusion

ACE should persist one school-side ownership map from skill identity to import source.
The map records **who materialized a path**, not which revision or bytes were imported.
It contains no commit, tag, content hash, timestamp, or Markdown fingerprint.

`[[imports]]` remains desired selection: which sources and match handles a school wants.
The ownership map records materialized state: which source last successfully wrote each
skill identity. Reconciliation compares those two sets with the current school tree and
the latest upstream discoveries.

That is sufficient to distinguish:

- a normal refresh from the same source;
- a newly selected skill;
- an upstream deletion;
- a path ACE never imported;
- two sources claiming one identity;
- an explicit source handoff;
- a school author deleting an already-stale skill.

It deliberately cannot detect arbitrary edits inside an owned skill. That is correct:
the existing philosophy says wildcard imports always track latest and overwrite. Content
comparison would create the file-version model ACE has rejected.

## Current system

### Desired provenance exists only during resolution

`school.toml` already declares import sources and selectors:

```toml
[[imports]]
source = "ace-rs/school"
skills = ["*"]
```

The imports resolver attaches source and declaration index to every selected skill. It
uses that ephemeral provenance for first-declared collision resolution, warnings, and
future explanation. The type model prevents an import-resolved skill from losing its
source before `copy_into`.

This answers “where would this skill come from now?” while the upstream cache is
available. It does not answer “who wrote the directory already in the school?” after the
process exits.

### Materialization destroys the evidence

`PullImports` resolves the latest selected set, attaches each winning source to a
discovered skill, validates it, then passes only identity names to `copy_into`.
`copy_into` replaces every selected destination directory. The source is discarded at
the persistence boundary.

The resulting `<school>/skills/<identity>/` tree has no machine-readable connection to
the declaration that produced it. On a later pull, ACE cannot distinguish an old import
from a locally authored skill at the same path.

### Deletion is therefore impossible to reconcile safely

The current spec makes writes additive or overwriting and forbids deletion. If a source
removes a selected skill, the old directory remains forever. That protects downstream
work from upstream destruction, but ACE cannot even state whether the directory is stale
or locally owned.

The rule is broader than the preservation requirement needs. “Never silently propagate
an upstream deletion” does not require “forget which source created the path.” The latter
is an information loss caused by the current copy boundary.

### Existing collision provenance is not durable ownership

Resolver provenance and materialization provenance answer different questions:

- Resolver provenance: which declaration wins for the latest upstream sets?
- Materialization provenance: which source owns the existing school path?

Declaration indexes are useful diagnostics and bad durable identities. Reordering
`[[imports]]` changes every later index without changing any source. Durable ownership
must use a canonical source identity, never list position.

## Non-goals

This design does not add:

- source commit or tag pins;
- imported revision history;
- content hashes or checksums;
- three-way Markdown merges;
- compatibility versions;
- per-project lockfiles;
- automatic upstream deletion;
- provenance in downstream project links;
- ownership metadata inside `SKILL.md` frontmatter.

Schools remain Git repositories, so ordinary school commits record content history.
ACE does not create a second history for imported files.

## Proposed model

### Desired state

`[[imports]]` continues to own source declarations, selectors, exclusions, and tier
flags. It is author intent and remains hand-editable.

### Materialized state

Add one generated map to `school.toml`:

```toml
[skill_provenance]
"ace-school" = "ace-rs/school"
"rust-coding" = "ace-rs/school"
"typescript/coding" = "company/engineering-skills"
```

Each key is the canonical skill identity path. Each value is the canonical import source
that last completed the write. The map is school-side because the authored school owns
the copied files and commits both files and provenance together.

The exact table name remains open. `skill_provenance` says what the data means;
`imported_skills` says where it came from. Avoid `lock`, `version`, `revision`, and
`manifest`: none describes the source-ownership relation precisely.

### Why `school.toml`

The ownership map belongs beside `[[imports]]` rather than in a new file:

- both sides of reconciliation load and save as one config unit;
- an import write can update content and ownership in one operation;
- moving a school preserves its provenance without hidden state;
- Git review shows selection and ownership together;
- no second parser, migration surface, or file lifecycle exists;
- the repository already treats `school.toml` as the school authoring record.

This does make part of `school.toml` generated. ACE already canonicalizes the file on
authoring writes, so generated materialized state is not a new writer class.

### Why identity maps directly to source

The relation is one skill identity to at most one owner. A source-to-list shape duplicates
identity membership across arrays and makes collisions representable. A keyed identity
map makes duplicate ownership structurally impossible.

Source identity should use the canonical clone identity already produced when parsing
import sources. Raw spelling is presentation, not identity: `owner/repo` and its HTTPS
URL must not appear to be different owners if ACE resolves both to the same repository.
If the current source parser does not expose a stable canonical identity, that is the
abstraction to add; do not normalize strings again inside provenance code.

## Reconciliation

For each pull, derive four inputs:

```text
desired     latest import resolution: identity -> winning source
owned       school.toml skill_provenance: identity -> prior source
present     skill identities currently present under school/skills
upstream    discovered identities per source at latest main
```

Then classify every identity before any filesystem mutation.

| Prior owner | Desired owner | Path exists | Classification          | Action |
|-------------|---------------|-------------|-------------------------|--------|
| none        | source A      | no          | new import              | copy; record A |
| none        | source A      | yes         | unmanaged collision     | block identity |
| source A    | source A      | yes         | normal refresh          | overwrite latest |
| source A    | source A      | no          | manually removed        | restore latest |
| source A    | none          | yes         | stale owned skill       | retain; report |
| source A    | none          | no          | acknowledged removal    | drop provenance |
| source A    | source B      | yes/no      | ownership handoff       | require transfer |

The classifier returns a complete plan before writes begin. A conflict in one identity
does not justify a partial hidden decision. Healthy independent identities may still
update if the command reports the blocked set and exits non-zero, matching the current
rejected-import behavior.

### Normal refresh

When desired and prior owners match, ACE overwrites the directory with latest upstream
content exactly as it does today. It does not compare bytes or ask whether Markdown was
edited. This preserves the versioning philosophy rather than weakening it under a new
name.

### Unmanaged collision

When a desired import targets an existing path with no ownership record, ACE cannot know
whether the path is locally authored, imported by an older ACE, or copied by hand. It
must not claim or overwrite the path during an ordinary pull.

The diagnostic names the identity, source, and existing path, then directs the author to
one explicit adoption operation. Adoption may be part of `ace import` or a dedicated
command, but it must be a visibly mutating verb because it transfers ownership to ACE.

### Upstream deletion and deselection

An owned identity absent from desired state is stale. Two causes share the same safe
default:

- the source no longer exposes the selected identity;
- the school declaration no longer selects it.

ACE retains the directory and provenance, reports the reason when it can distinguish
one, and includes it in a prune preview. It never silently removes the directory.

Keeping provenance on a retained stale path matters: dropping it would turn the next
pull into an unmanaged collision and lose the answer to “where did this come from?”

If the school author deletes the stale path themselves, the next pull removes the orphan
provenance entry. The absent path is the explicit destructive action; cleanup of its
metadata is lossless.

### Explicit prune

Deletion needs its own user verb. A suitable surface is:

```text
ace school prune                 # preview stale owned skills
ace school prune <identity>      # remove one stale owned skill
```

The exact CLI is open, but ordinary `pull` must not perform the deletion. Prune accepts
only identities currently classified as stale and owned. It refuses unmanaged paths and
currently desired imports.

After deleting the skill directory, prune removes the provenance entry and saves
`school.toml`. Git remains the recovery and review surface.

### Ownership handoff

Changing declaration order, selectors, or exclusions can make a different source win an
identity. An ordinary pull must not silently transfer ownership: the two sources may
carry unrelated skills with the same path.

The conflict names old and proposed sources. An explicit `ace import <new-source>
--skill <identity>` is a natural transfer verb because direct import already means
“install this source at this identity.” On success it overwrites the directory and
changes exactly one ownership entry.

No content similarity heuristic should infer rename or handoff. Same bytes do not prove
same ownership; different bytes are normal on every refresh.

## Transaction boundary

Skill directories and `school.toml` must change as one logical unit. Today `copy_into`
may update several directories before a later error. Provenance makes partial persistence
observable, so the mutation boundary must become explicit.

The implementation should:

1. resolve and classify the full reconciliation plan;
2. reject unsafe identities before touching their paths;
3. stage healthy directory replacements;
4. apply each replacement;
5. update provenance only for successful replacements;
6. save `school.toml` after the filesystem result is known;
7. report any partial operating-system failure loudly with the exact applied set.

A fully atomic multi-directory filesystem transaction is unavailable. The correct design
is an explicit unit of work with recoverable, truthful state—not a claim of atomicity.
Because provenance updates last, an interrupted run fails safe: a newly written but
unrecorded path becomes an unmanaged collision rather than being silently claimed.

## Migration

Existing schools have imported directories and no ownership map. Automatic inference
from current `[[imports]]` would misclassify locally authored paths, especially where a
wildcard now happens to match them.

Migration must therefore be conservative:

- an empty or absent map is valid;
- ordinary pull blocks desired identities whose paths already exist unowned;
- a preview shows the ownership ACE could adopt from current declarations;
- the author explicitly adopts selected identities or all unambiguous candidates;
- adoption records source ownership without changing content;
- later pulls regain normal overwrite behavior for adopted identities.

This creates one deliberate migration ceremony at the school boundary and no ceremony
for consuming projects.

An alternative migration that overwrites and claims every currently selected path is
consistent with the old pull behavior but destroys the very ownership distinction this
feature exists to create. Reject it.

## User-facing explanations

`ace school skills` should show source ownership for materialized skills:

```text
ace-school          ace-rs/school
rust-coding         ace-rs/school
typescript/coding   company/engineering-skills
local-review        local
```

`local` means present with no provenance entry; it is a display classification, not a
stored fake source.

A future `ace school explain <identity>` can combine:

- current materialized owner;
- latest desired winner;
- matched and excluded declarations;
- upstream presence or deletion;
- stale, unmanaged, or handoff classification.

This fulfills the existing selection spec's deferred explanation surface with durable
materialization evidence.

## Placement in the type model

Discovery provenance on `Skill<Discovered>` remains ephemeral and source-specific.
Persisted ownership is a separate school-domain type, not another typestate marker.

Suggested concepts:

```text
SkillOwnership       identity + canonical source
OwnershipMap         identity -> canonical source
ReconciliationPlan  changes + conflicts + stale identities
ReconciliationChange
  Add | Refresh | Restore | ForgetMissing | Transfer
ReconciliationConflict
  UnmanagedPath | OwnershipHandoff
```

`PullImports` should consume a plan produced from resolved imports, ownership, and disk
inventory. It should not grow conditionals around `copy_into`; the classifier is pure,
and a school-side mutation action owns application of the plan.

The persisted source type must be the same source identity type used by import cache
resolution. String comparison at the action layer would recreate source normalization
policy in the wrong module.

## Spec impact if ruled

The decision would revise these current contracts:

- `docs/spec/skills/emit.md`: replace “no manifest, never deletes, intentionally dumb”
  with ownership-aware reconciliation and explicit prune.
- `docs/spec/skills/selection.md`: distinguish resolution provenance from materialized
  ownership and extend school explanation.
- `docs/spec/school/school-toml.md`: add the ownership-map schema and normalization.
- `docs/spec/school/school-commands.md`: define pull conflicts, adoption, prune, and
  direct-import ownership transfer.
- `docs/spec/skills/lifecycle.md`: name persisted school ownership as separate from
  in-process typestate provenance.
- `docs/spec/overview.md`: state plainly that ownership metadata is not file versioning
  or pinning.

## Tracker-ready task

### Title

Track school-side skill ownership for safe import reconciliation

### Problem

ACE knows import source provenance only while resolving a pull. Once a skill is copied
into an authored school, its source is lost. ACE therefore cannot distinguish stale
upstream deletions, unmanaged local paths, or source ownership handoffs and must retain
every old import forever.

### Outcome

Persist identity→source ownership in `school.toml`; reconcile latest import resolution,
prior ownership, and current disk state; keep same-source refreshes overwrite-latest;
block unmanaged collisions and implicit handoffs; report stale skills; require an
explicit prune verb for deletion.

### Guardrail

Store no revision, tag, commit, hash, timestamp, Markdown fingerprint, or compatibility
version. Provenance answers ownership only. Git remains the sole content-history surface.

### Acceptance shape

- A successful import or pull records each materialized identity's canonical source.
- Same-owner pulls overwrite latest without content comparison.
- Existing unowned destinations are not claimed or overwritten implicitly.
- Upstream deletions and deselections retain files and remain attributable.
- Explicit removal deletes only stale owned skills and clears their ownership entries.
- Source changes require an explicit ownership transfer.
- Existing schools migrate by explicit adoption, never inferred claiming.
- Project-side linking and Markdown version philosophy remain unchanged.

## Open rulings

1. Name the generated table: `skill_provenance`, `imported_skills`, or another exact
   ownership term.
2. Choose the explicit migration/adoption command surface.
3. Choose the explicit prune command surface and whether it supports an interactive
   multi-select.
4. Decide whether declaration removal and upstream deletion use distinct stale reasons
   in persistent state or are derived afresh on each pull.
5. Decide whether healthy identities update when sibling identities conflict, or whether
   any conflict blocks the entire pull unit.
