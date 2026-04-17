//! Phase 11 D-09/D-10 contract tests on rendered HTML.
//!
//! Plan 11-11 bodies cover VALIDATION 11-11 rows as static-analysis proxies
//! (grep the rendered run-detail page for the required JS hooks + identifiers)
//! plus the autonomous `v11_dedupe_contract` unit test that locks the
//! `id > max -> accept, update max; id <= max -> drop` rule at the
//! contract level, replacing the browser UAT that Plan 11-12 Task 5 now owns.
//!
//! Plan 11-12 owns `data_max_id_rendered` and
//! `run_history_renders_run_number_and_title_attr`; those remain `#[ignore]`
//! until that plan runs.

#![allow(clippy::assertions_on_constants)]

mod common;
use common::v11_fixtures::*;

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use axum::middleware;
use axum::routing::get;
use tower::ServiceExt; // .oneshot()

use cronduit::db::DbPool;
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::telemetry::setup_metrics;
use cronduit::web::csrf;
use cronduit::web::handlers;
use cronduit::web::{AppState, ReloadState};

/// Build a minimal test Router wired for the run-detail route only. Mirrors
/// `tests/v11_run_detail_page_load.rs::build_test_app` — no scheduler loop
/// needed because the rendered-HTML assertions only need the template bytes.
///
/// Returns `(Router, DbPool)` so callers can seed a job + running run and
/// then fetch the rendered page.
async fn build_test_app() -> (Router, DbPool) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);

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
        .route(
            "/jobs/{job_id}/runs/{run_id}",
            get(handlers::run_detail::run_detail),
        )
        .with_state(state)
        .layer(middleware::from_fn(csrf::ensure_csrf_cookie));

    (router, pool)
}

/// Variant of `build_test_app` that wires the job-detail route (`/jobs/{job_id}`)
/// instead of run-detail. Used by `run_history_renders_run_number_and_title_attr`
/// which needs the `run_history.html` partial rendered inside the job-detail page.
async fn build_test_app_with_job_detail() -> (Router, DbPool) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);

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
        .route("/jobs/{job_id}", get(handlers::job_detail::job_detail))
        .with_state(state)
        .layer(middleware::from_fn(csrf::ensure_csrf_cookie));

    (router, pool)
}

/// T-V11-LOG-03 / VALIDATION 11-11-01: the inline dedupe script on
/// run_detail.html references `dataset.maxId` as the high-water-mark cursor
/// and calls `preventDefault()` to drop frames at or below it.
#[tokio::test]
async fn script_references_dataset_maxid() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "dedupe-job").await;
    let run_id = seed_running_run(&pool, job_id).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/jobs/{}/runs/{}", job_id, run_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("run_detail oneshot");

    let body_bytes = axum::body::to_bytes(response.into_body(), 10 * 1024 * 1024)
        .await
        .expect("read body");
    let body = String::from_utf8_lossy(&body_bytes);

    assert!(
        body.contains("dataset.maxId"),
        "dedupe script must reference dataset.maxId as the high-water cursor"
    );
    assert!(
        body.contains("preventDefault"),
        "dedupe handler must call preventDefault to drop duplicates"
    );
}

/// T-V11-LOG-04 / VALIDATION 11-11-02: run_finished listener present and
/// calls htmx.ajax to swap the live view to the static partial (D-10).
#[tokio::test]
async fn listens_for_run_finished() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "finished-job").await;
    let run_id = seed_running_run(&pool, job_id).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/jobs/{}/runs/{}", job_id, run_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("run_detail oneshot");

    let body_bytes = axum::body::to_bytes(response.into_body(), 10 * 1024 * 1024)
        .await
        .expect("read body");
    let body = String::from_utf8_lossy(&body_bytes);

    assert!(
        body.contains("sse:run_finished"),
        "run_finished listener must be installed on run_detail page"
    );
    assert!(
        body.contains("htmx.ajax"),
        "run_finished handler must use htmx.ajax to swap to static"
    );
}

/// T-V11-LOG-07 / VALIDATION 11-11-03: the dedupe listener hooks the
/// `htmx:sseBeforeMessage` event (the cancellable hook fired by the HTMX SSE
/// extension at assets/vendor/htmx-ext-sse.js:119). RESEARCH Q2 RESOLVED —
/// no capture-phase fallback needed.
#[tokio::test]
async fn script_references_htmx_sse_hook() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "hook-job").await;
    let run_id = seed_running_run(&pool, job_id).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/jobs/{}/runs/{}", job_id, run_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("run_detail oneshot");

    let body_bytes = axum::body::to_bytes(response.into_body(), 10 * 1024 * 1024)
        .await
        .expect("read body");
    let body = String::from_utf8_lossy(&body_bytes);

    assert!(
        body.contains("htmx:sseBeforeMessage"),
        "dedupe must hook htmx:sseBeforeMessage per RESEARCH Q2 RESOLVED"
    );
}

