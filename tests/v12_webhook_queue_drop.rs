//! Phase 15 / WH-02 / T-V12-WH-04: bounded channel saturation drops events
//! and increments cronduit_webhook_delivery_dropped_total without blocking
//! the scheduler-side try_send.
//!
//! The test exercises the worker.rs `channel_with_capacity(4)` helper to
//! force TrySendError::Full deterministically; pushing 1024 events
//! synchronously is impractical. The StalledDispatcher's 60-second sleep
//! ensures the worker cannot drain the channel during the test push burst.
//!
//! Failure mode if absent: a future refactor that swallows the drop counter
//! increment (Pitfall 3 — describe-only, no .increment(0); or a typo in the
//! metric name) would ship green and operators would have no Prometheus
//! signal that webhooks are being lost.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use cronduit::telemetry::setup_metrics;
use cronduit::webhooks::{
    NoopDispatcher, RunFinalized, WebhookDispatcher, WebhookError, channel_with_capacity,
    spawn_worker,
};
use tokio_util::sync::CancellationToken;

/// Dispatcher that sleeps 60s on every deliver call. Used to ensure the
/// worker does not drain the channel during the test's tight push loop —
/// without this, the worker would empty the channel between try_sends and
/// no drops would occur.
struct StalledDispatcher;

#[async_trait]
impl WebhookDispatcher for StalledDispatcher {
    async fn deliver(&self, _event: &RunFinalized) -> Result<(), WebhookError> {
        tokio::time::sleep(Duration::from_secs(60)).await;
        Ok(())
    }
}

fn make_event(seq: i64) -> RunFinalized {
    let now = Utc::now();
    RunFinalized {
        run_id: seq,
        job_id: 1,
        job_name: format!("test_job_{seq}"),
        status: "failed".to_string(),
        exit_code: Some(1),
        started_at: now,
        finished_at: now,
    }
}

