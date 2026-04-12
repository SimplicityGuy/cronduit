//! Scheduler command channel types.
//!
//! D-08: mpsc channel bridges web handlers to the scheduler loop.
//! D-09: Enum designed for extensibility -- Reload and Reroll added in Phase 5.

/// Commands that can be sent to the scheduler via the mpsc channel.
#[derive(Debug)]
pub enum SchedulerCmd {
    /// Trigger a manual run for a specific job (UI-12).
    RunNow { job_id: i64 },
    /// Trigger a full config reload (RELOAD-01, RELOAD-03).
    Reload {
        response_tx: tokio::sync::oneshot::Sender<ReloadResult>,
    },
    /// Re-roll the @random schedule for a single job (D-06).
    Reroll {
        job_id: i64,
        response_tx: tokio::sync::oneshot::Sender<ReloadResult>,
    },
}

/// Status of a reload or reroll operation.
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
