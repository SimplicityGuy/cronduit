---
phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship
plan: 06
subsystem: ui
tags: [rust, axum, askama, htmx, settings-page, audit-surface, phase-14, wave-4, erg-03]

# Dependency graph
requires:
  - phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship
    provides: "Plan 02 — jobs.enabled_override schema column (DB-14)"
  - phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship
    provides: "Plan 03 — queries::get_overridden_jobs read-side query (alphabetical, NOT-NULL filter)"
  - phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship
    provides: "Plan 04 — POST /api/jobs/bulk-toggle handler with CSRF + form-urlencoded contract"
  - phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship
    provides: "Plan 05 — .cd-badge--forced CSS selector + cd-btn-secondary action pattern"
provides:
  - "ERG-03 settings 'Currently Overridden' audit surface (table with Name | Override State | Clear)"
  - "Per-row Clear button posting to /api/jobs/bulk-toggle with action=enable + single job_ids (D-05)"
  - "Empty-state hide via {% if !overridden_jobs.is_empty() %} wrapper (D-10a)"
  - "pub SettingsPage + pub OverriddenJobView types so integration tests can render the template directly"
  - "Plan 01 scoreboard 100% green (15/15 v11_bulk_toggle SQLite tests)"
affects: [phase-14-plan-09 HUMAN-UAT, phase-14-plan-10 release notes, future v1.2 force-enable UI]

# Tech tracking
tech-stack:
  added: []  # No new crates; pure handler + askama template extension
  patterns:
    - "Co-located ViewModel pattern: OverriddenJobView lives in settings.rs alongside SettingsPage (mirrors dashboard.rs DashboardJobView precedent)"
    - "Non-fatal degradation pattern: query failure logs + returns Vec::new() so the page still renders (T-14-06-05; mirrors dashboard.rs error swallowing)"
    - "Empty-state-hides-section pattern: askama {% if !collection.is_empty() %} wrapper avoids placeholder noise (D-10a)"
    - "Per-row form posting to bulk endpoint with single job_ids — reuses Plan 04 handler contract verbatim, zero new endpoints"

key-files:
  created: []
  modified:
    - "src/web/handlers/settings.rs (+49 lines / -11 lines: pub SettingsPage, OverriddenJobView, hydration via queries::get_overridden_jobs)"
    - "templates/pages/settings.html (+57 lines: <section> with 3-column table + per-row Clear form)"

key-decisions:
  - "Made SettingsPage + all fields pub so the test in tests/v11_bulk_toggle.rs can construct a SettingsPage instance directly and call .render() to assert the empty-state-hide behavior (test author intent confirmed by the import path cronduit::web::handlers::settings::{OverriddenJobView, SettingsPage})"
  - "Filter overridden_jobs at the view-model boundary with filter_map(|j| j.enabled_override.map(...)) — defensive belt-and-braces even though the SQL query already filters WHERE enabled_override IS NOT NULL"
  - "Defensive third-state branch in the template: {% else %}STATE {{ job.enabled_override }}{% endif %} renders any out-of-range tri-state value (e.g., 2, -1) without panic — matches DB-14's 'tri-state column with reserved values' contract"
  - "Reused .cd-badge--disabled and .cd-badge--forced shipped in Plan 05; zero new CSS in Plan 06"
  - "Followed UI-SPEC § Surface D markup verbatim (heading uses --cd-text-lg per D-09, section margin-top --cd-space-8, three-column table with width:120px on the Clear column, hover:bg-(--cd-bg-hover) row hover)"

patterns-established:
  - "Audit-surface section pattern: a settings page can grow new audit surfaces below the existing card-grid by appending a <section> wrapped in an empty-state-hide {% if %}; future surfaces (e.g., dead-letter queue audit) follow this exact shape"
  - "View-model exposure for testing: when an integration test needs to assert template rendering of a specific template-data shape (rather than going through a full Router round-trip), make the askama Template struct + its fields pub. This is now documented as the v1.1 way to test settings/dashboard templates that have empty-state branches."

requirements-completed: [ERG-03]

# Metrics
duration: 5min
completed: 2026-04-22
---

# Phase 14 Plan 06: Settings "Currently Overridden" Audit Surface Summary

**Settings page now lists every job with a non-NULL enabled_override in a hidden-when-empty audit table, with a per-row Clear button that POSTs to /api/jobs/bulk-toggle with action=enable; closes ERG-03 and flips the final red-bar test (settings_empty_state_hides_section) green — Plan 01 scoreboard now 15/15.**

## Performance

