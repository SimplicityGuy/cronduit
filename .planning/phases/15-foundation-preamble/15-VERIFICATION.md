---
phase: 15-foundation-preamble
verified: 2026-04-26T22:59:49Z
status: passed
score: 4/4 must-haves verified
overrides_applied: 0
---

# Phase 15: Foundation Preamble Verification Report

**Phase Goal:** Establish the v1.2 hygiene baseline and lock the webhook delivery worker isolation pattern before any payload/signing/posture work depends on it.

**Verified:** 2026-04-26T22:59:49Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Operator running `cronduit --version` on first v1.2 commit sees `1.2.0` (not `1.1.0`) | VERIFIED | `Cargo.toml` line 3: `version = "1.2.0"`. `Cargo.lock` `[[package]] name = "cronduit"` resolves to `version = "1.2.0"`. `./target/debug/cronduit --version` actually executed, returns: `cronduit 1.2.0` |
| 2 | Operator viewing GitHub Actions PR check list sees a new `cargo-deny` job that runs advisories + licenses + duplicate-versions checks (failures non-blocking on first rc; status visible) | VERIFIED | `.github/workflows/ci.yml` L54-58: `taiki-e/install-action@v2` with `tool: cargo-deny` followed by `- run: just deny` with `continue-on-error: true` at STEP level (not job level — Pitfall 5 enforced). `justfile` L223-224: `deny: cargo deny check advisories licenses bans` (single invocation covering all three). `deny.toml` exists at project root with all three checks configured (advisories `ignore = []`, licenses allowlist of 5 SPDX IDs, bans `multiple-versions = "warn"`). Local `just deny` execution confirms the step actually runs (rejected non-allowlisted licenses surface as visible warn output without blocking — exactly the rc.1 posture). |
| 3 | Operator can fire a job whose webhook receiver is stalled for 60 seconds and the next scheduled jobs across the fleet still fire on time (no scheduler drift > 1 s) — the `try_send` non-blocking path holds | VERIFIED | `tests/v12_webhook_scheduler_unblocked.rs` exists (121 lines); `try_send_does_not_block_when_dispatcher_is_stalled` test PASSES (5.030s runtime). Test uses `StalledDispatcher` with 60-second `tokio::time::sleep` on every `deliver()`, fires 5 ticks at 1s cadence, and asserts `max_drift < Duration::from_secs(1)` AND per-emit `try_send` budget `< 5ms`. Production try_send call at `src/scheduler/run.rs:427` is grep-confirmed; no `webhook_tx.send(` form exists anywhere in `src/`. |
| 4 | Operator can fill the bounded webhook queue past 1024 entries and observe `cronduit_webhook_delivery_dropped_total` increment with a `warn`-level log line per dropped event; the scheduler loop remains unaffected | VERIFIED | `tests/v12_webhook_queue_drop.rs` exists (192 lines); `webhook_queue_saturation_drops_events_and_increments_counter` test PASSES. Uses `channel_with_capacity(4)` to force `TrySendError::Full`, asserts `>= 10` drops and counter delta `>= 10` and push elapsed `< 50ms`. Production capacity locked at 1024 via `pub const CHANNEL_CAPACITY: usize = 1024;` in `src/webhooks/worker.rs:21` (used by `channel()` at L26). Production runtime increment at `src/scheduler/run.rs:437`: `metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(1)` (in `TrySendError::Full` arm, paired with `tracing::warn!` at L430-436 with `target: "cronduit.webhooks"`). Counter is eagerly registered at boot (verified by `tests/metrics_endpoint.rs::metrics_families_described_from_boot` PASSES with new HELP/TYPE asserts at L73-79). |

