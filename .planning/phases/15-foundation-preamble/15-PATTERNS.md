# Phase 15: Foundation Preamble — Pattern Map

**Mapped:** 2026-04-25
**Files analyzed:** 14 (4 NEW source/test files; 10 modify)
**Analogs found:** 14 / 14 (every file has a strong in-repo analog)

## File Classification

| New / Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---------------------|------|-----------|----------------|---------------|
| `Cargo.toml` (L3 version bump) | config (manifest) | build-time string | `Cargo.toml` pre-`10-01` (1.0.1 → 1.1.0) via `10-01-PLAN.md` | exact (structural twin per D-12) |
| `Cargo.toml` (`[dependencies]` async-trait promotion) | config (manifest) | build-time dep graph | `Cargo.toml:65,69,82` thiserror/secrecy/libc style (single-line crate version pin) | exact |
| `justfile` (new `deny:` recipe) | config (build orchestration) | shell invocation | `justfile:131-139` (`fmt-check:`, `clippy:`) `[group('quality')]` recipes | exact |
| `deny.toml` (NEW, project root) | config (CI tool) | static config | None (no analog in-repo); pattern lifted from cargo-deny v0.19.x upstream docs (verbatim shape in RESEARCH.md §Code Examples) | role-match (config sibling to `Cargo.toml`); content from upstream docs |
| `.github/workflows/ci.yml` (new step in `lint` job) | config (CI workflow) | GHA step graph | `.github/workflows/ci.yml:73-75` (`taiki-e/install-action@v2 with: tool: nextest`) | exact |
| `src/lib.rs` (`pub mod webhooks;`) | module (re-export) | static linkage | `src/lib.rs:2-8` (existing module list — `pub mod scheduler;` etc.) | exact |
| `src/webhooks/mod.rs` (NEW) | module root (re-exports + spawn entry) | static linkage | `src/scheduler/mod.rs:1-25` (module declarations + re-exports) | exact |
| `src/webhooks/event.rs` (NEW, RunFinalized struct) | model (channel-message DTO) | producer→consumer (one-way) | `src/scheduler/log_pipeline.rs:21-37` (`LogLine` channel-message struct) | exact |
| `src/webhooks/dispatcher.rs` (NEW, trait + Noop + Error) | service (trait + impl) | request-response (one-shot per event) | `src/scheduler/cmd.rs:10-92` (closed enum with `thiserror`-derived result types — adjacent shape; the trait is novel but `WebhookError` mirrors `ReloadStatus`/`StopResult` shape) | role-match |
| `src/webhooks/worker.rs` (NEW, channel + worker spawn) | service (background worker) | bounded mpsc → tokio task | `src/scheduler/log_pipeline.rs` end-to-end (channel + worker pattern) AND `src/scheduler/mod.rs:543-572` (`spawn` JoinHandle pattern) | exact (structural twin) |
| `src/scheduler/mod.rs` (modify — add `webhook_tx` field + spawn arg) | model (struct field) | dependency-injected channel handle | `src/scheduler/mod.rs:68-80` (existing `SchedulerLoop` struct with `cmd_rx: mpsc::Receiver<…>` field) | exact (mirror image — Sender vs Receiver) |
| `src/scheduler/run.rs` (modify — step 7d emit) | controller (lifecycle step) | side-effect emission (try_send) | `src/scheduler/run.rs:343-353` (existing step 7b `metrics::counter!` increment) AND `src/scheduler/run.rs:355-378` (existing step 7c broadcast pattern) | exact |
| `src/telemetry.rs` (modify — describe + zero-baseline drop counter) | utility (metric registration) | metrics-registry initialization | `src/telemetry.rs:107-110, 125` (existing `cronduit_run_failures_total` describe + zero-baseline pair) | exact (verbatim two-line idiom) |
| `tests/v12_webhook_queue_drop.rs` (NEW) | test (integration) | bounded-channel saturation + metrics scrape | `tests/metrics_stopped.rs:200-228` (`handle.render()` parsing) + `tests/v11_run_now_sync_insert.rs:46-83` (`build_test_app` harness) | role-match (composes two existing patterns) |
| `tests/v12_webhook_scheduler_unblocked.rs` (NEW) | test (integration) | timing assertion across spawn batch | `tests/v11_run_now_sync_insert.rs:46-83` (`build_test_app` in-process Scheduler harness) | exact harness; novel timing assertion |
| `tests/metrics_endpoint.rs` (modify — extend `metrics_families_described_from_boot`) | test (integration extension) | metrics-handle render assertion | `tests/metrics_endpoint.rs:44-69` (existing `cronduit_runs_total` / `cronduit_run_failures_total` HELP+TYPE asserts) | exact (verbatim assert pair) |

---

## Pattern Assignments

### `Cargo.toml` (L3 version bump) — Plan 15-01

**Analog:** `.planning/milestones/v1.1-phases/10-stop-a-running-job-hygiene-preamble/10-01-PLAN.md` (the structural twin per D-12)

**Edit pattern** (`Cargo.toml:3`):
```toml
# BEFORE:
version = "1.1.0"

# AFTER:
version = "1.2.0"
```

**Verify pattern** (lifted verbatim from `10-01-PLAN.md` line 84):
```bash
grep -q '^version = "1.2.0"$' Cargo.toml \
  && cargo build -p cronduit 2>&1 | tail -5 \
  && ./target/debug/cronduit --version | grep -q '1.2.0'
```

