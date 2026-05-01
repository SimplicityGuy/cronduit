//! Phase 20 / WH-05 / D-07 + D-08: End-to-end Retry-After honoring + cap.
//!
//! Locks the B1 regression: receiver-supplied Retry-After hint MUST flow through
//! HttpDispatcher's WebhookError::HttpStatus { retry_after } variant into
//! RetryingDispatcher's compute_sleep_delay → tokio::time::sleep().
//!
//! Coverage:
//!   - receiver_429_with_retry_after_header_extends_sleep_to_hint_within_cap
//!     — B1 regression lock (end-to-end).
//!   - receiver_429_with_retry_after_9999_is_capped — DoS cap regression lock.
//!   - receiver_200_no_sleep — happy-path control.
//!   - cap_for_slot_matches_research_table — direct call to public helper.
//!   - parse_retry_after_integer_seconds_only — direct call to public helper.

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
use cronduit::webhooks::retry::{cap_for_slot, parse_retry_after_from_response};
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
async fn receiver_429_with_retry_after_header_extends_sleep_to_hint_within_cap() {
    // B1 regression lock: receiver returns 429 + Retry-After: 350.
    //
    // Schedule = [0s, 30s, 300s]. cap_for_slot(1) = schedule[2]*1.2 = 360s.
    // Retry-After: 350 > jitter(schedule[2]) max (300*1.2=360 — borderline but
    // 350 fits in jitter range). The truth this test asserts: with paused
    // clock, the sum of advances needed to drain the chain reflects the
    // honored Retry-After. We measure virtual elapsed time at chain exit.
    let pool = setup_test_db().await;
    let (job_id, run_id) = seed_job_with_failed_run(&pool, "retry-after-honor-job").await;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "350"))
        .mount(&server)
        .await;
    let server_uri = server.uri();

    let cancel = CancellationToken::new();
    let dispatcher = build_dispatcher(pool.clone(), &server_uri, job_id, cancel.clone()).await;
    let event = make_run_finalized(run_id, job_id, "retry-after-honor-job");

    tokio::time::pause();
    let start_virtual = tokio::time::Instant::now();

    let result = tokio::select! {
        r = dispatcher.deliver(&event) => r,
        _ = async {
            loop {
                tokio::task::yield_now().await;
                tokio::time::advance(Duration::from_millis(500)).await;
            }
        } => unreachable!(),
    };

    let elapsed_virtual = start_virtual.elapsed();
    assert!(result.is_err(), "exhausted 429s must return Err");

    let requests = server.received_requests().await.expect("wiremock requests");
    assert_eq!(
        requests.len(),
        3,
        "429 + Retry-After should still retry the schedule's 3 attempts"
    );

    // With Retry-After: 350 honored end-to-end:
    //   sleep before attempt 2 = min(cap_for_slot(0)=36s, max(jitter(30s), 350s))
    //                          = min(36, 350) = 36s
    //   sleep before attempt 3 = min(cap_for_slot(1)=360s, max(jitter(300s), 350s))
    //                          = min(360, 350) = 350s
    // Total chain virtual time ≈ 36 + 350 = ~386s, plus driver yield slop.
    //
    // Without Retry-After honoring (B1 regression), the chain would sleep
    // ~jitter(30) + jitter(300) = ~24..36 + ~240..360 = at most ~396s,
    // typically ~330s. The 350s on the second sleep is the distinguishing
    // signal — it MUST be present, not the jittered ~300s. We assert
    // elapsed >= 380s (350 + 30 = 380 minimum).
    assert!(
        elapsed_virtual >= Duration::from_secs(380),
        "with Retry-After: 350 honored, total chain virtual time must be ≥ 380s; \
         got {:?}. If this is ≪ 380s, Retry-After is being ignored (B1 bug regressed).",
        elapsed_virtual
    );
}

