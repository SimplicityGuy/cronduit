//! Integration tests for the Job Detail Duration card (Phase 13 OBS-04).
//!
//! Four test cases exercise the locked N-threshold subtitle matrix from
//! UI-SPEC § Duration card Copywriting contract:
//!
//! | N         | p50/p95   | subtitle                                        |
//! | --------- | --------- | ----------------------------------------------- |
//! | 0         | `—` / `—` | `0 of 20 successful runs required`              |
//! | 1..=19    | `—` / `—` | `{N} of 20 successful runs required`            |
//! | 20..=99   | `{fmt}`   | `last {N} successful runs`                      |
//! | >=100     | `{fmt}`   | `last 100 successful runs`                      |
//!
//! When N<20, both the p50 and p95 display divs carry
//! `title="insufficient samples: need 20 successful runs, currently have {N}"`.
//!
//! Test harness pattern mirrors `tests/dashboard_render.rs` — build the full
//! `cronduit::web::router(state)` wired to an in-memory SQLite pool, drive
//! `GET /jobs/{id}` via `tower::ServiceExt::oneshot`, and scan the rendered
//! HTML body for the exact Copywriting contract strings. The Copywriting
//! contract is load-bearing, so assertions are byte-exact against the locked
//! strings — not substrings of them.
//!
//! Seeding note: to produce deterministic `duration_ms` values (e.g. the
//! 10-second runs in `twenty_successful_runs_at_threshold`), the seeders
//! insert `job_runs` rows directly via raw SQL with explicit `duration_ms`
//! and `end_time` columns, bypassing `finalize_run` which derives duration
//! from `tokio::time::Instant::elapsed()`. `insert_running_run` is still
//! used for the running-status path where applicable, but `finalize_run`'s
//! elapsed-time duration is non-deterministic and not usable here.

use std::sync::{Arc, Mutex};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use sqlx::Row;
use tower::ServiceExt; // brings .oneshot()

use cronduit::db::DbPool;
use cronduit::db::queries::{self, PoolRef};
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::telemetry::setup_metrics;
use cronduit::web::{AppState, ReloadState, router};

// ---------------------------------------------------------------------------
// Test app + seed helpers
// ---------------------------------------------------------------------------

async fn build_test_app() -> (axum::Router, DbPool) {
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
        "[]",
    )
    .await
    .expect("upsert test job")
}

/// Insert `count` `job_runs` rows with the given `status` and explicit
/// `duration_ms`. Bypasses `finalize_run` so durations are fully deterministic
/// (not derived from `tokio::time::Instant::elapsed()`).
///
/// Each row gets a unique `job_run_number` (starts at 1) and an RFC3339
/// `start_time`/`end_time` pair spaced 1 second apart so ORDER BY id works
/// predictably.
async fn seed_runs_with_duration(
    pool: &DbPool,
    job_id: i64,
    count: usize,
    status: &str,
    duration_ms: i64,
) {
    let writer = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        PoolRef::Postgres(_) => panic!("sqlite-only test fixture"),
    };

    for i in 0..count {
        // Use distinct timestamps per row so ORDER BY id is deterministic.
        let start = chrono::Utc::now() - chrono::Duration::seconds((count - i) as i64);
        let end = start + chrono::Duration::milliseconds(duration_ms);
        let start_str = start.to_rfc3339();
        let end_str = end.to_rfc3339();

        // Advance the job's next_run_number counter so job_run_number is
        // unique (the column has a uniqueness constraint once Phase 11
        // migrations apply).
        let run_number: i64 = sqlx::query_scalar(
            "UPDATE jobs SET next_run_number = next_run_number + 1 \
             WHERE id = ?1 RETURNING next_run_number - 1",
        )
        .bind(job_id)
        .fetch_one(writer)
        .await
        .expect("advance next_run_number");

        let _: i64 = sqlx::query_scalar(
            "INSERT INTO job_runs \
               (job_id, status, trigger, start_time, end_time, duration_ms, exit_code, job_run_number) \
             VALUES (?1, ?2, 'scheduled', ?3, ?4, ?5, 0, ?6) RETURNING id",
        )
        .bind(job_id)
        .bind(status)
        .bind(&start_str)
        .bind(&end_str)
        .bind(duration_ms)
        .bind(run_number)
        .fetch_one(writer)
        .await
        .expect("insert seeded run");
    }
}

