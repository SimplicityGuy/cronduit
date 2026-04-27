---
phase: 10
reviewed: 2026-04-15T00:00:00Z
depth: standard
status: clean
files_reviewed: 34
findings:
  critical: 0
  high: 0
  medium: 0
  low: 0
  info: 4
  total: 4
files_reviewed_list:
  - Cargo.toml
  - Cargo.lock
  - THREAT_MODEL.md
  - assets/src/app.css
  - design/DESIGN_SYSTEM.md
  - src/scheduler/cmd.rs
  - src/scheduler/command.rs
  - src/scheduler/control.rs
  - src/scheduler/docker.rs
  - src/scheduler/docker_orphan.rs
  - src/scheduler/mod.rs
  - src/scheduler/random.rs
  - src/scheduler/reload.rs
  - src/scheduler/run.rs
  - src/scheduler/script.rs
  - src/scheduler/sync.rs
  - src/telemetry.rs
  - src/web/csrf.rs
  - src/web/handlers/api.rs
  - src/web/handlers/job_detail.rs
  - src/web/handlers/run_detail.rs
  - src/web/handlers/sse.rs
  - src/web/mod.rs
  - templates/pages/run_detail.html
  - templates/partials/run_history.html
  - tests/docker_container_network.rs
  - tests/docker_executor.rs
  - tests/docker_orphan_guard.rs
  - tests/metrics_stopped.rs
  - tests/process_group_kill.rs
  - tests/scheduler_integration.rs
  - tests/stop_executors.rs
  - tests/stop_handler.rs
  - tests/stop_race.rs
---

# Phase 10: Code Review Report

**Reviewed:** 2026-04-15
**Depth:** standard
**Files Reviewed:** 34
**Status:** clean (4 info-level observations, no actionable bugs/security issues)

## Summary

Phase 10 delivers a clean, well-instrumented "Stop a Running Job" vertical slice layered on top of two hygiene preambles (Cargo 1.0.1 -> 1.1.0 and rand 0.8 -> 0.9). I reviewed all 34 changed files against the areas called out in the review brief and the project's locked constraints (rustls, bollard, croner, askama_web, sqlx, axum 0.8). All load-bearing invariants from the research and pitfalls docs are correctly implemented and regression-locked by a strong test battery:

- **D-17 process-group kill preservation** is byte-for-byte intact in `src/scheduler/command.rs` (`.process_group(0)` at spawn + `libc::kill(-pid_i32, libc::SIGKILL)` in `kill_process_group`). `src/scheduler/script.rs` inherits via `execute_child`. Sentinel-file probes in `tests/process_group_kill.rs` prove shell-pipeline grandchildren and backgrounded subshells are reaped on operator Stop.
- **D-17 docker operator-stop** uses `docker.kill_container(signal="KILL")` with no grace for the Operator branch in `src/scheduler/docker.rs:358-374`, while preserving the v1.0 `stop_container(t=10)` semantics for Shutdown at 376-384. The Pitfall 3 natural-exit race is acknowledged and the tolerant error log is correct.
- **D-10 failure-counter exemption** is correctly implemented at `src/scheduler/run.rs:294` (`if status_str != "success" && status_str != "stopped"`) and regression-locked by `tests/metrics_stopped.rs::stop_increments_runs_total_stopped` which scrapes `/metrics` and asserts `cronduit_run_failures_total{job=...}` is absent-or-zero for stopped runs.
- **D-07 silent-refresh race case** is correct in `src/web/handlers/api.rs::stop_run`: `StopResult::Stopped` emits 200 + `HX-Refresh: true` + `HX-Trigger: showToast`, while both `StopResult::AlreadyFinalized` and the `get_run_by_id -> Ok(None)` pre-lookup branch emit 200 + `HX-Refresh: true` with **no** `HX-Trigger`. All four branches are covered by `tests/stop_handler.rs`.
- **CSRF coverage** on `POST /api/runs/{run_id}/stop` is verbatim-copied from `run_now`: cookie-token + form-token double-submit, constant-time compare, validated before the DB or channel is touched. 403 regression test present.
- **Memory ordering** for `RunControl` uses `Ordering::SeqCst` for both the reason store (happens-before cancel fire) and the reason load (happens-after cancel.cancelled() yields), so executors always observe the correct reason when the cancel arm trips.
- **Lock-scope invariants** in `src/web/handlers/sse.rs::sse_logs` and the scheduler's Stop arm (both in the top-level and in the reload-coalesce drain loop) correctly release `active_runs.read()` before calling `control.stop()` — no awaits held under a read lock.
- **Broadcast refcount drop order** in `src/scheduler/run.rs` is correct: `active_runs.write().await.remove(&run_id)` then `drop(broadcast_tx)` produces `RecvError::Closed` for SSE subscribers.
- **1000-iteration race test** (`tests/stop_race.rs`) uses `tokio::time::pause` + `advance` to deterministically drive the Stop-vs-natural-completion race and passes on every iteration with `final_status in {success, stopped}`.
- **rand 0.9 migration** in `src/scheduler/random.rs`, `src/scheduler/reload.rs`, `src/scheduler/sync.rs`, `src/web/csrf.rs` correctly uses the new `rand::rng()` + `random_range()` + `fill()` API surface.
- **Cargo 1.1.0 version bump** is consistent in `Cargo.toml` (package.version and the rand dep comment updated together).
- **Threat model** is updated to document the Stop button blast radius in the unauthenticated-UI posture (`THREAT_MODEL.md` L113).
- **Design system** adds the `stopped` status token and `.cd-btn-stop` class following existing patterns in `assets/src/app.css` and `design/DESIGN_SYSTEM.md`.

