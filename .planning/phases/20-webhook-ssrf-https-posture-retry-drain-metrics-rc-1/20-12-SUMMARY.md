---
phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
plan: 12
subsystem: docs
tags: [threat-model, ssrf, webhook, accepted-risk, words-only-stub, gap-closure, wh-08, defer-to-phase-24]

# Dependency graph
requires:
  - phase: 06
    provides: THREAT_MODEL.md skeleton (TM1–TM4 + STRIDE Summary + Changelog)
  - phase: 20-08
    provides: WH-07 HTTPS-required validator (`src/config/validate.rs::check_webhook_url`) — cited as the v1.2 mitigation code path
  - phase: 20
    provides: 20-VERIFICATION.md WH-08 finding (line 108 — "MISSING SECTION") that this plan closes
provides:
  - THREAT_MODEL.md § Threat Model 5 (Webhook Outbound) words-only stub satisfying WH-08
  - Phase 24 close-out forward pointer (markdown link to .planning/ROADMAP.md)
  - Cross-reference from TM5 → TM2 (loopback default rationale lives in TM2; TM5 does not duplicate)
  - Changelog row "Phase 20 stub | 2026-05-01" documenting the interim status
affects:
  - 24-milestone-close-out (Phase 24 owns the canonical TM5 entry — full STRIDE rows, residual-risk wording for v1.3 deferred allowlist, document Revision bump)
  - 20-VERIFICATION re-run (WH-08 truth #5 grep targets all hit ≥1 after this plan)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Threat-model stubs: when a threat is known but the canonical close-out belongs to a later phase, ship a words-only stub that names the threat, lists v-current mitigations, declares accepted residual risk, and links to the close-out phase. The stub is a holding signal — verifier greps pass, audit trail intact, no false sense of completeness."
    - "Document audit trail: the Changelog row dates the stub and names the phase; the document Revision header is bumped only by the canonical close-out (Phase 24 here). Two-tier audit (row vs. revision) preserves the trust signal that the stub is interim."

key-files:
  created:
    - .planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-12-SUMMARY.md
  modified:
    - THREAT_MODEL.md (lines 189–231 new TM5 section; line 300 new Changelog row; header Revision unchanged at line 3)

key-decisions:
  - "Stay words-only: CONTEXT.md `<domain>` says the stub is words-only and the canonical close-out belongs to Phase 24. No mermaid diagram added; the existing TM2 mermaid for the loopback boundary is cross-referenced rather than duplicated. Mermaid-only project rule trivially satisfied (no diagrams of any kind = no ASCII art risk)."
  - "Do NOT bump THREAT_MODEL.md `**Revision:**` header — anti-pattern in plan frontmatter. The Changelog row is the trust signal; the Revision bump belongs to Phase 24's milestone close-out so the document audit trail tells future readers the stub was interim."
  - "Cite `src/config/validate.rs::check_webhook_url` by exact symbol path so verifier's grep `check_webhook_url` returns ≥1; this also wires the threat model to the WH-07 enforcement code path so the doc carries the concrete rejection logic, not just prose."
  - "Defer destination allow/block-list explicitly to v1.3 with the rationale 'a half-built filter is worse than no filter.' This locks the accepted residual risk verbatim against future verification rounds."

patterns-established:
  - "Stub → canonical-close-out forward-pointer pattern: stub section ends with a 'Phase N Close-Out' subsection that lists exactly what the canonical close-out replaces (STRIDE rows / residual-risk language / cross-link / Revision bump). The stub is self-describing about its interim status."

requirements-completed: [WH-08]

# Metrics
duration: 5min
completed: 2026-05-01
---

# Phase 20 Plan 12: Threat Model 5 Webhook Outbound (SSRF Accepted Risk) Summary

**Words-only Threat Model 5 stub added to THREAT_MODEL.md — closes WH-08 by enumerating the operator-with-UI-access SSRF threat, three layered v1.2 mitigations (loopback default + HTTPS validator + reverse-proxy fronting), accepted residual risk (no destination allow/block-list — deferred to v1.3), and an explicit Phase 24 close-out forward pointer.**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-05-01T23:49:45Z
- **Completed:** 2026-05-01T23:53:00Z
- **Tasks:** 1
- **Files modified:** 1 (THREAT_MODEL.md)

## Accomplishments

- THREAT_MODEL.md gains a `## Threat Model 5: Webhook Outbound (SSRF Accepted Risk)` section between TM4 (Malicious Image, line 157) and STRIDE Summary (now line 233). New section spans lines 189–231 (with the trailing `---` separator at line 232).
- The new section enumerates the three REQUIREMENTS.md WH-08 items verbatim: (a) operator-with-UI-access can configure a webhook URL pointing at any internal service; (b) cronduit is loopback-bound by default; (c) reverse-proxy fronting + operator-side auth is the v1.2 deployment expectation.
- Cites `src/config/validate.rs::check_webhook_url` (the WH-07 HTTPS-required validator) as the concrete v1.2 mitigation code path.
- Cross-references `## Threat Model 2: Untrusted Client` for the canonical loopback-default rationale (D-13) rather than duplicating it.
- Documents the v1.3-deferred destination allow/block-list filter as the accepted residual risk with concrete rationale ("a half-built filter is worse than no filter").
- Adds a "Phase 24 Close-Out" subsection with a markdown link `[Phase 24 (Milestone Close-Out)](.planning/ROADMAP.md)` listing exactly what the canonical entry replaces (STRIDE rows for `T-WH-OUT`, v1.3 filter recommendation, TM6 cross-link, Revision bump).
- Appends a `Phase 20 stub | 2026-05-01 | …` row to the Changelog table (now line 300) documenting the interim status; document `**Revision:**` header at line 3 is intentionally left at `2026-04-12 (Phase 6 -- complete)` — Phase 24 owns the next bump.

## Task Commits

Each task was committed atomically:

1. **Task 1: Insert Threat Model 5 (Webhook Outbound) words-only stub into THREAT_MODEL.md** — `526c816` (docs)

## Files Created/Modified

- `THREAT_MODEL.md` — Inserted TM5 section between TM4 (line 157) and STRIDE Summary (now line 233), spanning lines 189–231 with closing `---` at line 232; appended Changelog row at line 300. Document `**Revision:**` header at line 3 unchanged. Net diff: +45 insertions, 0 deletions.

## Decisions Made

- **Words-only is enough.** CONTEXT.md `<domain>` is explicit that the Phase 20 stub is words-only and the canonical close-out belongs to Phase 24. No mermaid diagram was added; the operator → UI → webhook flow is described in prose. This also trivially satisfies the project's mermaid-only diagram rule (no diagrams of any kind = no ASCII-art risk).
- **No `**Revision:**` header bump.** The plan's anti-patterns explicitly forbid it; Phase 24's milestone close-out owns the next revision bump. The two-tier audit trail (Changelog row dates the stub; Revision header dates the canonical close-out) preserves the trust signal that this stub is interim.
- **Cite the validator by symbol, not by line number.** `src/config/validate.rs::check_webhook_url` (today at line 436) was cited by exact symbol path so the doc doesn't break when the file shifts; this also makes the verifier's `grep -c 'check_webhook_url'` test stable across future refactors.
- **Cross-reference TM2 rather than restating loopback.** Per the plan's anti-patterns, do not duplicate the loopback-default rationale; TM2 § Mitigations already documents it. TM5 adds an in-line markdown anchor link `[Threat Model 2: Untrusted Client](#threat-model-2-untrusted-client)`.

## Deviations from Plan

None — plan executed exactly as written. The exact stub text from `<interfaces>` was inserted verbatim; the Changelog row matches the plan's Step 3 text verbatim; the document Revision header was left untouched per Step 4 / anti-pattern.

## Issues Encountered

None.

## User Setup Required

None — pure-doc gap-closure. No environment variables, no service configuration, no migrations, no version bumps, no tag work.

## Verification Evidence

All acceptance criteria from the plan pass:

| Verification grep | Required | Actual | Result |
|---|---|---|---|
| `grep -c 'Threat Model 5' THREAT_MODEL.md` | ≥1 | 2 | PASS |
| `grep -c 'Webhook Outbound' THREAT_MODEL.md` | ≥1 | 2 | PASS |
| `grep -c 'check_webhook_url' THREAT_MODEL.md` | ≥1 | 1 | PASS |
| `grep -c 'Phase 24' THREAT_MODEL.md` | ≥1 | 6 | PASS |
| `grep -c 'loopback' THREAT_MODEL.md` | ≥2 | 14 | PASS |
| `grep -cE 'reverse-proxy\|reverse proxy' THREAT_MODEL.md` | ≥1 | 9 | PASS |
| `grep -cE 'WH-08\|WH-07' THREAT_MODEL.md` | ≥1 | 3 | PASS |
| `grep -c '## Threat Model 5: Webhook Outbound' THREAT_MODEL.md` | =1 | 1 | PASS |
| `grep -cE 'allow/block-list\|allow-list\|allowlist' THREAT_MODEL.md` | ≥1 | 5 | PASS |
| `grep -c 'Phase 20 stub' THREAT_MODEL.md` | =1 | 1 | PASS |
| `grep -nE '^\*\*Revision:\*\* 2026-04-12' THREAT_MODEL.md` | =1 | 1 (line 3) | PASS — header unchanged |
| TM4 < TM5 < STRIDE in source order | OK | TM4(157) < TM5(189) < STRIDE(233) | PASS |
| `git diff --stat -- src/ migrations/ Cargo.toml Cargo.lock` | empty | empty | PASS — pure-doc plan |
| ASCII-art boxes (`+--+` style) in TM5 | none | none | PASS — words-only |

## Next Phase Readiness

- **Unblocks `/gsd-verify-phase 20` re-run** for WH-08 truth #5: the verifier's grep targets now all return ≥1 match. Combined with 20-10 (WH-06 + WH-07 alignment) and 20-11 (HUMAN-UAT enforcement on `just`), wave 8 closes all three gap-closure plans cleanly. These plans share zero files, so they ran in parallel within wave 8.
- **Pointer to Phase 24:** the Phase 24 (`Milestone Close-Out — final v1.2.0 ship`) planner reads this stub when authoring the canonical TM5 entry. The "Phase 24 Close-Out" subsection in the new TM5 lists exactly what to replace: full STRIDE rows for `T-WH-OUT` (S/T/I categories), concrete v1.3 destination filter scope recommendation, cross-link to TM6 (Operator-supplied Docker labels) which Phase 24 also closes, and the document `**Revision:**` header bump. This stub is the holding signal until Phase 24 lands.
- **No code/migration/Cargo touches**, so the rustls invariant (`cargo tree -i openssl-sys` returns empty) is trivially intact (D-38) and CI will not need a re-run for this plan.

## Self-Check: PASSED

- File `THREAT_MODEL.md` modified — verified by `git status --short` (`M  THREAT_MODEL.md`).
- Commit `526c816` exists — verified by `git rev-parse --short HEAD` (current HEAD before SUMMARY commit).
- New TM5 section heading present at line 189 — verified by `grep -n '^## ' THREAT_MODEL.md`.
- Changelog row present at line 300 — verified by `grep -n 'Phase 20 stub' THREAT_MODEL.md`.
- Document `**Revision:**` header at line 3 unchanged — verified by `grep -nE '^\*\*Revision:\*\* 2026-04-12'`.
- No source/migrations/Cargo files touched — verified by `git diff --stat -- src/ migrations/ Cargo.toml Cargo.lock` returning empty.

---
*Phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1*
*Plan: 12*
*Completed: 2026-05-01*
