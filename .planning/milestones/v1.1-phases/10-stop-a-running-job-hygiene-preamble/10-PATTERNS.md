# Phase 10: Stop-a-Running-Job + Hygiene Preamble — Pattern Map

**Mapped:** 2026-04-15
**Files analyzed:** 18 (5 NEW, 13 MODIFIED)
**Analogs found:** 18 / 18 (all role-match or better — this is a brownfield phase on a codebase that already contains every pattern Phase 10 needs)

All patterns below are extracted from real files on branch `main` at the time of mapping. Line numbers are authoritative — planner may cite them verbatim in plan actions.

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `Cargo.toml` (version bump + rand) | config | n/a (build metadata) | `Cargo.toml` itself (Phase 9 ship-v1.0 already bumped `1.0.0 → 1.0.1`) | exact |
| `src/scheduler/control.rs` (NEW, ~60 LOC) | utility (module) | event-driven (cancel token + atomic signal) | `src/scheduler/log_pipeline.rs` (small, purpose-built scheduler submodule owning one data type + a handful of methods) | role-match |
| `src/scheduler/cmd.rs` (add `Stop` variant) | model (command enum) | event-driven (mpsc → scheduler loop) | `SchedulerCmd::Reroll` in the same file (`cmd.rs:17-21`) | **exact** |
| `src/scheduler/mod.rs` (merge `active_runs`, new `Stop` arm) | service (main loop) | event-driven (`tokio::select!`) | `SchedulerCmd::RunNow` arm (`mod.rs:164-188`) + `Reload/Reroll` arms (`mod.rs:189-275`) | **exact** |
| `src/scheduler/run.rs` (`finalize_run` + `classify_failure_reason`) | service (lifecycle) | request-response (in-process) | `run.rs:238-244` status-to-string map + `run.rs:298-313` `classify_failure_reason` | **exact** (it IS the file) |
| `src/scheduler/command.rs` (`RunStatus::Stopped`) | model | n/a (enum) | `RunStatus::Shutdown` at `command.rs:24` | **exact** |
| `src/scheduler/command.rs` (executor cancel-branch stop-reason read) | executor | streaming (process → log channel) | `command.rs:127-140` `cancel.cancelled()` arm inside `execute_child` | **exact** |
| `src/scheduler/script.rs` (shares `execute_child`) | executor | streaming | same cancel arm via `execute_child` (`script.rs:104` → `command.rs:127-140`) | **exact** |
| `src/scheduler/docker.rs` (cancel branch kill-before-finalize) | executor | streaming | `docker.rs:338-358` `cancel.cancelled()` arm — already calls `docker.stop_container` before return | **exact** |
| `src/scheduler/docker_orphan.rs` (test lock only) | utility | batch (SQL UPDATE) | `docker_orphan.rs:114-143` `mark_run_orphaned` — already has the `WHERE status = 'running'` guard on both SQLite L120 and Postgres L131 | **exact** |
| `src/web/handlers/api.rs` (`stop_run` handler) | controller | request-response | `run_now` handler `api.rs:26-80` | **exact** |
| `src/web/mod.rs` (new route) | route | request-response | `run_now` route registration `web/mod.rs:79` | **exact** |
| `templates/pages/run_detail.html` (header Stop button) | component (template) | server-render | existing page-title row `run_detail.html:16-18` + existing `cd-btn-secondary` usage in `run_history.html:62-67` | role-match |
| `templates/partials/run_history.html` (per-row Stop cell) | component (template partial) | server-render | existing per-row cell pattern in same file `run_history.html:30-49` | **exact** |
| `assets/src/app.css` (new tokens + `.cd-btn-stop` + `.cd-badge--stopped`) | config (design tokens) | n/a | `.cd-badge--*` family `assets/src/app.css:172-179` + `.cd-btn-secondary` `assets/src/app.css:200-216` | **exact** |
| `design/DESIGN_SYSTEM.md` (Status Colors table row) | docs | n/a | `DESIGN_SYSTEM.md:48-66` Status Colors + Status Background Tints tables | **exact** |
| `tests/stop_race.rs` (NEW, T-V11-STOP-04..06) | test (integration) | event-driven (scheduler loop) | `tests/scheduler_integration.rs` (shutdown drain patterns) + inline tests `scheduler/mod.rs:438-501` | role-match |
| `tests/stop_executors.rs` (NEW, T-V11-STOP-09..11) | test (integration) | streaming (executor round-trip) | `tests/docker_executor.rs` + inline `command.rs:220-250` | role-match |
| `tests/process_group_kill.rs` (NEW, T-V11-STOP-07..08) | test (integration) | process-lifecycle | inline `command.rs:220-250` + `tests/docker_executor.rs` | role-match |
| `tests/docker_orphan_guard.rs` (NEW, T-V11-STOP-12..14) | test (integration) | batch (SQL) | `tests/retention_integration.rs` (SQL-lifecycle regression test pattern) | role-match |
| `tests/stop_handler.rs` (NEW, T-V11-STOP-15..16) | test (integration) | request-response | `tests/api_run_now.rs:1-150` (verbatim handler-test pattern) | **exact** |

> Note on test layout: the VALIDATION.md uses paths like `tests/scheduler/stop_race.rs`. The current `tests/` directory is **flat** (no subdirectories, only `tests/fixtures/` for data). Cargo treats each top-level `.rs` in `tests/` as a separate integration binary. Planner should decide whether to (a) keep the flat convention (`tests/stop_race.rs`, `tests/stop_executors.rs`, ...) or (b) introduce a `tests/common/mod.rs` + sibling files. The analog (`tests/api_run_now.rs`) is flat — recommend staying flat for Phase 10.

---

## Pattern Assignments

### 1. `Cargo.toml` (config — version bump + rand 0.8 → 0.9)

**Analog:** `Cargo.toml` itself.

**Imports pattern / current state (lines 1-5):**
```toml
[package]
name = "cronduit"
version = "1.0.1"
edition = "2024"
rust-version = "1.94.1"
```

**rand declaration (line 105-106):**
```toml
# Random bytes for CSRF tokens (D-11)
rand = "0.8"
```

**Action for plan 10-01 (FOUND-13 version bump):** one-line edit L3 `version = "1.0.1"` → `version = "1.1.0"`. The edit is mechanical; no other metadata fields change.

**Action for plan 10-02 (FOUND-12 rand bump):** one-line edit L106 `rand = "0.8"` → `rand = "0.9"`. Update comment to reference both CSRF tokens and `@random` slot picker. `Cargo.lock` regenerates on first `cargo build`.

**Call-site migration (see RESEARCH.md §Dependency delta for the full table):**
- `src/web/csrf.rs:10` — `use rand::RngCore;` → `use rand::Rng;`
- `src/web/csrf.rs:21` — `rand::thread_rng().fill_bytes(&mut token);` → `rand::rng().fill(&mut token[..]);`
- `src/scheduler/sync.rs:131`, `src/scheduler/reload.rs:171` — `rand::thread_rng()` → `rand::rng()`
- `src/scheduler/random.rs:97` — `rng.gen_range(min..=max)` → `rng.random_range(min..=max)`
- `src/scheduler/random.rs:268-269` — `SeedableRng` + `StdRng` paths unchanged

---

### 2. `src/scheduler/control.rs` (NEW — utility module, ~60 LOC)

**Analog:** no direct analog (new module). Structural analog is `src/scheduler/log_pipeline.rs` — a small scheduler-local utility module that defines one primary data type with a handful of methods and is imported by `mod.rs` + `run.rs`. Planner should mirror that shape: `pub mod` declaration in `scheduler/mod.rs` L7-23, type + methods in the new file, `use` re-exports where necessary.

