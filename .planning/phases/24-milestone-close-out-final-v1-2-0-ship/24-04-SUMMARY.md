---
phase: 24-milestone-close-out-final-v1-2-0-ship
plan: 04
subsystem: docs
tags: [readme, milestone-close-out, v1.2, webhooks, hero-block, milestones-cross-link]

# Dependency graph
requires:
  - phase: 24-milestone-close-out-final-v1-2-0-ship/24-01
    provides: "THREAT_MODEL.md anchors #threat-model-5-webhook-outbound and #threat-model-6-operator-supplied-docker-labels (referenced by the hero block's threat-model footer line) + §Security widening (preserved verbatim — boundary owned by 24-01)"
  - phase: 24-milestone-close-out-final-v1-2-0-ship/24-03
    provides: "MILESTONES.md v1.2 entry at top of file (cross-link target for the hero block + §Releases section)"
provides:
  - "README v1.2 'What's New' hero block above §Security (single-paragraph format with five v1.2 feature bullets + threat-model footer)"
  - "README §Configuration §Webhooks subsection (LEAN shape: 1 intro + TOML example + 5 behavior bullets + forward-pointer to docs/WEBHOOKS.md)"
  - "README §Features pointer for FCTX panel + exit-code histogram (absorbed into hero block — README has no top-level ## Features section)"
  - "README §Releases section above §License with MILESTONES.md + GitHub Releases links"
