//! Phase 20 / WH-05 / D-10..D-12: DLQ row write-path integration tests.
//!
//! Coverage:
//!   - dlq_row_no_payload_no_signature_columns — schema hygiene (D-12).
//!   - dlq_reasons_table_coverage — http_4xx + http_5xx + network paths.
//!   - dlq_url_matches_configured_url — B2 regression lock (D-10/D-12).

use std::collections::{HashMap, HashSet};
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

#[tokio::test]
async fn dlq_row_no_payload_no_signature_columns() {
    // D-12 schema hygiene: webhook_deliveries MUST NOT have any column that
    // could leak request bodies, headers, or HMAC material. Audit-table-only.
    let pool = setup_test_db().await;
    let read_pool = match pool.reader() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };

    let cols: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('webhook_deliveries')")
            .fetch_all(read_pool)
            .await
            .expect("PRAGMA table_info(webhook_deliveries)");
    let names: HashSet<String> = cols.into_iter().map(|(n,)| n).collect();

    for forbidden in ["payload", "body", "headers", "signature", "secret", "hmac"] {
        assert!(
            !names.contains(forbidden),
            "column `{forbidden}` must NOT exist in webhook_deliveries (D-12 hygiene); \
             present columns: {names:?}"
        );
    }

    for required in [
        "id",
        "run_id",
        "job_id",
        "url",
        "attempts",
        "last_status",
        "last_error",
        "dlq_reason",
        "first_attempt_at",
        "last_attempt_at",
    ] {
        assert!(
            names.contains(required),
            "required column `{required}` missing from webhook_deliveries; \
             present columns: {names:?}"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn dlq_reasons_table_coverage() {
    // Run three scenarios sequentially with fresh seeds + fresh wiremock
    // mounts; assert each writes a DLQ row with the expected dlq_reason.
    let pool = setup_test_db().await;

    // Scenario 1: 404 → http_4xx (1 attempt)
    {
        let (job_id, run_id) = seed_job_with_failed_run(&pool, "dlq-404-job").await;
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let cancel = CancellationToken::new();
        let dispatcher =
            build_dispatcher(pool.clone(), &server.uri(), job_id, cancel.clone()).await;
        let event = make_run_finalized(run_id, job_id, "dlq-404-job");
        let _ = dispatcher.deliver(&event).await;

        let read_pool = match pool.reader() {
            PoolRef::Sqlite(p) => p,
            _ => panic!("sqlite-only test"),
        };
        let reason: String =
            sqlx::query_scalar("SELECT dlq_reason FROM webhook_deliveries WHERE run_id = ?1")
                .bind(run_id)
                .fetch_one(read_pool)
                .await
                .expect("DLQ row for 404");
        assert_eq!(reason, "http_4xx");
    }

    // Scenario 2: 500 exhausted → http_5xx (3 attempts; needs paused clock)
    {
        let (job_id, run_id) = seed_job_with_failed_run(&pool, "dlq-500-job").await;
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let server_uri = server.uri();
        let cancel = CancellationToken::new();
        let dispatcher = build_dispatcher(pool.clone(), &server_uri, job_id, cancel.clone()).await;
        let event = make_run_finalized(run_id, job_id, "dlq-500-job");

        tokio::time::pause();
        let _ = tokio::select! {
            r = dispatcher.deliver(&event) => r,
            _ = async {
                loop {
                    tokio::task::yield_now().await;
                    tokio::time::advance(Duration::from_millis(500)).await;
                }
            } => unreachable!(),
        };
        tokio::time::resume();

        let read_pool = match pool.reader() {
            PoolRef::Sqlite(p) => p,
            _ => panic!("sqlite-only test"),
        };
        let reason: String =
            sqlx::query_scalar("SELECT dlq_reason FROM webhook_deliveries WHERE run_id = ?1")
                .bind(run_id)
                .fetch_one(read_pool)
                .await
                .expect("DLQ row for 500");
        assert_eq!(reason, "http_5xx");
    }

    // Scenario 3: connection refused (unused port) → network (3 attempts)
    {
        let (job_id, run_id) = seed_job_with_failed_run(&pool, "dlq-net-job").await;
        // Use a deliberately-unused localhost port for connection refused.
        // Port 1 is privileged and unbound on test machines.
        let unused_url = "http://127.0.0.1:1".to_string();
        let cancel = CancellationToken::new();
        let dispatcher = build_dispatcher(pool.clone(), &unused_url, job_id, cancel.clone()).await;
        let event = make_run_finalized(run_id, job_id, "dlq-net-job");

        tokio::time::pause();
        let _ = tokio::select! {
            r = dispatcher.deliver(&event) => r,
            _ = async {
                loop {
                    tokio::task::yield_now().await;
                    tokio::time::advance(Duration::from_millis(500)).await;
                }
            } => unreachable!(),
        };
        tokio::time::resume();

        let read_pool = match pool.reader() {
            PoolRef::Sqlite(p) => p,
            _ => panic!("sqlite-only test"),
        };
        let reason: String =
            sqlx::query_scalar("SELECT dlq_reason FROM webhook_deliveries WHERE run_id = ?1")
                .bind(run_id)
                .fetch_one(read_pool)
                .await
                .expect("DLQ row for network failure");
        assert_eq!(reason, "network", "connect-refused → dlq_reason='network'");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn dlq_url_matches_configured_url() {
    // B2 regression lock: DLQ row's `url` column MUST equal the configured
    // webhook URL for that job_id. The fix wires Arc<HashMap<i64, WebhookConfig>>
    // through RetryingDispatcher::new so write_dlq can look up url at write time.
    let pool = setup_test_db().await;
    let (job_id, run_id) = seed_job_with_failed_run(&pool, "dlq-url-test-job").await;

    let server = MockServer::start().await;
    let configured_url = server.uri();
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let cancel = CancellationToken::new();
    let dispatcher = build_dispatcher(pool.clone(), &configured_url, job_id, cancel.clone()).await;
    let event = make_run_finalized(run_id, job_id, "dlq-url-test-job");

    tokio::time::pause();
    let _ = tokio::select! {
        r = dispatcher.deliver(&event) => r,
        _ = async {
            loop {
                tokio::task::yield_now().await;
                tokio::time::advance(Duration::from_millis(500)).await;
            }
        } => unreachable!(),
    };
    tokio::time::resume();

    let read_pool = match pool.reader() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let stored_url: String =
        sqlx::query_scalar("SELECT url FROM webhook_deliveries WHERE run_id = ?1")
            .bind(run_id)
            .fetch_one(read_pool)
            .await
            .expect("DLQ url column readable");

    assert!(
        !stored_url.is_empty(),
        "DLQ url column must NOT be empty (B2 regression)"
    );
    assert_eq!(
        stored_url, configured_url,
        "DLQ url column must equal the configured webhook URL; \
         got `{stored_url}` expected `{configured_url}`"
    );
}
