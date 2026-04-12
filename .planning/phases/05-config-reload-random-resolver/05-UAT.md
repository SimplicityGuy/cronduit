---
status: complete
phase: 05-config-reload-random-resolver
source: [05-01-SUMMARY.md, 05-02-SUMMARY.md, 05-03-SUMMARY.md, 05-04-SUMMARY.md, 05-05-SUMMARY.md]
started: 2026-04-12T01:30:00Z
updated: 2026-04-12T01:45:00Z
---

## Current Test

[all tests complete]

## Tests

### 1. Cold Start Smoke Test
expected: Kill any running server. Delete cronduit.db. Run `cargo run -- run --config examples/cronduit.toml --database-url "sqlite://cronduit.db"`. Server boots without errors, migrations complete, jobs are synced, and http://127.0.0.1:8080 shows the dashboard.
result: pass

### 2. @random Badge on Dashboard
expected: The dashboard job table shows an "@random" badge pill next to any job with `@random` in its schedule (e.g., health-probe). Jobs with fixed schedules do NOT show the badge.
result: pass

### 3. Resolved Schedule on Job Detail
expected: Click a job with @random schedule. The Job Detail page shows both the raw schedule (e.g., "@random */15 * * *") and the resolved schedule (e.g., "37 */15 * * *") clearly labeled. A "Re-roll" button is visible.
result: pass

### 4. Re-roll Button Works
expected: On the Job Detail page for an @random job, click "Re-roll". A success toast appears ("Schedule re-rolled for ..."), the page refreshes, and the resolved schedule shows a new random value (different minute/hour).
result: pass

### 5. Settings Page Shows Reload Card
expected: Navigate to the Settings page. A "Config Reload" card is visible (this is the focal point). It shows watcher status (enabled/disabled) and a "Reload Config" button.
result: issue
reported: "after clicking reload, the page doesn't refresh automatically. but if i refresh it, the data is there."
severity: minor

### 6. Reload Config Button Works
expected: On Settings page, click "Reload Config". A success toast appears ("Config reloaded: 0 added, 0 updated, 0 disabled") and the reload card shows the last reload timestamp.
result: pass

### 7. Config Edit Triggers File-Watch Reload
expected: While server is running, edit examples/cronduit.toml (e.g., add a new [[jobs]] section). Within ~1 second the server logs show "file change detected, triggering reload" and the new job appears on the dashboard.
result: pass

### 8. Failed Reload Preserves Running Config
expected: Edit the config file to introduce a syntax error (e.g., delete a closing bracket). The server logs a reload error but continues serving the dashboard with the previous valid config. Fix the syntax error — the next file-watch reload succeeds.
result: pass

### 9. Toast Dismiss Behavior
expected: Success toasts auto-dismiss after ~5 seconds. Error toasts persist until manually dismissed (click X or close).
result: pass

## Summary

total: 9
passed: 8
issues: 1
pending: 0
skipped: 0
blocked: 0

## Gaps

- truth: "Settings page reload card auto-refreshes after reload to show updated timestamp"
  status: failed
  reason: "User reported: after clicking reload, the page doesn't refresh automatically. but if i refresh it, the data is there."
  severity: minor
  test: 5
  artifacts: [templates/pages/settings.html, src/web/handlers/api.rs]
  missing: [HX-Refresh or hx-swap-oob on reload response to update settings card]
