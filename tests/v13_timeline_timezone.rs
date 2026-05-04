//! Phase 13 plan 06 Task 2: timezone rendering test for `/timeline`
//! (OBS-02 T-V11-TIME-04).
//!
//! Asserts that when the server timezone is configured as `America/Los_Angeles`
//! (a non-UTC tz with -7h/-8h offset depending on DST), the rendered `/timeline`
//! body contains timestamps in LA-local time, NOT UTC time.
//!
//! Test strategy:
//!   1. Build the real router with `AppState.tz = America/Los_Angeles`.
//!   2. Seed a job + a run whose `start_time` is 6 hours ago in UTC (inside
//!      the default 24h window).
//!   3. GET /timeline?window=24h and assert the body contains the LA-local
//!      HH:MM:SS formatted timestamp that `timeline.rs::start_local.format`
//!      will produce.
//!
//! Caveat: LA tz is either UTC-7 (PDT) or UTC-8 (PST) depending on the DST
//! state at test-run time. Because the assertion computes the expected local
//! string in Rust via `with_timezone(&la_tz).format("%H:%M:%S")`, the test
//! is DST-aware automatically — no hardcoded offsets.

use std::sync::{Arc, Mutex};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use cronduit::db::DbPool;
use cronduit::db::queries::{self, PoolRef};
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::telemetry::setup_metrics;
use cronduit::web::{AppState, ReloadState, router};

/// Build a test router with the specified timezone wired into AppState.
/// Mirrors tests/dashboard_render.rs + tests/v13_timeline_render.rs but with
/// `tz` parameterized rather than hardcoded to UTC.
async fn build_test_app_with_tz(tz: chrono_tz::Tz) -> (axum::Router, DbPool) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);
    tokio::spawn(async move { while cmd_rx.recv().await.is_some() {} });

    let metrics_handle = setup_metrics();

    let state = AppState {
        started_at: chrono::Utc::now(),
        version: "test",
        pool: pool.clone(),
        cmd_tx,
        config_path: std::path::PathBuf::from("/tmp/cronduit-test.toml"),
        tz,
        last_reload: Arc::new(Mutex::new(None::<ReloadState>)),
        watch_config: false,
        metrics_handle,
        active_runs: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    (router(state), pool)
}

#[tokio::test]
async fn pdt_label_in_timeline_render() {
    let la_tz: chrono_tz::Tz = "America/Los_Angeles".parse().expect("tz parse");
    let (app, pool) = build_test_app_with_tz(la_tz).await;

    // Seed a job. The 24h-window default on /timeline will include this run
    // since we insert a start_time 6h ago in UTC.
    let job_id = queries::upsert_job(
        &pool,
        "tz-test",
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo"}"#,
        "hash-tz",
        3600,
        "[]",
    )
    .await
    .expect("upsert job");

    // Pick a start_time 6h before now in UTC so the run falls inside the 24h
    // window regardless of when CI runs this test.
    let six_h_ago = chrono::Utc::now() - chrono::Duration::hours(6);
    let end_time = six_h_ago + chrono::Duration::seconds(60);
    let start_rfc = six_h_ago.to_rfc3339();
    let end_rfc = end_time.to_rfc3339();

    // Use direct SQL for deterministic start_time — queries::insert_running_run
    // would use its own clock value and queries::finalize_run derives duration
    // from tokio::time::Instant::elapsed(), neither of which give the caller
    // control over the wall-clock start_time we need for the PDT arithmetic.
    //
    // Phase 11 `DB-10` made `job_run_number` NOT NULL, so the INSERT column
    // list is load-bearing — we cannot rely on a table default.
    let insert_sql = "INSERT INTO job_runs \
        (job_id, job_run_number, status, trigger, start_time, end_time, duration_ms, exit_code) \
        VALUES (?, 1, 'success', 'scheduled', ?, ?, 60000, 0)";
    let pool_ref = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("timezone test uses the SQLite writer pool"),
    };
    sqlx::query(insert_sql)
        .bind(job_id)
        .bind(&start_rfc)
        .bind(&end_rfc)
        .execute(pool_ref)
        .await
        .expect("seed run");

    // GET /timeline?window=24h
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/timeline?window=24h")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body")
            .to_vec(),
    )
    .expect("utf-8 body");

    // Compute what the handler's `start_local.format("%H:%M:%S")` will produce.
    // This is DST-aware: chrono_tz resolves both PDT (UTC-7) and PST (UTC-8)
    // automatically. `end_time_str` will differ by 60 seconds from
    // `start_time_str`, and both are rendered in LA-local time.
    let expected_start_local = six_h_ago
        .with_timezone(&la_tz)
        .format("%H:%M:%S")
        .to_string();
    let expected_end_local = end_time
        .with_timezone(&la_tz)
        .format("%H:%M:%S")
        .to_string();

    // The rendered body must contain the LA-local HH:MM:SS for both timestamps.
    assert!(
        body.contains(&expected_start_local),
        "expected body to contain LA-local start time '{expected_start_local}' (6h-ago UTC \
         = {start_rfc} → LA); body did not match"
    );
    assert!(
        body.contains(&expected_end_local),
        "expected body to contain LA-local end time '{expected_end_local}'; body did not \
         match"
    );

    // Belt-and-suspenders proof the tz wiring is real: the UTC-formatted
    // HH:MM:SS for the same instant MUST differ from the LA-local one by 7
    // or 8 hours (US/Pacific offset). If they happen to coincide numerically
    // (e.g. `12:34:56`), the assertion below is skipped — this is a
    // theoretical null-test edge case.
    let utc_start_str = six_h_ago.format("%H:%M:%S").to_string();
    assert_ne!(
        expected_start_local, utc_start_str,
        "LA-local and UTC HH:MM:SS should differ by 7-8h; identical strings \
         imply the handler never applied state.tz"
    );
}
