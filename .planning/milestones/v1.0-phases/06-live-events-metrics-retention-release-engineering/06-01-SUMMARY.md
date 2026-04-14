---
phase: 06-live-events-metrics-retention-release-engineering
plan: 01
subsystem: web, scheduler
tags: [sse, htmx, broadcast, tokio, streaming, real-time]

# Dependency graph
requires:
  - phase: 02-scheduler-core-command-script-executor
    provides: LogSender/LogReceiver pipeline, run_job lifecycle
  - phase: 03-read-only-web-ui-health-endpoint
    provides: Run Detail page, log_viewer.html partial, AppState, axum router
provides:
  - SSE log streaming endpoint at GET /events/runs/{run_id}/logs
  - Broadcast channel fan-out for active run log lines
  - Run Detail page with live SSE / static conditional rendering
  - HTMX OOB swap from live to static log viewer on run completion
  - Static log partial endpoint at GET /partials/runs/{run_id}/logs
  - SSE integration test stubs
affects: [06-02, 06-03, 06-04, 06-05]

# Tech tracking
tech-stack:
  added: [async-stream 0.3, htmx-ext-sse 2.2.2]
  patterns: [tokio::sync::broadcast per active run, SSE via axum Sse response, HTMX SSE extension for live updates]

key-files:
  created:
    - src/web/handlers/sse.rs
    - templates/partials/static_log_viewer.html
    - assets/vendor/htmx-ext-sse.js
    - tests/sse_streaming.rs
  modified:
    - Cargo.toml
    - src/web/mod.rs
    - src/web/handlers/mod.rs
    - src/web/handlers/run_detail.rs
    - src/scheduler/run.rs
    - src/scheduler/mod.rs
    - src/cli/run.rs
    - templates/pages/run_detail.html
    - templates/base.html
    - tests/health_endpoint.rs
    - tests/scheduler_integration.rs

key-decisions:
  - "Broadcast channel capacity 256 lines per active run (matches existing log pipeline capacity)"
  - "Broadcast publish happens in log_writer_task alongside DB inserts (single fan-out point)"
  - "Tasks 1+2 committed together because SSE handler and run lifecycle wiring are compilation-interdependent"
  - "HTMX SSE extension vendored at v2.2.2 for offline/airgap homelab compatibility"

patterns-established:
  - "SSE streaming: axum Sse + async-stream + tokio broadcast for real-time event delivery"
  - "Active run tracking: Arc<RwLock<HashMap<i64, broadcast::Sender>>> shared between scheduler and web layer"
  - "Conditional template rendering: is_running flag drives SSE vs static log viewer in askama templates"

requirements-completed: [UI-14]

# Metrics
duration: 21min
completed: 2026-04-12
---

# Phase 06 Plan 01: SSE Log Streaming Summary

**Real-time SSE log streaming for in-progress runs with broadcast fan-out, HTMX live-to-static transition, and auto-scrolling log viewer**

## Performance

- **Duration:** 21 min
- **Started:** 2026-04-12T20:38:34Z
- **Completed:** 2026-04-12T20:59:12Z
- **Tasks:** 4
- **Files modified:** 15

## Accomplishments
- SSE endpoint streams log lines in real time via broadcast channel per active run
- Run Detail page conditionally renders live SSE viewer (with LIVE badge, auto-scroll, placeholder) or static paginated viewer based on run status
- Run completion triggers seamless HTMX swap from live to static view without page reload (D-04)
- Slow SSE subscribers receive skip markers instead of blocking the log pipeline (D-01, T-6-02)
- All log content HTML-escaped in SSE events to prevent XSS (T-6-03)

## Task Commits

Each task was committed atomically:

1. **Tasks 1+2: SSE infrastructure + run lifecycle wiring** - `207bbf2` (feat)
2. **Task 3: Run Detail SSE/static conditional rendering** - `e9861a9` (feat)
3. **Task 4: SSE integration test stubs** - `1a8eb0f` (test)

## Files Created/Modified
- `src/web/handlers/sse.rs` - SSE log streaming handler with HTML escaping and skip markers
- `src/web/handlers/run_detail.rs` - Added is_running flag and static_log_partial endpoint
- `src/web/mod.rs` - Added active_runs to AppState, registered SSE and static log partial routes
- `src/scheduler/run.rs` - Broadcast channel creation/publish/cleanup in run lifecycle
- `src/scheduler/mod.rs` - Threading active_runs through scheduler to run_job
- `src/cli/run.rs` - Active runs initialization and passing to scheduler spawn
- `templates/pages/run_detail.html` - Conditional SSE/static rendering with auto-scroll JS
- `templates/partials/static_log_viewer.html` - Static log viewer partial for OOB swap
- `templates/base.html` - Added htmx-ext-sse.js script tag
- `assets/vendor/htmx-ext-sse.js` - Vendored HTMX SSE extension v2.2.2
- `tests/sse_streaming.rs` - Four SSE integration test stubs (todo! for Nyquist)
- `Cargo.toml` - Added async-stream dependency

## Decisions Made
- Broadcast channel capacity set to 256 (matches existing LogSender channel capacity) for consistent backpressure behavior
- Broadcast publish integrated into log_writer_task rather than a separate fan-out task, keeping the architecture simple with a single publication point
- Tasks 1 and 2 committed together because they are compilation-interdependent (SSE handler references active_runs which requires scheduler wiring)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed missing active_runs field in test AppState constructions**
- **Found during:** Tasks 1+2 (compilation verification)
- **Issue:** Adding active_runs to AppState broke health_endpoint.rs and scheduler_integration.rs test files
- **Fix:** Added test_active_runs() helper and updated all test call sites
- **Files modified:** tests/health_endpoint.rs, tests/scheduler_integration.rs
- **Verification:** cargo test passes
- **Committed in:** 207bbf2

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Auto-fix necessary for compilation correctness when adding a public field to AppState. No scope creep.

## Known Stubs

| File | Line | Reason |
|------|------|--------|
| tests/sse_streaming.rs | 15, 23, 31, 39 | Intentional todo!() stubs for Wave 0 Nyquist compliance -- test bodies to be implemented in gap closure |

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- SSE infrastructure is complete and ready for use
- Plan 02 (Prometheus metrics) can proceed independently
- Plan 03 (retention pruner) can proceed independently
- The active_runs pattern established here is the foundation for any future real-time features

## Self-Check: PASSED

All created files verified present. All commit hashes verified in git log.

---
*Phase: 06-live-events-metrics-retention-release-engineering*
*Completed: 2026-04-12*
