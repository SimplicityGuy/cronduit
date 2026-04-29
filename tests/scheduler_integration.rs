//! End-to-end integration tests for the Phase 2 scheduler pipeline.
//!
//! Validates the complete flow: config sync -> job execution (command + script) ->
//! log capture to DB -> exit code mapping -> timeout handling -> job disabling ->
//! concurrent runs.
//!
//! All tests run against in-memory SQLite. No web server or signal handler involved.

use std::time::Duration;

use cronduit::config::{Config, JobConfig, ServerConfig};
use cronduit::db::DbPool;
use cronduit::db::queries::{self, DbJob, PoolRef};
use cronduit::scheduler::RunEntry;
use cronduit::scheduler::run::run_job;
use cronduit::scheduler::sync::sync_config_to_db;
use secrecy::SecretString;
use sqlx::Row;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

fn test_active_runs() -> Arc<RwLock<HashMap<i64, RunEntry>>> {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Phase 15 / WH-02 — per-test webhook channel. The Receiver is dropped
/// immediately. `finalize_run`'s `try_send` on a closed channel returns
/// `TrySendError::Closed` (logged at error per D-04); the integration tests
/// here do not assert on webhook behavior so this is harmless.
fn test_webhook_tx() -> tokio::sync::mpsc::Sender<cronduit::webhooks::RunFinalized> {
    let (tx, _rx) = cronduit::webhooks::channel_with_capacity(8);
    tx
}

async fn setup_test_db() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    pool.migrate().await.unwrap();
    pool
}

fn make_server_config() -> ServerConfig {
    ServerConfig {
        bind: "127.0.0.1:0".to_string(),
        database_url: SecretString::from("sqlite::memory:"),
        timezone: "UTC".to_string(),
        shutdown_grace: Duration::from_secs(5),
        log_retention: Duration::from_secs(86400),
        watch_config: true,
    }
}

fn make_job(
    name: &str,
    schedule: &str,
    command: Option<&str>,
    script: Option<&str>,
    timeout: Option<Duration>,
) -> JobConfig {
    JobConfig {
        name: name.into(),
        schedule: schedule.into(),
        command: command.map(|s| s.into()),
        script: script.map(|s| s.into()),
        image: None,
        use_defaults: None,
        env: BTreeMap::new(),
        volumes: None,
        labels: None,
        network: None,
        container_name: None,
        timeout,
        delete: None,
        cmd: None,
        webhook: None,
    }
}

fn test_config_with_jobs(jobs: Vec<JobConfig>) -> Config {
    Config {
        server: make_server_config(),
        defaults: None,
        jobs,
    }
}

/// Helper: sync config and get the DbJob for a given job name.
async fn sync_and_get_job(pool: &DbPool, config: &Config, name: &str) -> DbJob {
    sync_config_to_db(pool, config, Duration::from_secs(0))
        .await
        .unwrap();
    queries::get_job_by_name(pool, name)
        .await
        .unwrap()
        .expect("job should exist after sync")
}

