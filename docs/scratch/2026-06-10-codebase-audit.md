# Codebase Audit — 2026-06-10

Not spec/decision because: findings, not rulings — each one still has to be argued and
fixed on its own.

**Status (re-verified 2026-07-29): mostly open.** The High imports-source traversal
finding is **fixed** (`086dbd3`) — `cache_path` in `src/git.rs` rebuilds every path
segment, so containment is structural. The rest of the spot-check stands: `copy_dir_recursive`
still follows symlinks (the 2026-06-10 gitlink fix skips `.git`/`.gitmodules` only, not
symlinks), `cmd/upgrade.rs` still calls `exit(1)` directly, the update check still has
no `ureq` timeout, `src/school/skill_count.rs` still imports `ace`/`actions`/`cmd` from
below, and `--include-internal` still isn't on the CLI. The priority order at the bottom
stands, **except step 6 (the spec sweep), which was done on 2026-07-22** — shallow-clone
refs, index path, validate exit code, `ace --new`, `ace skills add`, learn's `ace*` append,
and Droid's status (dropped; `backends/droid.md` deleted) are all corrected in `docs/spec/`.
Current task ownership and known closures are in
[backlog I](../backlog/i-quality.md#local-records) and
[the source reconciliation](../backlog/reconciliation.md#historical-audit-coverage).
The findings below are historical evidence requiring revalidation before fixes.

Full-tree audit (~105 Rust files, ~25.5k lines) across five lenses: readability,
spec compliance, performance, security, and architecture/abstractions. Judged
against `general-coding` / `rust-coding` skill rules, `docs/spec/`, and
`docs/decisions/`. Five parallel read-only audit agents; findings deduplicated
and cross-checked below. Items flagged by two or more independent lenses are
marked **[×N]**.

## Executive summary

The codebase is in strong health for its size. The skills typestate pipeline
(sealed `Vetted` gate, `Locator` identity, provenance tracing) is the strongest
abstraction in the tree and was praised by every lens that touched it. Naming,
unwrap discipline, module docs, and test placement are near-exemplary. Git
subprocess budget and config parsing are lean.

The debt clusters in four places:

1. **Security: the hardening stops at skill identity.** The import `source`
   string and content provisioning are unguarded (path traversal, symlink
   following) even though the equivalent guard already exists for the
   project-mode specifier. One High finding.
2. **A layering inversion** in `src/school/skill_count.rs` that imports `ace`,
   `actions`, *and* `cmd` from below — plus a lossy `Ace::skills()` that trains
   consumers to re-run the discovery pipeline by hand (3–4× per launch).
3. **Stringly-typed errors at the backend/MCP seam** undercutting the otherwise
   strict layered-error contract.
4. **Command surfaces that missed reworks**: `ace school skills` predates the
   nested-identity model; `ace upgrade` bypasses the exit-code contract; spec
   text not swept after the shallow-clone and index-location decisions.

## Security

Threat model: malicious/compromised school repo; credential handling.

| Sev  | Where                                  | Finding                                                       |
| ---- | -------------------------------------- | ------------------------------------------------------------- |
| High | `src/git.rs:37` `ensure_source_cache`  | Import `source` traversal escapes cache root                  |
| Med  | `src/fsutil.rs:3` `copy_dir_recursive` | Follows symlinks — host-file exfil into school tree           |
| Med  | `src/backend/{claude,codex}.rs`        | MCP name/url passed positionally, no `--` separator           |
| Med  | `src/actions/project/register_mcp.rs`  | Confirm prompt hides MCP URL → credential-phishing vector     |
| Low  | `src/cmd/upgrade.rs:51`                | Downloaded binary applied with no checksum/signature          |
| Low  | `src/backend/claude.rs:196`            | Resolved header secrets visible in argv (`ps`)                |

- **[High] Path traversal via `[[imports]].source`** (`src/git.rs:37-40`,
  reached from `pull_imports.rs:63` / `add_import.rs:41`). A school.toml with
  `source = "../../../../tmp/evil"` makes `ensure_source_cache` clone an
  attacker repo outside the cache root. The project-mode specifier already has
  `has_traversal` (`src/config/school_paths.rs:39`); the guard was never
  extended to imports — a fail-open gap against the repo's own
  whitelist/fail-closed convention. Fix: whitelist clean `owner/repo` shape
  after normalization, or canonicalize and assert `starts_with(cache_root)`.
- **[Med] Symlink-following copy** (`src/fsutil.rs:3-18`). A skill dir in an
  import source containing `data -> ~/.ssh/id_rsa` gets the target *content*
  copied into `skills/` on `ace school pull` — exfil into a tree that gets
  committed and fed to agents. Fix: `symlink_metadata` check, skip or reject
  symlinks during copy.
- **[Med] Argument injection into backend CLIs** (`claude.rs:190`,
  `codex.rs:277`). School-controlled `name`/`url` pushed without `--`; an
  option-looking url can inject flags into `claude mcp add`. Exploitability
  depends on the backend's parser (unverified), but the missing separator is
  real. Fix: literal `--` before positionals + require `https://` scheme.
  (Git commands were checked and are NOT injectable — URLs are always built
  with an `https://github.com/` prefix.)
- **[Med] MCP prompt omits destination** (`register_mcp.rs:43,127-161`). Prompt
  says only `Register MCP '<name>'?`; a hostile school can name itself
  `github`, point at an attacker URL, template `{{ github_pat }}` into headers,
  and harvest the secret the user types. Fix: show the URL (and that entered
  secrets go to that host) in the confirm prompt.
- **[Low] Upgrade integrity** rests solely on TLS; trigger version comes from
  `ace-rs.dev/latest`, binary from GitHub releases, no sha/signature check.
- Verified clean: no `unsafe`; TOML/frontmatter parsing panic-free; `Locator`
  admission is whitelist-by-construction; symlink reconciliation confined to
  the managed root; git runs with `GIT_TERMINAL_PROMPT=0` / SSH `BatchMode`.

## Architecture & abstractions

- **[High, ×2] `src/school/skill_count.rs` inverts the layer law** — imports
  `crate::ace::Ace`, `actions::project::learn::LearnAction`, *and*
  `crate::cmd::CmdError` from the bindings layer (`skill_count.rs:10-12`). It
  is a UI workflow parked in `school/` by topic, not layer. Move
  `maybe_offer_learn`/`maybe_hint_relearn`/`record_decline` up into
  `actions/project/` (or `cmd/`); pure `count()` may stay below only if it
  sheds `Ace`.
- **[Med, ×2] `Ace::skills()` is lossy → pipeline re-assembled 3×** — the
  accessor discards discovery prunes, so `link_skills::prepare`
  (`link_skills.rs:158-173`) and `skill_count::count` rebuild
  discover→validate→resolve→with_rejected from disk. This is both the main
  perf redundancy (~3–4 full skill-tree walks + SKILL.md reads per launch) and
  the "lifecycle not fully expressed in types" smell. Fix: carry prunes on
  `Skills<Decided>` (peer to `rejected`), make `Ace::skills()` the sole
  assembly point.
- **[Med] `config/` imports `backend::Kind`** (`config/resolve/merge.rs:8`),
  breaking architecture.md's "config imports nothing". It only needs the
  default backend *name*; a `DEFAULT_BACKEND: &str` in config + a backend-side
  equality test is more honest.
- **[Med] `School` is a field-for-field clone of `SchoolToml`**
  (`school/mod.rs:33-58`) — a binding that binds nothing. Give it real
  semantics or collapse to a re-export.
- **[Med] Upward imports from standalone modules** — `templates/session.rs:4`
  imports `ChangeKind`/`SkillChange` via `actions::project` though they live in
  `skills/`; `upgrade/mod.rs:7` takes `&mut Ace` for a flag + warn sink.
- **[Low] Capability mask as bare `u32`** threaded through
  `link_skills::prepare`; `Kind::is_folder_supported` string-matches
  `"skills"`/`"commands"`/`"agents"` one screen below the bitmask built to
  kill that pattern. A `Features(u32)` newtype + feature bits.
- **[Low] `skill_count::has_explicit_skills_key` re-parses raw TOML** because
  `AceToml` collapses missing-`skills` vs `skills = []`. The tristate belongs
  in the config type, not a raw re-read two layers up.
- **[Low] `cmd/mod.rs` (869 lines) is the gravity well** — `CmdError`/
  `ExitCode` + classifiers deserve their own file before the next variants.
- Suggested guard: a layering test asserting no `crate::{cmd,ace,actions}`
  imports below the ace layer — converts the doc-only dependency law into CI.
- Sound: `Ace` is a disciplined lazy-cache session view, not a god object;
  backend enum-dispatch seam is the right shape; skills typestate genuinely
  makes invalid states unrepresentable.

## Spec compliance

- **[High] `ace school skills` predates the nested-identity rework**
  (`src/cmd/school.rs:123-177`): single non-recursive `read_dir` (nested
  `typescript/coding` shows as bogus `typescript`), porcelain keys on
  frontmatter `name` — both contradict `docs/spec/school/school-commands.md`
  and the model rule that frontmatter `name` is never a key. Fix: reuse
  `Skills::discover`. (Also flagged by the readability lens.)
- **[High] `ace import --include-internal` missing** — spec'd in
  school-commands.md + selection.md; `include_internal` fully wired in
  config/resolve but absent from the CLI (`cmd/mod.rs:147-163`). A glob import
  cannot record it without hand-editing school.toml.
- **[Med, ×2] `ace upgrade` bypasses the exit-code contract**
  (`cmd/upgrade.rs:4-12`): hardcoded `exit(1)` flattens all classes, hints
  never print — against `docs/decisions/2026-05-30-exit-codes.md`. Route
  through `exit_on_err`. (`docs/spec/upgrade.md`'s "1 on failure" sentence is
  also stale.)
- **Stale spec text** (update docs, code is right):
  - `school-commands.md` validate exit code: says 1, code correctly exits 3.
  - Shallow-clone references in `setup.md` + `school-commands.md` contradict
    decision 2026-03-25-no-shallow-clones and `git.rs:209`.
  - `skills/sync.md` says index at `~/.cache/ace/index.toml`; lives at
    `~/.local/share/ace/` per decision 2026-04-22.
  - `configuration.md`: `ace --new`/`-n` flag doesn't exist (it's `ace new`);
    built-ins list omits `opencode`.
  - `selection.md` references nonexistent `ace skills add` (real verbs:
    `include`/`exclude`/`reset`).
  - `setup.md` "only unique responsibilities" omits the backend instructions
    file written from `PROJECT_CLAUDE_MD`.
- **Spec gaps**: `learn.rs:182` force-appends `ace`/`ace-*` to the skills
  filter — sensible but undocumented in learn.md; `ace fmt` and `ace link`
  have no spec home; `ace school init --force` undocumented.
- **Droid backend**: fully spec'd (`backend.md`, `backends/droid.md`, sync
  matrix) but zero code. Mark unimplemented or track as an issue.
