# Offer a set as one checklist, never N sequential confirms

- **Date:** 2026-07-22
- **PR:** manual
- **Status:** accepted

## Decision

When ACE must ask the user about a *set* of things — MCP servers to register, unhealthy
servers to re-register, skills to import — it presents one multi-select checklist, not a
`Confirm` per item.

The primitive is `Io::prompt_multiselect(prompt, options, default_all)`, returning the
**indices** of the ticked options; `partition_picked` splits the caller's own list into
(chosen, declined) from that. Indices rather than values, so no caller has to match a
displayed string back onto its source item — labels are then free to be sanitized display
forms.

`default_all` sets the initial tick state and, outside `Human` output mode, *is* the
answer: all or nothing, mirroring how `prompt_confirm` returns its default when there is
no terminal.

Declines still persist. Unticking an MCP server writes it to `exclude_mcp` in
`ace.local.toml` exactly as answering "no" did — [ux.md](../spec/ux.md) §4 is unchanged,
only the number of keystrokes it takes to express the decision.

## Rationale

A school with six MCP servers produced six yes/no prompts on a first run, each requiring
a keystroke and each losing the context of the other five. The user could not see the
shape of the decision — how many servers, which ones — until they had already answered
most of it. §1 of ux.md ("the backend is the product; ace is the doorway") makes this a
defect, not a polish item: it is pure friction between the user and their session.

A checklist shows the whole set at once, defaults sensibly, and costs one Enter to accept.

Sequential confirms also quietly capped `ace import` at one skill per invocation — the
prompt was a single-select because a `Confirm` loop over a discovered skill list would
have been worse. Multi-select removes the cap.

## Consequences

- Import can now install several skills in one run, which made the school.toml write path
  append one `[[imports]]` block per pick. Folded into a `merge_import` that reuses the
  literal decl already covering a source (glob decls are left alone). This closes
  PROD9-243 as a precondition rather than a follow-up.
- The interactive layer is not unit-testable; `prompt_multiselect` has no test. The pure
  halves — `partition_picked`, `merge_import`, `inventory` — carry the coverage, and the
  TUI itself is verified by hand.
- `default_all: false` is the right default for *additive* pickers (import), `true` for
  *confirmatory* ones (register these, re-register those). No global rule; the callsite
  knows which it is.
