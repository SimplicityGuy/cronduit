---
phase: 15-foundation-preamble
reviewed: 2026-04-26T00:00:00Z
depth: standard
files_reviewed: 20
files_reviewed_list:
  - .github/workflows/ci.yml
  - Cargo.lock
  - Cargo.toml
  - deny.toml
  - justfile
  - src/cli/run.rs
  - src/lib.rs
  - src/scheduler/mod.rs
  - src/scheduler/run.rs
  - src/telemetry.rs
  - src/webhooks/dispatcher.rs
  - src/webhooks/event.rs
  - src/webhooks/mod.rs
  - src/webhooks/worker.rs
  - tests/metrics_endpoint.rs
  - tests/metrics_stopped.rs
  - tests/scheduler_integration.rs
  - tests/stop_executors.rs
  - tests/v12_webhook_queue_drop.rs
  - tests/v12_webhook_scheduler_unblocked.rs
findings:
  critical: 0
  warning: 4
  info: 6
  total: 10
status: issues_found
---

# Phase 15: Code Review Report

**Reviewed:** 2026-04-26
**Depth:** standard
**Files Reviewed:** 20
**Status:** issues_found

## Summary

Phase 15 lands three orthogonal changes — version bump, cargo-deny preamble (warn-only), and an in-process webhook worker scaffolding (`NoopDispatcher` + bounded mpsc + `try_send` emit). The webhook worker design is structurally sound: `try_send` correctly avoids blocking the scheduler (proven by `tests/v12_webhook_scheduler_unblocked.rs`), the drop counter is eagerly registered so `/metrics` renders it from boot, and the trait object `Arc<dyn WebhookDispatcher>` is the right shape for P18's `HttpDispatcher` swap. No security defects, no correctness defects in the locked Pitfall-1 surface.

The findings below cluster around two themes: (1) **shutdown ordering between the cancel token and the scheduler drain produces a small but real race window** where webhook events finalized during the grace period can be dropped because the worker's `cancel.cancelled()` arm fires the moment `cancel.cancel()` is called — before the scheduler finishes draining its `JoinSet`; (2) **the queue-drop test exercises the metric registry but not the production try_send call site**, weakening the regression contract it claims to lock. Both are WARNING-class; neither blocks v1.2 rc.1.

Several smaller items (test brittleness, metric-line parser edge case, license-allowlist drift, version mismatch between dependencies and dev-dependencies tokio pin) are tracked as INFO.

## Warnings

### WR-01: Worker can exit on cancel before scheduler finishes draining — events dropped during grace period

**File:** `src/webhooks/worker.rs:50-96`, `src/cli/run.rs:250-282`
**Issue:** The worker's `tokio::select!` has a `cancel.cancelled()` arm that fires immediately when the parent cancel token is cancelled. The scheduler shutdown path in `cli/run.rs` cancels the token, then awaits the scheduler's drain (which executes the `_ = self.cancel.cancelled() =>` block in `SchedulerLoop::run()` at `src/scheduler/mod.rs:462-548`), during which in-flight runs continue to call `finalize_run` step 7d (`webhook_tx.try_send(event)`). Because the worker shares the same cancel signal (`cancel.child_token()` propagates parent cancellation), the worker can win the race and exit while the scheduler is still draining, causing:

1. Events emitted during the grace period after worker exit hit a closed receiver and produce `TrySendError::Closed` — every drained run logs an `error!` line ("webhook delivery channel closed — worker is gone") at `src/scheduler/run.rs:439-448`. In production this means a normal SIGTERM under load produces a flurry of error-level logs.
2. Events still buffered in the channel at the moment `cancel.cancelled()` wins are silently dropped — they are never dispatched and there is no counter increment for this case (the drop counter only fires on `TrySendError::Full`). The `tracing::info!` at `worker.rs:87-91` even logs `remaining = rx.len()` confirming events ARE in the channel at exit time.

The `biased;` directive does NOT prevent this: `biased;` only changes the per-iteration poll order. Once `rx.recv()` returns `Poll::Pending` (channel empty for an instant), the select moves on to poll `cancel.cancelled()` and exits. During the scheduler drain, the channel is repeatedly emptied between events — every such empty interval is a window for the worker to exit.

The comment at `src/cli/run.rs:243-249` recognizes the inverse race ("awaiting the worker before the scheduler drains would race finalize_run's last try_send calls") and orders the awaits scheduler-then-worker, but using `cancel.child_token()` defeats that ordering: cancellation flows to the worker the same instant it reaches the scheduler.

**Fix:** Use a separate cancel token for the worker that is only cancelled after the scheduler's `JoinHandle` resolves. Sketch:

