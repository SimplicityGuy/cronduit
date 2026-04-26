---
phase: 15-foundation-preamble
plan: 04
subsystem: webhooks
tags: [webhooks, scheduler, rust, integration, tokio, mpsc]

# Dependency graph
requires:
  - phase: 15-foundation-preamble
    provides: "src/webhooks/ module skeleton with NoopDispatcher + RunFinalized + spawn_worker (15-03)"
provides:
  - "SchedulerLoop has a non-Option `webhook_tx: tokio::sync::mpsc::Sender<RunFinalized>` field (D-03 always-on)"
  - "scheduler::spawn(...) accepts and stores the webhook_tx parameter; struct literal carries it"
  - "All 6 production run_job / run_job_with_existing_run_id call sites pass self.webhook_tx.clone()"
  - "run_job, run_job_with_existing_run_id, and continue_run signatures carry webhook_tx as their last parameter"
  - "finalize_run step 7d emits RunFinalized via try_send AFTER step 7c sentinel broadcast and BEFORE the renumbered step 7e cleanup"
  - "Drop semantics wired per D-04: TrySendError::Full → warn + counter increment; TrySendError::Closed → error"
  - "Bin layer (src/cli/run.rs) constructs the channel via crate::webhooks::channel(), spawns the worker with NoopDispatcher + child cancel token, and awaits the worker JoinHandle AFTER the scheduler drains"
  - "Step numbering hygiene: exactly one `// 7d.` (NEW webhook emit) and exactly one `// 7e.` (renumbered cleanup) — Pitfall 6 enforced"
affects: [15-05 integration-tests, P18 HttpDispatcher swap, P20 webhook metric family + drain accounting]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SchedulerLoop carries an mpsc::Sender field analogous to its mpsc::Receiver (cmd_rx)"
    - "Per-run mpsc::Sender clone at every spawn site (mirrors self.pool.clone() / self.active_runs.clone())"
    - "try_send + match on TrySendError::Full / TrySendError::Closed in the producer-side scheduler path (Pitfall 1 / Pitfall 28 prevention)"
    - "Bin-layer worker lifetime hierarchy: bin owns the JoinHandle + parent cancel; scheduler drops Sender clones; worker awaited AFTER scheduler drains (mirrors v1.0 graceful-shutdown for the log pipeline)"

key-files:
  created: []
  modified:
    - "src/scheduler/mod.rs (webhook_tx field on SchedulerLoop, spawn signature, 6 production call sites, 2 test-site channel constructions)"
    - "src/scheduler/run.rs (run_job + run_job_with_existing_run_id + continue_run signatures, NEW step 7d webhook emit block, renumber 7d→7e, 5 lib-test channel constructions)"
    - "src/cli/run.rs (channel construction + spawn_worker(NoopDispatcher, cancel.child_token()) + scheduler::spawn arg + JoinHandle await ordered AFTER scheduler drains)"
    - "tests/scheduler_integration.rs (test_webhook_tx() helper + 6 call site updates)"
    - "tests/stop_executors.rs (test_webhook_tx() helper + 3 call site updates)"
    - "tests/metrics_stopped.rs (inline per-test webhook channel + 1 call site update)"

key-decisions:
  - "test_webhook_tx() helpers added to integration test files (scheduler_integration.rs, stop_executors.rs) so the new arg threads cleanly through ~10 call sites; metrics_stopped.rs uses an inline channel since it has only one call site"
  - "continue_run gets #[allow(clippy::too_many_arguments)] (8 params, one over the default 7-threshold) because a SchedulerSpawnConfig-style refactor is out of P15 scope and the related run_job/run_job_with_existing_run_id wrappers stay at exactly 7 params"
  - "Bin-layer wiring committed in two stages within plan 15-04: Task 1 lands a provisional channel-only construction (no worker, dropped Receiver) so cargo build passes after Task 1's signature change in isolation; Task 3 replaces it with the real spawn_worker + JoinHandle await"
  - "Inline-comment phrasing in step 7d avoids the literal string `webhook_tx.send(` so the Pitfall-1 grep guard (`! grep -rq 'webhook_tx\\.send(' src/`) stays clean even against the prohibition comment itself"

patterns-established:
  - "Step 7d → 7e renumber + insert pattern locked: any future per-run side-effect emission slots in BEFORE the active_runs cleanup step, with the comment-numbered marker preserved"
  - "Producer-side try_send + drop-on-full counter increment as the canonical scheduler-survival contract (replicated for any future bounded-channel dataflow out of finalize_run)"

