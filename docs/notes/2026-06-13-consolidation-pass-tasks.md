# ACE Issue Consolidation Pass — Task List

## STATUS (2026-06-13, end of session) — pass complete, output written

All five tasks done. **Output: [`2026-06-13-consolidation-plane-seed.md`](2026-06-13-consolidation-plane-seed.md)**
(supersedes `2026-06-09-roadmap-consolidation.md`).

- **Scope locked with chakrit**: 5-state remap (BACKLOG/PLANNED/MERGED/RELEASED/CANCELLED);
  On Production→RELEASED, MERGED empty (transient), epics span all statuses (orthogonal axis).
- **Done**: status remap (29 PLANNED / 47 BACKLOG / 1 verify-done=74), de-dup vs full bodies
  (all four 2026-06-09 merges confirmed + two new: `253⊇242`, `197↔248`), 12 epics (Modules),
  4 Cycles, verify-done list (125 likely already-done), CANCELLED dispositions, label set.

**1-by-1 walk DONE (2026-06-13).** All flagged decisions ratified — see blueprint §8
(sign-off) and the updated inline sections. Summary: 242+253+236-arm merged → "selection UX
improvement"; 197+248 merged → "backend model config" (opaque slots, no model list); 44→
PLANNED; 70/199/33/226→BACKLOG; 60/125→RELEASED (verified in source); 74→split (impl
RELEASED + Windows-verify BACKLOG); cancelled 9/26/22/38→import-as-cancelled; new BACKLOG:
investigate `skills.json` format. Blueprint is the authoritative Plane-seed.

Housekeeping: Linear personal API key revocation — chakrit handling himself.

**NEXT SESSION: Plane setup / seed.** The blueprint is the authoritative seed. Migration
itself still parked on the plane.so deploy — confirm deploy is up first, then seed Modules
(12 epics A–L), Cycles (1–3), 5-state statuses, and labels (§7) per the blueprint.

Also note: **RTK uninstalled from this repo** this session (RTK.md + .rtk/ deleted, CLAUDE.md
§RTK removed, school-toml.md example de-rtk'd) — unrelated to the catalog, done on request.

---

Handoff for a fresh (`/clear`ed) session. Goal: a **big consolidation pass** over the ACE
issue catalog — group into epics/phases, de-dup, merge related minor items — to produce a
clean structure to seed **Plane** with, instead of flat-importing 134 issues. This is prep
for the Linear→Plane migration (parent task), which is itself parked on the plane.so deploy.

## Inputs (already in place)

- **`2026-06-13-linear-ace-catalog.json`** — complete, faithful export of all 134 issues
  (full descriptions + comments, parent hierarchy). This is the consolidation source.
  Enriched today via `2026-06-13-refetch-linear-catalog.sh`.
- **`2026-06-09-roadmap-consolidation.md`** — prior pass: 15 thematic clusters (A–O) +
  a merge/close decision list. Built from titles only (no full descriptions/comments).
  **Build on this, then re-check against the full bodies — don't restart from scratch.**

## Tasks

1. **Confirm scope** (decision for chakrit at start):
   - Consolidate the **open work** (77 Backlog + any started) into epics/phases.
   - **On Production (53)** → import as completed history, flat, *not* restructured.
   - **Canceled + Duplicate (4)** → drop or import as archived/canceled.
   - Default recommendation above; confirm before producing the structure.

2. **De-dup pass using the full data.** The 2026-06-09 note worked from titles; now read
   the full descriptions + comments to catch dupes/overlaps it couldn't see. Confirm or
   revise the existing merge calls and hunt for new ones.

3. **Apply the already-identified merges/closes** from `2026-06-09` §1:
   - Close **PROD9-74** (verify the release path ships a Windows binary first).
   - **124 ⊇ 195** (one feature; host runner in 123 `ace doctor`).
   - **236 ⊇ 215 + 244** (fold corrections into the redesign; ties to the pending
     `ace learn` re-run threshold note).
   - **234 + 68 + 235 + 228** → one "generalise sync beyond skills" epic (needs the
     skills-only scope decision doc superseded — biggest latent epic).
   - **56 + 15** → one integration-test-strategy item.
   - Re-scope shipped-work follow-ups: 66 unblocked; 187 is a 65 follow-up bug; 146/147/197
     build on shipped backend work; 121/67 reframed by shipped caching.

4. **Form the epic/phase structure.** Consolidate the 15 clusters into **bigger epics**
   (some clusters merge), then assign a **phase/sequencing** layer (milestones). Map onto
   Plane's model: epics ≈ **Modules**, phases ≈ **Cycles**. The High tier (243, 64, 216,
   17, 122 after 74 closes) is the coherent next-milestone slice — likely Phase 1.

5. **Write the output doc** — `2026-06-13-consolidation-<slug>.md` that **supersedes**
   `2026-06-09-roadmap-consolidation.md`: epics → member issues, phase assignment, merge
   map, close list. This becomes the Plane-seed blueprint.

## Notes

- This is structure/blueprint work — **no edits to Linear** (it's being abandoned).
- Decisions (which merges, what closes, epic boundaries) are chakrit's calls — walk them
  for sign-off rather than committing unilaterally; the 1-by-1 protocol fits the merge list.
- Catalog JSON + scripts are untracked by design (migration scratch).
- Housekeeping: revoke the Linear personal API key pasted earlier in the prior session
  (Settings → Security & access → Personal API keys) if not already done.
