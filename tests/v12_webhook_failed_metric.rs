//! v12_webhook_failed_metric.rs (Phase 18 / D-17)
//!
//! Asserts that a non-2xx HTTP response from the receiver increments
//! `cronduit_webhook_delivery_failed_total` by 1 and leaves
//! `cronduit_webhook_delivery_sent_total` unchanged. Delta-asserted
//! (final - baseline) — same idiom as tests/v12_webhook_queue_drop.rs.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use secrecy::SecretString;
use sqlx::Row;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use cronduit::config::WebhookConfig;
use cronduit::db::DbPool;
use cronduit::db::queries::PoolRef;
use cronduit::telemetry::setup_metrics;
use cronduit::webhooks::{HttpDispatcher, RunFinalized, WebhookDispatcher};

fn read_counter(body: &str, name: &str) -> f64 {
    let prefix_unlabeled = format!("{name} ");
    let prefix_labeled = format!("{name}{{");
    body.lines()
        .find(|l| l.starts_with(&prefix_unlabeled) || l.starts_with(&prefix_labeled))
        .and_then(|l| l.rsplit_once(' ').and_then(|(_, n)| n.trim().parse().ok()))
        .unwrap_or(0.0)
}

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
         VALUES ('failed-metric-job', '* * * * *', '* * * * *', 'command', '{}', 'seed-cfg', 60, ?1, ?1) RETURNING id",
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
async fn webhook_failed_metric_increments_failed_total_on_non_2xx() {
    let handle = setup_metrics();
    let baseline_sent = read_counter(&handle.render(), "cronduit_webhook_delivery_sent_total");
    let baseline_failed = read_counter(&handle.render(), "cronduit_webhook_delivery_failed_total");

    // 5xx receiver.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let url = server.uri();

    let pool = setup_test_db().await;
    let (job_id, run_id) = seed_job_with_failed_run(&pool).await;

    let cfg = WebhookConfig {
        url,
        states: vec!["failed".into()],
        secret: Some(SecretString::from("k")),
        unsigned: false,
        fire_every: 0,
    };
    let mut webhooks = HashMap::new();
    webhooks.insert(job_id, cfg);
    let dispatcher = HttpDispatcher::new(pool, Arc::new(webhooks)).unwrap();

    let event = RunFinalized {
        run_id,
        job_id,
        job_name: "failed-metric-job".to_string(),
        status: "failed".to_string(),
        exit_code: Some(1),
        started_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 0).unwrap(),
        finished_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 1).unwrap(),
    };
    // Phase 20 / WH-05: HttpDispatcher now returns Err on non-2xx so the
    // RetryingDispatcher can decide whether to retry. The failed-metric
    // increment still fires inside HttpDispatcher::deliver. Accept either Ok
    // or Err here — what matters for this test is the counter delta.
    let _ = dispatcher.deliver(&event).await;

    let final_sent = read_counter(&handle.render(), "cronduit_webhook_delivery_sent_total");
    let final_failed = read_counter(&handle.render(), "cronduit_webhook_delivery_failed_total");

    assert_eq!(
        final_failed - baseline_failed,
        1.0,
        "non-2xx response must increment cronduit_webhook_delivery_failed_total by 1; \
         baseline={baseline_failed}, final={final_failed}"
    );
    assert_eq!(
        final_sent - baseline_sent,
        0.0,
        "non-2xx response must NOT increment cronduit_webhook_delivery_sent_total; \
         baseline={baseline_sent}, final={final_sent}"
    );
}
