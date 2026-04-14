---
phase: 07-v1-cleanup-bookkeeping
plan: 04
subsystem: testing
tags: [regression-test, http-handler, axum, htmx, reload, csrf, bookkeeping]

# Dependency graph
requires:
  - phase: 05-config-reload
    provides: POST /api/reload handler, SchedulerCmd::Reload + ReloadResult types, CSRF middleware
  - phase: 06-observability
    provides: HX-Refresh header fix in src/web/handlers/api.rs (commit 8b69cb8, PR #9)
provides:
  - First HTTP-handler-level regression test in the repo for reload
  - tests/reload_api.rs::reload_response_includes_hx_refresh_header referenceable from Plan 03
  - Reusable pattern for future axum handler tests: Router + stub scheduler + tower::ServiceExt::oneshot
affects: [07-03, phase-08-v1-gap-closure]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - HTTP-layer axum handler testing via tower::ServiceExt::oneshot (new pattern for this repo)
    - Background stub scheduler via tokio::spawn + mpsc drain (replies Ok to Reload commands)
    - setup_metrics() reused in test harness (safe due to OnceLock memoization, WR-01)

key-files:
  created:
    - tests/reload_api.rs
  modified: []

key-decisions:
  - "Used cronduit::telemetry::setup_metrics() directly instead of reimplementing PrometheusHandle construction — setup_metrics is memoized via OnceLock (WR-01 fix in src/telemetry.rs:62), so repeated calls from test harnesses are safe and return the handle actually attached to the global metrics facade."
  - "Test name is exactly reload_response_includes_hx_refresh_header to match the cross-reference in Plan 03 / 05-VERIFICATION.md."
  - "No production code touched (D-13 locked: fix is already on main from PR #9)."

patterns-established:
  - "HTTP-handler regression testing: build a minimal Router with the single route under test + stubbed AppState + background scheduler drain, then oneshot a request through tower::ServiceExt. First instance in this repo."
  - "CSRF test harness: reuse the same string in the cookie and form body — validate_csrf accepts any byte-equal non-empty pair."

requirements-completed: [RELOAD-04]

# Metrics
duration: 9min
completed: 2026-04-13
---

# Phase 07 Plan 04: HX-Refresh Regression Test Summary

**HTTP-handler-level regression test locks in the PR #9 fix that makes the settings Reload Config card auto-refresh after a successful POST /api/reload.**

## Performance

- **Duration:** ~9 min
- **Started:** 2026-04-13T20:49:00Z (approx)
- **Completed:** 2026-04-13T20:58:31Z
- **Tasks:** 1
- **Files modified:** 1 (1 created, 0 modified)

## Accomplishments

- Added `tests/reload_api.rs` (116 lines) — the first HTTP-handler-level reload test in the repo.
- `reload_response_includes_hx_refresh_header` passes on the current `main` tree (commit 6688bb0) in 0.21s.
- Established a reusable axum-handler test harness pattern (Router + stub scheduler + `tower::ServiceExt::oneshot`) that the remaining v1-cleanup / v1-gap-closure phases can copy.
- Made Plan 03's `05-VERIFICATION.md` re-verification block cite a real, check-able regression test name instead of a TODO.

## Task Commits

1. **Task 1: Create tests/reload_api.rs — HX-Refresh regression test (D-14)** — `6688bb0` (test)

_Note: This task was TDD-flagged but executed in a single write-compile-run-commit cycle because the production fix (PR #9) is already on main — the test is the verification artifact, not a driver for new code. RED/GREEN/REFACTOR collapsed to a single GREEN step._

## Files Created/Modified

- `tests/reload_api.rs` (116 lines, created) — Single `#[tokio::test]` asserting `POST /api/reload` with valid CSRF returns HTTP 200 + `HX-Refresh: true` header via `tower::ServiceExt::oneshot` against a Router wrapping the real `cronduit::web::handlers::api::reload` handler. Stubs `AppState` with an in-memory SQLite pool, a background scheduler task replying `ReloadResult { status: Ok, unchanged: 3 }`, and `setup_metrics()` as the `PrometheusHandle`.

## AppState Literal Adjustments

The `AppState { ... }` literal in the test matches `src/web/mod.rs:27-48` exactly — no field drift since the plan's 2026-04-12 interface snapshot. Fields constructed:

| Field | Value |
|-------|-------|
| `started_at` | `chrono::Utc::now()` |
| `version` | `"test"` |
| `pool` | `DbPool::connect("sqlite::memory:").await` + `migrate` |
| `cmd_tx` | `tokio::sync::mpsc::channel::<SchedulerCmd>(16).0` |
| `config_path` | `PathBuf::from("/tmp/cronduit-test.toml")` |
| `tz` | `chrono_tz::UTC` |
| `last_reload` | `Arc::new(Mutex::new(None))` |
| `watch_config` | `false` |
| `metrics_handle` | `cronduit::telemetry::setup_metrics()` (memoized) |
| `active_runs` | `Arc::new(RwLock::new(HashMap::new()))` |

Plan skeleton used `PrometheusBuilder::new().install_recorder()` with a fallback to `build_recorder().handle()`. Replaced that with `setup_metrics()` because `src/telemetry.rs` (WR-01 fix) memoizes the installed handle via `OnceLock`, which is both simpler and semantically correct — the detached fallback handle the plan suggested would render an empty body if anything queried the `/metrics` path, though the reload handler itself never touches the metrics handle so it didn't matter for this specific test.

## Decisions Made

- **setup_metrics() reuse over hand-rolled PrometheusBuilder:** `cronduit::telemetry::setup_metrics()` is public, memoized via `OnceLock`, and returns the handle actually attached to the global `metrics::` facade. Cleaner than the plan's fallback closure and avoids the "detached handle returning empty body" footgun that WR-01 was fixed for.
- **Comment phrasing to avoid `do_reload` literal:** The plan acceptance criterion requires `grep -c 'do_reload' tests/reload_api.rs` to return 0 (test must NOT use the library-level pattern). Rephrased the docstring from "call `do_reload()` directly" to "call the library reload entry point directly" so the string literal stays out of the file while preserving the intent of the comment.

## Deviations from Plan

None - plan executed as written. The two small departures from the skeleton (`setup_metrics()` instead of `PrometheusBuilder::new().install_recorder()`, and docstring rephrasing) are documented under "AppState Literal Adjustments" and "Decisions Made" and were explicitly permitted by the plan's adjustment bullets (items 2 and 5 under the `<action>` block).

## Issues Encountered

- First run of the acceptance criteria grep showed `grep -c 'do_reload' tests/reload_api.rs` returning 1 because the original docstring contained "`do_reload()` directly at the library level". Rephrased to "the library reload entry point directly" and re-verified the count drops to 0. Re-ran the test — still passes in 0.21s.

## Acceptance Criteria Verification

| Check | Expected | Actual | Status |
|-------|----------|--------|--------|
| `test -f tests/reload_api.rs` | exit 0 | exit 0 | PASS |
| `grep -c 'fn reload_response_includes_hx_refresh_header' tests/reload_api.rs` | 1 | 1 | PASS |
| `grep -c '#\[tokio::test\]' tests/reload_api.rs` | >= 1 | 1 | PASS |
| `grep -c 'tower::ServiceExt' tests/reload_api.rs` | >= 1 | 2 | PASS |
| `grep -c 'do_reload' tests/reload_api.rs` | 0 | 0 | PASS |
| `grep -c 'HX-Refresh' tests/reload_api.rs` | >= 2 | 6 | PASS |
| `grep '"true"' tests/reload_api.rs` | >= 1 | 1 | PASS |
| `grep -c 'CSRF_COOKIE_NAME' tests/reload_api.rs` | >= 1 | 2 | PASS |
| `cargo check --tests --test reload_api` | exit 0 | exit 0 | PASS (54.7s cold) |
| `cargo test --test reload_api reload_response_includes_hx_refresh_header -- --exact` | exit 0 | exit 0 (0.21s runtime) | PASS |
| `git diff --name-only src/` | empty | empty | PASS |
| `git diff --name-only Cargo.toml` | empty | empty | PASS |
| Test runtime < 10s | < 10s | 0.21s | PASS |
| File length >= 60 lines | >= 60 | 116 | PASS |

## Acknowledgements

- **D-13 (fix already on main):** Acknowledged. No changes to `src/web/handlers/api.rs` — the `headers.insert("HX-Refresh", "true".parse().unwrap())` call at `src/web/handlers/api.rs:181` is exactly what this test verifies.
- **D-16 (browser UAT deferred to Phase 8):** Acknowledged. This plan adds only the automated regression test; live-browser UAT of the reload card's auto-refresh behavior will run in the Phase 8 v1 gap-closure UAT pass.

## Next Phase Readiness

- Plan 03 (`07-03-PLAN.md`) can now cite `tests/reload_api.rs::reload_response_includes_hx_refresh_header` as a concrete regression reference in its `05-VERIFICATION.md` update block.
- Future phases needing handler-level tests (settings page, job detail, run detail) can copy the `build_test_app()` pattern from `tests/reload_api.rs` as a reference.
- No blockers. Phase 7 Plan 04 is fully closed.

## Self-Check: PASSED

- `tests/reload_api.rs` exists (116 lines) — FOUND
- Commit `6688bb0` exists in `git log --oneline` — FOUND
- No `src/` or `Cargo.toml` changes in the commit — VERIFIED via `git show --stat 6688bb0`
- Test passes in 0.21s — VERIFIED via `cargo test --test reload_api reload_response_includes_hx_refresh_header -- --exact`

---
*Phase: 07-v1-cleanup-bookkeeping*
*Plan: 04*
*Completed: 2026-04-13*
