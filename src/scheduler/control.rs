//! Per-run control plane: cancel token + stop reason (SCHED-10, D-09).
//!
//! `RunControl` bundles a `tokio_util::sync::CancellationToken` with an
//! `Arc<AtomicU8>` carrying the reason the token was fired. This is the
//! mechanism by which executor cancel arms distinguish an operator-initiated
//! Stop from a graceful Shutdown.
//!
//! Memory-ordering contract: `RunControl::stop(reason)` stores the reason
//! with `Ordering::SeqCst` **before** firing the cancel token. The executor's
//! cancel arm subsequently loads the reason (also SeqCst) **after**
//! `cancel.cancelled()` yields, so the happens-before relationship is
//! guaranteed by the SeqCst ordering of the two operations.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use tokio_util::sync::CancellationToken;

/// Why a run was cancelled. Encoded as `u8` so it can live in an `AtomicU8`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// Default cancel path — graceful shutdown of the whole scheduler. This
    /// is the reason any bare `CancellationToken::cancel()` will produce,
    /// because `RunControl::new` initializes the atomic to `Shutdown = 0`.
    Shutdown = 0,
    /// Operator-initiated stop via the UI / API Stop button. Set explicitly
    /// by `RunControl::stop(StopReason::Operator)`.
    Operator = 1,
}

impl StopReason {
    /// Decode a `u8` (as produced by the atomic load) back into a `StopReason`.
    /// Any unknown value collapses to `Shutdown` so executors never see
    /// undefined behavior if a new variant is added without the atomic being
    /// updated — they'll simply observe the default.
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Operator,
            _ => Self::Shutdown,
        }
    }
}

/// Per-run control: cancel token plus an atomic reason slot.
///
/// Cloneable because executors take it by reference but tests (and the
/// eventual scheduler Stop arm, plan 10-05) need to keep a second handle
/// to call `stop()` from outside the executor future.
#[derive(Clone)]
pub struct RunControl {
    /// The cancel token the executor's `tokio::select!` arm awaits.
    pub cancel: CancellationToken,
    /// Reason for the cancel. Lives in an `Arc<AtomicU8>` so cloning is
    /// cheap and multiple handles share state.
    pub stop_reason: Arc<AtomicU8>,
}

impl RunControl {
    /// Construct a new `RunControl` wrapping an existing cancel token.
    /// The reason defaults to `Shutdown` so a bare `cancel.cancel()` (as
    /// used by the existing shutdown drain path in `mod.rs`) is classified
    /// as Shutdown, not Operator — the existing shutdown semantics are
    /// preserved without any caller changes.
    pub fn new(cancel: CancellationToken) -> Self {
        Self {
            cancel,
            stop_reason: Arc::new(AtomicU8::new(StopReason::Shutdown as u8)),
        }
    }

    /// Set the stop reason atomically, then fire the cancel token.
    ///
    /// The `SeqCst` store **must** happen before `cancel.cancel()` so that
    /// the executor, which reads the atomic only after `cancel.cancelled()`
    /// yields, is guaranteed to observe the reason. See module-level doc.
    pub fn stop(&self, reason: StopReason) {
        self.stop_reason.store(reason as u8, Ordering::SeqCst);
        self.cancel.cancel();
    }

    /// Load the current stop reason. Called by executor cancel arms after
    /// `cancel.cancelled()` yields.
    pub fn reason(&self) -> StopReason {
        StopReason::from_u8(self.stop_reason.load(Ordering::SeqCst))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-V11-STOP-02 default-path regression lock: a freshly-constructed
    /// `RunControl` reports `Shutdown` and is not cancelled.
    #[test]
    fn new_run_control_defaults_to_shutdown() {
        let c = RunControl::new(CancellationToken::new());
        assert_eq!(c.reason(), StopReason::Shutdown);
        assert!(!c.cancel.is_cancelled());
    }

    /// T-V11-STOP-01: calling `stop(Operator)` sets the reason and fires
    /// the cancel token in a single observable step.
    #[test]
    fn stop_reason_operator_roundtrip() {
        let c = RunControl::new(CancellationToken::new());
        c.stop(StopReason::Operator);
        assert_eq!(c.reason(), StopReason::Operator);
        assert!(c.cancel.is_cancelled());
    }

    /// T-V11-STOP-02 regression lock: cancelling via the underlying token
    /// directly (NOT via `RunControl::stop`) leaves `reason()` at `Shutdown`.
    /// This is what the existing `cancel.cancel()` shutdown path does today
    /// and it must continue to classify as Shutdown.
    #[test]
    fn shutdown_cancel_stays_shutdown() {
        let c = RunControl::new(CancellationToken::new());
        c.cancel.cancel();
        assert_eq!(c.reason(), StopReason::Shutdown);
        assert!(c.cancel.is_cancelled());
    }

    /// T-V11-STOP-03: cloning a `RunControl` shares the atomic and the
    /// cancel token, so a `stop()` on one handle is observable from another.
    /// This is required because the scheduler Stop arm (plan 10-05) will
    /// hold a clone and the executor future will hold another.
    #[test]
    fn clone_shares_state() {
        let c1 = RunControl::new(CancellationToken::new());
        let c2 = c1.clone();
        c2.stop(StopReason::Operator);
        assert_eq!(c1.reason(), StopReason::Operator);
        assert!(c1.cancel.is_cancelled());
    }
}