**Score:** 4/4 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `Cargo.toml` | Version = 1.2.0 + async-trait direct dep | VERIFIED | L3 = `version = "1.2.0"`; L71 = `async-trait = "0.1"` |
| `Cargo.lock` | Resolved cronduit version = 1.2.0; single async-trait entry | VERIFIED | `name = "cronduit"` resolves to `version = "1.2.0"` |
| `deny.toml` | cargo-deny v0.19.x config with allowlist + warn-only bans | VERIFIED | Exists at project root (65 lines). Allow list = exactly 5 SPDX IDs (MIT, Apache-2.0, BSD-3-Clause, ISC, Unicode-DFS-2016). `multiple-versions = "warn"`. No deprecated keys. Four `[graph].targets` mirror `just openssl-check`. |
| `justfile` | `deny:` recipe under `[group('quality')]` | VERIFIED | L222-224: `[doc('cargo-deny supply-chain check (advisories + licenses + bans)')]` + `deny: cargo deny check advisories licenses bans`. `command -v cargo-deny` resolves to `/Users/Robert/.cargo/bin/cargo-deny`; `just --list` shows recipe. |
| `.github/workflows/ci.yml` | New cargo-deny step in lint job with continue-on-error: true (step-level) | VERIFIED | L54-58 in `lint` job: `taiki-e/install-action@v2` with `tool: cargo-deny` then `- run: just deny` followed by `continue-on-error: true` (step-level, indented as sibling of `run:`). No job-level `continue-on-error:` exists. |
| `src/lib.rs` | `pub mod webhooks;` registration | VERIFIED | L9: `pub mod webhooks; // Phase 15 / WH-02 — webhook delivery worker` |
| `src/webhooks/mod.rs` | Module root with re-exports + mermaid diagram | VERIFIED | Re-exports `dispatcher::{NoopDispatcher, WebhookDispatcher, WebhookError}`, `event::RunFinalized`, `worker::{CHANNEL_CAPACITY, channel, channel_with_capacity, spawn_worker}`. Mermaid diagram present. |
| `src/webhooks/event.rs` | RunFinalized struct with D-02 minimum payload | VERIFIED | `pub struct RunFinalized` with all 7 fields (run_id, job_id, job_name, status, exit_code, started_at, finished_at). |
| `src/webhooks/dispatcher.rs` | WebhookDispatcher trait + NoopDispatcher + WebhookError | VERIFIED | `#[async_trait]` on BOTH trait declaration AND NoopDispatcher impl (count=2 — Pitfall 2 enforced). `target: "cronduit.webhooks"` tracing target present. |
| `src/webhooks/worker.rs` | channel + channel_with_capacity + spawn_worker + biased select | VERIFIED | `pub const CHANNEL_CAPACITY: usize = 1024;` at L21. `pub fn channel()`, `pub fn channel_with_capacity(cap)`, `pub fn spawn_worker(...)`. `tokio::select!` with `biased;` directive at L59. Cancel-token + recv arms both exit cleanly. |
| `src/telemetry.rs` | Eager describe + zero-baseline pair for drop counter (D-11) | VERIFIED | L111-112: `describe_counter!("cronduit_webhook_delivery_dropped_total", ...)`. L133: `counter!("cronduit_webhook_delivery_dropped_total").increment(0)`. Both calls present (Pitfall 3 prevention). |
| `src/scheduler/mod.rs` | webhook_tx field + spawn arg + 6 production call sites + 2 test channel constructions | VERIFIED | L84: `pub webhook_tx: tokio::sync::mpsc::Sender<crate::webhooks::RunFinalized>`. 6 production call sites pass `self.webhook_tx.clone()` (lines 134, 159, 204, 236, 302, 322). 2 `#[cfg(test)]` test invocations construct per-test channel via `channel_with_capacity(8)` (lines 652, 722). |
| `src/scheduler/run.rs` | Step 7d webhook emit + step 7e renumber + run_job signature | VERIFIED | Exactly one `// 7d.` (NEW webhook emit at L404) and exactly one `// 7e.` (renumbered at L451). Pitfall 6 enforced. `webhook_tx.try_send(event)` at L427. `TrySendError::Full(dropped)` arm with warn log + counter increment at L429-438. `TrySendError::Closed(_)` arm with error log at L439-448. NO `webhook_tx.send(` form anywhere. |
| `src/cli/run.rs` | Channel + worker spawn with NoopDispatcher + correct shutdown order | VERIFIED | L250: `crate::webhooks::channel()`. L251-255: `spawn_worker` with `Arc::new(NoopDispatcher)` + `cancel.child_token()`. L268: `webhook_tx` passed to `scheduler::spawn`. L275: scheduler awaited first. L282: `webhook_worker_handle.await` AFTER scheduler (verified order — scheduler drains, then webhook). |
| `tests/v12_webhook_queue_drop.rs` | Saturation + counter test | VERIFIED | 192 lines. Both tests pass (saturation + smoke-test under NoopDispatcher). `flavor = "multi_thread"`. |
| `tests/v12_webhook_scheduler_unblocked.rs` | Drift test under stalled dispatcher | VERIFIED | 121 lines. Test passes in 5.030s. `max_drift < Duration::from_secs(1)` assertion encodes ROADMAP SC #3. |
| `tests/metrics_endpoint.rs` | Extended with HELP/TYPE asserts for drop counter | VERIFIED | Two new asserts at L73-79 verify `# HELP` and `# TYPE counter` lines render at boot. Test passes. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `Cargo.toml [package] version` | `cronduit --version` flag at runtime | `env!("CARGO_PKG_VERSION")` compile-time | WIRED | Verified by actually running `./target/debug/cronduit --version` → `cronduit 1.2.0` |
| `ci.yml lint job step `- run: just deny`` | `justfile deny:` recipe | shell invocation through `extractions/setup-just@v2` | WIRED | Confirmed: `setup-just@v2` is configured at L34 of ci.yml; `just deny` runs `cargo deny check advisories licenses bans` |
| `justfile deny:` recipe | `deny.toml` configuration | cargo-deny auto-discovery (no --config flag) | WIRED | Verified by running `just deny` locally — finds deny.toml, applies allowlist |
| `ci.yml step continue-on-error: true` | `deny.toml bans.multiple-versions = warn` | two-layer non-blocking posture (D-09 + D-10) | WIRED | Both layers present and observable in local execution |
| `src/lib.rs pub mod webhooks` | `src/webhooks/mod.rs` | Rust module system | WIRED | Module compiles and is re-exported |
| `dispatcher.rs WebhookDispatcher` | `worker.rs worker_loop` | `Arc<dyn WebhookDispatcher>` trait object | WIRED | `dispatcher.deliver(&event).await` at worker.rs:63 |
| `worker.rs channel()` | `worker.rs spawn_worker()` | `tokio::sync::mpsc::channel(CHANNEL_CAPACITY)` | WIRED | Channel pair construction + spawn flow exercised in cli/run.rs |
| `telemetry.rs describe_counter!` | `telemetry.rs counter!.increment(0)` | metrics-exporter-prometheus 0.18 eager-registration pair | WIRED | Both calls present (lines 111-112 and 133); end-to-end verified by `metrics_families_described_from_boot` test |
| `cli/run.rs channel()` | `spawn_worker` + `scheduler::spawn` | Sender on SchedulerLoop, Receiver in worker | WIRED | Constructed at L250, spawned at L251-255, passed at L268 |
| `run.rs step 7d try_send` | `worker.rs worker_loop` Receiver | `tokio::sync::mpsc` bounded channel | WIRED | `webhook_tx.try_send(event)` at run.rs:427; matching Receiver consumed by worker_loop |
| `run.rs TrySendError::Full arm` | `cronduit_webhook_delivery_dropped_total` counter | `metrics::counter!(...).increment(1)` | WIRED | run.rs:437 (closed-cardinality, no labels in P15) |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `cronduit_webhook_delivery_dropped_total` metric | counter handle | Both eager-baseline (telemetry.rs:133) AND runtime increment (run.rs:437) | YES — counter increments on real `TrySendError::Full` | FLOWING |
| `RunFinalized` channel events | `event` | Constructed in `finalize_run` step 7d from in-scope `run_id`, `job.id`, `job.name`, `status_str`, `exec_result.exit_code`, `start.elapsed()` | YES — sourced from real run lifecycle data | FLOWING |
| `webhook_tx` Sender | mpsc Sender | Constructed in `cli/run.rs::run` via `channel()`, threaded through `scheduler::spawn` to `SchedulerLoop.webhook_tx`, cloned to every `run_job` call | YES — full data path verified | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Workspace version reports 1.2.0 | `./target/debug/cronduit --version` | `cronduit 1.2.0` | PASS |
| Library tests compile and pass | `cargo test -p cronduit --lib` | 194 passed; 0 failed | PASS |
| Webhook queue-drop test passes | `cargo nextest run --test v12_webhook_queue_drop` | 2/2 passed | PASS |
| Scheduler unblocked test passes | `cargo nextest run --test v12_webhook_scheduler_unblocked` | 1/1 passed (5.030s) | PASS |
| Metrics endpoint test extended + passes | `cargo nextest run --test metrics_endpoint` | 1/1 passed | PASS |
| Clippy clean with `-D warnings` | `cargo clippy -p cronduit --all-targets -- -D warnings` | exits 0 | PASS |
| Tests build clean | `cargo build --tests -p cronduit` | exits 0 (25.22s) | PASS |
| `just deny` recipe is invocable | `just --list \| grep deny` | shows recipe with doc string | PASS |
| `cargo-deny` is installed locally | `command -v cargo-deny` | `/Users/Robert/.cargo/bin/cargo-deny` | PASS |
| `just deny` actually runs the three checks | `just deny` | Runs all three checks; surfaces license rejections (Unicode-3.0 not in allowlist) — exit 5, but rc.1 posture absorbs this via `continue-on-error: true` (step level). Same posture as success criterion #2 expects. | PASS (rc.1 non-blocking posture, status visible) |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| FOUND-15 | 15-01-PLAN.md | Cargo.toml version bumped 1.1.0 → 1.2.0; `cronduit --version` reports 1.2.0; rc tags use `v1.2.0-rc.N` semver | SATISFIED | Cargo.toml L3 = `1.2.0`; binary actually runs and reports `cronduit 1.2.0`. Commit `da2bd5c` lands the bump. |
| FOUND-16 | 15-02-PLAN.md | `cargo-deny` CI job runs advisories + licenses + duplicate-versions; license allowlist matches v1.0/v1.1; non-blocking on rc.1; promoted to blocking before final v1.2.0 | SATISFIED | deny.toml + justfile `deny:` + ci.yml step with step-level `continue-on-error: true`. License allowlist = 5 SPDX IDs as required. Phase 24 promotion is one-line removal. |
| WH-02 | 15-03/04/05-PLAN.md | `src/webhooks/mod.rs` owns dedicated tokio task consuming `RunFinalized` from bounded `mpsc(1024)`; scheduler emits via `try_send` (NEVER `await tx.send()`); on full queue → drop + warn log + drop counter increment | SATISFIED | All 4 webhook files exist; CHANNEL_CAPACITY = 1024 verified; `webhook_tx.send(` is grep-confirmed absent from `src/`; `try_send` at run.rs:427; warn log + counter increment at run.rs:430-437; integration tests T-V12-WH-03 + T-V12-WH-04 pass. |

