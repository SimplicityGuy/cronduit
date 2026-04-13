---
phase: 06-live-events-metrics-retention-release-engineering
fixed_at: 2026-04-13T00:00:00Z
review_path: .planning/phases/06-live-events-metrics-retention-release-engineering/06-REVIEW.md
iteration: 1
findings_in_scope: 2
fixed: 2
skipped: 0
status: all_fixed
---

# Phase 6 Gap-Closure Code Review Fix Report

**Fixed at:** 2026-04-13
**Source review:** `.planning/phases/06-live-events-metrics-retention-release-engineering/06-REVIEW.md`
**Iteration:** 1

This report overwrites the prior 06-REVIEW-FIX.md from the original phase run, matching the 06-REVIEW.md overwrite noted in the review summary. Scope is the Phase 6 gap-closure work for plans 06-06 and 06-07 only; earlier plans 06-01..06-05 are already merged and out of scope.

**Summary:**
- Findings in scope: 2 (WR-01, WR-02) — Critical + Warning scope; 5 Info findings out of scope without `--all` flag
- Fixed: 2
- Skipped: 0

## Fixed Issues

### WR-01: `setup_metrics()` fallback path returns a handle that will not render facade-recorded metrics

**Files modified:** `src/telemetry.rs`
**Commit:** 71a03a6
**Applied fix:** Replaced the `install_recorder() -> build_recorder().handle()` fallback with a `OnceLock<PrometheusHandle>`-memoized installer. The first call eagerly installs the recorder with the configured histogram buckets, runs every `describe_*` / zero-observation call through the global `metrics::` facade, and stores the resulting handle; subsequent calls return a clone of that same handle. This eliminates the latent footgun where the fallback branch returned a detached handle disconnected from the global facade (rendering an empty body), and also prevents the silent histogram-bucket-config regression the old fallback had. Verified with `cargo check --all-targets` (clean, no warnings).

### WR-02: `retention_pruner_emits_startup_log_on_spawn` uses a wall-clock sleep that can race under CI starvation

**Files modified:** `tests/retention_integration.rs`
**Commit:** 9b54a50
**Applied fix:** Replaced the fixed `tokio::time::sleep(Duration::from_millis(50))` with a bounded-poll loop that scans the captured tracing buffer for `"retention pruner started"` every 10 ms and panics after 5 s with the captured contents included in the diagnostic message. Happy-path latency stays ~10 ms (the startup line is synchronous and runs before the first `.await`, so it lands essentially immediately once the task is actually polled); the upper bound is now explicit and large enough to survive GitHub shared-runner starvation. Verified via `cargo test --test retention_integration retention_pruner_emits_startup_log_on_spawn` (1 passed, 0.02 s).

---

_Fixed: 2026-04-13_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
