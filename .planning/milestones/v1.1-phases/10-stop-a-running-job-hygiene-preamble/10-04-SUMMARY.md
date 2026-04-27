---
phase: 10-stop-a-running-job-hygiene-preamble
plan: 04
subsystem: scheduler+web
tags: [stop, runcontrol, runentry, active-runs, sched-10, refactor]
requires:
  - 10-03 (RunControl + StopReason + executor cancel-arm reason branching)
provides:
  - RunEntry { broadcast_tx, control, job_name } at src/scheduler/mod.rs
  - active_runs is now Arc<RwLock<HashMap<i64, RunEntry>>> across scheduler + web + cli
  - SSE handler subscribes via entry.broadcast_tx.subscribe() with lock-scope invariant preserved
  - job_name stashed at run-start (Pitfall 4 staleness-on-rename semantic accepted)
affects:
  - src/scheduler/mod.rs (RunEntry struct + active_runs field type + spawn signature + test helper)
  - src/scheduler/run.rs (run_job signature + insert RunEntry + test helper)
  - src/web/mod.rs (AppState.active_runs field type)
  - src/web/handlers/sse.rs (subscribe via entry.broadcast_tx)
  - tests/scheduler_integration.rs (test helper signature — Rule 3 blocking fix)
tech-stack:
  added: []
  patterns:
    - "Per-run authoritative record (RunEntry) merging log-broadcast and control-plane lookups"
    - "Atomic single-statement .write().await.insert(...) preserving lock-scope invariant"
    - "Cascading drop of RunEntry releases broadcast_tx + control clones in one step"
key-files:
  created: []
  modified:
    - src/scheduler/mod.rs
    - src/scheduler/run.rs
    - src/web/mod.rs
    - src/web/handlers/sse.rs
    - tests/scheduler_integration.rs
decisions:
  - "D-01 honored: single merged map (no parallel running_handles map exists in src/)"
  - "Pitfall 4 stash: job_name captured at run-start, not looked up via DB on Stop — accept the staleness-on-rename semantic"
  - "Pitfall 2 lock scope: SSE handler keeps the read guard inside { ... } so stream construction never races with run.rs:290 .write().await.remove(&run_id)"
  - "Invariant 1 refcount preserved: drop(broadcast_tx) at run.rs:291 left intact; RunEntry drop cascade-releases the in-map clone after .remove()"
  - "cli/run.rs needed no edit — HashMap::new() infers RunEntry value type from AppState.active_runs"
metrics:
  duration_minutes: 12
  completed: 2026-04-15
  commits: 1
  tasks: 3
  call_sites_updated: 11
  files_in_atomic_commit: 5
  files_in_blocking_fix: 1
  loc_added: 57
  loc_removed: 27
  scheduler_tests_before: 79
  scheduler_tests_after: 79
  lib_tests_before: 168
  lib_tests_after: 168
  delta_tests: 0
---

# Phase 10 Plan 04: active_runs RunEntry Merge Summary

D-01 atomic merge: replace `HashMap<i64, broadcast::Sender<LogLine>>` with `HashMap<i64, RunEntry { broadcast_tx, control, job_name }>` across every call site so the scheduler Stop arm (plan 10-05) can look up RunControl by run_id and the SSE handler reads broadcast_tx through the same lock acquisition pattern.

## Objective

Close the data-structure half of SCHED-10 (the per-run record that stashes RunControl, established locally inside `run_job` by plan 10-03). With this plan landed, plan 10-05 can implement `SchedulerCmd::Stop { run_id }` as a one-line lookup-and-call (`active_runs.read().await.get(&run_id).map(|e| e.control.stop(StopReason::Operator))`) and plan 10-07's web handler can stash the toast text by reading `entry.job_name` at the same lookup.

## What Was Built

This is the largest single-commit change in Phase 10 by file count (5 files) but it is deliberately atomic. Intermediate states between call sites do not compile, so the merge had to land as one commit.

### Single atomic commit `f17e4c6`

**1. `src/scheduler/mod.rs`** — RunEntry struct + active_runs field type migration

- New `RunEntry` struct alongside `RunResult`, with three fields:
  - `broadcast_tx: tokio::sync::broadcast::Sender<log_pipeline::LogLine>` (D-01)
  - `control: crate::scheduler::control::RunControl` (D-01)
  - `job_name: String` (Pitfall 4 stash recommendation)