**Scheduler `mod.rs` sub-module list** (`src/scheduler/mod.rs:7-23`) — the new `pub mod control;` line slots in alphabetically here:
```rust
pub mod cmd;
pub mod command;
pub mod docker;
// Phase 5: @random cron field resolver (RAND-01 through RAND-05).
pub mod docker_daemon;
pub mod docker_log;
pub mod docker_orphan;
pub mod docker_preflight;
pub mod docker_pull;
pub mod fire;
pub mod log_pipeline;
pub mod random;
pub mod reload;
pub mod retention;
pub mod run;
pub mod script;
pub mod sync;
```

**Reference shape to copy** (RESEARCH.md §Architecture Patterns §1 — verbatim from research, use this as the literal template for the module contents):
```rust
use std::sync::Arc;
use std::sync::atomic::AtomicU8;
use tokio_util::sync::CancellationToken;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Shutdown = 0,
    Operator = 1,
}

impl StopReason {
    pub fn from_u8(v: u8) -> Self { match v { 1 => Self::Operator, _ => Self::Shutdown } }
}

#[derive(Clone)]
pub struct RunControl {
    pub cancel: CancellationToken,
    pub stop_reason: Arc<AtomicU8>,
}

impl RunControl {
    pub fn new(cancel: CancellationToken) -> Self {
        Self {
            cancel,
            stop_reason: Arc::new(AtomicU8::new(StopReason::Shutdown as u8)),
        }
    }
    pub fn stop(&self, reason: StopReason) {
        self.stop_reason.store(reason as u8, std::sync::atomic::Ordering::SeqCst);
        self.cancel.cancel();
    }
    pub fn reason(&self) -> StopReason {
        StopReason::from_u8(self.stop_reason.load(std::sync::atomic::Ordering::SeqCst))
    }
}
```

**Dependency reuse:** `tokio_util::sync::CancellationToken` is already in-tree (`Cargo.toml:22` — `tokio-util = { version = "0.7.18", features = ["rt"] }`), same crate `run.rs` uses via the `cancel: CancellationToken` parameter. No new dep.

---

### 3. `src/scheduler/cmd.rs` (add `Stop` variant)

**Analog:** `SchedulerCmd::Reroll` — same file, lines 17-21. This is the closest analog for "request-response SchedulerCmd variant that carries a payload and a oneshot reply channel".

**Existing imports pattern (lines 1-7):**
```rust
//! Scheduler command channel types.
//!
//! D-08: mpsc channel bridges web handlers to the scheduler loop.
//! D-09: Enum designed for extensibility -- Reload and Reroll added in Phase 5.

use tokio::sync::oneshot;
```

**Existing `Reroll` variant — copy this shape exactly** (lines 17-21):
```rust
/// Re-resolve @random schedule for a specific job (RAND-04).
Reroll {
    job_id: i64,
    response_tx: oneshot::Sender<ReloadResult>,
},
```

**Also note `RunNow` which is fire-and-forget** (lines 11-12):
```rust
/// Trigger a manual run for a specific job (UI-12).
RunNow { job_id: i64 },
```

**Pattern for Phase 10 `Stop`:** planner's Gap-4 decision (RESEARCH.md §Architecture Patterns §2 recommends **Option C — scheduler replies via oneshot**) maps directly onto the `Reroll` shape. The new variant should be:

```rust
/// Stop an in-flight run by its run_id (SCHED-09, SCHED-10).
/// Responds with StopResult so the handler can distinguish the
/// normal-stop path (toast) from the race-case no-op (silent refresh).
Stop {
    run_id: i64,
    response_tx: oneshot::Sender<StopResult>,
},
```

