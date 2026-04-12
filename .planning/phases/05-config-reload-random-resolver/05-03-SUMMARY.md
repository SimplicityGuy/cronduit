---
phase: 05-config-reload-random-resolver
plan: 03
subsystem: scheduler-loop, web-api
tags: [reload, reroll, api, coalescing, csrf]
dependency_graph:
  requires: [05-01, 05-02]
  provides: [reload-api, reroll-api, reload-coalescing, startup-wiring]
  affects: [src/scheduler/mod.rs, src/web/handlers/api.rs, src/web/mod.rs, src/scheduler/reload.rs, src/scheduler/sync.rs]
tech_stack:
  added: []
  patterns: [oneshot-channel-rpc, reload-coalescing, csrf-double-submit]
key_files:
  created: []
  modified:
    - src/scheduler/mod.rs
    - src/web/handlers/api.rs
    - src/web/mod.rs
    - src/scheduler/reload.rs
    - src/scheduler/sync.rs
decisions:
  - Shared CsrfForm struct extracted from RunNowForm for all CSRF-protected POST handlers
  - Reload coalescing drains queued commands inline rather than re-enqueuing to avoid ordering issues
  - Reroll success sends HX-Refresh header so job detail page updates with new resolved schedule
metrics:
  duration: 7m 26s
  completed: 2026-04-12
---

# Phase 5 Plan 3: Scheduler Loop Wiring + Reload/Reroll API Summary

Reload/reroll commands wired end-to-end from web API through scheduler loop with D-09 coalescing and CSRF protection on both endpoints.

## Completed Tasks

| # | Task | Commit | Key Changes |
|---|------|--------|-------------|
| 1 | Wire Reload/Reroll coalescing + startup flow | db64506 | D-09 coalescing in Reload branch, jobs_vec update on reload/reroll, fix do_reload random_min_gap arg, fix ThreadRng Send safety |
| 2 | Create reload and reroll API handlers with routes | 82ebbcc | POST /api/reload with JSON diff + toast, POST /api/jobs/{id}/reroll with toast, both CSRF-protected, routes registered |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed do_reload missing random_min_gap argument**
- **Found during:** Task 1
- **Issue:** `reload::do_reload()` called `sync::sync_config_to_db(pool, &parsed.config)` with 2 args, but the function requires 3 (including `random_min_gap`). This was a Wave 1 bug where the sync function signature was updated but the reload caller was not.
- **Fix:** Extract `random_min_gap` from parsed config defaults and pass to sync_config_to_db.
- **Files modified:** src/scheduler/reload.rs
- **Commit:** db64506

**2. [Rule 3 - Blocking] Fixed ThreadRng !Send across await in sync.rs**
- **Found during:** Task 1 (build failure)
- **Issue:** `rand::thread_rng()` was created before an async loop with `.await` calls. `ThreadRng` is `!Send`, which violates `tokio::spawn`'s `Send` bound on the future.
- **Fix:** Scoped `rng` creation into a block that drops before any subsequent `.await` calls.
- **Files modified:** src/scheduler/sync.rs
- **Commit:** db64506

## Verification

- `cargo build` exits 0
- `cargo test` -- all tests pass
- Routes confirmed: `/api/reload` and `/api/jobs/{id}/reroll` in web/mod.rs
- CSRF validation present in both new handlers
- Reload coalescing with `try_recv()` drain loop confirmed in scheduler mod.rs

## Self-Check: PASSED
