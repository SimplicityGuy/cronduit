//! v12_webhook_delivery_e2e.rs (Phase 18 / WH-03 / WH-09)
//!
//! End-to-end wiremock round-trip:
//!   - `webhook_delivery_e2e_signed` — single-job: 3 Standard Webhooks v1
//!     headers, 16-field body, recomputed signature matches.
//!   - `v12_webhook_two_jobs_distinct_urls` — 2-job alignment regression
//!     (T-18-36): job 1 → URL A, job 2 → URL B. Locks the name-keyed
//!     lookup in src/cli/run.rs against future drift if `sync_config_to_db`
//!     ever reorders or filters its returned job list.

use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine;
use chrono::{TimeZone, Utc};
use hmac::{KeyInit, Mac};
use secrecy::SecretString;
use sha2::Sha256;
use sqlx::Row;
use wiremock::matchers::{header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use cronduit::config::WebhookConfig;
use cronduit::db::DbPool;
use cronduit::db::queries::PoolRef;
use cronduit::webhooks::{HttpDispatcher, RunFinalized, WebhookDispatcher};

type HmacSha256 = hmac::Hmac<Sha256>;

/// In-memory SQLite + all migrations applied (mirrors v11_fixtures helper +
/// tests/v12_fctx_streak.rs:setup_pool).
async fn setup_test_db() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    pool.migrate().await.expect("apply migrations");
    pool
}

/// Insert a job row (config_hash placeholder) and a single `failed` job_run
/// at a known historical timestamp. Returns (job_id, run_id). The run's
/// `start_time` is older than the `event.started_at` we'll use below so
/// the filter_position SQL `start_time <= ?2` predicate matches.
async fn seed_named_job_with_failed_run(pool: &DbPool, job_name: &str) -> (i64, i64) {
    let now = chrono::Utc::now().to_rfc3339();
    let p = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };

    // 1. Job row.
    let job_row = sqlx::query(
        "INSERT INTO jobs (name, schedule, resolved_schedule, job_type, config_json, config_hash, timeout_secs, created_at, updated_at) \
         VALUES (?1, '* * * * *', '* * * * *', 'command', '{}', 'seed-cfg', 60, ?2, ?2) RETURNING id",
    )
    .bind(job_name)
    .bind(&now)
    .fetch_one(p)
    .await
    .expect("seed job");
    let job_id: i64 = job_row.get("id");

    // 2. Failed job_run. Use a fixed historical RFC3339 timestamp older than
    //    the event timestamp the tests pass to dispatcher.deliver().
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
async fn webhook_delivery_e2e_signed() {
    // 1. Mock receiver.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/hook"))
        .and(header_exists("content-type"))
        .and(header_exists("webhook-id"))
        .and(header_exists("webhook-timestamp"))
        .and(header_exists("webhook-signature"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    // 2. Build dispatcher with a single job webhook pointed at the mock.
    let pool = setup_test_db().await;
    let (job_id, run_id) = seed_named_job_with_failed_run(&pool, "e2e-test-job").await;
    let url = format!("{}/hook", server.uri());
    let secret = "test-secret-shh";
    let cfg = WebhookConfig {
        url: url.clone(),
        states: vec!["failed".into()],
        secret: Some(SecretString::from(secret)),
        unsigned: false,
        fire_every: 0, // always fire — simplifies coalesce
    };
    let mut webhooks = HashMap::new();
    webhooks.insert(job_id, cfg);
    let dispatcher = HttpDispatcher::new(pool, Arc::new(webhooks)).unwrap();

    // 3. Emit a RunFinalized.
    let event = RunFinalized {
        run_id,
        job_id,
        job_name: "e2e-test-job".to_string(),
        status: "failed".to_string(),
        exit_code: Some(1),
        started_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 0).unwrap(),
        finished_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 5).unwrap(),
    };
    dispatcher.deliver(&event).await.unwrap();

    // 4. Inspect the received request.
    let received = server.received_requests().await.unwrap();
    assert_eq!(received.len(), 1, "expected exactly 1 delivery");
    let req = &received[0];

    let h_id = req.headers.get("webhook-id").unwrap().to_str().unwrap();
    let h_ts = req
        .headers
        .get("webhook-timestamp")
        .unwrap()
        .to_str()
        .unwrap();
    let h_sig = req
        .headers
        .get("webhook-signature")
        .unwrap()
        .to_str()
        .unwrap();
    let h_ct = req.headers.get("content-type").unwrap().to_str().unwrap();

    assert_eq!(h_ct, "application/json", "Content-Type per D-11");
    assert_eq!(h_id.len(), 26, "webhook-id is 26-char ULID per D-09");
    assert_eq!(
        h_ts.len(),
        10,
        "webhook-timestamp is 10-digit Unix seconds per D-09 / Pitfall D"
    );
    assert!(
        h_sig.starts_with("v1,"),
        "signature header value is `v1,<base64>` per D-09"
    );

    // 5. Body — 16 fields.
    let body = std::str::from_utf8(&req.body).expect("utf-8 body");
    for field in [
        "\"payload_version\":\"v1\"",
        "\"event_type\":\"run_finalized\"",
        "\"run_id\":",
        "\"job_id\":",
        "\"job_name\":",
        "\"status\":\"failed\"",
        "\"exit_code\":",
        "\"started_at\":",
        "\"finished_at\":",
        "\"duration_ms\":",
        "\"streak_position\":",
        "\"consecutive_failures\":",
        "\"image_digest\":",
        "\"config_hash\":",
        "\"tags\":",
        "\"cronduit_version\":",
    ] {
        assert!(
            body.contains(field),
            "missing payload field {field}; body: {body}"
        );
    }

    // 6. Recompute the signature and compare.
    let prefix = format!("{h_id}.{h_ts}.");
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(prefix.as_bytes());
    mac.update(&req.body);
    let expected_b64 =
        base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
    let actual_b64 = h_sig.strip_prefix("v1,").unwrap();
    assert_eq!(
        actual_b64, expected_b64,
        "signature must equal HMAC-SHA256 over `${{id}}.${{ts}}.${{body}}`"
    );
}