- `#[derive(Clone)]` so the entry can be cloned in/out of the map cheaply (Sender is Clone, RunControl is Clone via Arc<AtomicU8> + Arc<CancellationToken>, String is Clone).
- Module-level doc comment documents Invariant 1 (broadcast_tx refcount arithmetic preserved).
- `SchedulerLoop::active_runs` field type migrated from `HashMap<i64, broadcast::Sender<LogLine>>` to `HashMap<i64, RunEntry>`.
- `spawn()` constructor signature parameter migrated to the new map type.
- `test_active_runs()` test helper return type migrated; the body `Arc::new(RwLock::new(HashMap::new()))` is unchanged because `HashMap::new()` infers from context.
- Removed the now-unused `use crate::scheduler::log_pipeline::LogLine;` top-level import (no other use in the file after the migration; `LogLine` is referenced inside `RunEntry` via fully-qualified `log_pipeline::LogLine`).
- The 4 spawn sites at L107, L131, L175, L224 (`self.active_runs.clone()`) needed no edit at the call site — type flows from the field declaration.
- The 3 mod.rs test sites (`shutdown_drain_completes_within_grace`, `shutdown_grace_expiry_force_kills`, `shutdown_summary_fields`) pass `test_active_runs()` directly to `run_job`, never inserting into the map themselves, so they needed no insertion-site updates.

**2. `src/scheduler/run.rs`** — run_job insert RunEntry, remove unchanged

- `run_job` signature parameter migrated to `Arc<RwLock<HashMap<i64, crate::scheduler::RunEntry>>>`.
- INSERT POINT (was L102): the bare `.insert(run_id, broadcast_tx.clone())` becomes:
  ```rust
  active_runs.write().await.insert(
      run_id,
      crate::scheduler::RunEntry {
          broadcast_tx: broadcast_tx.clone(),
          control: run_control.clone(),
          job_name: job.name.clone(),
      },
  );
  ```
  This is a single-statement write under the lock — Pitfall 2 lock-scope invariant preserved.
- The spike's local `let run_control = ...` (added by plan 10-03) was already constructed before the insert; this plan reorders it to live above the insert and clones it into the map. The three executor dispatches that thread `&run_control` are unchanged.
- REMOVE POINT (run.rs:290): **structurally unchanged**. `active_runs.write().await.remove(&run_id)` now drops a `RunEntry` whose Drop cascades to the contained broadcast_tx and control clones. Combined with the local `drop(broadcast_tx)` at run.rs:291 (Invariant 1 — preserved verbatim), the sender refcount drops to zero and SSE subscribers receive `RecvError::Closed`.
- `test_active_runs()` test helper return type migrated. The 4 run.rs unit tests (`run_job_command_success`, `run_job_script_success`, `run_job_timeout_preserves_partial_logs`, `concurrent_runs_create_separate_rows`) pass `test_active_runs()` directly to `run_job` and don't insert into the map themselves — no test-site insertion edits needed.

**3. `src/web/mod.rs`** — AppState.active_runs field type

- `AppState.active_runs` field type migrated to `std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<i64, crate::scheduler::RunEntry>>>`. Doc comment updated to mention `entry.broadcast_tx` for SSE and `entry.control` for the future plan 10-07 stop_run lookup.
- Router block unchanged — plan 10-07 will add the `POST /api/runs/{run_id}/stop` route.

**4. `src/web/handlers/sse.rs`** — subscribe via entry.broadcast_tx

- `sse_logs` handler subscribe pattern changed from `.map(|tx| tx.subscribe())` to `.map(|entry| entry.broadcast_tx.subscribe())`. (Rustfmt rewrapped this onto three lines as a chained `.get(...).map(...)` expression.)
- **Pitfall 2 lock-scope invariant preserved**: the entire subscribe block is still wrapped in `{ let active = state.active_runs.read().await; ... }` so the read guard drops BEFORE the `stream! { ... }` macro begins yielding — this prevents a deadlock against run.rs:290's `.write().await.remove(&run_id)`.
- No other changes to sse.rs (helpers, format functions, tests untouched).

**5. `tests/scheduler_integration.rs`** — Rule 3 blocking fix (test helper signature)

- The integration-test file has its own `test_active_runs()` helper at L24. Without updating it, all 6 integration tests fail to compile against the new `run_job` signature (E0308 mismatched types).
- Migrated the helper's return type to `Arc<RwLock<HashMap<i64, RunEntry>>>` and updated the imports: removed `cronduit::scheduler::log_pipeline::LogLine` (no longer used), added `cronduit::scheduler::RunEntry`.
- All 6 integration tests pass after the change; no test logic touched.

