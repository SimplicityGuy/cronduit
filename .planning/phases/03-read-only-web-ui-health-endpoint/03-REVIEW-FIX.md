---
phase: 03-read-only-web-ui-health-endpoint
fixed_at: 2026-04-11T00:00:00Z
review_path: .planning/phases/03-read-only-web-ui-health-endpoint/03-REVIEW.md
iteration: 1
findings_in_scope: 5
fixed: 5
skipped: 0
status: all_fixed
---

# Phase 03: Code Review Fix Report

**Fixed at:** 2026-04-11
**Source review:** .planning/phases/03-read-only-web-ui-health-endpoint/03-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 5
- Fixed: 5
- Skipped: 0

## Fixed Issues

### CR-01: CSRF cookie missing `Secure` flag

**Files modified:** `src/web/csrf.rs`
**Commit:** e91bd9e
**Applied fix:** Added `; Secure` to the CSRF cookie format string in the `ensure_csrf_cookie` middleware. The cookie is now set with `HttpOnly; SameSite=Strict; Path=/; Secure`, preventing transmission over plaintext HTTP connections.

### WR-01: Duplicate "Load older lines" button causes stale UI state

**Files modified:** `templates/pages/run_detail.html`
**Commit:** ba03e36
**Applied fix:** Removed the outer "Load older lines" button (and its `{% if has_older %}` conditional block) from `run_detail.html`. The button inside `templates/partials/log_viewer.html` remains and correctly manages its own state on every HTMX swap.

### WR-02: `/health` always returns HTTP 200 even when database is unreachable

**Files modified:** `src/web/handlers/health.rs`
**Commit:** 27559a9
**Applied fix:** Changed the health handler return type to `impl IntoResponse` and added logic to return `503 Service Unavailable` when the DB check fails. The response body now also reflects degraded status (`"status": "degraded"`) when the database is unreachable. Existing test continues to pass as it uses an in-memory SQLite DB that is always available.

### WR-03: Dashboard next-fire column always uses UTC, ignoring configured timezone

**Files modified:** `src/web/mod.rs`, `src/cli/run.rs`, `src/web/handlers/dashboard.rs`
**Commit:** 5d4ed6a
**Applied fix:** Added `pub tz: chrono_tz::Tz` field to `AppState`, populated it from the already-parsed `tz` variable in `cli/run.rs`, and replaced the hardcoded `chrono_tz::UTC` in the dashboard handler with `state.tz`. The TODO comment was removed as the timezone is now properly wired through.

### WR-04: `format_duration_ms` duplicated between two handler modules

**Files modified:** `src/web/format.rs` (new), `src/web/mod.rs`, `src/web/handlers/job_detail.rs`, `src/web/handlers/run_detail.rs`
**Commit:** 0f32768
**Applied fix:** Created `src/web/format.rs` as a shared module containing `format_duration_ms` with its unit test. Registered it as `pub mod format` in `src/web/mod.rs`. Replaced the duplicate function definitions in both `job_detail.rs` and `run_detail.rs` with `use crate::web::format::format_duration_ms`. Removed the redundant test from `job_detail.rs` (test now lives in `format.rs`).

---

_Fixed: 2026-04-11_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
