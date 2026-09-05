# F — School lifecycle, setup & env health

Source: [Outline][source], revision 8.
Status reconciled against repository records at `9df624a` on 2026-09-05.

[source]: https://outline.prodigy9.co/doc/f-school-lifecycle-setup-env-health-c8fcMTVW4C

Setup, school switching, and environment diagnostics.

- [x] **216** detect ace.toml school edit, stop spamming stale-symlink warnings · *High* —
      shipped `b86b63b`; outside-root links now fail with `ace link --force`
- [x] **pull-link-hint** `ace pull` updates the clone and points to `ace link` when skills
      change; `e8cd872` explicitly rejects automatic relinking. The former “Should pull
      relink too?” question is closed.
- [ ] **69** `ace switch` — change project school
- [ ] **43** `ace eject` — unlink a school (building block for 69)
- [ ] **123** `ace doctor` — general environment health check
- [ ] **124 (⊇195)** school.toml declares required CLI commands + AI-guided install flow;
      195's pluggable env checks fold in as the "recommendations" arm, hosted by 123

## Ideas / later

* **33** treat dirty school cache as the default working state
* **72** initial-setup module for non-technical users / junior devs
* **10** investigate school scripts for machine/software setup
* **52** log setup/sync failures for upstream reporting
* **252** `ace setup` seeds CLAUDE.md with an `/ace-init` (repo-bootstrap) hint
* 🆕 `--local` flag for `ace setup` — temporary workdir, don't commit `ace.toml`; put the
  school in `local.toml`
* 🆕 `--school <specifier>` **override flag** — runtime-only school override, the peer of
  the existing `--backend` / `-b`. Same shape: highest precedence, writes no config file,
  applies to every school-dependent command (bare `ace`, `ace setup`, `ace pull`,
  `ace link`). Lets a session run against a different school without editing `ace.toml` or
  `ace.local.toml`. Resolution order and the runtime-only rule are already spec'd for
  backend in `docs/spec/backend.md` §Resolution Order — mirror it for school.

## Shipped

6, 30, 57, 71, 7, 14, 49, 73, 125 (init writes ace.toml).

## Doctor scope

**123** also owns the skill-frontmatter diagnostics follow-up referenced by
`docs/spec/skills/model.md`: report specification violations without making liberal intake
reject them. Its boundary with explicit `ace mcp check` remains open (B's
**mcp-verb-naming** record); required CLI checks and recommendations remain grouped under
**124 (⊇195)**.