#[tokio::test]
async fn test_command_job_fires_and_captures_logs() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let pool = setup_test_db().await;
        let config = test_config_with_jobs(vec![make_job(
            "test-echo",
            "* * * * *",
            Some("echo integration-test-output"),
            None,
            None,
        )]);

        let job = sync_and_get_job(&pool, &config, "test-echo").await;

        // Verify job synced correctly.
        assert!(job.enabled);
        assert_eq!(job.job_type, "command");

        // Run the job directly (not through the scheduler loop).
        let cancel = CancellationToken::new();
        let result = run_job(
            pool.clone(),
            None,
            job.clone(),
            "scheduled".to_string(),
            cancel,
            test_active_runs(),
            test_webhook_tx(),
        )
        .await;

        assert_eq!(result.status, "success");
        assert!(result.run_id > 0);

        // Verify job_runs row.
        match pool.reader() {
            PoolRef::Sqlite(p) => {
                let row =
                    sqlx::query("SELECT status, trigger, exit_code FROM job_runs WHERE id = ?1")
                        .bind(result.run_id)
                        .fetch_one(p)
                        .await
                        .unwrap();
                assert_eq!(row.get::<String, _>("status"), "success");
                assert_eq!(row.get::<String, _>("trigger"), "scheduled");
                assert_eq!(row.get::<Option<i32>, _>("exit_code"), Some(0));
            }
            _ => unreachable!(),
        }

        // Verify log lines captured.
        match pool.reader() {
            PoolRef::Sqlite(p) => {
                let logs = sqlx::query("SELECT stream, line FROM job_logs WHERE run_id = ?1")
                    .bind(result.run_id)
                    .fetch_all(p)
                    .await
                    .unwrap();
                assert!(!logs.is_empty(), "should have captured log lines");
                let has_output = logs.iter().any(|r| {
                    let stream: String = r.get("stream");
                    let line: String = r.get("line");
                    stream == "stdout" && line == "integration-test-output"
                });
                assert!(
                    has_output,
                    "should have captured 'integration-test-output' on stdout"
                );
            }
            _ => unreachable!(),
        }

        pool.close().await;
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn test_script_job_fires_and_captures_logs() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let pool = setup_test_db().await;
        let config = test_config_with_jobs(vec![make_job(
            "test-script",
            "* * * * *",
            None,
            Some("echo script-output\necho err-output >&2"),
            None,
        )]);

        let job = sync_and_get_job(&pool, &config, "test-script").await;
        assert_eq!(job.job_type, "script");

        let cancel = CancellationToken::new();
        let result = run_job(
            pool.clone(),
            None,
            job,
            "scheduled".to_string(),
            cancel,
            test_active_runs(),
            test_webhook_tx(),
        )
        .await;

        assert_eq!(result.status, "success");

        // Verify both stdout and stderr captured.
        match pool.reader() {
            PoolRef::Sqlite(p) => {
                let logs = sqlx::query("SELECT stream, line FROM job_logs WHERE run_id = ?1")
                    .bind(result.run_id)
                    .fetch_all(p)
                    .await
                    .unwrap();

                let has_stdout = logs.iter().any(|r| {
                    let stream: String = r.get("stream");
                    let line: String = r.get("line");
                    stream == "stdout" && line == "script-output"
                });
                assert!(has_stdout, "should have captured 'script-output' on stdout");

                let has_stderr = logs.iter().any(|r| {
                    let stream: String = r.get("stream");
                    let line: String = r.get("line");
                    stream == "stderr" && line == "err-output"
                });
                assert!(has_stderr, "should have captured 'err-output' on stderr");
            }
            _ => unreachable!(),
        }

        pool.close().await;
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn test_failed_command_records_exit_code() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let pool = setup_test_db().await;
        let config = test_config_with_jobs(vec![make_job(
            "test-fail",
            "* * * * *",
            Some("sh -c 'exit 42'"),
            None,
            None,
        )]);

        let job = sync_and_get_job(&pool, &config, "test-fail").await;

        let cancel = CancellationToken::new();
        let result = run_job(
            pool.clone(),
            None,
            job,
            "scheduled".to_string(),
            cancel,
            test_active_runs(),
            test_webhook_tx(),
        )
        .await;

        assert_eq!(result.status, "failed");

        // Verify exit code in DB.
        match pool.reader() {
            PoolRef::Sqlite(p) => {
                let row = sqlx::query("SELECT status, exit_code FROM job_runs WHERE id = ?1")
                    .bind(result.run_id)
                    .fetch_one(p)
                    .await
                    .unwrap();
                assert_eq!(row.get::<String, _>("status"), "failed");
                assert_eq!(row.get::<Option<i32>, _>("exit_code"), Some(42));
            }
            _ => unreachable!(),
        }

        pool.close().await;
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn test_timeout_preserves_partial_logs() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let pool = setup_test_db().await;
        let config = test_config_with_jobs(vec![make_job(
            "test-timeout",
            "* * * * *",
            Some("sh -c 'echo before-timeout; sleep 60'"),
            None,
            Some(Duration::from_secs(1)),
        )]);

        let job = sync_and_get_job(&pool, &config, "test-timeout").await;

        let cancel = CancellationToken::new();
        let result = run_job(
            pool.clone(),
            None,
            job,
            "scheduled".to_string(),
            cancel,
            test_active_runs(),
            test_webhook_tx(),
        )
        .await;

        assert_eq!(result.status, "timeout");

        // Verify partial logs preserved.
        match pool.reader() {
            PoolRef::Sqlite(p) => {
                let row = sqlx::query("SELECT status FROM job_runs WHERE id = ?1")
                    .bind(result.run_id)
                    .fetch_one(p)
                    .await
                    .unwrap();
                assert_eq!(row.get::<String, _>("status"), "timeout");

                let logs = sqlx::query("SELECT line FROM job_logs WHERE run_id = ?1")
                    .bind(result.run_id)
                    .fetch_all(p)
                    .await
                    .unwrap();
                let has_before = logs.iter().any(|r| {
                    let line: String = r.get("line");
                    line == "before-timeout"
                });
                assert!(has_before, "partial logs should be preserved on timeout");
            }
            _ => unreachable!(),
        }

        pool.close().await;
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn test_sync_disables_removed_jobs() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let pool = setup_test_db().await;

        // First sync with two jobs.
        let config1 = test_config_with_jobs(vec![
            make_job("job-a", "* * * * *", Some("echo a"), None, None),
            make_job("job-b", "* * * * *", Some("echo b"), None, None),
        ]);
        let result1 = sync_config_to_db(&pool, &config1, Duration::from_secs(0))
            .await
            .unwrap();
        assert_eq!(result1.jobs.len(), 2);

        // Run job-b so it has run history.
        let job_b = queries::get_job_by_name(&pool, "job-b")
            .await
            .unwrap()
            .unwrap();
        let cancel = CancellationToken::new();
        let run_result = run_job(
            pool.clone(),
            None,
            job_b,
            "scheduled".to_string(),
            cancel,
            test_active_runs(),
            test_webhook_tx(),
        )
        .await;
        assert_eq!(run_result.status, "success");

        // Second sync with only job-a (job-b removed).
        let config2 = test_config_with_jobs(vec![make_job(
            "job-a",
            "* * * * *",
            Some("echo a"),
            None,
            None,
        )]);
        let result2 = sync_config_to_db(&pool, &config2, Duration::from_secs(0))
            .await
            .unwrap();
        assert_eq!(result2.disabled, 1);

        // Verify job-b is disabled.
        let disabled_b = queries::get_job_by_name(&pool, "job-b")
            .await
            .unwrap()
            .unwrap();
        assert!(!disabled_b.enabled);

        // Verify job-b's run history is still queryable.
        match pool.reader() {
            PoolRef::Sqlite(p) => {
                let runs = sqlx::query("SELECT status FROM job_runs WHERE job_id = ?1")
                    .bind(disabled_b.id)
                    .fetch_all(p)
                    .await
                    .unwrap();
                assert_eq!(
                    runs.len(),
                    1,
                    "run history should be preserved for disabled job"
                );
                assert_eq!(runs[0].get::<String, _>("status"), "success");
            }
            _ => unreachable!(),
        }

        // Verify job-a still enabled.
        let kept_a = queries::get_job_by_name(&pool, "job-a")
            .await
            .unwrap()
            .unwrap();
        assert!(kept_a.enabled);

        pool.close().await;
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn test_concurrent_runs_same_job() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let pool = setup_test_db().await;
        let config = test_config_with_jobs(vec![make_job(
            "concurrent-job",
            "* * * * *",
            Some("echo concurrent"),
            None,
            None,
        )]);

        let job = sync_and_get_job(&pool, &config, "concurrent-job").await;

        // Spawn two concurrent runs.
        let cancel1 = CancellationToken::new();
        let cancel2 = CancellationToken::new();
        let pool1 = pool.clone();
        let pool2 = pool.clone();
        let job1 = job.clone();
        let job2 = job.clone();

        let (r1, r2) = tokio::join!(
            run_job(
                pool1,
                None,
                job1,
                "scheduled".to_string(),
                cancel1,
                test_active_runs(),
                test_webhook_tx()
            ),
            run_job(
                pool2,
                None,
                job2,
                "scheduled".to_string(),
                cancel2,
                test_active_runs(),
                test_webhook_tx()
            ),
        );

        assert_ne!(
            r1.run_id, r2.run_id,
            "concurrent runs must have different run IDs"
        );
        assert_eq!(r1.status, "success");
        assert_eq!(r2.status, "success");

        // Verify two separate rows in job_runs.
        match pool.reader() {
            PoolRef::Sqlite(p) => {
                let count = sqlx::query("SELECT COUNT(*) as cnt FROM job_runs WHERE job_id = ?1")
                    .bind(job.id)
                    .fetch_one(p)
                    .await
                    .unwrap();
                let cnt: i64 = count.get("cnt");
                assert_eq!(cnt, 2, "should have two separate job_runs rows");
            }
            _ => unreachable!(),
        }

        pool.close().await;
    })
    .await
    .unwrap();
}
