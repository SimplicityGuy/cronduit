//! Webhook delivery worker task. Owns the receiver half of the bounded
//! mpsc channel; the scheduler holds the sender. Worker exits cleanly on
//! cancel-token fire or sender-side close (None from recv).
//!
//! Phase 15 / WH-02. Structural copy of the `src/scheduler/log_pipeline.rs`
//! pattern: bounded channel, dedicated tokio task, scheduler never awaits
//! the consumer.
//!
//! Phase 20 / WH-10 / D-15..D-18 + D-25 + D-26 extension: the 2-arm
//! `tokio::select!` becomes a 3-arm form with a drain-deadline state machine.
//! On the FIRST cancel-fire the worker enters drain mode (drain_deadline
//! becomes Some(Instant::now() + drain_grace)); subsequent loop iterations
//! continue draining `rx.recv()` and routing through `dispatcher.deliver()`
//! (which is cancel-aware on retry-sleep boundaries via Plan 02). When the
//! drain deadline elapses the third arm fires: remaining queued events are
//! drained-and-dropped via `rx.try_recv()` with per-event
//! `cronduit_webhook_deliveries_total{job, status="dropped"}` increments
//! (D-26 — distinct from the P15 channel-saturation drop counter that lives
//! in `src/scheduler/run.rs:445-452`; the worker NEVER touches that metric).
//! The `cronduit_webhook_queue_depth` gauge
//! is sampled on every `rx.recv()` boundary (D-25; no separate sampling
//! task; documented as approximate under contention).

use std::future::pending;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::{Instant, sleep_until};
use tokio_util::sync::CancellationToken;

use super::dispatcher::WebhookDispatcher;
use super::event::RunFinalized;

/// Channel capacity. WH-02 locks 1024 — large enough to absorb a transient
/// dispatcher stall without dropping events under normal homelab load,
/// small enough that a sustained outage produces visible drop-counter
/// activity within minutes (an operator-actionable signal).
pub const CHANNEL_CAPACITY: usize = 1024;

/// Construct the channel pair the scheduler + worker share. The Sender is
/// cloneable (cheap Arc-based refcount) and lives on Scheduler / SchedulerLoop;
/// the Receiver is consumed exactly once by spawn_worker.
pub fn channel() -> (mpsc::Sender<RunFinalized>, mpsc::Receiver<RunFinalized>) {
    mpsc::channel(CHANNEL_CAPACITY)
}

/// Test-only constructor with a tunable capacity. Integration tests in
/// `tests/v12_webhook_queue_drop.rs` use a small capacity (e.g., 4) to
/// force `TrySendError::Full` synchronously — pushing 1024 events from a
/// test is impractical.
pub fn channel_with_capacity(
    cap: usize,
) -> (mpsc::Sender<RunFinalized>, mpsc::Receiver<RunFinalized>) {
    mpsc::channel(cap)
}

/// Spawn the worker task. The task runs until either:
///   - the cancel token fires AND the drain budget elapses, OR
///   - the last sender clone is dropped (channel closed).
///
/// `drain_grace` (Phase 20 / WH-10) is the maximum wall-clock duration the
/// worker keeps draining the queue after the FIRST cancel-fire. The bin
/// layer (`src/cli/run.rs`, owned by Plan 06) threads
/// `cfg.server.webhook_drain_grace`. Worst-case shutdown ceiling is
/// `drain_grace + 10s` (D-18 + Pitfall 8 — reqwest's per-attempt timeout).
pub fn spawn_worker(
    rx: mpsc::Receiver<RunFinalized>,
    dispatcher: Arc<dyn WebhookDispatcher>,
    cancel: CancellationToken,
    drain_grace: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(worker_loop(rx, dispatcher, cancel, drain_grace))
}

