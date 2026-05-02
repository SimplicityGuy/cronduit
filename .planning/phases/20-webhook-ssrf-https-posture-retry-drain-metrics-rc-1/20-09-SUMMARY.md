---
phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
plan: 09
subsystem: release-engineering
tags: [release-engineering, rc-cut, preflight, maintainer-only, no-release-yml-edits, t-20-06-detection]

# Dependency graph
requires:
  - phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
    provides: Plans 01-08 merged on main (DLQ, retry, HTTPS validator, drain budget, metrics, config, docs, UAT)
  - phase: 12-docker-healthcheck-rc-1-cut
    provides: docs/release-rc.md runbook (REUSED VERBATIM per D-28/D-30) + release.yml hyphen-gate at line 134/135 (P12 D-10 — gates :latest skip on rc tags)
provides:
  - 20-RC1-PREFLIGHT.md — 8-section maintainer-validated checklist gating the v1.2.0-rc.1 tag cut
  - Explicit T-20-06 detection step (Section 8): :latest digest must equal :1.1.0 digest post-publish
  - Documented (NOT executed) tag command: git tag -a -s v1.2.0-rc.1 -m "v1.2.0-rc.1 — webhook block (P15..P20)"
  - Sign-off block capturing: maintainer name, date, tag SHA, amd64/arm64/:latest/:1.1.0 digests
affects: [post-rc.1-phases, P21-FCTX-UI, P22-tagging-schema, v1.2-final-ship]

# Tech tracking
tech-stack:
  added: []  # No code or dependency changes — pure release-engineering checklist
  patterns:
    - "Pre-flight checklist as Claude-authored / maintainer-validated artifact (mirrors v1.1 P12 release-rc.md precedent)"
    - "T-20-06 detection via post-publish digest comparison (programmatic check that hyphen-gate held)"
    - "Two-task split: autonomous authoring (Task 1) + checkpoint:human-action tag cut (Task 2)"

key-files:
  created:
    - .planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-RC1-PREFLIGHT.md
  modified: []

key-decisions:
  - "D-28/D-30 enforced: Phase 20 makes ZERO edits to release.yml, cliff.toml, or docs/release-rc.md — checklist references the existing v1.1 runbook verbatim"
  - "D-13/D-29 enforced: tag is cut LOCALLY by the maintainer (their GPG key is the trust anchor), not via workflow_dispatch — Claude documents the command but does not execute it"
  - "T-20-06 detection codified as a programmatic shell snippet (digest comparison) in Section 8, not just a visual check — operator runs the snippet and gets explicit OK/FAIL output"
  - "Task 2 (maintainer tag cut + sign-off) deferred to maintainer per parallel-executor instructions; this SUMMARY documents Task 1 done and Task 2 pending"

patterns-established:
  - "rc-cut preflight artifact: 8 sections (plans-merged → CI green → rustls invariant → release.yml gate intact → git-cliff preview → UAT sign-off → tag command → post-publish T-XX-YY detection) + Sign-off block"
  - "Reuse-verbatim discipline: v1.1's release engineering files (release.yml/cliff.toml/release-rc.md) are stable across rc cuts; new phases reference them, never edit them, until the milestone formally promotes them (Phase 24 for cargo-deny)"

requirements-completed: []  # Plan 09 has no requirements in its frontmatter (release-engineering checklist; not a feature requirement)

# Metrics
duration: ~2min
completed: 2026-05-01
---

# Phase 20 Plan 09: v1.2.0-rc.1 Pre-Flight Checklist Summary

**Authored 20-RC1-PREFLIGHT.md — the 8-section maintainer-validated checklist that gates the v1.2.0-rc.1 tag cut, with explicit T-20-06 detection (post-publish `:latest` ↔ `:1.1.0` digest comparison) codifying that the hyphen-gate at release.yml:134 held.**

## Performance

- **Duration:** ~2 min (Task 1 only; Task 2 deferred to maintainer)
- **Started:** 2026-05-01T22:22:28Z
- **Completed:** 2026-05-01T22:24:10Z (Task 1 + SUMMARY)
- **Tasks executed:** 1 of 2 (Task 2 is `checkpoint:human-action` — see Deferred Work below)
- **Files modified:** 1 created (20-RC1-PREFLIGHT.md)

## Accomplishments

- `20-RC1-PREFLIGHT.md` authored with all 8 numbered sections + Sign-off block per D-13/D-29 structure.
- Section 1: 8 plan-merged checkboxes (one per Plan 01..08).
- Section 2: 7 CI matrix checkboxes (amd64/arm64 × SQLite/Postgres + webhook-interop + cargo-deny + compose-smoke).
- Section 3: rustls invariant verification command (`cargo tree -i openssl-sys` returns empty — D-38).
- Section 4: visual-confirm release.yml lines 134/135 still gate `:latest` and `:rc` correctly (Pitfall 9 / T-20-06 visual gate; D-30 — no edits expected).
- Section 5: git-cliff release-notes preview command for P15..P20 commits (D-31 — auto-generated, no hand-editing).
- Section 6: 20-HUMAN-UAT.md sign-off gate (Plan 08 prerequisite).
- Section 7: documented tag command — `git tag -a -s v1.2.0-rc.1 -m "v1.2.0-rc.1 — webhook block (P15..P20)"` (NOT executed by Claude per D-13/D-29).
- Section 8: explicit `LATEST_DIGEST == V1_1_0_DIGEST` shell snippet codifies T-20-06 detection as a programmatic OK/FAIL check rather than a visual one.
- Sign-off block captures: Maintainer name, Date, Tag commit SHA, amd64 digest, arm64 digest, `:latest` digest, `:1.1.0` digest.

