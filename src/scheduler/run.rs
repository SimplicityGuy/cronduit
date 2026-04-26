//! Per-run task lifecycle: insert_running -> dispatch -> log writer -> finalize.
//!
//! SCHED-04: Each fired job spawns as a tokio task tracked in a JoinSet.
//! SCHED-05: Per-job timeout enforced; timeout produces status=timeout with partial logs.
//! SCHED-06: Concurrent runs of the same job each create separate job_runs rows.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bollard::Docker;
use serde::Deserialize;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use super::RunResult;
use super::command::{self, RunStatus};

/// Closed-enum failure reasons for the cronduit_run_failures_total metric (D-05).
/// Cardinality fixed at 6 values -- never add unbounded labels.
pub enum FailureReason {
    ImagePullFailed,
    NetworkTargetUnavailable,
    Timeout,
    ExitNonzero,
    Abandoned,
    Unknown,
}

impl FailureReason {
    pub fn as_label(&self) -> &'static str {
        match self {
            Self::ImagePullFailed => "image_pull_failed",
            Self::NetworkTargetUnavailable => "network_target_unavailable",
            Self::Timeout => "timeout",
            Self::ExitNonzero => "exit_nonzero",
            Self::Abandoned => "abandoned",
            Self::Unknown => "unknown",
        }
    }
}
use super::log_pipeline::{
    self, DEFAULT_BATCH_SIZE, DEFAULT_CHANNEL_CAPACITY, LogLine, LogReceiver,
};
use crate::db::DbPool;
use crate::db::queries::{DbJob, finalize_run, insert_log_batch, insert_running_run};

/// Config fields extracted from `config_json` for dispatch.
#[derive(Deserialize)]
struct JobExecConfig {
    command: Option<String>,
    script: Option<String>,
}

/// Execute a job run through its full lifecycle (scheduler-driven path).
///
/// Used by the cron-tick dispatch + catch-up + the legacy
/// `SchedulerCmd::RunNow` arm (RESEARCH Q1 RESOLVED: scheduled runs continue
/// to insert the row here, on the scheduler task, exactly as before Phase
/// 11). Manual UI "Run Now" clicks go through `run_job_with_existing_run_id`
/// instead — that path has the API handler insert the row on the handler
/// thread first to eliminate the run-detail 404 race (UI-19).
///
/// 1. Insert running row (via `insert_running_run` — the row creator)
/// 2. `continue_run` handles the rest of the lifecycle:
///    - Create log channel
///    - Spawn log writer task
///    - Dispatch to command/script/docker executor
///    - Close sender + wait for log writer
///    - Finalize run, record metrics, remove broadcast_tx
pub async fn run_job(
    pool: DbPool,
    docker: Option<Docker>,
    job: DbJob,
    trigger: String,
    cancel: CancellationToken,
    active_runs: Arc<RwLock<HashMap<i64, crate::scheduler::RunEntry>>>,
    webhook_tx: tokio::sync::mpsc::Sender<crate::webhooks::RunFinalized>,
) -> RunResult {
    let start = tokio::time::Instant::now();

    // 1. Insert running row (scheduler-driven path owns this step).
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

    // 2. Hand off to the shared lifecycle helper.
    continue_run(
        pool,
        docker,
        job,
        run_id,
        start,
        cancel,
        active_runs,
        webhook_tx,
    )
    .await
}