**`cli/run.rs` did NOT need editing.** The active_runs initialization at L133-134 uses `HashMap::new()` with no explicit type annotation, so the value type is inferred from the `AppState.active_runs` field declaration. The `AppState { active_runs, .. }` construction at L146 and the `scheduler::spawn(..., active_runs, ...)` call at L229 type-check transparently against the new `RunEntry` value type. This was a pleasant surprise — the call-site inventory in 10-RESEARCH.md flagged cli/run.rs as needing edits, but type inference made it a no-op.

## Call-Site Count

10-RESEARCH.md §Architecture §1 enumerated 11–12 call sites across 6 files. Actual measured count after the merge:

| # | File | Site | Edit kind |
|---|------|------|-----------|
| 1 | src/scheduler/mod.rs | RunEntry struct definition (L40-63) | NEW |
| 2 | src/scheduler/mod.rs | active_runs field decl (L65-67) | TYPE MIGRATE |
| 3 | src/scheduler/mod.rs | spawn() parameter (L388) | TYPE MIGRATE |
| 4 | src/scheduler/mod.rs | test_active_runs() return (L416) | TYPE MIGRATE |
| 5 | src/scheduler/mod.rs | LogLine import removal | DELETE (cleanup) |
| 6 | src/scheduler/run.rs | run_job parameter (L71) | TYPE MIGRATE |
| 7 | src/scheduler/run.rs | INSERT POINT (L113-121) | STRUCTURAL |
| 8 | src/scheduler/run.rs | REMOVE POINT (L290) | UNCHANGED (verified) |
| 9 | src/scheduler/run.rs | test_active_runs() return (L378) | TYPE MIGRATE |
| 10 | src/web/mod.rs | AppState.active_runs field decl | TYPE MIGRATE |
| 11 | src/web/handlers/sse.rs | subscribe map closure | STRUCTURAL |
| (extra) | tests/scheduler_integration.rs | test_active_runs() helper | TYPE MIGRATE (Rule 3 blocking fix) |
| (no-op) | src/cli/run.rs | active_runs init + AppState assign + spawn call | INFERENCE — no edit needed |

11 source-file call sites updated + 1 integration-test helper (Rule 3) — within the 11–12 prediction.

## Pitfall 2 Lock-Scope Invariant — Preserved

The SSE handler block is still wrapped in `{ ... }`:

```rust
let maybe_rx = {
    let active = state.active_runs.read().await;
    active
        .get(&run_id)
        .map(|entry| entry.broadcast_tx.subscribe())
};
```

The `active` read guard goes out of scope at the closing `}`, BEFORE `stream! { ... }` begins yielding. This means:

- `state.active_runs.read().await` is held for exactly the time it takes to `.get(&run_id)` and clone a sender via `.subscribe()` — microseconds.
- Concurrent `.write().await.remove(&run_id)` from run.rs:290 cannot deadlock against the SSE handler.
- Plan 10-10's stress test (100 concurrent SSE subscribers + Stop commands, no hang over 10s) will lock this in.

Confirmed by `grep`:

```
$ grep -c 'let maybe_rx = {' src/web/handlers/sse.rs
1
$ grep -c 'state.active_runs.read().await' src/web/handlers/sse.rs
1
$ grep -c 'entry.broadcast_tx.subscribe' src/web/handlers/sse.rs
1
$ grep -c '|tx| tx.subscribe' src/web/handlers/sse.rs
0   # old pattern gone
```

## Invariant 1 Refcount — Preserved

The broadcast_tx refcount arithmetic that triggers SSE `RecvError::Closed`:

1. `let (broadcast_tx, _rx) = broadcast::channel(256);` — 1 ref (local), 1 receiver.
2. `RunEntry { broadcast_tx: broadcast_tx.clone(), ... }` inserted into map — 2 refs.
3. `let writer_handle = tokio::spawn(log_writer_task(... broadcast_tx.clone() ...))` — 3 refs.
4. After log writer exits, drops its clone — 2 refs.
5. `active_runs.write().await.remove(&run_id)` drops the RunEntry which cascade-drops its broadcast_tx clone — 1 ref.
6. `drop(broadcast_tx)` — 0 refs. All `subscribe()` receivers get `RecvError::Closed`. SSE stream emits `run_complete` and closes.

The `drop(broadcast_tx)` line at run.rs:291 was preserved verbatim — confirmed by `grep -c 'drop(broadcast_tx)' src/scheduler/run.rs` returning 1.

## Zero parallel maps

`grep -r 'running_handles' src/` returns one match — the doc comment in `src/scheduler/mod.rs:47` referencing D-01's rejected design (`merged map from active_runs + running_handles separation`). No actual `running_handles` map exists in any source file. The merged map is the single source of truth.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] tests/scheduler_integration.rs has its own test_active_runs() helper**

