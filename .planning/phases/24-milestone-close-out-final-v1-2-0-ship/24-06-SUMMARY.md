---
phase: 24-milestone-close-out-final-v1-2-0-ship
plan: 06
subsystem: release
tags: [rc-preflight, runbook, release-engineering, maintainer-validated, ghcr, hyphen-gate, cargo-deny, audit-predicate]

# Dependency graph
requires:
  - phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
    provides: 21-RC2-PREFLIGHT.md (verbatim mirror target — primary structural lineage; 9-section shape, sign-off table, out-of-scope footer)
  - phase: 23-job-tagging-dashboard-filter-chips-rc-3
    provides: 23-RC3-PREFLIGHT.md (most-recent sibling — secondary mirror target; cross-reference footer shape; pending-maintainer-execution status pattern)
  - phase: 24-milestone-close-out-final-v1-2-0-ship
    provides: plans 24-01..24-05 (close-out PR contents that § 1 gates on; § 5 audit-predicate verification commands ride on plans 24-01/02/03/04; § 2 cargo-deny BLOCKING claim rides on plan 24-05)
  - phase: 12 (v1.1)
    provides: release.yml hyphen-gate at lines 132–135 (verified empty grep — `:latest` cannot promote on hyphenated tag)
provides:
  - 24-RC4-PREFLIGHT.md (maintainer-EXECUTES rc.4 tag-cut runbook; 190 lines; 9 sections + sign-off + out-of-scope + cross-reference)
  - § 5 audit-predicate verification recipe (T-V12-XCUT-05/06/07 grep commands gating rc.4 cut on Pitfall 56 close-out predicates)
  - Sign-off table with `:latest` invariant assertion (digest of `:latest` MUST equal `:1.1.0` digest)
affects: [24-07-HUMAN-UAT, 24-08-FINAL-SHIP-PREFLIGHT, v1.2 final-tag retag-the-rc-SHA discipline (D-01)]

# Tech tracking
tech-stack:
  added: []  # doc-only: no library/tool additions
  patterns:
    - "RC preflight verbatim-mirror lineage (P20 → P21 → P23 → P24) with per-rc substitutions"
    - "Audit-predicate verification section (§ 5) replaces the per-phase out-of-scope cardinality verification of prior preflights"
    - "Frontmatter `status: pending-maintainer-execution` (rc.4 has not yet been cut; matches P21 pre-execution shape)"

key-files:
  created:
    - .planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-RC4-PREFLIGHT.md
  modified: []

key-decisions:
  - "Mirror 23-RC3-PREFLIGHT.md byte-for-byte with the rc.4 / P24 / close-out substitutions specified in CONTEXT D-10; 21-RC2-PREFLIGHT.md is the secondary structural reference confirming two-sibling lineage."
  - "Replace § 5 (P21 EXIT-06 cardinality / P23 tags-as-Prometheus-label) with v1.2 close-out audit-predicate verification — T-V12-XCUT-05 (TM5/TM6 sections), T-V12-XCUT-06 (STRIDE rows T-S3/T-T4/T-I4/T-D4), T-V12-XCUT-07 (README anchor links to TM5/TM6) + REQUIREMENTS flips + audit doc + MILESTONES entry + README hero — per PATTERNS § Plan 24-06 § 5 (substantive divergence)."
  - "§ 2 cargo-deny row reads BLOCKING (FOUND-16 closed per plan 24-05) — NOT the still-non-blocking language from rc.2/rc.3 preflights. This is the second per-phase divergence after § 5."
  - "Sign-off table keeps the `:latest` invariant identical to rc.2/rc.3: digest of `:latest` MUST equal digest of `:1.1.0` (rc.4 is hyphenated; `:latest` does NOT promote until final v1.2.0 tag per P12 D-10 hyphen-gate)."
  - "NO modifications to `release.yml` / `cliff.toml` / `docs/release-rc.md` / `Cargo.toml` (per CONTEXT D-16 + D-18 informational); rc.4 is a tag-only `-rc.4` suffix on `Cargo.toml = 1.2.0`."

patterns-established:
  - "RC4 preflight = clean post-UAT cut, not a findings-driven rc — § 6 git-cliff preview budget is 3-5 commits (close-out PR commits only) per CONTEXT § Specifics."
  - "Frontmatter `created: 2026-05-16` (today, per env date); `status: pending-maintainer-execution` — sections 1-7 maintainer-verifiable; sections 8-9 fill after tag push."

