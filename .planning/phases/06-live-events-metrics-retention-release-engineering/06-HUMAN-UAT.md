---
status: pending
phase: 06-live-events-metrics-retention-release-engineering
source:
  - 06-VERIFICATION.md
  - .planning/phases/08-v1-final-human-uat-validation/08-CONTEXT.md
started: 2026-04-13T00:00:00Z
updated: 2026-04-13T00:00:00Z
---

# Phase 6 — Human UAT

**Purpose:** Close the two human-verification items carried over from
`06-VERIFICATION.md` (OPS-05 quickstart end-to-end + UI-14 SSE live log
streaming) that cannot be asserted programmatically with confidence. Phase 8
owns the walkthrough; results record in place here.

Prerequisite for both tests: Phase 8 Plans 01-04 are merged and CI
`compose-smoke` is green on both matrix axes. Without that, the fixtures are
not trustworthy.

## Current Test

[awaiting human testing]

## Tests

### 1. Quickstart end-to-end (OPS-05)
requirement: OPS-05
expected: |
  A new operator, starting from a fresh clone of the repository and a
  working Docker daemon, can run the quickstart in under 5 minutes.

  Fixture setup (run one of the two before starting the test):
    # Default variant (Linux with group_add)
    docker compose -f examples/docker-compose.yml up -d

    # Secure variant (macOS / Docker Desktop / defense-in-depth)
    docker compose -f examples/docker-compose.secure.yml up -d

  Test steps:
    1. Clone the repo fresh into a scratch directory.
    2. From the repo root, pick ONE of the two fixture commands above.
    3. Wait for /health to respond (curl http://localhost:8080/health).
    4. Open http://localhost:8080/ in a browser — the dashboard should load
       and show all four example jobs (echo-timestamp, http-healthcheck,
       disk-usage, hello-world).
    5. Within ~60 seconds, the echo-timestamp job should fire automatically
       (it's scheduled */1 * * * *); a new row should appear in its run
       history with status=success.
    6. Confirm the 5-minute budget: from `git clone` to "first echo-timestamp
       row visible with status=success", elapsed wall clock under 5 minutes.

  Record the result below. If any step fails, mark the result as an issue
  with severity (blocker / major / minor) and add reported: details.
result: [pending]

### 2. SSE live log streaming (UI-14)
requirement: UI-14
expected: |
  Triggering a long-running job shows live log streaming in the Run Detail
  view via Server-Sent Events, without a page reload, and cleanly transitions
  to the static log viewer on completion.

  Fixture setup: Same as Test 1 above (either compose file works).

  Test steps:
    1. On the dashboard, click the `http-healthcheck` or `disk-usage` job.
    2. Click "Run Now". A new run row should appear in RUNNING state.
    3. Click into the RUNNING run row to open the Run Detail page.
    4. Confirm the LIVE badge is visible next to the log viewer.
    5. Watch for log lines to stream in real time as the job runs (wget
       response headers for http-healthcheck, du/df output for disk-usage).
    6. When the job completes, the LIVE badge should disappear and the view
       should transition to the static log viewer — WITHOUT a manual page
       reload.
    7. Confirm the final log content matches what the live stream showed
       (no lost lines, no duplication).

  Record the result below. If the LIVE badge never appears, if log lines
  don't stream, if the transition requires a manual reload, or if the
  static viewer shows different content than the live stream, mark the
  result as an issue with severity and add reported: details.
result: [pending]

## Summary

total: 2
passed: 0
issues: 0
pending: 2
skipped: 0
blocked: 0

## Gaps

(User adds entries here if tests surface functional or observable gaps.
Triage rubric: see 08-HUMAN-UAT.md § Triage. Functional breakage gets a
gap entry + Phase 8 fix plan; visual/copy issues get a v1.1 BACKLOG entry.)
