//! Scheduler command channel types.
//!
//! D-08: mpsc channel bridges web handlers to the scheduler loop.
//! D-09: Enum designed for extensibility -- Reload and Reroll added in Phase 5.
//! Phase 10: Stop variant added to carry SCHED-09 / SCHED-10 operator-stop
//! requests from the web handler into the scheduler loop.

use tokio::sync::oneshot;

/// Commands that can be sent to the scheduler via the mpsc channel.
#[derive(Debug)]
pub enum SchedulerCmd {
    /// Trigger a manual run for a specific job (UI-12).
    ///
    /// Dispatched by the cron-tick scheduler when a scheduled run fires
    /// (see `SchedulerLoop::run`'s fire-due-jobs path) and the legacy code
    /// paths that rely on the scheduler inserting the `job_runs` row
    /// itself via `run::run_job`. The scheduler handler arm calls
    /// `run_job(pool, docker, job, "manual", cancel, active_runs)` which
    /// performs the INSERT internally. Still used by the cron path after
    /// Phase 11 (RESEARCH Q1 RESOLVED).
    RunNow { job_id: i64 },
    /// Trigger a manual run where the `job_runs` row has ALREADY been
    /// inserted on the API handler thread (Phase 11 UI-19 fix).
    ///
    /// Dispatched by `src/web/handlers/api.rs::run_now` which now inserts
    /// the row synchronously before sending this command — the browser's
    /// immediate follow-up navigation to `/jobs/{job_id}/runs/{run_id}`
    /// therefore always finds an existing row (no 404, no transient
    /// "Unable to stream logs" flash). The scheduler handler arm calls
    /// `run_job_with_existing_run_id(...)` which SKIPS the INSERT step.
    RunNowWithRunId { job_id: i64, run_id: i64 },
    /// Hot-reload config from disk (RELOAD-01, RELOAD-03).
    Reload {
        response_tx: oneshot::Sender<ReloadResult>,
    },
    /// Re-resolve @random schedule for a specific job (RAND-04).
    Reroll {
        job_id: i64,
        response_tx: oneshot::Sender<ReloadResult>,
    },
    /// Stop an in-flight run by its run_id (SCHED-09, SCHED-10).
    ///
    /// The scheduler loop looks up the RunEntry in `active_runs`, clones the
    /// `RunControl`, releases the read lock, then calls
    /// `control.stop(StopReason::Operator)` which stores the reason and fires
    /// the cancel token. The executor's cancel arm observes the reason and
    /// finalizes the DB row as `"stopped"`. The scheduler replies via
    /// `oneshot` so the handler can distinguish the normal-stop toast path
    /// from the race-case silent-refresh path (D-07).
    Stop {
        run_id: i64,
        response_tx: oneshot::Sender<StopResult>,
    },
}

/// Whether the reload/reroll succeeded or failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadStatus {
    Ok,
    Error,
}

/// Result of a config reload or schedule reroll.
#[derive(Debug)]
pub struct ReloadResult {
    pub status: ReloadStatus,
    pub added: u64,
    pub updated: u64,
    pub disabled: u64,
    pub unchanged: u64,
    pub error_message: Option<String>,
}

/// Result of a scheduler `Stop` command. A small `Copy` enum because Stop has
/// no diff-summary payload to return (unlike Reload / Reroll which reports
/// changed job counts).
///
/// D-07 / 10-RESEARCH.md §Scheduler Stop arm: we intentionally collapse
/// "unknown run_id" and "already finalized" into a single `AlreadyFinalized`
/// variant — the handler's action is identical in both cases (200 +
/// `HX-Refresh` + no toast) and the refreshed page shows the truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopResult {
    /// Scheduler found the `RunEntry`, set `stop_reason = Operator`, and
    /// fired the cancel token. Handler renders a "Stopping..." toast.
    Stopped,
    /// `run_id` was not in `active_runs` — the run finalized naturally just
    /// before the Stop arrived, or the id was never active. Handler replies
    /// 200 + `HX-Refresh` with no toast.
    AlreadyFinalized,
}
