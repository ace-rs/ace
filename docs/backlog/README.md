# ACE backlog

The repository is the authoritative task tracker for ACE. Start with the
[roadmap](roadmap.md) for ordering, then read the owning epic for scope and status.
Epics and priority bands guide selection; they do not authorize execution or releases.
Product behavior belongs in [specifications](../spec/README.md), and rulings belong in
[decisions](../decisions/README.md).

## Task owners

- [A — Backends](a-backends.md): implementations, configuration, capability research.
- [B — MCP](b-mcp.md): provisioning, checks, and MCP scope gates.
- [C — Imports](c-imports.md): selection, collisions, ownership, and supply chain.
- [D — Resource sync](d-resource-sync.md): expansion beyond skills; decision-gated.
- [E — Selection](e-selection.md): skill filtering and prompt-injection ideas.
- [F — School lifecycle](f-school-lifecycle.md): setup, switching, ejection, doctor.
- [G — Entrypoints](g-entrypoints.md): headless invocation and input automation.
- [H — CLI](h-cli.md): inspection, presentation, prompt override, editor side pane.
- [I — Quality](i-quality.md): defects, audit residuals, build and test research.
- [J — Docs](j-docs.md): templates, capability catalog, instruction delivery.
- [K — Research](k-research.md): unscheduled spikes and references to their owners.
- [L — Big bets](l-big-bets.md): desktop and ancillary product ideas.
- [M — Sessions](m-sessions.md): native supervision, controlled startup, connect,
  workspaces, and the deferred Claude transport choice.
- [School records](school.md): separate-school history and recorded pantry reproductions.
- [Cancelled / superseded](cancelled.md): rejection trail; never a queue of live work.

## Recording work

Give each task one owning entry. Cross-topic references and roadmap bullets link to that
owner instead of duplicating its checkbox. Keep existing names and legacy PROD9 numbers
as stable search handles; newly recorded work gets a descriptive name.

- `[x]` means recorded complete, with commit evidence where available; it does not
  imply a release or push.
- `[ ]` means unfinished, subject to any explicit deferred, gated, or unverified label.
- Ideas remain provisional; their presence does not settle their design or priority.
- Preserve the ask ledger's status and provenance: `agent:inferred` is a derivation,
  while `user:verbatim` requires the exact quote. Never promote an inference to a ruling.
- A conflict keeps both claims and their evidence until a current ruling or implementation
  resolves it. See [reconciliation](reconciliation.md) for the initial source comparison.

`.ace/save.md` holds session context; `.ace/save.ledger.md` links in-flight asks to these
owners. Long-lived task details belong here. Research stays in its existing document,
linked from the task; it is not a second checklist.

Outline and Linear are historical sources, not active ACE trackers. Outline source
documents remain intact; each imported page records its source revision. The collection's
skill-discovery reference is covered by the existing
[discovery specification](../spec/skills/model.md#discovery-cascade), not another backlog
page. Source coverage and unresolved discrepancies are listed in
[reconciliation](reconciliation.md).
