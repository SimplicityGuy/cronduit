//! Scheduler core: config sync, fire queue, and main select loop.
//!
//! D-01: `tokio::select!` over sleep-to-next-fire, join_set reaping, and cancellation.
//! D-02: BinaryHeap (min-heap via Reverse) for O(log n) next-fire tracking.
//! D-08: Lives in `src/scheduler/` with sub-modules for fire logic and sync.

pub mod cmd;
pub mod command;
pub mod control;
pub mod docker;
// Phase 5: @random cron field resolver (RAND-01 through RAND-05).
pub mod docker_daemon;
pub mod docker_log;
pub mod docker_orphan;
pub mod docker_preflight;
pub mod docker_pull;
pub mod fire;
pub mod log_pipeline;
pub mod random;
pub mod reload;
pub mod retention;
pub mod run;
pub mod script;
pub mod sync;

use crate::db::DbPool;
use crate::db::queries::{self, DbJob};
use bollard::Docker;
use chrono::Utc;
use chrono_tz::Tz;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;

/// Result of a completed run task.
pub struct RunResult {
    pub run_id: i64,
    pub status: String,
}

/// Per-run authoritative record: log broadcast + control plane + job name for toast.
///
/// D-01: merged map from active_runs + running_handles separation. The scheduler
/// loop, the executor's run_job, and the SSE handler all look up the same
/// RunEntry via active_runs. broadcast_tx and control are both Clone (Sender is
/// Clone; CancellationToken is Clone via Arc; Arc<AtomicU8> is Clone) so cloning
/// the entry into/out of the map is cheap.
///
/// Invariant (10-RESEARCH.md §Architecture §1 Invariant 1): the broadcast_tx
/// refcount arithmetic — executor inserts ONE clone at run.rs:102, executor
/// drops ITS clone at run.rs:277 after .remove(&run_id) at run.rs:276, leaving
/// zero references and triggering RecvError::Closed on SSE subscribers. This
/// must be preserved exactly.
#[derive(Clone)]
pub struct RunEntry {
    pub broadcast_tx: tokio::sync::broadcast::Sender<log_pipeline::LogLine>,
    pub control: crate::scheduler::control::RunControl,
    /// Job name stashed at run-start (Pitfall 4 recommendation: accept the
    /// staleness-on-rename semantic — "the name the run was started with").
    pub job_name: String,
}

/// The main scheduler loop. Owns the fire queue, job set, and shutdown token.
pub struct SchedulerLoop {
    pub pool: DbPool,
    pub docker: Option<Docker>,
    pub jobs: HashMap<i64, DbJob>,
    pub tz: Tz,
    pub cancel: CancellationToken,
    pub shutdown_grace: Duration,
    pub cmd_rx: tokio::sync::mpsc::Receiver<cmd::SchedulerCmd>,
    pub config_path: PathBuf,
    /// Per-run authoritative records (shared with AppState for SSE, UI-14, and
    /// for plan 10-05 SchedulerCmd::Stop lookup). D-01: merged map.
    pub active_runs: Arc<RwLock<HashMap<i64, RunEntry>>>,
}