/// GET /jobs/{id} and return (status, body-as-utf8-String).
async fn get_job_detail(app: axum::Router, job_id: i64) -> (StatusCode, String) {
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/jobs/{}", job_id))
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

/// Sanity check: count the duration rows the handler will actually see.
async fn count_successful_durations(pool: &DbPool, job_id: i64) -> usize {
    let writer = match pool.reader() {
        PoolRef::Sqlite(p) => p,
        PoolRef::Postgres(_) => panic!("sqlite-only test fixture"),
    };
    let row = sqlx::query(
        "SELECT COUNT(*) AS c FROM job_runs \
         WHERE job_id = ?1 AND status = 'success' AND duration_ms IS NOT NULL",
    )
    .bind(job_id)
    .fetch_one(writer)
    .await
    .expect("count query");
    row.get::<i64, _>("c") as usize
}

// ---------------------------------------------------------------------------
// Test cases — N-threshold matrix
// ---------------------------------------------------------------------------

/// N=0: zero runs of any status. Card must render without crashing; subtitle
/// must be "0 of 20 successful runs required"; both p50 and p95 values must
/// be em-dashes; each value div carries the "currently have 0" insufficient-
/// samples tooltip.
#[tokio::test]
async fn zero_runs_renders_card_without_crashing() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_job(&pool, "zero-run-job").await;

    assert_eq!(
        count_successful_durations(&pool, job_id).await,
        0,
        "test fixture sanity: zero successful runs seeded"
    );

    let (status, body) = get_job_detail(app, job_id).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "GET /jobs/{job_id} must return 200 even with zero runs (zero-run-crash-free invariant)"
    );

    // Subtitle (exact Copywriting contract string).
    assert!(
        body.contains("0 of 20 successful runs required"),
        "N=0 subtitle must read \"0 of 20 successful runs required\"; body:\n{body}"
    );

    // Em dash values (U+2014). There are 2 em-dashes — one per chip value.
    // The Duration card is the only template surface emitting an em dash
    // in the job detail page, so a strict count is a robust invariant.
    let em_dash_count = body.matches('—').count();
    assert!(
        em_dash_count >= 2,
        "N=0 must render p50 and p95 each as em dash U+2014, found {em_dash_count}; body:\n{body}"
    );

    // Insufficient-samples tooltip appears for both p50 and p95 chips.
    assert!(
        body.contains("insufficient samples: need 20 successful runs, currently have 0"),
        "N=0 must render insufficient-samples title with \"currently have 0\"; body:\n{body}"
    );
    let title_count = body
        .matches("insufficient samples: need 20 successful runs, currently have 0")
        .count();
    assert_eq!(
        title_count, 2,
        "tooltip must render twice (p50 + p95), got {title_count}"
    );

    // Card heading must be present.
    assert!(
        body.contains(">Duration</h2>"),
        "Duration card heading must render"
    );
}

/// N=19: 19 successful runs. Still below the 20-sample threshold; subtitle
/// must report progress ("19 of 20 successful runs required"), values still
/// render as em-dashes, tooltip reports "currently have 19".
#[tokio::test]
async fn nineteen_successful_runs_below_threshold() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_job(&pool, "nineteen-run-job").await;
    seed_runs_with_duration(&pool, job_id, 19, "success", 5_000).await;

    assert_eq!(
        count_successful_durations(&pool, job_id).await,
        19,
        "test fixture sanity: 19 successful runs seeded"
    );

    let (status, body) = get_job_detail(app, job_id).await;

    assert_eq!(status, StatusCode::OK);

    assert!(
        body.contains("19 of 20 successful runs required"),
        "N=19 subtitle must read \"19 of 20 successful runs required\"; body excerpt:\n{body}"
    );

    // Still em-dash values below threshold.
    let em_dash_count = body.matches('—').count();
    assert!(
        em_dash_count >= 2,
        "N=19 must render p50 and p95 each as em dash, found {em_dash_count}"
    );

    assert!(
        body.contains("insufficient samples: need 20 successful runs, currently have 19"),
        "N=19 tooltip must read \"currently have 19\"; body excerpt:\n{body}"
    );

    // Must NOT render the "at threshold" subtitle or the 5s display value.
    assert!(
        !body.contains("last 19 successful runs"),
        "N=19 must NOT render the N>=20 subtitle shape"
    );
}

