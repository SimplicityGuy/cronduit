---
phase: 02-scheduler-core-command-script-executor
plan: 03
subsystem: scheduler-run-lifecycle
tags: [scheduler, run-lifecycle, cli-boot, joinset, db-queries]
dependency_graph:
  requires: [02-01, 02-02]
  provides: [run_job-task, run-lifecycle-queries, cli-boot-sequence]
  affects: [src/scheduler/run.rs, src/scheduler/mod.rs, src/db/queries.rs, src/cli/run.rs, src/db/mod.rs]
tech_stack:
  added: []
  patterns: [per-run-task-lifecycle, log-writer-micro-batch, joinset-spawn-reap, hashmap-job-lookup]
key_files:
  created:
    - src/scheduler/run.rs
  modified:
    - src/scheduler/mod.rs
    - src/db/queries.rs
    - src/db/mod.rs
    - src/cli/run.rs
decisions:
  - D-finalize-via-instant: "finalize_run uses tokio::time::Instant for duration calculation instead of re-reading start_time from DB -- avoids extra query and clock skew"
  - D-default-shebang: "Script jobs default to #!/bin/sh per D-15 since shebang is not yet in JobConfig"
  - D-jobs-hashmap: "SchedulerLoop.jobs changed from Vec to HashMap<i64, DbJob> for O(1) lookup during fire dispatch"
metrics:
  duration_seconds: 469
  completed: "2026-04-10T20:28:09Z"
  tasks_completed: 2
  tasks_total: 2
  tests_added: 7
  tests_total: 67
---

# Phase 02 Plan 03: Run Lifecycle + CLI Boot Wiring Summary

Run lifecycle orchestration connecting Plans 01 and 02 into a working end-to-end scheduler: config -> DB sync -> scheduler fires -> run task spawns -> process executes -> logs captured to DB -> run finalized.

## Task Results

### Task 1: Run lifecycle queries + run_job task
**Commit:** f124eb0

Added three DB query functions to `src/db/queries.rs`:
- `insert_running_run`: Creates job_runs row with status='running', returns run id
- `finalize_run`: Updates status, exit_code, end_time, duration_ms, error_message
- `insert_log_batch`: Batch inserts log lines into job_logs in a single transaction

Created `src/scheduler/run.rs` with the full per-run task lifecycle:
- `run_job()`: Orchestrates insert_running -> dispatch to command/script -> log writer -> finalize
- `log_writer_task()`: Drains log lines in micro-batches of 64 (D-12) and inserts to DB
- Config dispatch via `JobExecConfig` deserialized from `config_json`
- Timeout from `job.timeout_secs`; 0 treated as effectively no timeout

7 new tests covering: DB query functions, command/script run success, timeout with partial log preservation, concurrent runs creating separate rows.

### Task 2: Wire scheduler into CLI boot sequence + JoinSet integration
**Commit:** 8fee2e4

Updated `src/scheduler/mod.rs`:
- Changed `SchedulerLoop.jobs` from `Vec<DbJob>` to `HashMap<i64, DbJob>` for O(1) lookup
- Fire arm: spawns `run::run_job` into JoinSet for each due FireEntry, requeues job
- Catch-up arm: spawns `run::run_job` with trigger="catch-up" for missed fires
- Join_next arm: logs completed run status or task panic
- Cancelled arm: logs shutdown, breaks loop (Plan 04 will add drain timeout)

Updated `src/cli/run.rs` boot sequence:
- After migrate: parse timezone, sync config to DB via `sync_config_to_db`
- Startup log now uses real `sync_result.jobs.len()` and `sync_result.disabled`
- Spawns scheduler via `crate::scheduler::spawn()` before web serve
- Awaits `scheduler_handle` after web serve returns on shutdown

## Verification

- `cargo build` -- clean (exit 0)
- `cargo test --lib` -- 67 tests pass (0 failures)
- `cargo clippy --all-targets -- -D warnings` -- clean (exit 0)

## Deviations from Plan

None -- plan executed exactly as written.

## Known Stubs

None -- all functions are fully implemented.

## Self-Check: PASSED

- All 5 key files exist on disk
- Both task commits (f124eb0, 8fee2e4) found in git log
