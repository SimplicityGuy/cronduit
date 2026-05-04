//! Integration test for POST /api/jobs/{id}/run (UI-12).
//!
//! Phase 3 validation gap closure: asserts that the `run_now` API handler
//!   1. validates CSRF (cookie + form double-submit),
//!   2. dispatches a `SchedulerCmd::RunNow` through the scheduler mpsc
//!      channel so the scheduler loop can spawn a manual run, and
//!   3. returns 200 with `HX-Refresh: true` on success / 404 for unknown
//!      job ids.
//!
//! Prior coverage was indirect via `compose-smoke` CI (120s per job); this
//! file gives UI-12 a unit-test-tier feedback loop. Follows the pattern in
//! `tests/reload_api.rs` verbatim.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use tower::ServiceExt; // brings .oneshot()

use cronduit::db::DbPool;
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::telemetry::setup_metrics;
use cronduit::web::csrf::CSRF_COOKIE_NAME;
use cronduit::web::handlers::api::run_now;
use cronduit::web::{AppState, ReloadState};

/// Shared CSRF token used for both the cookie and the form field.
/// `validate_csrf` accepts any byte-equal non-empty pair of equal length.
const TEST_CSRF: &str = "phase3-run-now-api-regression-test-token";

/// Build a test Router + the receiving end of the scheduler command
/// channel, so the test can observe what was dispatched.
async fn build_test_app() -> (Router, DbPool, tokio::sync::mpsc::Receiver<SchedulerCmd>) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    // Unbuffered-ish channel — the test drains it after the POST completes.
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);

    // `setup_metrics()` memoizes the installed PrometheusHandle via OnceLock.
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

    let router = Router::new()
        .route("/api/jobs/{id}/run", post(run_now))
        .with_state(state);

    (router, pool, cmd_rx)
}

async fn seed_job(pool: &DbPool, name: &str) -> i64 {
    cronduit::db::queries::upsert_job(
        pool,
        name,
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        "{}",
        "deadbeef",
        300,
        "[]",
    )
    .await
    .expect("upsert job")
}

fn build_run_now_request(job_id: i64) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/api/jobs/{}/run", job_id))
        .header("cookie", format!("{}={}", CSRF_COOKIE_NAME, TEST_CSRF))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(format!("csrf_token={}", TEST_CSRF)))
        .expect("build request")
}

#[tokio::test]
async fn run_now_dispatches_scheduler_cmd_and_returns_hx_refresh() {
    let (app, pool, mut cmd_rx) = build_test_app().await;
    let job_id = seed_job(&pool, "run-now-happy-path").await;

    let response = app
        .oneshot(build_run_now_request(job_id))
        .await
        .expect("oneshot");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "run_now must succeed with valid CSRF + existing job"
    );

    let hx_refresh = response
        .headers()
        .get("HX-Refresh")
        .expect("HX-Refresh header must be present so HTMX reloads the job detail page");
    assert_eq!(
        hx_refresh, "true",
        "HX-Refresh must be \"true\" so the dashboard/job detail refreshes and the newly queued run appears"
    );

    // The handler must have dispatched exactly one
    // SchedulerCmd::RunNowWithRunId (Phase 11 Plan 11-06 UI-19 fix). The
    // handler thread inserts the job_runs row synchronously BEFORE sending
    // the command — the `run_id` in the payload is that pre-inserted id
    // which the scheduler then passes to `run_job_with_existing_run_id`.
    let cmd = tokio::time::timeout(std::time::Duration::from_millis(500), cmd_rx.recv())
        .await
        .expect("cmd channel must receive a message within 500ms")
        .expect("channel must not be closed");

    match cmd {
        SchedulerCmd::RunNowWithRunId {
            job_id: got,
            run_id,
        } => {
            assert_eq!(
                got, job_id,
                "RunNowWithRunId must carry the job_id from the URL path"
            );
            assert!(
                run_id > 0,
                "RunNowWithRunId must carry a non-zero pre-inserted run_id"
            );
        }
        other => panic!("expected SchedulerCmd::RunNowWithRunId, got {:?}", other),
    }
}

#[tokio::test]
async fn run_now_returns_404_for_unknown_job() {
    let (app, _pool, mut cmd_rx) = build_test_app().await;

    // Do NOT seed any jobs — job id 999 must be unknown.
    let response = app
        .oneshot(build_run_now_request(999))
        .await
        .expect("oneshot");

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "unknown job ids must 404 so the UI can surface a clear error"
    );

    // And crucially: no SchedulerCmd should have been dispatched, because
    // the handler verifies the job exists before sending on the channel.
    assert!(
        cmd_rx.try_recv().is_err(),
        "no scheduler command may be dispatched for a nonexistent job"
    );
}