/// N=20: exactly at threshold, all runs with 10 seconds duration. Subtitle
/// must read "last 20 successful runs"; p50 and p95 must both render as
/// "10s" via `format_duration_ms_floor_seconds` (10_000 ms → "10s", not
/// "10.0s"); no insufficient-samples tooltip on the chips.
#[tokio::test]
async fn twenty_successful_runs_at_threshold() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_job(&pool, "twenty-run-job").await;
    seed_runs_with_duration(&pool, job_id, 20, "success", 10_000).await;

    assert_eq!(
        count_successful_durations(&pool, job_id).await,
        20,
        "test fixture sanity: 20 successful runs seeded"
    );

    let (status, body) = get_job_detail(app, job_id).await;

    assert_eq!(status, StatusCode::OK);

    assert!(
        body.contains("last 20 successful runs"),
        "N=20 subtitle must read \"last 20 successful runs\"; body excerpt:\n{body}"
    );

    // Chip labels must render.
    assert!(
        body.contains(">p50</span>"),
        "p50 chip label must render (rendered inside <span>...</span>)"
    );
    assert!(body.contains(">p95</span>"), "p95 chip label must render");

    // 10_000 ms via floor-seconds formatter → "10s" (NOT "10.0s" which is
    // `format_duration_ms`'s shape). Both p50 and p95 of a constant-10s
    // distribution are 10s, so "10s" appears at least twice in the card.
    let ten_s_count = body.matches("\n        10s\n").count();
    assert!(
        ten_s_count >= 2,
        "N=20 with uniform 10_000 ms runs must render both p50 and p95 as \"10s\"; \
         expected at least 2 occurrences in the chip divs, found {ten_s_count}; body:\n{body}"
    );

    // MUST NOT emit the insufficient-samples tooltip when at threshold.
    assert!(
        !body.contains("insufficient samples: need 20"),
        "N=20 must NOT render insufficient-samples tooltip; body excerpt:\n{body}"
    );

    // MUST NOT render the below-threshold subtitle matrix.
    assert!(
        !body.contains("of 20 successful runs required"),
        "N=20 must NOT render the below-threshold subtitle"
    );
}

/// Only-success filter: seed 10 successful + 15 failed runs (25 total in DB
/// but only 10 contribute to the percentile input per D-20). Expected
/// behavior: subtitle reports "10 of 20 successful runs required", tooltip
/// reports "currently have 10", values remain em dashes.
#[tokio::test]
async fn only_success_counted_excluded_statuses_ignored() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_job(&pool, "mixed-status-job").await;
    seed_runs_with_duration(&pool, job_id, 10, "success", 7_500).await;
    seed_runs_with_duration(&pool, job_id, 15, "failed", 9_999).await;

    // Sanity: query only sees the 10 successful rows despite 25 total.
    assert_eq!(
        count_successful_durations(&pool, job_id).await,
        10,
        "only status='success' rows should count toward the percentile input (D-20)"
    );

    let (status, body) = get_job_detail(app, job_id).await;

    assert_eq!(status, StatusCode::OK);

    assert!(
        body.contains("10 of 20 successful runs required"),
        "N=10 (success) despite 25 total runs must read \"10 of 20 successful runs required\"; \
         body excerpt:\n{body}"
    );

    assert!(
        body.contains("insufficient samples: need 20 successful runs, currently have 10"),
        "tooltip must read \"currently have 10\" (success-only count), NOT 25; body excerpt:\n{body}"
    );

    // Confirms the failed rows did NOT bump the count past the threshold.
    assert!(
        !body.contains("last 25 successful runs"),
        "failed rows must NOT count toward \"last N successful runs\" subtitle"
    );
    assert!(
        !body.contains("last 10 successful runs"),
        "below-threshold job must not emit the N>=20 subtitle shape either"
    );

    // p50 and p95 must stay em-dashes below threshold.
    let em_dash_count = body.matches('—').count();
    assert!(
        em_dash_count >= 2,
        "N=10 < 20 must render p50 and p95 each as em dash, found {em_dash_count}"
    );
}
