//! Scheduler command channel types.
//!
//! D-08: mpsc channel bridges web handlers to the scheduler loop.
//! D-09: Enum designed for extensibility (Phase 5 adds Reload + Reroll).

use tokio::sync::oneshot;

/// Commands that can be sent to the scheduler via the mpsc channel.
#[derive(Debug)]
pub enum SchedulerCmd {
    /// Trigger a manual run for a specific job (UI-12).
    RunNow { job_id: i64 },
    /// Hot-reload config from disk (RELOAD-01).
    Reload {
        response_tx: oneshot::Sender<ReloadResult>,
    },
    /// Re-resolve @random schedule for a specific job (RAND-04).
    Reroll {
        job_id: i64,
        response_tx: oneshot::Sender<ReloadResult>,
    },
}

/// Result of a reload or reroll operation.
#[derive(Debug)]
pub struct ReloadResult {
    pub status: ReloadStatus,
    pub added: u64,
    pub updated: u64,
    pub disabled: u64,
    pub unchanged: u64,
    pub error_message: Option<String>,
}

/// Whether the reload/reroll succeeded or failed.
#[derive(Debug, PartialEq)]
pub enum ReloadStatus {
    Ok,
    Error,
}