requirements-completed: [WH-02]

# Metrics
duration: 21min
completed: 2026-04-26
---

# Phase 15 Plan 04: Scheduler webhook wiring + step 7d emit (WH-02) Summary

**Scheduler now emits RunFinalized into the bounded webhook channel at finalize_run step 7d via try_send, with the always-on NoopDispatcher worker spawned at the bin layer — the in-process webhook delivery path is live end-to-end with the scheduler-survival contract enforced by drop-on-full semantics.**

## Performance

- **Duration:** ~21 minutes
- **Started:** 2026-04-26T22:11:53Z
- **Completed:** 2026-04-26T22:32:12Z
- **Tasks:** 3
- **Files modified:** 6 (3 src/ + 3 tests/)

## Accomplishments

- `SchedulerLoop` carries the always-on `webhook_tx: mpsc::Sender<RunFinalized>` field (D-03); `scheduler::spawn(...)` takes and stores it as the new last argument
- All 6 production `run_job` / `run_job_with_existing_run_id` call sites in `src/scheduler/mod.rs` pass `self.webhook_tx.clone()` (catch-up, scheduled, manual `RunNow`, manual `RunNowWithRunId`, plus the two drained variants in the reload-coalescing path)
- `finalize_run` step 7d emits `RunFinalized` into the channel via `try_send` after the step 7c sentinel broadcast and before the renumbered step 7e active_runs cleanup; drop semantics wired per D-04 (Full → warn + `cronduit_webhook_delivery_dropped_total.increment(1)`; Closed → error)
- Bin layer (`src/cli/run.rs`) constructs the channel via `crate::webhooks::channel()`, spawns the always-on `NoopDispatcher`-backed worker with `cancel.child_token()`, and awaits the worker's `JoinHandle` AFTER the scheduler drains (lifetime hierarchy per RESEARCH.md Open Question 2)
- Step-numbering hygiene enforced: exactly one `// 7d.` (NEW webhook emit) and exactly one `// 7e.` (renumbered cleanup) — Pitfall 6 prevention via grep gates in the acceptance criteria
- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib` (194 passed), and `just openssl-check` all green throughout each task commit

## Task Commits

Each task was committed atomically:

1. **Task 1: Thread `webhook_tx` through SchedulerLoop and run_job** — `1d78268` (feat)
2. **Task 2: Insert step 7d webhook emit + renumber existing 7d→7e** — `f1e210b` (feat)
3. **Task 3: Wire webhook worker into bin layer with NoopDispatcher** — `98f0abb` (feat)

## Files Created/Modified

### Modified (6)

- `src/scheduler/mod.rs` — Added `webhook_tx: tokio::sync::mpsc::Sender<crate::webhooks::RunFinalized>` field on `SchedulerLoop`. Extended `spawn(...)` to take and store the new parameter. Threaded `self.webhook_tx.clone()` through six production call sites. Updated the two `#[cfg(test)] mod tests` invocations (`shutdown_drain_completes_within_grace`, `shutdown_grace_expiry_force_kills`) to construct per-test channels via `crate::webhooks::channel_with_capacity(8)` and pass `webhook_tx_test` as the new last positional argument.
- `src/scheduler/run.rs` — Added `webhook_tx` parameter to `run_job`, `run_job_with_existing_run_id`, and the shared `continue_run` helper (the latter gets `#[allow(clippy::too_many_arguments)]`). Inserted the new step 7d block (`webhook_tx.try_send(RunFinalized { ... })` with the matched `TrySendError::Full` warn + counter and `TrySendError::Closed` error arms) and renumbered the existing step 7d (active_runs cleanup) to step 7e. Updated five lib-tests (`run_job_command_success`, `run_job_script_success`, `run_job_timeout_preserves_partial_logs`, `run_job_with_existing_run_id_skips_insert`, `concurrent_runs_create_separate_rows`) to construct per-test webhook channels.
- `src/cli/run.rs` — Replaced the Task-1 placeholder channel-only construction with the real always-on worker setup: `crate::webhooks::channel()` for the pair, `crate::webhooks::spawn_worker(webhook_rx, std::sync::Arc::new(crate::webhooks::NoopDispatcher), cancel.child_token())` for the JoinHandle, the new positional `webhook_tx` argument on `crate::scheduler::spawn(...)`, and `let _ = webhook_worker_handle.await;` ordered AFTER `let _ = scheduler_handle.await;`.
- `tests/scheduler_integration.rs` — Added a `test_webhook_tx()` helper (constructs an mpsc::channel(8) with the Receiver dropped) and threaded a `test_webhook_tx()` arg through six `run_job` invocations covering command, script, fail, timeout, sync-disable, and concurrent variants.
- `tests/stop_executors.rs` — Added a `test_webhook_tx()` helper and threaded the new arg through three `run_job` invocations (command/script via `replace_all` on the identical pattern; docker via a targeted edit because the Docker arg differs).
- `tests/metrics_stopped.rs` — Added an inline per-test webhook channel construction in the single `run_job` call site (the test file has only one).

