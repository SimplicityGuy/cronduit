---
phase: 15-foundation-preamble
plan: 03
subsystem: webhooks
tags: [webhooks, rust, tokio, async-trait, mpsc, metrics, telemetry, scheduler]

# Dependency graph
requires:
  - phase: 15-foundation-preamble
    provides: Cargo workspace bumped to 1.2.0 (15-01); cargo-deny CI gate at warn (15-02)
provides:
  - "src/webhooks/ module skeleton with Option B layout (mod, event, dispatcher, worker)"
  - "RunFinalized channel-message struct (D-02 minimum payload)"
  - "WebhookDispatcher async trait + NoopDispatcher impl + WebhookError enum (D-01 seam P18 implements against)"
  - "channel() / channel_with_capacity() / spawn_worker() / CHANNEL_CAPACITY=1024 worker entry points"
  - "worker_loop with tokio::select! biased; over rx.recv() and CancellationToken::cancelled()"
  - "async-trait 0.1 promoted from transitive to direct dep (zero compile cost)"
  - "Eager describe + zero-baseline pair for cronduit_webhook_delivery_dropped_total in src/telemetry.rs (D-11)"
affects: [15-04 scheduler-wires-worker-and-emit, 15-05 integration-tests, P16 failure-context, P18 HttpDispatcher, P20 webhook metric family]

# Tech tracking
tech-stack:
  added: ["async-trait 0.1.89 (promoted from transitive)"]
  patterns:
    - "Bounded mpsc + dedicated tokio task (structural copy of src/scheduler/log_pipeline.rs)"
    - "tokio::select! { biased; rx.recv(); cancel.cancelled(); } worker idiom"
    - "Trait-object dispatcher seam (#[async_trait] on trait + every impl)"
    - "Eager describe_counter! + zero-baseline counter!.increment(0) pair (Pitfall 3)"

key-files:
  created:
    - "src/webhooks/mod.rs"
    - "src/webhooks/event.rs"
    - "src/webhooks/dispatcher.rs"
    - "src/webhooks/worker.rs"
  modified:
    - "Cargo.toml (async-trait promoted to direct dep)"
    - "Cargo.lock (no new resolved versions; same single async-trait@0.1.89 entry)"
    - "src/lib.rs (pub mod webhooks; registration)"
    - "src/telemetry.rs (drop counter eager describe + zero-baseline)"

key-decisions:
  - "Option B module layout (split mod/event/dispatcher/worker from day one) to avoid the rename diff in P18"
  - "channel_with_capacity() exposed as pub (NOT #[cfg(test)]-gated) so integration tests in tests/ can reach it — they are separate crates"
  - "WebhookError single DispatchFailed(String) stub variant — P18 expands when HttpDispatcher arrives"
  - "Drop counter is unlabeled (zero labels in P15 per D-11); P20/WH-11 lands the labeled cronduit_webhook_* family separately"
  - "tokio::select! biased; gives recv arm priority over cancel — prevents tight-cancel-loop starvation of in-flight deliveries"
  - "tracing target 'cronduit.webhooks' for all webhook-related log events (D-04)"

patterns-established:
  - "Webhooks module convention mirrors src/scheduler/ split (mod.rs + event.rs + dispatcher.rs + worker.rs)"
  - "Public re-exports flatten the surface so integration tests use cronduit::webhooks::{NoopDispatcher, RunFinalized, …} (mirrors cronduit::scheduler::cmd::SchedulerCmd)"
  - "Mermaid-only diagrams in module doc-comments (D-14)"

requirements-completed: [WH-02]

# Metrics
duration: 9min
completed: 2026-04-26
---

# Phase 15 Plan 03: Webhook delivery worker scaffold (WH-02) Summary

**`src/webhooks/` module with `WebhookDispatcher` async trait + `NoopDispatcher` + bounded mpsc(1024) worker, plus `cronduit_webhook_delivery_dropped_total` eager-described from boot — the in-process foundation P18's HttpDispatcher will fill in.**

## Performance

- **Duration:** ~9 minutes
- **Started:** 2026-04-26T21:56:16Z
- **Completed:** 2026-04-26T22:05:20Z
- **Tasks:** 3
- **Files modified:** 6 (4 created + 2 modified + Cargo.lock churn)

## Accomplishments

- `src/webhooks/` module with Option B split (mod / event / dispatcher / worker) compiles in isolation
- `WebhookDispatcher` async trait locks the seam P18 implements against (D-01); `NoopDispatcher` ships as the always-on default
- `RunFinalized` channel-message struct carries the D-02 minimum payload (`run_id`, `job_id`, `job_name`, `status`, `exit_code`, `started_at`, `finished_at`)
- `worker_loop` uses `tokio::select! { biased; rx.recv(); cancel.cancelled(); }` — recv arm priority prevents cancel-token starvation of in-flight deliveries
- `channel()` returns the `mpsc::channel(1024)` pair; `channel_with_capacity(usize)` exposed as `pub` so integration tests can force `TrySendError::Full` synchronously
- `async-trait = "0.1"` promoted from transitive (Cargo.lock:237 via testcontainers + bollard/tonic) to direct dep — zero new compile cost
- `cronduit_webhook_delivery_dropped_total` eagerly described AND zero-baseline registered in `src/telemetry.rs` (D-11) — `/metrics` will render HELP/TYPE lines from boot, before any drop event fires (Pitfall 3 prevention)