```rust
// src/cli/run.rs — replace child_token() with an independent token.
let webhook_cancel = CancellationToken::new();
let (webhook_tx, webhook_rx) = crate::webhooks::channel();
let webhook_worker_handle = crate::webhooks::spawn_worker(
    webhook_rx,
    std::sync::Arc::new(crate::webhooks::NoopDispatcher),
    webhook_cancel.clone(),
);

// ... spawn scheduler with `cancel.clone()` (unchanged) ...

// Wait for serve + scheduler drain first.
let serve_result = web::serve(resolved_bind, state, cancel).await;
let _ = scheduler_handle.await;

// Now drop the SchedulerLoop's clone of webhook_tx (already happened with
// scheduler_handle resolution) and signal the worker to drain remaining
// events with a bounded budget, then exit.
//
// The Sender held in `cli/run.rs` itself (line 250) also needs to drop
// before the worker can exit on `None`. Either drop it explicitly here
// or scope it so it falls out before the worker await.
drop(webhook_tx);

// Optional: brief grace for the worker to drain anything still in-channel.
// If P15 doesn't want to add a budget knob, just await the worker — it
// will exit on `None` once the channel is empty.
let _ = tokio::time::timeout(Duration::from_secs(5), webhook_worker_handle).await;
webhook_cancel.cancel(); // belt-and-suspenders for the timeout path
```

Even simpler, if drain accounting is genuinely deferred to P20: don't pass any cancel token — let the worker exit purely on channel close. The `tokio::select!` becomes a plain `while let Some(ev) = rx.recv().await` loop. P20's WH-10 then re-introduces a cancel arm with explicit drain budget.

---

### WR-02: Queue-drop test increments the counter manually — does not lock the production try_send → counter contract

**File:** `tests/v12_webhook_queue_drop.rs:96-113`
**Issue:** The test loops over `tx.try_send(make_event(i))` and on `TrySendError::Full` does:
```rust
metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(1);
```
in the test body itself (line 107). The comment claims this "mirrors the production code path." It does not — it bypasses it. The test then asserts the counter delta is `>= 10` — but that delta is produced by the test's own increments, not by `src/scheduler/run.rs:437`. A future refactor that:

- Renames the metric in production code (e.g. `cronduit_webhooks_dropped_total`)
- Removes the `metrics::counter!(...)` call from `run.rs:437`
- Increments under the wrong condition (e.g. on `TrySendError::Closed` too, double-counting)

…would ship green because the test exercises only the metric registry, not the call site. The phase context calls out exactly this risk ("a future refactor that swallows the drop counter increment"); the current test does not actually guard against it.

**Fix:** Drive the assertion through the real `run_job` (or a wrapper that calls the same try_send pattern) so the counter increment under test is the one in `src/scheduler/run.rs`:

```rust
// Set up a real run_job invocation against an in-memory pool, with a
// pre-filled channel of capacity 4 and a stalled dispatcher worker so
// the channel is full when run_job's finalize_run step 7d fires.
//
// 1. Create channel_with_capacity(4) and spawn worker with StalledDispatcher.
// 2. Pre-fill the channel with 4 events.
// 3. Read counter baseline.
// 4. Invoke run_job(...) once with the now-full webhook_tx.
// 5. Assert counter delta == 1 (the single try_send in step 7d hit Full).
```

If full integration is too heavy, factor the try_send-and-classify logic in `run.rs:427-449` into a tiny helper:
```rust
fn emit_run_finalized(tx: &Sender<RunFinalized>, ev: RunFinalized) {
    match tx.try_send(ev) {
        Ok(()) => {}
        Err(TrySendError::Full(d)) => { tracing::warn!(...); metrics::counter!(...).increment(1); }
        Err(TrySendError::Closed(_)) => { tracing::error!(...); }
    }
}
```
…and invoke that helper from the test.

---

### WR-03: Webhook drop counter has no labels; carries no failure-mode signal an operator can act on

**File:** `src/scheduler/run.rs:437`, `src/telemetry.rs:111-117`
**Issue:** The metric `cronduit_webhook_delivery_dropped_total` is incremented as a bare counter with no labels. From an operator perspective, a non-zero rate on this counter answers "is something being dropped?" but not "what?" — and crucially does not differentiate `TrySendError::Full` (channel saturated, dispatcher slow) from `TrySendError::Closed` (worker dead, every future event lost). The current code only increments on `Full` — `Closed` produces an error log but no metric. So a Prometheus alert on this counter:

- Stays silent if the worker has exited cleanly under shutdown (`Closed` path) — which is exactly when an operator most wants to know.
- Cannot drive a routing rule like "page on Full > X/min, page on Closed != 0".

