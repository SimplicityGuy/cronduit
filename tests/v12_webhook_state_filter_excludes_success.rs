//! v12_webhook_state_filter_excludes_success.rs (Phase 18)
//!
//! End-to-end state-filter exclusion test. With `states = ["failed"]`,
//! emitting a `RunFinalized` whose status is `"success"` MUST NOT trigger
//! a webhook delivery. Locks the full pipeline behavior: dispatcher reads
//! `cfg.states`, sees `"success"` is not in the configured set, and
//! skips the delivery without any HTTP request.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use sqlx::Row;
use wiremock::matchers::method;
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

async fn seed_job_with_success_run(pool: &DbPool) -> (i64, i64) {
    let now = chrono::Utc::now().to_rfc3339();
    let p = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let job_row = sqlx::query(
        "INSERT INTO jobs (name, schedule, resolved_schedule, job_type, config_json, config_hash, timeout_secs, created_at, updated_at) \
         VALUES ('state-filter-test-job', '* * * * *', '* * * * *', 'command', '{}', 'seed-cfg', 60, ?1, ?1) RETURNING id",
    )
    .bind(&now)
    .fetch_one(p)
    .await
    .expect("seed job");
    let job_id: i64 = job_row.get("id");

    let start_time = "2026-04-27T00:01:00Z";
    let run_row = sqlx::query(
        "INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number, image_digest, config_hash) \
         VALUES (?1, 'success', 'manual', ?2, 1, NULL, 'seed-cfg') RETURNING id",
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
async fn success_run_with_failed_filter_does_not_fire() {
    // Permissive mock: the matcher accepts any POST. Since the dispatcher
    // SHOULD NOT issue any request, the response template here is
    // irrelevant — the assertion is "received_requests().is_empty()".
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let pool = setup_test_db().await;
    let (job_id, run_id) = seed_job_with_success_run(&pool).await;
    let url = server.uri();

    // states = ["failed"] — "success" is NOT in the filter set.
    let cfg = WebhookConfig {
        url,
        states: vec!["failed".into()],
        secret: Some(secrecy::SecretString::from("k")),
        unsigned: false,
        fire_every: 0,
    };
    let mut webhooks = HashMap::new();
    webhooks.insert(job_id, cfg);
    let dispatcher = HttpDispatcher::new(pool, Arc::new(webhooks)).unwrap();

    // Emit a SUCCESS event — must be filtered out.
    let event = RunFinalized {
        run_id,
        job_id,
        job_name: "state-filter-test-job".to_string(),
        status: "success".to_string(),
        exit_code: Some(0),
        started_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 0).unwrap(),
        finished_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 1).unwrap(),
    };
    dispatcher.deliver(&event).await.unwrap();

    // Assertion: ZERO requests reached the receiver.
    let received = server.received_requests().await.unwrap();
    assert!(
        received.is_empty(),
        "success run with states=[\"failed\"] must NOT fire (state filter exclusion); got {} requests",
        received.len()
    );
}
