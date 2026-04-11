//! Per-run task lifecycle: insert_running -> dispatch -> log writer -> finalize.
//!
//! SCHED-04: Each fired job spawns as a tokio task tracked in a JoinSet.
//! SCHED-05: Per-job timeout enforced; timeout produces status=timeout with partial logs.
//! SCHED-06: Concurrent runs of the same job each create separate job_runs rows.

use std::time::Duration;

use bollard::Docker;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use super::RunResult;
use super::command::{self, RunStatus};
use super::log_pipeline::{self, DEFAULT_BATCH_SIZE, DEFAULT_CHANNEL_CAPACITY, LogReceiver};
use crate::db::DbPool;
use crate::db::queries::{DbJob, finalize_run, insert_log_batch, insert_running_run};

/// Config fields extracted from `config_json` for dispatch.
#[derive(Deserialize)]
struct JobExecConfig {
    command: Option<String>,
    script: Option<String>,
}

/// Execute a job run through its full lifecycle.
///
/// 1. Insert running row
/// 2. Create log channel
/// 3. Spawn log writer task
/// 4. Dispatch to command/script executor
/// 5. Close sender (signals log writer to finish)
/// 6. Wait for log writer to complete
/// 7. Finalize run
/// 8. Return RunResult
pub async fn run_job(
    pool: DbPool,
    docker: Option<Docker>,
    job: DbJob,
    trigger: String,
    cancel: CancellationToken,
) -> RunResult {
    let start = tokio::time::Instant::now();

    // 1. Insert running row.
    let run_id = match insert_running_run(&pool, job.id, &trigger).await {
        Ok(id) => id,
        Err(e) => {
            tracing::error!(
                target: "cronduit.run",
                job = %job.name,
                error = %e,
                "failed to insert running run"
            );
            return RunResult {
                run_id: 0,
                status: "error".to_string(),
            };
        }
    };

    tracing::info!(
        target: "cronduit.run",
        job = %job.name,
        run_id,
        trigger = %trigger,
        "run started"
    );

    // 2. Create log channel.
    let (sender, receiver) = log_pipeline::channel(DEFAULT_CHANNEL_CAPACITY);

    // 3. Spawn log writer task.
    let writer_pool = pool.clone();
    let writer_handle = tokio::spawn(log_writer_task(writer_pool, run_id, receiver));

    // 4. Dispatch to executor based on job type.
    // Negative or zero timeout_secs (e.g. from corrupted DB) is treated as "no timeout".
    // The `<= 0` guard ensures negative i64 values never reach the `as u64` cast,
    // which would silently wrap to a very large duration.
    let timeout = if job.timeout_secs <= 0 {
        tracing::debug!(
            target: "cronduit.run",
            job = %job.name,
            timeout_secs = job.timeout_secs,
            "timeout_secs <= 0, using effectively-infinite timeout (1 year)"
        );
        Duration::from_secs(86400 * 365) // effectively no timeout
    } else {
        Duration::from_secs(job.timeout_secs as u64)
    };

    let mut container_id_for_finalize: Option<String> = None;

    let exec_result = match job.job_type.as_str() {
        "command" => {
            let config: JobExecConfig =
                serde_json::from_str(&job.config_json).unwrap_or(JobExecConfig {
                    command: None,
                    script: None,
                });
            match config.command {
                Some(cmd) => command::execute_command(&cmd, timeout, cancel, sender.clone()).await,
                None => {
                    sender.close();
                    command::ExecResult {
                        exit_code: None,
                        status: RunStatus::Error,
                        error_message: Some(
                            "command job missing 'command' field in config_json".to_string(),
                        ),
                    }
                }
            }
        }
        "script" => {
            let config: JobExecConfig =
                serde_json::from_str(&job.config_json).unwrap_or(JobExecConfig {
                    command: None,
                    script: None,
                });
            match config.script {
                Some(body) => {
                    // D-15: Default shebang is #!/bin/sh
                    super::script::execute_script(
                        &body,
                        "#!/bin/sh",
                        timeout,
                        cancel,
                        sender.clone(),
                    )
                    .await
                }
                None => {
                    sender.close();
                    command::ExecResult {
                        exit_code: None,
                        status: RunStatus::Error,
                        error_message: Some(
                            "script job missing 'script' field in config_json".to_string(),
                        ),
                    }
                }
            }
        }
        "docker" => match &docker {
            Some(docker_client) => {
                let docker_result = super::docker::execute_docker(
                    docker_client,
                    &job.config_json,
                    &job.name,
                    run_id,
                    timeout,
                    cancel,
                    sender.clone(),
                )
                .await;
                container_id_for_finalize = docker_result.image_digest.clone();
                docker_result.exec
            }
            None => {
                sender.close();
                command::ExecResult {
                    exit_code: None,
                    status: RunStatus::Error,
                    error_message: Some(
                        "docker executor unavailable (no Docker client)".to_string(),
                    ),
                }
            }
        },
        other => {
            sender.close();
            command::ExecResult {
                exit_code: None,
                status: RunStatus::Error,
                error_message: Some(format!("unknown job type: {other}")),
            }
        }
    };

    // 5. Ensure sender is closed (executor should have closed it, but be safe).
    sender.close();

    // 6. Wait for log writer to complete.
    if let Err(e) = writer_handle.await {
        tracing::error!(
            target: "cronduit.run",
            run_id,
            error = %e,
            "log writer task panicked"
        );
    }

    // 7. Finalize run.
    let status_str = match exec_result.status {
        RunStatus::Success => "success",
        RunStatus::Failed => "failed",
        RunStatus::Timeout => "timeout",
        RunStatus::Shutdown => "cancelled",
        RunStatus::Error => "error",
    };

    if let Err(e) = finalize_run(
        &pool,
        run_id,
        status_str,
        exec_result.exit_code,
        start,
        exec_result.error_message.as_deref(),
        container_id_for_finalize.as_deref(),
    )
    .await
    {
        tracing::error!(
            target: "cronduit.run",
            run_id,
            error = %e,
            "failed to finalize run"
        );
    }

    tracing::info!(
        target: "cronduit.run",
        job = %job.name,
        run_id,
        status = status_str,
        "run completed"
    );

    // 8. Return RunResult.
    RunResult {
        run_id,
        status: status_str.to_string(),
    }
}

