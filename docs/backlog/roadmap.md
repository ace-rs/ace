# Roadmap

Source: [Outline][source], revision 31.
Status reconciled against repository records at `9df624a` on 2026-09-05.

[source]: https://outline.prodigy9.co/doc/roadmap-2qndeh3bho

A priority guide, not a commitment. Work spans epics; releases ship independently.
Task status and acceptance details live in the owning epic, so this page carries no
duplicate checkboxes. Repository specifications govern product behavior.

## Now — managed connected sessions and hardening

Current implementation: **start-pipeline** and **native-session-supervision** in
[M](m-sessions.md) are complete through `9df624a`; controlled components are designed,
not currently shipped.

- [M — sessions](m-sessions.md): **runtime-endpoints** brings endpoints, controlled
  components, readiness, and primary handles together; then **component-supervision**
  and **mux-runtime** add cohort lifecycle, inspection, and attachment.
- [M — connect](m-sessions.md): **connect-core**, **connect-codex**,
  **connect-opencode**, and **connect-claude** follow the managed runtime.
- [B — MCP](b-mcp.md): **64**, stdio MCP declarations, remains recorded as High but
  is gated by the current remote-only decision.
- [A — backends](a-backends.md): **146**, scope-aware selector validation, and **147**,
  authentication conflict, need status verification before implementation.

The local ask ledger's next-step derivation maps to M's **runtime-endpoints**; it does
not authorize the implementation. The historical component-foundation and graph slices
were superseded by `9df624a`.

## Next — workspace composition and medium follow-ups

- [M](m-sessions.md): **workspace-manifest**, **workspace-expansion**, **workspace-mux**,
  and **bare-workspace-entry**.
- [C](c-imports.md): **187**, wildcard/explicit import collisions; **66**, wildcard
  imports and parent-school documentation (J references the same task).
- [B](b-mcp.md): **mcp-check-execution**, resolved backend execution and visible output.
- [I](i-quality.md): **152**, changed-skill reporting.
- [F](f-school-lifecycle.md): **124 (⊇195)**, CLI dependencies and recommendations,
  plus **123**, environment health and non-rejecting skill diagnostics.

## Later — architectural work

- [M](m-sessions.md): **ace-mutation-surface**, **advanced-session-lifecycle**, and
  **external-launch-hooks**, only after concrete demand.
- [G](g-entrypoints.md): **245**, headless serving through M's runtime boundary.
- [D](d-resource-sync.md): resource-sync generalisation, gated on a superseding ruling.
- [H](h-cli.md): **prompt-override** and dependent **config expand**; storage unresolved.
- [E](e-selection.md): **120**, per-repo skill selection and token usage.
- [F](f-school-lifecycle.md): **69**, school switching, and **43**, ejection.
- [C](c-imports.md): **226**, supply-chain checks, and the linked provenance/no-copy
  research; neither competing import model is approved.

## Icebox and unscheduled records

- Hermes support and backend capability research: [A](a-backends.md).
- Research spikes: [K](k-research.md); big bets: [L](l-big-bets.md).
- Original low-priority references: **214**, **13**, **126**, **134**, **155**, **67**,
  **70**, **33**, **161**, **246**, **156**, **199**, **127**, in their owning epics.
- **74**, Windows verification, remains hardware-gated; the supported target contract
  is [platforms](../spec/platforms.md), not an open-ended Windows support promise.
- **skills-json-investigation**: investigate the `skills.json` format, owned by
  [K](k-research.md).
- Local records without a priority ruling: [I](i-quality.md)'s audit residuals and build
  measurements; [J](j-docs.md)'s catalog, instruction delivery, and table alignment;
  [school](school.md)'s pantry reproductions.

**190** (`--yes`) is complete in H; **227** (`ace template`) is killed in
[cancelled](cancelled.md). Neither remains in the icebox.