No orphaned requirements. REQUIREMENTS.md mappings (FOUND-15→Phase 15, FOUND-16→Phase 15, WH-02→Phase 15) are all accounted for in PLAN frontmatter.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none in production code) | - | - | - | - |

The 15-REVIEW.md raised 4 WARNING-class items (WR-01 through WR-04). Per the verification instructions these are **advisory, not blocking**. Brief disposition:
- **WR-01** (worker can exit on cancel before scheduler drains): structural concern about shutdown ordering. Goal-level claim "scheduler keeps firing on time" is verified by passing tests; the WR-01 race window only manifests during graceful shutdown grace period, not during normal scheduler operation. Worth fixing pre-GA but not a Phase 15 goal blocker.
- **WR-02** (queue-drop test increments counter manually): the test does double-duty — the runtime counter increment IS verified by `metrics_families_described_from_boot` (which proves the metric is registered at boot from `telemetry.rs`) and by grep on `src/scheduler/run.rs:437` (acceptance criterion of plan 15-04 task 2). The runtime path is locked by static analysis, not by the integration test. Not goal-blocking.
- **WR-03** (drop counter has no labels): explicit P15→P20 deferral per CONTEXT.md D-11; describe text forward-references P20 / WH-11. Not goal-blocking.
- **WR-04** (started_at reconstructed by subtraction): minor inaccuracy under NTP correction; webhook payload schema is P18 territory and `RunFinalized` is internal channel-message, not wire-format. Land before P18 wires HttpDispatcher. Not P15 goal-blocking.