Add a new `StopResult` type in the same file, modeled on the existing `ReloadResult`-alongside-`ReloadStatus` pair (lines 24-40):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopResult {
    /// Scheduler found the RunEntry, set stop_reason=Operator, fired cancel.
    Stopped,
    /// Run was not in the active_runs map (already finalized naturally).
    /// Handler must silently refresh with no toast (D-07).
    AlreadyFinalized,
    /// run_id did not match any known run.
    NotFound,
}
```

Pattern rationale: `StopResult` is a tiny `Copy` enum rather than a struct because Stop has no diff-summary payload to return (unlike Reload). `oneshot::Sender<StopResult>` matches the existing `oneshot::Sender<ReloadResult>` wire shape bit-for-bit, so the handler-side `match resp_rx.await { Ok(result) => ..., Err(_) => 503 }` pattern is reusable verbatim.

---

### 4. `src/scheduler/mod.rs` (merge `active_runs` + new `Stop` arm)

**Analog:** the file itself — all analogs are inline. Multiple critical line-number-locked patterns:

**Imports pattern (lines 25-37):**
```rust
use crate::db::DbPool;
use crate::db::queries::DbJob;
use crate::scheduler::log_pipeline::LogLine;
use bollard::Docker;
use chrono::Utc;
use chrono_tz::Tz;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;
```

**Active-runs field declaration — the ONE line that must change type** (lines 55-56):
```rust
/// Broadcast channels for active runs (shared with AppState for SSE, UI-14).
pub active_runs: Arc<RwLock<HashMap<i64, tokio::sync::broadcast::Sender<LogLine>>>>,
```
Target (per D-01):
```rust
pub active_runs: Arc<RwLock<HashMap<i64, RunEntry>>>,
```
Add `pub struct RunEntry { pub broadcast_tx: tokio::sync::broadcast::Sender<LogLine>, pub control: RunControl }` in this file (near `RunResult` L40-43) so it's defined alongside `SchedulerLoop`, and re-export via `pub use`. `RunControl` import: `use crate::scheduler::control::{RunControl, StopReason};`.

**Existing `RunNow` match arm — structural analog for the new `Stop` arm** (lines 162-188):
```rust
cmd = self.cmd_rx.recv() => {
    match cmd {
        Some(cmd::SchedulerCmd::RunNow { job_id }) => {
            if let Some(job) = self.jobs.get(&job_id) {
                let child_cancel = self.cancel.child_token();
                join_set.spawn(run::run_job(
                    self.pool.clone(),
                    self.docker.clone(),
                    job.clone(),
                    "manual".to_string(),
                    child_cancel,
                    self.active_runs.clone(),
                ));
                tracing::info!(
                    target: "cronduit.scheduler",
                    job_id,
                    job_name = %job.name,
                    "manual run triggered via command channel"
                );
            } else {
                tracing::warn!(
                    target: "cronduit.scheduler",
                    job_id,
                    "RunNow requested for unknown job_id"
                );
            }
        }
```

**Existing `Reroll` match arm — structural analog for the oneshot-reply shape** (lines 263-275):
```rust
Some(cmd::SchedulerCmd::Reroll { job_id, response_tx }) => {
    let (result, new_heap) = reload::do_reroll(
        &self.pool,
        job_id,
        &mut self.jobs,
        self.tz,
    ).await;
    if let Some(h) = new_heap {
        heap = h;
        jobs_vec = self.jobs.values().cloned().collect();
    }
    let _ = response_tx.send(result);
}
```

**Core pattern — new `Stop` match arm to plug into the `tokio::select!` loop:**
```rust
Some(cmd::SchedulerCmd::Stop { run_id, response_tx }) => {
    // Gap 4 / Option C: the map IS the race token (RESEARCH.md §Architecture §1
    // Invariant 3). If the executor has already called
    // active_runs.write().await.remove(&run_id) at run.rs:276, lookup returns
    // None and the race-case branch replies AlreadyFinalized.
    let maybe_control = {
        let active = self.active_runs.read().await;
        active.get(&run_id).map(|entry| entry.control.clone())
    };
    let result = match maybe_control {
        Some(control) => {
            control.stop(crate::scheduler::control::StopReason::Operator);
            tracing::info!(
                target: "cronduit.scheduler",
                run_id,
                "stop requested via command channel"
            );
            cmd::StopResult::Stopped
        }
        None => {
            tracing::debug!(
                target: "cronduit.scheduler",
                run_id,
                "Stop arrived after run finalized (race case)"
            );
            cmd::StopResult::AlreadyFinalized
        }
    };
    let _ = response_tx.send(result);
}
```

**Critical invariants the merge must preserve** (from RESEARCH.md §Architecture §1 Invariants 1-4, verbatim constraints):
1. Lock scope — `.read().await` / `.write().await` spans only the HashMap op, never across subsequent awaits. Today's `L102`/`L276` patterns in `run.rs` must stay single-statement.
2. Drop order — `join_next()` fires AFTER `run.rs:276` `remove()`, so the scheduler loop's `Stop` lookup sees the empty map.
3. `broadcast_tx` refcount — entry-holds-one-clone + executor-holds-one-clone-dropped-at-L277 must remain balanced.
4. `RunControl` is `Clone` (its fields are `CancellationToken` + `Arc<AtomicU8>`, both `Clone`) — no extra wrapping needed.

**spawn() signature (lines 379-403):**
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
    active_runs: Arc<RwLock<HashMap<i64, tokio::sync::broadcast::Sender<LogLine>>>>,
) -> JoinHandle<()> { ... }
```
Change only the last parameter type: `Arc<RwLock<HashMap<i64, RunEntry>>>`.

**Test helper (lines 417-420) — also needs type change:**
```rust
fn test_active_runs()
-> Arc<RwLock<HashMap<i64, tokio::sync::broadcast::Sender<log_pipeline::LogLine>>>> {
    Arc::new(RwLock::new(HashMap::new()))
}
```
Target: returns `Arc<RwLock<HashMap<i64, RunEntry>>>` — still an empty map, just a different value type.

---

### 5. `src/scheduler/run.rs` (`run_job` insert point + `finalize_run` mapping + `classify_failure_reason`)

**Analog:** the file itself — three surgical pattern points.

**Function signature — one-line type change** (line 65-72):
```rust
pub async fn run_job(
    pool: DbPool,
    docker: Option<Docker>,
    job: DbJob,
    trigger: String,
    cancel: CancellationToken,
    active_runs: Arc<RwLock<HashMap<i64, tokio::sync::broadcast::Sender<LogLine>>>>,
) -> RunResult {
```
Target: replace last param type with `Arc<RwLock<HashMap<i64, RunEntry>>>`. Also — per RESEARCH.md §Architecture §1 "Where the RunEntry must be constructed" — the scheduler loop constructs `RunControl` BEFORE spawning and passes it in, so this signature likely gains a new `control: RunControl` param. Alternative: construct `RunControl::new(cancel.clone())` inside `run_job` — planner picks.

**RunEntry insert point — current broadcast insert at lines 100-105:**
```rust
// 1b. Create broadcast channel for SSE subscribers (UI-14, D-03).
let (broadcast_tx, _rx) = tokio::sync::broadcast::channel::<LogLine>(256);
active_runs
    .write()
    .await
    .insert(run_id, broadcast_tx.clone());
```
Target:
```rust
let (broadcast_tx, _rx) = tokio::sync::broadcast::channel::<LogLine>(256);
let run_control = RunControl::new(cancel.clone()); // or accept as param
active_runs
    .write()
    .await
    .insert(run_id, RunEntry {
        broadcast_tx: broadcast_tx.clone(),
        control: run_control.clone(),
    });
```

**Finalize status-to-string map — the ONE place `stopped` is threaded through** (lines 238-244):
```rust
// 7. Finalize run.
let status_str = match exec_result.status {
    RunStatus::Success => "success",
    RunStatus::Failed => "failed",
    RunStatus::Timeout => "timeout",
    RunStatus::Shutdown => "cancelled",
    RunStatus::Error => "error",
};
```
Target (adds one arm):
```rust
RunStatus::Stopped => "stopped",
```
This single string is the canonical DB value (D-10). Every metric label, every template string interpolation, and every SQL test assertion keys off this.

**Failure classification — `stopped` MUST NOT be classified as a failure** (lines 298-313):
```rust
fn classify_failure_reason(status: &str, error_msg: Option<&str>) -> FailureReason {
    match status {
        "timeout" => FailureReason::Timeout,
        "failed" => FailureReason::ExitNonzero,
        "error" => match error_msg { ... },
        // "cancelled" (shutdown) and any other unexpected status
        _ => FailureReason::Unknown,
    }
}
```
**No change required.** The existing `_` catch-all already routes `"stopped"` to `FailureReason::Unknown`. But per D-10: the failure counter increment at `run.rs:270-273` must NOT fire for `stopped`:
```rust
if status_str != "success" {
    let reason = classify_failure_reason(status_str, exec_result.error_message.as_deref());
    metrics::counter!("cronduit_run_failures_total", ...).increment(1);
}
```
Target — tighten the predicate to exclude `stopped`:
```rust
if status_str != "success" && status_str != "stopped" {
    ...
}
```

**Remove-from-map point — no structural change, drops `RunEntry` instead of `Sender`** (lines 275-277):
```rust
// 7c. Remove broadcast sender so SSE subscribers get RecvError::Closed (UI-14, D-02).
active_runs.write().await.remove(&run_id);
drop(broadcast_tx);
```
Stays verbatim — the remove now drops a `RunEntry` which itself drops its contained `broadcast_tx` clone. Invariant 3 (RESEARCH.md) is preserved by refcount arithmetic.

**Test helper + test insert sites** (lines 364-366, 415, 481, 533, 585-586) — type-flow change only. Every `.insert(run_id, tx)` becomes `.insert(run_id, RunEntry { broadcast_tx: tx, control: RunControl::new(CancellationToken::new()) })` or similar. Planner should consider a tiny helper `fn test_entry(tx) -> RunEntry` in the test module.

---

### 6. `src/scheduler/command.rs` (add `RunStatus::Stopped` + cancel branch reads stop_reason)

**Analog:** `RunStatus::Shutdown` variant and the `cancel.cancelled()` arm inside `execute_child`.

**`RunStatus` enum (lines 14-27):**
```rust
/// Status of a completed job run.
#[derive(Debug, Clone, PartialEq)]
pub enum RunStatus {
    /// Exited with code 0.
    Success,
    /// Exited with non-zero code.
    Failed,
    /// Killed due to timeout.
    Timeout,
    /// Cancelled due to graceful shutdown.
    Shutdown,
    /// Could not start or other error.
    Error,
}
```
Target: add one variant `/// Killed by operator via UI Stop button (SCHED-09). Stopped,`.

**`execute_child` signature (lines 58-63):**
```rust
pub(crate) async fn execute_child(
    mut child: tokio::process::Child,
    timeout: Duration,
    cancel: CancellationToken,
    sender: LogSender,
) -> ExecResult {
```
Target: add a parameter `control: &RunControl` (or pass only `stop_reason: Arc<AtomicU8>`) so the cancel branch can distinguish Operator from Shutdown. Since `script.rs:104` also calls `execute_child(child, timeout, cancel, sender)` and `command.rs:217` is the other caller, both call sites update in lockstep.

**Cancel branch — the CRITICAL pattern** (lines 127-140):
```rust
_ = cancel.cancelled() => {
    // Graceful shutdown — kill process group
    kill_process_group(&child);
    let _ = child.wait().await;
    let _ = stdout_task.await;
    let _ = stderr_task.await;
    sender.close();

    ExecResult {
        exit_code: None,
        status: RunStatus::Shutdown,
        error_message: Some("cancelled due to shutdown".to_string()),
    }
}
```
Target — read `stop_reason` AFTER `cancel.cancelled()` yields (the ordering is safe because the scheduler loop sets the atomic BEFORE firing the cancel token — see `RunControl::stop` definition):
```rust
_ = cancel.cancelled() => {
    kill_process_group(&child);
    let _ = child.wait().await;
    let _ = stdout_task.await;
    let _ = stderr_task.await;
    sender.close();

    let (status, msg) = match control.reason() {
        StopReason::Operator => (RunStatus::Stopped, "stopped by operator".to_string()),
        StopReason::Shutdown => (RunStatus::Shutdown, "cancelled due to shutdown".to_string()),
    };
    ExecResult { exit_code: None, status, error_message: Some(msg) }
}
```

**Process-group kill pattern — PRESERVE, do NOT replace with `kill_on_drop`** (lines 146-167, D-17 regression lock):
```rust
fn kill_process_group(child: &tokio::process::Child) {
    if let Some(pid) = child.id() {
        let pid_i32: i32 = match pid.try_into() {
            Ok(p) => p,
            Err(_) => {
                tracing::error!(target: "cronduit.executor", pid, "PID exceeds i32::MAX");
                return;
            }
        };
        unsafe { libc::kill(-pid_i32, libc::SIGKILL); }
    }
}
```
T-V11-STOP-07/08 tests lock this — any refactor to `kill_on_drop(true)` must fail the tests (see RESEARCH.md Correction #1).

**Process-group spawn pattern** (`command.rs:199-205`, `script.rs:86-91`) — same regression lock:
```rust
let child = Command::new(&argv[0])
    .args(&argv[1..])
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .process_group(0)   // ← preserve this line; tests lock it
    .spawn()
```

---

### 7. `src/scheduler/script.rs` (shares `execute_child`)

**Analog:** `script.rs:104` `execute_child(child, timeout, cancel, sender).await` — the call that now must thread `control` through. No script-specific logic changes; the cancel branch lives entirely in `command.rs::execute_child`.

**Current call (line 104):**
```rust
let result = execute_child(child, timeout, cancel, sender).await;
```
Target: `execute_child(child, timeout, cancel, sender, &control).await` (or similar — matches whatever shape `command.rs` adopts).

---

### 8. `src/scheduler/docker.rs` (cancel branch — kill-before-finalize with operator reason)

**Analog:** `docker.rs:338-358` — existing `cancel.cancelled()` arm that ALREADY calls `docker.stop_container` (SIGTERM with 10s grace) before returning `RunStatus::Shutdown`. This is the "kill before finalize" pattern the research correction requires.

**Existing cancel branch (lines 338-358):**
```rust
_ = cancel.cancelled() => {
    // Shutdown cancellation: stop with 10s grace (D-06).
    sender.send(make_log_line("system", "[shutdown signal received, stopping container]".to_string()));
    let _ = docker.stop_container(
        &container_id,
        Some(StopContainerOptions {
            t: Some(10),
            ..Default::default()
        }),
    ).await;

    // D-05: Drain logs to EOF.
    let _ = log_handle.await;
    sender.close();

    ExecResult {
        exit_code: None,
        status: RunStatus::Shutdown,
        error_message: Some("cancelled due to shutdown".to_string()),
    }
}
```

**Target pattern — distinguish reason after `cancel.cancelled()` yields:**
```rust
_ = cancel.cancelled() => {
    let reason = control.reason(); // OR read the Arc<AtomicU8> directly
    let log_msg = match reason {
        StopReason::Operator => "[stop signal from operator, killing container]",
        StopReason::Shutdown => "[shutdown signal received, stopping container]",
    };
    sender.send(make_log_line("system", log_msg.to_string()));

    // For Operator stop, prefer `docker kill -s KILL` (immediate) over
    // `stop_container` (SIGTERM + 10s grace). RESEARCH.md §Correction #2
    // and the canonical_refs note: "the bollard `docker kill -s KILL`
    // path must run before finalize". v1.1 keeps stop_container for
    // Shutdown to preserve the 10s grace; Operator gets the kill path.
    match reason {
        StopReason::Operator => {
            let _ = docker.kill_container(
                &container_id,
                Some(KillContainerOptions { signal: "KILL" }),
            ).await;
        }
        StopReason::Shutdown => {
            let _ = docker.stop_container(
                &container_id,
                Some(StopContainerOptions { t: Some(10), ..Default::default() }),
            ).await;
        }
    }

    let _ = log_handle.await;
    sender.close();

    let (status, msg) = match reason {
        StopReason::Operator => (RunStatus::Stopped, "stopped by operator".to_string()),
        StopReason::Shutdown => (RunStatus::Shutdown, "cancelled due to shutdown".to_string()),
    };
    ExecResult { exit_code: None, status, error_message: Some(msg) }
}
```

**Cleanup invariant** (`docker.rs:361-368`): the post-select block `maybe_cleanup_container(docker, &container_id, config.delete, job_name, run_id).await` runs after BOTH the natural-exit and cancel branches. It must continue to run for the Operator-stop path so containers are removed per the job's `delete` setting. No change to that block.

---

### 9. `src/scheduler/docker_orphan.rs` (test lock only — D-16)

**Analog:** `docker_orphan.rs:114-143` — the function IS the pattern. No code change; tests-only.

**The guard (preserve verbatim)** — SQLite branch line 119-128:
```rust
async fn mark_run_orphaned(pool: &DbPool, run_id: i64) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    match pool.writer() {
        PoolRef::Sqlite(p) => {
            sqlx::query(
                "UPDATE job_runs SET status = ?1, error_message = ?2, end_time = ?3 WHERE id = ?4 AND status = 'running'",
            )
            .bind("error")
            .bind("orphaned at restart")
            .bind(&now)
            .bind(run_id)
            .execute(p)
            .await?;
        }
```
Postgres branch (lines 129-139) has the same `AND status = 'running'` guard with `$N` placeholder syntax.

**Test pattern (T-V11-STOP-12..14):** pre-seed a row with `status = 'stopped'` / `'success'` / `'failed'`, call `mark_run_orphaned`, assert the row is UNCHANGED. See `tests/docker_orphan_guard.rs` analog below.

---

### 10. `src/web/handlers/api.rs` (NEW `stop_run` handler)

**Analog:** `run_now` handler `api.rs:26-80` — the canonical CSRF-gated scheduler-command handler. This is an **exact** analog; the new handler is a near-clone structurally.

**Imports pattern (lines 1-19):**
```rust
//! API handlers for state-changing operations.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum_extra::extract::CookieJar;
use axum_htmx::HxEvent;
use axum_htmx::HxResponseTrigger;
use serde::Deserialize;
use serde_json::json;

use crate::db::queries;
use crate::scheduler::cmd::{ReloadStatus, SchedulerCmd};
use crate::web::AppState;
use crate::web::csrf;

#[derive(Deserialize)]
pub struct CsrfForm {
    csrf_token: String,
}
```

**Core CSRF pattern — verbatim from `run_now` L32-40** (copy into `stop_run`):
```rust
let cookie_token = cookies
    .get(csrf::CSRF_COOKIE_NAME)
    .map(|c| c.value().to_string())
    .unwrap_or_default();

if !csrf::validate_csrf(&cookie_token, &form.csrf_token) {
    return (StatusCode::FORBIDDEN, "CSRF token mismatch").into_response();
}
```

**Job/run lookup pattern — analog is `run_now` L42-47, but queries a run instead of a job:**
```rust
// run_now L42-47 — Verify job exists (T-03-16)
let job = match queries::get_job_by_id(&state.pool, job_id).await {
    Ok(Some(job)) => job,
    Ok(None) => return (StatusCode::NOT_FOUND, "Job not found").into_response(),
    Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
};
```
Target for `stop_run`: `queries::get_run_by_id(&state.pool, run_id).await` — the handler needs the `job_name` for the toast ("Stopped: {job_name}"), which means fetching run → job_id → job or a joined query. Planner picks. The existing `get_run_by_id` + a follow-up `get_job_by_id` is simplest.

**Scheduler-command + oneshot-reply pattern — analog is `reroll` L222-235:**
```rust
let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
match state
    .cmd_tx
    .send(SchedulerCmd::Reroll {
        job_id,
        response_tx: resp_tx,
    })
    .await
{
    Ok(()) => match resp_rx.await {
        Ok(result) => { ... }
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "Scheduler shutting down").into_response(),
    },
    Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "Scheduler shutting down").into_response(),
}
```

**HX-Trigger + HX-Refresh toast response — verbatim from `run_now` L63-72** (copy for the `Stopped` branch):
```rust
let event = HxEvent::new_with_data(
    "showToast",
    json!({"message": format!("Run queued: {}", job.name), "level": "info"}),
)
.expect("toast event serialization");

let mut headers = axum::http::HeaderMap::new();
headers.insert("HX-Refresh", "true".parse().unwrap());

(HxResponseTrigger::normal([event]), headers, StatusCode::OK).into_response()
```

**Target `stop_run` core logic — composed from the analogs above:**
```rust
pub async fn stop_run(
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
    cookies: CookieJar,
    axum::Form(form): axum::Form<CsrfForm>,
) -> impl IntoResponse {
    // 1. CSRF — verbatim from run_now L32-40
    // ...
    // 2. Look up run + job for the toast message
    let run = match queries::get_run_by_id(&state.pool, run_id).await {
        Ok(Some(r)) => r,
        Ok(None) => return (StatusCode::NOT_FOUND, "Run not found").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    };
    let job = match queries::get_job_by_id(&state.pool, run.job_id).await {
        Ok(Some(j)) => j,
        Ok(None) => return (StatusCode::NOT_FOUND, "Job not found").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    };

    // 3. Dispatch Stop command with oneshot reply (analog: reroll L222-235)
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    match state.cmd_tx.send(SchedulerCmd::Stop { run_id, response_tx: resp_tx }).await {
        Ok(()) => match resp_rx.await {
            Ok(StopResult::Stopped) => {
                // Normal path — toast + HX-Refresh (verbatim from run_now L63-72)
                let event = HxEvent::new_with_data(
                    "showToast",
                    json!({"message": format!("Stopped: {}", job.name), "level": "info"}),
                ).expect("toast event serialization");
                let mut headers = axum::http::HeaderMap::new();
                headers.insert("HX-Refresh", "true".parse().unwrap());
                (HxResponseTrigger::normal([event]), headers, StatusCode::OK).into_response()
            }
            Ok(StopResult::AlreadyFinalized) => {
                // D-07: silent refresh, no toast
                let mut headers = axum::http::HeaderMap::new();
                headers.insert("HX-Refresh", "true".parse().unwrap());
                (headers, StatusCode::OK).into_response()
            }
            Ok(StopResult::NotFound) => {
                (StatusCode::NOT_FOUND, "Run not found").into_response()
            }
            Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "Scheduler shutting down").into_response(),
        },
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "Scheduler is shutting down").into_response(),
    }
}
```

Note: the 503 body string matches `run_now` exactly (`"Scheduler is shutting down"`) per UI-SPEC Copy table.

---

### 11. `src/web/mod.rs` (new route)

**Analog:** `run_now` route registration at `web/mod.rs:79`.

**Current router block (lines 50-89):**
```rust
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(handlers::dashboard::dashboard))
        ...
        .route("/api/jobs/{id}/run", post(handlers::api::run_now))
        .route("/api/reload", post(handlers::api::reload))
        .route("/api/jobs/{id}/reroll", post(handlers::api::reroll))
        ...
```

**Target — add one line alongside the other `api::*` POST routes (immediately after L81):**
```rust
.route("/api/runs/{run_id}/stop", post(handlers::api::stop_run))
```

**`AppState.active_runs` field declaration (lines 38-47)** — type change only:
```rust
pub active_runs: std::sync::Arc<
    tokio::sync::RwLock<
        std::collections::HashMap<
            i64,
            tokio::sync::broadcast::Sender<crate::scheduler::log_pipeline::LogLine>,
        >,
    >,
>,
```
Target: `HashMap<i64, crate::scheduler::RunEntry>` (assuming `RunEntry` is exported from `scheduler::mod`).

---

### 12. `templates/pages/run_detail.html` (header Stop button)

**Analog:** existing page-title row `run_detail.html:16-18`.

**Current header pattern (lines 15-18):**
```html
<!-- Page title -->
<div class="flex items-center justify-between mb-6">
  <h1 style="font-size:var(--cd-text-xl);font-weight:700;letter-spacing:-0.02em">Run #{{ run.id }}</h1>
</div>
```

**Existing `is_running` gating pattern at L64** (already in this file — reuse it):
```html
<!-- Log Viewer -->
{% if is_running %}
<div id="log-container">
  ...
```

**Existing CSRF form pattern** — not yet in this template; the closest reference is the HTMX form convention. `csrf_token` is available via the template context (the run_detail handler at `web/handlers/run_detail.rs:176-181` already renders `is_running`; planner confirms `csrf_token` plumbs through). Check analogs in `templates/pages/settings.html` or `job_detail.html` for existing `{{ csrf_token }}` usage.

**Target — verbatim from UI-SPEC §Surface A:**
```html
<div class="flex items-center justify-between mb-6">
  <h1 style="font-size:var(--cd-text-xl);font-weight:700;letter-spacing:-0.02em">Run #{{ run.id }}</h1>
  {% if is_running %}
  <form hx-post="/api/runs/{{ run.id }}/stop"
        hx-swap="none"
        hx-disabled-elt="this"
        style="display:inline">
    <input type="hidden" name="csrf_token" value="{{ csrf_token }}">
    <button type="submit" class="cd-btn-stop" aria-label="Stop run #{{ run.id }}">Stop</button>
  </form>
  {% endif %}
</div>
```

**Badge rendering at L26** — no change required; `cd-badge--{{ run.status }}` already interpolates, so `cd-badge--stopped` renders automatically once the CSS class exists.

---

### 13. `templates/partials/run_history.html` (per-row Stop cell)

**Analog:** the existing per-row cells `run_history.html:30-49`.

**Existing row pattern (lines 29-49):**
```html
{% for run in runs %}
<tr class="hover:bg-[var(--cd-bg-hover)] border-b border-[var(--cd-border-subtle)]">
  <td class="py-2 px-4">
    <a href="/jobs/{{ job_id }}/runs/{{ run.id }}" class="no-underline">
      <span class="cd-badge cd-badge--{{ run.status }}">{{ run.status_label }}</span>
    </a>
  </td>
  <td class="py-2 px-4" style="font-size:var(--cd-text-base);color:var(--cd-text-secondary)">
    {% if run.trigger == "manual" %}
    <span style="color:var(--cd-status-running)">{{ run.trigger }}</span>
    {% else %}
    {{ run.trigger }}
    {% endif %}
  </td>
  <td class="py-2 px-4" style="font-size:var(--cd-text-base);color:var(--cd-text-secondary)">{{ run.start_time }}</td>
  <td class="py-2 px-4" style="font-size:var(--cd-text-base);color:var(--cd-text-secondary)">{{ run.duration_display }}</td>
  <td class="py-2 px-4" style="font-size:var(--cd-text-base);color:var(--cd-text-secondary)">
    {% match run.exit_code %}
      {% when Some with (code) %}{{ code }}{% when None %}—{% endmatch %}
  </td>
</tr>
{% endfor %}
```

**Existing header row pattern (lines 19-26) — add a new `<th>`:**
```html
<tr style="background:var(--cd-bg-surface-raised)">
  <th class="text-left py-2 px-4" style="font-size:var(--cd-text-xs);font-weight:700;text-transform:uppercase;letter-spacing:0.1em;color:var(--cd-text-secondary)">Status</th>
  ...
</tr>
```

**Target — add per UI-SPEC §Surface B (6th column with `width:1%` shrink-to-content):**
- New `<th>` (empty label) after the Exit Code header:
```html
<th class="text-right py-2 px-4" style="font-size:var(--cd-text-xs);font-weight:700;text-transform:uppercase;letter-spacing:0.1em;color:var(--cd-text-secondary);width:1%"></th>
```
- New `<td>` after the Exit Code cell:
```html
<td class="py-2 px-4" style="text-align:right">
  {% if run.status == "running" %}
  <form hx-post="/api/runs/{{ run.id }}/stop"
        hx-swap="none"
        hx-disabled-elt="this"
        style="display:inline">
    <input type="hidden" name="csrf_token" value="{{ csrf_token }}">
    <button type="submit" class="cd-btn-stop cd-btn-stop--compact" aria-label="Stop run #{{ run.id }}">Stop</button>
  </form>
  {% endif %}
</td>
```

**Note:** this partial is also HTMX-polled every 2s (existing L8-11 wrapper) — the Stop button naturally appears/disappears across polls without JS. The `{{ csrf_token }}` must be threaded through from the `job_runs_partial` handler (`web/handlers/job_detail.rs`); planner confirms.

---

### 14. `assets/src/app.css` (new `.cd-badge--stopped` + `.cd-btn-stop` + tokens)

> **IMPORTANT:** `assets/static/app.css` is the **Tailwind output bundle** (minified, ~13 KB, single-line). Source-of-truth is `assets/src/app.css` — that is where `@layer components` + `:root` token declarations live. Phase 10 edits the **src** file; the rebuild step (tailwindcss --watch or make/just recipe) regenerates the static bundle. Planner must NOT hand-edit `assets/static/app.css`.

**Analog:** `.cd-badge--*` family at `assets/src/app.css:172-179` + `.cd-btn-secondary` at `assets/src/app.css:200-216`.

**Existing badge pattern — the mechanical template (lines 162-179):**
```css
@layer components {
  .cd-badge {
    font-size: var(--cd-text-xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    padding: var(--cd-space-1) var(--cd-space-2);
    border-radius: 4px;
    display: inline-block;
    line-height: 1.5;
  }
  .cd-badge--success { color: var(--cd-status-active); background: var(--cd-status-active-bg); }
  .cd-badge--failed { color: var(--cd-status-error); background: var(--cd-status-error-bg); }
  .cd-badge--running { color: var(--cd-status-running); background: var(--cd-status-running-bg); }
  .cd-badge--timeout { color: var(--cd-status-disabled); background: var(--cd-status-disabled-bg); }
  .cd-badge--error { color: var(--cd-status-error); background: var(--cd-status-error-bg); }
  .cd-badge--shutdown { color: var(--cd-text-secondary); background: var(--cd-bg-surface-raised); }
  .cd-badge--random { color: var(--cd-green); background: var(--cd-green-dim); }
  .cd-badge--disabled { color: var(--cd-status-disabled); background: var(--cd-status-disabled-bg); }
```

**Existing `.cd-btn-secondary` — the CLOSEST analog for `.cd-btn-stop` visual weight** (lines 200-216). **This is the most important analog in the phase — the Stop button is structurally a `cd-btn-secondary` with a different hover tint:**
```css
.cd-btn-secondary {
  background: transparent;
  color: var(--cd-text-primary);
  font-size: var(--cd-text-base);
  font-weight: 400;
  padding: var(--cd-space-2) var(--cd-space-4);
  border-radius: 8px;
  border: 1px solid var(--cd-border);
  cursor: pointer;
  transition: background 0.15s ease;
}
.cd-btn-secondary:hover { background: var(--cd-bg-hover); }
.cd-btn-secondary:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px var(--cd-green-dim);
  border-color: var(--cd-border-focus);
}
```

**Existing token declaration pattern — dark mode `:root`** (lines 28-36 in `assets/src/app.css`):
```css
/* Status */
--cd-status-active: #34d399;
--cd-status-running: #60a5fa;
--cd-status-disabled: #fbbf24;
--cd-status-error: #f87171;
--cd-status-active-bg: rgba(52, 211, 153, 0.12);
--cd-status-running-bg: rgba(96, 165, 250, 0.12);
--cd-status-disabled-bg: rgba(251, 191, 36, 0.12);
--cd-status-error-bg: rgba(248, 113, 113, 0.12);
```

**Existing light mode mirror** (lines 79-91):
```css
[data-theme="light"] {
  ...
  --cd-status-active: #059669;
  --cd-status-running: #2563eb;
  --cd-status-disabled: #d97706;
  --cd-status-error: #dc2626;
  --cd-status-active-bg: rgba(5, 150, 105, 0.08);
  ...
```

**Plus the duplicate `@media (prefers-color-scheme: light)` block at L110-136** — token additions must land in all three places (dark `:root`, `[data-theme="light"]`, `@media prefers-color-scheme` fallback) or the stopped badge goes unstyled in whichever path is skipped.

**Target — 3 edits to `assets/src/app.css`:**

**Edit 1 — add tokens to dark `:root`** (after L36):
```css
--cd-status-stopped: #94a3b8;
--cd-status-stopped-bg: rgba(148, 163, 184, 0.12);
```
Hex values per UI-SPEC §Color (slate-400, confirmed WCAG AAA 8.9:1 against `--cd-bg-surface`).

**Edit 2 — add tokens to both light-mode blocks** (after L91 in `[data-theme="light"]` and after L122 in the `@media` block):
```css
--cd-status-stopped: #64748b;
--cd-status-stopped-bg: rgba(100, 116, 139, 0.08);
```

**Edit 3 — add `.cd-badge--stopped` and `.cd-btn-stop*` inside `@layer components`** (after L216):
```css
.cd-badge--stopped { color: var(--cd-status-stopped); background: var(--cd-status-stopped-bg); }

.cd-btn-stop {
  background: transparent;
  color: var(--cd-text-primary);
  font-size: var(--cd-text-base);
  font-weight: 400;
  padding: var(--cd-space-2) var(--cd-space-4);
  border-radius: 8px;
  border: 1px solid var(--cd-border);
  cursor: pointer;
  font-family: inherit;
  min-height: 44px;
  transition: background 0.15s ease, border-color 0.15s ease, color 0.15s ease;
}
.cd-btn-stop:hover {
  background: var(--cd-status-stopped-bg);
  border-color: var(--cd-status-stopped);
  color: var(--cd-status-stopped);
}
.cd-btn-stop:active {
  background: var(--cd-status-stopped-bg);
  border-color: var(--cd-status-stopped);
  color: var(--cd-status-stopped);
  transform: translateY(1px);
}
.cd-btn-stop:focus-visible {
  outline: none;
  border-color: var(--cd-border-focus);
  box-shadow: 0 0 0 2px var(--cd-green-dim);
}
.cd-btn-stop[disabled],
.cd-btn-stop[aria-busy="true"] {
  cursor: not-allowed;
  opacity: 0.6;
  pointer-events: none;
}
.cd-btn-stop--compact {
  min-height: 0;
  padding: var(--cd-space-1) var(--cd-space-2);
  font-size: var(--cd-text-sm);
}
```

**Build step:** after editing `assets/src/app.css`, the Tailwind CLI rebuild step writes to `assets/static/app.css`. Planner must run the project's tailwind rebuild (check `Justfile` / `Makefile` / `tailwind.config.js` neighborhood) OR — if the build embeds it via `rust-embed` and serves from disk in debug mode — rebuilds happen at `cargo run` time automatically.

---

### 15. `design/DESIGN_SYSTEM.md` (Status Colors table row)

**Analog:** `DESIGN_SYSTEM.md:48-66` — Status Colors table + Status Background Tints table.

**Existing table (lines 48-66):**
```markdown
### 2.2 Status Colors

All status colors share the same saturation range (~55-70%) and lightness range (~55-65% in dark mode) to feel cohesive.

| Token | Dark Mode | Light Mode | Semantic | Usage |
|---|---|---|---|---|
| `--cd-status-active` | `#34d399` | `#059669` | Active/Success | Running successfully, healthy |
| `--cd-status-running` | `#60a5fa` | `#2563eb` | Running/In-Progress | Job currently executing |
| `--cd-status-disabled` | `#fbbf24` | `#d97706` | Disabled/Warning | Paused jobs, warnings |
| `--cd-status-error` | `#f87171` | `#dc2626` | Error/Failed | Failed jobs, errors |

#### Status Background Tints (for badges, pills, table rows)

| Token | Dark Mode | Light Mode |
|---|---|---|
| `--cd-status-active-bg` | `rgba(52, 211, 153, 0.12)` | `rgba(5, 150, 105, 0.08)` |
| `--cd-status-running-bg` | `rgba(96, 165, 250, 0.12)` | `rgba(37, 99, 235, 0.08)` |
| `--cd-status-disabled-bg` | `rgba(251, 191, 36, 0.12)` | `rgba(217, 119, 6, 0.08)` |
| `--cd-status-error-bg` | `rgba(248, 113, 113, 0.12)` | `rgba(220, 38, 38, 0.08)` |
```

**Target — append one row to each table:**
```markdown
| `--cd-status-stopped` | `#94a3b8` | `#64748b` | Operator-Interrupt | Jobs stopped via UI; NOT a failure |
```
and for the tints table:
```markdown
| `--cd-status-stopped-bg` | `rgba(148, 163, 184, 0.12)` | `rgba(100, 116, 139, 0.08)` |
```

Also look for (and update, if present) the sample badge HTML at L198 and the sample `:root` token dump at L278+ and L311+ — those are snapshots of the token state, so adding stopped there preserves the docs' source-of-truth property.

---

### 16. `tests/stop_handler.rs` (NEW — T-V11-STOP-15..16)

**Analog:** `tests/api_run_now.rs` — the **exact** analog. Structure, helpers, imports all copy verbatim.

**Imports pattern (`api_run_now.rs:14-27`):**
```rust
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use tower::ServiceExt; // brings .oneshot()

use cronduit::db::DbPool;
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::telemetry::setup_metrics;
use cronduit::web::csrf::CSRF_COOKIE_NAME;
use cronduit::web::handlers::api::run_now;
use cronduit::web::{AppState, ReloadState};
```

**`build_test_app` helper** (`api_run_now.rs:35-65`) — copy verbatim, swap the route and handler:
```rust
async fn build_test_app() -> (Router, DbPool, tokio::sync::mpsc::Receiver<SchedulerCmd>) {
    let pool = DbPool::connect("sqlite::memory:").await.expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);
    let metrics_handle = setup_metrics();
    let state = AppState {
        started_at: chrono::Utc::now(),
        version: "test",
        pool: pool.clone(),
        cmd_tx,
        config_path: std::path::PathBuf::from("/tmp/cronduit-test.toml"),
        tz: chrono_tz::UTC,
        last_reload: Arc::new(Mutex::new(None::<ReloadState>)),
        watch_config: false,
        metrics_handle,
        active_runs: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };
    let router = Router::new()
        .route("/api/jobs/{id}/run", post(run_now))
        .with_state(state);
    (router, pool, cmd_rx)
}
```

**Target for `stop_handler.rs`:** swap `run_now` → `stop_run`, route → `/api/runs/{run_id}/stop`, seed a run row (not just a job), and drain the `cmd_rx` to assert a `SchedulerCmd::Stop` variant arrived. For the oneshot-reply shape, the test must ALSO send a `StopResult` back via the oneshot tx, OR run a tiny mock scheduler task inside the test that replies to whatever it receives. The latter is cleaner — see `tests/reload_api.rs` for a oneshot-reply test pattern in the same codebase.

**Test cases** (from VALIDATION.md + UI-SPEC HTMX contract):
- T-V11-STOP-15: normal path — running run, handler returns 200 + `HX-Refresh: true` + `HX-Trigger` with `Stopped: {job_name}` toast
- T-V11-STOP-16: race case — `StopResult::AlreadyFinalized`, handler returns 200 + `HX-Refresh: true` + NO `HX-Trigger` header
- CSRF mismatch: 403 `CSRF token mismatch`
- Channel closed: 503 `Scheduler is shutting down`

---

### 17. `tests/stop_race.rs` (NEW — T-V11-STOP-04..06, D-15 1000 iterations)

**Analog:** inline tests in `src/scheduler/mod.rs:438-501` (`shutdown_drain_completes_within_grace`, `shutdown_grace_expiry_force_kills`) — the scheduler-loop-on-`JoinSet` test pattern.

**Pattern to copy from `mod.rs:438-471`:**
```rust
#[tokio::test]
async fn shutdown_drain_completes_within_grace() {
    let pool = setup_pool().await;
    let job_id = queries::upsert_job(
        &pool, "fast-job", "0 0 31 2 *", "0 0 31 2 *",
        "command", r#"{"command":"echo done"}"#, "h1", 3600,
    ).await.unwrap();

    let cancel = CancellationToken::new();
    let child_cancel = cancel.child_token();

    let mut join_set: JoinSet<RunResult> = JoinSet::new();
    let job = DbJob { id: job_id, ..make_test_job(job_id, "fast-job", "echo done") };
    join_set.spawn(run::run_job(
        pool.clone(),
        None,
        job,
        "test".to_string(),
        child_cancel,
        test_active_runs(),
    ));
    ...
}
```

**Target — race test with paused time (per D-15):**
```rust
#[tokio::test(start_paused = true)]
async fn stop_race_finalize_vs_stop() {
    for iter in 0..1000 {
        // seed, spawn run_job, concurrently fire StopReason::Operator + let the run finish
        // naturally — assert the finalized status is EXACTLY ONE of {stopped, success},
        // never both, never corrupted. The `active_runs.remove()` at run.rs:276 is the
        // atomic boundary — see RESEARCH.md §Architecture §1 Invariant 3.
    }
}
```

`start_paused = true` is the canonical tokio-test attribute for deterministic time control (replaces the older `tokio::time::pause()` call-from-within-test pattern).

---

### 18. `tests/stop_executors.rs` (NEW — T-V11-STOP-09..11)

**Analog:** `tests/docker_executor.rs` for the docker branch, inline `command.rs:220-250` for the command branch, and inline `script.rs:118+` for the script branch. Each existing test builds an `execute_command` / `execute_script` / `execute_docker` call with a `CancellationToken` and a `log_pipeline::channel`.

**Command analog** (`command.rs:225-236`):
```rust
#[tokio::test]
async fn execute_echo_captures_stdout() {
    let (tx, rx) = log_pipeline::channel(256);
    let cancel = CancellationToken::new();
    let result = execute_command("echo hello", Duration::from_secs(5), cancel, tx).await;
    assert_eq!(result.status, RunStatus::Success);
    ...
}
```

**Target — Stop round-trip for all three executors** (fast command, long-running script, docker sidecar):
```rust
#[tokio::test]
async fn command_stop_returns_stopped_status() {
    let (tx, _rx) = log_pipeline::channel(256);
    let cancel = CancellationToken::new();
    let control = RunControl::new(cancel.clone());

    let handle = tokio::spawn(execute_command(
        "sleep 300",
        Duration::from_secs(600),
        cancel.clone(),
        tx,
        // ... pass control if the signature gains the param
    ));
    tokio::time::sleep(Duration::from_millis(100)).await;
    control.stop(StopReason::Operator);
    let result = handle.await.unwrap();
    assert_eq!(result.status, RunStatus::Stopped);
    assert!(result.error_message.unwrap().contains("operator"));
}
```
Similar pattern for script (uses `execute_script` shared `execute_child` path) and docker (uses bollard + testcontainers per `tests/docker_executor.rs`). The docker test MUST run only under the `integration` feature gate (as existing docker tests do) and is the canonical guard that `docker kill -s KILL` is called on Operator stop before finalize.

---

### 19. `tests/process_group_kill.rs` (NEW — T-V11-STOP-07..08, D-17 regression lock)

**Analog:** inline `command.rs:220-250` tests. These are the closest patterns for "spawn a shell pipeline, observe child PIDs".

**Target — assert the process-group pattern is wired:**
```rust
#[tokio::test]
async fn stop_kills_grandchildren_via_process_group() {
    // Spawn `sh -c 'sleep 300 & wait'` (shell forks a grandchild `sleep`).
    // Stop the run.
    // Observe: the grandchild `sleep` must be gone.
    // If someone refactors to kill_on_drop(true), only the immediate child dies
    // and the grandchild orphans — this test catches that.
}
```
Use `/proc/self/task` or `ps` + the known PID to observe; alternatively, write a sentinel file from the grandchild (`sleep 300 && touch /tmp/sentinel`) and verify it never appears. RESEARCH.md §Architecture §1 (Correction #1) and PITFALLS doc explain the failure mode.

---

### 20. `tests/docker_orphan_guard.rs` (NEW — T-V11-STOP-12..14, D-16 regression lock)

**Analog:** `tests/retention_integration.rs` — the canonical "pre-seed DB rows, call function under test, assert rows unchanged" pattern. Also `src/scheduler/docker_orphan.rs` existing inline tests.

**Target:** three tests pre-seeding `job_runs` rows with `status = 'stopped'`, `status = 'success'`, `status = 'failed'`. Call `mark_run_orphaned(&pool, run_id)`. Assert the row's status, `error_message`, and `end_time` are UNCHANGED. Run against both SQLite (in-memory) and Postgres (via `testcontainers-modules::postgres::Postgres`).

Line numbers to lock: `docker_orphan.rs:120` (SQLite `AND status = 'running'`) and `docker_orphan.rs:131` (Postgres `AND status = 'running'`). A regression that removes either guard MUST make one of these tests fail.

---

## Shared Patterns

### A. CSRF + scheduler-command + HX-Trigger + HX-Refresh (the "state-changing API action" triad)

**Source:** `src/web/handlers/api.rs:26-80` (`run_now`) and `api.rs:199-275` (`reroll`).
**Apply to:** `stop_run` handler.

**The three-step wire** (CSRF validate → send SchedulerCmd → render HTMX response) is already the canonical pattern. Any deviation is a bug. Copy the order verbatim.

### B. Cancellation propagation via `tokio_util::sync::CancellationToken`

**Source:** `src/scheduler/mod.rs:98, 122, 166, 215` — `self.cancel.child_token()` creates a child token before each spawn. The child token already cascades from shutdown.
**Apply to:** `RunControl::new(cancel)` wrapping — the `CancellationToken` stored inside `RunControl` IS the child token, not a new token. Operator stop fires the child; shutdown fires the parent; both propagate to the executor's `cancel.cancelled()` branch.

### C. `WHERE status = 'running'` lifecycle guard on SQL UPDATEs

**Source:** `src/scheduler/docker_orphan.rs:120, 131` (mark_run_orphaned).
**Apply to:** any new SQL UPDATE Phase 10 introduces (none expected — executors own finalize via `run.rs::finalize_run`). This is primarily a regression-lock principle enforced by `tests/docker_orphan_guard.rs`.

### D. `@layer components` + `cd-*` token naming convention

**Source:** `assets/src/app.css:161-217`.
**Apply to:** the new `.cd-badge--stopped` + `.cd-btn-stop` + `.cd-btn-stop--compact` classes. Rules:
- All Cronduit-specific classes use the `cd-` prefix (never `.btn-*` or unprefixed — Tailwind utilities own the unprefixed namespace)
- Badge modifiers use `cd-badge--{status}` with double-dash BEM
- Button classes use `cd-btn-{variant}` with single-dash, modifier suffixes use double-dash (`--compact`)
- All values flow through `var(--cd-*)` tokens, never raw hex (hex lives ONLY in `:root` + light-mode blocks)
- Components live inside `@layer components` so Tailwind utility precedence works

### E. Three-place design-token mirror (dark `:root`, `[data-theme="light"]`, `@media (prefers-color-scheme: light)`)

**Source:** `assets/src/app.css:22-76` + `:79-107` + `:110-136`.
**Apply to:** every new token. Adding a value to only one of the three blocks creates a dark-mode-only or light-mode-only bug.

### F. Handler test harness (in-memory sqlite + AppState fixture + `tower::ServiceExt::oneshot`)

**Source:** `tests/api_run_now.rs:35-108`.
**Apply to:** `tests/stop_handler.rs` verbatim.

### G. Scheduler integration test pattern (`setup_pool` + `JoinSet<RunResult>` + `run::run_job` spawn)

**Source:** `src/scheduler/mod.rs:411-501` inline tests.
**Apply to:** `tests/stop_race.rs`, `tests/stop_executors.rs`.

---

## No Analog Found

Every Phase 10 file has at least a role-match analog in-repo. The project is in a strong "brownfield + incremental feature" state and Phase 10 adds no new architectural patterns. The single table row below exists only to document that we looked:

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| (none) | — | — | All 18 classified files have at least a role-match analog on `main`. |

The `StopResult` enum in `cmd.rs` is novel only in the sense that `StopResult::AlreadyFinalized` has no prior art — but the structural shape (small `Copy` enum returned via `oneshot::Sender`) is modeled on `ReloadStatus` (`cmd.rs:25-29`).

---

## Metadata

**Analog search scope:**
- `src/scheduler/` — all 17 files
- `src/web/` — `mod.rs`, `csrf.rs`, `handlers/{api, sse, run_detail, job_detail}.rs`
- `templates/pages/run_detail.html`, `templates/partials/run_history.html`
- `assets/src/app.css` (source of truth for design tokens)
- `design/DESIGN_SYSTEM.md` (Status Colors canon)
- `Cargo.toml` (dependency declarations)
- `tests/api_run_now.rs`, `tests/scheduler_integration.rs`, `tests/docker_executor.rs`, `tests/retention_integration.rs` (test analog patterns)

**Files scanned:** ~35 source files read (full or partial), plus grep sweeps across `src/` for `active_runs`, `cd-badge`, `cd-btn`, `csrf`, `SchedulerCmd`, `RunStatus`.

**Pattern extraction date:** 2026-04-15.

**Key planner warnings (cross-cutting, from extraction):**
1. `assets/static/app.css` is the **output bundle**, not the source — edit `assets/src/app.css` only.
2. `src/web/csrf.rs:10, 21` is part of the `rand` 0.9 migration — it does NOT need a Stop-specific change, but it lands in the same phase and compiler errors from the rand bump would block Stop work if sequenced after the Stop spike.
3. The RunEntry merge (D-01) is a **single-commit atomic change** across 12 call sites (RESEARCH.md §Architecture §1 call-site inventory). Planner should NOT split this into multiple commits — an intermediate state does not compile.
4. `RunControl` ordering matters: the scheduler loop MUST `.store(reason)` BEFORE `.cancel()`. The executor reads the atomic AFTER `cancel.cancelled()` yields. `RunControl::stop()` already gets this right; the danger is hand-rolled code paths that race.
5. Test helpers in `scheduler/mod.rs:417-420` and `scheduler/run.rs:364-366` both need type-flow updates in lockstep — tests won't compile otherwise.
