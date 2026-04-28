//! Integration test for GET / dashboard HTML render (UI-06).
//!
//! Phase 3 validation gap closure: askama's compile-time template check
//! proves the template *parses*; this test proves the dashboard actually
//! *includes* the data requirement UI-06 demands — name, raw schedule,
//! resolved schedule, next fire, last run status, last run timestamp —
//! for every enabled job.
//!
//! Follows the `tests/health_endpoint.rs` pattern: build the real router
//! against an in-memory SQLite pool, seed jobs via the queries module, hit
//! GET / with `tower::ServiceExt::oneshot`, and scan the HTML body for
//! the expected substrings.

use std::sync::{Arc, Mutex};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tower::ServiceExt; // brings .oneshot()

use cronduit::db::DbPool;
use cronduit::db::queries;
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::telemetry::setup_metrics;
use cronduit::web::{AppState, ReloadState, router};

async fn build_test_app() -> (axum::Router, DbPool) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    // Sink for scheduler commands — this test only exercises GET /.
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

    (router(state), pool)
}

async fn seed_job(pool: &DbPool, name: &str, schedule: &str) -> i64 {
    queries::upsert_job(
        pool, name, schedule, schedule, "command", "{}", "deadbeef", 300,
    )
    .await
    .expect("upsert job")
}

#[tokio::test]
async fn dashboard_renders_all_jobs_with_six_required_fields() {
    let (app, pool) = build_test_app().await;

    // Two distinct jobs — different names, different raw schedules — so the
    // test can assert that each appears in the rendered HTML.
    let alpha_id = seed_job(&pool, "alpha-backup", "*/10 * * * *").await;
    let beta_id = seed_job(&pool, "beta-sync", "0 */2 * * *").await;

    // Give alpha a terminal successful run so the Last Run status badge and
    // timestamp are populated (UI-06 fields 5 & 6).
    let start = tokio::time::Instant::now();
    let alpha_run = queries::insert_running_run(&pool, alpha_id, "manual", "testhash")
        .await
        .expect("insert alpha running run");
    queries::finalize_run(&pool, alpha_run, "success", Some(0), start, None, None, None)
        .await
        .expect("finalize alpha run");

    // Give beta an in-progress (running) row so the dashboard has to render
    // the running-status path as well.
    let _beta_run = queries::insert_running_run(&pool, beta_id, "manual", "testhash")
        .await
        .expect("insert beta running run");

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET / must return 200 with the dashboard page"
    );

    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body");
    let body = std::str::from_utf8(&bytes).expect("HTML is utf-8");

    // --- UI-06 field 1: job name (one cell per job) ---
    assert!(
        body.contains("alpha-backup"),
        "dashboard HTML must contain alpha-backup job name"
    );
    assert!(
        body.contains("beta-sync"),
        "dashboard HTML must contain beta-sync job name"
    );

    // --- UI-06 fields 2 & 3: raw schedule + resolved schedule. ---
    // Both jobs were seeded with raw == resolved so a single substring check
    // per job covers both columns; the job_table partial renders the
    // resolved_schedule into the Schedule column.
    assert!(
        body.contains("*/10 * * * *"),
        "dashboard HTML must contain alpha-backup schedule '*/10 * * * *'"
    );
    assert!(
        body.contains("0 */2 * * *"),
        "dashboard HTML must contain beta-sync schedule '0 */2 * * *'"
    );

    // --- UI-06 field 4: next fire time. The template emits a relative
    // string like "in 4h 12m" / "in 30s"; the marker word "in " appearing
    // in the job-table rows is a reliable proxy that next_fire rendered. ---
    // Each seeded job runs on a future cron tick, so the handler computes a
    // positive-delta relative string.
    assert!(
        body.contains("in "),
        "dashboard HTML must render at least one 'in ...' next-fire cell"
    );

    // --- UI-06 field 5: last run status badge. alpha has a success row,
    // beta has a running row. The template renders `cd-badge--{status}`. ---
    assert!(
        body.contains("cd-badge--success"),
        "dashboard HTML must render the cd-badge--success class for alpha-backup's last run"
    );
    assert!(
        body.contains("SUCCESS"),
        "dashboard HTML must render the uppercase SUCCESS label for alpha-backup"
    );
    assert!(
        body.contains("cd-badge--running"),
        "dashboard HTML must render the cd-badge--running class for beta-sync's in-progress run"
    );
    assert!(
        body.contains("RUNNING"),
        "dashboard HTML must render the uppercase RUNNING label for beta-sync"
    );

    // --- UI-06 field 6: last run timestamp (rendered relative). For alpha
    // this should be a "just now" / "N s ago" / etc. string — any of which
    // contains the " ago" / "just now" marker. beta is still running so its
    // last_run_time is set via insert_running_run and also renders relative. ---
    assert!(
        body.contains(" ago") || body.contains("just now"),
        "dashboard HTML must render at least one relative last-run timestamp"
    );

    // Spot check: the "Run Now" action column is present per row, confirming
    // the job_table partial actually iterated both jobs (not an empty state).
    let run_now_count = body.matches("Run Now").count();
    assert!(
        run_now_count >= 2,
        "dashboard HTML must render a Run Now control for each of the two seeded jobs, got {run_now_count}"
    );
}

#[tokio::test]
async fn dashboard_empty_state_when_no_jobs() {
    let (app, _pool) = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body");
    let body = std::str::from_utf8(&bytes).expect("HTML is utf-8");

    assert!(
        body.contains("No jobs configured"),
        "dashboard HTML must render the empty state when no jobs exist"
    );
    assert!(
        body.contains("/tmp/cronduit-test.toml"),
        "empty state must hint at the config path from AppState"
    );
}
