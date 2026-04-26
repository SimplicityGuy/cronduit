//! Channel-message contract for the webhook delivery worker.
//!
//! Phase 15 / WH-02 / D-02: self-contained minimum payload. Streak metrics,
//! image_digest, and config_hash come from P16's `get_failure_context` query
//! at delivery time inside the dispatcher — they are NOT carried on the
//! channel. This keeps the P15 message stable against P16's schema work.
//!
//! NOTE: this is the CHANNEL-MESSAGE contract, not the WIRE-FORMAT payload.
//! P18 / WH-03 introduces `src/webhooks/payload.rs` for the JSON wire format.

use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct RunFinalized {
    pub run_id: i64,
    pub job_id: i64,
    pub job_name: String,
    /// Canonical terminal status string. Matches `src/scheduler/run.rs`'s
    /// `status_str` mapping at L315-322:
    /// `"success" | "failed" | "timeout" | "cancelled" | "stopped" | "error"`.
    pub status: String,
    pub exit_code: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}