- **[Low, ×2] `ace diff` on embedded school** (`cmd/diff.rs:11-15`) maps
  missing `clone_path` to `NoSpecifier`, hinting "run `ace setup`" at a
  correctly configured repo. Needs its own message.
- Verified clean: `require_school` resolution + error split, action layout,
  `ace paths` contract, git-via-Command only, no crossterm, exit-code
  classifiers (modulo upgrade), feature-bitmask emit, learn thresholds, MCP
  surface, singular-`skill` backcompat alias.

## Performance

Overall healthy for a CLI: OnceCell-lazy config, warm `ace` = 2 git spawns,
full `ace pull` ≈ 7–8 distinct spawns, bounded walks.

- **[Med] Update check can hang every command** (`upgrade/check.rs:13`,
  invoked from `cmd/mod.rs:508`): synchronous `ureq` GET with **no timeout**
  before every non-porcelain command when the 4h marker is stale; offline or
  black-holed it stalls tens of seconds, and the marker only writes on success
  so it stalls again next run. Fix: 1–2s timeout on the agent, or move the
  stale-path fetch into the existing background-upgrade child.
- **[Med] 3–4× skill discovery per launch** — see architecture M2 above; same
  root cause, one fix.
- **[Low/Med, ×2] Double `mcp_list`** (`register_mcp.rs:35` + `:81`): parses
  the entire (often multi-MB) `~/.claude.json` twice, or spawns
  `codex mcp list` twice. Thread the first result through.
