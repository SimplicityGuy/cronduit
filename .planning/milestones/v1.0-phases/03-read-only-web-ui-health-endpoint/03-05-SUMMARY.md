---
phase: 03-read-only-web-ui-health-endpoint
plan: 05
subsystem: web-csrf-run-now
tags: [csrf, api, run-now, htmx, security]
dependency_graph:
  requires: [03-02, 03-03, 03-04]
  provides: [csrf-protection, run-now-endpoint, toast-notifications]
  affects: [web-router, dashboard-handler, job-detail-handler]
tech_stack:
  added: []
  patterns: [double-submit-csrf, constant-time-comparison, htmx-trigger-events]
key_files:
  created:
    - src/web/csrf.rs
    - src/web/handlers/api.rs
    - templates/partials/toast.html
  modified:
    - src/web/mod.rs
    - src/web/handlers/mod.rs
    - src/web/handlers/dashboard.rs
    - src/web/handlers/job_detail.rs
    - Cargo.toml
decisions:
  - "Used HxEvent::new_with_data with serde JSON payload for toast, matching existing base.html listener pattern (e.detail.message)"
  - "Used rand::RngCore::fill_bytes instead of Rng::gen to avoid Rust 2024 reserved keyword 'gen'"
  - "Returned 200 OK (not 204) from Run Now to allow HX-Trigger response header processing by HTMX"
metrics:
  duration: "~21 minutes"
  completed: "2026-04-11T01:32:00Z"
  tasks_completed: 1
  tasks_total: 1
  files_changed: 9
---

# Phase 03 Plan 05: CSRF Protection & Run Now API Summary

CSRF double-submit cookie with constant-time validation and POST /api/jobs/:id/run sending SchedulerCmd::RunNow through the mpsc channel with HTMX toast notification.

## What Was Built

### CSRF Module (`src/web/csrf.rs`)
- `generate_csrf_token()`: 32-byte random token as 64-char hex string using `rand::RngCore::fill_bytes`
- `validate_csrf()`: Constant-time XOR-based comparison preventing timing attacks
- `ensure_csrf_cookie`: Axum middleware that sets `cronduit_csrf` cookie (HttpOnly; SameSite=Strict) on every response if not present
- `get_token_from_cookies()`: Helper for templates to read CSRF token from cookie jar
- 7 unit tests covering token generation, matching, mismatching, empty inputs, and length differences

### Run Now API (`src/web/handlers/api.rs`)
- `POST /api/jobs/:id/run` handler with:
  1. CSRF token validation (cookie vs form field) -- returns 403 on mismatch
  2. Job existence check via `get_job_by_id` -- returns 404 for unknown jobs
  3. `SchedulerCmd::RunNow { job_id }` sent through mpsc channel (not bypassing scheduler)
  4. `HxResponseTrigger` with `showToast` event carrying `{message, level}` JSON payload
  5. Returns 503 if scheduler channel is closed (graceful shutdown)

### Router & Middleware Updates (`src/web/mod.rs`)
- Added `pub mod csrf` declaration
- Added `POST /api/jobs/{id}/run` route
- Added CSRF cookie middleware layer via `axum::middleware::from_fn`

### Handler Updates
- Dashboard and Job Detail handlers now accept `CookieJar` extractor
- Replaced placeholder `hex::encode(rand::random::<[u8; 16]>())` with `csrf::get_token_from_cookies()`

### Cargo.toml Changes
- Enabled `form` feature on `axum` (required for `axum::Form` extractor)
- Enabled `serde` feature on `axum-htmx` (required for `HxEvent::new_with_data`)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Rust 2024 `gen` reserved keyword**
- **Found during:** Task 1
- **Issue:** `rand::thread_rng().gen()` uses the `gen` identifier which is reserved in Rust 2024 edition
- **Fix:** Used `rand::RngCore::fill_bytes()` instead
- **Files modified:** `src/web/csrf.rs`
- **Commit:** b3f288e

**2. [Rule 3 - Blocking] Missing axum `form` feature**
- **Found during:** Task 1
- **Issue:** `axum::Form` extractor requires the `form` feature which was not enabled
- **Fix:** Added `form` to axum features in Cargo.toml
- **Files modified:** `Cargo.toml`
- **Commit:** b3f288e

**3. [Rule 1 - Bug] HxResponseTrigger tuple ordering**
- **Found during:** Task 1
- **Issue:** `(StatusCode, HxResponseTrigger)` does not implement `IntoResponse`; `HxResponseTrigger` implements `IntoResponseParts` and must come first in the tuple
- **Fix:** Reordered to `(HxResponseTrigger, StatusCode)` which compiles correctly
- **Files modified:** `src/web/handlers/api.rs`
- **Commit:** b3f288e

**4. [Rule 2 - Adaptation] Toast payload format aligned with existing JS**
- **Found during:** Task 1
- **Issue:** Plan suggested using `X-Toast-Message` custom header, but base.html already had a `showToast` listener expecting `e.detail.message` from HTMX event detail
- **Fix:** Used `HxEvent::new_with_data` with JSON `{message, level}` payload matching existing listener
- **Files modified:** `src/web/handlers/api.rs`
- **Commit:** b3f288e

## Verification Results

- `cargo test --lib web::csrf::tests`: 7/7 passed
- `cargo check`: clean (no errors)
- CSRF module has constant-time XOR comparison
- HttpOnly; SameSite=Strict cookie attributes present
- Run Now sends through scheduler channel (SchedulerCmd::RunNow)
- Toast wired via HxResponseTrigger showToast event

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | b3f288e | CSRF protection and Run Now API endpoint |
