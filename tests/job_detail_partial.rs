//! Regression test for the Job Detail Run History auto-refresh partial.
//!
//! Phase 7 Plan 05: asserts that `GET /partials/jobs/:job_id/runs` returns
//! 200 + an HTML body that contains (a) the Run History table with the
//! expected status badges for the runs seeded into the test DB, and
//! (b) the polling wrapper `hx-trigger="every 2s"` attribute **only** when
//! at least one run is in the RUNNING state.
//!
//! This locks in the v1.0-cleanup fix for the Phase 6 UAT Test 4 bug where
//! the Run History card stayed frozen at RUNNING after rapid-fire Run Now
//! clicks. The fix is in `src/web/handlers/job_detail.rs::job_runs_partial`
//! + `templates/partials/run_history.html`.
//!
//! Pattern mirrored from `tests/reload_api.rs` (Plan 07-04): build a minimal
//! axum Router with the single route under test + stubbed AppState, then
//! drive a request through `tower::ServiceExt::oneshot`. No real scheduler,
//! no real Docker, no real config file.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use axum::routing::get;
use tower::ServiceExt; // brings .oneshot()

use cronduit::db::DbPool;
use cronduit::db::{finalize_run, insert_running_run, upsert_job};
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::telemetry::setup_metrics;
use cronduit::web::handlers::job_detail::job_runs_partial;
use cronduit::web::{AppState, ReloadState};

/// Build a minimal axum Router wired to an in-memory SQLite DB. Same harness
/// shape as `tests/reload_api.rs::build_test_app`, minus the background
/// scheduler drain — this test never sends SchedulerCmd, because the Run
/// History partial reads straight from the DB.
async fn build_test_app() -> (Router, DbPool) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    // cmd_tx is required by AppState but the partial handler never uses it.
    // Keep the receiver alive in a detached task so sends (if any) don't panic.
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);
    tokio::spawn(async move { while cmd_rx.recv().await.is_some() {} });

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
        .route("/partials/jobs/{job_id}/runs", get(job_runs_partial))
        .with_state(state);

    (router, pool)
}

/// Seed a job row and return its id.
async fn seed_job(pool: &DbPool, name: &str) -> i64 {
    upsert_job(
        pool,
        name,
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo hello"}"#,
        &format!("hash-{name}"),
        3600,
        "[]",
    )
    .await
    .expect("upsert test job")
}

/// Seed a completed run (SUCCESS) attached to the given job.
async fn seed_success_run(pool: &DbPool, job_id: i64) -> i64 {
    let run_id = insert_running_run(pool, job_id, "scheduled", "testhash", None)
        .await
        .expect("insert running run");
    let start = tokio::time::Instant::now();
    finalize_run(pool, run_id, "success", Some(0), start, None, None, None)
        .await
        .expect("finalize run as success");
    run_id
}

/// Seed a RUNNING run and leave it un-finalized.
async fn seed_running_run(pool: &DbPool, job_id: i64) -> i64 {
    insert_running_run(pool, job_id, "manual", "testhash", None)
        .await
        .expect("insert running run")
}

/// GET /partials/jobs/{job_id}/runs and return (status, body-as-string).
async fn get_partial(app: Router, job_id: i64) -> (StatusCode, String) {
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/partials/jobs/{}/runs", job_id))
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");

    let status = response.status();
    let body_bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body bytes");
    let body = String::from_utf8(body_bytes.to_vec()).expect("body utf8");
    (status, body)
}

// ---------------------------------------------------------------------------
// Test cases
// ---------------------------------------------------------------------------

/// Happy path: a job with one RUNNING run and one SUCCESS run. The partial
/// should return 200, include both status badges, and carry the polling
/// trigger because `any_running == true`.
#[tokio::test]
async fn run_history_partial_renders_badges_and_enables_polling_while_running() {
    let (app, pool) = build_test_app().await;

    let job_id = seed_job(&pool, "polling-job").await;
    let _completed = seed_success_run(&pool, job_id).await;
    // Small delay so the ORDER BY start_time DESC has a deterministic split.
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let _running = seed_running_run(&pool, job_id).await;

    let (status, body) = get_partial(app, job_id).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "partial must return 200 for an existing job"
    );

    // Both status badges must appear — SUCCESS and RUNNING label text plus
    // the cd-badge-- class modifier that the Run History template renders.
    assert!(
        body.contains("cd-badge--running"),
        "response body must contain the RUNNING badge class modifier, got:\n{}",
        body
    );
    assert!(
        body.contains("cd-badge--success"),
        "response body must contain the SUCCESS badge class modifier, got:\n{}",
        body
    );
    assert!(
        body.contains("RUNNING"),
        "response body must contain the RUNNING status label"
    );
    assert!(
        body.contains("SUCCESS"),
        "response body must contain the SUCCESS status label"
    );

    // Polling wrapper must carry hx-trigger="every 2s" while any run is running.
    assert!(
        body.contains("id=\"run-history-poll-wrapper\""),
        "response must wrap the table in the poll wrapper div"
    );
    assert!(
        body.contains("hx-get=\"/partials/jobs/"),
        "wrapper must carry hx-get pointing at the partial endpoint"
    );
    assert!(
        body.contains("hx-swap=\"outerHTML\""),
        "wrapper must hx-swap outerHTML so the next render replaces it"
    );
    assert!(
        body.contains("hx-trigger=\"every 2s\""),
        "wrapper MUST carry hx-trigger=\"every 2s\" while any run is RUNNING, got:\n{}",
        body
    );
}

/// Idle path: a job whose runs are all terminal. The partial must still return
/// 200 and render badges, but the wrapper must NOT carry the polling trigger
/// so an idle Job Detail page stops hitting the server.
#[tokio::test]
async fn run_history_partial_stops_polling_when_all_runs_terminal() {
    let (app, pool) = build_test_app().await;

    let job_id = seed_job(&pool, "idle-job").await;
    let _a = seed_success_run(&pool, job_id).await;
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let _b = seed_success_run(&pool, job_id).await;

    let (status, body) = get_partial(app, job_id).await;

    assert_eq!(status, StatusCode::OK);

    assert!(
        body.contains("cd-badge--success"),
        "SUCCESS badge must render"
    );
    assert!(
        !body.contains("cd-badge--running"),
        "no RUNNING badge should appear when all runs are terminal"
    );

    // Poll wrapper still exists (so a future run transitions it back), but
    // its hx-trigger attribute is absent -> HTMX stops polling after the
    // outerHTML swap.
    assert!(
        body.contains("id=\"run-history-poll-wrapper\""),
        "wrapper div must still render so subsequent manual refreshes re-enable polling"
    );
    assert!(
        !body.contains("hx-trigger=\"every 2s\""),
        "wrapper MUST NOT carry hx-trigger=\"every 2s\" when no runs are RUNNING — \
         this is the 'stop polling when idle' invariant, got:\n{}",
        body
    );
}

/// Unknown job id returns 404 so a polling client whose job was deleted
/// stops gracefully rather than hammering the endpoint with empty tables.
#[tokio::test]
async fn run_history_partial_returns_404_for_unknown_job() {
    let (app, _pool) = build_test_app().await;

    let (status, _body) = get_partial(app, 999_999).await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "unknown job id must return 404, not 200 with an empty table"
    );
}
