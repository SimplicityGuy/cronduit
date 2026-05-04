//! T-V11-STOP-15/16: stop_run handler integration tests (SCHED-14).
//!
//! Covers the four response branches of
//! `POST /api/runs/{run_id}/stop`:
//!
//! 1. `StopResult::Stopped`       → 200 + `HX-Trigger` (showToast) + `HX-Refresh`
//! 2. `StopResult::AlreadyFinalized` → 200 + `HX-Refresh` + **no** `HX-Trigger`
//!    (D-07 silent refresh — the refreshed page shows the truth)
//! 3. CSRF mismatch               → 403 "CSRF token mismatch"
//! 4. Scheduler channel closed    → 503 "Scheduler is shutting down"
//!
//! Modeled verbatim on `tests/api_run_now.rs` (10-PATTERNS.md §16).

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use tower::ServiceExt; // brings .oneshot()

use cronduit::db::DbPool;
use cronduit::scheduler::cmd::{SchedulerCmd, StopResult};
use cronduit::telemetry::setup_metrics;
use cronduit::web::csrf::CSRF_COOKIE_NAME;
use cronduit::web::handlers::api::stop_run;
use cronduit::web::{AppState, ReloadState};

/// Shared CSRF token used for both cookie and form field.
/// `validate_csrf` accepts any non-empty pair of equal-length byte strings.
const TEST_CSRF: &str = "phase10-stop-handler-csrf-token";

async fn seed_running_run(pool: &DbPool, job_name: &str) -> i64 {
    let job_id = cronduit::db::queries::upsert_job(
        pool,
        job_name,
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        "{}",
        "deadbeef",
        300,
        "[]",
    )
    .await
    .expect("upsert job");
    cronduit::db::queries::insert_running_run(pool, job_id, "manual", "testhash", None)
        .await
        .expect("insert running run")
}

/// Build a router with a mock scheduler task that replies to every
/// `SchedulerCmd::Stop` with the supplied `StopResult`. Returns
/// `(router, pool, run_id)`. The mock scheduler task is detached and will
/// exit when the channel sender is dropped at test teardown.
async fn build_app_with_scheduler_reply(reply: StopResult) -> (Router, DbPool, i64) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    let run_id = seed_running_run(&pool, "stop-handler-test").await;

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);

    // Mock scheduler: for every SchedulerCmd::Stop that arrives, reply with
    // the canned `reply` value. Other variants are ignored (tests only
    // dispatch Stop commands).
    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            if let SchedulerCmd::Stop { response_tx, .. } = cmd {
                let _ = response_tx.send(reply);
            }
        }
    });

    let state = AppState {
        started_at: chrono::Utc::now(),
        version: "test",
        pool: pool.clone(),
        cmd_tx,
        config_path: std::path::PathBuf::from("/tmp/cronduit-test.toml"),
        tz: chrono_tz::UTC,
        last_reload: Arc::new(Mutex::new(None::<ReloadState>)),
        watch_config: false,
        metrics_handle: setup_metrics(),
        active_runs: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    let router = Router::new()
        .route("/api/runs/{run_id}/stop", post(stop_run))
        .with_state(state);

    (router, pool, run_id)
}

fn build_stop_request(run_id: i64, cookie_token: &str, form_token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/api/runs/{}/stop", run_id))
        .header("cookie", format!("{}={}", CSRF_COOKIE_NAME, cookie_token))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(format!("csrf_token={}", form_token)))
        .expect("build request")
}

