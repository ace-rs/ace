# D — Resource sync generalisation (gated)

Source: [Outline][source], revision 4.
Status reconciled against repository records at `9df624a` on 2026-09-05.

[source]: https://outline.prodigy9.co/doc/d-resource-sync-generalisation-gated-AwW9Dd3dKC

The biggest latent epic. All four members circle one decision: does ACE sync only skills,
or all four backend resource folders (skills, agents, commands, rules)?

**Gating step:** write a new dated `docs/decisions/` entry superseding the skills-only
scope ruling (`project_skill_scope`). Do not start any member until that lands.

- [ ] **234** first-class `agents/` sync
- [ ] **68** extend imports to rules, commands, and agents folders
- [ ] **235** first-class `plugins/` sync (supersedes skills-only scope decision)
- [ ] **228** unified backup strategy for pre-ACE content across all backend folders
