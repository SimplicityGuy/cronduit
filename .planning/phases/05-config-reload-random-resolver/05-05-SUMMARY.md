---
phase: 05-config-reload-random-resolver
plan: 05
subsystem: testing
tags: [integration-tests, reload, random, sighup, file-watcher, sqlite]

requires:
  - phase: 05-config-reload-random-resolver/05-01
    provides: "@random resolver (resolve_schedule, is_random_schedule, resolve_random_schedules_batch)"
  - phase: 05-config-reload-random-resolver/05-03
    provides: "reload infrastructure (do_reload, do_reroll, spawn_file_watcher, SchedulerCmd)"
  - phase: 05-config-reload-random-resolver/05-04
    provides: "web handlers for reload/reroll, template UI surfaces"
provides:
  - "Integration tests validating full reload path end-to-end"
  - "Integration tests for @random stability across reloads"
  - "Integration test for file watcher triggering reload"
  - "Fix: config validator now accepts @random schedules"
affects: [phase-06-release]

tech-stack:
  added: []
  patterns: ["tempfile-based integration tests for config reload", "@random validation substitution pattern"]

key-files:
  created:
    - tests/reload_sighup.rs
    - tests/reload_inflight.rs
    - tests/reload_random_stability.rs
    - tests/reload_file_watch.rs
  modified:
    - src/config/validate.rs

key-decisions:
  - "Config validator substitutes @random fields with '0' for croner validation instead of skipping validation entirely"

patterns-established:
  - "@random validation: substitute @random with '0' to validate remaining fields via croner"

requirements-completed: [RELOAD-01, RELOAD-02, RELOAD-03, RELOAD-04, RELOAD-05, RELOAD-06, RELOAD-07, RAND-01, RAND-02, RAND-03, RAND-06]

duration: 8min
completed: 2026-04-12
---

# Phase 5 Plan 05: Integration Tests and Visual Checkpoint Summary

**7 integration tests covering reload flow (SIGHUP, in-flight survival, @random stability, file watcher debounce) plus config validator fix for @random schedules**

## Performance

- **Duration:** 8 min
- **Started:** 2026-04-12T00:53:04Z
- **Completed:** 2026-04-12T01:01:04Z
- **Tasks:** 1 of 2 (Task 2 is visual checkpoint -- checklist below)
- **Files modified:** 5

## Accomplishments
- 7 integration tests across 4 test files validating all Phase 5 success criteria
- Config validator fixed to accept @random schedules (substitutes with '0' for croner validation)
- Full test suite (131+ tests) passes with zero regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Create integration tests for reload and @random stability** - `d4b6074` (test)

## Files Created/Modified
- `tests/reload_sighup.rs` - SIGHUP reload: creates/updates/disables jobs + parse error leaves config untouched (RELOAD-01, RELOAD-04, RELOAD-05, RELOAD-07)
- `tests/reload_inflight.rs` - In-flight run survives reload (RELOAD-06)
- `tests/reload_random_stability.rs` - @random resolved_schedule stability when unchanged, re-randomization on change (RAND-02, RAND-03)
- `tests/reload_file_watch.rs` - File watcher triggers reload after 500ms debounce, rapid edits coalesced (RELOAD-03)
- `src/config/validate.rs` - Fixed check_schedule to accept @random fields by substituting with '0' for croner validation

## Decisions Made
- Config validator substitutes @random with '0' for croner validation rather than skipping validation entirely -- this ensures non-random fields are still validated even in @random schedules

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed config validator rejecting @random schedules**
- **Found during:** Task 1 (reload_random_stability tests)
- **Issue:** `check_schedule` in `validate.rs` passed raw schedule to `croner::Cron::parse()`, which rejected `@random` as illegal characters. Integration tests using @random schedules could not pass.
- **Fix:** When schedule contains @random tokens, substitute each @random field with '0' before croner validation. This validates remaining fields while accepting the @random placeholder.
- **Files modified:** src/config/validate.rs
- **Verification:** All 7 new integration tests pass; full test suite (131+ tests) passes with zero regressions
- **Committed in:** d4b6074 (part of Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Essential fix -- @random schedules would have been rejected at config parse time without this change. No scope creep.

## Issues Encountered
None beyond the validator fix documented above.

## Visual Checkpoint Checklist (Task 2)

Task 2 is a visual verification checkpoint. The following items require human visual confirmation:

- [ ] **Dashboard @random badge**: Jobs with @random schedules show a green `@random` badge pill next to their schedule
- [ ] **Dashboard fixed jobs**: Fixed-schedule jobs do NOT show the @random badge
- [ ] **Job Detail raw schedule**: @random job shows raw schedule `@random NN * * *` with badge
- [ ] **Job Detail resolved schedule**: "Resolved to `XX NN * * *`" appears below with a "Re-roll Schedule" button
- [ ] **Re-roll button**: Clicking "Re-roll Schedule" shows toast and changes resolved value
- [ ] **Settings page reload button**: "Reload Config" button visible in top-right header
- [ ] **Settings last reload card**: Shows "Never" initially
- [ ] **Settings reload success**: Clicking "Reload Config" shows green success toast with diff summary (auto-dismisses ~5s)
- [ ] **Settings last reload updated**: Card now shows timestamp and summary after reload
- [ ] **Settings config watcher**: "Config Watcher" card shows "WATCHING" badge
- [ ] **Settings error toast**: TOML syntax error in config produces RED error toast that does NOT auto-dismiss
- [ ] **Settings error dismiss**: Error toast can be dismissed with X button
- [ ] **Settings error state**: "Last Reload" card shows error state in red

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All Phase 5 integration tests pass
- Full test suite has zero regressions
- Visual checkpoint items documented for orchestrator to present
- Phase 5 functionality is ready for Phase 6 (Release)

---
*Phase: 05-config-reload-random-resolver*
*Completed: 2026-04-12*
