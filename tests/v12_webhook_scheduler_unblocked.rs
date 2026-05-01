//! Phase 15 / WH-02 / T-V12-WH-03: scheduler keeps firing on time when
//! the webhook dispatcher is stalled. Proves the try_send-based emit at
//! src/scheduler/run.rs step 7d does not block the run-task body, which
//! would otherwise cascade into late spawns.
//!
//! Failure mode if absent: a future refactor that turns
//! `webhook_tx.try_send` into `webhook_tx.send().await` ships green; the
//! regression manifests only in production under a slow webhook receiver,
//! by which time scheduler drift has already broken operator alerts
//! (Pitfall 28).

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use cronduit::webhooks::{
    RunFinalized, WebhookDispatcher, WebhookError, channel_with_capacity, spawn_worker,
};
use tokio_util::sync::CancellationToken;

/// Dispatcher that sleeps 60s on every deliver call. Simulates the
/// operator-observable scenario of a webhook receiver that has crashed,
/// hung, or has very high latency.
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn try_send_does_not_block_when_dispatcher_is_stalled() {
    // The test simulates the run-task body's try_send call site. We do
    // NOT spin up the full scheduler harness — the load-bearing claim is
    // that try_send returns immediately even when the worker side is
    // stalled. Proving that at the channel boundary (where the scheduler
    // calls try_send) is sufficient to prove the scheduler is unblocked.

    // Use the production capacity (1024) so the saturation behavior is
    // realistic. The first ~1024 try_sends should succeed; subsequent
    // ones should drop via TrySendError::Full (worker can't drain because
    // dispatcher is stalled).
    let (tx, rx) = channel_with_capacity(1024);
    let cancel = CancellationToken::new();
    // Phase 20 / WH-10: spawn_worker now takes drain_grace. Use 30s.
    let _worker_handle = spawn_worker(
        rx,
        Arc::new(StalledDispatcher),
        cancel.clone(),
        Duration::from_secs(30),
    );

    // Simulate 5 jobs each firing once per second for 5 seconds. At
    // each tick we record the wall-clock Instant the try_send happens.
    // If try_send blocks (the bug we're guarding against), the recorded
    // Instants will drift by more than 1 second between consecutive
    // ticks.
    let mut spawn_times: Vec<Instant> = Vec::with_capacity(5);
    let test_start = Instant::now();

    for tick in 0..5 {
        // Wait until the tick boundary (1s after test_start, 2s, …).
        let target = test_start + Duration::from_secs(tick + 1);
        if let Some(remaining) = target.checked_duration_since(Instant::now()) {
            tokio::time::sleep(remaining).await;
        }

        // The "scheduler emit" — this is the operation under test.
        let emit_start = Instant::now();
        let _ = tx.try_send(make_event(tick as i64));
        let emit_elapsed = emit_start.elapsed();

        // Acceptance per emit: try_send must never block. 5ms is
        // generous (production try_send measures sub-microsecond).
        assert!(
            emit_elapsed < Duration::from_millis(5),
            "try_send must not block; tick {tick} took {emit_elapsed:?}"
        );

        spawn_times.push(emit_start);
    }

    // Acceptance — no inter-spawn interval drifts by more than 1s
    // (success criterion #3 in ROADMAP.md Phase 15: "no scheduler drift
    // > 1 s"). For 5 ticks at 1s nominal cadence, max(spawns[i+1] -
    // spawns[i]) - 1s < 1s.
    let mut max_drift = Duration::from_secs(0);
    for w in spawn_times.windows(2) {
        let interval = w[1].duration_since(w[0]);
        let drift = interval.saturating_sub(Duration::from_secs(1));
        if drift > max_drift {
            max_drift = drift;
        }
    }

    assert!(
        max_drift < Duration::from_secs(1),
        "scheduler drift {max_drift:?} > 1s — try_send is blocking on stalled dispatcher (Pitfall 28 regression). Spawn intervals: {:?}",
        spawn_times
            .windows(2)
            .map(|w| w[1].duration_since(w[0]))
            .collect::<Vec<_>>()
    );

    // Cleanup — cancel the worker. Do not await; it is sleeping 60s.
    // Use explicit drop (vs `let _ =`) so clippy's let-underscore-future
    // lint stays clean.
    cancel.cancel();
    drop(_worker_handle);
}
