# B — MCP provisioning

Source: [Outline][source], revision 13.
Status reconciled against repository records at `9df624a` on 2026-09-05.

[source]: https://outline.prodigy9.co/doc/b-mcp-provisioning-TysmxPORVh

Provision MCP servers per school through each resolved backend instance.

- [ ] **64** support stdio MCP servers in school.toml · *High*
- [x] **225** `ace mcp list` — lightweight, side-effect-free (decouple from
      health/register) · shipped 2026-07-22, `name<TAB>state` rows, no health probe

- [x] **mcp-verb-naming** MCP subcommands didn't rhyme with the outer verbs — bare
      `ace mcp` mutated where bare `ace skills` and `ace config` only read. Shipped
      2026-07-28: bare `ace mcp` is a read-only listing (`list` folded in), the automatic
      health check is deleted (`check` remains the only probe), `unregister <name>` (alias
      `remove`) is new, `reset` takes no argument and is the mass form, `clear` is
      dropped. Breaking on the CLI surface, which is pre-1.0. The `check` vs `ace doctor`
      (**123**) collision was ruled out of scope and is still open.
- [ ] **mcp-reset-picker** `ace mcp reset` with no name removes every school-registered
      server with **no confirmation at all** — and "remove these three, keep that one"
      needs four separate invocations today. Wants the multi-select picker (pre-ticked).
      Note `-p` is taken by the global `--prompt` one-shot flag. *(raised 2026-07-22)*

## Ideas / later

* **199** support `[[mcp]]` decls at project/user/local layers (needs merge-semantics
  design)
* **237** school ships a Dockerfile, ACE builds + boots it as an MCP server (spike)
* **34** ACE as an MCP server inside the backend (spike)
* **mcp-check-execution** — complete the model-driven health-check abstraction. Every
  check must consume the selected resolved `Backend`, including its command, environment,
  model, and effort; no backend-specific hard-coded launch may bypass that instance.
  Stream backend output to the terminal so warnings, progress, and prompts remain visible
  while any structured result is interpreted. The implementation may fully delegate to
  each backend kind or use one generic prompt through a shared backend execution path;
  settle that boundary before code.
* 🆕 ACE MCP proxy — with multiple active backend sessions each launching its own copy of
  an MCP process, ACE runs one shared instance behind a pass-through proxy and multiplexes
  the sessions onto it (resource optimization; ties to per-session process management)

## Shipped

48, 53, 42.

## Scope gate

**64** and the Dockerfile **237** proposal conflict with the current
[remote-only MCP decision](../decisions/2026-03-04-remote-only-mcp.md). Their source
priority does not supersede that ruling; scope must be explicitly revisited before
implementation.
