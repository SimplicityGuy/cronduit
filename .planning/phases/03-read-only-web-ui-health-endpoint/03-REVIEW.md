---
phase: 03-read-only-web-ui-health-endpoint
reviewed: 2026-04-10T00:00:00Z
depth: standard
files_reviewed: 31
files_reviewed_list:
  - Cargo.toml
  - build.rs
  - justfile
  - tailwind.config.js
  - assets/src/app.css
  - assets/static/theme.js
  - src/cli/run.rs
  - src/db/mod.rs
  - src/db/queries.rs
  - src/scheduler/cmd.rs
  - src/scheduler/mod.rs
  - src/web/ansi.rs
  - src/web/assets.rs
  - src/web/csrf.rs
  - src/web/handlers/api.rs
  - src/web/handlers/dashboard.rs
  - src/web/handlers/health.rs
  - src/web/handlers/job_detail.rs
  - src/web/handlers/mod.rs
  - src/web/handlers/run_detail.rs
  - src/web/handlers/settings.rs
  - src/web/mod.rs
  - templates/base.html
  - templates/pages/dashboard.html
  - templates/pages/job_detail.html
  - templates/pages/run_detail.html
  - templates/pages/settings.html
  - templates/partials/job_table.html
  - templates/partials/log_viewer.html
  - templates/partials/run_history.html
  - templates/partials/toast.html
  - tests/health_endpoint.rs
  - tests/xss_log_safety.rs
findings:
  critical: 1
  warning: 4
  info: 4
  total: 9
status: issues_found
---

# Phase 03: Code Review Report

**Reviewed:** 2026-04-10
**Depth:** standard
**Files Reviewed:** 31
**Status:** issues_found

## Summary

Phase 03 delivers the read-only web UI, health endpoint, CSRF protection for the Run Now action, ANSI-to-HTML log rendering, and embedded static asset serving. Overall quality is high: all queries are parameterized, the double-submit CSRF pattern is correctly implemented, XSS protection is verified by both unit tests and a template-walker test, and the graceful-shutdown logic in the scheduler is sound.

One critical issue exists: the CSRF cookie is set without the `Secure` attribute, meaning it transmits in plaintext over HTTP connections. Four warnings cover a logic correctness issue (health endpoint always returns 200 even on DB error), a duplicate "Load older lines" button that persists stale state, the UTC-hardcoded timezone in dashboard next-fire computation, and duplicate `format_duration_ms` code. Four info items note a theme icon sync gap, a potential duplicate toast container, a `build.rs` unwrap, and a `NULLS LAST` SQLite version dependency.

---

## Critical Issues

### CR-01: CSRF cookie missing `Secure` flag

**File:** `src/web/csrf.rs:62-69`

**Issue:** The `ensure_csrf_cookie` middleware sets:
```
cronduit_csrf=<token>; HttpOnly; SameSite=Strict; Path=/
```
The `Secure` attribute is absent. Without it, browsers transmit the cookie over plain HTTP. An active network attacker on a LAN path can read the cookie from any unencrypted request and replay it to satisfy the double-submit CSRF check. Since the Run Now form is the only state-changing operation in v1, this is the only CSRF-protected endpoint — losing this protection is meaningful even in a homelab context.

**Fix:** Add `Secure` to the cookie string. For deployments that are loopback-only (no TLS at all), consider exposing an `--allow-insecure-cookies` startup flag rather than removing `Secure` by default:

```rust
// secure-by-default
let cookie = format!(
    "{}={}; HttpOnly; SameSite=Strict; Path=/; Secure",
    CSRF_COOKIE_NAME, token
);
```

---

## Warnings

### WR-01: Duplicate "Load older lines" button causes stale UI state

**File:** `templates/pages/run_detail.html:72-80`

**Issue:** `run_detail.html` renders a "Load older lines" button (lines 72-80) that targets `#log-lines` with `hx-swap="afterbegin"`. The `log_viewer.html` partial, which is `{% include %}`-d into `#log-lines`, independently renders the same button when `has_older` is true. After the first HTMX swap refreshes `#log-lines`, the partial's button is correctly updated based on the new `has_older` state. However, the outer button in `run_detail.html` lives outside `#log-lines` and is never swapped out — it persists on the page even after all older lines have been loaded, where clicking it would re-request the same already-loaded offset.

**Fix:** Remove the outer "Load older lines" button from `run_detail.html` (lines 72-80) and rely entirely on the button inside `log_viewer.html`. The partial already handles its own state correctly on every swap.

---

### WR-02: `/health` always returns HTTP 200 even when database is unreachable

**File:** `src/web/handlers/health.rs:10-22`

**Issue:** The health handler responds with `200 OK` regardless of `db_status`. Docker `HEALTHCHECK`, Kubernetes liveness probes, and uptime monitors key on HTTP status codes, not JSON bodies. A deployment tool will report the service healthy even when the database is down:

```rust
// current — always 200
Json(json!({ "status": "ok", "db": db_status, "scheduler": "running" }))
```

**Fix:** Return `503 Service Unavailable` when the DB check fails:

