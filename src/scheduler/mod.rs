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

use crate::db::queries::DbJob;
use crate::db::DbPool;
use chrono::Utc;
use chrono_tz::Tz;
use std::time::Duration;
use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;

/// Placeholder for run completion results. Will be expanded in Plan 03.
pub struct RunResult {
    pub run_id: i64,
    pub status: String,
}

/// The main scheduler loop. Owns the fire queue, job set, and shutdown token.
pub struct SchedulerLoop {
    pub pool: DbPool,
    pub jobs: Vec<DbJob>,
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
        let mut heap = fire::build_initial_heap(&self.jobs, self.tz);
        let mut join_set: JoinSet<RunResult> = JoinSet::new();
        let mut last_expected_wake: chrono::DateTime<Tz> = Utc::now().with_timezone(&self.tz);

        loop {
            let next_fire = heap.peek().map(|r| r.0.instant);
            let sleep_target = match next_fire {
                Some(t) => t,
                None => tokio::time::Instant::now() + Duration::from_secs(60),
            };

            // Track expected wake for clock-jump detection (D-03).
            let sleep_duration = sleep_target.saturating_duration_since(tokio::time::Instant::now());
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
                        &self.jobs,
                    );
                    // TODO (Plan 03): Enqueue catch-up runs from missed fires.
                    let _ = &missed;

                    last_expected_wake = now_tz;

                    // Fire due jobs.
                    let due = fire::fire_due_jobs(&mut heap, tokio::time::Instant::now());
                    for entry in &due {
                        tracing::info!(
                            target: "cronduit.scheduler",
                            job = %entry.job_name,
                            fire_time = %entry.fire_time,
                            "firing job"
                        );
                        // TODO (Plan 03): Spawn run task into join_set.

                        // Requeue with next fire time.
                        if let Some(db_job) = self.jobs.iter().find(|j| j.id == entry.job_id) {
                            fire::requeue_job(&mut heap, db_job, &entry.fire_time, self.tz);
                        }
                    }
                }
                Some(result) = join_set.join_next() => {
                    // TODO (Plan 03): Handle completed run.
                    let _ = result;
                }
                _ = self.cancel.cancelled() => {
                    // TODO (Plan 04): Graceful shutdown drain.
                    tracing::info!(
                        target: "cronduit.scheduler",
                        "shutdown signal received, stopping scheduler"
                    );
                    break;
                }
            }
        }
    }
}

/// Spawn the scheduler loop on a new tokio task.
///
/// Returns a `JoinHandle` that resolves when the loop exits (on cancellation).
pub fn spawn(
    pool: DbPool,
    jobs: Vec<DbJob>,
    tz: Tz,
    cancel: CancellationToken,
    shutdown_grace: Duration,
) -> JoinHandle<()> {
    let scheduler = SchedulerLoop {
        pool,
        jobs,
        tz,
        cancel,
        shutdown_grace,
    };
    tokio::spawn(scheduler.run())
}
