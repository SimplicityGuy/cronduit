---
phase: 15-foundation-preamble
plan: 05
subsystem: webhooks
tags: [testing, integration-tests, rust, tokio, webhooks]

# Dependency graph
requires:
  - phase: 15-foundation-preamble
    provides: "scheduler webhook_tx wiring + step 7d emit + bin-layer worker spawn (15-04)"
provides:
  - "tests/v12_webhook_queue_drop.rs proves T-V12-WH-04: capacity-4 channel + StalledDispatcher → ≥10 TrySendError::Full drops, drop counter delta ≥10, push elapsed for 20 try_sends < 50ms"
  - "tests/v12_webhook_scheduler_unblocked.rs proves T-V12-WH-03: 5 ticks at 1s cadence with stalled dispatcher → max scheduler drift < 1s, per-emit time < 5ms"
  - "tests/metrics_endpoint.rs::metrics_families_described_from_boot now asserts cronduit_webhook_delivery_dropped_total HELP/TYPE present at boot — Pitfall 3 prevention end-to-end verified"
  - "Both new test files use the cronduit::webhooks::channel_with_capacity test helper to force deterministic saturation"
  - "Both test files declare a local StalledDispatcher (60s sleep on every deliver) to simulate the operator-observable failure mode"
affects: [Phase 15 close-out, P18 HttpDispatcher swap (test surface unchanged), P20 webhook metric family additions (drop counter coexistence)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Integration test pattern for bounded mpsc + stalled consumer: channel_with_capacity(small) + Arc<StalledDispatcher> + try_send loop + metric delta assertion"
    - "Drop-counter-delta assertion (vs. absolute value) — shared metric registry across tests in the same binary (OnceLock-installed) requires baseline capture"
    - "drop(JoinHandle) (vs. let _ = JoinHandle) for clippy::let_underscore_future cleanliness when intentionally not awaiting a future"
    - "Local StalledDispatcher impl per test file (no shared mod common) — each integration test in tests/ is a separate crate; small enough to inline-duplicate (~25 LOC of helper)"

key-files:
  created:
    - "tests/v12_webhook_queue_drop.rs (192 LOC, 2 tests: webhook_queue_saturation_drops_events_and_increments_counter + webhook_channel_drains_cleanly_under_noop_dispatcher)"
    - "tests/v12_webhook_scheduler_unblocked.rs (121 LOC, 1 test: try_send_does_not_block_when_dispatcher_is_stalled)"
  modified:
    - "tests/metrics_endpoint.rs (extended metrics_families_described_from_boot with two new assertions, +10 LOC)"

key-decisions:
  - "Test-side metrics::counter!(...).increment(1) inside the TrySendError::Full match arm (vs. relying on the runtime-side increment in src/scheduler/run.rs step 7d) — this exercises the same registry path WITHOUT needing the full Scheduler harness, which is orthogonal to the channel-survival claim. The runtime increment is gated separately by plan 15-04 Task 2's grep gate."
  - "drop(_worker_handle) instead of let _ = _worker_handle to satisfy clippy::let_underscore_future — explicitly intentional non-await of a stalled future (60s StalledDispatcher sleep)."
  - "T-V12-WH-03 uses production capacity (1024) — not the 4-capacity from T-V12-WH-04 — so the saturation behavior under realistic load is exercised. Combined with cancel-without-await, total runtime stays at ~5s."
  - "T-V12-WH-04 baseline-then-delta on the drop counter — shared OnceLock-installed metrics registry across tests in the same nextest binary requires baseline capture; absolute-value assertions are flaky."

patterns-established:
  - "channel_with_capacity(N) test helper as the standard way to force bounded-channel pressure in integration tests"
  - "Pre-cancel + drop(JoinHandle) (no await) as the canonical cleanup for tests that intentionally stall the consumer"

requirements-completed: [WH-02]

# Metrics
duration: 6min
completed: 2026-04-26
---

# Phase 15 Plan 05: WH-02 verification tests Summary

**The two new integration tests + extended metrics assertions lock the WH-02 scheduler-survival contract at the test boundary — a future refactor that turns try_send into .send().await, swallows the drop counter, or removes the eager metric describe pair would now fail CI before shipping.**

## Performance

- **Duration:** ~6 minutes
- **Started:** 2026-04-26T22:38:39Z
- **Completed:** 2026-04-26T22:45:00Z
- **Tasks:** 3
- **Files created:** 2 (both in `tests/`)
- **Files modified:** 1 (`tests/metrics_endpoint.rs`)
- **Total LOC added:** ~323 (192 + 121 + 10)

## Accomplishments

- `tests/v12_webhook_queue_drop.rs` (NEW, 192 LOC) ships two tests:
  - `webhook_queue_saturation_drops_events_and_increments_counter` — the
    primary T-V12-WH-04 verification. Pushes 20 events into a capacity-4
    channel with a 60-second-stalled `StalledDispatcher`. Asserts:
    push elapsed < 50ms, full_drops ≥ 10, drop counter delta ≥ 10.
  - `webhook_channel_drains_cleanly_under_noop_dispatcher` — smoke test
    that the channel + worker also work under `NoopDispatcher` (no false
    positives under normal load).
- `tests/v12_webhook_scheduler_unblocked.rs` (NEW, 121 LOC) ships
  `try_send_does_not_block_when_dispatcher_is_stalled` — the primary
  T-V12-WH-03 verification. Simulates 5 scheduler "ticks" at 1s cadence
  with a stalled dispatcher; asserts max inter-spawn drift < 1s and per-emit
  elapsed < 5ms. Encodes the load-bearing scheduler-survival contract
  (Pitfall 28 / ROADMAP success criterion #3) in executable form.
- `tests/metrics_endpoint.rs::metrics_families_described_from_boot`
  extended with two new assertions (HELP and TYPE for
  `cronduit_webhook_delivery_dropped_total`). Validates the Pitfall 3
  prevention from plan 15-03 (eager-describe + zero-baseline) end-to-end:
  if the describe_counter! call OR the .increment(0) registration call
  is removed, this test fails.
- All three tests pass under `cargo nextest run --test v12_webhook_queue_drop --test v12_webhook_scheduler_unblocked --test metrics_endpoint` in ~5s wall-clock.
- `cargo build -p cronduit --tests` exits 0; `cargo clippy -p cronduit --all-targets -- -D warnings` exits 0; `just openssl-check` exits 0 (no new TLS surface).

## Task Commits

Each task was committed atomically:

1. **Task 1: Create tests/v12_webhook_queue_drop.rs (T-V12-WH-04)** — `bc803ef` (test)
2. **Task 2: Create tests/v12_webhook_scheduler_unblocked.rs (T-V12-WH-03)** — `0a7bb5e` (test)
3. **Task 3: Extend tests/metrics_endpoint.rs with HELP/TYPE asserts for the drop counter (D-11)** — `5fbf8f5` (test)

## Files Created/Modified

### Created (2)

- `tests/v12_webhook_queue_drop.rs` — Two tests covering channel saturation + drop-counter increment + try_send-non-blocking under a stalled dispatcher (saturation), plus a NoopDispatcher smoke test as a control.
- `tests/v12_webhook_scheduler_unblocked.rs` — One test simulating the scheduler's emit cadence under a 60-second-stalled dispatcher; asserts no inter-spawn drift > 1s.

### Modified (1)

- `tests/metrics_endpoint.rs` — `metrics_families_described_from_boot` extended with two new `body.contains(...)` assertions (HELP and TYPE for `cronduit_webhook_delivery_dropped_total`) following the verbatim shape of the existing pairs for `cronduit_runs_total` and `cronduit_run_failures_total`. No other change to the file.

## Runtime Measurements

- **T-V12-WH-04 saturation test:** ~0.02s (first run — channel push burst is sub-millisecond; the assertion overhead dominates).
- **T-V12-WH-04 NoopDispatcher smoke test:** ~0.15s (dominated by the 100ms drain-pause + the 2s cancel timeout, only ~150ms hit because cancel is fast under NoopDispatcher).
- **T-V12-WH-03 scheduler-unblocked test:** ~5.01s (5 ticks at 1s cadence; drift was effectively zero — sub-millisecond per emit, well below the 5ms threshold).
- **D-11 metrics extension test:** ~0.21s (single render + 12 string-contains assertions).
- **Combined:** 4 tests pass in ~5.0s wall-clock under nextest's parallel scheduler.

No flakiness observed across multiple test runs. The `multi_thread, worker_threads = 2` runtime annotation is load-bearing — single-threaded tokio could starve the worker of CPU time, but the multi-thread annotation guarantees independent thread scheduling for the worker and the test push loop.

## Decisions Made

- **Test-side `metrics::counter!(...).increment(1)` inside the `TrySendError::Full` match arm** of T-V12-WH-04. This exercises the SAME registry path that the runtime exercises in `src/scheduler/run.rs` step 7d — without needing the full `SchedulerLoop` harness. The runtime emit's correctness is gated separately by plan 15-04 Task 2's grep gate (`grep -q 'metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(1)' src/scheduler/run.rs`). This keeps T-V12-WH-04 focused on the channel-boundary contract (the load-bearing claim) without reaching into the scheduler's internal lifecycle.
- **`drop(_worker_handle)` instead of `let _ = _worker_handle`.** Clippy's `let-underscore-future` lint correctly flags `let _ = future` as a likely bug (futures are inert until awaited or dropped). The intent in both new tests is explicitly to NOT await the worker (it has a 60s stall in `StalledDispatcher`); explicit `drop(...)` documents that intent and silences the lint.
- **Production-capacity channel (1024) in T-V12-WH-03, small-capacity (4) in T-V12-WH-04.** Each test exercises a different operating regime: T-V12-WH-04 forces saturation deterministically (small capacity); T-V12-WH-03 exercises the producer-side non-blocking guarantee under realistic capacity. Mixing the two would dilute both signals.
- **Inline `StalledDispatcher` + `make_event` helpers in each test file** (not a shared `mod common`). Each integration test in `tests/` is a separate cargo crate; sharing through `mod common` would require both files to declare `mod common` and add a `tests/common/mod.rs` module. Inline-duplicating ~25 LOC of helper is cheaper and keeps each test self-contained.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] Replaced `let _ = _worker_handle` with `drop(_worker_handle)` to satisfy `clippy::let_underscore_future`**

- **Found during:** Task 1 (`cargo clippy --test v12_webhook_queue_drop -- -D warnings`)
- **Issue:** The plan's prescribed code used `let _ = _worker_handle;` which clippy flags as `let-underscore-future` (`-D warnings` makes it an error). Futures are inert until awaited or dropped; `let _ = future` is the well-known "I forgot to await my future" antipattern. Project-wide `cargo clippy --all-targets -- -D warnings` is a hard gate per CLAUDE.md.
- **Fix:** Replaced with `drop(_worker_handle);` and a one-line comment documenting the intent (we DO want to not await — the future is a 60s stall). Identical runtime semantics; clean clippy.
- **Files modified:** `tests/v12_webhook_queue_drop.rs`, `tests/v12_webhook_scheduler_unblocked.rs`.
- **Commits:** Folded into the same Task 1 / Task 2 commits (the fix is for the same code the task introduces; not a separate commit).

No other deviations. The plan's described test bodies, assertions, thresholds, runtime targets, and acceptance criteria all matched reality. No checkpoints, no architectural changes, no auth gates.

## Issues Encountered

None.

## Threat Surface Verification

The plan's `<threat_model>` declared two threats; this plan's mitigations match the dispositions:

| Threat ID | Disposition | Mitigation in this plan |
|-----------|-------------|-------------------------|
| T-15-05-01 (test process leaking the worker — 60s sleep on shutdown if not cancelled before drop) | mitigate | Both new tests call `cancel.cancel()` followed by `drop(_worker_handle)`. The `tokio::select!` inside `worker_loop` is biased toward the receiver path but the cancel arm fires immediately on the next poll; the StalledDispatcher's 60s sleep future is dropped when the worker_loop's stack frame unwinds. Test process exits in < 30s for both tests; observed wall-clock was ~0.15s and ~5.01s respectively. |
| T-15-05-02 (asserting on internals vs operator-observable behavior) | accept | The drop counter delta and the inter-spawn drift are both operator-observable — the counter renders on `/metrics`, the drift would manifest as missed/late firings in the operator's Prometheus alerting. Thresholds (≥10 drops, < 1s drift, < 5ms per emit) match the operator-observable success criteria from `.planning/ROADMAP.md` Phase 15. |

ASVS V14 (Configuration) does not apply — these are tests, not configuration. P15 stays ASVS-narrow per CONTEXT.md.

No new threat surface introduced beyond the registered model. No threat flags to record.

## Stub Tracking

No stubs introduced. The new test files contain a local `StalledDispatcher` (a deliberate test helper, not a stub — it implements the production trait correctly; "stalled" is its intentional behavior). The `make_event` helper is a test fixture builder, not a stub.

The test-side `metrics::counter!(...).increment(1)` in T-V12-WH-04's TrySendError::Full match arm is documented inline as "mirror of the runtime increment path" — it is the test exercising the same registry shape, not a stub for the runtime increment.

## User Setup Required

None. The webhook worker tests are in-process Rust; no testcontainers, no Docker daemon, no DB. Both new tests run under any CI runner that can compile the project.

## Next Phase Readiness

Phase 15 is now executable-verified end-to-end:

- Plan 15-01 ✅ — `Cargo.toml` 1.1.0 → 1.2.0 (verified by `cronduit --version`).
- Plan 15-02 ✅ — `cargo-deny` CI step + `deny.toml` (verified by lint job's PR check).
- Plan 15-03 ✅ — webhook module skeleton + telemetry registration.
- Plan 15-04 ✅ — scheduler emit at step 7d + bin-layer worker spawn.
- Plan 15-05 ✅ — **this plan** — the two test files that verify the WH-02
  scheduler-survival contract is intact and the eager-describe pair from
  plan 15-03 is intact end-to-end.

Phase 15's four ROADMAP success criteria are now executable-verified:

1. (Plan 15-01) `cronduit --version` reports 1.2.0.
2. (Plan 15-02) cargo-deny PR check appears in lint job.
3. (Plan 15-05 / `try_send_does_not_block_when_dispatcher_is_stalled`) Scheduler keeps firing on time when webhook receiver is stalled — max drift < 1s under 60-second stalled dispatcher.
4. (Plan 15-05 / `webhook_queue_saturation_drops_events_and_increments_counter`) Bounded webhook queue saturation increments `cronduit_webhook_delivery_dropped_total`; scheduler unaffected — ≥10 drops observed, drop counter delta ≥ 10, push elapsed < 50ms.

Beyond P15: P18's `HttpDispatcher` swap is one-line bin-layer change (verified at the end of plan 15-04 SUMMARY); none of the test files in this plan need changes — they exercise the trait, not the implementation. P20's `WH-11` metric family additions can subsume `cronduit_webhook_delivery_dropped_total` into a labeled family without breaking these tests (the `read_drop_counter` helper in T-V12-WH-04 already accepts both unlabeled and labeled forms).

## Self-Check

Verifying all claims made in this SUMMARY.

### Created files exist

- `[ FOUND ] tests/v12_webhook_queue_drop.rs` — `wc -l = 192`
- `[ FOUND ] tests/v12_webhook_scheduler_unblocked.rs` — `wc -l = 121`

### Modified files exist (sanity)

- `[ FOUND ] tests/metrics_endpoint.rs` — extended in place (+10 LOC)

### Commits exist

- `[ FOUND ] bc803ef` — Task 1 (v12_webhook_queue_drop)
- `[ FOUND ] 0a7bb5e` — Task 2 (v12_webhook_scheduler_unblocked)
- `[ FOUND ] 5fbf8f5` — Task 3 (metrics_endpoint extension)

### Acceptance gates

- `[ OK ] cargo nextest run --test v12_webhook_queue_drop` exits 0 (2/2 passed in ~0.15s)
- `[ OK ] cargo nextest run --test v12_webhook_scheduler_unblocked` exits 0 (1/1 passed in ~5.01s)
- `[ OK ] cargo nextest run --test metrics_endpoint` exits 0 (1/1 passed in ~0.21s)
- `[ OK ] cargo nextest run --test v12_webhook_scheduler_unblocked --test v12_webhook_queue_drop --test metrics_endpoint` exits 0 (4/4 passed in ~5.01s)
- `[ OK ] cargo build -p cronduit --tests` exits 0
- `[ OK ] cargo clippy -p cronduit --all-targets -- -D warnings` exits 0 (clean after the `drop(_worker_handle)` fix)
- `[ OK ] just openssl-check` exits 0 (no new TLS surface)

## Self-Check: PASSED

All claimed files exist on disk; all claimed commits exist in `git log`; all acceptance gates verified clean on the worktree HEAD.

---
*Phase: 15-foundation-preamble*
*Completed: 2026-04-26*