## Task Commits

1. **Task 1: Author 20-RC1-PREFLIGHT.md** — `b8ca2e6` (docs)

_Note: Task 2 (maintainer tag cut + sign-off) is `checkpoint:human-action` and is intentionally NOT executed by Claude. See "Deferred Work" below._

## Files Created/Modified

- `.planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-RC1-PREFLIGHT.md` — 8-section maintainer-validated preflight checklist gating the v1.2.0-rc.1 tag cut. References `docs/release-rc.md` (REUSED VERBATIM from v1.1 — D-28/D-30) for the full tag-cut runbook.

## Decisions Made

None new — Plan 09 follows the locked D-13/D-28/D-29/D-30/D-31/D-32/D-36 decisions from `20-CONTEXT.md`. Specifically:

- **D-13 / D-29** — tag cut is a maintainer-local action (their GPG key is the trust anchor); Claude documents but does not execute.
- **D-28 / D-30** — Phase 20 makes NO edits to `release.yml`, `cliff.toml`, or `docs/release-rc.md`. Verified: only `.planning/phases/20-.../20-RC1-PREFLIGHT.md` was created in this plan.
- **D-31** — GitHub Release body is auto-generated by git-cliff post-publish; do NOT hand-edit.
- **D-32** — tag is cut on `main` AFTER all Phase 20 plans merged via PR (Plan 09 itself merges via PR; the maintainer cuts the tag from `main` post-merge per `feedback_no_direct_main_commits.md`).

## Deviations from Plan

None — Task 1 executed exactly as written in the plan's `<action>` block. The Write tool produced the file verbatim from the structure specified in the plan.

## Issues Encountered

None.

## Deferred Work

**Task 2 — Maintainer cuts v1.2.0-rc.1 tag locally + verifies GHCR publish + signs off** is `checkpoint:human-action` and is explicitly NOT executed by this parallel executor agent (per the `<parallel_execution>` directive: "Plan 09's Task 2 is the human cutting the tag — DO NOT execute it. Just produce the checklist artifact (Task 1).").

What the maintainer must do (per `20-RC1-PREFLIGHT.md` and the plan's `<how-to-verify>` block):

1. Review the checklist for accuracy.
2. Run each verification step locally (`gh` CLI, `cargo`, `git cliff`, etc.) and tick boxes in Sections 1-6.
3. Confirm `20-HUMAN-UAT.md` (Plan 08) is signed off (Section 6).
4. Cut the tag locally with `git tag -a -s v1.2.0-rc.1 -m "v1.2.0-rc.1 — webhook block (P15..P20)"` and `git push origin v1.2.0-rc.1` (Section 7 — manual maintainer action; Claude cannot use the maintainer's GPG key).
5. Wait ~10-20 min for `release.yml` to publish to GHCR on amd64 + arm64.
6. Run Section 8's T-20-06 detection shell snippet (`LATEST_DIGEST == V1_1_0_DIGEST` comparison) — must print `OK: T-20-06 mitigation verified`.
7. Fill in the Sign-off block (name, date, tag SHA, amd64/arm64/:latest/:1.1.0 digests).

The plan's `<resume-signal>` describes the post-cut signal: "approved" once the tag is cut and digests verified, or "fix:<description>" if any preflight section fails (which would trigger a hotfix PR before re-tagging).

## Self-Check

Verifying claims:

- File exists: `.planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-RC1-PREFLIGHT.md` — FOUND
- Commit exists: `b8ca2e6` — FOUND
- File contains 8 numbered sections (`## 1.` .. `## 8.`) + `## Sign-off` — FOUND (10 `## ` headers total: 8 numbered + Sign-off + Tag command preamble in Section 7)
- File contains `git tag -a -s v1.2.0-rc.1` literal — FOUND (1 occurrence)
- File contains `release.yml:134` and `release.yml:135` references — FOUND (3 references in total: lines 134, 135, plus the verification grep snippet)
- File contains `LATEST_DIGEST == V1_1_0_DIGEST` comparison — FOUND (1 occurrence)
- Sign-off block has placeholders for Maintainer/Date/Tag SHA/amd64/arm64/:latest/:1.1.0 digests — FOUND (all 6 underscored placeholder lines present)

## Self-Check: PASSED

## Next Phase Readiness

- The checklist is ready for the maintainer to walk through.
- Phase 20 plan-set authoring is complete; all 9 plans have summaries on disk.
- The maintainer's next action: review `20-RC1-PREFLIGHT.md`, run through Sections 1-6 verifications, sign off `20-HUMAN-UAT.md` if not already, cut the v1.2.0-rc.1 tag (Section 7), and run the T-20-06 detection (Section 8) post-publish.
- After tag cut + sign-off: Phase 21 (FCTX UI + Exit history) starts the next planning round (depends on rc.1 baseline + P16 FCTX schema).

---
*Phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1*
*Completed: 2026-05-01 (Task 1 + SUMMARY; Task 2 awaits maintainer)*