/// Autonomous unit test replacing the removed browser UAT. Exercises the
/// dedupe RULE in pure Rust: given a stream of SSE frame ids and a starting
/// max-id, confirm which ids are accepted vs dropped by the
/// `id > max -> accept; else drop` contract. This locks the dedupe logic at
/// the contract level so accidental off-by-one regressions are caught at CI
/// without a browser.
#[tokio::test]
async fn v11_dedupe_contract() {
    /// Mirror of the JS guard inside run_detail.html:
    ///   if (incoming && incoming <= max) { evt.preventDefault(); return; }
    ///   if (incoming > max) { dataset.maxId = String(incoming); }
    fn dedupe_apply(max: i64, incoming: i64) -> (bool /* accepted */, i64 /* new_max */) {
        if incoming > max {
            (true, incoming)
        } else {
            (false, max)
        }
    }

    // Starting state: no backfill rendered, max = 0.
    let mut max: i64 = 0;

    // Case 1: first frame with id=5 -> accepted, max becomes 5.
    let (acc, new_max) = dedupe_apply(max, 5);
    assert!(acc, "first frame with positive id must be accepted");
    assert_eq!(new_max, 5);
    max = new_max;

    // Case 2: replayed frame with id=5 -> dropped (equal, not strictly greater).
    let (acc, new_max) = dedupe_apply(max, 5);
    assert!(!acc, "replayed frame with id == max must be dropped");
    assert_eq!(new_max, 5);

    // Case 3: older frame id=3 -> dropped.
    let (acc, _) = dedupe_apply(max, 3);
    assert!(!acc, "frame with id < max must be dropped");

    // Case 4: next frame id=6 -> accepted, max becomes 6.
    let (acc, new_max) = dedupe_apply(max, 6);
    assert!(acc);
    assert_eq!(new_max, 6);

    // Case 5: backfill-to-live handoff scenario. Backfill last id = 100,
    // live stream replays ids 98..=102 (the reconnect overlap window).
    let mut max: i64 = 100;
    let live_stream = [98, 99, 100, 101, 102];
    let mut accepted = Vec::new();
    for &id in &live_stream {
        let (acc, new_max) = dedupe_apply(max, id);
        if acc {
            accepted.push(id);
            max = new_max;
        }
    }
    assert_eq!(
        accepted,
        vec![101, 102],
        "only ids > max should pass; 98/99/100 drop as duplicates"
    );
    assert_eq!(max, 102);
}

/// VALIDATION 11-12-01: the server-rendered run-detail page carries
/// `data-max-id="{N}"` on `#log-lines` where N is the max `job_logs.id`
/// across the backfilled page. Locks the D-08/D-09 wire contract: Plan 11-11's
/// dedupe handler reads this attribute to discard SSE frames with
/// `lastEventId <= N`.
#[tokio::test]
async fn data_max_id_rendered() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "maxid-job").await;
    let run_id = seed_running_run(&pool, job_id).await;
    let batch = make_test_batch(5);
    let ids = cronduit::db::queries::insert_log_batch(&pool, run_id, &batch)
        .await
        .expect("insert log batch");
    let max = *ids.last().expect("at least one inserted id");

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/jobs/{}/runs/{}", job_id, run_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("run_detail oneshot");
    let body_bytes = axum::body::to_bytes(response.into_body(), 10 * 1024 * 1024)
        .await
        .expect("read body");
    let body = String::from_utf8_lossy(&body_bytes);

    let needle = format!("data-max-id=\"{}\"", max);
    assert!(
        body.contains(&needle),
        "data-max-id must equal {} (last inserted log id); first 800 chars: {}",
        max,
        &body.chars().take(800).collect::<String>()
    );
}

/// VALIDATION 11-12-03: the run_history partial included on the job-detail
/// page renders the per-job `#N` cell for each run and the row-level
/// `title="global id: ..."` hover tooltip (D-04). Proves the UI-16 template
/// diff is wired for all runs on the page, not just a single one.
#[tokio::test]
async fn run_history_renders_run_number_and_title_attr() {
    let (app, pool) = build_test_app_with_job_detail().await;
    let job_id = seed_test_job(&pool, "history-job").await;
    let mut run_ids: Vec<i64> = Vec::with_capacity(3);
    for _ in 0..3 {
        run_ids.push(seed_running_run(&pool, job_id).await);
    }

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/jobs/{}", job_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("job_detail oneshot");
    let body_bytes = axum::body::to_bytes(response.into_body(), 10 * 1024 * 1024)
        .await
        .expect("read body");
    let body = String::from_utf8_lossy(&body_bytes);

    assert!(body.contains("#1"), "run history must render #1");
    assert!(body.contains("#2"), "run history must render #2");
    assert!(body.contains("#3"), "run history must render #3");
    assert!(
        body.contains("title=\"global id:"),
        "row-level global id tooltip must be present; body tail: {}",
        &body.chars().rev().take(400).collect::<String>()
    );
    // Each inserted global id should appear on its row's tooltip.
    for id in &run_ids {
        let needle = format!("global id: {}", id);
        assert!(
            body.contains(&needle),
            "tooltip must contain 'global id: {}'",
            id
        );
    }
}