requirements-completed: []  # plan frontmatter requirements field is empty per CONTEXT § REQ-IDs (P24 has no v1.2 REQ-IDs — all 41 covered by P15-23)

# Metrics
duration: ~10min
completed: 2026-05-17
---

# Phase 24 Plan 06: rc.4 Tag-Cut Pre-Flight Runbook Summary

**Authored 190-line maintainer-validated rc.4 tag-cut runbook mirroring 21-RC2-PREFLIGHT.md / 23-RC3-PREFLIGHT.md verbatim, with § 5 substantively diverged to verify the v1.2 close-out audit predicates (TM5/TM6 sections, STRIDE rows T-S3/T-T4/T-I4/T-D4, README anchor links) and § 2 cargo-deny row promoted to BLOCKING.**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-05-17T02:18:00Z (approximate, per pre-task plan read)
- **Completed:** 2026-05-17T02:28:40Z
- **Tasks:** 1 (single-task plan per `<tasks>` block)
- **Files modified:** 1 created (zero modified)

## Accomplishments

- Authored `.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-RC4-PREFLIGHT.md` (190 lines) — frontmatter + preamble + 9 sections + sign-off table + out-of-scope footer + cross-reference footer.
- Section 5 carries the v1.2 close-out audit-predicate verification triple (T-V12-XCUT-05/06/07) with executable grep recipes — substantive divergence from P21/P23 per PATTERNS § Plan 24-06 § 5.
- Section 2 cargo-deny row reads BLOCKING (FOUND-16 closed in plan 24-05) — the only other per-phase divergence from the verbatim mirror.
- Sections 3 (rustls invariant — `cargo tree -i openssl-sys` empty) and 4 (release.yml hyphen-gate at lines 132–135) AUTHORED in full (not "mirror reference only") per acceptance criteria.
- Sign-off table preserves the `:latest` invariant assertion verbatim from rc.2/rc.3: digest of `:latest` MUST equal `:1.1.0` digest.
- Cross-reference footer cites `21-RC2-PREFLIGHT.md` + `23-RC3-PREFLIGHT.md` with explicit substitution enumeration (rc.2/rc.3 → rc.4, P21/P23 → P24, plan list 01-10/01-07 → 01-05).
- NO modifications to `release.yml`, `cliff.toml`, `docs/release-rc.md`, or `Cargo.toml` (per CONTEXT D-16 / D-18 informational).

## Task Commits

1. **Task 1: Author 24-RC4-PREFLIGHT.md** — `7a40cd3` (docs)

## Files Created/Modified

- `.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-RC4-PREFLIGHT.md` (190 lines, NEW) — maintainer-EXECUTES rc.4 tag-cut runbook with 9 sections + sign-off + footers.

## Mirror Provenance — Section-by-Section Line-Range Cross-Reference

