# I — Quality — testing & internals

Source: [Outline][source], revision 4.
Status reconciled against repository records at `9df624a` on 2026-09-05.

[source]: https://outline.prodigy9.co/doc/i-quality-testing-internals-GDgCsAPMPB

- [ ] **152** `ace pull` misreports tier folder name as the changed skill
- [ ] **36** simplify flaude: print diagnostics to stdout instead of a JSONL file
- [x] **150** hide the test-only backend from release CLI surfaces — `177252e` closes
      PROD9-150 using `cfg(debug_assertions)`, not a feature flag.

## Ideas / later

* **56 (+15)** design + fill live backend integration-test coverage (one test-strategy
  item)
* **154** reconsider per-binding error variant naming (Config → TreeLoad?)
* **241** surface discovery structural prunes in read-only paths (`ace skills` /
  skill_count)

## Shipped

37, 131.

## Local records

- [ ] **link-removal-preview** implement the intended preview of project link removals in
      [sync](../spec/skills/sync.md#reconciliation); distinct from school import pruning.

- [ ] **historical-audit-residuals** · agent:inferred, needs revalidation. Reconcile the
      remaining findings in [the June audit](../scratch/2026-06-10-codebase-audit.md)
      before selecting fixes; the report's old paths and “mostly open” label are not
      current verification. See
      [reconciliation](reconciliation.md#historical-audit-coverage) for grouping and known
      closures.
- [ ] **build-speedups** · agent:inferred, unmeasured options.
      [Build research](../scratch/2026-05-09-build-test-speedups.md) owns the measurements
      and candidate rationale: release stripping/optimization, dependency
      timings/features, portable registry caching, and Linux linker feasibility. No
      release profile exists at reconciliation time; benchmark each option separately
      without raising budgets. Test-side speedups and rejected alternatives remain closed.
