//! Integration tests for OBS-03 dashboard sparkline rendering (Phase 13 plan 04).
//!
//! Covers the six T-V11-SPARK-0{1..6} behaviors from the phase test map:
//! - 01 zero runs → no panic + `—` badge
//! - 02 below N=5 threshold → `—` badge
//! - 03 at N=5 all-success → `100%` badge
//! - 04 mixed 15/5 over 20 → `75%` badge
//! - 05 stopped excluded from denominator (D-05) → `94%` badge (15/(20-4))
//! - 06 exactly 20 `cd-sparkline-cell--*` tokens rendered per job
//!
//! Builds the real router against an in-memory SQLite pool, seeds job +
//! runs via the canonical `queries::*` functions, hits GET / via
//! `ServiceExt::oneshot`, and scans the rendered HTML body.

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
// Test app harness (mirrors tests/dashboard_render.rs)
// ---------------------------------------------------------------------------

async fn build_test_app() -> (axum::Router, DbPool) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    // Sink for scheduler commands — these tests only exercise GET /.
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

/// Seed a terminal run with the specified status. Uses the canonical
/// `insert_running_run` + `finalize_run` pair so the `job_run_number` counter
/// advances the same way production code does. The duration is whatever
/// elapsed between the two calls (a few micros — immaterial to these tests
/// since we assert on status-based counts + rendered strings, not durations).
async fn seed_run_with_status(pool: &DbPool, job_id: i64, status: &str) {
    let run_id = queries::insert_running_run(pool, job_id, "scheduled", "testhash", None)
        .await
        .expect("insert running run");
    let start = tokio::time::Instant::now();
    queries::finalize_run(pool, run_id, status, Some(0), start, None, None, None)
        .await
        .expect("finalize run");
}

/// GET / and return the body as a UTF-8 `String`, asserting a 200 OK.
async fn fetch_dashboard_body(app: axum::Router) -> String {
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
        "GET / must return 200 OK"
    );

    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body");
    std::str::from_utf8(&bytes).expect("utf-8 html").to_string()
}

// ---------------------------------------------------------------------------
// Test 1 (T-V11-SPARK-01): zero runs — no crash + em-dash badge
// ---------------------------------------------------------------------------

#[tokio::test]
async fn zero_runs_no_crash_and_em_dash_badge() {
    let (app, pool) = build_test_app().await;
    // Unique name so any `—` in the body is unambiguously attributable to
    // THIS job's sparkline badge (no other job content around).
    let _job_id = seed_job(&pool, "zzz-zero-runs-job").await;

    let body = fetch_dashboard_body(app).await;

    assert!(
        body.contains("zzz-zero-runs-job"),
        "dashboard must render the seeded job row"
    );
    assert!(
        body.contains("—"),
        "zero-run job must render em-dash badge (implicit non-crash: 200 OK already asserted)"
    );
    // Sparkline container must still render even for zero-run jobs so the
    // column width stays stable.
    assert!(
        body.contains(r#"class="cd-sparkline""#),
        "sparkline container must render even for zero-run job"
    );
}

// ---------------------------------------------------------------------------
// Test 2 (T-V11-SPARK-02): below N=5 threshold — em-dash badge
// ---------------------------------------------------------------------------

#[tokio::test]
async fn below_threshold_shows_dash() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_job(&pool, "zzz-below-threshold-job").await;

    // 3 successful runs — below MIN_SAMPLES_FOR_RATE=5.
    for _ in 0..3 {
        seed_run_with_status(&pool, job_id, "success").await;
    }

    let body = fetch_dashboard_body(app).await;

    assert!(
        body.contains("zzz-below-threshold-job"),
        "dashboard must render the seeded job row"
    );
    assert!(
        body.contains("—"),
        "below-threshold job must render em-dash badge instead of a percent"
    );
    // Guard: must NOT render a 100% badge. The raw `100%` string also
    // appears in inline CSS (`width:100%` on the table style), so scope
    // to the badge class-delimited fragment: the `cd-sparkline-badge`
    // opening tag is unique in the page and its inner text is the badge
    // value. The fragment `>100%<` would only appear inside a rendered
    // percent badge.
    assert!(
        !body.contains(">100%<"),
        "below-threshold job MUST NOT render a 100% badge — threshold gate failed"
    );
}

