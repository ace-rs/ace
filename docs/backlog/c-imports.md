# C — Skill imports & supply chain

Source: [Outline][source], revision 20.
Status reconciled against repository records at `9df624a` on 2026-09-05.

[source]: https://outline.prodigy9.co/doc/c-skill-imports-supply-chain-AMblsRDACh

Importing skills from external repos + keeping that path safe.

- [x] **247** path traversal via `[[imports]].source` in `ensure_source_cache` · *High,
      security* — shipped `086dbd3`; containment is structural, not a denylist:
      `cache_path` splits the path itself and rewrites every segment (`src/git.rs:281`),
      so a segment cannot carry a separator and `.`/`..` cannot climb. Tests at
      `src/git.rs:468`
- [x] **243** `ace import` merges into existing `[[imports]] skills=` instead of appending
      a duplicate block · *High*
- [ ] **187** `ace school pull` silently shadows skills when a `*` import collides
      with explicit decls
- [ ] **121** parallelize import-source fetches in `ace school pull`
- [ ] **selective-pull** `ace school pull` picks which imports to pull. Today it takes no
      arguments and pulls every `[[imports]]` source. Ruled surface: bare
      `ace school pull` presents a multi-select of the declared sources; `--all` pulls
      everywhere. Uses the existing `Io::prompt_multiselect` primitive
      (`docs/decisions/2026-07-22-batch-selection-prompts.md`).
- [ ] **66** document wildcard imports + parent-school pattern on the website
- [x] **selection-UX** multi-select TUI picker (⊇242 + 253) — tag `tui-multiselect` ·
      shipped 2026-07-22 across mcp register / re-register / import; primitive is
      `Io::prompt_multiselect`, ruling in
      `docs/decisions/2026-07-22-batch-selection-prompts.md`

## Ideas / later

* **226** supply-chain safety checks (static scan + LLM audit)
* **155** rethink skill-import propagation across nested schools
* **67** explore git-based skill import instead of file copy
* **70** handle deleted upstream skills on school update
* 🆕 school import provenance — track which skill came from where, so the importer knows
  ownership (hit a case where an agent disowned a skill it authored, due to `*` imports)
* 🆕 **rethink the import model — no skill copy in the school** *(think later, hard)*.
  Since there are no lockfiles and provenance isn't tracked anyway, the school may not
  need any copied skill content at all (or a different structure) — resolve imports only
  at `ace pull` time, letting the user pick overrides then. Could delete a swath of
  copy-handling code. Needs a long, careful design pass before any move. Relates to the
  provenance idea above and to Epic D (sync generalisation).

## Shipped

65, 75, 76, 62.

## Provenance research

The school import provenance idea owns
[school-side skill ownership research](../scratch/2026-08-25-school-skill-provenance.md).
It is a proposal, not a ruled schema: table naming, adoption, prune, stale-reason storage,
and partial-update policy remain open. **70**, **155**, and **187** are related
requirements, not duplicate ownership-map implementations. The no-copy import-model
alternative also remains undecided.