#[tokio::test(flavor = "current_thread")]
async fn receiver_429_with_retry_after_9999_is_capped() {
    // T-20-03 (DoS) regression lock: receiver-controlled Retry-After: 9999 must
    // be capped at cap_for_slot(prev_slot). The chain must NOT sleep 9999s.
    //
    // Sleep before attempt 2 = min(cap_for_slot(0)=36s, max(jitter, 9999)) = 36s
    // Sleep before attempt 3 = min(cap_for_slot(1)=360s, max(jitter, 9999)) = 360s
    // Total ≈ 36 + 360 = 396s. Without the cap it would be ~9999 + 9999 = 19998s.
    let pool = setup_test_db().await;
    let (job_id, run_id) = seed_job_with_failed_run(&pool, "retry-after-cap-job").await;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "9999"))
        .mount(&server)
        .await;
    let server_uri = server.uri();

    let cancel = CancellationToken::new();
    let dispatcher = build_dispatcher(pool.clone(), &server_uri, job_id, cancel.clone()).await;
    let event = make_run_finalized(run_id, job_id, "retry-after-cap-job");

    tokio::time::pause();
    let start_virtual = tokio::time::Instant::now();

    let result = tokio::select! {
        r = dispatcher.deliver(&event) => r,
        _ = async {
            loop {
                tokio::task::yield_now().await;
                tokio::time::advance(Duration::from_millis(500)).await;
            }
        } => unreachable!(),
    };

    let elapsed_virtual = start_virtual.elapsed();
    assert!(result.is_err(), "exhausted capped 429s must return Err");

    // Cap upper bound: 36 + 360 = 396s. Slack 50s for driver loop.
    assert!(
        elapsed_virtual <= Duration::from_secs(450),
        "Retry-After: 9999 must be capped at cap_for_slot(); chain virtual time \
         must be ≤ ~450s, got {:?}. If this is in the thousands, the cap was \
         not honored (T-20-03 regression).",
        elapsed_virtual
    );
}

#[tokio::test(flavor = "current_thread")]
async fn receiver_200_no_sleep() {
    // Happy-path control: receiver returns 200 → chain returns Ok on attempt 1
    // with no sleep. Virtual elapsed time must be well under schedule[1] = 30s.
    let pool = setup_test_db().await;
    let (job_id, run_id) = seed_job_with_failed_run(&pool, "happy-path-job").await;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let server_uri = server.uri();

    let cancel = CancellationToken::new();
    let dispatcher = build_dispatcher(pool.clone(), &server_uri, job_id, cancel.clone()).await;
    let event = make_run_finalized(run_id, job_id, "happy-path-job");

    tokio::time::pause();
    let start_virtual = tokio::time::Instant::now();

    let result = tokio::select! {
        r = dispatcher.deliver(&event) => r,
        _ = async {
            loop {
                tokio::task::yield_now().await;
                tokio::time::advance(Duration::from_millis(500)).await;
            }
        } => unreachable!(),
    };
    let elapsed = start_virtual.elapsed();

    assert!(result.is_ok(), "200 → Ok; got {result:?}");
    // schedule[1] = 30s — assert virtual elapsed is well under that to prove
    // the chain did NOT enter the retry sleep path. Driver loop overhead
    // accounts for ~10-15s of virtual-time slop on a 200 response.
    assert!(
        elapsed < Duration::from_secs(25),
        "200 on attempt 1 → must NOT enter schedule sleep (≪ 30s); got {elapsed:?}"
    );
    let requests = server.received_requests().await.expect("wiremock requests");
    assert_eq!(requests.len(), 1, "200 must NOT retry");
}

#[test]
fn cap_for_slot_matches_research_table() {
    // RESEARCH §4.7 + CONTEXT D-08 lock. Locked schedule [0s, 30s, 300s]:
    //   slot 0 → schedule[1]*1.2 = 36s
    //   slot 1 → schedule[2]*1.2 = 360s
    //   slot 2 → no slot 3, fallback schedule[2]*1.2 = 360s
    let schedule = [
        Duration::ZERO,
        Duration::from_secs(30),
        Duration::from_secs(300),
    ];
    assert_eq!(cap_for_slot(0, &schedule), Duration::from_secs_f64(36.0));
    assert_eq!(cap_for_slot(1, &schedule), Duration::from_secs_f64(360.0));
    assert_eq!(cap_for_slot(2, &schedule), Duration::from_secs_f64(360.0));
}

#[test]
fn parse_retry_after_integer_seconds_only() {
    // D-07 lock: integer-seconds form parses; HTTP-date form returns None
    // (and emits WARN — log assertion lives in the in-module unit test).
    use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};

    let mut h = HeaderMap::new();
    h.insert(RETRY_AFTER, HeaderValue::from_static("60"));
    assert_eq!(
        parse_retry_after_from_response(&h, "http://test/", 429),
        Some(Duration::from_secs(60))
    );

    let mut h = HeaderMap::new();
    h.insert(
        RETRY_AFTER,
        HeaderValue::from_static("Wed, 01 May 2026 12:00:00 GMT"),
    );
    assert_eq!(
        parse_retry_after_from_response(&h, "http://test/", 429),
        None,
        "HTTP-date form must return None per D-07"
    );

    let h = HeaderMap::new();
    assert_eq!(
        parse_retry_after_from_response(&h, "http://test/", 200),
        None,
        "missing header → None"
    );
}