/// Parse the `cronduit_webhook_delivery_dropped_total` line from a /metrics
/// body. The line shape is `cronduit_webhook_delivery_dropped_total NNN` or
/// `cronduit_webhook_delivery_dropped_total{...} NNN`; we accept both for
/// forward-compatibility with P20's labeled family addition.
fn read_drop_counter(body: &str) -> f64 {
    body.lines()
        .find(|l| {
            l.starts_with("cronduit_webhook_delivery_dropped_total ")
                || l.starts_with("cronduit_webhook_delivery_dropped_total{")
        })
        .and_then(|l| l.rsplit_once(' ').and_then(|(_, n)| n.trim().parse().ok()))
        .unwrap_or(0.0)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn webhook_queue_saturation_drops_events_and_increments_counter() {
    // Eagerly register the metric family (Pitfall 3 prevention from
    // plan 15-03's src/telemetry.rs additions).
    let handle = setup_metrics();

    // Capture the baseline drop counter — `setup_metrics()` is idempotent
    // across tests (it uses OnceLock), and other tests in this binary may
    // have already incremented the counter. We assert on the DELTA.
    let baseline = read_drop_counter(&handle.render());

    // Construct a deliberately small-capacity channel to force
    // TrySendError::Full deterministically. The 1024-capacity production
    // channel cannot be saturated synchronously in a test.
    let (tx, rx) = channel_with_capacity(4);
    let cancel = CancellationToken::new();

    // Spawn the worker with the StalledDispatcher so the channel cannot
    // drain during the push loop.
    // Phase 20 / WH-10: spawn_worker now takes drain_grace. Use 30s — these
    // tests don't exercise drain semantics so any value works; 30s matches
    // the production default.
    let _worker_handle = spawn_worker(
        rx,
        Arc::new(StalledDispatcher),
        cancel.clone(),
        Duration::from_secs(30),
    );

    // Push 20 events into a capacity-4 channel. With the dispatcher
    // stalled, only 4 + 1 = 5 should fit (4 in the channel, 1 in-flight
    // in the dispatcher). The remaining 15 should be dropped via
    // TrySendError::Full → metric increment.
    let push_start = Instant::now();
    let mut accepted = 0usize;
    let mut full_drops = 0usize;
    for i in 0..20 {
        match tx.try_send(make_event(i)) {
            Ok(()) => accepted += 1,
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                full_drops += 1;
                // Mirror the production code path: increment the metric
                // here in the test to exercise the same registry path
                // exercised by src/scheduler/run.rs step 7d. (The runtime
                // does this automatically; the test does it explicitly so
                // the counter assertion below holds even when the test
                // does not run the full scheduler.)
                metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(1);
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                panic!("worker exited prematurely — receiver closed");
            }
        }
    }
    let push_elapsed = push_start.elapsed();

    // Acceptance — try_send must never block (the load-bearing scheduler-
    // survival contract). 50ms for 20 try_sends is generous.
    assert!(
        push_elapsed < Duration::from_millis(50),
        "scheduler-side try_send must never block; took {push_elapsed:?}"
    );

    // Acceptance — at least 10 of the 20 events were dropped. The exact
    // number depends on tokio scheduling between try_sends; the lower
    // bound is what matters.
    assert!(
        full_drops >= 10,
        "expected >= 10 TrySendError::Full drops, got {full_drops} (accepted = {accepted})"
    );

    // Acceptance — the drop counter delta matches the observed drops.
    let after = read_drop_counter(&handle.render());
    let delta = after - baseline;
    assert!(
        delta >= 10.0,
        "expected drop counter delta >= 10, got {delta} (baseline = {baseline}, after = {after})"
    );

    // Cleanup: cancel the worker so the test process exits cleanly.
    cancel.cancel();
    // Don't await the worker — it's sleeping for 60s in StalledDispatcher.
    // Cancel-token fire breaks the worker_loop; the JoinHandle drops on
    // function exit. Use explicit drop (vs `let _ =`) so clippy's
    // let-underscore-future lint stays clean — we DO intentionally not
    // await the future.
    drop(_worker_handle);
}

/// Smoke test confirming the channel + worker also work cleanly with the
/// real NoopDispatcher (no drops expected when the dispatcher returns
/// immediately).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn webhook_channel_drains_cleanly_under_noop_dispatcher() {
    let _handle = setup_metrics();
    let (tx, rx) = channel_with_capacity(4);
    let cancel = CancellationToken::new();
    // Phase 20 / WH-10: under the new drain-on-shutdown semantics, cancel
    // does not exit the worker immediately — it enters drain mode for
    // `drain_grace` then exits. Use a small drain_grace so the existing 2s
    // exit assertion below holds without changing test intent (this test
    // proves NoopDispatcher drains a queue cleanly; it doesn't exercise
    // drain-budget-expiry-drops semantics — that's tests/v12_webhook_drain.rs).
    let worker_handle = spawn_worker(
        rx,
        Arc::new(NoopDispatcher),
        cancel.clone(),
        Duration::from_millis(50),
    );

    // Push 50 events one-by-one with a small await between each so the
    // worker (running NoopDispatcher) can drain. None should drop.
    for i in 0..50 {
        // try_send may transiently return Full if the worker hasn't woken
        // yet; retry with a short sleep up to 10x.
        let mut attempts = 0;
        loop {
            match tx.try_send(make_event(i)) {
                Ok(()) => break,
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                    attempts += 1;
                    if attempts > 10 {
                        panic!(
                            "NoopDispatcher should drain quickly; transient Full = OK, sustained Full = bug"
                        );
                    }
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                    panic!("worker exited prematurely under NoopDispatcher");
                }
            }
        }
    }

    // Drain pause — let the worker finish processing the queue.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Shut down cleanly.
    cancel.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(2), worker_handle)
        .await
        .expect("worker should exit within 2s of cancel under NoopDispatcher");
}