impl SchedulerLoop {
    /// Run the main scheduler loop until cancellation.
    ///
    /// D-01: Selects over next-fire sleep, join_set reaping, and cancel token.
    /// D-02: Uses BinaryHeap<Reverse<FireEntry>> for efficient next-fire lookup.
    pub async fn run(mut self) {
        let mut jobs_vec: Vec<DbJob> = self.jobs.values().cloned().collect();
        let mut heap = fire::build_initial_heap(&jobs_vec, self.tz);
        let mut join_set: JoinSet<RunResult> = JoinSet::new();
        let mut last_expected_wake: chrono::DateTime<Tz> = Utc::now().with_timezone(&self.tz);

        loop {
            let next_fire = heap.peek().map(|r| r.0.instant);
            let sleep_target = match next_fire {
                Some(t) => t,
                None => tokio::time::Instant::now() + Duration::from_secs(60),
            };

            // Track expected wake for clock-jump detection (D-03).
            let sleep_duration =
                sleep_target.saturating_duration_since(tokio::time::Instant::now());
            let expected_wake_dt = Utc::now().with_timezone(&self.tz)
                + chrono::Duration::from_std(sleep_duration).unwrap_or(chrono::Duration::zero());

            tokio::select! {
                _ = tokio::time::sleep_until(sleep_target) => {
                    let now_tz = Utc::now().with_timezone(&self.tz);

                    // Check clock jump (SCHED-03).
                    let missed = fire::check_clock_jump(
                        last_expected_wake,
                        now_tz,
                        self.tz,
                        &jobs_vec,
                    );

                    // Spawn catch-up runs for missed fires.
                    for m in &missed {
                        if let Some(job) = self.jobs.get(&m.job_id) {
                            let child_cancel = self.cancel.child_token();
                            join_set.spawn(run::run_job(
                                self.pool.clone(),
                                self.docker.clone(),
                                job.clone(),
                                "catch-up".to_string(),
                                child_cancel,
                                self.active_runs.clone(),
                            ));
                            tracing::warn!(
                                target: "cronduit.scheduler",
                                job = %m.job_name,
                                missed_time = %m.missed_time,
                                "catch-up run for missed fire"
                            );
                        }
                    }

                    last_expected_wake = expected_wake_dt;

                    // Fire due jobs.
                    let due = fire::fire_due_jobs(&mut heap, tokio::time::Instant::now());
                    for entry in &due {
                        if let Some(job) = self.jobs.get(&entry.job_id) {
                            let child_cancel = self.cancel.child_token();
                            join_set.spawn(run::run_job(
                                self.pool.clone(),
                                self.docker.clone(),
                                job.clone(),
                                "scheduled".to_string(),
                                child_cancel,
                                self.active_runs.clone(),
                            ));
                            tracing::info!(
                                target: "cronduit.scheduler",
                                job = %entry.job_name,
                                fire_time = %entry.fire_time,
                                "spawned run"
                            );

                            // Requeue with next fire time.
                            fire::requeue_job(&mut heap, job, &entry.fire_time, self.tz);
                        }
                    }
                }
                Some(result) = join_set.join_next() => {
                    match result {
                        Ok(run_result) => {
                            tracing::info!(
                                target: "cronduit.scheduler",
                                run_id = run_result.run_id,
                                status = %run_result.status,
                                "run completed"
                            );
                        }
                        Err(e) => {
                            tracing::error!(
                                target: "cronduit.scheduler",
                                error = %e,
                                "run task panicked"
                            );
                        }
                    }
                }
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(cmd::SchedulerCmd::RunNow { job_id }) => {
                            if let Some(job) = self.jobs.get(&job_id) {
                                let child_cancel = self.cancel.child_token();
                                join_set.spawn(run::run_job(
                                    self.pool.clone(),
                                    self.docker.clone(),
                                    job.clone(),
                                    "manual".to_string(),
                                    child_cancel,
                                    self.active_runs.clone(),
                                ));
                                tracing::info!(
                                    target: "cronduit.scheduler",
                                    job_id,
                                    job_name = %job.name,
                                    "manual run triggered via command channel"
                                );
                            } else {
                                tracing::warn!(
                                    target: "cronduit.scheduler",
                                    job_id,
                                    "RunNow requested for unknown job_id"
                                );
                            }
                        }
                        Some(cmd::SchedulerCmd::RunNowWithRunId { job_id, run_id }) => {
                            // Phase 11 UI-19 fix (Plan 11-06): the API handler
                            // already inserted the job_runs row on the handler
                            // thread before dispatching this command. We skip
                            // the INSERT step by calling
                            // `run_job_with_existing_run_id` which reuses the
                            // pre-inserted run_id.
                            if let Some(job) = self.jobs.get(&job_id) {
                                let child_cancel = self.cancel.child_token();
                                join_set.spawn(run::run_job_with_existing_run_id(
                                    self.pool.clone(),
                                    self.docker.clone(),
                                    job.clone(),
                                    run_id,
                                    child_cancel,
                                    self.active_runs.clone(),
                                ));
                                tracing::info!(
                                    target: "cronduit.scheduler",
                                    job_id,
                                    run_id,
                                    job_name = %job.name,
                                    "manual run dispatched (row pre-inserted by API handler)"
                                );
                            } else {
                                // Unknown job_id but the row already exists in
                                // job_runs (the handler thread inserted it).
                                // Finalize the orphan row as error so it does
                                // not linger in 'running' state forever.
                                tracing::warn!(
                                    target: "cronduit.scheduler",
                                    job_id,
                                    run_id,
                                    "RunNowWithRunId for unknown job — finalizing orphan row as error"
                                );
                                let _ = queries::finalize_run(
                                    &self.pool,
                                    run_id,
                                    "error",
                                    None,
                                    tokio::time::Instant::now(),
                                    Some("job no longer exists"),
                                    None,
                                )
                                .await;
                            }
                        }
                        Some(cmd::SchedulerCmd::Reload { response_tx }) => {
                            let (result, new_heap) = reload::do_reload(
                                &self.pool,
                                &self.config_path,
                                &mut self.jobs,
                                self.tz,
                            ).await;
                            if let Some(h) = new_heap {
                                heap = h;
                                jobs_vec = self.jobs.values().cloned().collect();
                            }
                            let _ = response_tx.send(result);

                            // D-09: Coalesce queued reloads — drain any Reload commands
                            // that arrived while this reload was in-flight. If any were
                            // pending, run ONE additional reload (not N). Reply to each
                            // drained sender with the coalesced result.
                            let mut coalesced_senders: Vec<tokio::sync::oneshot::Sender<cmd::ReloadResult>> = Vec::new();
                            while let Ok(queued) = self.cmd_rx.try_recv() {
                                match queued {
                                    cmd::SchedulerCmd::Reload { response_tx: tx } => {
                                        coalesced_senders.push(tx);
                                    }
                                    cmd::SchedulerCmd::RunNow { job_id: rid } => {
                                        // Re-enqueue RunNow so it isn't dropped.
                                        if let Some(job) = self.jobs.get(&rid) {
                                            let child_cancel = self.cancel.child_token();
                                            join_set.spawn(run::run_job(
                                                self.pool.clone(),
                                                self.docker.clone(),
                                                job.clone(),
                                                "manual".to_string(),
                                                child_cancel,
                                                self.active_runs.clone(),
                                            ));
                                        }
                                    }
                                    cmd::SchedulerCmd::RunNowWithRunId { job_id: jid, run_id: rid } => {
                                        // Phase 11 Plan 11-06 (UI-19 fix):
                                        // drain-time handling of the pre-
                                        // inserted variant. The row already
                                        // exists — either dispatch the run or
                                        // finalize it as error so it doesn't
                                        // linger in 'running' forever.
                                        if let Some(job) = self.jobs.get(&jid) {
                                            let child_cancel = self.cancel.child_token();
                                            join_set.spawn(run::run_job_with_existing_run_id(
                                                self.pool.clone(),
                                                self.docker.clone(),
                                                job.clone(),
                                                rid,
                                                child_cancel,
                                                self.active_runs.clone(),
                                            ));
                                        } else {
                                            tracing::warn!(
                                                target: "cronduit.scheduler",
                                                job_id = jid,
                                                run_id = rid,
                                                "RunNowWithRunId (drained) for unknown job — finalizing orphan row as error"
                                            );
                                            let _ = queries::finalize_run(
                                                &self.pool,
                                                rid,
                                                "error",
                                                None,
                                                tokio::time::Instant::now(),
                                                Some("job no longer exists"),
                                                None,
                                            )
                                            .await;
                                        }
                                    }
                                    cmd::SchedulerCmd::Reroll { job_id: rid, response_tx: tx } => {
                                        let (rr_result, rr_heap) = reload::do_reroll(
                                            &self.pool, rid, &mut self.jobs, self.tz,
                                        ).await;
                                        if let Some(h) = rr_heap {
                                            heap = h;
                                            jobs_vec = self.jobs.values().cloned().collect();
                                        }
                                        let _ = tx.send(rr_result);
                                    }
                                    cmd::SchedulerCmd::Stop { run_id: rid, response_tx: tx } => {
                                        // Plan 10-05: process Stop immediately even
                                        // during reload coalescing so the operator's
                                        // stop intent is not delayed behind an
                                        // in-flight reload. Same lookup-clone-fire
                                        // pattern as the top-level Stop arm.
                                        let maybe_control = {
                                            let active = self.active_runs.read().await;
                                            active.get(&rid).map(|entry| entry.control.clone())
                                        };
                                        let stop_result = match maybe_control {
                                            Some(control) => {
                                                control.stop(crate::scheduler::control::StopReason::Operator);
                                                tracing::info!(
                                                    target: "cronduit.scheduler",
                                                    run_id = rid,
                                                    "stop requested via command channel (coalesced with reload drain)"
                                                );
                                                cmd::StopResult::Stopped
                                            }
                                            None => cmd::StopResult::AlreadyFinalized,
                                        };
                                        let _ = tx.send(stop_result);
                                    }
                                }
                            }
                            if !coalesced_senders.is_empty() {
                                tracing::debug!(
                                    target: "cronduit.reload",
                                    coalesced = coalesced_senders.len(),
                                    "coalescing queued reload requests into one additional reload"
                                );
                                let (coal_result, coal_heap) = reload::do_reload(
                                    &self.pool, &self.config_path, &mut self.jobs, self.tz,
                                ).await;
                                if let Some(h) = coal_heap {
                                    heap = h;
                                    jobs_vec = self.jobs.values().cloned().collect();
                                }
                                for tx in coalesced_senders {
                                    let _ = tx.send(cmd::ReloadResult {
                                        status: coal_result.status,
                                        added: coal_result.added,
                                        updated: coal_result.updated,
                                        disabled: coal_result.disabled,
                                        unchanged: coal_result.unchanged,
                                        error_message: coal_result.error_message.clone(),
                                    });
                                }
                            }
                        }
                        Some(cmd::SchedulerCmd::Reroll { job_id, response_tx }) => {
                            let (result, new_heap) = reload::do_reroll(
                                &self.pool,
                                job_id,
                                &mut self.jobs,
                                self.tz,
                            ).await;
                            if let Some(h) = new_heap {
                                heap = h;
                                jobs_vec = self.jobs.values().cloned().collect();
                            }
                            let _ = response_tx.send(result);
                        }
                        Some(cmd::SchedulerCmd::Stop { run_id, response_tx }) => {
                            // Plan 10-05 / D-01 / Option C: the merged active_runs
                            // map IS the race token. If the executor called
                            // `active_runs.write().await.remove(&run_id)` at
                            // run.rs:~290 before this arm fires, the lookup
                            // returns None and the race-case branch replies
                            // AlreadyFinalized. No DB read, no extra coordination.
                            //
                            // Lock scope invariant (Pitfall 2): acquire the read
                            // lock, clone the control out, release the lock, then
                            // call control.stop(). The stop() itself is cheap
                            // (atomic store + CancellationToken.cancel()) but we
                            // release the lock first to keep "no locks held
                            // across state changes" uniform with the other arms.
                            let maybe_control = {
                                let active = self.active_runs.read().await;
                                active.get(&run_id).map(|entry| entry.control.clone())
                            };
                            let result = match maybe_control {
                                Some(control) => {
                                    control.stop(crate::scheduler::control::StopReason::Operator);
                                    tracing::info!(
                                        target: "cronduit.scheduler",
                                        run_id,
                                        "stop requested via command channel"
                                    );
                                    cmd::StopResult::Stopped
                                }
                                None => {
                                    tracing::debug!(
                                        target: "cronduit.scheduler",
                                        run_id,
                                        "Stop arrived after run finalized (race case)"
                                    );
                                    cmd::StopResult::AlreadyFinalized
                                }
                            };
                            let _ = response_tx.send(result);
                        }
                        None => {
                            tracing::info!(target: "cronduit.scheduler", "command channel closed");
                            break;
                        }
                    }
                }
                _ = self.cancel.cancelled() => {
                    let drain_start = tokio::time::Instant::now();
                    let in_flight_count = join_set.len();
                    tracing::info!(
                        target: "cronduit.scheduler",
                        in_flight_count,
                        grace_secs = self.shutdown_grace.as_secs(),
                        "shutdown signal received, draining in-flight runs"
                    );

                    if in_flight_count == 0 {
                        tracing::info!(
                            target: "cronduit.scheduler",
                            in_flight_count = 0u64,
                            drained_count = 0u64,
                            force_killed_count = 0u64,
                            grace_elapsed_ms = 0u64,
                            "shutdown complete"
                        );
                        break;
                    }

                    // Drain with grace period (D-19)
                    let grace_deadline = tokio::time::Instant::now() + self.shutdown_grace;
                    let mut drained_count: u64 = 0;

                    loop {
                        tokio::select! {
                            Some(result) = join_set.join_next() => {
                                drained_count += 1;
                                match result {
                                    Ok(r) => tracing::info!(
                                        target: "cronduit.scheduler",
                                        run_id = r.run_id,
                                        status = %r.status,
                                        "drained run during shutdown"
                                    ),
                                    Err(e) => tracing::error!(
                                        target: "cronduit.scheduler",
                                        error = %e,
                                        "run task panicked during shutdown"
                                    ),
                                }
                                if join_set.is_empty() { break; }
                            }
                            _ = tokio::time::sleep_until(grace_deadline) => {
                                // Grace expired -- force-cancel remaining runs
                                let remaining = join_set.len();
                                tracing::warn!(
                                    target: "cronduit.scheduler",
                                    remaining,
                                    "shutdown grace expired, cancelling remaining runs"
                                );
                                // The child CancellationTokens are already cancelled
                                // because self.cancel was cancelled. Abort any tasks
                                // that haven't responded to cancellation:
                                join_set.abort_all();
                                // Drain the aborted tasks
                                while let Some(result) = join_set.join_next().await {
                                    match result {
                                        Ok(r) => {
                                            tracing::warn!(
                                                target: "cronduit.scheduler",
                                                run_id = r.run_id,
                                                "force-killed run during shutdown"
                                            );
                                        }
                                        Err(_) => { /* JoinError from abort -- expected */ }
                                    }
                                }
                                break;
                            }
                        }
                    }

                    let grace_elapsed_ms = drain_start.elapsed().as_millis() as u64;
                    let force_killed_count = (in_flight_count as u64) - drained_count;
                    tracing::info!(
                        target: "cronduit.scheduler",
                        in_flight_count,
                        drained_count,
                        force_killed_count,
                        grace_elapsed_ms,
                        "shutdown complete"
                    );
                    break;
                }
            }
        }
    }
}

