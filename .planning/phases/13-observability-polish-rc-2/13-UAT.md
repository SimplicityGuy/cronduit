---
status: partial
phase: 13-observability-polish-rc-2
source:
  - 13-01-SUMMARY.md
  - 13-02-SUMMARY.md
  - 13-03-SUMMARY.md
  - 13-04-SUMMARY.md
  - 13-05-SUMMARY.md
  - 13-06-SUMMARY.md
started: 2026-04-21T20:59:24.808Z
updated: 2026-04-21T21:50:00.000Z
---

## Current Test

[testing complete — 9/11 passed, 1 skipped (Phase 14 scope), 1 blocked (post-merge maintainer action)]

## Tests

### 1. Cold Start Smoke Test
expected: Kill any running cronduit instance. Start from scratch (`cargo run` or the docker image). Server boots without errors, SQLite DB is created/migrated, and the dashboard loads at http://127.0.0.1:8080/ showing the terminal-green Cronduit UI.
result: pass

### 2. Dashboard — sparkline + success-rate badge on every job card
expected: The dashboard (http://127.0.0.1:8080/) shows a "Recent" column on each job card with a 20-cell sparkline colored by status (success/failed/timeout/cancelled/stopped/running using the existing `--cd-status-*` tokens) and a success-rate badge (e.g. `100%`, `80%`) to the right.
result: pass
note: "Initial report was against a stale Docker image in the Rancher Desktop Lima VM holding port 8080 — not against current source. Re-tested after rebuild; passes."

### 3. Dashboard — em-dash for jobs with < 5 terminal runs
expected: Any job with fewer than 5 terminal runs (newly added, recently reset, or mostly-running) shows `—` as the success-rate badge instead of a fake number. No zero-run job crashes the dashboard.
result: pass

### 4. Job detail — Duration card with p50/p95 (≥ 20 successful runs)
expected: On a job detail page (e.g. click any job card → /jobs/{name}) for a job with ≥ 20 successful runs, a "Duration" card appears between Config and Run History showing `p50: Xs` and `p95: Ys` computed over the last 100 successful runs. Values are floor-seconds (e.g. `42s`, not `42.0s`).
result: pass

### 5. Job detail — em-dash Duration for jobs with < 20 successful runs
expected: For a job with fewer than 20 successful runs, the Duration card renders `p50: —` and `p95: —` with a clarifying subtitle (e.g. "collected X / 20 samples").
result: pass

### 6. Timeline — /timeline page loads with 24h gantt view
expected: Opening http://127.0.0.1:8080/timeline renders a cross-job gantt-style chart for the last 24 hours (default window). Each bar represents a run, color-coded by status via `--cd-status-*` tokens. Running runs have a pulsing animation.
result: pass

### 7. Timeline — 7d window toggle re-renders with wider range
expected: Clicking the "7d" toggle (or visiting `/timeline?window=7d`) re-renders the page with a 7-day gantt view. Bars span more total time; more runs are visible.
result: pass

### 8. Timeline — disabled/hidden jobs are excluded
expected: Disable a job (edit config to `enabled = false` and reload) OR mark a job hidden. Reload `/timeline`. The disabled/hidden job does NOT appear in the timeline, even if it has recent runs in history.
result: skipped
reason: "Pre-Phase-14 scope — cronduit.toml `enabled = false` does not propagate to the DB; `upsert_job` hardcodes `enabled = 1` and the operator-facing `enabled_override` tri-state column is locked as Phase 14 work (v1.1 milestone decision). Phase 13's timeline filter itself is verified by integration test `tests/v13_timeline_render::disabled_jobs_excluded` which directly sets `enabled = 0` in the DB fixture."

### 9. Timeline — timestamps in configured server timezone (not UTC)
expected: With `[server].timezone = "America/Los_Angeles"` (or your configured zone) in cronduit.toml, the axis labels and run tooltips on `/timeline` show times in that zone — NOT UTC. For example, a run at 15:30 PDT should show as 15:30, not 22:30.
result: pass

### 10. Navigation — Timeline link visible in top nav on every page
expected: The base layout nav (present on Dashboard, Job detail, Settings, etc.) shows a "Timeline" link that routes to /timeline. The link is styled consistently with other nav items.
result: pass

### 11. v1.1.0-rc.2 tag cut (maintainer action — HUMAN-UAT.md runbook)
expected: After the Phase 13 PR merges to main, run the runbook in `.planning/phases/13-observability-polish-rc-2/HUMAN-UAT.md`. Confirms: tag `v1.1.0-rc.2` exists, multi-arch image pushed to GHCR, release notes published, and `:latest` remains pinned to the v1.0.1 digest (verified via `scripts/verify-latest-retag.sh 1.0.1`).
result: blocked
blocked_by: prior-phase
reason: "Tag cut is a maintainer action that runs AFTER the Phase 13 PR merges to main (Phase 12 D-13 / D-22 — trust anchor is the maintainer's signing key, not a GHA runner). Re-run /gsd-verify-work 13 after the runbook in HUMAN-UAT.md has been executed to mark this complete."

## Summary

total: 11
passed: 9
issues: 0
pending: 0
skipped: 1
blocked: 1

## Gaps

[none yet]