/// Execute a job run through its lifecycle with a PRE-INSERTED run_id.
///
/// Used by the UI Run Now path (Phase 11 UI-19 fix): the API handler at
/// `src/web/handlers/api.rs::run_now` inserts the `job_runs` row on the
/// handler thread BEFORE returning `HX-Refresh: true`, so the browser's
/// immediate navigation to `/jobs/{job_id}/runs/{run_id}` always finds the
/// row. The scheduler's `SchedulerCmd::RunNowWithRunId` arm dispatches this
/// function with the pre-inserted id; this function SKIPS the insert step
/// and calls the shared `continue_run` helper for the rest of the lifecycle.
///
/// The legacy scheduler-driven `run_job` path is preserved unchanged for
/// cron-tick + catch-up runs (RESEARCH Q1 RESOLVED).
pub async fn run_job_with_existing_run_id(
    pool: DbPool,
    docker: Option<Docker>,
    job: DbJob,
    run_id: i64,
    cancel: CancellationToken,
    active_runs: Arc<RwLock<HashMap<i64, crate::scheduler::RunEntry>>>,
    webhook_tx: tokio::sync::mpsc::Sender<crate::webhooks::RunFinalized>,
) -> RunResult {
    let start = tokio::time::Instant::now();
    tracing::info!(
        target: "cronduit.run",
        job = %job.name,
        run_id,
        trigger = "manual",
        "run started (pre-inserted by handler — UI-19)"
    );
    continue_run(
        pool,
        docker,
        job,
        run_id,
        start,
        cancel,
        active_runs,
        webhook_tx,
    )
    .await
}