#[tokio::test]
async fn stop_run_happy_path() {
    let (app, _pool, run_id) = build_app_with_scheduler_reply(StopResult::Stopped).await;

    let response = app
        .oneshot(build_stop_request(run_id, TEST_CSRF, TEST_CSRF))
        .await
        .expect("oneshot");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "happy path must return 200"
    );

    let headers = response.headers();

    let hx_refresh = headers
        .get("HX-Refresh")
        .expect("HX-Refresh header must be present on Stopped branch");
    assert_eq!(
        hx_refresh, "true",
        "HX-Refresh must be \"true\" so the UI reloads and shows the stopped run"
    );

    let hx_trigger = headers
        .get("HX-Trigger")
        .expect("HX-Trigger header must be present on Stopped branch (toast)")
        .to_str()
        .expect("HX-Trigger header must be valid UTF-8");
    assert!(
        hx_trigger.contains("showToast"),
        "HX-Trigger must carry a showToast event, got: {hx_trigger}"
    );
    assert!(
        hx_trigger.contains("Stopped:"),
        "HX-Trigger toast message must start with \"Stopped:\", got: {hx_trigger}"
    );
    assert!(
        hx_trigger.contains("stop-handler-test"),
        "HX-Trigger toast must include the job name, got: {hx_trigger}"
    );
    assert!(
        hx_trigger.contains("\"level\":\"info\""),
        "HX-Trigger toast level must be info, got: {hx_trigger}"
    );
}

#[tokio::test]
async fn stop_run_already_finalized_silent_refresh() {
    let (app, _pool, run_id) = build_app_with_scheduler_reply(StopResult::AlreadyFinalized).await;

    let response = app
        .oneshot(build_stop_request(run_id, TEST_CSRF, TEST_CSRF))
        .await
        .expect("oneshot");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "AlreadyFinalized must still return 200 — the page refresh shows the truth"
    );

    let headers = response.headers();
    assert_eq!(
        headers
            .get("HX-Refresh")
            .expect("HX-Refresh must be set on AlreadyFinalized branch"),
        "true",
        "HX-Refresh must be \"true\" so the UI reloads"
    );

    assert!(
        headers.get("HX-Trigger").is_none(),
        "D-07: AlreadyFinalized must NOT emit HX-Trigger — silent refresh, no toast"
    );
}

#[tokio::test]
async fn stop_run_csrf_mismatch_returns_403() {
    let (app, _pool, run_id) = build_app_with_scheduler_reply(StopResult::Stopped).await;

    // Cookie token and form token differ — validate_csrf rejects.
    let response = app
        .oneshot(build_stop_request(
            run_id,
            "cookie-side-token-aaaaaaaaaaaa",
            "form-side-token-bbbbbbbbbbbbbb",
        ))
        .await
        .expect("oneshot");

    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "CSRF mismatch must return 403 and short-circuit before scheduler dispatch"
    );

    let body = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .expect("read body");
    assert!(
        body.as_ref().starts_with(b"CSRF token mismatch"),
        "403 body must be the exact string \"CSRF token mismatch\", got: {:?}",
        std::str::from_utf8(body.as_ref()).unwrap_or("<invalid utf8>")
    );
}

#[tokio::test]
async fn stop_run_channel_closed_returns_503() {
    // Build the app manually so we can drop the receiver before the request
    // fires — that forces the handler's `cmd_tx.send().await` to error and
    // exercise the 503 branch.
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");
    let run_id = seed_running_run(&pool, "stop-handler-channel-closed").await;

    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);
    drop(cmd_rx); // Receiver closed → sender's .send() will err.

    let state = AppState {
        started_at: chrono::Utc::now(),
        version: "test",
        pool,
        cmd_tx,
        config_path: std::path::PathBuf::from("/tmp/cronduit-test.toml"),
        tz: chrono_tz::UTC,
        last_reload: Arc::new(Mutex::new(None::<ReloadState>)),
        watch_config: false,
        metrics_handle: setup_metrics(),
        active_runs: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };
    let app = Router::new()
        .route("/api/runs/{run_id}/stop", post(stop_run))
        .with_state(state);

    let response = app
        .oneshot(build_stop_request(run_id, TEST_CSRF, TEST_CSRF))
        .await
        .expect("oneshot");

    assert_eq!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "channel-closed path must return 503"
    );

    let body = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .expect("read body");
    assert!(
        body.as_ref().starts_with(b"Scheduler is shutting down"),
        "503 body must be the exact string \"Scheduler is shutting down\", got: {:?}",
        std::str::from_utf8(body.as_ref()).unwrap_or("<invalid utf8>")
    );
}
