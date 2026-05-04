---
phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
plan: 11
subsystem: release-engineering
tags: [uat, rc-preflight, rc.2, fctx, exit-histogram, maintainer-validated, autonomous-false]

# Dependency graph
requires:
  - phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
    provides: "Plans 21-01..21-10 (FCTX panel + exit-code histogram + uat-* recipes)"
  - phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
    provides: "20-RC1-PREFLIGHT.md — literal precedent mirrored verbatim with rc.1 → rc.2 / P20 → P21 substitutions per D-22"
  - phase: 12 (release engineering)
    provides: "release.yml :latest hyphen-gate (D-10), docs/release-rc.md runbook, cliff.toml grammar"
provides:
  - "21-HUMAN-UAT.md — maintainer-validated UAT covering FCTX-01..06 + EXIT-01..06 across 8 scenarios + a11y umbrella + EXIT-06 cardinality grep + D-32 rustls invariant spot check"
  - "21-RC2-PREFLIGHT.md — v1.2.0-rc.2 tag-cut runbook reusing docs/release-rc.md verbatim with :latest invariant detection + GHCR digest sign-off table"
  - "Maintainer sign-off recorded 2026-05-02 — gates the rc.2 tag cut"
affects:
  - "v1.2.0-rc.2 release engineering (tag cut + GHCR push)"
  - "Phase 22 planning (rc.2 → final v1.2.0 path)"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "rc-preflight-mirrors-precedent: rc.2 preflight reuses P20 RC1-PREFLIGHT structure with locked substitutions per D-22"
    - "human-uat-as-runbook: every scenario references an existing `just uat-*` recipe per `feedback_uat_use_just_commands.md`"
    - "out-of-scope-as-grep-gate: EXIT-06 cardinality discipline verified via static grep + runtime /metrics scrape, not via positive code"
    - "maintainer-only-tag-cut: D-26 — Claude authors the runbook, maintainer runs it (`feedback_uat_user_validates.md`)"

key-files:
  created:
    - ".planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-HUMAN-UAT.md (shipped in PR #55, 2eea055)"
    - ".planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-RC2-PREFLIGHT.md (shipped in PR #55, 2eea055)"
    - ".planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-11-SUMMARY.md (this file)"
  modified: []

key-decisions:
  - "Plan 21-11 deliverables already shipped in PR #55 (2eea055) ahead of this SUMMARY — branch phase-21/uat-and-rc2-preflight is even with main; this SUMMARY is the missing traceability artifact, not new work."
  - "EXIT-06 grep target corrected: src/metrics.rs (plan template) → src/telemetry.rs (actual path in this repo). The metrics module was renamed in an earlier phase; intent unchanged. Captured in-line in both as-shipped documents."
  - "Scenario 6 (FCTX-06 fire-skew) verified via hand-seeded +30000ms row on the maintainer host (no Docker daemon available). Real-container fire-skew is exercised by tests/v12_fctx_explain.rs in CI."
  - "DO NOT overwrite the maintainer-signed UAT with the plan's literal template. Preserving the 2026-05-02 sign-off + 3 hotfix references is required by `feedback_uat_user_validates.md` (the rule covers both marking AND un-marking maintainer sign-off)."

patterns-established:
  - "Plan-already-shipped close-out: when a plan's deliverables land in a prior PR, the executor writes a SUMMARY that records the fact + cites the shipping commit, rather than re-creating files from the literal plan template."
  - "Sign-off preservation: a maintainer-signed UAT artifact is immutable from Claude's side — neither overwrite nor un-tick is permitted."

requirements-completed: [FCTX-01, FCTX-02, FCTX-03, FCTX-05, FCTX-06, EXIT-01, EXIT-02, EXIT-03, EXIT-04, EXIT-05, EXIT-06]

# Metrics
duration: 0min (no implementation work — SUMMARY-only close-out)
completed: 2026-05-03
---

# Phase 21 Plan 11: rc.2 UAT + Preflight Close-Out Summary

**Records that the rc.2 maintainer UAT (`21-HUMAN-UAT.md`, 8 scenarios + EXIT-06 cardinality grep + D-32 rustls spot check) and the v1.2.0-rc.2 tag-cut runbook (`21-RC2-PREFLIGHT.md`) shipped in PR #55 (commit `2eea055`) ahead of this SUMMARY, with the maintainer sign-off captured 2026-05-02 by Robert Wlodarczyk and three documented hotfix-during-walkthrough commits (`7b3e38d`, `b7c42e9`, `06235ae`).**

## Performance

