//! v12_webhook_failed_metric.rs (Phase 18 / D-17 + Phase 20 / WH-11 / D-22)
//!
//! Phase 20 BREAKING CHANGE: the unlabeled P18 failure-counter (the
//! delivery-failed flat counter) is REPLACED by
//! `cronduit_webhook_deliveries_total{job, status="failed"}`
//! which fires at the OUTER `RetryingDispatcher::deliver` chain-terminal-failure
//! boundary (after retry exhaustion or 4xx-permanent or shutdown_drain).
//!
//! The receiver returns 4xx-permanent (404) so the chain short-circuits after
//! attempt 1 — keeps the test fast without paused-clock plumbing. The success
//! row of the labeled family must NOT increment.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use secrecy::SecretString;
use sqlx::Row;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use cronduit::config::WebhookConfig;
use cronduit::db::DbPool;
use cronduit::db::queries::PoolRef;
use cronduit::telemetry::setup_metrics;
use cronduit::webhooks::{HttpDispatcher, RetryingDispatcher, RunFinalized, WebhookDispatcher};

/// Sum values for rows matching `name` whose label string contains
/// `status="<status>"` regardless of the `job` label. Returns 0.0 if no rows
/// match. Phase 20 / WH-11 / D-22 helper.
fn sum_status(rendered: &str, name: &str, status: &str) -> f64 {
    let prefix = format!("{name}{{");
    let needle = format!("status=\"{status}\"");
    let mut total = 0.0;
    for line in rendered.lines() {
        let Some(rest) = line.strip_prefix(&prefix) else {
            continue;
        };
        let Some(end) = rest.find('}') else {
            continue;
        };
        let labels = &rest[..end];
        if !labels.contains(&needle) {
            continue;
        }
        let after = &rest[end + 1..];
        total += after.trim().parse::<f64>().unwrap_or(0.0);
    }
    total
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
async fn webhook_failed_metric_increments_deliveries_status_failed() {
    let handle = setup_metrics();
    let baseline_success = sum_status(
        &handle.render(),
        "cronduit_webhook_deliveries_total",
        "success",
    );
    let baseline_failed = sum_status(
        &handle.render(),
        "cronduit_webhook_deliveries_total",
        "failed",
    );

    // 4xx-permanent receiver — the retry classifier short-circuits after
    // attempt 1 (D-06: 4xx-other → Permanent), so the chain runs in
    // milliseconds without needing paused-clock plumbing.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(404))
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
    let mut webhooks_map = HashMap::new();
    webhooks_map.insert(job_id, cfg);
    let webhooks = Arc::new(webhooks_map);
    let http = HttpDispatcher::new(pool.clone(), webhooks.clone()).unwrap();
    // Phase 20 / WH-11: the labeled per-DELIVERY counter increments at the
    // OUTER RetryingDispatcher::deliver chain-terminal-failure boundary
    // (after DLQ row write), NOT inside HttpDispatcher::deliver.
    let cancel = CancellationToken::new();
    let dispatcher = RetryingDispatcher::new(http, pool, cancel, webhooks);

    let event = RunFinalized {
        run_id,
        job_id,
        job_name: "failed-metric-job".to_string(),
        status: "failed".to_string(),
        exit_code: Some(1),
        started_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 0).unwrap(),
        finished_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 1).unwrap(),
    };
    // Terminal failure expected — chain returns Err after writing one DLQ row.
    let _ = dispatcher.deliver(&event).await;

    let final_success = sum_status(
        &handle.render(),
        "cronduit_webhook_deliveries_total",
        "success",
    );
    let final_failed = sum_status(
        &handle.render(),
        "cronduit_webhook_deliveries_total",
        "failed",
    );

    assert_eq!(
        final_failed - baseline_failed,
        1.0,
        "4xx-permanent response must increment \
         cronduit_webhook_deliveries_total{{status=\"failed\"}} by 1 at the \
         chain-terminal boundary; baseline={baseline_failed}, final={final_failed}"
    );
    assert_eq!(
        final_success - baseline_success,
        0.0,
        "4xx-permanent response must NOT increment \
         cronduit_webhook_deliveries_total{{status=\"success\"}}; \
         baseline={baseline_success}, final={final_success}"
    );
}
