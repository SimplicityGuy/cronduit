---
phase: 05-config-reload-random-resolver
plan: 01
subsystem: scheduler
tags: [random-resolver, scheduler-cmd, sync-engine]
dependency_graph:
  requires: []
  provides: [random-resolver, scheduler-cmd-reload, sync-random-wiring]
  affects: [scheduler-loop, cli-run, sync-engine]
tech_stack:
  added: []
  patterns: [slot-based-gap-enforcement, circular-distance, tdd]
key_files:
  created:
    - src/scheduler/random.rs
  modified:
    - src/scheduler/cmd.rs
    - src/scheduler/sync.rs
    - src/scheduler/mod.rs
    - src/cli/run.rs
    - tests/scheduler_integration.rs
decisions:
  - "Slot-based gap enforcement with circular distance for @random min_gap"
  - "Infeasible gap relaxation: divide day minutes by job count instead of failing"
  - "Field count validation (T-05-01): reject malformed input, return unchanged"
  - "Retry cap at 10 for resolve, 100 for slot placement (T-05-02)"
metrics:
  duration: 13m
  completed: 2026-04-12
  tasks_completed: 2
  tasks_total: 2
  files_changed: 6
---

# Phase 5 Plan 1: @random Resolver + SchedulerCmd Extension Summary

**Implemented the @random cron field resolver with slot-based gap enforcement and wired it into the sync engine, plus extended SchedulerCmd with Reload/Reroll variants carrying oneshot response channels.**

## What Was Built

### Task 1: @random Resolver Module (`src/scheduler/random.rs`)

Created the core @random resolution module with three public functions:

- **`is_random_schedule()`** -- Detects `@random` tokens in any position of a 5-field cron schedule.
- **`resolve_schedule()`** -- Replaces `@random` tokens with random values from valid field ranges (minute 0-59, hour 0-23, etc.). Preserves existing resolved values when passed via `existing_resolved` parameter (stability across reloads). Validates results with `croner::Cron::from_str()` and retries up to 10 times if invalid.
- **`resolve_random_schedules_batch()`** -- Batch resolver with slot-based minimum gap enforcement. Sorts jobs by constraint severity (fewer @random fields = more constrained = allocated first). Includes feasibility pre-check: if `num_jobs * gap > 1440 minutes`, relaxes the gap to `1440 / num_jobs` with a WARN log. Retries up to 100 times per job to find a gap-satisfying slot; falls back to best candidate (maximum minimum distance) if exhausted.

TDD approach: RED phase committed failing tests, GREEN phase implemented all functions.

14 unit tests covering:
- Single/multiple @random field detection
- Non-random passthrough
- Existing resolved preservation (stable_across_reload)
- Gap enforcement with 3 jobs at 90-minute intervals
- Infeasible gap relaxation with 30 jobs
- croner validation of fully-random schedules
- Malformed input rejection (T-05-01)

### Task 2: SchedulerCmd Extension + Sync Engine Wiring

**cmd.rs:**
- Added `Reload { response_tx }` and `Reroll { job_id, response_tx }` variants
- Added `ReloadResult` struct with status, added/updated/disabled/unchanged counts, and optional error message
- Added `ReloadStatus` enum (Ok, Error)

**sync.rs:**
- Added `random_min_gap: Duration` parameter to `sync_config_to_db()`
- Build batch resolver input from DB state (existing resolved schedule when config_hash matches)
- Replace placeholder `let resolved_schedule = job.schedule.clone()` with batch resolver output
- Added `unchanged: u64` field to `SyncResult`

**mod.rs:**
- Added `pub mod random` declaration
- Stub handlers for Reload/Reroll in scheduler loop (Phase 5 Plan 02 implements full logic)

**cli/run.rs:**
- Extract `random_min_gap` from config defaults, pass to `sync_config_to_db`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added stub Reload/Reroll handlers in scheduler loop**
- **Found during:** Task 2
- **Issue:** Adding Reload and Reroll variants to SchedulerCmd made the match arm in `SchedulerLoop::run()` non-exhaustive, causing a compile error.
- **Fix:** Added stub handlers that return ReloadStatus::Error with "not yet implemented" message. Phase 5 Plan 02 will implement the full reload logic.
- **Files modified:** src/scheduler/mod.rs
- **Commit:** 1508d9f

**2. [Rule 3 - Blocking] Updated integration test call sites**
- **Found during:** Task 2
- **Issue:** `sync_config_to_db` signature change broke `tests/scheduler_integration.rs`.
- **Fix:** Added `Duration::from_secs(0)` third argument to all call sites.
- **Files modified:** tests/scheduler_integration.rs
- **Commit:** 1508d9f

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 (RED) | 050aaa6 | test(05-01): add failing tests for @random resolver module |
| 1 (GREEN) | 2ae9f8e | feat(05-01): implement @random cron field resolver module |
| 2 | 1508d9f | feat(05-01): extend SchedulerCmd and wire @random resolver into sync engine |

## Verification

- `cargo test random` -- 14/14 pass
- `cargo test scheduler::sync::tests` -- 6/6 pass
- `cargo build` -- exits 0 with no errors

## Self-Check: PASSED

All 6 files verified present. All 3 commits verified in git log. All 18 acceptance criteria grep checks passed.