The describe text ("Closed-cardinality (no labels in P15). … The full cronduit_webhook_* family lands in P20 / WH-11") acknowledges the deferral but the drop-classification gap is operator-visible TODAY in v1.2.0 if any webhook channel saturation occurs.

**Fix:** Either (a) increment a separate counter on `TrySendError::Closed`:

```rust
Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
    tracing::error!(...);
    metrics::counter!("cronduit_webhook_delivery_dropped_total", "reason" => "closed".to_string()).increment(1);
}
Err(tokio::sync::mpsc::error::TrySendError::Full(dropped)) => {
    // ...
    metrics::counter!("cronduit_webhook_delivery_dropped_total", "reason" => "full".to_string()).increment(1);
}
```

…and pre-declare both label values in `setup_metrics` so they render from boot (same pattern as `cronduit_runs_total{status=...}` at `src/telemetry.rs:153-163`). Closed cardinality preserved (2 values).

Or (b) document the limitation in the describe text loud enough that a P20 plan is required to ship before v1.2.0 GA. Currently the describe text says "no labels in P15" without flagging that `Closed` is silently uncounted.

---

### WR-04: Started_at is reconstructed by subtracting elapsed from now — non-monotonic and prone to drift

**File:** `src/scheduler/run.rs:414-426`
**Issue:** The `RunFinalized` event carries `started_at: DateTime<Utc>` and `finished_at: DateTime<Utc>`. The code computes:
```rust
let finished_at = chrono::Utc::now();
let started_at = finished_at
    - chrono::Duration::from_std(start.elapsed())
        .unwrap_or_else(|_| chrono::Duration::zero());
```
Two issues:

1. `start: tokio::time::Instant` is a monotonic clock; `chrono::Utc::now()` is a wall clock. Subtracting elapsed from now is not equivalent to capturing wall-clock at run start. If the system clock jumped backward during the run (NTP correction, manual `date` change), the computed `started_at` will land in the future relative to the actual wall-clock start. The scheduler already has clock-jump detection at `src/scheduler/mod.rs:115-122` for fire scheduling; webhook timestamps inherit the same exposure but with no detection.
2. `chrono::Duration::from_std(start.elapsed())` only fails if elapsed exceeds `i64::MAX` milliseconds (~292M years). The `unwrap_or_else(|_| chrono::Duration::zero())` fallback would set `started_at == finished_at` — a wrong-but-not-catastrophic value. Practically unreachable.

The correct fix is to capture wall-clock start at the row's actual start: `insert_running_run` already records a row with `start_time` in the DB. The webhook event should either query that column, or `started_at` should be threaded into `continue_run` from the caller (`run_job` and `run_job_with_existing_run_id`) where wall-clock-now is captured at the same instant `start` is taken.

**Fix:**
```rust
// At top of run_job:
let started_at_wall = chrono::Utc::now();
let start = tokio::time::Instant::now();
// ...pass started_at_wall through continue_run...
```
And in `continue_run`:
```rust
let event = crate::webhooks::RunFinalized {
    // ...
    started_at: started_at_wall,  // wall-clock capture, not reconstructed
    finished_at: chrono::Utc::now(),
};
```

For P15 this manifests as a small inaccuracy in webhook payloads under NTP correction; under stable clocks the elapsed-subtraction approximation is fine. Land before P18 wires `HttpDispatcher` so external consumers don't see drift.

## Info

### IN-01: Phase-context spec drift — `Box<dyn WebhookDispatcher>` vs `Arc<dyn WebhookDispatcher>`

**File:** `src/webhooks/worker.rs:44,52`
**Issue:** The reviewer prompt and (presumably) the phase plans reference `Box<dyn WebhookDispatcher>` as the trait-object boundary. The actual code uses `Arc<dyn WebhookDispatcher>` (worker.rs:44 and worker.rs:52). `Arc` is the correct choice here — `spawn_worker` moves the dispatcher into the spawned task, and a Box would also work, but the existing call site in `cli/run.rs:253` constructs `std::sync::Arc::new(crate::webhooks::NoopDispatcher)`, which would not coerce to `Box`. Either update the design doc to match the code, or update the code to match the doc; do not let the drift sit through P18.
**Fix:** Trivial — update planning docs to say `Arc<dyn WebhookDispatcher>`.

---

### IN-02: `channel_with_capacity` is a public API but documented as test-only

**File:** `src/webhooks/worker.rs:30-38`, `src/webhooks/mod.rs:23`
**Issue:** The doc comment at `worker.rs:30-33` says: *"Test-only constructor with a tunable capacity."* It is exported via `mod.rs:23` (`pub use worker::{... channel_with_capacity ...}`) and used by integration tests as a public API. There is no `#[cfg(test)]` gate and no sealing pattern; downstream consumers (or P18) could call it from non-test code. Either add `#[cfg(any(test, feature = "test-util"))]` or drop the "test-only" claim from the doc.
**Fix:** Replace the doc claim with: *"Constructor with explicit capacity, primarily used by integration tests; production code calls `channel()` to get the locked 1024 capacity."*