## Task Commits

Each task was committed atomically:

1. **Task 1: Promote `async-trait` to direct dep** — `086c546` (feat)
2. **Task 2: Create `src/webhooks/` module + register in `src/lib.rs`** — `11e7a34` (feat)
3. **Task 3: Eager-describe + zero-baseline `cronduit_webhook_delivery_dropped_total`** — `2ffd632` (feat)

## Files Created/Modified

### Created (4)
- `src/webhooks/mod.rs` — Module root: declares `event` / `dispatcher` / `worker` submodules and re-exports the public surface (`NoopDispatcher`, `WebhookDispatcher`, `WebhookError`, `RunFinalized`, `channel`, `channel_with_capacity`, `spawn_worker`, `CHANNEL_CAPACITY`). Includes mermaid dataflow diagram in the module doc-comment (D-14).
- `src/webhooks/event.rs` — `RunFinalized` channel-message struct (`#[derive(Debug, Clone)]`, all-pub fields). Comment explicitly distinguishes channel-message contract (here) from wire-format payload (`P18 / WH-03`).
- `src/webhooks/dispatcher.rs` — `WebhookDispatcher` async trait (`Send + Sync` bounds), `NoopDispatcher` impl logging at `tracing::debug!` and returning `Ok(())`, `WebhookError::DispatchFailed(String)` enum stub for P18. `#[async_trait]` macro on BOTH trait declaration AND impl block (Pitfall 2 — Rust 1.94.1 native dyn async-fn-in-trait is not object-safe).
- `src/webhooks/worker.rs` — `CHANNEL_CAPACITY: usize = 1024` const, `channel()` + `channel_with_capacity()` + `spawn_worker()` constructors, `worker_loop` async fn with `tokio::select!` `biased;` priority on `rx.recv()` over `cancel.cancelled()`. Logs at `info` level on both clean-exit paths (channel closed vs cancel fired); logs at `warn` level if the dispatcher returns `Err`.

### Modified (2 source + Cargo manifest)
- `Cargo.toml` — Added `async-trait = "0.1"` between `# Errors` and `# Secrets` blocks with explanatory comment block. Pinned to the `0.1` line (not the exact `0.1.89` already-resolved version) so cargo can pick up future patch updates.
- `Cargo.lock` — Reflects the new `[dependencies]` entry but keeps a single resolved `async-trait@0.1.89` (no duplicates introduced).
- `src/lib.rs` — Added `pub mod webhooks;` line after `pub mod web;` (alphabetical) with phase-pointer comment.
- `src/telemetry.rs` — Added `metrics::describe_counter!("cronduit_webhook_delivery_dropped_total", …)` in the describe block (between `cronduit_run_failures_total` and `cronduit_docker_reachable`), and the paired `metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(0)` in the zero-baseline block. Help text references closed-cardinality (no labels in P15), the `cronduit.webhooks` tracing target, and the future `P20 / WH-11` labeled family.

## Decisions Made