| § in 24-RC4 | Mirrored from 23-RC3-PREFLIGHT.md | Mirrored from 21-RC2-PREFLIGHT.md | Substitutions / Divergence |
|---|---|---|---|
| Frontmatter | L1-11 | L1-9 | `phase: 24`, `plan: 06`, `rc_tag: v1.2.0-rc.4`, `created: 2026-05-16`, `status: pending-maintainer-execution` |
| Preamble | L13-24 | L11-22 | P24 / rc.4 / D-10 / D-14 / D-16 / D-18 substitutions for D-15/D-16/D-18/D-22/D-24/D-26/D-27 |
| § 1 (plans merged) | L26-40 | L24-41 | Plan list compressed from 10/7 to 5 (24-01..24-05; plans 06/07/08 are autonomous=false maintainer runbooks not gated by § 1) |
| § 2 (CI matrix) | L42-55 | L43-56 | **DIVERGENCE:** cargo-deny row reads BLOCKING (FOUND-16 closed per plan 24-05) instead of still-non-blocking |
| § 3 (rustls) | L57-65 | L58-66 | rc.3→rc.4 substitution + appended P24 dep-rev-risk note (plan 24-05 Branch B advisory remediation) |
| § 4 (release.yml hyphen-gate) | L67-79 | L68-82 | rc.3→rc.4 substitution; lines 132-134 + 135 references verified against current `release.yml` (still accurate) |
| § 5 (close-out audit predicates) | L83-94 (REPLACED) | L84-95 (REPLACED) | **SUBSTANTIVE DIVERGENCE:** P21 EXIT-06 / P23 tags-as-Prometheus-label OUT — Pitfall 56 T-V12-XCUT-05/06/07 + REQUIREMENTS flips + audit doc + MILESTONES + README hero verification IN |
| § 6 (git-cliff preview) | L96-106 | L97-107 | rc.4 substitution; small-delta budget noted (3-5 commits since rc.3 per CONTEXT § Specifics) |
| § 7 (HUMAN-UAT placeholder) | L108-113 | L109-114 | Six-scenario placeholder list (matches plan 24-07 shape per CONTEXT D-10) — explicitly blank at preflight time |
| § 8 (tag command) | L115-134 | L116-133 | Message swap `"v1.2.0-rc.4 — milestone close-out (P24)"`; signed (-s) + unsigned fallback both presented per `docs/release-rc.md` Step 2a/2b |
| § 9 (post-publish verification) | L136-157 | L135-156 | rc.4 substitution; same `:latest` invariant detection (`:latest` digest MUST equal `:1.1.0` digest); cargo-deny line tightened to BLOCKING |
| Sign-off | L179-197 | L178-196 | Verbatim with rc.4 substitution; `:latest` invariant row identical |
| Out-of-scope | L159-168 | L158-167 | Verbatim with rc.4 / D-10 / D-16 / 24-08 substitutions |
| Cross-reference | L204-206 | L203-205 | Authored per PATTERNS § Plan 24-06 — cites both P21 + P23 with explicit substitution enumeration |

## Substitutions Applied (Per CONTEXT D-10 + PATTERNS Plan 24-06)

- `rc.2` / `rc.3` → `rc.4` (every occurrence)
- `P21` / `P23` → `P24`
- `FCTX UI panel + exit-code histogram` / `dashboard tag filter chips` → `milestone close-out`
- Plan list `01-10` (P21) / `01-07` (P23) → `01-05` (P24 close-out PR plans only; plans 06/07/08 are autonomous=false maintainer runbooks, NOT gated by § 1)
- D-22..D-27 (P21) / D-15..D-18 (P23) → D-10 / D-14 / D-16 / D-18 (P24 CONTEXT numbering)
- Tag message: `"v1.2.0-rc.2 — FCTX UI panel + exit-code histogram (P21)"` / `"v1.2.0-rc.3 — dashboard tag filter chips (P23)"` → `"v1.2.0-rc.4 — milestone close-out (P24)"`
- compose-smoke regression baseline: `v1.1+v1.2-rc.1` / `v1.1+v1.2-rc.2` → `v1.1+v1.2-rc.3`

## § 5 Audit-Predicate Verification Commands (Authored Verbatim — Pitfall 56)

```bash
grep -c "^## Threat Model 5: Webhook Outbound$" THREAT_MODEL.md         # expect 1
grep -c "^## Threat Model 6: Operator-supplied Docker labels$" THREAT_MODEL.md  # expect 1
grep -E "^\| T-(S3|T4|I4|D4) \|" THREAT_MODEL.md                         # expect 4 lines
grep -c "#threat-model-[56]-" README.md                                   # expect 2
grep -c "^- \[ \]" .planning/REQUIREMENTS.md                              # expect 0
test -f .planning/milestones/v1.2-MILESTONE-AUDIT.md && grep -q "passed\|tech_debt" .planning/milestones/v1.2-MILESTONE-AUDIT.md
grep -c "^## v1.2 — Operator Integration & Insight" MILESTONES.md         # expect 1
grep -c "^## What's New in v1.2$" README.md                               # expect 1
grep -c "^### Webhooks$" README.md                                        # expect 1
```

Maintainer runs each at preflight execution time to gate the rc.4 cut on the close-out PR's documentary artifacts landing intact on `main`.

## Sign-off Table State (Pending Maintainer Execution)