- **[Low] `read_frontmatter_flags` reads whole SKILL.md** for two keys
  (`discover.rs:286`); read a bounded prefix.

## Readability & code quality

Verdict from the dedicated pass: unusually good health — precise naming,
near-perfect unwrap discipline, design-rationale module docs, tests beside
code. Remaining items beyond those already covered above:

- **[Med] `PrepareError::Clone(String)` is a stringly catch-all**
  (`actions/project/prepare.rs:17`): 8× `.map_err(|e| Clone(e.to_string()))`
  in pull.rs erases the GitError/ConfigError split. Carry typed sources.
- **[Med] Backend MCP API returns `Result<_, String>` throughout**
  (`backend/mod.rs:195-211` + per-backend files); `RemoveMcp::run` declares
  `Result<(), String>` but always returns `Ok(())`. Introduce `McpError`;
  make `RemoveMcp` infallible. (Same finding from the architecture lens.)
- **[Med] `tree.rs:58` silently swallows school.toml parse errors** — a
  corrupt school.toml behaves as "no school" with zero diagnostics. Propagate
  or warn.
- **[Med] Backend exec prologue copy-pasted 8×** across
  claude/codex/opencode/flaude; extract `base_command(launch, fallback, dir,
  env)`.
- **[Med] `cmd/config.rs:186-329` `explain()`** — 143 lines of six identical
  stanzas; table-drive it. Also duplicate backend-name enumeration vs
  `cmd/main.rs::list_known_backend_names`.
