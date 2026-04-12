---
phase: 06-live-events-metrics-retention-release-engineering
reviewed: 2026-04-12T12:00:00Z
depth: standard
files_reviewed: 22
files_reviewed_list:
  - assets/vendor/htmx-ext-sse.js
  - Cargo.toml
  - examples/prometheus.yml
  - src/cli/run.rs
  - src/db/queries.rs
  - src/scheduler/mod.rs
  - src/scheduler/retention.rs
  - src/scheduler/run.rs
  - src/scheduler/sync.rs
  - src/telemetry.rs
  - src/web/handlers/metrics.rs
  - src/web/handlers/mod.rs
  - src/web/handlers/run_detail.rs
  - src/web/handlers/sse.rs
  - src/web/mod.rs
  - templates/base.html
  - templates/pages/run_detail.html
  - templates/partials/static_log_viewer.html
  - tests/health_endpoint.rs
  - tests/metrics_endpoint.rs
  - tests/retention_integration.rs
  - tests/scheduler_integration.rs
  - tests/sse_streaming.rs
findings:
  critical: 0
  warning: 5
  info: 4
  total: 9
status: issues_found
---

# Phase 6: Code Review Report

**Reviewed:** 2026-04-12T12:00:00Z
**Depth:** standard
**Files Reviewed:** 22
**Status:** issues_found

## Summary

Phase 6 adds SSE log streaming, Prometheus metrics, retention pruning, and supporting infrastructure. The core implementations are solid: SSE streaming properly handles broadcast channel semantics (lagged subscribers, closed channels), metrics are wired through the `metrics` facade correctly, and the retention pruner uses batched deletes with cancellation checks between batches.

Key concerns: (1) the `setup_metrics()` function will panic if called more than once (affects tests), (2) the `format_log_line_html` function in `sse.rs` has a subtle HTML attribute injection via the `stderr_class` variable trimming, (3) three test files contain only `todo!()` stubs that will panic at runtime, and (4) the `duration_ms` cast in `finalize_run` can overflow for very long-running jobs.

## Warnings

### WR-01: `setup_metrics()` panics on double-call -- unsafe for test harness

**File:** `src/telemetry.rs:52`
**Issue:** `install_recorder()` returns `Err` if a recorder is already installed (global singleton). The `.expect()` call will panic. In integration tests (`tests/health_endpoint.rs:17`), each test builds its own `PrometheusHandle` via `build_recorder().handle()` rather than `install_recorder()`, which works but creates an inconsistency: the test handle is disconnected from the global recorder. If any test calls `setup_metrics()` twice (or two tests run in the same process), the second call panics.
**Fix:** Use `try_install_recorder()` and handle the error gracefully, or use `build_recorder()` + `handle()` pattern consistently and set the global recorder with `once_cell`/`std::sync::Once`:
```rust
pub fn setup_metrics() -> PrometheusHandle {
    let builder = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("cronduit_run_duration_seconds".to_string()),
            &[1.0, 5.0, 15.0, 30.0, 60.0, 300.0, 900.0, 1800.0, 3600.0],
        )
        .expect("valid bucket config");

    match builder.install_recorder() {
        Ok(handle) => handle,
        Err(_) => {
            tracing::warn!("metrics recorder already installed, building detached handle");
            PrometheusBuilder::new().build_recorder().handle()
        }
    }
}
```

### WR-02: `duration_ms` cast can overflow for multi-day runs

**File:** `src/db/queries.rs:326`
**Issue:** `start_instant.elapsed().as_millis()` returns `u128`, cast to `i64`. A job running longer than ~24.8 days (2^63 ms) will overflow, producing a negative duration in the database. While unlikely for most cron jobs, a misconfigured timeout (or the 1-year fallback timeout at `src/scheduler/run.rs:121`) makes this reachable.
**Fix:** Cap the value before casting:
```rust
let duration_ms = start_instant.elapsed().as_millis().min(i64::MAX as u128) as i64;
```

### WR-03: Three test files are entirely `todo!()` stubs -- will panic on any test run