- **Option B module layout from day one** (per CONTEXT.md "Claude's Discretion"). Splits into `mod.rs` + `event.rs` + `dispatcher.rs` + `worker.rs` even though P15's surface is small — avoids the rename diff in P18 when `payload.rs` and `http.rs` arrive. Mirrors the `src/scheduler/` precedent in this codebase.
- **`channel_with_capacity()` exposed as `pub`** rather than `#[cfg(test)]`-gated. Integration tests in `tests/` are separate crates and cannot reach `#[cfg(test)]` items. The function is a single-line wrapper around `mpsc::channel(cap)`, so the public-surface cost is negligible.
- **Single `WebhookError::DispatchFailed(String)` stub variant.** P18 will expand the enum (likely to add `HttpStatus(u16, String)`, `Timeout`, `InvalidUrl`, etc. — that is P18's call). Keeping P15 lean reduces the seam P18 has to keep stable.
- **`async-trait` pinned to `"0.1"` (not `"0.1.89"`).** Lets cargo pick up patch-level updates within the 0.1 line; the transitive constraint determines what actually resolves. `grep -c '^name = "async-trait"$' Cargo.lock` remains `1` post-promotion.
- **Drop counter description is forward-pointing** (mentions `P20 / WH-11`). Operator dashboards built against rc.1 will see this counter and the description will tell them where the rest of the family is coming from. The unlabeled rc.1 counter and the labeled P20 family (`cronduit_webhook_deliveries_total{job, status="dropped"}`) coexist on `/metrics` without confusion — different cardinalities by design.

## Deviations from Plan

None — plan executed exactly as written. Each acceptance criterion in `15-03-PLAN.md` was verified before each commit (`grep`-based structural checks, `cargo build`, `cargo clippy`, `cargo test --lib`). No auto-fix rules triggered; no checkpoints; no architectural changes proposed.

The one minor procedural quirk: my first Edit attempt against `Cargo.toml` modified the main repository (`/Users/Robert/Code/public/cronduit/Cargo.toml`) instead of the worktree path. I caught this immediately via `git status` showing "working tree clean" inside the worktree, reverted the main-repo edit (`git checkout -- Cargo.toml`), and re-applied the change against the correct worktree path (`/Users/Robert/Code/public/cronduit/.claude/worktrees/agent-a32a5e82470bb4cf5/Cargo.toml`). Net result: the main repo is back to its original state and only the worktree branch carries the plan's changes — exactly the parallel-worktree contract. No code or commit history was harmed.

## Issues Encountered

None.

## Threat Surface Verification

The plan's `<threat_model>` declared three threats; this plan's mitigations match the dispositions:

| Threat ID | Disposition | Mitigation in this plan |
|-----------|-------------|-------------------------|
| T-15-03-01 (DoS via slow dispatcher) | accept | `tokio::select! biased;` on rx.recv() vs cancel — slow dispatcher slows the worker but cannot stall the scheduler. Producer-side guard (`try_send` + drop on full) lands in plan 15-04. |
| T-15-03-02 (drop counter HELP/TYPE absent until first drop — Pitfall 3) | mitigate | Both `describe_counter!` AND `counter!(…).increment(0)` present in `src/telemetry.rs` (verified via grep). Plan 15-05 will extend `tests/metrics_endpoint.rs::metrics_families_described_from_boot` to assert the boot-time HELP/TYPE lines. |
| T-15-03-03 (native dyn async-fn-in-trait fails on Rust 1.94.1 — Pitfall 2) | mitigate | `#[async_trait]` macro on BOTH the trait declaration AND the `NoopDispatcher` impl block (verified by grep returning 2). `async-trait` pinned at the crate boundary (Task 1). |

No new threat surface introduced beyond the registered model (in-process trait object boundary + tokio mpsc producer/consumer). No threat flags to record.

## Stub Tracking

`NoopDispatcher::deliver` returns `Ok(())` after a single `tracing::debug!` line. This is **intentional** per CONTEXT.md D-01: P15 ships only the no-op default; P18's `HttpDispatcher` fills in the trait against the same seam. Not a true stub — it is the always-on default behavior when no webhooks are configured.

`WebhookError::DispatchFailed(String)` is the only variant. **Intentional** per CONTEXT.md "Claude's Discretion" — P18 expands the enum when HTTP arrives.

No UI surfaces added. No data-rendering paths created. Nothing the verifier would flag as "wired to empty data".

## User Setup Required

None — no external service configuration. The webhook worker is in-process Rust code; `NoopDispatcher` requires zero configuration. P18 will introduce the `[webhooks]` TOML schema and HTTP delivery — at that point operators will need to configure `webhook = "https://…"` per-job or `[defaults].webhook`.

## Next Phase Readiness

Plan 15-04 (scheduler integration) can proceed. The contract it integrates against is fully locked here:

- Construct the channel pair: `let (webhook_tx, webhook_rx) = cronduit::webhooks::channel();`
- Spawn the worker at startup: `cronduit::webhooks::spawn_worker(webhook_rx, Arc::new(NoopDispatcher), cancel.child_token())`
- Wire `webhook_tx` into `SchedulerLoop` as a non-`Option` field (D-03 always-on)
- Emit at `finalize_run` step 7d (NEW — between current 7c sentinel broadcast and current 7d active_runs cleanup, which becomes 7e per RESEARCH.md Pitfall 6)
- On `TrySendError::Full`: `metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(1)` + `tracing::warn!(target: "cronduit.webhooks", …)`
- On `TrySendError::Closed`: `tracing::error!(target: "cronduit.webhooks", …)`

Plan 15-05 (integration tests) can also proceed — `cronduit::webhooks::{NoopDispatcher, RunFinalized, WebhookDispatcher, WebhookError, channel_with_capacity, spawn_worker}` are all reachable as a public surface.

## Self-Check

Verifying all claims made in this SUMMARY.

### Created files exist

- `[ FOUND ] src/webhooks/mod.rs`
- `[ FOUND ] src/webhooks/event.rs`
- `[ FOUND ] src/webhooks/dispatcher.rs`
- `[ FOUND ] src/webhooks/worker.rs`

### Commits exist

- `[ FOUND ] 086c546` — Task 1 (async-trait promotion)
- `[ FOUND ] 11e7a34` — Task 2 (webhooks module scaffold)
- `[ FOUND ] 2ffd632` — Task 3 (telemetry drop counter)

## Self-Check: PASSED

All claimed files exist on disk; all claimed commits exist in `git log`. `cargo build`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --lib` (194 tests) all pass on the worktree HEAD.

---
*Phase: 15-foundation-preamble*
*Completed: 2026-04-26*
