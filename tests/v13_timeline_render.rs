//! Integration tests for OBS-01 /timeline page rendering (Phase 13 plan 05).
//!
//! Covers the six scenarios locked in the plan's Task 4 behavior list:
//!   1. `timeline_returns_200_and_extends_base_layout`
//!   2. `empty_window_renders_message_24h`
//!   3. `empty_window_renders_message_7d`
//!   4. `timeline_renders_rows_per_job_alphabetical`
//!   5. `disabled_jobs_excluded`
//!   6. `running_run_has_pulsing_class`
//!
//! Builds the real router against an in-memory SQLite pool, seeds jobs +
//! runs via the canonical `queries::*` functions, hits `/timeline` (or
//! `/timeline?window=7d`) via `ServiceExt::oneshot`, and scans the rendered
//! HTML body for UI-SPEC copywriting + status class substrings.

use std::sync::{Arc, Mutex};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tower::ServiceExt; // brings .oneshot()

use cronduit::db::DbPool;
use cronduit::db::queries;
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::telemetry::setup_metrics;
use cronduit::web::{AppState, ReloadState, router};

// ---------------------------------------------------------------------------
// Test app harness (mirrors tests/dashboard_render.rs + tests/v13_sparkline_render.rs)
// ---------------------------------------------------------------------------

async fn build_test_app() -> (axum::Router, DbPool) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    // Sink for scheduler commands — these tests only exercise GET /timeline.
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

async fn seed_job(pool: &DbPool, name: &str) -> i64 {
    queries::upsert_job(
        pool,
        name,
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo hello"}"#,
        &format!("hash-{name}"),
        3600,
    )
    .await
    .expect("upsert job")
}

/// Seed a terminal run with the given status. Duration is whatever elapsed
/// between `insert_running_run` and `finalize_run` (a few microseconds —
/// immaterial here, since every assertion checks rendered strings and
/// status classes, not durations).
async fn seed_terminal_run(pool: &DbPool, job_id: i64, status: &str) {
    let run_id = queries::insert_running_run(pool, job_id, "scheduled", "testhash")
        .await
        .expect("insert running run");
    let start = tokio::time::Instant::now();
    queries::finalize_run(pool, run_id, status, Some(0), start, None, None, None)
        .await
        .expect("finalize run");
}

/// Seed a still-running run (no `finalize_run`). Returns the run_id so tests
/// can reference it in href assertions.
async fn seed_running_run(pool: &DbPool, job_id: i64) -> i64 {
    queries::insert_running_run(pool, job_id, "scheduled", "testhash")
        .await
        .expect("insert running run")
}

/// GET `/timeline` (or an explicit URI) and return the body, asserting 200 OK.
async fn fetch_timeline_body(app: axum::Router, uri: &str) -> String {
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET {uri} must return 200"
    );

    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body");
    String::from_utf8(bytes.to_vec()).expect("HTML is utf-8")
}

// ---------------------------------------------------------------------------
// Test 1 — 200 OK + base layout pieces present
// ---------------------------------------------------------------------------

#[tokio::test]
async fn timeline_returns_200_and_extends_base_layout() {
    let (app, _pool) = build_test_app().await;

    let body = fetch_timeline_body(app, "/timeline").await;

    assert!(
        body.contains("Timeline - Cronduit"),
        "timeline page must set <title>Timeline - Cronduit</title>"
    );
    assert!(
        body.contains("cronduit"),
        "timeline page must carry the cronduit brand nav link (base.html inheritance)"
    );
    // Nav must include a Timeline link entry (from base.html).
    assert!(
        body.contains("href=\"/timeline\""),
        "timeline page must render at least one /timeline nav href (base.html + pill)"
    );
    // The <h1>Timeline</h1> lives inside the content block.
    assert!(
        body.contains(">Timeline<"),
        "timeline page must render the <h1>Timeline</h1> heading"
    );
}

// ---------------------------------------------------------------------------
// Test 2 — Empty-window message for 24h default
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_window_renders_message_24h() {
    let (app, _pool) = build_test_app().await;

    let body = fetch_timeline_body(app, "/timeline").await;

    assert!(
        body.contains("No runs in the last 24h."),
        "empty 24h window must render the UI-SPEC copywriting line 1"
    );
    assert!(
        body.contains("Try widening the window to 7d."),
        "empty 24h window must render the UI-SPEC copywriting line 2"
    );
}