I found **zero** bugs, security issues, or correctness regressions. The four items below are info-level observations — documentation drift and a pre-existing behavior that is out of scope for v1.1 but worth flagging.

## Critical Issues

None.

## High Issues

None.

## Medium Issues

None.

## Low Issues

None.

## Info

### IN-01: Doc drift in `src/scheduler/mod.rs::RunEntry` line-number refs

**File:** `src/scheduler/mod.rs:52-57`
**Issue:** The doc comment for `RunEntry` references "executor inserts ONE clone at run.rs:102, executor drops ITS clone at run.rs:277 after .remove(&run_id) at run.rs:276". After the Phase 10 merge of `active_runs` into a unified `HashMap<i64, RunEntry>`, the actual lines in `src/scheduler/run.rs` are now 114 (insert), 300 (remove), and 301 (drop). The semantic invariant is preserved — the drift is purely in the numeric citation.
**Fix:** Update the doc comment to reference the current line numbers, or replace the numeric pointer with a named anchor such as "at the `active_runs.write().await.insert` call in `run_job`". Preferred: drop the line numbers entirely — they will drift again on the next edit.

### IN-02: `tests/stop_race.rs` module doc overstates the production single-writer mechanism

**File:** `tests/stop_race.rs:10-14`
**Issue:** The module doc claims "The `WHERE status = 'running'` guard on both terminal UPDATE statements is what enforces single-writer semantics — whichever completion path fires first wins, the loser's UPDATE matches zero rows and is a no-op." This is true of the **mock** executor this test defines inline (lines 153-189) which writes its own `WHERE status = 'running'` guards. It is **not** how the production `finalize_run` in `src/db/queries.rs:316` works — production `finalize_run` issues `UPDATE job_runs SET ... WHERE id = ?7` with no status guard. The real single-writer invariant in production is that `tokio::select!` inside `execute_child` (command/script) and `execute_docker` (docker) commits to exactly one branch, so `finalize_run` is called exactly once per run. `docker_orphan::mark_run_orphaned` is the only place the SQL guard appears in production, and that's for a different concern (restart orphan reconciliation, not Stop race).
**Fix:** Clarify the module comment to distinguish "this test's mock executor" from "production's `select!`-exclusivity-based single-writer invariant". Example wording: "In production, `tokio::select!` ensures `finalize_run` is called exactly once. This test's mock executor uses `WHERE status = 'running'` guards instead to model the same exclusivity without pulling in the whole executor pipeline." No code change required — the test correctness is unaffected.

### IN-03: Pre-declared `cronduit_runs_total` label-only series differ in shape from real samples

**File:** `src/telemetry.rs:128-149`
**Issue:** The pre-declaration loop emits `cronduit_runs_total{status="stopped"}` (and the other five terminal statuses) with **only** the `status` label, whereas real samples in `src/scheduler/run.rs:289` emit `cronduit_runs_total{job="<name>",status="<status>"}` with both `job` and `status`. The `metrics-exporter-prometheus` exporter renders distinct label sets as distinct samples, so `/metrics` ends up with two parallel series for every stopped job (one with just `status`, one with `job`+`status`). This is **documented in the comment** (lines 142-147) and **intentional** — alerts meant to be cross-job can reference the label-only series or use `sum by (status) (cronduit_runs_total)`. Not a bug, but operationally surprising for anyone grepping `/metrics` for the first time and seeing the dimension mismatch.
**Fix:** No action required. Optionally add a one-line note to `docs/RUNBOOK.md` or wherever Prometheus scrape config is documented, explaining that alerts referencing `cronduit_runs_total{status="stopped"}` without `job` will match the label-only series from boot.

### IN-04: Shutdown-cancelled runs still increment `cronduit_run_failures_total`

**File:** `src/scheduler/run.rs:294-297`
**Issue:** The failure-metric exemption correctly skips `success` and `stopped`, but a run whose final status is `cancelled` (graceful shutdown — `RunStatus::Shutdown` mapped at line 263) still counts toward `cronduit_run_failures_total` with `reason = unknown` (via `classify_failure_reason`'s default arm). D-10's spirit ("operator stops are not failures") arguably extends to "graceful shutdowns are not failures either" — they're both intentional operator actions, not job problems. This is **pre-existing** behavior from earlier phases (not a phase 10 regression), and D-10 as written is specifically about operator stops. Flagging only for visibility.
**Fix:** Out of scope for v1.1. Candidate for a follow-up hygiene plan: extend the exemption to `"cancelled"` and update the matching test in `tests/metrics_stopped.rs` to assert both paths. Will require a deliberate decision because shutdown during a long-running critical backup may be something operators *do* want to alert on — the current behavior is defensible.

---

_Reviewed: 2026-04-15_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