/// Shared per-run lifecycle AFTER the `job_runs` row has been inserted.
///
/// Callers (`run_job`, `run_job_with_existing_run_id`) are responsible for
/// the INSERT step; this helper handles everything downstream:
/// broadcast channel creation, active_runs insertion, executor dispatch,
/// log writer task lifecycle, finalization, metrics, cleanup.
#[allow(clippy::too_many_arguments)]
async fn continue_run(
    pool: DbPool,
    docker: Option<Docker>,
    job: DbJob,
    run_id: i64,
    start: tokio::time::Instant,
    cancel: CancellationToken,
    active_runs: Arc<RwLock<HashMap<i64, crate::scheduler::RunEntry>>>,
    webhook_tx: tokio::sync::mpsc::Sender<crate::webhooks::RunFinalized>,
) -> RunResult {
    // 1b. Create broadcast channel for SSE subscribers (UI-14, D-03).
    let (broadcast_tx, _rx) = tokio::sync::broadcast::channel::<LogLine>(256);

    // 1c. Construct per-run control (SCHED-10, D-09). Default reason is
    // Shutdown, so any bare `cancel.cancel()` (existing shutdown drain path
    // in `mod.rs`) continues to be classified correctly. Plan 10-04 (this
    // plan) merges the control plane into the active_runs RunEntry so the
    // scheduler Stop arm (plan 10-05) can look it up by run_id.
    let run_control = crate::scheduler::control::RunControl::new(cancel.clone());

    // 1d. Insert the merged RunEntry (D-01). Atomic single-statement write
    // (10-RESEARCH.md §Architecture §1 Invariant 2): the .write().await guard
    // is held only across the .insert() call — never across any subsequent
    // await — so concurrent .read().await in sse.rs cannot deadlock.
    active_runs.write().await.insert(
        run_id,
        crate::scheduler::RunEntry {
            broadcast_tx: broadcast_tx.clone(),
            control: run_control.clone(),
            job_name: job.name.clone(),
        },
    );

    // 2. Create log channel.
    let (sender, receiver) = log_pipeline::channel(DEFAULT_CHANNEL_CAPACITY);

    // 3. Spawn log writer task.
    let writer_pool = pool.clone();
    let writer_handle = tokio::spawn(log_writer_task(
        writer_pool,
        run_id,
        receiver,
        broadcast_tx.clone(),
    ));

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
                Some(cmd) => {
                    command::execute_command(&cmd, timeout, cancel, sender.clone(), &run_control)
                        .await
                }
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
                        &run_control,
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
                    &run_control,
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
        RunStatus::Stopped => "stopped",
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

    // 7b. Record Prometheus metrics (OPS-02, D-07).
    let duration_secs = start.elapsed().as_secs_f64();
    metrics::counter!("cronduit_runs_total", "job" => job.name.clone(), "status" => status_str.to_string()).increment(1);
    metrics::histogram!("cronduit_run_duration_seconds", "job" => job.name.clone())
        .record(duration_secs);
    // D-10 / Pitfall 1: operator-stopped runs must NOT count as failures.
    // The "stopped" status is canonical per finalize_run's mapping above.
    if status_str != "success" && status_str != "stopped" {
        let reason = classify_failure_reason(status_str, exec_result.error_message.as_deref());
        metrics::counter!("cronduit_run_failures_total", "job" => job.name.clone(), "reason" => reason.as_label().to_string()).increment(1);
    }

    // 7c. Phase 11 D-10: broadcast the __run_finished__ sentinel BEFORE the
    // broadcast sender is dropped so every live SSE subscriber receives a
    // graceful terminal frame (`event: run_finished`) and can swap the
    // running log pane to the static partial (Plan 11-11). Ordering
    // (RESEARCH.md §P10): (1) writer_handle already awaited at step 6 → every
    // persisted log_line has been broadcast with `id: Some(n)`; (2)
    // finalize_run DB update has run at step 7; (3) send the sentinel now;
    // (4) remove the run from active_runs so the next Run Now finds a clean
    // slate; (5) drop broadcast_tx — the `RecvError::Closed` arm in sse.rs
    // stays as the abrupt-disconnect fallback (emits `run_complete`) and is
    // only reached if a subscriber somehow misses the sentinel.
    //
    // `stream = "__run_finished__"` is the pattern-match key; `line =
    // run_id.to_string()` carries the payload the SSE handler serializes as
    // `{"run_id": N}`. `id = None` because the sentinel is not a persisted
    // `job_logs` row — the client's dedupe cursor (Plan 11-14) ignores
    // frames without `id:`, which is the correct semantics here. A
    // `SendError` (no live subscribers) is intentionally discarded.
    let _ = broadcast_tx.send(LogLine {
        stream: "__run_finished__".to_string(),
        ts: chrono::Utc::now().to_rfc3339(),
        line: run_id.to_string(),
        id: None,
    });

    // 7d. (Phase 15 / WH-02 / D-04 + D-05) Emit RunFinalized event for the
    // webhook delivery worker. NEVER use the awaiting `send().await` form
    // on this Sender — that would block the scheduler loop on a slow
    // receiver (Pitfall 28). try_send returns immediately; on full queue
    // we drop with a warn log + counter increment (D-04) so scheduler
    // timing is preserved.
    //
    // started_at is recovered from the monotonic `start` Instant by
    // subtracting from now: finished_at = Utc::now(); started_at =
    // finished_at - chrono::Duration::from_std(start.elapsed()).
    let finished_at = chrono::Utc::now();
    let started_at = finished_at
        - chrono::Duration::from_std(start.elapsed())
            .unwrap_or_else(|_| chrono::Duration::zero());
    let event = crate::webhooks::RunFinalized {
        run_id,
        job_id: job.id,
        job_name: job.name.clone(),
        status: status_str.to_string(),
        exit_code: exec_result.exit_code,
        started_at,
        finished_at,
    };
    match webhook_tx.try_send(event) {
        Ok(()) => {}
        Err(tokio::sync::mpsc::error::TrySendError::Full(dropped)) => {
            tracing::warn!(
                target: "cronduit.webhooks",
                run_id = dropped.run_id,
                job_id = dropped.job_id,
                status = %dropped.status,
                "webhook delivery channel saturated — event dropped"
            );
            metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(1);
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
            // Worker has gone away — only happens during shutdown. P20 will
            // add drain accounting (WH-10); P15 logs once per occurrence.
            tracing::error!(
                target: "cronduit.webhooks",
                run_id,
                job_id = job.id,
                "webhook delivery channel closed — worker is gone"
            );
        }
    }

    // 7e. (renumbered from 7d in Phase 15) Remove broadcast sender so SSE
    // subscribers get RecvError::Closed (UI-14, D-02).
    active_runs.write().await.remove(&run_id);
    drop(broadcast_tx);

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

/// Classify a run failure into one of the 6 FailureReason variants (D-05).
///
/// Maps error_message strings from docker_preflight, docker_pull, and docker_orphan
/// to the closed enum. Unknown errors default to `Unknown`.
fn classify_failure_reason(status: &str, error_msg: Option<&str>) -> FailureReason {
    match status {
        "timeout" => FailureReason::Timeout,
        "failed" => FailureReason::ExitNonzero,
        "error" => match error_msg {
            Some(msg) if msg.starts_with("network_target_unavailable:") => {
                FailureReason::NetworkTargetUnavailable
            }
            Some(msg) if msg.starts_with("image pull failed:") => FailureReason::ImagePullFailed,
            Some("orphaned at restart") => FailureReason::Abandoned,
            _ => FailureReason::Unknown,
        },
        // "cancelled" (shutdown) and any other unexpected status
        _ => FailureReason::Unknown,
    }
}

/// Log writer task that drains log lines from the receiver in micro-batches,
/// inserts them into job_logs, then broadcasts each persisted line (with its
/// `job_logs.id` populated from `RETURNING id`) to the SSE channel (UI-14).
///
/// D-12: Micro-batch inserts of DEFAULT_BATCH_SIZE (64) lines per transaction.
///
/// Phase 11 D-01 / UI-20 (Option A): insert-then-broadcast. After
/// `insert_log_batch` returns `Vec<i64>`, we zip the ids with the input batch
/// (guaranteed equal length) and send each `LogLine { id: Some(id), ..line }`
/// through the broadcast channel. Subscribers never see a line that has not
/// yet been persisted; on insert error, we broadcast nothing (the operator
/// sees the failure via the `cronduit.log_writer` tracing target, and the
/// run will show no logs for that batch — the D-01 "never leak unpersisted
/// lines" choice).
async fn log_writer_task(
    pool: DbPool,
    run_id: i64,
    receiver: LogReceiver,
    broadcast_tx: tokio::sync::broadcast::Sender<LogLine>,
) {
    loop {
        let batch = receiver.drain_batch_async(DEFAULT_BATCH_SIZE).await;
        if batch.is_empty() {
            // Channel closed and fully drained.
            break;
        }
        // Phase 11 D-01 / UI-20: persist FIRST, then broadcast with the
        // `RETURNING id` values zipped onto each line so subscribers can
        // dedupe on a stable, monotonic identifier.
        let tuples: Vec<(String, String, String)> = batch
            .iter()
            .map(|l| (l.stream.clone(), l.ts.clone(), l.line.clone()))
            .collect();
        match insert_log_batch(&pool, run_id, &tuples).await {
            Ok(ids) => {
                // SQLite INTEGER PRIMARY KEY and Postgres BIGSERIAL both
                // preserve insert order inside a single tx, so zipping the
                // returned `Vec<i64>` with `batch` in input order assigns
                // each line its own persisted id.
                for (line, id) in batch.into_iter().zip(ids.into_iter()) {
                    let _ = broadcast_tx.send(LogLine {
                        id: Some(id),
                        ..line
                    });
                }
            }
            Err(e) => {
                tracing::error!(
                    target: "cronduit.log_writer",
                    run_id,
                    error = %e,
                    "failed to insert log batch"
                );
                // Subscribers never see unpersisted lines (D-01 lock).
            }
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

    fn test_active_runs() -> Arc<RwLock<HashMap<i64, crate::scheduler::RunEntry>>> {
        Arc::new(RwLock::new(HashMap::new()))
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
            enabled_override: None,
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
        // Phase 15 / WH-02 — per-test webhook channel; receiver dropped.
        let (webhook_tx_test, _webhook_rx_test) = crate::webhooks::channel_with_capacity(8);
        let result = run_job(
            pool.clone(),
            None,
            job,
            "scheduled".to_string(),
            cancel,
            test_active_runs(),
            webhook_tx_test,
        )
        .await;

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
        // Phase 15 / WH-02 — per-test webhook channel; receiver dropped.
        let (webhook_tx_test, _webhook_rx_test) = crate::webhooks::channel_with_capacity(8);
        let result = run_job(
            pool.clone(),
            None,
            job,
            "scheduled".to_string(),
            cancel,
            test_active_runs(),
            webhook_tx_test,
        )
        .await;

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
        // Phase 15 / WH-02 — per-test webhook channel; receiver dropped.
        let (webhook_tx_test, _webhook_rx_test) = crate::webhooks::channel_with_capacity(8);
        let result = run_job(
            pool.clone(),
            None,
            job_with_short_timeout,
            "scheduled".to_string(),
            cancel,
            test_active_runs(),
            webhook_tx_test,
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

    /// Phase 11 Plan 11-06 (UI-19 fix): `run_job_with_existing_run_id` skips the
    /// `insert_running_run` step because the API handler thread already inserted
    /// the row. Test pre-inserts a row, calls the new fn, and asserts:
    ///   1. No duplicate row was created (exactly ONE row exists for the job).
    ///   2. The returned `run_id` matches the pre-inserted id.
    ///   3. The row's status is finalized (not "running") — proving the
    ///      lifecycle after insert still runs (log capture + finalize).
    #[tokio::test]
    async fn run_job_with_existing_run_id_skips_insert() {
        let pool = setup_pool().await;
        let job = insert_test_job(
            &pool,
            "skip-insert-job",
            "command",
            r#"{"command":"echo pre-inserted"}"#,
        )
        .await;

        // Pre-insert a row on behalf of the "API handler".
        let pre_run_id = crate::db::queries::insert_running_run(&pool, job.id, "manual")
            .await
            .unwrap();

        let cancel = CancellationToken::new();
        // Phase 15 / WH-02 — per-test webhook channel; receiver dropped.
        let (webhook_tx_test, _webhook_rx_test) = crate::webhooks::channel_with_capacity(8);
        let result = run_job_with_existing_run_id(
            pool.clone(),
            None,
            job.clone(),
            pre_run_id,
            cancel,
            test_active_runs(),
            webhook_tx_test,
        )
        .await;

        assert_eq!(
            result.run_id, pre_run_id,
            "run_id must be the pre-inserted id (not a new row)"
        );
        assert_eq!(result.status, "success");

        // Exactly one row must exist for this job.
        match pool.reader() {
            PoolRef::Sqlite(p) => {
                let row = sqlx::query("SELECT COUNT(*) as cnt FROM job_runs WHERE job_id = ?1")
                    .bind(job.id)
                    .fetch_one(p)
                    .await
                    .unwrap();
                let cnt: i64 = row.get("cnt");
                assert_eq!(
                    cnt, 1,
                    "run_job_with_existing_run_id must NOT create a second row"
                );

                // Status must be finalized.
                let r = sqlx::query("SELECT status FROM job_runs WHERE id = ?1")
                    .bind(pre_run_id)
                    .fetch_one(p)
                    .await
                    .unwrap();
                let status: String = r.get("status");
                assert_eq!(status, "success", "row must be finalized after run");
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

        let active1 = test_active_runs();
        let active2 = test_active_runs();
        // Phase 15 / WH-02 — per-test webhook channels; receivers dropped.
        let (webhook_tx_test1, _webhook_rx_test1) = crate::webhooks::channel_with_capacity(8);
        let (webhook_tx_test2, _webhook_rx_test2) = crate::webhooks::channel_with_capacity(8);
        let (r1, r2) = tokio::join!(
            run_job(
                pool1,
                None,
                job1,
                "scheduled".to_string(),
                cancel1,
                active1,
                webhook_tx_test1
            ),
            run_job(
                pool2,
                None,
                job2,
                "scheduled".to_string(),
                cancel2,
                active2,
                webhook_tx_test2
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
    }
}
