//! Docker job `success_exit_codes` integration test.
//!
//! Requires a running Docker daemon; gated with `#[ignore]` like the other
//! Docker executor tests. Run with:
//! `cargo test --test success_exit_codes_docker -- --ignored --nocapture --test-threads=1`
//!
//! Containers are created, owned and removed by `execute_docker` itself
//! (cleanup also on failure paths); no unrelated containers are touched.

use std::collections::HashMap;
use std::sync::Arc;

use bollard::Docker;
use cronduit::db::DbPool;
use cronduit::db::queries::{self, DbJob};
use cronduit::scheduler::RunEntry;
use cronduit::scheduler::run::run_job;
use sqlx::Row;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

async fn seed_docker_job(pool: &DbPool, name: &str, config_json: &str) -> DbJob {
    let id = queries::upsert_job(
        pool,
        name,
        "0 0 31 2 *",
        "0 0 31 2 *",
        "docker",
        config_json,
        "sec-docker-hash",
        3600,
        "[]",
    )
    .await
    .expect("upsert job");
    DbJob {
        id,
        name: name.to_string(),
        schedule: "0 0 31 2 *".to_string(),
        resolved_schedule: "0 0 31 2 *".to_string(),
        job_type: "docker".to_string(),
        config_json: config_json.to_string(),
        config_hash: "sec-docker-hash".to_string(),
        enabled: true,
        enabled_override: None,
        timeout_secs: 3600,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

async fn run_docker(name: &str, config_json: &str) -> (String, Option<i32>, String, Option<i32>) {
    let docker = Docker::connect_with_local_defaults().expect("Docker daemon must be running");
    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    pool.migrate().await.unwrap();
    let job = seed_docker_job(&pool, name, config_json).await;
    let active: Arc<RwLock<HashMap<i64, RunEntry>>> = Arc::new(RwLock::new(HashMap::new()));
    let (tx, mut rx) = cronduit::webhooks::channel_with_capacity(8);
    let result = run_job(
        pool.clone(),
        Some(docker),
        job,
        "manual".to_string(),
        CancellationToken::new(),
        active,
        tx,
        None,
    )
    .await;
    let code = match pool.reader() {
        cronduit::db::queries::PoolRef::Sqlite(p) => {
            sqlx::query("SELECT exit_code FROM job_runs WHERE id = ?1")
                .bind(result.run_id)
                .fetch_one(p)
                .await
                .unwrap()
                .get::<Option<i32>, _>("exit_code")
        }
        _ => unreachable!(),
    };
    let ev = rx.try_recv().expect("webhook event");
    pool.close().await;
    (result.status, code, ev.status, ev.exit_code)
}

#[tokio::test]
#[ignore]
async fn docker_job_listed_exit_code_is_success() {
    let (status, code, ev_status, ev_code) = run_docker(
        "sec-docker-listed",
        r#"{"image":"alpine:latest","cmd":["sh","-c","exit 75"],"success_exit_codes":[0,75]}"#,
    )
    .await;
    assert_eq!(status, "success");
    assert_eq!(code, Some(75));
    assert_eq!(ev_status, "success");
    assert_eq!(ev_code, Some(75));
}

#[tokio::test]
#[ignore]
async fn docker_job_unlisted_exit_code_still_fails() {
    let (status, code, ev_status, _) = run_docker(
        "sec-docker-unlisted",
        r#"{"image":"alpine:latest","cmd":["sh","-c","exit 3"],"success_exit_codes":[0,75]}"#,
    )
    .await;
    assert_eq!(status, "failed");
    assert_eq!(code, Some(3));
    assert_eq!(ev_status, "failed");
}