- **Duration:** 5 min
- **Started:** 2026-04-22T22:49:49Z
- **Completed:** 2026-04-22T22:55:02Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Operators can now see, at a glance from /settings, every job whose enabled_override is non-NULL (the v1.1 promise that bulk-disabled jobs aren't silently lost for months — ERG-03)
- Each overridden job has a one-click Clear button that clears the override (D-05 symmetry: enable=clear-to-NULL via the same Plan 04 bulk-toggle endpoint, no new route)
- Section is hidden entirely when no overrides exist (D-10a) — no "no overrides to display" placeholder noise on the common-case clean dashboard
- Defensive FORCED ON badge for the reserved enabled_override = 1 schema state (defensive rendering only; v1.1 UI never writes 1)
- Plan 01 scoreboard 100% complete: all 15 v11_bulk_toggle SQLite tests now PASS

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend SettingsPage + settings() handler with overridden_jobs** — `e945549` (feat)
2. **Task 2: Append "Currently Overridden" section to settings.html** — `353eb04` (feat)

_TDD execution note: the plan marks both tasks `tdd="true"` but the RED tests (`settings_empty_state_hides_section` + the import path `cronduit::web::handlers::settings::{OverriddenJobView, SettingsPage}`) were already authored in Plan 01 and shipped to the repo as part of the Wave-0 red-bar tests. Both tasks ran in classical GREEN-only mode against pre-existing RED tests; no new test commits were required from this plan._

## Files Created/Modified

- `src/web/handlers/settings.rs` — Made `SettingsPage` and all its fields `pub`, added new `pub struct OverriddenJobView { id, name, enabled_override }`, hydrated `overridden_jobs` from `queries::get_overridden_jobs(&state.pool)` with non-fatal error handling.
- `templates/pages/settings.html` — Appended `<section>` with `<h2>Currently Overridden</h2>`, description paragraph, and 3-column table iterating `{% for job in overridden_jobs %}`; entire section wrapped in `{% if !overridden_jobs.is_empty() %}` per D-10a; per-row Clear form posts to `/api/jobs/bulk-toggle` with `action=enable` and single `job_ids` per D-05.

## Final Test Output

```
$ cargo nextest run --test v11_bulk_toggle
    Starting 15 tests across 1 binary
        PASS [   0.020s] ( 1/15) cronduit::v11_bulk_toggle get_overridden_jobs_alphabetical
        PASS [   0.020s] ( 2/15) cronduit::v11_bulk_toggle disable_missing_clears_override
        PASS [   0.021s] ( 3/15) cronduit::v11_bulk_toggle dashboard_filter
        PASS [   0.021s] ( 4/15) cronduit::v11_bulk_toggle handler_dedupes_ids
        PASS [   0.022s] ( 5/15) cronduit::v11_bulk_toggle handler_disable
        PASS [   0.023s] ( 6/15) cronduit::v11_bulk_toggle handler_accepts_repeated_job_ids
        PASS [   0.023s] ( 7/15) cronduit::v11_bulk_toggle handler_csrf
        PASS [   0.024s] ( 8/15) cronduit::v11_bulk_toggle handler_enable
        PASS [   0.028s] ( 9/15) cronduit::v11_bulk_toggle handler_partial_invalid
        PASS [   0.010s] (10/15) cronduit::v11_bulk_toggle settings_empty_state_hides_section
        PASS [   0.015s] (11/15) cronduit::v11_bulk_toggle upsert_invariant
        PASS [   0.018s] (12/15) cronduit::v11_bulk_toggle handler_partial_invalid_toast_uses_rows_affected
        PASS [   0.021s] (13/15) cronduit::v11_bulk_toggle reload_invariant
        PASS [   0.217s] (14/15) cronduit::v11_bulk_toggle handler_rejects_empty
        PASS [   0.271s] (15/15) cronduit::v11_bulk_toggle handler_fires_reload_after_update
     Summary [   0.271s] 15 tests run: 15 passed, 0 skipped
```

```
$ cargo build --quiet                 # EXIT 0
$ cargo clippy --quiet -- -D warnings # EXIT 0
```

## Plan 01 Scoreboard: 15/15 Green

The `tests/v11_bulk_toggle.rs` file documents a 15-row coverage map at the top:

| Test                                                       | Plan | Status |
|------------------------------------------------------------|------|--------|
| upsert_invariant                                           | 02+03 | GREEN |
| reload_invariant                                           | 03   | GREEN |
| disable_missing_clears_override                            | 03   | GREEN |
| dashboard_filter                                           | 03   | GREEN |
| handler_csrf                                               | 04   | GREEN |
| handler_disable                                            | 04   | GREEN |
| handler_enable                                             | 04   | GREEN |
| handler_partial_invalid                                    | 04   | GREEN |
| handler_partial_invalid_toast_uses_rows_affected           | 04   | GREEN |
| handler_dedupes_ids                                        | 04   | GREEN |
| handler_rejects_empty                                      | 04   | GREEN |
| handler_accepts_repeated_job_ids                           | 04   | GREEN |
| handler_fires_reload_after_update                          | 04   | GREEN |
| get_overridden_jobs_alphabetical                           | 03   | GREEN |
| **settings_empty_state_hides_section**                     | **06** | **GREEN (this plan)** |

Note: PLAN.md's body text references "14 SQLite tests" in two places; the actual coverage map and test binary count is 15. The discrepancy is purely cosmetic in PLAN.md — the test file ships 15 tests and all are GREEN.

Postgres parity tests (testcontainers-modules) are gated behind a separate feature/CI job and are not in the SQLite-default test binary. Per PLAN.md output spec, "5 Postgres" parity tests will run in the Plan 09 CI matrix; this plan's SQLite-side scoreboard is fully green.

## Decisions Made

- **Make `SettingsPage` and `OverriddenJobView` public.** The integration test at `tests/v11_bulk_toggle.rs:46` imports `cronduit::web::handlers::settings::{OverriddenJobView, SettingsPage}` and constructs a `SettingsPage { ... overridden_jobs: Vec::<OverriddenJobView>::new(), ... }` directly to call `.render()`. The cleanest path was to pub-ify both types and all `SettingsPage` fields. This mirrors the precedent that handler view-models can be public when test harness needs them (e.g., the bulk_toggle handler is itself `pub` for the same reason). Documented as a pattern for future settings-template tests.
- **Defensive `{% else %}STATE {{ ov }}{% endif %}` branch in the template.** UI-SPEC § Surface D shows only two badges (DISABLED + FORCED ON), but DB-14 reserves `enabled_override` as a tri-state-or-more INTEGER column. A direct DB write of `enabled_override = 2` shouldn't crash the page; the catch-all branch renders `STATE 2` in a yellow DISABLED-styled badge so an operator can still see the row exists and click Clear. Zero cost; defensive belt-and-braces.
- **Filter at the view-model boundary even though SQL already filters.** The SQL `WHERE enabled_override IS NOT NULL` guarantees no NULL rows, but the Rust hydration uses `filter_map(|j| j.enabled_override.map(...))` to convert `Option<i64>` → `i64` safely. This avoids `unwrap()` in the hot path and survives a future SQL refactor that might widen the query.

## Deviations from Plan

None — plan executed exactly as written. The only minor adaptation:

- The plan's per-row "STATE {{ job.enabled_override }}" defensive branch was authored in Task 2's `<action>` block but called out as a load-bearing piece in `<acceptance_criteria>`. Followed the action block verbatim; this is by-design from the plan author, not a deviation.
- Replaced "test count = 14" cosmetic in plan body with actual count "15" in the SUMMARY (per-test scoreboard above) — the test file actually ships 15 tests and the coverage map at `v11_bulk_toggle.rs:6-25` lists 15 rows. PLAN.md will need a one-character cosmetic touch-up at some future revision, but no plan logic changes.

## Issues Encountered

None. Both task acceptance grep checks passed on first attempt; build + clippy + nextest all green on first run.

The PreToolUse hook briefly warned about read-before-edit (a stale check after my multi-edit cycle on the same file); re-Read calls satisfied the hook without changes to the underlying edits, which had already been applied.

## Files Outside My Scope (Untouched)

Per parallel-execution boundary: did NOT modify STATE.md, ROADMAP.md, THREAT_MODEL.md, justfile, or examples/* (those are owned by the concurrent 14-07 ops/docs agent). My scope was strictly:

- `src/web/handlers/settings.rs`
- `templates/pages/settings.html`

## User Setup Required

None — pure code change.

## Self-Check: PASSED

- `src/web/handlers/settings.rs` — present, modified (commit `e945549`)
- `templates/pages/settings.html` — present, modified (commit `353eb04`)
- Commit `e945549` — present in `git log` (verified)
- Commit `353eb04` — present in `git log` (verified)
- All 15 v11_bulk_toggle tests — PASS (verified)
- `cargo build --quiet` — EXIT 0 (verified)
- `cargo clippy --quiet -- -D warnings` — EXIT 0 (verified)

## Next Plan Readiness

- Plan 01's RED-bar scoreboard is now 100% GREEN (15/15 SQLite tests).
- Plan 09 (HUMAN-UAT) can now exercise step 5 ("Navigate to /settings → 'Currently Overridden' section lists all 3 bulk-disabled jobs with a Clear button each") and step 6 ("Click Clear on one → toast 'override cleared'; job returns to dashboard active state within one poll cycle") against a live server.
- No blockers for the remaining Wave-4/Wave-5 plans (14-07 ops/docs running concurrently, 14-08 verifier, 14-09 release-engineering).

---
*Phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship*
*Completed: 2026-04-22*
