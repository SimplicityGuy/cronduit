---
phase: 05-config-reload-random-resolver
plan: 04
subsystem: web-ui
tags: [ui, templates, toast, settings, dashboard, random-badge, reroll]
dependency_graph:
  requires: [05-01, 05-02]
  provides: [ui-reload-surfaces, ui-random-badge, ui-reroll-button]
  affects: [templates, web-handlers, css]
tech_stack:
  added: []
  patterns: [htmx-form-post, hx-trigger-toast, arc-mutex-shared-state]
key_files:
  created: []
  modified:
    - templates/base.html
    - templates/pages/settings.html
    - templates/pages/dashboard.html
    - templates/pages/job_detail.html
    - templates/partials/job_table.html
    - assets/src/app.css
    - src/web/mod.rs
    - src/web/handlers/settings.rs
    - src/web/handlers/api.rs
    - src/web/handlers/dashboard.rs
    - src/web/handlers/job_detail.rs
    - src/cli/run.rs
    - src/scheduler/reload.rs
    - src/scheduler/sync.rs
    - tests/health_endpoint.rs
decisions:
  - "Merged schedule+resolved columns on dashboard into single Schedule column showing resolved_schedule + @random badge"
  - "ReloadState tracked via Arc<Mutex<Option<ReloadState>>> on AppState for cross-handler access"
  - "Reload and reroll API endpoints added in this plan (not deferred) to support HTMX form actions"
metrics:
  duration: 15m
  completed: 2026-04-12
---

# Phase 5 Plan 04: UI Surfaces for Reload and @random Summary

Variable-duration toast JS with persistent error dismiss, settings page with reload card/button/watcher status, dashboard @random badge, and job detail resolved schedule with Re-roll button.

## Tasks Completed

| Task | Name | Commit | Key Changes |
|------|------|--------|-------------|
| 1 | Update toast JS, AppState for reload tracking, and settings page | f2629c9 | Toast JS variable duration + error dismiss, ReloadState on AppState, settings page with reload card/button/watcher status, /api/reload and /api/jobs/{id}/reroll endpoints |
| 2 | Add @random badge to dashboard and resolved schedule to job detail | fefee19 | cd-badge--random CSS, dashboard badge, job detail resolved schedule with Re-roll button |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed sync.rs ThreadRng Send issue**
- **Found during:** Task 1
- **Issue:** `rand::thread_rng()` is not `Send` and was held across `.await` points in `sync_config_to_db`, causing compile error
- **Fix:** Scoped rng creation into a block `{ let mut rng = ...; ... }` so it drops before the next await
- **Files modified:** src/scheduler/sync.rs
- **Commit:** f2629c9

**2. [Rule 3 - Blocking] Fixed reload.rs missing random_min_gap parameter**
- **Found during:** Task 1
- **Issue:** `do_reload()` called `sync_config_to_db` without the `random_min_gap` argument (Wave 1 merge gap)
- **Fix:** Extract `random_min_gap` from the freshly parsed config inside `do_reload()` rather than requiring it as a parameter
- **Files modified:** src/scheduler/reload.rs
- **Commit:** f2629c9

**3. [Rule 3 - Blocking] Fixed health_endpoint.rs missing AppState fields**
- **Found during:** Task 1
- **Issue:** Test file constructing AppState didn't include new `last_reload` and `watch_config` fields
- **Fix:** Added the fields with default values to the test
- **Files modified:** tests/health_endpoint.rs
- **Commit:** f2629c9

**4. [Rule 2 - Missing functionality] Added missing CSS tokens**
- **Found during:** Task 1
- **Issue:** Toast JS and UI-SPEC reference `--cd-space-3`, `--cd-text-sm`, `--cd-radius-sm`, `--cd-radius-md` tokens not in CSS
- **Fix:** Added missing spacing, typography, and radius tokens to the design system CSS variables
- **Files modified:** assets/src/app.css
- **Commit:** f2629c9

**5. [Rule 2 - Missing functionality] Added reload and reroll API endpoints**
- **Found during:** Task 1
- **Issue:** Settings template and job detail template post to /api/reload and /api/jobs/{id}/reroll but no handlers existed
- **Fix:** Added `reload()` and `reroll()` handlers to api.rs with CSRF validation, scheduler channel communication, and toast response triggers
- **Files modified:** src/web/handlers/api.rs, src/web/mod.rs
- **Commit:** f2629c9

## Verification

- `cargo build` exits 0
- `cargo test` -- all 39 tests pass (7 doc, 32 unit/integration)
- Grep confirms `cd-badge--random` in both CSS and template
- Grep confirms `Re-roll Schedule` in job_detail template
- Grep confirms `Reload Config` in settings template

## Self-Check: PASSED

All files exist and both commits verified:
- f2629c9: Task 1 -- toast JS, AppState, settings page, API endpoints
- fefee19: Task 2 -- @random badge, resolved schedule display, Re-roll button
