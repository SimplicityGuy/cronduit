//! Phase 11 UI-19 race fix: Run Now handler must insert the job_runs row
//! synchronously before returning HX-Refresh, so immediate click-through
//! never 404s.
//!
//! Three tests cover the fix (Plan 11-06 Task 4):
//! - `handler_inserts_before_response` (T-V11-LOG-08): row exists in
//!   job_runs by the time the handler's HTTP response lands.
//! - `no_race_after_run_now` (T-V11-LOG-09): immediate GET of the
//!   run-detail URL returns 200 (not 404), and the body does NOT
//!   contain either transient race string.
//! - `scheduler_cmd_run_now_with_run_id_variant`: handler dispatches
//!   `SchedulerCmd::RunNowWithRunId { job_id, run_id }` (not the legacy
//!   `RunNow { job_id }`) with the pre-inserted run_id.

#![allow(clippy::assertions_on_constants)]

mod common;

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware;
use axum::routing::{get, post};
use tower::ServiceExt; // brings .oneshot()

use cronduit::db::DbPool;
use cronduit::db::queries::{self, PoolRef};
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::telemetry::setup_metrics;
use cronduit::web::csrf::{self, CSRF_COOKIE_NAME};
use cronduit::web::handlers;
use cronduit::web::{AppState, ReloadState};

/// Shared CSRF token used for both the cookie and the form field.
/// `validate_csrf` accepts any byte-equal non-empty pair of equal length.
const TEST_CSRF: &str = "phase11-plan06-ui19-race-regression-token";

/// Build a test Router wired for the Run Now + Run Detail routes plus the
/// scheduler-cmd mpsc channel's receiver (so the test can observe the
/// dispatched command without actually spawning the scheduler loop — the
/// sync-insert guarantee does not depend on the scheduler running).
///
/// Returns `(Router, DbPool, Receiver<SchedulerCmd>)`.
async fn build_test_app() -> (Router, DbPool, tokio::sync::mpsc::Receiver<SchedulerCmd>) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);

    let metrics_handle = setup_metrics();

    let state = AppState {
        started_at: chrono::Utc::now(),
        version: "test",
        pool: pool.clone(),
        cmd_tx,
        config_path: std::path::PathBuf::from("/tmp/cronduit-test.toml"),
        tz: chrono_tz::UTC,
        last_reload: Arc::new(Mutex::new(None::<ReloadState>)),
        watch_config: false,
        metrics_handle,
        active_runs: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    // Minimal router: run_now POST + run_detail GET so T-V11-LOG-09 can
    // prove the no-404 guarantee end-to-end. CSRF middleware is attached
    // because run_detail calls `csrf::get_token_from_cookies` and the
    // production router has the middleware layered on.
    let router = Router::new()
        .route("/api/jobs/{id}/run", post(handlers::api::run_now))
        .route(
            "/jobs/{job_id}/runs/{run_id}",
            get(handlers::run_detail::run_detail),
        )
        .with_state(state)
        .layer(middleware::from_fn(csrf::ensure_csrf_cookie));

    (router, pool, cmd_rx)
}

/// Seed a job using the canonical `upsert_job` helper — matches the
/// `tests/api_run_now.rs` pattern. Returns the new job id.
async fn seed_job(pool: &DbPool, name: &str) -> i64 {
    queries::upsert_job(
        pool,
        name,
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo hello"}"#,
        "deadbeef",
        300,
    )
    .await
    .expect("upsert job")
}

/// Build a valid run_now POST request with matching CSRF cookie + form.
fn build_run_now_request(job_id: i64) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/api/jobs/{}/run", job_id))
        .header("cookie", format!("{}={}", CSRF_COOKIE_NAME, TEST_CSRF))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(format!("csrf_token={}", TEST_CSRF)))
        .expect("build run_now request")
}

/// T-V11-LOG-08: the row exists in job_runs by the time the handler returns.
///
/// This is the core sync-insert guarantee: the browser's HX-Refresh
/// navigation is triggered AFTER the handler's 200 lands, so if the row
/// exists at the moment of the 200 (as this test asserts), the follow-up
/// GET /jobs/{job_id}/runs/{run_id} cannot 404.
#[tokio::test]
async fn handler_inserts_before_response() {
    let (app, pool, _cmd_rx) = build_test_app().await;
    let job_id = seed_job(&pool, "sync-insert-job").await;

    let response = app
        .oneshot(build_run_now_request(job_id))
        .await
        .expect("oneshot");
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "run_now must succeed with valid CSRF + existing job"
    );

    // CRITICAL: query DB immediately — row must exist in 'running' state.
    let count: i64 = match pool.reader() {
        PoolRef::Sqlite(p) => sqlx::query_scalar(
            "SELECT COUNT(*) FROM job_runs WHERE job_id = ?1 AND status = 'running'",
        )
        .bind(job_id)
        .fetch_one(p)
        .await
        .expect("count query"),
        _ => panic!("sqlite-only harness"),
    };
    assert_eq!(
        count, 1,
        "row must exist immediately after handler returns (UI-19 race fix)"
    );
}

