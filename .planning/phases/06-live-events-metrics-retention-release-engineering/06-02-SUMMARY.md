---
phase: 06-live-events-metrics-retention-release-engineering
plan: 02
subsystem: observability
tags: [prometheus, metrics, metrics-exporter-prometheus, histogram, counter, gauge]

# Dependency graph
requires:
  - phase: 02-scheduler-core-command-script-executor
    provides: "Scheduler run lifecycle with finalize_run and sync_config_to_db"
provides:
  - "GET /metrics endpoint returning Prometheus text format"
  - "Four metric families: jobs_total, runs_total, run_duration_seconds, run_failures_total"
  - "FailureReason closed enum with 6 variants for cardinality control"
  - "PrometheusHandle in AppState for metrics rendering"
  - "examples/prometheus.yml scrape config"
affects: [release-engineering, docker-compose, README]

# Tech tracking
tech-stack:
  added: [metrics 0.24, metrics-exporter-prometheus 0.18]
  patterns: [metrics facade decoupled from exporter, closed-enum labels for cardinality control]

key-files:
  created:
    - src/web/handlers/metrics.rs
    - examples/prometheus.yml
    - tests/metrics_endpoint.rs
  modified:
    - Cargo.toml
    - src/telemetry.rs
    - src/web/mod.rs
    - src/web/handlers/mod.rs
    - src/scheduler/run.rs
    - src/scheduler/sync.rs
    - src/cli/run.rs
    - tests/health_endpoint.rs

key-decisions:
  - "Used PrometheusBuilder.build_recorder().handle() in tests to avoid global recorder conflict"
  - "classify_failure_reason uses starts_with/exact match on actual error_message strings from docker modules"

patterns-established:
  - "Metrics facade pattern: instrument with metrics::counter!/histogram!/gauge! macros, render via PrometheusHandle"
  - "Closed-enum labels: FailureReason with 6 fixed variants prevents cardinality explosion"

requirements-completed: [OPS-02]

# Metrics
duration: 11min
completed: 2026-04-12
---

# Phase 06 Plan 02: Prometheus Metrics Summary

**Prometheus /metrics endpoint with four metric families, homelab-tuned histogram buckets, and closed-enum failure reason labels**

## Performance

- **Duration:** 11 min
- **Started:** 2026-04-12T21:02:35Z
- **Completed:** 2026-04-12T21:13:59Z
- **Tasks:** 3
- **Files modified:** 11

## Accomplishments
- GET /metrics endpoint returning Prometheus text format with proper Content-Type header
- Four metric families instrumented at scheduler lifecycle points: cronduit_jobs_total gauge, cronduit_runs_total counter, cronduit_run_duration_seconds histogram, cronduit_run_failures_total counter
- FailureReason closed enum with 6 variants mapping actual error_message strings from docker_preflight, docker_pull, and docker_orphan modules
- Homelab-tuned histogram buckets [1, 5, 15, 30, 60, 300, 900, 1800, 3600] seconds
- examples/prometheus.yml with ready-to-use scrape config

## Task Commits

Each task was committed atomically:

1. **Task 1: Metrics facade setup + /metrics endpoint + FailureReason enum** - `ce6da92` (feat)
2. **Task 2: Instrument scheduler with metrics recording** - `54cb652` (feat)
3. **Task 3: Metrics integration test stubs** - `0ad0e8e` (test)

## Files Created/Modified
- `src/web/handlers/metrics.rs` - Prometheus /metrics HTTP handler
- `src/telemetry.rs` - PrometheusBuilder setup with custom histogram buckets
- `src/web/mod.rs` - metrics_handle field in AppState, /metrics route
- `src/web/handlers/mod.rs` - metrics module export
- `src/scheduler/run.rs` - FailureReason enum, classify_failure_reason, metrics recording at finalization
- `src/scheduler/sync.rs` - cronduit_jobs_total gauge after sync
- `src/cli/run.rs` - setup_metrics() call and scheduler_up gauge at startup
- `examples/prometheus.yml` - Ready-to-use Prometheus scrape config
- `tests/metrics_endpoint.rs` - Four integration test stubs
- `tests/health_endpoint.rs` - Fixed to include metrics_handle in AppState
- `Cargo.toml` - Added metrics and metrics-exporter-prometheus dependencies

## Decisions Made
- Used `PrometheusBuilder::new().build_recorder().handle()` in test code to avoid global recorder installation conflict (multiple tests in same process would panic)
- classify_failure_reason uses `starts_with` matching on actual error_message strings from docker modules (e.g., `"network_target_unavailable:"`, `"image pull failed:"`, `"orphaned at restart"`) rather than generic `contains` patterns

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed health_endpoint test missing metrics_handle field**
- **Found during:** Task 1
- **Issue:** Adding metrics_handle to AppState broke existing health_endpoint integration test
- **Fix:** Added PrometheusBuilder::new().build_recorder().handle() to test AppState construction (avoids global recorder conflict)
- **Files modified:** tests/health_endpoint.rs
- **Verification:** cargo test --test health_endpoint passes
- **Committed in:** ce6da92 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Necessary fix for test compilation. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Metrics endpoint ready for integration with retention pruner (plan 03) and release engineering (plans 04/05)
- examples/prometheus.yml ready for inclusion in README quickstart documentation

---
*Phase: 06-live-events-metrics-retention-release-engineering*
*Completed: 2026-04-12*

## Self-Check: PASSED

- All 3 created files exist on disk
- All 3 task commit hashes found in git log
