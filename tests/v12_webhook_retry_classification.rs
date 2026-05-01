//! Phase 20 / WH-05 / D-06: Classification table integration coverage.
//!
//! Verifies the classify() decision table flows correctly through
//! RetryingDispatcher::deliver:
//!   - 4xx-other → permanent: 1 attempt only, 1 DLQ row, dlq_reason='http_4xx'.
//!   - 408 → transient (treated like 5xx for retry): 3 attempts, dlq_reason='http_5xx'.
//!
//! 5xx + 429 + Network + Timeout transient classification is covered by
//! tests/v12_webhook_retry.rs (chain shape) and the in-module unit tests
//! in src/webhooks/retry.rs (classify_response_table).

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

async fn seed_job_with_failed_run(pool: &DbPool, name: &str) -> (i64, i64) {
    let now = chrono::Utc::now().to_rfc3339();
    let p = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let job_row = sqlx::query(
        "INSERT INTO jobs (name, schedule, resolved_schedule, job_type, config_json, config_hash, timeout_secs, created_at, updated_at) \
         VALUES (?1, '* * * * *', '* * * * *', 'command', '{}', 'seed-cfg', 60, ?2, ?2) RETURNING id",
    )
    .bind(name)
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

fn make_run_finalized(run_id: i64, job_id: i64, name: &str) -> RunFinalized {
    RunFinalized {
        run_id,
        job_id,
        job_name: name.to_string(),
        status: "failed".to_string(),
        exit_code: Some(1),
        started_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 0).unwrap(),
        finished_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 1).unwrap(),
    }
}

async fn build_dispatcher(
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
async fn four_oh_four_writes_one_dlq_row_no_retry() {
    // D-06: 404 → Permanent(Http4xx). Chain stops at attempt 1; ONE DLQ row
    // with attempts=1, dlq_reason='http_4xx'. wiremock should see exactly 1 request.
    let pool = setup_test_db().await;
    let (job_id, run_id) = seed_job_with_failed_run(&pool, "404-job").await;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let server_uri = server.uri();

    let cancel = CancellationToken::new();
    let dispatcher = build_dispatcher(pool.clone(), &server_uri, job_id, cancel.clone()).await;
    let event = make_run_finalized(run_id, job_id, "404-job");

    // No clock-pause needed: 4xx-permanent short-circuits before any sleep.
    let result = dispatcher.deliver(&event).await;
    assert!(result.is_err(), "4xx-permanent must return Err");

    let requests = server.received_requests().await.expect("wiremock requests");
    assert_eq!(
        requests.len(),
        1,
        "4xx-permanent must NOT retry; expected 1 request, got {}",
        requests.len()
    );

    let read_pool = match pool.reader() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let row = sqlx::query("SELECT attempts, dlq_reason FROM webhook_deliveries WHERE run_id = ?1")
        .bind(run_id)
        .fetch_one(read_pool)
        .await
        .expect("DLQ row present after 4xx");
    let attempts: i64 = row.get("attempts");
    let reason: String = row.get("dlq_reason");
    assert_eq!(attempts, 1, "4xx writes attempts=1 (no retry)");
    assert_eq!(reason, "http_4xx", "404 must classify as http_4xx");
}

#[tokio::test(flavor = "current_thread")]
async fn four_oh_eight_retries_per_schedule() {
    // D-06: 408 → Transient (treated like 5xx for retry). Chain runs all 3
    // attempts; DLQ row has attempts=3, dlq_reason='http_5xx'.
    let pool = setup_test_db().await;
    let (job_id, run_id) = seed_job_with_failed_run(&pool, "408-job").await;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(408))
        .mount(&server)
        .await;
    let server_uri = server.uri();

    let cancel = CancellationToken::new();
    let dispatcher = build_dispatcher(pool.clone(), &server_uri, job_id, cancel.clone()).await;
    let event = make_run_finalized(run_id, job_id, "408-job");

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
    assert!(result.is_err(), "408-exhausted must return Err");

    let requests = server.received_requests().await.expect("wiremock requests");
    assert_eq!(
        requests.len(),
        3,
        "408 must retry per schedule; expected 3 attempts, got {}",
        requests.len()
    );

    tokio::time::resume();
    let read_pool = match pool.reader() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let row = sqlx::query("SELECT attempts, dlq_reason FROM webhook_deliveries WHERE run_id = ?1")
        .bind(run_id)
        .fetch_one(read_pool)
        .await
        .expect("DLQ row present after 408 exhaustion");
    let attempts: i64 = row.get("attempts");
    let reason: String = row.get("dlq_reason");
    assert_eq!(attempts, 3, "408 retries fully; attempts=3");
    assert_eq!(
        reason, "http_5xx",
        "408 classifies as transient (Http5xx) per D-06"
    );
}
