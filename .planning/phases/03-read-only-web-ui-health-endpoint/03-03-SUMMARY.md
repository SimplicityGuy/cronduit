---
phase: 03-read-only-web-ui-health-endpoint
plan: 03
subsystem: ui
tags: [dashboard, htmx, askama, polling, filter, sort, design-system]

# Dependency graph
requires:
  - phase: 03-read-only-web-ui-health-endpoint
    plan: 01
    provides: "base.html template, CSS custom properties, rust-embed asset serving, HTMX vendored"
  - phase: 03-read-only-web-ui-health-endpoint
    plan: 02
    provides: "get_dashboard_jobs query, DashboardJob struct, AppState with pool/cmd_tx/config_path"
provides:
  - "Dashboard page at GET / with full job table"
  - "HTMX-swappable table partial at GET /partials/job-table"
  - "DashboardJobView with computed next-fire and last-run relative times"
  - "Filter by name with debounced HTMX request (300ms)"
  - "Sortable columns (Name, Next Fire, Status, Last Run) with URL-preserved state"
  - "3-second auto-polling on table body for live refresh"
  - "Empty state with config path hint"
affects: [03-04, 03-05]

# Tech tracking
tech-stack:
  added: []
  patterns: [askama_web WebTemplateExt for IntoResponse, HxRequest extractor for partial detection, croner next_fire computation in view model, relative time formatting]

key-files:
  created: [src/web/handlers/dashboard.rs, templates/pages/dashboard.html, templates/partials/job_table.html]
  modified: [src/web/handlers/mod.rs, src/web/mod.rs, Cargo.toml]

key-decisions:
  - "Added axum query feature to Cargo.toml -- required for axum::extract::Query with default-features=false"
  - "Used askama_web::WebTemplateExt::into_web_template() instead of derive macro for IntoResponse on templates"
  - "Inline sort header HTML instead of askama macros -- askama macro system does not support let with conditional expressions"

patterns-established:
  - "HTMX partial pattern: same handler for full page and partial, HxRequest(bool) distinguishes response type"
  - "View model pattern: DashboardJob (DB) -> DashboardJobView (template) with computed fields"
  - "Relative time formatting: format_relative_future/format_relative_past helper functions"

requirements-completed: [UI-06, UI-07, UI-13]

# Metrics
duration: 9min
completed: 2026-04-11
---

# Phase 3 Plan 03: Dashboard Page & HTMX Job Table Summary

**Dashboard handler with askama templates, HTMX 3s polling table partial, filter/sort with URL state, next-fire via croner, and design system token styling**

## Performance

- **Duration:** 9 min
- **Started:** 2026-04-11T00:30:52Z
- **Completed:** 2026-04-11T00:39:51Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Dashboard handler at GET / serves full page (DashboardPage template) and at GET /partials/job-table serves table partial (JobTablePartial template)
- HxRequest extractor from axum-htmx distinguishes full page vs HTMX partial responses
- DashboardJobView computes next-fire time using croner::Cron::find_next_occurrence and formats as relative time (e.g., "in 4h 12m")
- Last-run relative time formatted as "2m ago", "3h ago", "1d 5h ago", "never" etc.
- Filter input with HTMX debounced request (keyup changed delay:300ms)
- Sortable column headers (Name, Next Fire, Status, Last Run) with asc/desc toggle and URL push
- 3-second auto-polling on tbody for live table refresh with hx-include preserving filter/sort state
- Empty state shows "No jobs configured" with config path hint
- Job table partial includes status badges (cd-badge--success/failed/running/timeout/error), Run Now button with CSRF token
- 4 unit tests for relative time formatting (future, past, days, just-now)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create dashboard handler with askama templates** - `e3ba9f9` (feat)
2. **Task 2: Create dashboard and job table templates with HTMX polling** - `28a3448` (feat)

## Files Created/Modified
- `src/web/handlers/dashboard.rs` - Dashboard handler with DashboardPage, JobTablePartial, DashboardJobView, relative time helpers, 4 unit tests
- `src/web/handlers/mod.rs` - Added `pub mod dashboard`
- `src/web/mod.rs` - Replaced placeholder index handler with dashboard routes (GET / and GET /partials/job-table), removed unused StatusCode import
- `Cargo.toml` - Added `query` feature to axum (required for `axum::extract::Query` with default-features=false)
- `templates/pages/dashboard.html` - Full dashboard page extending base.html with filter bar, sortable table headers, HTMX polling, empty state
- `templates/partials/job_table.html` - HTMX-swappable table rows with status badges, Run Now button, CSRF token

## Decisions Made
- **axum query feature:** `axum::extract::Query` requires the `query` feature flag which was not enabled since axum was configured with `default-features = false`. Added `"query"` to the features list.
- **WebTemplateExt over derive:** Used `askama_web::WebTemplateExt::into_web_template()` method to get `IntoResponse` on template structs, rather than adding a separate `#[derive(WebTemplate)]` which would require importing the derive macro.
- **Inline sort headers:** Askama's macro system does not support `let` with inline conditional expressions. Replaced the planned macro-based sort headers with inline `{% if %}` conditionals for each sortable column.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added axum query feature to Cargo.toml**
- **Found during:** Task 1
- **Issue:** `axum::extract::Query` not available with `default-features = false`
- **Fix:** Added `"query"` to axum features in Cargo.toml
- **Files modified:** Cargo.toml
- **Committed in:** e3ba9f9 (Task 1)

**2. [Rule 3 - Blocking] Fixed askama template IntoResponse**
- **Found during:** Task 1
- **Issue:** Askama Template structs don't impl IntoResponse directly; need askama_web integration
- **Fix:** Added `use askama_web::WebTemplateExt` and called `.into_web_template().into_response()`
- **Files modified:** src/web/handlers/dashboard.rs
- **Committed in:** e3ba9f9 (Task 1)

**3. [Rule 3 - Blocking] Replaced askama macros with inline conditionals**
- **Found during:** Task 2
- **Issue:** Askama macro system does not support `{% let %}` with inline if-else expressions for sort header logic
- **Fix:** Wrote sort headers inline with `{% if %}` blocks instead of macro calls
- **Files modified:** templates/pages/dashboard.html
- **Committed in:** 28a3448 (Task 2)

---

**Total deviations:** 3 auto-fixed (all blocking)
**Impact on plan:** All fixes were necessary for compilation. No scope creep.

## Known Stubs
- **CSRF token:** `src/web/handlers/dashboard.rs` line ~198 generates a random hex string as placeholder CSRF token. Plan 05 will wire the full CSRF middleware.
- **Timezone:** `src/web/handlers/dashboard.rs` line ~192 hardcodes `chrono_tz::UTC` for next-fire calculation. Will be wired to config `[server].timezone` in future refinement.

## Issues Encountered
None beyond the auto-fixed deviations above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Dashboard page renders at GET / with full job table
- HTMX partial at GET /partials/job-table ready for 3s polling
- Plans 03-04 (Job Detail) can link from dashboard table rows (`/jobs/{{ job.id }}`)
- Plan 03-05 (Run Now) can wire the `hx-post="/api/jobs/{{ job.id }}/run"` form already in the template

## Self-Check: PASSED
