//! Phase 20 / WH-05: 3-attempt retry chain integration tests.
//!
//! Asserts the locked retry posture from CONTEXT D-01..D-04:
//!   - Schedule [0s, 30s, 300s] — exactly 3 attempts on transient failure.
//!   - 5xx-exhausted path writes ONE DLQ row with attempts=3, dlq_reason='http_5xx'.
//!
//! Uses `tokio::time::pause()` + `tokio::time::advance(...)` for deterministic
//! sleep — wall-clock-free; tests complete in milliseconds.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{TimeZone, Utc};
use secrecy::SecretString;
use sqlx::Row;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use cronduit::config::WebhookConfig;
use cronduit::db::DbPool;
use cronduit::db::queries::PoolRef;
use cronduit::webhooks::{HttpDispatcher, RetryingDispatcher, RunFinalized, WebhookDispatcher};

async fn setup_test_db() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    pool.migrate().await.expect("apply migrations");
    pool
}

async fn seed_job_with_failed_run(pool: &DbPool) -> (i64, i64) {
    let now = chrono::Utc::now().to_rfc3339();
    let p = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let job_row = sqlx::query(
        "INSERT INTO jobs (name, schedule, resolved_schedule, job_type, config_json, config_hash, timeout_secs, created_at, updated_at) \
         VALUES ('retry-test-job', '* * * * *', '* * * * *', 'command', '{}', 'seed-cfg', 60, ?1, ?1) RETURNING id",
    )
    .bind(&now)
    .fetch_one(p)
    .await
    .expect("seed job");
    let job_id: i64 = job_row.get("id");

    let start_time = "2026-04-27T00:01:00Z";
    let run_row = sqlx::query(
        "INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number, image_digest, config_hash) \
         VALUES (?1, 'failed', 'manual', ?2, 1, NULL, 'seed-cfg') RETURNING id",
    )
    .bind(job_id)
    .bind(start_time)
    .fetch_one(p)
    .await
    .expect("seed run");
    let run_id: i64 = run_row.get("id");

    (job_id, run_id)
}

fn make_run_finalized(run_id: i64, job_id: i64, name: &str, status: &str) -> RunFinalized {
    RunFinalized {
        run_id,
        job_id,
        job_name: name.to_string(),
        status: status.to_string(),
        exit_code: Some(1),
        started_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 0).unwrap(),
        finished_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 1).unwrap(),
    }
}

/// Build a RetryingDispatcher pointed at the wiremock URL with `fire_every=0`
/// (always fire) and `states=["failed"]`. Returns the dispatcher and the
/// shared webhooks Arc (so the test can verify the same Arc backs the DLQ url
/// lookup and the HttpDispatcher's per-job lookup).
async fn build_retrying_dispatcher_for_test(
    pool: DbPool,
    wiremock_uri: &str,
    job_id: i64,
    cancel: CancellationToken,
) -> RetryingDispatcher<HttpDispatcher> {
    let cfg = WebhookConfig {
        url: wiremock_uri.to_string(),
        states: vec!["failed".into()],
        secret: Some(SecretString::from("k")),
        unsigned: false,
        fire_every: 0,
    };
    let mut webhooks_map = HashMap::new();
    webhooks_map.insert(job_id, cfg);
    let webhooks = Arc::new(webhooks_map);
    let http = HttpDispatcher::new(pool.clone(), webhooks.clone()).unwrap();
    RetryingDispatcher::new(http, pool, cancel, webhooks)
}

#[tokio::test(flavor = "current_thread")]
async fn three_attempts_at_locked_schedule_under_paused_clock() {
    // CONTEXT D-01..D-04: 5xx every time → 3 attempts at t=0, t=30s, t=300s,
    // then ONE DLQ row written with attempts=3, dlq_reason='http_5xx'.
    //
    // Setup uses real time (DbPool::connect, MockServer::start), then we
    // pause the clock just before dispatch so the schedule's 30s+300s sleeps
    // collapse to virtual-time advances. This avoids the SQLite acquire-timeout
    // hang seen with `start_paused = true`.
    let pool = setup_test_db().await;
    let (job_id, run_id) = seed_job_with_failed_run(&pool).await;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let server_uri = server.uri();

    let cancel = CancellationToken::new();
    let dispatcher =
        build_retrying_dispatcher_for_test(pool.clone(), &server_uri, job_id, cancel.clone()).await;

    let event = make_run_finalized(run_id, job_id, "retry-test-job", "failed");

    // Drive the dispatcher concurrently with a clock-driver that bumps
    // virtual time forward each yield. The driver-side advances skip the
    // schedule's 30s + 300s sleeps; the dispatcher-side runs HTTP requests
    // in real time against wiremock (microseconds in-process). The select
    // returns when the dispatcher completes; the driver is dropped/cancelled.
    tokio::time::pause();

    let result = tokio::select! {
        r = dispatcher.deliver(&event) => r,
        _ = async {
            loop {
                tokio::task::yield_now().await;
                tokio::time::advance(Duration::from_millis(500)).await;
            }
        } => unreachable!(),
    };

    assert!(
        result.is_err(),
        "3-attempt 5xx exhaustion should return Err"
    );

    let requests = server.received_requests().await.expect("wiremock requests");
    assert_eq!(
        requests.len(),
        3,
        "expected 3 attempts (D-01 schedule), got {}",
        requests.len()
    );

    // Inspect the DLQ row. Resume real time so SQL queries don't hang.
    tokio::time::resume();
    let read_pool = match pool.reader() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let row = sqlx::query(
        "SELECT attempts, dlq_reason, run_id FROM webhook_deliveries WHERE run_id = ?1",
    )
    .bind(run_id)
    .fetch_one(read_pool)
    .await
    .expect("DLQ row present after exhausted 5xx chain");
    let attempts: i64 = row.get("attempts");
    let reason: String = row.get("dlq_reason");
    assert_eq!(attempts, 3, "DLQ row attempts must equal schedule length");
    assert_eq!(reason, "http_5xx", "5xx exhaustion → dlq_reason='http_5xx'");
}