/// Log writer task that drains log lines from the receiver in micro-batches
/// and inserts them into job_logs.
///
/// D-12: Micro-batch inserts of DEFAULT_BATCH_SIZE (64) lines per transaction.
async fn log_writer_task(pool: DbPool, run_id: i64, receiver: LogReceiver) {
    loop {
        let batch = receiver.drain_batch_async(DEFAULT_BATCH_SIZE).await;
        if batch.is_empty() {
            // Channel closed and fully drained.
            break;
        }
        let tuples: Vec<(String, String, String)> = batch
            .into_iter()
            .map(|l| (l.stream, l.ts, l.line))
            .collect();
        if let Err(e) = insert_log_batch(&pool, run_id, &tuples).await {
            tracing::error!(
                target: "cronduit.log_writer",
                run_id,
                error = %e,
                "failed to insert log batch"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::queries::PoolRef;
    use sqlx::Row;

    async fn setup_pool() -> DbPool {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        pool.migrate().await.unwrap();
        pool
    }

    async fn insert_test_job(
        pool: &DbPool,
        name: &str,
        job_type: &str,
        config_json: &str,
    ) -> DbJob {
        let id = crate::db::queries::upsert_job(
            pool,
            name,
            "* * * * *",
            "* * * * *",
            job_type,
            config_json,
            "testhash",
            3600,
        )
        .await
        .unwrap();

        DbJob {
            id,
            name: name.to_string(),
            schedule: "* * * * *".to_string(),
            resolved_schedule: "* * * * *".to_string(),
            job_type: job_type.to_string(),
            config_json: config_json.to_string(),
            config_hash: "testhash".to_string(),
            enabled: true,
            timeout_secs: 3600,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[tokio::test]
    async fn run_job_command_success() {
        let pool = setup_pool().await;
        let job =
            insert_test_job(&pool, "echo-job", "command", r#"{"command":"echo hello"}"#).await;

        let cancel = CancellationToken::new();
        let result = run_job(pool.clone(), None, job, "scheduled".to_string(), cancel).await;

        assert_eq!(result.status, "success");
        assert!(result.run_id > 0);

        // Verify job_runs row.
        match pool.reader() {
            PoolRef::Sqlite(p) => {
                let row = sqlx::query("SELECT status, trigger, exit_code, end_time, duration_ms FROM job_runs WHERE id = ?1")
                    .bind(result.run_id)
                    .fetch_one(p)
                    .await
                    .unwrap();
                let status: String = row.get("status");
                let trigger: String = row.get("trigger");
                let exit_code: Option<i32> = row.get("exit_code");
                assert_eq!(status, "success");
                assert_eq!(trigger, "scheduled");
                assert_eq!(exit_code, Some(0));
                assert!(row.get::<Option<String>, _>("end_time").is_some());
                assert!(row.get::<Option<i64>, _>("duration_ms").is_some());
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
                let has_hello = logs.iter().any(|r| {
                    let line: String = r.get("line");
                    line == "hello"
                });
                assert!(has_hello, "should have captured 'hello' output");
            }
            _ => unreachable!(),
        }

        pool.close().await;
    }

    #[tokio::test]
    async fn run_job_script_success() {
        let pool = setup_pool().await;
        let job = insert_test_job(
            &pool,
            "script-job",
            "script",
            r#"{"script":"echo script-output"}"#,
        )
        .await;

        let cancel = CancellationToken::new();
        let result = run_job(pool.clone(), None, job, "scheduled".to_string(), cancel).await;

        assert_eq!(result.status, "success");
        assert!(result.run_id > 0);

        // Verify log lines captured.
        match pool.reader() {
            PoolRef::Sqlite(p) => {
                let logs = sqlx::query("SELECT line FROM job_logs WHERE run_id = ?1")
                    .bind(result.run_id)
                    .fetch_all(p)
                    .await
                    .unwrap();
                let has_output = logs.iter().any(|r| {
                    let line: String = r.get("line");
                    line == "script-output"
                });
                assert!(has_output, "should have captured script output");
            }
            _ => unreachable!(),
        }

        pool.close().await;
    }

    #[tokio::test]
    async fn run_job_timeout_preserves_partial_logs() {
        let pool = setup_pool().await;
        let mut job = insert_test_job(
            &pool,
            "timeout-job",
            "command",
            r#"{"command":"sh -c 'echo before-timeout; sleep 60'"}"#,
        )
        .await;
        job.timeout_secs = 0; // Will be overridden below

        // Override timeout to be very short.
        let job_with_short_timeout = DbJob {
            timeout_secs: 1, // 1 second - enough time for echo but not sleep
            ..job
        };

        let cancel = CancellationToken::new();
        let result = run_job(
            pool.clone(),
            None,
            job_with_short_timeout,
            "scheduled".to_string(),
            cancel,
        )
        .await;

        assert_eq!(result.status, "timeout");

        // Verify partial logs preserved.
        match pool.reader() {
            PoolRef::Sqlite(p) => {
                let row = sqlx::query("SELECT status, error_message FROM job_runs WHERE id = ?1")
                    .bind(result.run_id)
                    .fetch_one(p)
                    .await
                    .unwrap();
                let status: String = row.get("status");
                assert_eq!(status, "timeout");

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
    }

    #[tokio::test]
    async fn concurrent_runs_create_separate_rows() {
        let pool = setup_pool().await;
        let job = insert_test_job(
            &pool,
            "concurrent-job",
            "command",
            r#"{"command":"echo concurrent"}"#,
        )
        .await;

        let cancel1 = CancellationToken::new();
        let cancel2 = CancellationToken::new();
        let pool1 = pool.clone();
        let pool2 = pool.clone();
        let job1 = job.clone();
        let job2 = job.clone();

        let (r1, r2) = tokio::join!(
            run_job(pool1, None, job1, "scheduled".to_string(), cancel1),
            run_job(pool2, None, job2, "scheduled".to_string(), cancel2),
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
    }
}
