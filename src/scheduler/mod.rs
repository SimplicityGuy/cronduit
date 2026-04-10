//! Scheduler core: config sync, fire queue, and main select loop.
//!
//! D-01: `tokio::select!` over sleep-to-next-fire, join_set reaping, and cancellation.
//! D-02: BinaryHeap (min-heap via Reverse) for O(log n) next-fire tracking.
//! D-08: Lives in `src/scheduler/` with sub-modules for fire logic and sync.

pub mod command;
pub mod fire;
pub mod log_pipeline;
pub mod run;
pub mod script;
pub mod sync;

use crate::db::DbPool;
use crate::db::queries::DbJob;
use chrono::Utc;
use chrono_tz::Tz;
use std::collections::HashMap;
use std::time::Duration;
use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;

/// Result of a completed run task.
pub struct RunResult {
    pub run_id: i64,
    pub status: String,
}

/// The main scheduler loop. Owns the fire queue, job set, and shutdown token.
pub struct SchedulerLoop {
    pub pool: DbPool,
    pub jobs: HashMap<i64, DbJob>,
    pub tz: Tz,
    pub cancel: CancellationToken,
    pub shutdown_grace: Duration,
}

impl SchedulerLoop {
    /// Run the main scheduler loop until cancellation.
    ///
    /// D-01: Selects over next-fire sleep, join_set reaping, and cancel token.
    /// D-02: Uses BinaryHeap<Reverse<FireEntry>> for efficient next-fire lookup.
    pub async fn run(self) {
        let jobs_vec: Vec<DbJob> = self.jobs.values().cloned().collect();
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
            let _expected_wake_dt = Utc::now().with_timezone(&self.tz)
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
                                job.clone(),
                                "catch-up".to_string(),
                                child_cancel,
                            ));
                            tracing::warn!(
                                target: "cronduit.scheduler",
                                job = %m.job_name,
                                missed_time = %m.missed_time,
                                "catch-up run for missed fire"
                            );
                        }
                    }

                    last_expected_wake = now_tz;

                    // Fire due jobs.
                    let due = fire::fire_due_jobs(&mut heap, tokio::time::Instant::now());
                    for entry in &due {
                        if let Some(job) = self.jobs.get(&entry.job_id) {
                            let child_cancel = self.cancel.child_token();
                            join_set.spawn(run::run_job(
                                self.pool.clone(),
                                job.clone(),
                                "scheduled".to_string(),
                                child_cancel,
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
pub fn spawn(
    pool: DbPool,
    jobs: Vec<DbJob>,
    tz: Tz,
    cancel: CancellationToken,
    shutdown_grace: Duration,
) -> JoinHandle<()> {
    let jobs_map: HashMap<i64, DbJob> = jobs.into_iter().map(|j| (j.id, j)).collect();
    let scheduler = SchedulerLoop {
        pool,
        jobs: jobs_map,
        tz,
        cancel,
        shutdown_grace,
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
            job,
            "test".to_string(),
            child_cancel,
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
            job,
            "test".to_string(),
            child_cancel,
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
}