/// T-18-36 regression — locks the name-keyed lookup in src/cli/run.rs.
/// Two jobs with DIFFERENT webhook URLs must each route to their own URL,
/// even if `sync_config_to_db`'s returned `jobs[]` list is not in cfg.jobs[]
/// declaration order.
#[tokio::test]
async fn v12_webhook_two_jobs_distinct_urls() {
    // 1. Two independent mock receivers (different URLs).
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/job-a"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server_a)
        .await;
    Mock::given(method("POST"))
        .and(path("/job-b"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server_b)
        .await;
    let url_a = format!("{}/job-a", server_a.uri());
    let url_b = format!("{}/job-b", server_b.uri());

    // 2. Seed two named jobs in the DB.
    let pool = setup_test_db().await;
    let (job_a_id, run_a_id) = seed_named_job_with_failed_run(&pool, "job-alpha").await;
    let (job_b_id, run_b_id) = seed_named_job_with_failed_run(&pool, "job-beta").await;

    // 3. Build the per-job webhook map directly (mirrors what src/cli/run.rs
    //    constructs via the name-keyed lookup). Each job points at its OWN URL.
    let cfg_a = WebhookConfig {
        url: url_a.clone(),
        states: vec!["failed".into()],
        secret: Some(SecretString::from("k-a")),
        unsigned: false,
        fire_every: 0,
    };
    let cfg_b = WebhookConfig {
        url: url_b.clone(),
        states: vec!["failed".into()],
        secret: Some(SecretString::from("k-b")),
        unsigned: false,
        fire_every: 0,
    };
    let mut webhooks = HashMap::new();
    webhooks.insert(job_a_id, cfg_a);
    webhooks.insert(job_b_id, cfg_b);
    let dispatcher = HttpDispatcher::new(pool, Arc::new(webhooks)).unwrap();

    // 4. Fire job A first, then job B.
    let mk_event = |run_id: i64, job_id: i64, name: &str| RunFinalized {
        run_id,
        job_id,
        job_name: name.to_string(),
        status: "failed".to_string(),
        exit_code: Some(1),
        started_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 0).unwrap(),
        finished_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 1).unwrap(),
    };
    dispatcher
        .deliver(&mk_event(run_a_id, job_a_id, "job-alpha"))
        .await
        .unwrap();
    dispatcher
        .deliver(&mk_event(run_b_id, job_b_id, "job-beta"))
        .await
        .unwrap();

    // 5. Assert: server A got exactly 1 hit (from job A), server B got
    //    exactly 1 hit (from job B). NEITHER server got crossed traffic.
    let recv_a = server_a.received_requests().await.unwrap();
    let recv_b = server_b.received_requests().await.unwrap();
    assert_eq!(recv_a.len(), 1, "job-alpha must hit server A exactly once");
    assert_eq!(recv_b.len(), 1, "job-beta must hit server B exactly once");

    // 6. Cross-check via job_name in the body — server A's request must
    //    carry job_name="job-alpha", server B's must carry "job-beta".
    let body_a = std::str::from_utf8(&recv_a[0].body).unwrap();
    let body_b = std::str::from_utf8(&recv_b[0].body).unwrap();
    assert!(
        body_a.contains("\"job_name\":\"job-alpha\""),
        "server A's body must reference job-alpha; got: {body_a}"
    );
    assert!(
        body_b.contains("\"job_name\":\"job-beta\""),
        "server B's body must reference job-beta; got: {body_b}"
    );
    assert!(
        !body_a.contains("\"job_name\":\"job-beta\""),
        "server A must NOT receive job-beta's payload (alignment leak)"
    );
    assert!(
        !body_b.contains("\"job_name\":\"job-alpha\""),
        "server B must NOT receive job-alpha's payload (alignment leak)"
    );
}