- **Found during:** Task 3 (cargo clippy --all-targets after Task 3 edits)
- **Issue:** The plan's `files_modified` listed only 5 files (mod.rs, run.rs, web/mod.rs, sse.rs, cli/run.rs). After the Task 3 edits, `cargo clippy --all-targets` failed with 7 × E0308 errors in `tests/scheduler_integration.rs` because that file has its own copy of `test_active_runs()` returning the old map type and 6 tests pass it to `run_job`.
- **Fix:** Updated the integration-test helper's return type to `Arc<RwLock<HashMap<i64, RunEntry>>>` and swapped the `LogLine` import for `RunEntry`. No test logic touched.
- **Files modified:** `tests/scheduler_integration.rs`
- **Commit:** `f17e4c6` (atomic with the rest of the merge — splitting would leave the tree broken)

**2. [Rule 1 - Cleanup] Removed unused `use crate::scheduler::log_pipeline::LogLine;` import in src/scheduler/mod.rs**

- **Found during:** First `cargo build` after Task 1 edits
- **Issue:** The bare `Sender<LogLine>` reference in mod.rs's active_runs field type was the only top-level use of `LogLine`. After migrating the field to `HashMap<i64, RunEntry>` (which references `log_pipeline::LogLine` via a fully-qualified path inside the struct), the top-level import becomes unused and rustc emits `warning: unused import` which would block `cargo clippy -- -D warnings`.
- **Fix:** Removed the `use crate::scheduler::log_pipeline::LogLine;` line. RunEntry's `broadcast_tx` field uses the fully-qualified `tokio::sync::broadcast::Sender<log_pipeline::LogLine>` so it still resolves through the existing `pub mod log_pipeline;` declaration.
- **Files modified:** `src/scheduler/mod.rs`
- **Commit:** `f17e4c6`

### Pre-existing issues NOT fixed (deferred)

**1. [Out-of-scope] Pre-existing fmt drift in src/scheduler/command.rs**