- **Duration:** 0 min implementation (this is a SUMMARY-only close-out — the deliverables already shipped in PR #55).
- **Started:** 2026-05-03T (executor invocation)
- **Completed:** 2026-05-03
- **Tasks:** 2 plan tasks (both deliverables already shipped in PR #55) + 1 SUMMARY commit on this branch
- **Files modified:** 1 (`21-11-SUMMARY.md` only — no other paths touched)

## Accomplishments

- **`21-HUMAN-UAT.md`** (221 lines, shipped in PR #55) — 8-scenario maintainer-validated UAT covering FCTX-01..06 + EXIT-01..06, references all 4 plan-21-10 recipes (`uat-fctx-panel`, `uat-exit-histogram`, `uat-fire-skew`, `uat-fctx-a11y`), includes EXIT-06 cardinality grep verification (out-of-scope sanity check), `cargo tree -i openssl-sys` D-32 final gate, and a sign-off block. **Maintainer-signed 2026-05-02.**
- **`21-RC2-PREFLIGHT.md`** (205 lines, shipped in PR #55) — v1.2.0-rc.2 tag-cut runbook with 9 numbered sections: plans-merged checklist, CI matrix gate, rustls invariant, release.yml `:latest` gate audit (D-24), EXIT-06 cardinality, git-cliff preview, UAT sign-off cross-reference, **literal tag command per D-23** (`git tag -a -s v1.2.0-rc.2 -m "v1.2.0-rc.2 — FCTX UI panel + exit-code histogram (P21)"`), and post-publish verification with `:latest` invariant digest comparison.
- **Maintainer sign-off verbatim (from `21-HUMAN-UAT.md` § Sign-off):** Robert Wlodarczyk <robert@simplicityguy.com>, Date 2026-05-02 (UTC). Comment: *"All 8 scenarios + D-32 + `/metrics` runtime scrape passed on `phase21/ui-spec`."*

## Maintainer-Discovered Hotfix Commits (during UAT walkthrough)

Three unblockers landed on `phase21/ui-spec` while the maintainer was walking the scenarios. All three are documented inside the as-shipped UAT sign-off block:

| SHA | What it fixed |
|-----|---------------|
| `7b3e38d` | `examples/cronduit.toml` `fire-skew-demo` job schema correction: `command = [...]` → `cmd = [...]`, removed unsupported `type` field. Required for Scenario 6 (FCTX-06 fire-skew). |
| `b7c42e9` | Pinned `DATABASE_URL=sqlite://./cronduit.dev.db?mode=rwc` in `just dev` / `just dev-ui` so all justfile recipes share the same SQLite file. Required for `just db-reset` + `just dev` to operate on the same DB across Scenarios 1, 2, 5. |
| `06235ae` | Corrected EXIT-06 grep target `src/metrics.rs` → `src/telemetry.rs` in both `21-HUMAN-UAT.md` and `21-RC2-PREFLIGHT.md` (the metrics module was renamed in an earlier phase; intent unchanged). |

A subsequent rc.2 hotfix landed on `main` after PR #55: PR #56 (`2d87ee3`) — rustfmt drift + 2 `doc_lazy_continuation` clippy errors. Not part of plan 21-11 but referenced here for rc.2 lineage completeness.

## Task Commits

This plan's deliverables shipped ahead of this SUMMARY:

1. **Task 1: Create 21-HUMAN-UAT.md** — landed in `2eea055` (PR #55), maintainer-signed 2026-05-02
2. **Task 2: Create 21-RC2-PREFLIGHT.md** — landed in `2eea055` (PR #55)

**Plan metadata commit (this branch, `phase-21/uat-and-rc2-preflight`):** `docs(21-11): add SUMMARY recording rc.2 UAT + preflight already shipped in PR #55`

## Files Created/Modified

- `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-HUMAN-UAT.md` — created in PR #55 (`2eea055`); not touched by this branch.
- `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-RC2-PREFLIGHT.md` — created in PR #55 (`2eea055`); not touched by this branch.
- `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-11-SUMMARY.md` — created by this branch (the missing traceability artifact).

## Decisions Made

- **Refused the destructive plan-template overwrite.** The plan template body (lines ~123-269 / ~316-384 of `21-11-PLAN.md`) is a strict subset of the as-shipped documents and is *unsigned*. Writing the literal template would have erased the maintainer's 2026-05-02 sign-off, the three hotfix references, the `:latest` invariant digest-comparison procedure, the "What if UAT fails" rollback policy, and the documented `src/metrics.rs` → `src/telemetry.rs` correction. Project memory `feedback_uat_user_validates.md` covers both marking AND un-marking maintainer sign-off — the existing artifact is immutable from Claude's side.
- **Recorded the rename `src/metrics.rs` → `src/telemetry.rs` as a documented deviation.** The plan-template grep target is stale; the actual metrics module in this repo is `src/telemetry.rs`. Both as-shipped artifacts already note this in-line; this SUMMARY surfaces it for traceability.

## Deviations from Plan

### Rule 4 — Architectural decision to NOT execute the literal plan templates

- **Found during:** Pre-Task-1 read of the existing files.
- **Issue:** The plan's `<action>` blocks instructed creation of two files that **already exist on `main`** (PR #55, `2eea055`) with substantively richer content than the templates and with the maintainer's `[x]` sign-off recorded 2026-05-02. The branch `phase-21/uat-and-rc2-preflight` was even with `main` at start (no commits to land).
- **Resolution:** Halted and surfaced a Rule-4 checkpoint to the orchestrator with three options: (A) write a SUMMARY-only close-out, (B) abandon branch, (C) destructive overwrite. The orchestrator approved Option A.
- **Files modified by this resolution:** Only `21-11-SUMMARY.md` (this file).
- **Verification:** All `<acceptance_criteria>` greps from both Task 1 and Task 2 already PASS against the as-shipped files (HUMAN-UAT: 10 recipe references vs ≥4 required, 16 FCTX/EXIT IDs vs ≥8, 0 forbidden recipes; RC2-PREFLIGHT: literal tag command exact, 19 pre-flight references vs ≥4, 3 "DO NOT modify" lines, 6 `:latest` gating references). Re-run available in the executor's checkpoint message.
- **Committed in:** N/A (the deviation was a *non-action* — preserving signed-off work).

### Rule 1 — Bug fix to plan template (already applied in PR #55)

- **Found during:** PR #55 implementation (pre-this-SUMMARY).
- **Issue:** Plan template hardcoded `src/metrics.rs` for the EXIT-06 grep verification; that file does not exist in this repo. The actual metrics module is `src/telemetry.rs`.
- **Fix:** Both as-shipped artifacts use the correct path and document the rename in-line.
- **Files modified:** `21-HUMAN-UAT.md` § Scenario 8, `21-RC2-PREFLIGHT.md` § 5.
- **Verification:** `grep -rn 'exit_code' src/telemetry.rs` returns empty (per maintainer 2026-05-02 walk).
- **Committed in:** `06235ae` (referenced in the as-shipped UAT sign-off block).

---

**Total deviations:** 1 architectural (refused destructive overwrite — orchestrator-approved Option A) + 1 bug-fix (`src/metrics.rs` → `src/telemetry.rs`, already shipped in PR #55).
**Impact on plan:** Zero scope creep. Plan deliverables exist, exceed acceptance criteria, and are maintainer-signed; this SUMMARY is the only artifact this branch authors.

## Issues Encountered

None during this branch's execution. The maintainer's UAT walk on PR #55 surfaced three issues (the hotfixes `7b3e38d`, `b7c42e9`, `06235ae`) which were resolved before sign-off; those are recorded in the UAT itself.

## User Setup Required

None — no environment variables or external services are introduced by plan 21-11.

## Open Follow-Ups (Maintainer-Only)

These actions are **maintainer-only per D-26** (Claude does not run `git tag` and does not mark UAT passed):

1. **Cut `v1.2.0-rc.2`** per `21-RC2-PREFLIGHT.md` § 8 (Tag Cut Command). The literal command is locked:
   ```
   git tag -a -s v1.2.0-rc.2 -m "v1.2.0-rc.2 — FCTX UI panel + exit-code histogram (P21)"
   ```
2. **Verify post-publish** per `21-RC2-PREFLIGHT.md` § 9: GHCR multi-arch manifest, `:latest` invariant (digest must equal `v1.1.0`), `:rc` rolling tag updated, `:1` / `:1.1` unchanged, healthcheck on the shipped compose stack.
3. **Update `.planning/STATE.md` + `.planning/ROADMAP.md`** to reflect Phase 21 → SHIPPED at rc.2 (orchestrator owns those writes per project workflow; not this branch's responsibility).

## Next Phase Readiness

- All 11 plans of Phase 21 have SUMMARY.md present (21-01..21-10 from PR #55 + this 21-11 close-out).
- rc.2 tag cut is gated only on the maintainer's local `git tag -a -s` invocation per the locked preflight runbook.
- No code changes pending; no blockers.

---
*Phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2*
*Plan: 11*
*Completed: 2026-05-03*

## Self-Check: PASSED

- `21-11-SUMMARY.md` written at the canonical phase path: PASS
- `21-HUMAN-UAT.md` exists on disk (referenced as already-shipped): PASS (221 lines, PR #55 / `2eea055`)
- `21-RC2-PREFLIGHT.md` exists on disk (referenced as already-shipped): PASS (205 lines, PR #55 / `2eea055`)
- Commits referenced exist in `git log --oneline --all`: `2eea055` PASS, `2d87ee3` PASS, `7b3e38d` / `b7c42e9` / `06235ae` are referenced as documented in the as-shipped UAT sign-off (verified via the file content, not as commits on this repo's main — they are part of the `phase21/ui-spec` history that landed via PR #55's squash).
- Plan acceptance criteria for both tasks already PASS against the on-disk files (verified via grep before writing this SUMMARY).
- No source code modified; no STATE.md / ROADMAP.md modified; no destructive overwrite of maintainer-signed artifact.
