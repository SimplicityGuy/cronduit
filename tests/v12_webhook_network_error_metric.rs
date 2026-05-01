//! v12_webhook_network_error_metric.rs (Phase 18 / D-17 + Phase 20 / WH-11 / D-22)
//!
//! Phase 20 BREAKING CHANGE: the unlabeled P18 failure-counter (the
//! delivery-failed flat counter) is REPLACED by
//! `cronduit_webhook_deliveries_total{job, status="failed"}`
//! which fires at the OUTER `RetryingDispatcher::deliver` chain-terminal-failure
//! boundary. Network errors classify as TRANSIENT (D-06: reqwest network →
//! Transient(Network)) so the chain runs all 3 attempts before terminal
//! failure — paused-clock + driver loop pattern (same as v12_webhook_retry.rs)
//! keeps the test fast despite the 30s + 300s schedule sleeps.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{TimeZone, Utc};
use secrecy::SecretString;
use sqlx::Row;
use tokio_util::sync::CancellationToken;

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
         VALUES ('netfail-metric-job', '* * * * *', '* * * * *', 'command', '{}', 'seed-cfg', 60, ?1, ?1) RETURNING id",
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

#[tokio::test(flavor = "current_thread")]
async fn webhook_network_error_increments_deliveries_status_failed() {
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

    // Point at a guaranteed-unbound TCP address. Port 1 (tcpmux) is a
    // privileged/system port — connecting to it from userspace does NOT
    // require root, but binding does, so port 1 is reliably closed on
    // every dev machine and CI runner. The kernel returns ECONNREFUSED
    // immediately, which reqwest classifies as a network error and the
    // RetryingDispatcher classifies as Transient(Network) → retries 3
    // times then writes the DLQ row + terminal-failure counter increment.
    let url = "http://127.0.0.1:1".to_string();

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
    let cancel = CancellationToken::new();
    let dispatcher = RetryingDispatcher::new(http, pool, cancel, webhooks);

    let event = RunFinalized {
        run_id,
        job_id,
        job_name: "netfail-metric-job".to_string(),
        status: "failed".to_string(),
        exit_code: Some(1),
        started_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 0).unwrap(),
        finished_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 1).unwrap(),
    };

    // Network errors are Transient(Network) → 3 attempts at t=0/30s/300s.
    // Use the same paused-clock + driver-loop pattern as
    // tests/v12_webhook_retry.rs::three_attempts_at_locked_schedule_under_paused_clock
    // so the schedule's 30s + 300s sleeps collapse to virtual-time advances.
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
        "3-attempt network-error exhaustion must return Err"
    );
    // Resume real time so the metrics render path doesn't block on virtual time.
    tokio::time::resume();

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
        "network error must increment \
         cronduit_webhook_deliveries_total{{status=\"failed\"}} by 1 at the \
         chain-terminal boundary; baseline={baseline_failed}, final={final_failed}"
    );
    assert_eq!(
        final_success - baseline_success,
        0.0,
        "network error must NOT increment \
         cronduit_webhook_deliveries_total{{status=\"success\"}}; \
         baseline={baseline_success}, final={final_success}"
    );
}