**Acceptance criteria** (lifted from `10-01-PLAN.md` lines 87-92, with version substituted):
- `grep -c '^version = "1.2.0"$' Cargo.toml` returns exactly `1`
- `grep -c '^version = "1.1.0"$' Cargo.toml` returns exactly `0`
- `cargo build -p cronduit` exits 0 with no warnings beyond baseline
- `./target/debug/cronduit --version` prints a line containing `1.2.0` and NOT `1.1.0`
- `Cargo.lock` `name = "cronduit"` block carries `version = "1.2.0"`
- Diff is exactly `Cargo.toml` and `Cargo.lock`, nothing else

**Commit message shape** (mirrors Phase 10's): `chore(15): bump workspace version to 1.2.0 (FOUND-15)`

---

### `Cargo.toml` (`[dependencies]` — promote `async-trait` to direct dep) — Plan 15-03

**Analog:** existing single-line pinned crates in `Cargo.toml:65, 66, 69` (`anyhow`, `thiserror`, `secrecy`).

**Insertion pattern** — slot near other "stub trait support" deps (between `thiserror = "2.0.18"` at L66 and `secrecy = { … }` at L69):
```toml
# Errors
anyhow = "1.0.102"
thiserror = "2.0.18"

# Async trait macro — required for `dyn WebhookDispatcher` (Phase 15 D-01).
# Already transitive via testcontainers + axum-htmx (Cargo.lock:237);
# promoted to direct so the trait shape is documented at the crate boundary.
async-trait = "0.1"
```

**Verification:** `Cargo.lock:237` already shows `name = "async-trait"` (verified 2026-04-25). Adding the direct entry causes zero new compile cost (`grep -c 'name = "async-trait"' Cargo.lock` stays `1`).

---

### `justfile` (new `deny:` recipe) — Plan 15-02

**Analog:** `justfile:131-139` (`fmt-check:` and `clippy:`).

**Imports / preamble pattern** (lines 8-9 — file-level, already present, no change needed):
```just
set shell := ["bash", "-euo", "pipefail", "-c"]
set dotenv-load := true
```

**Recipe pattern** (mirror of `fmt-check:` at L131-134 — `[group('quality')]` + `[doc(...)]` annotation):
```just
# Phase 15 / FOUND-16. Supply-chain hygiene gate: advisories + licenses +
# duplicate-versions in a single invocation. Non-blocking on rc.1
# (continue-on-error in ci.yml + bans.multiple-versions = "warn" in deny.toml);
# promoted to blocking before final v1.2.0 (Phase 24).
[group('quality')]
[doc('cargo-deny supply-chain check (advisories + licenses + bans)')]
deny:
    cargo deny check advisories licenses bans
```

**Placement:** insert AFTER the existing `clippy:` (L138-139) and BEFORE `test:` (L143). The recipe has no `just`-level dependencies (mirrors `fmt-check:`, `clippy:`).

**Convention reaffirmed:** every existing `[group('quality')]` recipe is a single-line `cargo …` invocation. The new `deny:` recipe matches.

---

### `deny.toml` (NEW, project root) — Plan 15-02

**Analog:** None in-repo. Content lifted verbatim from RESEARCH.md §Code Examples (which itself was synthesized from cargo-deny v0.19.4 upstream docs verified 2026-04-25).

**Critical guardrails (Pitfall 4, RESEARCH.md):**
- Use ONLY `allow = [...]` in `[licenses]`. Do NOT use `default`, `unlicensed`, `copyleft`, `deny`, or `allow-osi-fsf-free` — all are removed in v0.19.x and emit errors.
- `bans.multiple-versions = "warn"` (D-10) — NOT `"deny"` for rc.1.
- License allowlist EXACTLY: `MIT`, `Apache-2.0`, `BSD-3-Clause`, `ISC`, `Unicode-DFS-2016`. Adding any other license is a deliberate decision requiring an inline comment.

**Skeleton** (verbatim from RESEARCH.md §Code Examples lines 344-409):
```toml
# deny.toml — cargo-deny v0.19.x configuration for cronduit.
# Phase 15 / FOUND-16. License allowlist matches v1.0/v1.1 posture.

[graph]
targets = [
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
]
all-features = true

[advisories]
ignore = []

[licenses]
allow = [
    "MIT",
    "Apache-2.0",
    "BSD-3-Clause",
    "ISC",
    "Unicode-DFS-2016",
]
confidence-threshold = 0.93

[bans]
multiple-versions = "warn"
wildcards = "warn"
skip = []
skip-tree = []

[sources]
unknown-registry = "warn"
unknown-git = "warn"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

**Targets rationale:** the four triples mirror the targets `just openssl-check` already loops over (justfile:177) — the same multi-arch musl + gnu posture cronduit ships for.

---

### `.github/workflows/ci.yml` (new step in `lint` job) — Plan 15-02

**Analog:** `.github/workflows/ci.yml:73-75` (existing `taiki-e/install-action@v2 with: tool: nextest,cargo-zigbuild` in the `test` job).

**Imports pattern** (already in the `lint` job, no change needed):
```yaml
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - uses: extractions/setup-just@v2
```

**`just <recipe>` exclusivity** (file-level convention reaffirmed at L1-4): every CI step invokes `just <name>` only. Inline `cargo` is forbidden. The new step uses `run: just deny`, NOT `run: cargo deny check ...`.

**Insertion point** — IMMEDIATELY AFTER `- run: just grep-no-percentile-cont` at L46 (currently the last step in the `lint` job). The `test` job begins at L54 — insert before that.

**Step pattern** (mirrors the `taiki-e/install-action` invocation at L73-75 + adds the step-level `continue-on-error: true` per D-09):
```yaml
      - run: just grep-no-percentile-cont
      # Phase 15 / FOUND-16. cargo-deny supply-chain check (advisories +
      # licenses + duplicate-versions). Non-blocking on rc.1 per D-09 — the
      # step is marked continue-on-error: true so a transient advisory or
      # transitive duplicate-version finding cannot redden CI in v1.2 hands.
      # Promoted to blocking (single-line removal of continue-on-error)
      # before final v1.2.0 ships in Phase 24. Pairs with deny.toml's
      # `bans.multiple-versions = "warn"` for two-layer non-blocking (D-10).
      - uses: taiki-e/install-action@v2
        with:
          tool: cargo-deny
      - run: just deny
        continue-on-error: true
```

**Critical placement (Pitfall 5, RESEARCH.md):** `continue-on-error: true` MUST be at step level (sibling of `run:`), NOT job level. Putting it under `lint:` would silence ALL lint failures (clippy, openssl-check, fmt-check) — a much wider hole than intended.

---

### `src/lib.rs` — register `webhooks` module — Plan 15-03

**Analog:** `src/lib.rs:2-8` (existing module declarations).

**Insertion pattern** — alphabetical placement within the existing list:
```rust
//! cronduit library crate root. Re-exports modules for integration tests.
pub mod cli;
pub mod config;
pub mod db;
pub mod scheduler;
pub mod shutdown;
pub mod telemetry;
pub mod web;
pub mod webhooks;  // Phase 15 / WH-02 — webhook delivery worker
```

---

### `src/webhooks/mod.rs` (NEW) — Plan 15-03

**Analog:** `src/scheduler/mod.rs:1-25` (module-root re-export pattern).

**Pattern (Option B layout — recommended):**
```rust
//! Webhook delivery worker (Phase 15 / WH-02).
//!
//! Bounded mpsc + dedicated tokio task pattern: the scheduler emits
//! `RunFinalized` events via `try_send` (NEVER `send().await`); the worker
//! consumes them and dispatches via the `WebhookDispatcher` trait. Phase 15
//! ships only `NoopDispatcher`; P18 swaps in `HttpDispatcher` against the
//! same trait.
//!
//! ```mermaid
//! flowchart LR
//!     SCHED[scheduler<br/>finalize_run] -->|try_send| CHAN[(mpsc bounded 1024)]
//!     CHAN --> WORKER[worker_loop<br/>tokio::select!]
//!     WORKER --> DISP[dyn WebhookDispatcher]
//!     SCHED -->|TrySendError::Full| METRIC[cronduit_webhook_delivery_dropped_total ++]
//! ```

pub mod dispatcher;
pub mod event;
pub mod worker;

pub use dispatcher::{NoopDispatcher, WebhookDispatcher, WebhookError};
pub use event::RunFinalized;
pub use worker::{CHANNEL_CAPACITY, channel, spawn_worker};
```

**Convention notes:**
- Module declarations are `pub mod` (matches `src/scheduler/mod.rs:7-24`).
- Re-exports flatten the public surface so integration tests can `use cronduit::webhooks::RunFinalized` instead of `use cronduit::webhooks::event::RunFinalized` (matches the `cronduit::scheduler::cmd::SchedulerCmd` shape used in `tests/v11_run_now_sync_insert.rs:30`).
- Mermaid diagram in the doc-comment satisfies the "all diagrams are mermaid" project rule (D-14).

---

### `src/webhooks/event.rs` (NEW) — Plan 15-03

**Analog:** `src/scheduler/log_pipeline.rs:21-37` (`LogLine` channel-message struct).

**Imports pattern** (mirror of `log_pipeline.rs:7-9` for cross-cutting deps + `chrono` for the timestamp fields):
```rust
use chrono::{DateTime, Utc};
```

**Struct pattern** (mirror of `LogLine`'s `#[derive(Debug, Clone)]` plus pub fields with field-level doc comments):
```rust
//! Channel-message contract for the webhook delivery worker.
//!
//! Phase 15 / WH-02 / D-02: self-contained minimum payload. Streak metrics,
//! image_digest, and config_hash come from P16's `get_failure_context` query
//! at delivery time inside the dispatcher — they are NOT carried on the
//! channel. This keeps the P15 message stable against P16's schema work.
//!
//! NOTE: this is the CHANNEL-MESSAGE contract, not the WIRE-FORMAT payload.
//! P18 / WH-03 introduces `src/webhooks/payload.rs` for the JSON wire format.

use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct RunFinalized {
    pub run_id: i64,
    pub job_id: i64,
    pub job_name: String,
    /// Canonical terminal status string. Matches `src/scheduler/run.rs`'s
    /// `status_str` mapping at L315-322:
    /// `"success" | "failed" | "timeout" | "cancelled" | "stopped" | "error"`.
    pub status: String,
    pub exit_code: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}
```

**Convention notes:**
- `#[derive(Debug, Clone)]` matches `LogLine` at `log_pipeline.rs:21`.
- Pub fields (no getter functions) match `LogLine` and `SchedulerCmd::RunNow` (`scheduler/cmd.rs:22`).
- Field-level `///` doc comments on the non-obvious `status` field mirror `LogLine.id` doc at `log_pipeline.rs:30-36`.

---

### `src/webhooks/dispatcher.rs` (NEW) — Plan 15-03

**Analog:** `src/scheduler/cmd.rs:65-93` (`StopResult` closed-enum + doc comment style) for `WebhookError`. The trait shape itself is novel but follows standard `#[async_trait]` Rust convention.

**Imports pattern:**
```rust
use async_trait::async_trait;
use thiserror::Error;

use super::event::RunFinalized;
```

**Error enum pattern** (mirror of `StopResult` at `cmd.rs:83-92` — closed enum, `#[derive(Debug)]`, derived display via `thiserror`):
```rust
#[derive(Debug, Error)]
pub enum WebhookError {
    /// Stub variant for P18 to expand. P15 never produces this — the
    /// `NoopDispatcher` always returns `Ok(())`.
    #[error("webhook dispatch failed: {0}")]
    DispatchFailed(String),
}
```

**Trait + impl pattern** (the seam P18 implements against — D-01 verbatim):
```rust
#[async_trait]
pub trait WebhookDispatcher: Send + Sync {
    async fn deliver(&self, event: &RunFinalized) -> Result<(), WebhookError>;
}

/// Always-on default dispatcher. Logs at debug and returns `Ok(())`.
pub struct NoopDispatcher;

#[async_trait]
impl WebhookDispatcher for NoopDispatcher {
    async fn deliver(&self, event: &RunFinalized) -> Result<(), WebhookError> {
        tracing::debug!(
            target: "cronduit.webhooks",
            run_id = event.run_id,
            job_id = event.job_id,
            status = %event.status,
            "noop webhook dispatch"
        );
        Ok(())
    }
}
```

**Tracing pattern (from `src/scheduler/run.rs:85-90, 306-311`):** `tracing::<level>!(target: "cronduit.<subsystem>", <structured fields>, "<static message>")`. The new target is `cronduit.webhooks` (matches D-04).

**Pitfall 2 (RESEARCH.md):** `#[async_trait]` IS required on BOTH the trait declaration AND every impl block. Native `dyn` async-fn-in-trait is not object-safe on Rust 1.94.1.

---

### `src/webhooks/worker.rs` (NEW) — Plan 15-03

**Analog:** `src/scheduler/log_pipeline.rs:159-174` (channel constructor pattern) + `src/scheduler/mod.rs:543-572` (`spawn` JoinHandle wrapper).

**Imports pattern** (mirror of `log_pipeline.rs:7-9` + `mod.rs:35-37` for tokio + tokio-util types):
```rust
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::dispatcher::WebhookDispatcher;
use super::event::RunFinalized;
```

**Channel constants pattern** (mirror of `log_pipeline.rs:11-18` `pub const DEFAULT_CHANNEL_CAPACITY: usize = 256;`):
```rust
/// Channel capacity. WH-02 locks 1024 — large enough to absorb a transient
/// dispatcher stall without dropping events under normal homelab load,
/// small enough that a sustained outage produces visible drop-counter
/// activity within minutes (an operator-actionable signal).
pub const CHANNEL_CAPACITY: usize = 1024;
```

**Channel constructor pattern** (mirror of `log_pipeline.rs:159-174`):
```rust
pub fn channel() -> (mpsc::Sender<RunFinalized>, mpsc::Receiver<RunFinalized>) {
    mpsc::channel(CHANNEL_CAPACITY)
}

/// Test-only constructor with a tunable capacity. Integration tests in
/// `tests/v12_webhook_queue_drop.rs` use a small capacity (e.g., 4) to
/// force `TrySendError::Full` synchronously.
pub fn channel_with_capacity(
    cap: usize,
) -> (mpsc::Sender<RunFinalized>, mpsc::Receiver<RunFinalized>) {
    mpsc::channel(cap)
}
```

**Worker spawn pattern** (mirror of `src/scheduler/mod.rs:543-572` `pub fn spawn(...) -> JoinHandle<...>`):
```rust
/// Spawn the worker task. The task runs until either the cancel token fires
/// or the last sender clone is dropped.
pub fn spawn_worker(
    rx: mpsc::Receiver<RunFinalized>,
    dispatcher: Arc<dyn WebhookDispatcher>,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(worker_loop(rx, dispatcher, cancel))
}
```

**Receiver loop pattern** — `tokio::select!` with `biased;` arm priority (mirror of `src/scheduler/mod.rs:106-165` select-loop shape):
```rust
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
```

**Convention notes:**
- `tokio::select! { biased; <recv arm>; <cancel arm> }` mirrors the scheduler's main loop priority idiom.
- `tracing::warn!` / `tracing::info!` use `target: "cronduit.webhooks"` per D-04.
- Worker exit on `recv()` returning `None` is "log info + break" (Claude's Discretion clarified in CONTEXT.md).

---

### `src/scheduler/mod.rs` (modify — add `webhook_tx` field + spawn arg) — Plan 15-03

**Analog:** `src/scheduler/mod.rs:75` (existing `pub cmd_rx: tokio::sync::mpsc::Receiver<cmd::SchedulerCmd>` field) is the mirror image of what we're adding (Sender vs Receiver).

**Field insertion** — at the end of the `SchedulerLoop` struct (between L79 `active_runs` and L80 closing brace):
```rust
pub struct SchedulerLoop {
    pub pool: DbPool,
    pub docker: Option<Docker>,
    pub jobs: HashMap<i64, DbJob>,
    pub tz: Tz,
    pub cancel: CancellationToken,
    pub shutdown_grace: Duration,
    pub cmd_rx: tokio::sync::mpsc::Receiver<cmd::SchedulerCmd>,
    pub config_path: PathBuf,
    pub active_runs: Arc<RwLock<HashMap<i64, RunEntry>>>,
    /// Phase 15 / WH-02 / D-03: always-on sender to the webhook delivery
    /// worker. Cloned into every `run::run_job(...)` call so `finalize_run`
    /// can emit `RunFinalized` at step 7d. The worker is spawned by the
    /// bin layer (`src/cli/run.rs`) at startup with the matching Receiver.
    pub webhook_tx: tokio::sync::mpsc::Sender<crate::webhooks::RunFinalized>,
}
```

**`spawn(...)` modification** — `src/scheduler/mod.rs:548-572`. Add `webhook_tx: tokio::sync::mpsc::Sender<crate::webhooks::RunFinalized>` parameter and copy into the struct literal:
```rust
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    pool: DbPool,
    docker: Option<Docker>,
    jobs: Vec<DbJob>,
    tz: Tz,
    cancel: CancellationToken,
    shutdown_grace: Duration,
    cmd_rx: tokio::sync::mpsc::Receiver<cmd::SchedulerCmd>,
    config_path: PathBuf,
    active_runs: Arc<RwLock<HashMap<i64, RunEntry>>>,
    webhook_tx: tokio::sync::mpsc::Sender<crate::webhooks::RunFinalized>,  // NEW
) -> JoinHandle<()> {
    let jobs_map: HashMap<i64, DbJob> = jobs.into_iter().map(|j| (j.id, j)).collect();
    let scheduler = SchedulerLoop {
        pool,
        docker,
        jobs: jobs_map,
        tz,
        cancel,
        shutdown_grace,
        cmd_rx,
        config_path,
        active_runs,
        webhook_tx,  // NEW
    };
    tokio::spawn(scheduler.run())
}
```

**`run_job(...)` call sites** (lines 122, 146, 190, 221, 286, 305 per RESEARCH.md) — each must additionally pass `self.webhook_tx.clone()`. The Sender is `Clone` (Arc-based, cheap).

**Wiring in `src/cli/run.rs`** (around L134 + L243):
```rust
// At L134 — alongside the existing cmd channel construction:
let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<crate::scheduler::cmd::SchedulerCmd>(32);
let (webhook_tx, webhook_rx) = crate::webhooks::channel();  // NEW

// New: spawn the webhook worker with NoopDispatcher (D-03 always-on).
// Lifetime: owned by the bin layer; scheduler shutdown fires the cancel
// token; this layer awaits the worker's JoinHandle after the scheduler
// finishes draining (mirrors the existing graceful-shutdown pattern).
let webhook_worker_handle = crate::webhooks::spawn_worker(
    webhook_rx,
    std::sync::Arc::new(crate::webhooks::NoopDispatcher),
    cancel.child_token(),
);

// At L243 — pass webhook_tx into scheduler::spawn:
let scheduler_handle = crate::scheduler::spawn(
    pool.clone(),
    docker,
    sync_result.jobs,
    tz,
    cancel.clone(),
    cfg.server.shutdown_grace,
    cmd_rx,
    config_path.to_path_buf(),
    active_runs,
    webhook_tx,  // NEW
);

// At L259 — drain webhook worker AFTER scheduler drains:
let _ = scheduler_handle.await;
let _ = webhook_worker_handle.await;  // NEW
```

---

### `src/scheduler/run.rs` (modify — step 7d emit) — Plan 15-03

**Analog 1 (counter idiom):** `src/scheduler/run.rs:345-352` (existing closed-cardinality `metrics::counter!` increments at step 7b).

**Analog 2 (step ordering + comment style):** `src/scheduler/run.rs:355-378` (existing step 7c broadcast block with extensive rationale comment).

**Analog 3 (`tracing::error!` shape):** `src/scheduler/run.rs:85-90, 306-311, 335-340` (existing target + structured-field error logs).

**CRITICAL — step renumbering (Pitfall 6, RESEARCH.md):**

Current (`run.rs:355-382`):
- `// 7c. ... broadcast __run_finished__ sentinel ...`
- `// 7d. Remove broadcast sender ...`

After this plan:
- `// 7c. ... broadcast __run_finished__ sentinel ...` (UNCHANGED)
- `// 7d. (NEW) webhook_tx.try_send(...)` (INSERTED)
- `// 7e. (renumbered from 7d) Remove broadcast sender ...`

Verify with `grep -n '// 7' src/scheduler/run.rs` — expect 7, 7b, 7c, 7d (NEW), 7e (was 7d) in that order. Two `// 7d` headers in the same function is an unacceptable diff — the planner MUST update the existing comment.

**`run_job(...) / continue_run(...)` signature change:** add `webhook_tx: tokio::sync::mpsc::Sender<crate::webhooks::RunFinalized>` to the parameter list (mirrors how `active_runs` was added in earlier phases — see `run.rs:71-77`). Every call site in `scheduler/mod.rs` (lines 122, 146, 190, 221, 286, 305) passes `self.webhook_tx.clone()`.

**Insertion pattern** (between current L378 `let _ = broadcast_tx.send(LogLine { … });` and current L380 `// 7d. Remove broadcast sender …`):
```rust
    // 7c. (existing — UNCHANGED) broadcast __run_finished__ sentinel
    let _ = broadcast_tx.send(LogLine {
        stream: "__run_finished__".to_string(),
        ts: chrono::Utc::now().to_rfc3339(),
        line: run_id.to_string(),
        id: None,
    });

    // 7d. (NEW — Phase 15 / WH-02 / D-04 + D-05) Emit RunFinalized event
    // for the webhook delivery worker. NEVER use webhook_tx.send().await —
    // that would block the scheduler loop on a slow receiver (Pitfall 28).
    // try_send returns immediately; on full queue we drop with a warn log
    // + counter increment (D-04) so scheduler timing is preserved.
    let finished_at = chrono::Utc::now();
    let started_at = finished_at
        - chrono::Duration::from_std(start.elapsed())
            .unwrap_or_else(|_| chrono::Duration::zero());
    let event = crate::webhooks::RunFinalized {
        run_id,
        job_id: job.id,
        job_name: job.name.clone(),
        status: status_str.to_string(),
        exit_code: exec_result.exit_code,
        started_at,
        finished_at,
    };
    match webhook_tx.try_send(event) {
        Ok(()) => {}
        Err(tokio::sync::mpsc::error::TrySendError::Full(dropped)) => {
            tracing::warn!(
                target: "cronduit.webhooks",
                run_id = dropped.run_id,
                job_id = dropped.job_id,
                status = %dropped.status,
                "webhook delivery channel saturated — event dropped"
            );
            metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(1);
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
            tracing::error!(
                target: "cronduit.webhooks",
                run_id,
                job_id = job.id,
                "webhook delivery channel closed — worker is gone"
            );
        }
    }

    // 7e. (renumbered from 7d) Remove broadcast sender so SSE subscribers
    // get RecvError::Closed (UI-14, D-02).
    active_runs.write().await.remove(&run_id);
    drop(broadcast_tx);
```

**Counter idiom convention** (lifted from `run.rs:345-352`): `metrics::counter!("cronduit_<name>", "<label>" => <value>).increment(1)`. The drop counter is the **unlabeled** variant — `metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(1)` — because P15 has zero labels (D-11). The full labeled `cronduit_webhook_*` family arrives in P20 / WH-11.

**Tracing convention** (lifted from `run.rs:85-90`): `tracing::warn!(target: "cronduit.<subsystem>", <field> = <value>, "<static message>")`. Use `%` for `Display` formatting (e.g., `status = %dropped.status`); plain `=` for `Debug`-renderable scalars.

---

### `src/telemetry.rs` (modify — describe + zero-baseline drop counter) — Plan 15-03

**Analog:** `src/telemetry.rs:107-110` (`cronduit_run_failures_total` describe) + `src/telemetry.rs:125` (paired zero-baseline call).

**Insertion location** — INSIDE the `OnceLock::get_or_init` closure in `setup_metrics()`. Specifically, between L110 (end of `cronduit_run_failures_total` describe) and L111 (`cronduit_docker_reachable` describe). Then between L125 (`cronduit_run_failures_total` zero-baseline) and L126 (`cronduit_docker_reachable` zero-baseline).

**Describe pattern** (verbatim mirror of L107-110):
```rust
            metrics::describe_counter!(
                "cronduit_webhook_delivery_dropped_total",
                "Total webhook events dropped because the bounded delivery channel was \
                 saturated. Closed-cardinality (no labels in P15). Increments correlate \
                 with WARN-level events on the cronduit.webhooks tracing target. The \
                 full cronduit_webhook_* family lands in P20 / WH-11."
            );
```

**Zero-baseline pattern** (verbatim mirror of L125):
```rust
            metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(0);
```

**CRITICAL pairing (Pitfall 3, RESEARCH.md + the comment block at `src/telemetry.rs:75-90`):** the `describe_counter!` call alone does NOT register the metric in the Prometheus registry. The exporter renders HELP/TYPE lines only AFTER a paired macro call (`counter!`/`gauge!`/`histogram!`) constructs a registry handle. Both calls are required; omitting the zero-baseline ships a silent regression where the metric line is absent from `/metrics` until the first drop fires.

**Convention notes:**
- The describe call uses a multi-line raw description string with the `\` line continuation (mirrors the existing `cronduit_runs_total` describe at L99-102).
- The zero-baseline call has no labels (mirrors the existing `cronduit_run_failures_total` zero-baseline at L125 — note that the labeled `cronduit_runs_total` zero-baseline at L145-155 is structurally different because it pre-declares the closed-enum status label values; the drop counter has no labels at all in P15, so the simple unlabeled `.increment(0)` form is correct).

---

### `tests/v12_webhook_queue_drop.rs` (NEW) — Plan 15-03

**Analog 1 (test framework):** `tests/v11_run_now_sync_insert.rs:46-83` (`build_test_app` in-process Scheduler harness — no testcontainers).

**Analog 2 (metrics scrape):** `tests/metrics_stopped.rs:200-228` (`handle.render()` → line-find → trailing-number parse pattern).

**Imports pattern** (mirror of `tests/v11_run_now_sync_insert.rs:17-34`):
```rust
//! Phase 15 / WH-02 / T-V12-WH-04: bounded channel saturation drops events
//! and increments cronduit_webhook_delivery_dropped_total without blocking
//! the scheduler-side try_send.

use std::sync::Arc;
use std::time::{Duration, Instant};

use cronduit::telemetry::setup_metrics;
use cronduit::webhooks::{self, NoopDispatcher, RunFinalized, WebhookDispatcher, WebhookError};
use tokio_util::sync::CancellationToken;
```

**Stalled dispatcher mock pattern** (mirror of CONTEXT.md / RESEARCH.md spec):
```rust
struct StalledDispatcher;

#[async_trait::async_trait]
impl WebhookDispatcher for StalledDispatcher {
    async fn deliver(&self, _event: &RunFinalized) -> Result<(), WebhookError> {
        tokio::time::sleep(Duration::from_secs(60)).await;
        Ok(())
    }
}
```

**Metrics scrape + counter delta pattern** (mirror of `tests/metrics_stopped.rs:202-228`):
```rust
let handle = setup_metrics();

fn read_drop_counter(body: &str) -> f64 {
    body.lines()
        .find(|l| l.starts_with("cronduit_webhook_delivery_dropped_total ")
                || l.starts_with("cronduit_webhook_delivery_dropped_total{"))
        .and_then(|l| l.rsplit_once(' ').and_then(|(_, n)| n.trim().parse().ok()))
        .unwrap_or(0.0)
}

let baseline = read_drop_counter(&handle.render());
// … push N=20 events into a capacity=4 channel …
let after = read_drop_counter(&handle.render());
let delta = after - baseline;
assert!(delta >= 10.0, "expected >= 10 drops, got {delta}");
```

**Channel saturation driver pattern** (lifts the `channel_with_capacity` test helper documented in RESEARCH.md §Wave 0 Gaps):
```rust
let (tx, rx) = webhooks::worker::channel_with_capacity(4);
let cancel = CancellationToken::new();
let _worker = webhooks::spawn_worker(rx, Arc::new(StalledDispatcher), cancel.clone());

let push_start = Instant::now();
for i in 0..20 {
    let _ = tx.try_send(make_event(i));
}
let push_elapsed = push_start.elapsed();

assert!(
    push_elapsed < Duration::from_millis(50),
    "scheduler-side try_send must never block; took {push_elapsed:?}"
);
```

**Naming convention** — `tests/v12_webhook_queue_drop.rs` matches the existing `vNN_<feature>_<scenario>.rs` pattern (see `tests/v11_bulk_toggle.rs`, `tests/v11_run_now_sync_insert.rs`).

---

### `tests/v12_webhook_scheduler_unblocked.rs` (NEW) — Plan 15-03

**Analog:** `tests/v11_run_now_sync_insert.rs:46-83` (`build_test_app` in-process Scheduler harness).

**Imports pattern** — same as `v12_webhook_queue_drop.rs` plus:
```rust
use cronduit::db::DbPool;
use cronduit::scheduler;
use cronduit::scheduler::cmd::SchedulerCmd;
```

**Test specification** (per RESEARCH.md §Test Specifications T-V12-WH-03):
- Spawn `StalledDispatcher` from `v12_webhook_queue_drop.rs` (share via `mod common;` or duplicate inline).
- Construct an in-process Scheduler harness (mirror of `tests/v11_run_now_sync_insert.rs:46-83`).
- Seed N=5 jobs; drive `run_job` directly via `tokio::spawn` in a 1-second-cadence loop, capturing wall-clock `Instant` of each spawn.
- Assert: `max(spawns[i+1] - spawns[i]) - Duration::from_secs(1) < Duration::from_secs(1)`. No inter-spawn interval exceeds 2s.

**Critical assertion** — proves the stalled dispatcher does NOT block run-task body's webhook emit, which would cascade into late spawns.

---

### `tests/metrics_endpoint.rs` (modify — extend `metrics_families_described_from_boot`) — Plan 15-03

**Analog:** `tests/metrics_endpoint.rs:62-69` (existing `cronduit_run_failures_total` HELP+TYPE assert pair).

**Insertion pattern** — append after L69 closing brace of the existing failures assert, before the function's closing brace at L70:
```rust
    // Phase 15 / WH-02 / D-11 — drop counter must render HELP/TYPE from boot.
    assert!(
        body.contains("# HELP cronduit_webhook_delivery_dropped_total"),
        "missing HELP for cronduit_webhook_delivery_dropped_total; body: {body}"
    );
    assert!(
        body.contains("# TYPE cronduit_webhook_delivery_dropped_total counter"),
        "missing TYPE for cronduit_webhook_delivery_dropped_total; body: {body}"
    );
```

**Convention notes:**
- Verbatim mirror of the existing `cronduit_run_failures_total` assert pair at L62-69.
- Single-line `body.contains("# HELP ...")` / `body.contains("# TYPE ... counter")` pair.
- Failure message shape: `"missing <KIND> for <metric>; body: {body}"` (matches existing assertions).

---

## Shared Patterns

### Tracing target convention (`cronduit.<subsystem>`)
**Source:** `src/scheduler/run.rs:85-90, 98-104, 306-311, 384-389` (every cronduit log uses `target: "cronduit.<subsystem>"`).
**Apply to:** All new logging in `src/webhooks/dispatcher.rs`, `src/webhooks/worker.rs`, and the new step 7d block in `src/scheduler/run.rs`.
**New target:** `cronduit.webhooks` (per D-04).
**Pattern:**
```rust
tracing::warn!(
    target: "cronduit.webhooks",
    run_id = dropped.run_id,
    job_id = dropped.job_id,
    status = %dropped.status,
    "webhook delivery channel saturated — event dropped"
);
```
- Use `%` sigil for `Display` (e.g., `%status_string`, `%error`).
- Plain `=` for `Debug`-renderable scalars (`run_id`, `job_id`).
- Static message string is the LAST positional argument and contains no interpolation (the structured fields carry the variable data).

### Closed-cardinality counter idiom
**Source:** `src/scheduler/run.rs:345-352` + `src/telemetry.rs:99-110, 123-125`.
**Apply to:** New `cronduit_webhook_delivery_dropped_total` increment at the new step 7d in `src/scheduler/run.rs` AND the eager-describe + zero-baseline pair in `src/telemetry.rs`.
**Pattern (no labels):**
```rust
metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(1);
```
**Pattern (eager describe + zero-baseline pair — REQUIRED, see Pitfall 3):**
```rust
metrics::describe_counter!("cronduit_webhook_delivery_dropped_total", "<help text>");
// … later in the same closure:
metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(0);
```

### `tokio::select!` `biased;` priority idiom
**Source:** `src/scheduler/mod.rs:106-165` (scheduler main loop), and recommended in RESEARCH.md §Code Examples for the worker loop.
**Apply to:** `src/webhooks/worker.rs::worker_loop`.
**Pattern:**
```rust
tokio::select! {
    biased;
    maybe_event = rx.recv() => { /* … */ }
    _ = cancel.cancelled() => { /* … */ break; }
}
```
The `biased;` keyword makes the recv arm checked first every iteration — prevents a tight cancel loop from starving in-flight deliveries.

### `mpsc` channel + `JoinHandle` worker spawn
**Source:** `src/scheduler/log_pipeline.rs:159-174` (channel constructor) + `src/scheduler/mod.rs:543-572` (`spawn(...) -> JoinHandle<…>` wrapper).
**Apply to:** `src/webhooks/worker.rs::channel()` and `src/webhooks/worker.rs::spawn_worker(...)`.
**Pattern:** module-level `pub fn channel()` returning the sender/receiver pair; module-level `pub fn spawn_worker(...) -> JoinHandle<()>` that wraps `tokio::spawn(worker_loop(...))`.

### `[group('quality')]` justfile recipe
**Source:** `justfile:131-149` (`fmt-check:`, `clippy:`, `test:`, `nextest:`).
**Apply to:** New `just deny` recipe in `justfile`.
**Pattern:** `[group('quality')]` annotation, optional `[doc('…')]` annotation, single-line `cargo …` invocation, no `just`-level dependencies.

### `taiki-e/install-action@v2` CI tool installation
**Source:** `.github/workflows/ci.yml:73-75` (existing `nextest,cargo-zigbuild` install in the `test` job).
**Apply to:** New cargo-deny install step in the `lint` job of `.github/workflows/ci.yml`.
**Pattern:** `- uses: taiki-e/install-action@v2` followed by `with: { tool: <crate-name> }`. No version pin — installs latest by default.

### `every CI step invokes just <recipe>` exclusivity
**Source:** `.github/workflows/ci.yml:1-4` (preamble) + every existing `run:` step in the file (all are `run: just <name>`).
**Apply to:** New cargo-deny step. Use `run: just deny`, NOT `run: cargo deny check ...`.

### Mermaid-only diagrams (project rule D-14)
**Source:** Project rule (CLAUDE.md and `feedback_diagrams_mermaid.md` global memory).
**Apply to:** Any diagram in `src/webhooks/mod.rs` doc-comments, plan docs, PR description, README. NEVER ASCII art.
**Format:** triple-backtick mermaid fenced block.

### Test file naming `vNN_<feature>_<scenario>.rs`
**Source:** Existing tests — `tests/v11_bulk_toggle.rs`, `tests/v11_run_now_sync_insert.rs`, `tests/v13_timeline_explain.rs`.
**Apply to:** `tests/v12_webhook_queue_drop.rs` and `tests/v12_webhook_scheduler_unblocked.rs`.

### `mpsc::Sender` clone into per-task call sites
**Source:** `src/scheduler/mod.rs:122-129` (`run_job(self.pool.clone(), self.docker.clone(), …, self.active_runs.clone())` — every per-task spawn clones every shared handle).
**Apply to:** Every `run_job(...) / run_job_with_existing_run_id(...)` call site at lines 122, 146, 190, 221, 286, 305 must additionally pass `self.webhook_tx.clone()`. The Sender clone is cheap (Arc-style refcount per tokio docs).

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `deny.toml` | config (CI tool) | static config | First cargo-deny config in this repo. Content lifted verbatim from cargo-deny v0.19.4 upstream docs (verified 2026-04-25 in RESEARCH.md). The format is well-defined (TOML); the role-twin in-repo is `Cargo.toml` (project-root config sibling). |

For this single file, the planner uses RESEARCH.md §Code Examples lines 344-409 directly. All other files have strong in-repo analogs.

---

## Metadata

**Analog search scope:**
- `src/scheduler/` (subdirectories: `cmd.rs`, `log_pipeline.rs`, `mod.rs`, `run.rs`)
- `src/telemetry.rs`
- `src/lib.rs`, `src/cli/run.rs`
- `tests/` (specifically `metrics_endpoint.rs`, `metrics_stopped.rs`, `v11_run_now_sync_insert.rs`)
- `justfile`, `.github/workflows/ci.yml`, `Cargo.toml`, `Cargo.lock`
- `.planning/milestones/v1.1-phases/10-stop-a-running-job-hygiene-preamble/10-01-PLAN.md` (Phase 10 plan-01 structural twin for plan 15-01)

**Files scanned:** 12 source files + 4 test files + 4 config files = 20 files

**Pattern extraction date:** 2026-04-25

**Key invariants for the planner:**
1. The new `// 7d` step in `src/scheduler/run.rs` REQUIRES renaming the existing `// 7d` to `// 7e` — two `// 7d` headers in one function is an unacceptable diff (Pitfall 6).
2. `metrics::describe_counter!` MUST be paired with `metrics::counter!(…).increment(0)` in the same closure — describe alone does not register the metric (Pitfall 3).
3. `webhook_tx.try_send(...)` ONLY — never `.send().await` (Pitfall 28). The whole point of WH-02.
4. `#[async_trait::async_trait]` on BOTH the trait declaration AND every impl block (Pitfall 2).
5. cargo-deny `[licenses]` uses ONLY `allow = [...]` — `default`, `unlicensed`, `copyleft`, `deny`, `allow-osi-fsf-free` are deprecated and emit errors (Pitfall 4).
6. `continue-on-error: true` MUST be at step level in `ci.yml`, NOT job level (Pitfall 5).
7. License allowlist EXACTLY: `MIT`, `Apache-2.0`, `BSD-3-Clause`, `ISC`, `Unicode-DFS-2016`. No others without a documented justification (D-10).
8. Plan order is STRICT per D-12: `15-01` (Cargo bump) → `15-02` (cargo-deny preamble) → `15-03..N` (webhook worker scaffold). Bump-first is the project rule (Phase 10 D-12 precedent).