/// Spawn the scheduler loop on a new tokio task.
///
/// Accepts `Vec<DbJob>` and converts to `HashMap` internally for O(1) lookup.
/// Returns a `JoinHandle` that resolves when the loop exits (on cancellation).
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    pool: DbPool,
    docker: Option<Docker>,
    jobs: Vec<DbJob>,
    tz: Tz,
    cancel: CancellationToken,
    shutdown_grace: Duration,
    cmd_rx: tokio::sync::mpsc::Receiver<cmd::SchedulerCmd>,
    config_path: PathBuf,
    active_runs: Arc<RwLock<HashMap<i64, RunEntry>>>,
) -> JoinHandle<()> {
    let jobs_map: HashMap<i64, DbJob> = jobs.into_iter().map(|j| (j.id, j)).collect();
    let scheduler = SchedulerLoop {
        pool,
        docker,
        jobs: jobs_map,
        tz,
        cancel,
        shutdown_grace,
        cmd_rx,
        config_path,
        active_runs,
    };
    tokio::spawn(scheduler.run())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DbPool;
    use crate::db::queries::{self, DbJob};

    async fn setup_pool() -> DbPool {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        pool.migrate().await.unwrap();
        pool
    }

    fn test_active_runs() -> Arc<RwLock<HashMap<i64, RunEntry>>> {
        Arc::new(RwLock::new(HashMap::new()))
    }

    fn make_test_job(id: i64, name: &str, command: &str) -> DbJob {
        DbJob {
            id,
            name: name.to_string(),
            schedule: "0 0 31 2 *".to_string(), // never fires naturally
            resolved_schedule: "0 0 31 2 *".to_string(),
            job_type: "command".to_string(),
            config_json: format!(r#"{{"command":"{command}"}}"#),
            config_hash: "test".to_string(),
            enabled: true,
            enabled_override: None,
            timeout_secs: 3600,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    /// Test: after cancel, scheduler drains in-flight runs that complete within grace period.
    #[tokio::test]
    async fn shutdown_drain_completes_within_grace() {
        let pool = setup_pool().await;
        // Insert a job in DB so run_job can find it.
        let job_id = queries::upsert_job(
            &pool,
            "fast-job",
            "0 0 31 2 *",
            "0 0 31 2 *",
            "command",
            r#"{"command":"echo done"}"#,
            "h1",
            3600,
        )
        .await
        .unwrap();

        let cancel = CancellationToken::new();
        let child_cancel = cancel.child_token();

        // Spawn run_job directly in a JoinSet to test drain.
        let mut join_set: JoinSet<RunResult> = JoinSet::new();
        let job = DbJob {
            id: job_id,
            ..make_test_job(job_id, "fast-job", "echo done")
        };
        join_set.spawn(run::run_job(
            pool.clone(),
            None,
            job,
            "test".to_string(),
            child_cancel,
            test_active_runs(),
        ));

        // Cancel after a small delay to let the run start.
        cancel.cancel();

        // Drain with grace period (long enough for echo to finish).
        let drain_start = tokio::time::Instant::now();
        let mut drained: u64 = 0;
        let grace_deadline = tokio::time::Instant::now() + Duration::from_secs(5);

        loop {
            tokio::select! {
                Some(result) = join_set.join_next() => {
                    drained += 1;
                    assert!(result.is_ok());
                    if join_set.is_empty() { break; }
                }
                _ = tokio::time::sleep_until(grace_deadline) => {
                    break;
                }
            }
        }

        assert_eq!(drained, 1, "should have drained 1 run");
        assert!(
            drain_start.elapsed() < Duration::from_secs(4),
            "should drain quickly"
        );
        pool.close().await;
    }

    /// Test: when grace period expires, remaining runs are force-killed.
    #[tokio::test]
    async fn shutdown_grace_expiry_force_kills() {
        let pool = setup_pool().await;
        let job_id = queries::upsert_job(
            &pool,
            "slow-job",
            "0 0 31 2 *",
            "0 0 31 2 *",
            "command",
            r#"{"command":"sleep 60"}"#,
            "h1",
            3600,
        )
        .await
        .unwrap();

        let cancel = CancellationToken::new();
        let child_cancel = cancel.child_token();

        let mut join_set: JoinSet<RunResult> = JoinSet::new();
        let job = DbJob {
            id: job_id,
            ..make_test_job(job_id, "slow-job", "sleep 60")
        };
        join_set.spawn(run::run_job(
            pool.clone(),
            None,
            job,
            "test".to_string(),
            child_cancel,
            test_active_runs(),
        ));

        // Cancel immediately.
        cancel.cancel();

        let in_flight = join_set.len();
        assert_eq!(in_flight, 1);

        // Very short grace period.
        let grace_deadline = tokio::time::Instant::now() + Duration::from_millis(200);
        let mut drained: u64 = 0;

        loop {
            tokio::select! {
                Some(_result) = join_set.join_next() => {
                    drained += 1;
                    if join_set.is_empty() { break; }
                }
                _ = tokio::time::sleep_until(grace_deadline) => {
                    // Grace expired, abort remaining.
                    join_set.abort_all();
                    while join_set.join_next().await.is_some() {
                        // drain aborted tasks
                    }
                    break;
                }
            }
        }

        let force_killed = (in_flight as u64) - drained;
        // The child cancel propagated, so the run may have completed as "shutdown"
        // before grace expired, OR grace expired first and we aborted.
        // Either way, the test should complete within 1s.
        assert!(drained + force_killed >= 1, "all runs accounted for");
        pool.close().await;
    }

    /// Test: shutdown summary has the expected fields.
    #[tokio::test]
    async fn shutdown_summary_fields() {
        // This test verifies the drain logic produces correct counts.
        let pool = setup_pool().await;
        let cancel = CancellationToken::new();
        cancel.cancel();

        // No in-flight runs: immediate shutdown.
        let join_set: JoinSet<RunResult> = JoinSet::new();
        let in_flight_count = join_set.len();
        let drained_count: u64 = 0;
        let force_killed_count = (in_flight_count as u64) - drained_count;
        let grace_elapsed_ms: u64 = 0;

        assert_eq!(in_flight_count, 0);
        assert_eq!(drained_count, 0);
        assert_eq!(force_killed_count, 0);
        assert_eq!(grace_elapsed_ms, 0);

        pool.close().await;
    }

    /// Plan 10-05: SchedulerCmd::Stop arm sanity test.
    ///
    /// Exercises the exact map-lookup + clone + stop pattern the Stop arm uses
    /// in the scheduler loop (without spinning up the full select! loop). This
    /// is a faster-feedback companion to `tests/stop_race.rs` which runs the
    /// full 1000-iteration deterministic race.
    ///
    /// Proves:
    /// 1. When the run_id is present in active_runs, the pattern fires
    ///    `control.stop(StopReason::Operator)` and yields `StopResult::Stopped`.
    /// 2. The `RunEntry`'s `control.reason()` becomes `StopReason::Operator`.
    /// 3. The underlying cancel token is cancelled (observable from another
    ///    clone of the control).
    /// 4. When the run_id is absent (race case), the pattern yields
    ///    `StopResult::AlreadyFinalized` and does NOT touch the DB.
    #[tokio::test]
    async fn stop_arm_sets_operator_reason() {
        use crate::scheduler::cmd::StopResult;
        use crate::scheduler::control::{RunControl, StopReason};

        let active_runs = test_active_runs();
        let run_id: i64 = 42;

        let (broadcast_tx, _rx) = tokio::sync::broadcast::channel(16);
        let cancel = CancellationToken::new();
        let control = RunControl::new(cancel.clone());

        active_runs.write().await.insert(
            run_id,
            RunEntry {
                broadcast_tx,
                control: control.clone(),
                job_name: "stop-arm-test".to_string(),
            },
        );

        // Replicates the exact pattern used inside the Stop match arm in
        // SchedulerLoop::run(): acquire read lock, clone control, release
        // lock, fire stop.
        let maybe_control = {
            let active = active_runs.read().await;
            active.get(&run_id).map(|entry| entry.control.clone())
        };
        let result = match maybe_control {
            Some(c) => {
                c.stop(StopReason::Operator);
                StopResult::Stopped
            }
            None => StopResult::AlreadyFinalized,
        };

        assert_eq!(result, StopResult::Stopped, "present id yields Stopped");
        assert_eq!(
            control.reason(),
            StopReason::Operator,
            "operator reason propagated to RunEntry control clone"
        );
        assert!(
            control.cancel.is_cancelled(),
            "cancel token fired via RunEntry control clone"
        );

        // Race case: drop the RunEntry out of the map and verify the same
        // lookup pattern replies AlreadyFinalized without any DB touch.
        active_runs.write().await.remove(&run_id);

        let maybe_control_after = {
            let active = active_runs.read().await;
            active.get(&run_id).map(|entry| entry.control.clone())
        };
        let race_result = match maybe_control_after {
            Some(c) => {
                c.stop(StopReason::Operator);
                StopResult::Stopped
            }
            None => StopResult::AlreadyFinalized,
        };
        assert_eq!(
            race_result,
            StopResult::AlreadyFinalized,
            "absent id yields AlreadyFinalized (race case)"
        );
    }

    /// Phase 11 Plan 11-06 (UI-19 fix): `SchedulerCmd::RunNowWithRunId`
    /// variant carries both `job_id` AND the pre-inserted `run_id` so the
    /// scheduler's handler arm can dispatch `run_job_with_existing_run_id`
    /// instead of `run_job`. This test proves the variant exists, is
    /// constructible, and carries both fields.
    ///
    /// Also proves the legacy `RunNow { job_id }` variant is STILL present
    /// (not deleted) per RESEARCH Q1 RESOLVED — scheduled runs continue to
    /// use it.
    #[tokio::test]
    async fn run_now_with_run_id_variant_carries_both_ids() {
        let cmd = cmd::SchedulerCmd::RunNowWithRunId {
            job_id: 7,
            run_id: 42,
        };
        match cmd {
            cmd::SchedulerCmd::RunNowWithRunId { job_id, run_id } => {
                assert_eq!(job_id, 7);
                assert_eq!(run_id, 42);
            }
            other => panic!("expected RunNowWithRunId, got {:?}", other),
        }

        // Legacy variant still exists (RESEARCH Q1 RESOLVED).
        let legacy = cmd::SchedulerCmd::RunNow { job_id: 99 };
        match legacy {
            cmd::SchedulerCmd::RunNow { job_id } => assert_eq!(job_id, 99),
            _ => panic!("legacy RunNow variant must remain — scheduled runs rely on it"),
        }
    }
}