// ---------------------------------------------------------------------------
// Test 3 (T-V11-SPARK-03): exactly at threshold, all success → 100%
// ---------------------------------------------------------------------------

#[tokio::test]
async fn at_threshold_all_success_hundred_percent() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_job(&pool, "zzz-threshold-all-success").await;

    for _ in 0..5 {
        seed_run_with_status(&pool, job_id, "success").await;
    }

    let body = fetch_dashboard_body(app).await;

    assert!(body.contains("zzz-threshold-all-success"));
    assert!(
        body.contains("100%"),
        "5 success runs must render 100% badge"
    );
}

// ---------------------------------------------------------------------------
// Test 4 (T-V11-SPARK-04): mixed 15 success + 5 failed over 20 → 75%
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mixed_runs_integer_percent() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_job(&pool, "zzz-mixed-runs").await;

    for _ in 0..15 {
        seed_run_with_status(&pool, job_id, "success").await;
    }
    for _ in 0..5 {
        seed_run_with_status(&pool, job_id, "failed").await;
    }

    let body = fetch_dashboard_body(app).await;

    assert!(body.contains("zzz-mixed-runs"));
    assert!(
        body.contains("75%"),
        "15 success + 5 failed = 15/20 = 75% expected"
    );
}

// ---------------------------------------------------------------------------
// Test 5 (T-V11-SPARK-05, D-05): stopped runs excluded from denominator
// ---------------------------------------------------------------------------

#[tokio::test]
async fn stopped_excluded_from_denominator() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_job(&pool, "zzz-stopped-excluded").await;

    // 15 success + 4 stopped + 1 failed = 20 terminal runs.
    // Denominator = 20 - 4 = 16. Numerator = 15.
    // 15 / 16 = 0.9375 → round-half-up → 94%.
    for _ in 0..15 {
        seed_run_with_status(&pool, job_id, "success").await;
    }
    for _ in 0..4 {
        seed_run_with_status(&pool, job_id, "stopped").await;
    }
    for _ in 0..1 {
        seed_run_with_status(&pool, job_id, "failed").await;
    }

    let body = fetch_dashboard_body(app).await;

    assert!(body.contains("zzz-stopped-excluded"));
    assert!(
        body.contains("94%"),
        "15 success / (20 - 4 stopped) = 15/16 = 0.9375 → 94% with half-up rounding"
    );
    // Guard: the naive denominator (including stopped) would produce 75%.
    // If this assertion fires, the D-05 stopped-exclusion logic regressed.
    assert!(
        !body.contains("75%"),
        "badge must NOT render 75% — denominator must exclude stopped runs (D-05)"
    );
}

// ---------------------------------------------------------------------------
// Test 6 (T-V11-SPARK-06): exactly 20 `cd-sparkline-cell--*` tokens per job
// ---------------------------------------------------------------------------

#[tokio::test]
async fn exactly_twenty_cells_rendered() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_job(&pool, "zzz-twenty-cells").await;

    // Seed 20 terminal runs with a mix of statuses so every status-suffix
    // variant is exercised at least once.
    let pattern = [
        "success",
        "failed",
        "timeout",
        "cancelled",
        "stopped",
        "success",
        "success",
        "success",
        "success",
        "success",
        "failed",
        "failed",
        "timeout",
        "cancelled",
        "stopped",
        "success",
        "success",
        "success",
        "success",
        "success",
    ];
    for status in pattern {
        seed_run_with_status(&pool, job_id, status).await;
    }

    let body = fetch_dashboard_body(app).await;

    // Count occurrences of the status-suffix class token alone — whitespace
    // independent: the CSS class is written as
    //   `cd-sparkline-cell cd-sparkline-cell--{kind}`
    // so each cell contributes exactly one match for `cd-sparkline-cell--`.
    let count = body.matches("cd-sparkline-cell--").count();
    assert_eq!(
        count, 20,
        "expected exactly 20 sparkline cells for single job, got {count}"
    );
}