| Field | Status |
|-------|--------|
| Maintainer signature | pending (filled at rc.4 tag-cut time) |
| Date (UTC) | pending |
| Tag commit SHA | pending (will equal close-out PR merge SHA per CONTEXT D-01 final-ship strategy) |
| GHCR amd64 digest | pending (filled after release.yml workflow completes ~10-15 min post tag push) |
| GHCR arm64 digest | pending |
| GHCR `:latest` digest (must equal `v1.1.0` digest) | pending (invariant: `:latest` MUST stay at `:1.1.0` because rc.4 is hyphenated — P12 D-10 hyphen-gate) |
| GHCR `:1.1.0` digest (for comparison) | pending |
| GHCR `:rc` digest (must equal `v1.2.0-rc.4` digest) | pending |

Sections 1-7 ticked by maintainer (or by Claude on the maintainer's behalf per `23-RC3-PREFLIGHT.md` precedent — `sections_1_7_verified_by:` frontmatter pattern) AFTER the close-out PR merges; sections 8-9 ticked AFTER `git push origin v1.2.0-rc.4` and GHCR publish.

## Decisions Made

- Followed plan as specified — no auto-fixes or deviations needed. The plan provided byte-level content for every section; execution was mechanical substitution + line-range provenance accounting.
- Created at `2026-05-16` (frontmatter `created:`) per env current date; this is one day before the commit timestamp (`2026-05-17` UTC), which is consistent with the plan being authored on day-of and the SUMMARY rolling into the next UTC day.

## Deviations from Plan

None — plan executed exactly as written. The plan's `<action>` block provided verbatim text for every section + footer; verification ran one-shot and passed all acceptance criteria:

- `rc_tag: v1.2.0-rc.4` frontmatter present
- `autonomous: false` frontmatter present
- `## 1.` through `## 9.` section headers all present (grep loop 1..9)
- `## Sign-off` present
- `T-V12-XCUT-05` audit-predicate reference present (also `T-V12-XCUT-06` and `T-V12-XCUT-07` — 5 total mentions)
- `BLOCKING` (cargo-deny § 2) present; `still non-blocking` absent (confirms § 2 divergence applied)
- `must equal \`v1.1.0\` digest` (`:latest` invariant) present
- File length 190 lines (≥ 150 acceptance threshold + ≥ 180 frontmatter `min_lines`)
- `docs/release-rc.md` reused verbatim (6 cross-references in the runbook); NO edits to `release.yml` / `cliff.toml` / `docs/release-rc.md` / `Cargo.toml` (verified via `git status --short`)

---

**Total deviations:** 0
**Impact on plan:** None — the runbook lands exactly as specified.

## Issues Encountered

None.

## User Setup Required

None — doc-only deliverable. The runbook ITSELF is what the maintainer executes later; this plan delivers the runbook file, not its execution.

## Next Phase Readiness

- `24-RC4-PREFLIGHT.md` ready for maintainer execution after the close-out PR (plans 24-01..24-05) merges to `main`.
- Plan 24-07 (`24-HUMAN-UAT.md`) is the next plan in the close-out wave (Wave 4 sibling); per CONTEXT plan order: PR merge → 24-06 (rc.4 cut preflight; THIS PLAN) → 24-07 (rc.4 UAT) → 24-08 (final retag).
- Plan 24-08 (`24-FINAL-SHIP-PREFLIGHT.md`) will mirror this runbook but for the non-hyphenated `v1.2.0` tag — the `:latest` invariant flips at that step (`:latest` MUST equal `v1.2.0` digest, NOT `:1.1.0`).
- No blockers. The runbook's § 1 will tick once the close-out PR merges; § 2 will tick once CI completes on the merge commit (with cargo-deny BLOCKING per plan 24-05).

## Self-Check: PASSED

- File exists: `.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-RC4-PREFLIGHT.md` — FOUND (190 lines)
- File exists: `.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-06-SUMMARY.md` — being written now
- Commit exists: `7a40cd3` — FOUND on `worktree-agent-affc951f32e9ff97e`
- `git status --short` shows no modifications to `release.yml` / `cliff.toml` / `docs/release-rc.md` / `Cargo.toml` — VERIFIED
- All acceptance criteria from plan `<verify>` block satisfied — VERIFIED (grep loop PASS — 190 lines)

---
*Phase: 24-milestone-close-out-final-v1-2-0-ship*
*Completed: 2026-05-17*