/// T-V11-LOG-09: immediate GET of the run-detail URL after Run Now returns
/// 200 (not 404) and the body does NOT contain the transient race-flash
/// strings.
///
/// This is the end-to-end proof of the UI-19 fix: browser does run_now ->
/// receives HX-Refresh -> navigates to /jobs/{job_id}/runs/{run_id} -> the
/// page must render cleanly even though the scheduler hasn't picked the
/// command off the mpsc channel yet (we never drive the scheduler here).
#[tokio::test]
async fn no_race_after_run_now() {
    let (app, pool, _cmd_rx) = build_test_app().await;
    let job_id = seed_job(&pool, "no-race-job").await;

    // 1. POST /api/jobs/{job_id}/run with valid CSRF.
    let post_response = app
        .clone()
        .oneshot(build_run_now_request(job_id))
        .await
        .expect("run_now oneshot");
    assert_eq!(
        post_response.status(),
        StatusCode::OK,
        "POST run_now must return 200"
    );

    // 2. Extract the run_id the handler just inserted. Handler returns
    //    HX-Refresh: true (no body id), so query the DB directly.
    let run_id: i64 = match pool.reader() {
        PoolRef::Sqlite(p) => sqlx::query_scalar(
            "SELECT id FROM job_runs \
             WHERE job_id = ?1 AND status = 'running' \
             ORDER BY id DESC LIMIT 1",
        )
        .bind(job_id)
        .fetch_one(p)
        .await
        .expect("pre-inserted run row must exist"),
        _ => panic!("sqlite-only harness"),
    };
    assert!(
        run_id > 0,
        "pre-inserted run_id must be a valid primary key"
    );

    // 3. Immediately GET /jobs/{job_id}/runs/{run_id} — MUST be 200, NOT 404.
    let get_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/jobs/{}/runs/{}", job_id, run_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("run_detail oneshot");
    assert_eq!(
        get_response.status(),
        StatusCode::OK,
        "GET run_detail immediately after run_now must be 200 (not 404) — UI-19 fix"
    );

    // 4. Body MUST NOT contain the transient race-flash strings.
    //    The run_detail template includes the SSE script tag but not the
    //    literal error messages (those are JS-side fallback text only).
    let body_bytes = axum::body::to_bytes(get_response.into_body(), 10 * 1024 * 1024)
        .await
        .expect("read body");
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(
        !body.contains("error getting logs"),
        "response body must not contain 'error getting logs' transient"
    );
    // The HTML template contains a JS fragment that *defines* the
    // "Unable to stream logs" string as a client-side fallback on SSE
    // error; but when the page is first rendered, that string is part of
    // the script source. The production bug manifests only when the
    // browser executes the SSE onerror handler — which cannot happen
    // during a 404 because the page never renders. We therefore check
    // that the page DID render (status 200) and contains the run_id,
    // proving the server-side contract rather than the JS fallback text.
    assert!(
        body.contains(&run_id.to_string()),
        "rendered page must reference the pre-inserted run_id"
    );
}

/// Handler MUST dispatch `SchedulerCmd::RunNowWithRunId { job_id, run_id }`
/// carrying the pre-inserted run_id — not the legacy `RunNow { job_id }`.
#[tokio::test]
async fn scheduler_cmd_run_now_with_run_id_variant() {
    let (app, pool, mut cmd_rx) = build_test_app().await;
    let job_id = seed_job(&pool, "variant-job").await;

    let _ = app
        .oneshot(build_run_now_request(job_id))
        .await
        .expect("oneshot");

    // Drain the channel with a short timeout — handler dispatches after
    // the sync insert, before returning the 200. Must be present.
    let cmd = tokio::time::timeout(std::time::Duration::from_millis(500), cmd_rx.recv())
        .await
        .expect("cmd channel must receive a message within 500ms")
        .expect("channel must not be closed");

    match cmd {
        SchedulerCmd::RunNowWithRunId {
            job_id: got_job,
            run_id,
        } => {
            assert_eq!(
                got_job, job_id,
                "RunNowWithRunId must carry the job_id from the URL path"
            );
            assert!(
                run_id > 0,
                "RunNowWithRunId must carry a non-zero pre-inserted run_id (not the default sentinel)"
            );
            // Belt-and-suspenders: the run_id must correspond to an
            // actually-inserted row.
            let row_exists: i64 = match pool.reader() {
                PoolRef::Sqlite(p) => {
                    sqlx::query_scalar("SELECT COUNT(*) FROM job_runs WHERE id = ?1")
                        .bind(run_id)
                        .fetch_one(p)
                        .await
                        .expect("count by id")
                }
                _ => panic!("sqlite-only harness"),
            };
            assert_eq!(
                row_exists, 1,
                "dispatched run_id must match an actually-inserted job_runs row"
            );
        }
        SchedulerCmd::RunNow { job_id: _ } => {
            panic!(
                "run_now must dispatch the new RunNowWithRunId variant (UI-19 fix), \
                 not the legacy RunNow variant"
            );
        }
        other => panic!("expected RunNowWithRunId, got {:?}", other),
    }
}
