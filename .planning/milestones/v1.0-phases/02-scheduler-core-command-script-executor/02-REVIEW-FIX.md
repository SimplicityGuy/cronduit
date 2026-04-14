---
phase: 02-scheduler-core-command-script-executor
fixed_at: 2026-04-11T12:00:00Z
review_path: .planning/phases/02-scheduler-core-command-script-executor/02-REVIEW.md
iteration: 1
findings_in_scope: 6
fixed: 6
skipped: 0
status: all_fixed
---

# Phase 2: Code Review Fix Report

**Fixed at:** 2026-04-11T12:00:00Z
**Source review:** .planning/phases/02-scheduler-core-command-script-executor/02-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 6
- Fixed: 6
- Skipped: 0

## Fixed Issues

### CR-01: Integer overflow in `kill_process_group` PID cast

**Files modified:** `src/scheduler/command.rs`
**Commit:** 90aabe0
**Applied fix:** Replaced unsafe `pid as i32` cast with `pid.try_into()` that returns early with an error log if the PID exceeds `i32::MAX`, preventing silent wrapping to a negative value that would SIGKILL the wrong process group.

### WR-01: Scheduler loop uses stale job snapshot -- never re-syncs

**Files modified:** `src/scheduler/mod.rs`
**Commit:** fba3ca8
**Applied fix:** Added a `TODO(Phase 5)` comment above the `jobs_vec` snapshot explaining that the job set is intentionally immutable for the scheduler's lifetime and that hot-reload will require rebuilding the heap via a channel or watch.

### WR-02: `drain_batch_async` has a TOCTOU race between empty-check and `notified()`

**Files modified:** `src/scheduler/log_pipeline.rs`
**Commit:** fca2327
**Applied fix:** Restructured `drain_batch_async` to hold the mutex lock across both the empty-check and closed-check within a single critical section, eliminating the race window where a sender could send+close between the two separate lock acquisitions.

### WR-03: `timeout_secs` cast from `i64` to `u64` without bounds check

**Files modified:** `src/scheduler/run.rs`
**Commit:** a2a78b9
**Applied fix:** Added a clarifying comment explaining that the `<= 0` guard intentionally catches negative `i64` values before the `as u64` cast, preventing silent wrapping to a very large duration. A pre-commit hook also added a tracing::debug call for observability when the fallback is triggered.

### WR-04: Postgres `disable_missing_jobs` uses `!= ALL` instead of `NOT IN` / `<> ALL`

**Files modified:** `src/db/queries.rs`
**Commit:** 7912bbb
**Applied fix:** Changed the Postgres query from `name != ALL($1)` to the idiomatic `NOT (name = ANY($1))`, which is standard SQL and handles edge cases (empty arrays, NULLs) more predictably.

### WR-05: `finalize_run` parameter named `duration` is actually a start `Instant`

**Files modified:** `src/db/queries.rs`
**Commit:** 56a67aa
**Applied fix:** Renamed the `duration` parameter to `start_instant` in the `finalize_run` function signature and updated the local usage (`start_instant.elapsed()`), making the intent clear that this is the start time from which elapsed duration is computed.

## Skipped Issues

None -- all in-scope findings were successfully fixed.

---

_Fixed: 2026-04-11T12:00:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