async fn worker_loop(
    mut rx: mpsc::Receiver<RunFinalized>,
    dispatcher: Arc<dyn WebhookDispatcher>,
    cancel: CancellationToken,
    drain_grace: Duration,
) {
    // Drain-deadline state machine (D-15 step 1):
    //   None  → normal mode; Arm 2 (cancel) is enabled, Arm 3 (sleep) is disabled.
    //   Some  → drain mode; Arm 2 disabled (the cancel future stays "fired"
    //           but its arm guard prevents re-entry), Arm 3 enabled.
    // The deadline is set ONCE on the FIRST cancel-fire (`if drain_deadline.is_none()`
    // guard on Arm 2), NOT on every iteration.
    let mut drain_deadline: Option<Instant> = None;

    loop {
        // Per RESEARCH §13.4 verbatim: the drain-deadline arm is "never-completes"
        // outside drain mode. `std::future::pending::<()>().await` parks forever;
        // gated by `if drain_deadline.is_some()` so the arm is permanently
        // disabled when no deadline has been set.
        let sleep_arm = async {
            match drain_deadline {
                Some(dl) => sleep_until(dl).await,
                None => pending::<()>().await,
            }
        };

        tokio::select! {
            // Bias toward draining events over checking cancel — prevents
            // a tight cancel loop from starving in-flight deliveries.
            biased;

            // Arm 1: deliver next event (preserves existing behavior; adds
            // queue_depth gauge sample per D-25).
            maybe_event = rx.recv() => {
                // D-25 + RESEARCH §6.4 + Pitfall 7: sample queue depth at the
                // recv boundary. NO separate sampling task. Approximate under
                // contention (other tasks may try_send between rx.recv() and
                // .len() — that's documented and acceptable).
                metrics::gauge!("cronduit_webhook_queue_depth").set(rx.len() as f64);
                match maybe_event {
                    Some(event) => {
                        if let Err(err) = dispatcher.deliver(&event).await {
                            tracing::warn!(
                                target: "cronduit.webhooks",
                                run_id = event.run_id,
                                job_id = event.job_id,
                                job_name = %event.job_name,
                                status = %event.status,
                                error = %err,
                                "webhook dispatch returned error"
                            );
                        }
                    }
                    None => {
                        // All senders dropped — Scheduler is gone or shutting
                        // down. Exit cleanly.
                        tracing::info!(
                            target: "cronduit.webhooks",
                            "webhook worker exiting: channel closed"
                        );
                        break;
                    }
                }
            }

            // Arm 2: first cancel-fire enters drain mode (does NOT break).
            // The `if drain_deadline.is_none()` guard makes this arm
            // permanently disabled after the first fire — once `drain_deadline`
            // is Some, the cancel future stays fired but its arm is gated off,
            // so the loop keeps select-ing on Arms 1 + 3 only.
            _ = cancel.cancelled(), if drain_deadline.is_none() => {
                let dl = Instant::now() + drain_grace;
                drain_deadline = Some(dl);
                tracing::info!(
                    target: "cronduit.webhooks",
                    drain_grace_secs = drain_grace.as_secs(),
                    remaining = rx.len(),
                    "webhook worker entering drain mode"
                );
            }

            // Arm 3: drain budget elapsed — drop remaining queued events
            // and break.
            _ = sleep_arm, if drain_deadline.is_some() => {
                // D-15 step (4) + D-26: drain-and-drop via try_recv() in a
                // tight loop; per-event counter increment with closed-enum
                // status="dropped" + per-event WARN log. The P15
                // channel-saturation drop counter (src/scheduler/run.rs:450)
                // is NOT touched here — the two semantics are intentionally
                // distinct: queue saturation at try_send vs drain budget
                // expiry mid-delivery. Operators read both metrics to
                // distinguish "scheduler-side overload" from
                // "shutdown-time queue truncation".
                let mut dropped: u64 = 0;
                while let Ok(event) = rx.try_recv() {
                    metrics::counter!(
                        "cronduit_webhook_deliveries_total",
                        "job" => event.job_name.clone(),
                        "status" => "dropped",
                    )
                    .increment(1);
                    tracing::warn!(
                        target: "cronduit.webhooks",
                        run_id = event.run_id,
                        job_id = event.job_id,
                        job_name = %event.job_name,
                        "drain budget expired; dropping queued event"
                    );
                    dropped += 1;
                }
                tracing::info!(
                    target: "cronduit.webhooks",
                    dropped,
                    "webhook worker exiting: drain budget elapsed"
                );
                break;
            }
        }
    }
}
