# Backlog source reconciliation

Evidence cutoff: repository `9df624a`; Outline ACE collection read on 2026-09-05.
This record is for the maintainer or a fresh agent checking why a task has its status.
The epic files own current status; this page records source coverage and discrepancies.

## Source coverage

All 17 documents in the Outline ACE collection were read. The 13 epics, roadmap,
cancelled record, and School record have local counterparts linked from the
[index](README.md); source URLs and revision numbers are on each page. The collection
comment query returned no comments, and each document reported zero comments.

The remaining document, [Skill discovery cascade][discovery-source], is a reference,
not a task list. Its behavior already belongs in
[the discovery spec](../spec/skills/model.md#discovery-cascade); its linked historical
decision filenames no longer exist. Its unreachable sibling-root layout example and
vendoring guidance remain available at the source. No new behavior or task is inferred
from that reference.

[discovery-source]: https://outline.prodigy9.co/doc/skill-discovery-cascade-rFnzX3zXmd

Local sources inspected: `.ace/save.md`, `.ace/save.ledger.md`, `CLAUDE.md` (also exposed
through the `AGENTS.md` symlink), all Markdown scratch notes, specification follow-ups,
and source TODO/FIXME markers. No separate current ask ledger, roadmap, or task file was
found beyond those records. Deleted historical roadmaps remain in Git; their replacement
is represented by Outline, so old queues are not revived as new work.

## Status reconciliation

- **Managed startup (M):** Outline marks component foundations and controlled backend
  graphs complete through `c75aeb4`. The local trail repeats that shape and records
  supervision at `65dc1bf`. Newer `9df624a` removes the unused component abstractions and
  controlled launch materialization, leaving one supervised `SessionProcess`. Therefore
  native supervision is complete; endpoints, components, readiness, handles, and cohort
  ownership belong to the next coherent boundary. The old “component supervision first”
  roadmap ordering is superseded by the current session spec's implementation sequence.
- **Controlled startup ask:** the ledger's open `agent:inferred` item maps to M's
  **runtime-endpoints** and **component-supervision**, not a second task or approval.
- **Claude transport:** the ledger's deferred `user:verbatim` item is preserved in M,
  including the exact quote. It does not block the monitor adapter or settle MCP design.
- **Table alignment:** the ledger's presented `agent:inferred` item belongs to J as
  **markdown-table-alignment**. The eight paths were not recorded; identifying them is
  still required before claiming a scoped fix.
- **Pantry defects:** both reproduction commands from the local trail belong to
  [school records](school.md), with owner uncertainty preserved. No other checkout was
  inspected or changed.
- **Pull relinking (F):** Outline asks whether pull should relink. Commit `e8cd872`
  explicitly rejects relinking and implements an `ace link` hint; the question is closed.
- **Test backend visibility (I/150):** Outline lists it open; `177252e` explicitly closes
  PROD9-150 using debug-assertion gating. Keep the actual implementation choice.
- **Polymorphic invocation (G/159):** `4185348` implements one-shot transport under this
  issue number. The remaining normalization scope needs revalidation; G's source claim
  that none of its work was committed is too broad.
- **190 and 227:** the roadmap icebox conflicts with H and the cancelled record. Global
  `--yes` is complete at `dcb5c2e`; `ace template` is killed, not deferred. Both leave the
  live roadmap while their records remain searchable.

Other Outline completion claims retain their source status; this reconciliation is not
a fresh acceptance audit of every historical feature.

## Duplicate ownership

- **66** has one checklist in C; J and the roadmap point to it.
- **126** belongs to H; L references it.
- **149** belongs to A; K references it.
- **237** and **34** belong to B; K references them.
- **start-mode**, the always-on bridge, and auto-pause point to M's corresponding work.
  Remote/tunnel ideas stay superseded by SSH and tmux attachment.
- **mcp-check-execution**, backend config mutation, and startup wordmarks already match
  B, A, and H respectively; local trail mentions do not create new tasks.
- Doctor **123** includes the skill-frontmatter diagnostics follow-up from
  `docs/spec/skills/model.md`. Required dependencies and recommendations stay under
  **124 (⊇195)**; the boundary with MCP checking is still undecided.
- J's **200** owns the capability-catalog research; H's **13** remains deferred until
  its proposed supersession is decided. Instruction delivery is a related research item.
- C's provenance idea owns the August ownership-map proposal. **70**, **155**, **187**,
  and the no-copy alternative remain related requirements or competing designs.

## Unresolved discrepancies and gates

- **146 / 147:** the roadmap lists them as Now, but A has no detailed entries and the
  local ledger supplies no closure. A preserves them as needing status verification.
- **Prompt override:** H records an earlier file-based proposal at
  `~/.config/ace/prompts/session.md` and an `ace.toml` alternative. The latest roadmap
  supplies no choice. H owns the unresolved storage design.
- **Cursor:** A suggests revisiting it, but cancelled **9** supersedes the older survey.
  Keep the idea provisional; no decision to revive the cancelled task is evidenced.
- **MCP stdio / Dockerfile:** B's **64** priority and **237** idea do not supersede the
  remote-only MCP ruling. Both remain gated.
- **Resource sync:** D remains blocked on an explicit scope decision; the old
  `project_skill_scope` handle is historical provenance, not an existing decision file.
- **Windows 74:** remains hardware-gated. The local trail's missing cross-compiler is
  an incomplete verification, not a failure of the declared platform contract.
- **Source TODO:** `src/config/ace_toml.rs` still proposes roles/description. The June
  audit calls it stale and L records roles as removed; it belongs to audit cleanup,
  not a revived roles implementation.

## Historical audit coverage

[The June audit](../scratch/2026-06-10-codebase-audit.md) retains every original finding
and its evidence. I's **historical-audit-residuals** owns revalidation; its findings are
not silently asserted to remain bugs after later refactors. The residual groups are:

- Import-copy symlinks; backend MCP positional arguments; visible MCP destinations;
  upgrade integrity; header secrets in argv. Traversal **247** is already closed.
- Skill discovery/prune propagation and repeated walks; `School` versus `SchoolToml`;
  standalone-module dependency direction; config/backend imports; capability bits;
  layering checks. Learn-specific `skill_count` work is superseded by learn removal.
- Nested school listing, internal-skill CLI surface, upgrade exit classes and network
  deadlines, embedded-school diff diagnostics, and documentation gaps.
- Duplicate MCP listing, bounded frontmatter reads, typed prepare/MCP errors, malformed
  school config diagnostics, backend launch duplication, config explanation duplication,
  prompt gating, test helper duplication, and competing frontmatter parsers.
- The audit's low-level leftovers: diff-error handling, token docs, dead-code allowances,
  readiness helpers, basename helpers, override folding, rejection naming, Codex config
  boilerplate, iterator idioms, unused action parameters, and one-shot exit structure.

Known closures: the audit's July spec sweep is recorded complete; learn and Droid items
are superseded; `CmdError` extraction is complete at `ebe4f1d`; newer `9df624a` addresses
startup abstraction scope. The old list cannot reinstate those tasks. Other overlaps,
including I's **241** discovery diagnostics, remain under one audit revalidation owner.

## Other local research

- [Build speedups](../scratch/2026-05-09-build-test-speedups.md): I owns the surviving
  unmeasured build options. Test improvements are complete; nextest, separate target
  directories, nightly codegen, and dropping static targets remain rejected/unpursued.
  The old provider-specific cache recipe is replaced by portable caching as an option.
- [Capability catalog](../scratch/2026-05-30-school-instructions-catalog.md): J/200 and
  H/13 own the open choice; the phantom school-fix command is already closed. Its old
  command/config inventory is evidence to recheck, not current product documentation.
- [School provenance](../scratch/2026-08-25-school-skill-provenance.md): C owns all five
  open rulings and the proposed acceptance shape; no ownership schema is approved.
- [Skill rearchitecture](../scratch/2026-06-02-skill-model-rearchitect.md) and the
  [prior-art digest](../scratch/prior-art.md): the recorded rearchitecture, test work,
  and school proposals are complete. Old hosted/runtime ideas map to G/M, macros to G,
  and content-scope ideas to D; historical sketches do not become new commitments.
- `docs/spec/skills/sync.md`'s intended link-removal preview belongs to I's
  **link-removal-preview**; it is distinct from C's proposed school import pruning.
- The skill lifecycle HTML is an explanatory artifact linked to the completed model,
  not a task source. Third-party vendor manuals are reference material, not ACE queues.