```rust
use axum::http::StatusCode;

pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = check_db(&state).await;
    let status_code = if db_ok { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    let body = Json(json!({
        "status": if db_ok { "ok" } else { "degraded" },
        "db": if db_ok { "ok" } else { "error" },
        "scheduler": "running"
    }));
    (status_code, body)
}
```

The test `health_returns_200_with_ok_status` in `tests/health_endpoint.rs` uses an in-memory SQLite DB that is always up, so it will continue to assert 200 correctly after this change.

---

### WR-03: Dashboard next-fire column always uses UTC, ignoring configured timezone

**File:** `src/web/handlers/dashboard.rs:196`

**Issue:**
```rust
// TODO: pass tz through AppState when config timezone is wired
let tz: Tz = chrono_tz::UTC;
```

The scheduler fires jobs in the operator's configured timezone (passed correctly to `scheduler::spawn`), but the dashboard's "Next Fire" column always computes relative time in UTC. For any operator using a non-UTC timezone, the displayed next-fire time will be offset from wall-clock reality by the timezone difference, making it misleading.

**Fix:** Add `tz: chrono_tz::Tz` to `AppState` and populate it from the parsed timezone in `src/cli/run.rs`:

```rust
// In AppState (src/web/mod.rs):
pub tz: chrono_tz::Tz,

// In cli/run.rs, after parsing tz:
let state = AppState { tz, /* ... */ };

// In dashboard.rs handler:
let job_views: Vec<DashboardJobView> =
    jobs.into_iter().map(|j| to_view(j, state.tz)).collect();
```

---

### WR-04: `format_duration_ms` duplicated between two handler modules

**File:** `src/web/handlers/job_detail.rs:88-97` and `src/web/handlers/run_detail.rs:81-97`

**Issue:** The `format_duration_ms` function is defined with identical logic in both modules. A future change to duration display (e.g., adding days, changing the separator) must be applied in two places, and divergence is likely.

**Fix:** Extract to a shared module:

```rust
// src/web/format.rs
pub fn format_duration_ms(ms: Option<i64>) -> String { /* shared impl */ }

// In both handlers:
use crate::web::format::format_duration_ms;
```

---

## Info

### IN-01: `build.rs` uses `unwrap()` on path canonicalization — can produce a cryptic build failure

**File:** `build.rs:29`

**Issue:** `binary.canonicalize().unwrap()` panics if canonicalization fails (e.g., if `bin/tailwindcss` is a dangling symlink — `exists()` returns false for dangling symlinks so the guard on line 13 would prevent reaching this, but the unwrap is still fragile). A panic in `build.rs` produces a terse linker-style error that is harder to diagnose than a `cargo:warning=` message.

**Fix:**
```rust
let canonical = binary.canonicalize().unwrap_or_else(|_| binary.to_path_buf());
let status = Command::new(canonical).args([...]).status();
```

---

### IN-02: Theme toggle icon does not update on page load or toggle

**File:** `assets/static/theme.js:1-10`

**Issue:** On page load the script sets `data-theme` from localStorage, but `#theme-icon` in `base.html` is hardcoded to `&#9790;` (moon) and never updated. If a user has stored `light` mode, the page renders in light mode but shows a moon icon. The toggle function also updates `data-theme` and localStorage but does not update the icon. The icon always displays moon regardless of active theme.

**Fix:** Add an `updateIcon` call in the IIFE and inside the toggle function to sync the icon with the current theme. Use sun (`&#9788;`) for light mode and moon (`&#9790;`) for dark mode.

---

### IN-03: Potential duplicate `#toast-container` element if `toast.html` partial is ever included

**File:** `templates/partials/toast.html:5` and `templates/base.html:49`

**Issue:** `base.html` already renders `<div id="toast-container" ...>` at line 49. The `toast.html` partial renders an identical element. No template currently includes this partial, but if one is added that extends `base.html`, there will be two elements with the same ID. `document.getElementById('toast-container')` returns only the first match; the second is dead markup.

**Fix:** Either delete `templates/partials/toast.html` (nothing currently uses it and `base.html` handles the container), or add a comment to `toast.html` explicitly marking it as only for use in non-`base.html` contexts.

---

### IN-04: `NULLS LAST` in SQL requires SQLite 3.30.0+, no version guard

**File:** `src/db/queries.rs:480-484`

**Issue:** The dashboard order clauses use `NULLS LAST` (e.g., `"ORDER BY lr.start_time DESC NULLS LAST"`). This syntax was added in SQLite 3.30.0 (2019-10-04). Deployments on older Linux distributions (Ubuntu 18.04 ships 3.22) will receive a SQL parse error at runtime rather than a compile-time failure. The project does not document a minimum SQLite version.

**Fix:** Either document the minimum SQLite version as 3.30.0+ in README, or rewrite the ordering using `COALESCE` for portability:

```sql
ORDER BY COALESCE(lr.start_time, '') DESC
```
Empty string sorts before any timestamp value in DESC order, which is equivalent to `NULLS LAST` for this schema.

---

_Reviewed: 2026-04-10_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