## Decisions Made

- **`test_webhook_tx()` helpers in cross-crate test files.** Adding the new positional argument cleanly through ~10 integration-test call sites needed a one-line wrapper. Each helper constructs `cronduit::webhooks::channel_with_capacity(8)` and immediately drops the receiver — `finalize_run`'s `try_send` then returns `TrySendError::Closed` on every call (logged at error per D-04), but the tests do not assert on webhook behavior so the noise is harmless. `metrics_stopped.rs` has a single call site and uses an inline channel rather than a helper.
- **`#[allow(clippy::too_many_arguments)]` on `continue_run` only.** `continue_run` now has 8 parameters (one over clippy's default 7-threshold). The wrapper functions `run_job` (7 params) and `run_job_with_existing_run_id` (7 params) stay at exactly the threshold. A `RunCtx` / `SchedulerSpawnConfig` refactor is explicitly P21+ scope per the plan's wording on `spawn(...)`.
- **Two-stage bin-layer wiring within plan 15-04.** Task 1 lands a provisional channel-only construction in `src/cli/run.rs` (channel pair built, Receiver bound to `_webhook_rx` so it isn't dropped) so `cargo build` passes after Task 1's signature change in isolation. Task 3 replaces it with the real `spawn_worker(NoopDispatcher, cancel.child_token())` setup and the JoinHandle await ordered AFTER the scheduler drains. The plan's Task 1 verification gate (`cargo build -p cronduit` exits 0) made this two-stage approach mandatory.
- **Comment phrasing avoids the literal string `webhook_tx.send(`.** The plan's Task 2 acceptance criterion `! grep -q 'webhook_tx.send(' src/scheduler/run.rs` would otherwise fail against the prohibition comment itself ("NEVER use `webhook_tx.send().await`"). Reworded to "NEVER use the awaiting `send().await` form on this Sender" preserves the meaning while keeping the grep gate green. This is documented as Decision-04 above for downstream agents who need to extend the comment.

## Deviations from Plan

None — plan executed as written. Each task's acceptance criteria were verified before commit (`grep`-based structural checks, `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p cronduit --lib`). No auto-fix rules triggered; no checkpoints; no architectural changes proposed.

The plan's call-site count and step-numbering invariants matched reality exactly: 6 production `run_job` call sites in `mod.rs`, 2 test-site invocations in `mod.rs`'s `#[cfg(test)]` block, and the existing step layout `// 7. → // 7b. → // 7c. → // 7d.` was renumbered cleanly to `// 7. → // 7b. → // 7c. → // 7d. (NEW) → // 7e. (was 7d)`.

The cross-crate integration tests (`tests/scheduler_integration.rs`, `tests/stop_executors.rs`, `tests/metrics_stopped.rs`) needed the same arg threaded through ~10 additional call sites — the plan's `<files>` spec scoped src-only, but cross-crate test compilation is implicit in the `cargo clippy --all-targets` and `cargo build -p cronduit` verification gates, so these updates land in the same Task 1 commit as the `mod.rs` edits. This is not a deviation; it is the natural scope expansion of "must compile under all-targets".

## Issues Encountered

None.

## Threat Surface Verification

The plan's `<threat_model>` declared four threats; this plan's mitigations match the dispositions:

| Threat ID | Disposition | Mitigation in this plan |
|-----------|-------------|-------------------------|
| T-15-04-01 (DoS via `webhook_tx.send().await`) | mitigate | Step 7d block uses `try_send` exclusively. `! grep -rq 'webhook_tx\.send(' src/` returns OK after Task 2. The integration test in plan 15-05 will stress this end-to-end. |
| T-15-04-02 (two `// 7d` headers — Pitfall 6) | mitigate | `grep -c '// 7d\.' src/scheduler/run.rs` returns exactly `1`; `grep -c '// 7e\.' src/scheduler/run.rs` returns exactly `1`. Renumber and insertion landed in the same Task 2 commit. |
| T-15-04-03 (worker awaited before scheduler — race) | mitigate | `awk '/scheduler_handle\.await/{a=NR} /webhook_worker_handle\.await/{b=NR; if(a&&b&&b>a) print "OK"; exit}' src/cli/run.rs` returns OK. Worker JoinHandle is strictly after scheduler drain. |
| T-15-04-04 (drop counter coexists with future P20 labeled family) | accept | No code change in this plan; the coexistence is documented in plan 15-03's telemetry registration HELP text. |

No new threat surface introduced beyond the registered model. No threat flags to record.

## Stub Tracking

No new stubs introduced. The bin-layer wiring uses `NoopDispatcher` per CONTEXT.md D-01/D-03 — that is the always-on default behavior in P15, not a stub. P18's `HttpDispatcher` will swap in against the same trait without touching the worker loop or the scheduler-side emit path. No UI surfaces added in this plan.

## User Setup Required

None. The webhook worker is in-process Rust; `NoopDispatcher` requires zero configuration. Operators will configure `webhook = "https://…"` per-job or `[defaults].webhook` once P18 introduces the `[webhooks]` TOML schema and the HTTP delivery path.

## Next Phase Readiness

Plan 15-05 (integration tests for the scheduler-survival contract under saturation and stalled-receiver scenarios) can proceed. The full producer→channel→worker→dispatcher pipeline is reachable from integration tests as a public surface:

- Construct a stalled dispatcher (impl `WebhookDispatcher` returning `Ok(())` after a long sleep).
- Wire it in via `crate::webhooks::spawn_worker(rx, Arc::new(StalledDispatcher), cancel)` against a small-capacity channel built with `crate::webhooks::channel_with_capacity(4)`.
- Push events via `tx.try_send(...)` and assert `cronduit_webhook_delivery_dropped_total` increments.
- Spawn a real `SchedulerLoop` and assert spawn-cadence remains < 2s under a stalled dispatcher (T-V12-WH-03).

Beyond P15: P18's `HttpDispatcher` swap is a one-line change at `src/cli/run.rs` — replace `Arc::new(crate::webhooks::NoopDispatcher)` with the new HTTP impl and the rest of the pipeline is unchanged.

## Self-Check

Verifying all claims made in this SUMMARY.

### Created files exist

(no files created)

### Modified files exist (sanity)

- `[ FOUND ] src/scheduler/mod.rs`
- `[ FOUND ] src/scheduler/run.rs`
- `[ FOUND ] src/cli/run.rs`
- `[ FOUND ] tests/scheduler_integration.rs`
- `[ FOUND ] tests/stop_executors.rs`
- `[ FOUND ] tests/metrics_stopped.rs`

### Commits exist

- `[ FOUND ] 1d78268` — Task 1 (thread webhook_tx through SchedulerLoop and run_job)
- `[ FOUND ] f1e210b` — Task 2 (emit RunFinalized at finalize_run step 7d)
- `[ FOUND ] 98f0abb` — Task 3 (wire webhook worker into bin layer with NoopDispatcher)

### Acceptance gates

- `[ OK ] grep -c '// 7d\.' src/scheduler/run.rs` returns `1`
- `[ OK ] grep -c '// 7e\.' src/scheduler/run.rs` returns `1`
- `[ OK ] ! grep -rq 'webhook_tx\.send(' src/` exits 0 (no `.send().await` anywhere — Pitfall 1)
- `[ OK ] awk` ordering check on `src/cli/run.rs` confirms scheduler awaited before worker
- `[ OK ] cargo build -p cronduit` exits 0
- `[ OK ] cargo clippy -p cronduit --all-targets -- -D warnings` exits 0
- `[ OK ] cargo test -p cronduit --lib` exits 0 (194 passed)
- `[ OK ] just openssl-check` exits 0 (no new TLS surface)

## Self-Check: PASSED

All claimed files exist on disk; all claimed commits exist in `git log`; all acceptance gates verified clean on the worktree HEAD.

---
*Phase: 15-foundation-preamble*
*Completed: 2026-04-26*