---

### IN-03: Tokio version mismatch between `[dependencies]` and `[dev-dependencies]`

**File:** `Cargo.toml:21,150`
**Issue:** `[dependencies] tokio = { version = "1.52", ... }` and `[dev-dependencies] tokio = { version = "1.51", ... }`. Cargo will resolve to the higher (1.52) for the unified dep graph, so this is not a build error, but it is a maintenance smell — a future bump of one and not the other could leave the dev-deps line as the SemVer-pinning floor and silently constrain the resolved version. The version was bumped to "1.52" in this phase (per Phase 15 scope) and the dev-deps line was not updated alongside.
**Fix:** Bump the dev-deps line to `version = "1.52"` to match.

---

### IN-04: License allowlist `Unicode-DFS-2016` will not match `Unicode-3.0` ICU crates

**File:** `deny.toml:32-38`
**Issue:** The allowlist contains `Unicode-DFS-2016` but modern Unicode-licensed crates (notably `icu_collections`, `icu_normalizer`, `icu_normalizer_data`, present in `Cargo.lock` lines 1638, 1665, 1679 etc.) ship as SPDX `Unicode-3.0`. Once `cargo deny check licenses` runs, these will be flagged. The CI step is `continue-on-error: true` so PR red is avoided, but the new "blocker" promotion in Phase 24 will fail on this. Better to land the right allowlist entry now than discover it during the Phase 24 gate flip.
**Fix:** Add `"Unicode-3.0"` to the allow list:
```toml
allow = [
    "MIT",
    "Apache-2.0",
    "BSD-3-Clause",
    "ISC",
    "Unicode-3.0",
    "Unicode-DFS-2016",  # keep for older ICU pins; can drop in Phase 24 if no usage remains
]
```

---

### IN-05: Drop-counter line parser matches metric-name prefix only — fragile under family expansion

**File:** `tests/v12_webhook_queue_drop.rs:58-66`
**Issue:** `read_drop_counter` accepts both `cronduit_webhook_delivery_dropped_total ` (with trailing space) and `cronduit_webhook_delivery_dropped_total{` (with brace). This is fine today. When P20 adds the `cronduit_webhook_*` family (per the describe text), a future metric named `cronduit_webhook_delivery_dropped_total_per_target` (hypothetical) would also match the second branch because `starts_with` is a prefix test. This is exactly the kind of subtle test-helper rot that goes unnoticed for releases.
**Fix:** Tighten the parser:
```rust
.find(|l| {
    l.starts_with("cronduit_webhook_delivery_dropped_total ")
        || (l.starts_with("cronduit_webhook_delivery_dropped_total{") && l.contains("} "))
})
```
Or use a regex anchored on `^cronduit_webhook_delivery_dropped_total(\{[^}]*\})?\s+`.

---

### IN-06: Timing-sensitive assertions in webhook tests — possible CI flake under heavy GHA load

**File:** `tests/v12_webhook_queue_drop.rs:118-121`, `tests/v12_webhook_scheduler_unblocked.rs:86-89,107-114`
**Issue:** Two timing budgets:

1. `queue_drop` asserts 20 try_sends complete in `< 50ms`. On a quiet machine try_send is sub-microsecond, but a heavily-loaded GHA runner under matrix arm64 emulation (or even ubuntu-latest under contention) has measurably worse mutex acquisition latency. 50ms is generous but not bulletproof.
2. `scheduler_unblocked` asserts each try_send completes in `< 5ms` AND that the inter-tick drift across 5 one-second sleeps is `< 1s`. The drift assertion is fine — `tokio::time::sleep` is reliable to milliseconds — but the 5ms per-try_send budget is tight; transient GC-like jitter from a noisy neighbor could blow it.

These are not bugs and the contract these tests lock is real (Pitfall 28). But CI flakes from timing assertions erode signal quality. Note: tests do NOT use `tokio::time::pause`/`advance` — they rely on real wall-clock sleep — so virtual-time deflaking is not available without restructuring.
**Fix:** Loosen budgets to absorb CI noise. 50ms → 250ms; 5ms → 25ms. The contract these tests lock is "try_send is non-blocking", not "try_send is sub-millisecond"; a 25ms budget is still 4-6 orders of magnitude away from a blocking `send().await` on a stalled dispatcher (60 seconds). Drift budget can stay at `< 1s`.

---

_Reviewed: 2026-04-26_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
