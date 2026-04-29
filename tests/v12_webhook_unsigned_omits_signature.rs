//! v12_webhook_unsigned_omits_signature.rs (Phase 18 / D-05)
//!
//! Asserts that `WebhookConfig.unsigned = true` causes the dispatcher to
//! emit the `webhook-id` and `webhook-timestamp` headers but OMIT the
//! `webhook-signature` header entirely. This is cronduit's extension to
//! Standard Webhooks v1 for receivers like Slack/Discord that don't
//! HMAC-verify (D-05).

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use sqlx::Row;
use wiremock::matchers::{header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use cronduit::config::WebhookConfig;
use cronduit::db::DbPool;
use cronduit::db::queries::PoolRef;
use cronduit::webhooks::{HttpDispatcher, RunFinalized, WebhookDispatcher};

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
         VALUES ('unsigned-test-job', '* * * * *', '* * * * *', 'command', '{}', 'seed-cfg', 60, ?1, ?1) RETURNING id",
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

#[tokio::test]
async fn unsigned_webhook_omits_signature_header() {
    // Mock receiver — only requires webhook-id + webhook-timestamp
    // to match. (We do NOT add `header_exists("webhook-signature")` —
    // that header is what we are asserting absent below.)
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/hook"))
        .and(header_exists("webhook-id"))
        .and(header_exists("webhook-timestamp"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let pool = setup_test_db().await;
    let (job_id, run_id) = seed_job_with_failed_run(&pool).await;
    let url = format!("{}/hook", server.uri());

    // D-05: unsigned = true with secret = None.
    let cfg = WebhookConfig {
        url,
        states: vec!["failed".into()],
        secret: None,
        unsigned: true,
        fire_every: 0,
    };
    let mut webhooks = HashMap::new();
    webhooks.insert(job_id, cfg);
    let dispatcher = HttpDispatcher::new(pool, Arc::new(webhooks)).unwrap();

    let event = RunFinalized {
        run_id,
        job_id,
        job_name: "unsigned-test-job".to_string(),
        status: "failed".to_string(),
        exit_code: Some(1),
        started_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 0).unwrap(),
        finished_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 1).unwrap(),
    };
    dispatcher.deliver(&event).await.unwrap();

    let received = server.received_requests().await.unwrap();
    assert_eq!(received.len(), 1, "expected exactly 1 delivery");
    let req = &received[0];

    // D-05: signature header MUST be absent when unsigned = true.
    assert!(
        req.headers.get("webhook-signature").is_none(),
        "webhook-signature MUST be omitted when cfg.unsigned == true (D-05)"
    );

    // But webhook-id and webhook-timestamp MUST still be present.
    assert!(
        req.headers.get("webhook-id").is_some(),
        "webhook-id MUST be emitted even when unsigned == true"
    );
    assert!(
        req.headers.get("webhook-timestamp").is_some(),
        "webhook-timestamp MUST be emitted even when unsigned == true"
    );
}
