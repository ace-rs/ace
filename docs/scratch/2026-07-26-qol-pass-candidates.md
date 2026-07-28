# Quality-of-life pass — candidate list

Not spec/decision because nothing here is ruled on yet — it's a candidate set awaiting
chakrit's vetting. Survivors graduate to Outline (the only tracker); this file dies after.

Swept the whole ACE collection on 2026-07-26: epics A–L, the Roadmap doc, and the School
doc. Every item below is quoted or paraphrased from one of those; the epic letter and
issue ID follow each line. Nothing here is invented — where I merged two entries into one
line I say so.

**Filter used.** QoL = friction a user feels while *using* ACE day to day, fixable
without a new architecture, a spec gate, or a security review. That excludes the security
fix (247), backend completion (17, hermes, start-mode), MCP provisioning (64), and all of
Epic D — those are the roadmap's "Now" band and are not QoL.

## Tier 1 — the pass proper

Small, no design gate, felt on ordinary use. Recommended as one batch.

| # | Item | Epic | Why it's QoL |
|---|------|------|--------------|
| 1 | `ace mcp reset` with no name wipes every school-registered server with **no confirmation**; "keep one of four" costs four invocations | B (`mcp-reset-picker`) | Near-footgun. The multi-select primitive already exists (`Io::prompt_multiselect`), so this is wiring, not design. Note `-p` is taken by global `--prompt`. |
| 2 | `ace pull` misreports the tier folder name as the changed skill | I (152) | Every pull prints a wrong name; pure noise-to-trust cost. |
| 3 | Pipe `ace diff` through a pager | H (44) | Long diffs currently scroll off. |
| 4 | Hide the flaude backend from user-facing help + docs | I (150) | Test-only surface leaking into `--help`; CLAUDE.md already says flaude is test-only. |
| 5 | gitignore block enumerates all backends from the registry | A (119) | Adding a backend today means hand-editing ignores. |
| 6 | Surface discovery structural prunes in read-only paths (`ace skills`, skill_count) | I (241) | Skills silently vanish from counts with no explanation. |
| 7 | `ace setup` seeds CLAUDE.md with an `/ace-init` hint | F (252) | One-line onboarding nudge; removes the "now what" beat after setup. |

## Tier 2 — QoL, but each needs one small ruling first

Worth the pass if you rule on them now; otherwise they stall mid-batch.

| # | Item | Epic | The ruling needed |
|---|------|------|-------------------|
| 8 | `ace pull` never links — only updates the clone, unlike bare `ace` / `setup` / `link` which share one link action | F (🆕, filed 2026-07-26) | Should pull relink? It's a behavior change to a public verb, not a bug fix. |
| 9 | MCP subcommand naming doesn't rhyme with the outer verbs — bare `ace skills` is read-only but bare `ace mcp` *mutates*; `register`/`reset`/`check` vs outer `import`/`link`/`pull`; `check` will collide with `ace doctor` (123) | B (`mcp-verb-naming`) | Naming pass = renames = backcompat aliases per CLAUDE.md §Backcompat. Cheap only if done before more subcommands accrete. |
| 10 | Global `--yes` / auto-confirm flag | H (190) | Interacts with the just-shipped multi-select picker ruling (`docs/decisions/2026-07-22-batch-selection-prompts.md`) — what does `--yes` mean for a multi-select? |
| 11 | One inspection surface: `ace template` renders builtin prompt templates to stdout **+** `ace explain`/`show` surfaces a skill's frontmatter | H (227 + 🆕) | I merged these two — both are "print the thing ACE would use". Ruling: one verb or two? 227 is also roadmap-coupled to prompt-override. |
| 12 | Configurable session name + tab color for the terminal/backend | H (214) | Roadmap parks it in Icebox. Reversing that is yours. |
| 13 | `[[backends]]` ANTHROPIC_API_KEY conflicts with claude.ai login | A (147) | Real daily annoyance, but the fix is a policy call (which wins, and does ACE warn or unset?). |
| 14 | Parallelize import-source fetches in `ace school pull` | C (121) | Speed-only. Ruling: is pull slow enough to be worth concurrency risk? |
| 15 | `--local` flag for `ace setup` — temp workdir, school goes in `local.toml`, no `ace.toml` commit | F (🆕) | New surface, small. Ruling: is this a real workflow or a one-off? |

## Explicitly out of this pass

QoL-flavored but each is its own project, gated, or already Iceboxed:

- `ace doctor` (F 123) and required-CLI-deps + install flow (F 124 ⊇ 195) — an epic.
- `ace switch` / `ace eject` (F 69, 43) — lifecycle, not ergonomics.
- `ace llm-help` (H 13), `ace --bare` (G 160), polymorphic flags (G 159), backend-agnostic
  `--chrome` (A 🆕) — new surfaces needing design.
- `inject=` (E 🆕), token-compress at link time (E 134), per-repo skill selection (E 120) —
  all touch the `skills=` write path; Epic E wants a decision first.
- Selective `school pull` (C 🆕), MCP health-check slowness (B 🆕), MCP proxy (B 🆕).
- The whole Epic G session-runtime substrate — its own note says design it as one
  substrate, not six one-offs.

## My read

Tier 1 is seven items, none of which needs you in the loop past approval, and five of the
seven are one-file changes. That's the pass. Tier 2 is where the value is but it's really
a decision session, not a coding session — worth splitting.

Sharpest single item across both: **#1**, because it's the only one that can destroy user
state without asking.
