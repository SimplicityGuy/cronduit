---
phase: 05-config-reload-random-resolver
plan: 02
subsystem: scheduler
tags: [notify, sighup, file-watcher, config-reload, debounce]

# Dependency graph
requires:
  - phase: 02-scheduler-core-command-script-executor
    provides: scheduler loop, fire queue, SchedulerCmd channel
  - phase: 01-foundation
    provides: config parsing, DB pool, shutdown handling
provides:
  - do_reload() function for config-to-DB sync with heap rebuild
  - do_reroll() function for @random schedule re-randomization
  - spawn_file_watcher() with 500ms tokio debounce
  - install_sighup() SIGHUP handler sending SchedulerCmd::Reload
  - ReloadResult/ReloadStatus types for reload response
  - SchedulerCmd::Reload and SchedulerCmd::Reroll variants
  - watch_config field in ServerConfig (default true)
  - update_resolved_schedule DB query
affects: [05-03, 05-04, 05-05]

# Tech tracking
tech-stack:
  added: [notify 8.2]
  patterns: [tokio-based debounce for file watching, fire-and-forget oneshot for signal-triggered reload]

key-files:
  created: [src/scheduler/reload.rs]
  modified: [src/config/mod.rs, src/shutdown.rs, src/scheduler/cmd.rs, src/scheduler/mod.rs, src/db/queries.rs, src/db/mod.rs, src/cli/run.rs, src/scheduler/sync.rs, tests/scheduler_integration.rs]

key-decisions:
  - "Manual tokio debounce instead of notify-debouncer-mini -- simpler for single-file watching"
  - "Watch parent directory instead of file directly to handle rename-based atomic saves"
  - "Fire-and-forget oneshot for SIGHUP -- no caller to receive the result"
  - "Added Reload/Reroll variants to SchedulerCmd in this plan since Plan 01 runs in parallel"

patterns-established:
  - "Reload code path: parse_and_validate -> sync_config_to_db -> rebuild heap"
  - "Failed reloads return Error status without mutating DB or in-memory state"
  - "File watcher filters events by config filename, debounces at 500ms"

requirements-completed: [RELOAD-01, RELOAD-03, RELOAD-04, RELOAD-05, RELOAD-06, RELOAD-07]

# Metrics
duration: 13min
completed: 2026-04-12
---

# Phase 5 Plan 02: Config Reload Infrastructure Summary

**Config reload via SIGHUP, file watcher with 500ms debounce, and do_reload/do_reroll functions that parse config, sync to DB, and rebuild the fire heap**

## Performance

- **Duration:** 13 min
- **Started:** 2026-04-12T00:13:04Z
- **Completed:** 2026-04-12T00:26:12Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments
- Implemented do_reload() that parses config, syncs to DB, and rebuilds the scheduler fire heap; failed reloads leave running config untouched (RELOAD-04)
- Added SIGHUP handler (install_sighup) with platform-conditional compilation and fire-and-forget oneshot pattern
- Created spawn_file_watcher() with 500ms tokio debounce, parent-directory watching for atomic saves, and filename filtering
- Added SchedulerCmd::Reload and SchedulerCmd::Reroll variants with full scheduler loop handling
- Added watch_config field to ServerConfig defaulting to true

## Task Commits

Each task was committed atomically:

1. **Task 1: Add watch_config to ServerConfig and install_sighup to shutdown.rs** - `0bfccce` (feat)
2. **Task 2: Create reload.rs with do_reload(), do_reroll(), and spawn_file_watcher()** - `3e8064e` (feat)

## Files Created/Modified
- `src/scheduler/reload.rs` - Core reload functions: do_reload, do_reroll, spawn_file_watcher
- `src/shutdown.rs` - SIGHUP handler (install_sighup) with unix/non-unix variants
- `src/config/mod.rs` - watch_config field on ServerConfig with default true
- `src/scheduler/cmd.rs` - ReloadResult, ReloadStatus, Reload/Reroll SchedulerCmd variants
- `src/scheduler/mod.rs` - Added reload module, config_path to SchedulerLoop, Reload/Reroll handling in select loop
- `src/db/queries.rs` - update_resolved_schedule query for @random re-roll
- `src/db/mod.rs` - Re-export update_resolved_schedule
- `src/cli/run.rs` - Wire SIGHUP handler and file watcher at startup
- `src/scheduler/sync.rs` - Updated test helper for new watch_config field
- `tests/scheduler_integration.rs` - Updated test helper for new watch_config field
- `Cargo.toml` - Added notify 8.2 dependency

## Decisions Made
- Used manual tokio debounce (500ms sleep in select loop) instead of notify-debouncer-mini for simplicity with single-file watching
- Watch parent directory rather than file directly to handle editor atomic saves (write-then-rename)
- Added SchedulerCmd::Reload/Reroll/ReloadResult/ReloadStatus in this plan since Plan 01 runs in parallel wave and these types are needed for compilation
- do_reroll checks for "@random" in schedule string as a simple heuristic; Plan 01 will add the proper random module

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added SchedulerCmd::Reload/Reroll types and variants to cmd.rs**
- **Found during:** Task 1
- **Issue:** Plan indicated these would come from Plan 01, but Plan 01 runs in parallel (same wave). Code needs these types to compile.
- **Fix:** Added ReloadResult, ReloadStatus, and Reload/Reroll variants directly to cmd.rs
- **Files modified:** src/scheduler/cmd.rs
- **Verification:** cargo build succeeds
- **Committed in:** 0bfccce (Task 1 commit)

**2. [Rule 3 - Blocking] Added config_path to SchedulerLoop and updated spawn()**
- **Found during:** Task 1
- **Issue:** do_reload needs the config file path, but SchedulerLoop didn't have it
- **Fix:** Added config_path: PathBuf field to SchedulerLoop and updated spawn() signature and call site
- **Files modified:** src/scheduler/mod.rs, src/cli/run.rs
- **Verification:** cargo build succeeds
- **Committed in:** 0bfccce (Task 1 commit)

**3. [Rule 3 - Blocking] Updated all ServerConfig struct constructions**
- **Found during:** Task 1
- **Issue:** Adding watch_config field broke test helpers that construct ServerConfig directly
- **Fix:** Added watch_config: true to make_server_config() in sync.rs tests and scheduler_integration.rs
- **Files modified:** src/scheduler/sync.rs, tests/scheduler_integration.rs
- **Verification:** cargo test passes
- **Committed in:** 0bfccce (Task 1 commit)

**4. [Rule 2 - Missing Critical] Adapted do_reroll for missing random module**
- **Found during:** Task 2
- **Issue:** Plan references scheduler::random module and resolve_schedule() which don't exist yet (Plan 01 scope)
- **Fix:** Implemented @random check as string contains heuristic; will be replaced when Plan 01 lands random module
- **Files modified:** src/scheduler/reload.rs
- **Verification:** cargo build succeeds
- **Committed in:** 3e8064e (Task 2 commit)

---

**Total deviations:** 4 auto-fixed (3 blocking, 1 missing critical)
**Impact on plan:** All deviations necessary for compilation in parallel execution context. No scope creep. Plan 01 merge will require reconciliation of SchedulerCmd types.

## Issues Encountered
None beyond the parallel-execution deviations documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Reload infrastructure ready for Plan 03 (API handler wiring)
- Plan 01 merge will need to reconcile SchedulerCmd types and add proper @random resolution to do_reroll
- sync_config_to_db unchanged count tracking deferred (TODO in reload.rs)

---
*Phase: 05-config-reload-random-resolver*
*Completed: 2026-04-12*
