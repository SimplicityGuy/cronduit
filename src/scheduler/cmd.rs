//! Scheduler command channel types.
//!
//! D-08: mpsc channel bridges web handlers to the scheduler loop.
//! D-09: Enum designed for extensibility -- Reload and Reroll added in Phase 5.

use tokio::sync::oneshot;

/// Commands that can be sent to the scheduler via the mpsc channel.
#[derive(Debug)]
pub enum SchedulerCmd {
    /// Trigger a manual run for a specific job (UI-12).
    RunNow { job_id: i64 },
    /// Hot-reload config from disk (RELOAD-01, RELOAD-03).
    Reload {
        response_tx: oneshot::Sender<ReloadResult>,
    },
    /// Re-resolve @random schedule for a specific job (RAND-04).
    Reroll {
        job_id: i64,
        response_tx: oneshot::Sender<ReloadResult>,
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
