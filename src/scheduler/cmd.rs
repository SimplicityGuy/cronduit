//! Scheduler command channel types.
//!
//! D-08: mpsc channel bridges web handlers to the scheduler loop.
//! D-09: Enum designed for extensibility (Phase 5 adds Reload).

/// Commands that can be sent to the scheduler via the mpsc channel.
#[derive(Debug)]
pub enum SchedulerCmd {
    /// Trigger a manual run for a specific job (UI-12).
    RunNow { job_id: i64 },
    // Phase 5 will add: Reload { response_tx: oneshot::Sender<Result<(), String>> }
}