affects: ["24-05 (rc.4 close-out PR review)", "24-06 (rc.4 image's README will surface v1.2 content)", "24-07 (HUMAN-UAT operator first-encounter surface)", "v1.2.0 final-ship README content"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "README v1.2 hero block above §Security — single-paragraph format with inline anchor links (novel pattern for this repo; future milestone close-outs can mirror)"
    - "§Configuration cumulative-subsection-per-feature pattern continued (P17 added §Labels, P23 added §Tag Filter Chips, 24-04 adds §Webhooks)"
    - "§Releases terminal section above §License (new pattern; supersedes the alternative §Security footer-line approach offered by CONTEXT D-13)"

key-files:
  created:
    - ".planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-04-SUMMARY.md"
  modified:
    - "README.md (49 additive lines across 4 anchored insert sites; 0 deletions)"

key-decisions:
  - "§Features decision (W2): hero block IS the §Features pointer surface — README has no top-level ## Features section (planner-verified-at-2026-05-16; grep -nE '^## Features$' returned empty). FCTX panel + exit-code histogram are bullets #3 and #4 in the hero block. No separate ## Features section created."
  - "§Webhooks placement: after §Tag Filter Chips (L315) and before §Job Types (L317). Natural reading flow — §Tag Filter Chips footer at the prior L315 already cross-references docs/WEBHOOKS.md, so §Webhooks immediately after creates a tags → webhook-payload-includes-tags → webhook-config-details narrative."
  - "§Webhooks shape: LEAN (~28 added lines) mirroring §Tag Filter Chips (L287-315) rather than §Labels (L206-285) deeper shape. docs/WEBHOOKS.md is 649 lines and already carries the operator-facing detail; README §Webhooks defers there. Aligns with PATTERNS § Plan 24-04 + CONTEXT § Claude's Discretion."
  - "Hero block format: single-paragraph with inline anchor links. Simplest of the three options offered by CONTEXT § Claude's Discretion (the alternatives were a <details> collapsible block and a mermaid timeline)."
  - "§Releases placement: terminal section above §License (parallels §Contributing / §License terminal-section pattern). Preferred over the §Security footer-line alternative because §Releases is more discoverable for operators scanning the README ToC."
  - "Section-boundary discipline (W8): §Security paragraph (now L33-48; was L19-34 in HEAD) is byte-identical between HEAD and worktree. Plan 24-01 owns §Security; plan 24-04 inserts strictly above (hero block) and strictly below (§Webhooks, §Releases). git diff -U0 hunks land at +19..32 / +331..358 / +549..555 — all outside L33-48."

patterns-established:
  - "Pattern 1: 'What's New in vX.Y' hero block above §Security — single-paragraph format with five inline anchor links to feature subsections + closing line cross-linking MILESTONES.md and threat-model anchors. First instance for this repo; reusable for v1.3+ milestone close-outs."
  - "Pattern 2: §Releases terminal section above §License — links to MILESTONES.md (full release log) and GitHub Releases page (binaries + git-cliff notes). Replaces ad-hoc §Security footer cross-link approach."
  - "Pattern 3: §Configuration §Webhooks LEAN subsection — when the operator-facing detail doc (docs/WEBHOOKS.md) is large (~649 lines here), the README subsection stays at 1 intro + 1 TOML example + 5 behavior bullets + forward-pointer. Reusable shape for any future feature with a separate operator doc."

requirements-completed: []  # Plan 24-04 has requirements: [] in frontmatter (no REQ-IDs — paperwork close-out plan)

# Metrics
duration: 8min
completed: 2026-05-17
---

# Phase 24 Plan 04: README v1.2 close-out Summary

**v1.2 'What's New' hero block above §Security + §Configuration §Webhooks LEAN subsection + §Releases terminal section + MILESTONES cross-link — operator-discoverability of all five v1.2 features lands in the rc.4 image's README.**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-05-17T01:48Z
- **Completed:** 2026-05-17T01:55:50Z
- **Tasks:** 2 (1 read-only inventory + 1 four-insert edit)
- **Files modified:** 1 (README.md)
- **Diff shape:** 49 additions, 0 deletions across 3 hunks

## Accomplishments

- README v1.2 'What's New' hero block (L19-31) lands immediately above ## Security at L33, listing all five v1.2 features with inline anchor links and a closing threat-model + MILESTONES cross-link line.
- README §Webhooks subsection (L331-357) lands under ## Configuration between §Tag Filter Chips and §Job Types, mirroring the §Tag Filter Chips lean shape: 1 intro paragraph + 1 TOML example + 5 behavior bullets + forward-pointer to `docs/WEBHOOKS.md`.
- §Features pointer for FCTX panel + exit-code histogram is absorbed into the hero block (bullets #3 and #4) — README has no top-level §Features section, so the hero block is single source of truth (planner-verified default per checker W2).
- README §Releases terminal section (L549-555) lands immediately above ## License at L557, linking to MILESTONES.md (full v1.0 / v1.1 / v1.2 release log) and the GitHub Releases page (binaries + git-cliff notes).
- §Security boundary discipline preserved: plan 24-01's edits to the §Security paragraph (now L33-48) are byte-identical between HEAD and worktree; plan 24-04's three hunks land at +19..32 / +331..358 / +549..555, all OUTSIDE the §Security span.
- All edits are purely additive — zero deletions; no mermaid diagrams introduced (none warranted by the four anchored inserts).

## Task Commits

Each task was committed atomically per the close-out PR's per-plan convention:

1. **Task 1: Inventory** — no commit (read-only pass; planning notes captured inline in Task 2's commit body)
2. **Task 2: Apply the four README inserts in one diff** — `bc42337` (docs)

**Plan metadata commit (this SUMMARY):** to be appended after this file lands.

## Files Created/Modified

- `README.md` — +49 lines across 4 anchored insert sites:
  - L19-31: `## What's New in v1.2` hero block (single paragraph + 5 feature bullets + threat-model + MILESTONES footer line)
  - L331-357: `### Webhooks` subsection under `## Configuration` (intro + TOML example + 5 behavior bullets + forward-pointer)
  - L549-555: `## Releases` terminal section (MILESTONES.md + GitHub Releases links)
- `.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-04-SUMMARY.md` — this file (NEW)

## Exact Insertion Line Ranges (post-edit)

| Anchor | Pre-edit line | Post-edit line | Insert | Span |
|---|---|---|---|---|
| `## What's New in v1.2` | — | L19 | A. Hero block | L19-31 (+ blank lines + `---` at L32) |
| `## Security` (relocated by hero block) | L19 | L33 | unchanged (owned by 24-01) | L33-48 (byte-identical to HEAD) |
| `### Webhooks` | — | L331 | B. §Webhooks subsection | L331-357 |
| `### Job Types` (relocated) | L317 (pre-Task-2 numbering before §Webhooks) | L359 | unchanged | — |
| `## Releases` | — | L549 | D. §Releases section | L549-555 |
| `## License` (relocated) | L507 (pre-Task-2 numbering) | L557 | unchanged | L557-558 |

Total file length: 509 → 558 lines (+49).

## Planner-Discretion Decisions (per checker W2 + CONTEXT § Claude's Discretion)

1. **§Features decision (W2):** `grep -nE "^## Features$" README.md` returned empty (no top-level ## Features section). **Path taken: hero block ABSORBS the §Features pointer.** FCTX panel + exit-code histogram are bullets #3 and #4 in the hero block — `**Failure-context panel** on run-detail — …` and `**Exit-code histogram card** on job-detail — …`. Rationale: adding a separate ## Features section would duplicate hero-block content and violate the "single source of truth" reading flow.
2. **§Webhooks placement:** **AFTER §Tag Filter Chips (L315), BEFORE §Job Types (L317).** Rationale: §Tag Filter Chips' final paragraph already cross-references `docs/WEBHOOKS.md` (the `tags` field of `run_finalized` event), so §Webhooks immediately after creates a natural narrative flow.
3. **§Webhooks shape:** **LEAN (28 added lines)** mirroring §Tag Filter Chips (L287-315 leaner-shape precedent) rather than §Labels (L206-285 deeper-shape precedent). Per CONTEXT § Claude's Discretion and PATTERNS § Plan 24-04: since `docs/WEBHOOKS.md` is 649 lines and already carries the full operator-facing detail (table of contents at L11-28 lists 16 sections), the README §Webhooks subsection defers there for depth.
4. **Hero block format:** **Single-paragraph with inline anchor links.** Simplest of the three CONTEXT § Claude's Discretion options. Reads quickly; operators get all five feature pointers + threat-model + MILESTONES cross-link without expanding a `<details>` or rendering a mermaid timeline.
5. **§Releases placement:** **Terminal section above ## License.** Preferred over the §Security footer-line alternative because:
   - More discoverable for operators scanning the README ToC.
   - Parallels the §Contributing / §License terminal-section visual pattern.
   - Aligns with PROJECT.md "README sufficient for a stranger to self-host" quality bar.

## Anchor Links Added (GitHub-Markdown auto-derived)

| Anchor in hero block | Resolves to |
|---|---|
| `#webhooks` | `### Webhooks` at L331 (this plan's insert) |
| `#labels` | `### Labels` at L206 (P17) |
| `#tag-filter-chips` | `### Tag Filter Chips` at L287 (P23) |
| `./MILESTONES.md` | MILESTONES.md v1.2 entry at top-of-file (24-03's insert) |
| `./THREAT_MODEL.md#threat-model-5-webhook-outbound` | TM5 canonical (24-01's rewrite) |
| `./THREAT_MODEL.md#threat-model-6-operator-supplied-docker-labels` | TM6 new section (24-01's insert) |

Anchor verification: all six links resolve to real subsections / files at HEAD; no dangling links.

## Section-boundary Diff Check Result (W8)

**PASS.** Three checks performed:

1. **Hunk-overlap heuristic:** The hero-block insertion creates a hunk header that the conservative overlap heuristic flags because the hunk window (`@@ -18,0 +19,14 @@`) is adjacent to (not inside) the post-edit §Security span L33-48. This is expected and EXPLICITLY ALLOWED per the plan's acceptance criteria: *"the boundary check excludes the §Security header line itself; the hero block's new ## What's New in v1.2 becomes the new section above §Security."*
2. **Precise byte-identity check:** `git show HEAD:README.md` extract of §Security (L19-34 in HEAD) compared to current worktree §Security (L33-48) via `diff -q` → **clean** (byte-identical).
3. **Strict line-range filter:** Python parser on `git diff -U0 HEAD -- README.md` confirms zero `+` lines land inside the post-edit §Security span L33-48. The three hunks land at:
   - `+19..32` — hero block (ABOVE §Security)
   - `+331..358` — §Webhooks subsection (FAR below §Security)
   - `+549..555` — §Releases section (FAR below §Security)

§Security paragraph (the actual content lines owned by plan 24-01) is preserved verbatim.

## Decisions Made

See Planner-Discretion Decisions above. All five planner-discretion choices documented with rationale.

## Deviations from Plan

None — plan executed exactly as written. Task 1's inventory output matched the planner-verified default for the §Features decision (no `## Features` section exists), so the §Features pointer is absorbed into the hero block per CONTEXT D-13's planner-verified-at-2026-05-16 note.

## Issues Encountered

None substantive. Two minor scripting friction items resolved inline:

- **macOS `awk` lacks `match()` array-arg.** The plan's `<verify><automated>` block uses `awk` with the `match($0, regex, arr)` form which is a GNU `gawk` extension not present on macOS BSD `awk`. Replaced with a Python parser for the strict line-range boundary check. Result identical (PASS).
- **Python heredoc via stdin.** First attempt piped `git diff` to a `python3 <<EOF` heredoc; Python's argv parsing tripped on the diff's leading `index abcdef..fedcba` line (it was passed as args). Resolved by writing the diff to `/tmp/readme_diff_u0.patch` and reading from the file inside the heredoc. No content impact.

Neither friction item required a code change in README.md.

## User Setup Required

None — doc-only changes. The four README inserts surface to operators automatically when they read the README at the rc.4 / v1.2.0 SHA.

## Next Phase Readiness

- **rc.4 close-out PR (plan 24-05 / cargo-deny promotion + close-out PR merge):** ready. README v1.2 content present at HEAD; rc.4 image's README will carry it per CONTEXT § Specifics ("README hero block timing — pre-rc.4 only").
- **24-RC4-PREFLIGHT.md (plan 24-06):** unblocked. README v1.2 content is in place for the rc.4 cut.
- **24-HUMAN-UAT.md (plan 24-07):** unblocked. Operator first-encounter surface is updated (hero block + §Webhooks + §Releases all visible on the rc.4 image's README).
- **24-FINAL-SHIP-PREFLIGHT.md (plan 24-08):** unblocked. README state at the rc.4-SHA (= future v1.2.0-SHA) carries the v1.2 content.

No blockers. No follow-up work for plan 24-04 itself.

## Self-Check: PASSED

- `[x]` README.md exists and contains all four anchored inserts (hero block / §Webhooks / hero-block-absorbed §Features pointer / §Releases).
- `[x]` Commit `bc42337` exists on the worktree-agent branch.
- `[x]` §Security paragraph byte-identical between HEAD and worktree (boundary check W8 PASS).
- `[x]` All anchor links (`#webhooks`, `#labels`, `#tag-filter-chips`, `./MILESTONES.md`, `./THREAT_MODEL.md#threat-model-5-webhook-outbound`, `./THREAT_MODEL.md#threat-model-6-operator-supplied-docker-labels`) resolve to real subsections / files at HEAD.
- `[x]` Pure additive edit: 49 +, 0 -, no file deletions, no untracked files left behind.
- `[x]` Per CLAUDE.md mermaid-only mandate: zero ASCII diagrams introduced (and zero diagrams of any kind — none warranted by the four inserts).
- `[x]` Per CLAUDE.md PR-only workflow: commit landed on `worktree-agent-a0c3b6bd8ff13ced7`, not `main`.

---
*Phase: 24-milestone-close-out-final-v1-2-0-ship*
*Plan: 04*
*Completed: 2026-05-17*