**File:** `tests/metrics_endpoint.rs:19`, `tests/retention_integration.rs:15`, `tests/sse_streaming.rs:14`
**Issue:** All test functions in these three files contain only `todo!()` macros. Running `cargo test` will compile them (they are `#[tokio::test]`), and if any test runner selects them, they will panic at runtime. This is especially problematic because CI will report these as test failures, not skips.
**Fix:** Either implement the tests, or mark them with `#[ignore]` so they are compiled but not run by default:
```rust
#[tokio::test]
#[ignore = "not yet implemented"]
async fn metrics_endpoint_returns_prometheus_format() {
    todo!("Implement metrics endpoint format test")
}
```

### WR-04: `run_detail` handler silently swallows database errors in `fetch_logs`

**File:** `src/web/handlers/run_detail.rs:102-107`
**Issue:** The `fetch_logs` function uses `.unwrap_or()` to silently replace database errors with an empty result. This means a database connectivity issue will render an empty log view with no indication of failure -- the user will see "No log output" instead of an error message. The `get_run_by_id` call on line 136 correctly propagates errors with `INTERNAL_SERVER_ERROR`.
**Fix:** Propagate the error or log it:
```rust
let log_result = match queries::get_log_lines(pool, run_id, LOG_PAGE_SIZE, offset).await {
    Ok(r) => r,
    Err(e) => {
        tracing::error!(target: "cronduit.web", run_id, error = %e, "failed to fetch log lines");
        queries::Paginated { items: vec![], total: 0 }
    }
};
```

### WR-05: Log viewer ordered DESC but appends SSE lines in arrival order

**File:** `src/db/queries.rs:798` and `src/web/handlers/sse.rs:46`
**Issue:** The database query `get_log_lines` orders logs by `id DESC` (most recent first), but the SSE handler appends new lines via `beforeend` swap (most recent last). When a live run completes and the `run_complete` event triggers a swap to the static log viewer (which re-fetches from DB ordered DESC), the log order will visually flip. Users watching a live run will see logs in chronological order, then after completion the view swaps to reverse-chronological.
**Fix:** Either change the DB query to `ORDER BY id ASC` for consistency with the live view, or reverse the SSE display order. Since chronological (ASC) is the more natural reading order for logs:
```sql
-- In get_log_lines:
ORDER BY id ASC LIMIT ?2 OFFSET ?3
```

## Info

### IN-01: Unused `notify` dependency in Cargo.toml

**File:** `Cargo.toml:121`
**Issue:** The `notify = "8.2"` dependency is listed at the top level alongside `mime_guess`, but in the reviewed files the file watcher is referenced via `crate::scheduler::reload::spawn_file_watcher` which is not in the reviewed file set. If `notify` is used only in the reload module this is fine, but it appears alongside `mime_guess` without a comment grouping, which may indicate it was added opportunistically.
**Fix:** Add a comment clarifying the dependency's purpose, consistent with the other groupings in `Cargo.toml`:
```toml
# File watching for config reload (D-10)
notify = "8.2"
```

### IN-02: `container_id_for_finalize` stores `image_digest`, not container ID

**File:** `src/scheduler/run.rs:190`
**Issue:** The variable `container_id_for_finalize` is assigned from `docker_result.image_digest`, but the database column and `finalize_run` parameter are named `container_id`. This naming mismatch will confuse future maintainers -- either the variable stores a container ID or an image digest, and the names should agree.
**Fix:** Rename to match what is actually stored, or fix the assignment to use the actual container ID if available.

### IN-03: Vendored htmx-ext-sse.js lacks version annotation

**File:** `assets/vendor/htmx-ext-sse.js:1`
**Issue:** The vendored SSE extension has no version number, commit hash, or source URL in its header comment. Per the project constraints, HTMX assets are vendored (not loaded from CDN), so knowing the exact version is important for security audits and upgrade planning.
**Fix:** Add a version comment at the top of the file:
```javascript
/* htmx-ext-sse v2.x.x - vendored from https://github.com/bigskysoftware/htmx-extensions */
```

### IN-04: `examples/prometheus.yml` suggests `!include` which is not standard Prometheus syntax

**File:** `examples/prometheus.yml:5`
**Issue:** The comment suggests using `!include examples/prometheus.yml` which is a YAML tag, not a Prometheus configuration directive. Prometheus does not support `!include`. Users following this instruction will get a configuration error.
**Fix:** Update the comment to reflect the correct usage:
```yaml
# Copy the job definition below into your existing prometheus.yml
# under the `scrape_configs:` section.
```

---

_Reviewed: 2026-04-12T12:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
