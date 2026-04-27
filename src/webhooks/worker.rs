//! Webhook delivery worker task. Owns the receiver half of the bounded
//! mpsc channel; the scheduler holds the sender. Worker exits cleanly on
//! cancel-token fire or sender-side close (None from recv).
//!
//! Phase 15 / WH-02. Structural copy of the `src/scheduler/log_pipeline.rs`
//! pattern: bounded channel, dedicated tokio task, scheduler never awaits
//! the consumer.

use std::sync::Arc;

use tokio::sync::mpsc;
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

/// Spawn the worker task. The task runs until either the cancel token fires
/// or the last sender clone is dropped.
pub fn spawn_worker(
    rx: mpsc::Receiver<RunFinalized>,
    dispatcher: Arc<dyn WebhookDispatcher>,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(worker_loop(rx, dispatcher, cancel))
}

async fn worker_loop(
    mut rx: mpsc::Receiver<RunFinalized>,
    dispatcher: Arc<dyn WebhookDispatcher>,
    cancel: CancellationToken,
) {
    loop {
        tokio::select! {
            // Bias toward draining events over checking cancel — prevents
            // a tight cancel loop from starving in-flight deliveries.
            biased;
            maybe_event = rx.recv() => {
                match maybe_event {
                    Some(event) => {
                        if let Err(err) = dispatcher.deliver(&event).await {
                            tracing::warn!(
                                target: "cronduit.webhooks",
                                run_id = event.run_id,
                                job_id = event.job_id,
                                status = %event.status,
                                error = %err,
                                "webhook dispatch returned error"
                            );
                        }
                    }
                    None => {
                        // All senders dropped — Scheduler is gone or shutting
                        // down. Exit cleanly. P20 will refine this path with
                        // drain accounting (WH-10).
                        tracing::info!(
                            target: "cronduit.webhooks",
                            "webhook worker exiting: channel closed"
                        );
                        break;
                    }
                }
            }
            _ = cancel.cancelled() => {
                tracing::info!(
                    target: "cronduit.webhooks",
                    remaining = rx.len(),
                    "webhook worker exiting: cancel token fired"
                );
                break;
            }
        }
    }
}