- **Discovered during:** Task 3 verification (`cargo fmt --check`)
- **State at base commit f821cd9:** `cargo fmt -- --check` reports 2 violations in `src/scheduler/command.rs` at L348 and L385. Two `execute_command(...)` test calls were left broken across multiple lines and rustfmt wants them collapsed onto a single line.
- **Verified pre-existing:** `git stash; cargo fmt -- --check src/scheduler/command.rs` (before applying any of plan 10-04's edits) reports the same diff. The drift was introduced by plan 10-03's spike commits which did not run `cargo fmt` before committing.
- **Why NOT fixed:** Per executor scope-boundary rules ("only auto-fix issues directly caused by the current task's changes; pre-existing warnings/lint failures in unrelated files are out of scope"), this fmt drift is logged to `.planning/phases/10-stop-a-running-job-hygiene-preamble/deferred-items.md` and left for a follow-up plan to fix in a single `cargo fmt` commit.
- **Impact:** Will block `cargo fmt --check` on CI for any commit until fixed. Plan 10-05 or 10-10 can resolve in 18 lines.
- **Logged to:** `.planning/phases/10-stop-a-running-job-hygiene-preamble/deferred-items.md`

## Verification Results

### Test Suite

| Tier | Before | After | Delta |
|------|--------|-------|-------|
| `cargo test -p cronduit --lib` | 168 passed, 0 failed | 168 passed, 0 failed | 0 (no new tests; none lost) |
| `cargo test -p cronduit scheduler::` | 79 passed | 79 passed | 0 |
| `cargo test -p cronduit web::` | 20 passed | 20 passed | 0 |
| `cargo test -p cronduit --test scheduler_integration` | 6 passed | 6 passed | 0 |

Wave 4 baseline test count maintained — zero regressions.

### Quality Gates

- `cargo build -p cronduit` — clean (no warnings)
- `cargo clippy -p cronduit --all-targets -- -D warnings` — clean
- `cargo tree -i openssl-sys` — empty (`error: package ID specification 'openssl-sys' did not match any packages`), rustls-only confirmed
- `cargo fmt -p cronduit -- --check` for plan-touched files (mod.rs, run.rs, web/mod.rs, sse.rs, scheduler_integration.rs) — clean

### Acceptance Criteria

| Task | Criterion | Status |
|------|-----------|--------|
| 1 | `grep -c 'pub struct RunEntry' src/scheduler/mod.rs == 1` | PASS (1) |
| 1 | `grep -c 'pub broadcast_tx:' src/scheduler/mod.rs >= 1` | PASS (1) |
| 1 | `grep -c 'pub control:' src/scheduler/mod.rs >= 1` | PASS (1) |
| 1 | `grep -c 'pub job_name: String' src/scheduler/mod.rs >= 1` | PASS (1) |
| 1 | `grep -c 'HashMap<i64, RunEntry>' src/scheduler/mod.rs >= 2` | PASS (3 — field + spawn + test helper) |
| 2 | `grep -c 'HashMap<i64, crate::scheduler::RunEntry>' src/scheduler/run.rs >= 2` | PASS (2 — run_job + test helper) |
| 2 | `grep -c 'RunEntry {' src/scheduler/run.rs >= 1` | PASS (1) |
| 2 | `grep -c 'broadcast_tx: broadcast_tx.clone()' src/scheduler/run.rs >= 1` | PASS (1) |
| 2 | `grep -c 'job_name: job.name.clone()' src/scheduler/run.rs >= 1` | PASS (1) |
| 2 | `grep -c 'drop(broadcast_tx)' src/scheduler/run.rs >= 1` | PASS (1) — Invariant 1 preserved |
| 2 | `grep -c 'active_runs.write().await.remove(&run_id)' src/scheduler/run.rs >= 1` | PASS (1) — remove point unchanged |
| 3 | `grep -c 'HashMap<i64, crate::scheduler::RunEntry>' src/web/mod.rs >= 1` | PASS (1) |
| 3 | `grep -c 'entry.broadcast_tx.subscribe' src/web/handlers/sse.rs == 1` | PASS (1) |
| 3 | `grep -c '\\|tx\\| tx.subscribe' src/web/handlers/sse.rs == 0` | PASS (0) — old pattern gone |
| 3 | `grep -c 'state.active_runs.read().await' src/web/handlers/sse.rs >= 1` | PASS (1) |
| 3 | `grep -c 'let maybe_rx = {' src/web/handlers/sse.rs == 1` | PASS (1) — wrapped block preserved |
| 3 | `cargo build -p cronduit` exits 0 | PASS |
| 3 | `cargo test -p cronduit --lib` zero regressions | PASS (168 → 168) |
| 3 | `cargo clippy -p cronduit --all-targets -- -D warnings` exits 0 | PASS |
| 3 | `cargo tree -i openssl-sys` empty | PASS |
| 3 | SSE read-lock block still wrapped in `{ ... }` | PASS (verified by inspection + grep) |

All acceptance criteria PASS.

## What This Unblocks

- **Plan 10-05 (SchedulerCmd::Stop scheduler arm):** Can now look up `RunControl` by `run_id` via `active_runs.read().await.get(&run_id).map(|e| e.control.stop(StopReason::Operator))` — a one-line lookup that did not exist before this plan.
- **Plan 10-07 (web stop_run handler):** Can read `entry.job_name` from the same map lookup that fires the stop, eliminating a separate DB query for the toast text.
- **Plan 10-10 (race tests T-V11-STOP-04..06):** Can exercise the merged-map lifecycle directly — `join_next()` removes the RunEntry atomically, so the race window between "Stop arrives" and "executor finalizes naturally" is observable through the single map's state.

## Known Stubs

None — this plan is a pure refactor with no UI surface, no new endpoints, no new data fields visible to the operator. The added `RunEntry::job_name` field is wired into the insert site (`job.name.clone()`) and will be consumed by plan 10-07's stop_run handler.

## Self-Check: PASSED

- `src/scheduler/mod.rs` — FOUND (RunEntry struct present, active_runs field type migrated, spawn signature migrated, test helper migrated, LogLine import removed)
- `src/scheduler/run.rs` — FOUND (run_job signature migrated, INSERT POINT wraps in RunEntry with all 3 fields, REMOVE POINT unchanged, drop(broadcast_tx) preserved, test helper migrated)
- `src/web/mod.rs` — FOUND (AppState.active_runs field type migrated)
- `src/web/handlers/sse.rs` — FOUND (entry.broadcast_tx.subscribe pattern, lock-scope block preserved)
- `tests/scheduler_integration.rs` — FOUND (test_active_runs helper migrated)
- Commit `f17e4c6` — FOUND in `git log --oneline` (single atomic refactor)
- `grep -r 'running_handles' src/` — returns 1 match (doc-comment reference only); no parallel map exists
- `cargo build -p cronduit` — clean
- `cargo test -p cronduit --lib` — 168 / 168 passing (no regressions vs Wave 3 baseline)
- `cargo clippy -p cronduit --all-targets -- -D warnings` — clean
- `cargo tree -i openssl-sys` — empty (rustls-only)