- **[Med] `prompt_text`/`prompt_select` lack the non-Human guard**
  (`ace/io.rs:237-263`) that `prompt_confirm` has — porcelain/CI runs can hang
  on an inquire prompt.
- **[Med] Test helper `link_all` duplicates `Link::run` verbatim**
  (`link.rs:483-528`) — will drift; call the real action with a Silent Ace.
- **[Med] Stale TODO contradicts a ruling** (`config/ace_toml.rs:72`): "add
  `role`…" vs the 2026-05-22 roles-removed decision. Delete.
- **[Med] Two hand-rolled frontmatter scanners** (`skills/discover.rs:286` vs
  `config/skill_meta.rs:21`) with subtly different delimiter handling;
  consolidate.
- Low: `pull.rs:176` swallows `git diff` failures as "no changes";
  `looks_like_skill_token` doc/code drift; stale `#[allow(dead_code)]` on
  `MAX_SKILL_DEPTH`; dead `is_ready`; `basename_of` re-implements
  `Locator::leaf()`; three copies of the resolve-override fold in
  `cmd/mod.rs:570-657`; `warn_if_rejected` boolean reads backwards;
  codex config read/write boilerplate ×2; `find(...).is_some()` → `any`;
  unused `_ace` param on `Link::run`; raw `exit` mid-`run_inner` (mixed
  altitude) — extract `run_one_shot`.

## Suggested priority order

1. **Security trio**: imports-source traversal guard (High), symlink-safe
   copy, MCP prompt shows URL (+ `--` separator). Small, contained fixes.
2. **`ace upgrade` → `exit_on_err`** (decision violation) + ureq timeout
   (worst-case UX). Both tiny.
3. **`skill_count.rs` relocation + prunes on `Skills<Decided>`** — one
   refactor fixes the layering inversion, the 3× discovery, and the raw-TOML
   re-read together; add the layering test while there.
4. **`ace school skills` → `Skills::discover`** + `--include-internal` flag
   (spec compliance, user-visible).
5. **Error-type cleanup**: `McpError`, `PrepareError` typed sources,
   school.toml parse-error surfacing.
6. **Spec sweep**: shallow-clone refs, index path, validate exit code,
   `--new`, `ace skills add`, learn's `ace*` append; decide droid's status.
7. Remaining readability dedup (backend prologue, `explain()`, frontmatter
   scanner, test helper) as opportunistic slices.