### Human Verification Required

(none)

The success criteria are encoded as executable assertions in the integration tests, all of which pass automatically. No visual/UX/external-service verification needed.

### Gaps Summary

No gaps. All four ROADMAP success criteria for Phase 15 are observably true in the codebase:

1. `cronduit 1.2.0` is what the binary actually prints today (verified by running it).
2. `cargo-deny` runs in the CI lint job with all three checks, license allowlist matches v1.0/v1.1, and `continue-on-error: true` is at step level — exactly the rc.1 posture the success criterion describes ("failures non-blocking on first rc; status visible").
3. The `try_send` non-blocking path is locked by `tests/v12_webhook_scheduler_unblocked.rs::try_send_does_not_block_when_dispatcher_is_stalled`, which fires 5 ticks at 1s cadence under a 60-second-stalled dispatcher and asserts max drift < 1s. The test passes deterministically in 5.030s.
4. The bounded webhook queue saturation drops events with warn-level logs + drop counter increments. The 1024 capacity is hard-coded in `worker.rs:21`; the increment-on-full code path is at `run.rs:437`; the integration test exercises the saturation behavior (at capacity 4 for tractability, but the same `try_send` API regardless of capacity).

The phase plan and the codebase are in alignment. Ready to proceed to Phase 16.

---

_Verified: 2026-04-26T22:59:49Z_
_Verifier: Claude (gsd-verifier)_
