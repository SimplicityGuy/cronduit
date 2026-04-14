//! Integration tests for the read-only JSON job API endpoints added in
//! Phase 8 Bug-1 fix: GET /api/jobs and GET /api/jobs/{id}/runs.
//!
//! These endpoints unblock the compose-smoke CI matrix (08-04) which needs
//! to resolve job names to ids and poll run status without HTML scraping.
//! They are read-only and do NOT require CSRF — unlike the form-based
//! POST /api/jobs/{id}/run handler.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use axum::routing::get;
use serde_json::Value;
use tower::ServiceExt;

use cronduit::db::DbPool;
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::telemetry::setup_metrics;
use cronduit::web::handlers::api::{list_job_runs, list_jobs};
use cronduit::web::{AppState, ReloadState};

async fn build_test_app() -> (Router, DbPool) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    // Sink for scheduler commands — these tests don't exercise POST routes.
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
        .route("/api/jobs", get(list_jobs))
        .route("/api/jobs/{id}/runs", get(list_job_runs))
        .with_state(state);

    (router, pool)
}

/// Seed a single job row directly via SQL so tests don't depend on config sync.
async fn seed_job(pool: &DbPool, name: &str, job_type: &str) -> i64 {
    cronduit::db::queries::upsert_job(
        pool,
        name,
        "*/5 * * * *",
        "*/5 * * * *",
        job_type,
        "{}",
        "deadbeef",
        300,
    )
    .await
    .expect("upsert job")
}

async fn body_to_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body");
    serde_json::from_slice(&bytes).expect("body is JSON")
}

#[tokio::test]
async fn get_api_jobs_returns_empty_array_with_no_jobs() {
    let (app, _pool) = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/jobs")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body.as_array().expect("array").len(), 0);
}

#[tokio::test]
async fn get_api_jobs_returns_seeded_jobs() {
    let (app, pool) = build_test_app().await;
    seed_job(&pool, "alpha", "command").await;
    seed_job(&pool, "beta", "docker").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/jobs")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    let arr = body.as_array().expect("array");
    assert_eq!(arr.len(), 2, "both seeded jobs present");

    // Sorted by name ASC (default order from get_dashboard_jobs).
    let names: Vec<&str> = arr.iter().map(|j| j["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["alpha", "beta"]);

    // Every row exposes the fields the compose-smoke CI needs to map
    // name → id and inspect schedule.
    for row in arr {
        assert!(
            row["id"].is_i64(),
            "id must be numeric for POST /run resolution"
        );
        assert!(row["name"].is_string());
        assert!(row["schedule"].is_string());
        assert!(row["type"].is_string());
    }
}

#[tokio::test]
async fn get_api_jobs_id_runs_returns_404_for_unknown_job() {
    let (app, _pool) = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/jobs/999/runs")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "unknown job ids return 404 so CI can distinguish missing job from empty history"
    );
}

#[tokio::test]
async fn get_api_jobs_id_runs_returns_empty_array_for_job_with_no_runs() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_job(&pool, "empty-history", "command").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/jobs/{}/runs", job_id))
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body.as_array().expect("array").len(), 0);
}

#[tokio::test]
async fn get_api_jobs_id_runs_honors_limit_query_param() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_job(&pool, "with-runs", "command").await;

    // Seed 5 runs directly through the queries module.
    for _ in 0..5 {
        let run_id = cronduit::db::queries::insert_running_run(&pool, job_id, "manual")
            .await
            .expect("insert run");
        let start = tokio::time::Instant::now();
        cronduit::db::queries::finalize_run(&pool, run_id, "success", Some(0), start, None, None)
            .await
            .expect("finalize run");
    }

    // Default limit: should return all 5.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/jobs/{}/runs", job_id))
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body.as_array().expect("array").len(), 5);

    // Explicit limit=1: should return just the most recent.
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/jobs/{}/runs?limit=1", job_id))
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    let arr = body.as_array().expect("array");
    assert_eq!(arr.len(), 1, "limit=1 must return exactly one run");
    assert_eq!(arr[0]["status"].as_str().unwrap(), "success");
    assert!(arr[0]["id"].is_i64());
    assert!(arr[0]["job_id"].is_i64());
}