// ---------------------------------------------------------------------------
// Test 3 — Empty-window message for 7d
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_window_renders_message_7d() {
    let (app, _pool) = build_test_app().await;

    let body = fetch_timeline_body(app, "/timeline?window=7d").await;

    assert!(
        body.contains("No runs in the last 7d."),
        "empty 7d window must render the UI-SPEC copywriting line 1"
    );
    assert!(
        body.contains("Configure a job and run it to populate the timeline."),
        "empty 7d window must render the UI-SPEC copywriting line 2"
    );
}

// ---------------------------------------------------------------------------
// Test 4 — Row-per-job alphabetical ordering
// ---------------------------------------------------------------------------

#[tokio::test]
async fn timeline_renders_rows_per_job_alphabetical() {
    let (app, pool) = build_test_app().await;

    // Seed in intentionally-non-alphabetical insertion order to prove the
    // rendered HTML is sorted by name, not by insertion or id.
    let zeta_id = seed_job(&pool, "zeta-backup").await;
    let alpha_id = seed_job(&pool, "alpha-cron").await;
    let middle_id = seed_job(&pool, "middle-sync").await;

    seed_terminal_run(&pool, zeta_id, "success").await;
    seed_terminal_run(&pool, alpha_id, "success").await;
    seed_terminal_run(&pool, middle_id, "success").await;

    let body = fetch_timeline_body(app, "/timeline").await;

    // All three job names present.
    assert!(
        body.contains("alpha-cron"),
        "alpha-cron must appear in timeline"
    );
    assert!(
        body.contains("middle-sync"),
        "middle-sync must appear in timeline"
    );
    assert!(
        body.contains("zeta-backup"),
        "zeta-backup must appear in timeline"
    );

    // Row order is alphabetical: first-byte-offset of each job name in the
    // rendered body must be strictly monotonic (alpha < middle < zeta).
    let alpha_pos = body.find("alpha-cron").expect("alpha-cron offset");
    let middle_pos = body.find("middle-sync").expect("middle-sync offset");
    let zeta_pos = body.find("zeta-backup").expect("zeta-backup offset");

    assert!(
        alpha_pos < middle_pos,
        "alpha-cron must render before middle-sync in alphabetical row order (alpha={alpha_pos}, middle={middle_pos})"
    );
    assert!(
        middle_pos < zeta_pos,
        "middle-sync must render before zeta-backup in alphabetical row order (middle={middle_pos}, zeta={zeta_pos})"
    );
}

// ---------------------------------------------------------------------------
// Test 5 — Disabled jobs excluded from timeline rows
// ---------------------------------------------------------------------------

#[tokio::test]
async fn disabled_jobs_excluded() {
    let (app, pool) = build_test_app().await;

    let enabled_id = seed_job(&pool, "enabled-job").await;
    let disabled_id = seed_job(&pool, "disabled-job").await;

    seed_terminal_run(&pool, enabled_id, "success").await;
    seed_terminal_run(&pool, disabled_id, "success").await;

    // Disable one job by calling `disable_missing_jobs` with the enabled one
    // as the only "active" name. The query's `j.enabled = 1` filter then
    // excludes disabled-job from the timeline rows.
    let rows_disabled = queries::disable_missing_jobs(&pool, &["enabled-job".to_string()])
        .await
        .expect("disable missing jobs");
    assert_eq!(
        rows_disabled, 1,
        "exactly one job (disabled-job) must have been disabled, got {rows_disabled}"
    );
    // Silence the warning about the unused id — the id is captured only to
    // confirm the seed happened, not to reference it later.
    let _ = disabled_id;

    let body = fetch_timeline_body(app, "/timeline").await;

    assert!(
        body.contains("enabled-job"),
        "enabled-job must appear in timeline"
    );
    assert!(
        !body.contains("disabled-job"),
        "disabled-job must NOT appear in timeline (j.enabled = 1 filter in get_timeline_runs)"
    );
}

// ---------------------------------------------------------------------------
// Test 6 — Running runs render with pulsing class
// ---------------------------------------------------------------------------

#[tokio::test]
async fn running_run_has_pulsing_class() {
    let (app, pool) = build_test_app().await;

    let job_id = seed_job(&pool, "running-job").await;
    let _run_id = seed_running_run(&pool, job_id).await;

    let body = fetch_timeline_body(app, "/timeline").await;

    assert!(
        body.contains("running-job"),
        "running-job must render a row on the timeline"
    );
    assert!(
        body.contains("cd-timeline-bar--pulsing"),
        "running runs must render with the cd-timeline-bar--pulsing class (D-11 animation)"
    );
    // Running bars also carry the --running status class.
    assert!(
        body.contains("cd-timeline-bar--running"),
        "running bars must render with the cd-timeline-bar--running status class"
    );
}
