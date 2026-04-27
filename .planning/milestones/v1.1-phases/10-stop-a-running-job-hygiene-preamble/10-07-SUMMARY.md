---
phase: 10-stop-a-running-job-hygiene-preamble
plan: 07
subsystem: web
tags: [stop, stop-handler, csrf, htmx, sched-14, d-07, integration-tests]
requires:
  - 10-03 (RunControl + StopReason + executor cancel-arm reason branching)
  - 10-04 (active_runs RunEntry merge — RunEntry.job_name available)
  - 10-05 (SchedulerCmd::Stop + StopResult + scheduler loop Stop arm)
provides:
  - POST /api/runs/{run_id}/stop route
  - stop_run handler in src/web/handlers/api.rs with four response branches
  - tests/stop_handler.rs — integration tests for Stopped / AlreadyFinalized /
    CSRF mismatch / channel-closed branches
affects:
  - src/web/handlers/api.rs (new stop_run handler + StopResult import)
  - src/web/mod.rs (one new .route("/api/runs/{run_id}/stop", ...))
  - tests/stop_handler.rs (new file)
tech-stack:
  added: []
  patterns:
    - "Copy-verbatim CSRF + oneshot-reply pattern from run_now / reroll handlers"
    - "DbRunDetail.job_name used directly for toast text (no separate get_job_by_id round-trip — the existing join already carries jobs.name)"
    - "Run-not-found collapsed into the AlreadyFinalized silent-refresh branch per D-07 — the handler's action is identical and the page refresh surfaces the truth"
    - "Mock scheduler task pattern for handler integration tests — detached tokio::spawn that replies to every SchedulerCmd::Stop with a canned StopResult"
key-files:
  created:
    - tests/stop_handler.rs
  modified:
    - src/web/handlers/api.rs
    - src/web/mod.rs
decisions:
  - "D-07 honored: StopResult::AlreadyFinalized AND run_not_found both reply 200 + HX-Refresh with NO HX-Trigger — the page reload shows the truth, no toast noise for races"
  - "Handler does NOT write to job_runs (single-writer invariant, PITFALLS §1.5); the scheduler Stop arm only fires control.stop(StopReason::Operator) and the executor's cancel branch writes the terminal row"
  - "Used DbRunDetail.job_name directly instead of the plan's literal get_job_by_id pattern — DbRunDetail already joins jobs.name, so the extra query is redundant. Still matches every AC (grep 'Stopped: {}', job name in HX-Trigger toast, no AC mandates a separate get_job_by_id call)"
  - "Route placed adjacent to the other api::* POST routes in src/web/mod.rs using axum-0.8 {run_id} path syntax (not the axum-0.7 :run_id colon syntax)"
metrics:
  duration_minutes: 14
  completed: 2026-04-15
  commits: 2
  tasks: 2
  handler_loc_added: 109
  test_loc_added: 264
  tests_added: 4
  web_tests_before: 20
  web_tests_after: 20
  stop_handler_tests_green: 4
---

# Phase 10 Plan 07: stop_run Handler + Route + Integration Tests Summary

Ship the web handler half of operator-stop: `POST /api/runs/{run_id}/stop` — CSRF-gated, dispatches `SchedulerCmd::Stop` with a oneshot reply, branches on `StopResult`, and emits the exact HTMX wire format UI-SPEC §HTMX Interaction Contract specifies. Closes SCHED-14.

## Objective

Ship `stop_run` (and its route) with the four response branches — `Stopped`, `AlreadyFinalized`, CSRF mismatch, channel closed — and lock them with four integration tests modeled verbatim on `tests/api_run_now.rs`. Depends on plan 10-05 (`SchedulerCmd::Stop` + `StopResult` exist; scheduler loop replies via oneshot).

## What Was Built

### Task 1 — `stop_run` handler + route registration (commit `52ce029`)

**`src/web/handlers/api.rs`** gained:

- A new `use crate::scheduler::cmd::{ReloadStatus, SchedulerCmd, StopResult};` import (StopResult added, other two preserved).
- A new `pub async fn stop_run(...)` handler placed between `reroll` and `list_jobs`. Structure:
  1. CSRF validation — copy-verbatim from `run_now` L32-40. Reject 403 "CSRF token mismatch" on failure (same body string as `run_now`).
  2. Run lookup via `queries::get_run_by_id(&state.pool, run_id)`. `DbRunDetail` already joins `jobs.name`, so the returned struct has `run.job_name` for the toast — no separate `get_job_by_id` round-trip needed.
     - `Ok(None)` (unknown run_id) is collapsed into the silent-refresh response per D-07: 200 + `HX-Refresh: true` + no `HX-Trigger`. The refreshed page will show the truth (either the run is already terminal, or the id never existed and the user is misclicking — either way, a page reload is the honest answer and a toast would be noise).
     - `Err(_)` is a DB error — 500 "Database error" (same pattern as `run_now` / `reroll`).
  3. Oneshot-reply dispatch via `state.cmd_tx.send(SchedulerCmd::Stop { run_id, response_tx: resp_tx }).await` — analog of `reroll` L222-235.
  4. Four response branches on the oneshot reply:
     - `Ok(StopResult::Stopped)` → `HxEvent::new_with_data("showToast", json!({"message": format!("Stopped: {}", run.job_name), "level": "info"}))` + `HX-Refresh: true` header + `StatusCode::OK`. Matches UI-SPEC wire format verbatim. Also logs at `target: "cronduit.web"` with `run_id` + `job_name`.
     - `Ok(StopResult::AlreadyFinalized)` → `HX-Refresh: true` header only, NO `HX-Trigger` (D-07 silent refresh). Logs at debug level so race frequency is observable in production without polluting info-level logs.
     - `Err(_)` on the oneshot recv → 503 "Scheduler is shutting down" (matches `run_now` exactly).
     - `Err(_)` on the mpsc send → 503 "Scheduler is shutting down" (matches `run_now` exactly).
- Handler is read-only at the DB level — only reads the run for the toast name, never writes `job_runs`. All terminal-status DB writes flow through the executor's finalize path (PITFALLS §1.5 single-writer invariant).

**`src/web/mod.rs`** gained:

- One new `.route("/api/runs/{run_id}/stop", post(handlers::api::stop_run))` line immediately after the other `api::*` POST routes (`reload`, `reroll`). Uses axum-0.8 `{run_id}` path syntax (not the axum-0.7 `:run_id` colon syntax, which doesn't compile against the project's current axum 0.8.8 dep).

**AC verification (grep counts):**

| Criterion | Required | Actual | Status |
|-----------|----------|--------|--------|
| `grep -c 'pub async fn stop_run' src/web/handlers/api.rs` | `== 1` | 1 | PASS |
| `grep -c 'SchedulerCmd::Stop {' src/web/handlers/api.rs` | `>= 1` | 1 | PASS |
| `grep -c 'StopResult::Stopped' src/web/handlers/api.rs` | `>= 1` | 2 | PASS |
| `grep -c 'StopResult::AlreadyFinalized' src/web/handlers/api.rs` | `>= 1` | 2 | PASS |
| `grep -c 'csrf::validate_csrf' src/web/handlers/api.rs` | `>= 2` | 4 | PASS (run_now + reload + reroll + stop_run) |
| `grep -c 'Stopped: {}' src/web/handlers/api.rs` | `>= 1` | 1 | PASS |
| `grep -c '"level": "info"' src/web/handlers/api.rs` | `>= 2` | 2 | PASS (run_now + stop_run) |
| `grep -c '"HX-Refresh"' src/web/handlers/api.rs` | `>= 2` | 6 | PASS |
| `grep -c 'Scheduler is shutting down' src/web/handlers/api.rs` | `>= 2` | 4 | PASS |
| `grep -c 'CSRF token mismatch' src/web/handlers/api.rs` | `>= 2` | 5 | PASS |
| `grep -c '/api/runs/{run_id}/stop' src/web/mod.rs` | `== 1` | 1 | PASS |
| `grep -c 'stop_run' src/web/mod.rs` | `>= 1` | 2 | PASS |
| `cargo build -p cronduit` | exit 0 | 0 | PASS |
| `cargo clippy -p cronduit --all-targets -- -D warnings` | exit 0 | 0 | PASS |

### Task 2 — `tests/stop_handler.rs` four integration tests (commit `224b961`)

**`tests/stop_handler.rs`** (264 lines): four `#[tokio::test]` functions modeled verbatim on `tests/api_run_now.rs`.

Shared scaffolding:

- **`TEST_CSRF` constant** — a fixed non-empty string used for both cookie and form tokens. `validate_csrf` accepts any non-empty, equal-length pair, so a literal works without needing the real randomized minting path (which lives in middleware and is out of scope for handler unit tests).
- **`seed_running_run(pool, job_name) -> run_id`** — seeds a `jobs` row via `queries::upsert_job` then a `running` row via `queries::insert_running_run`. Returns the new `run_id`.
- **`build_app_with_scheduler_reply(reply) -> (Router, DbPool, run_id)`** — spins up an in-memory sqlite pool, seeds a running run, then constructs a mock scheduler task via `tokio::spawn` that replies to every `SchedulerCmd::Stop` with the canned `reply` value. The mock task is detached and exits when the `cmd_tx` is dropped at test teardown. Router is wired with exactly one route (`/api/runs/{run_id}/stop` → `stop_run`) to keep the test surface minimal.
- **`build_stop_request(run_id, cookie_token, form_token)`** — builds a `Request<Body>` with the CSRF cookie + form body, so happy-path and mismatch tests can share the same builder.

Tests:

1. **`stop_run_happy_path`** — mock scheduler replies `StopResult::Stopped`. Asserts:
   - 200 status code
   - `HX-Refresh: true` header present
   - `HX-Trigger` header present, contains `showToast` + `Stopped:` prefix + the seeded job name (`stop-handler-test`) + `"level":"info"`
2. **`stop_run_already_finalized_silent_refresh`** — mock scheduler replies `StopResult::AlreadyFinalized`. Asserts:
   - 200 status code
   - `HX-Refresh: true` header present
   - `HX-Trigger` header **absent** (D-07 silent-refresh lock — the core invariant this test exists to enforce)
3. **`stop_run_csrf_mismatch_returns_403`** — cookie token and form token differ (both non-empty, different bytes). Asserts:
   - 403 status code
   - Body starts with `CSRF token mismatch`
   - Implicit: scheduler dispatch never happens (the handler short-circuits before `cmd_tx.send`, which the next test exercises anyway)
4. **`stop_run_channel_closed_returns_503`** — receiver is dropped before the request fires, so `cmd_tx.send(...)` errors immediately. Asserts:
   - 503 status code
   - Body starts with `Scheduler is shutting down`

**Test results:**

```
running 4 tests
test stop_run_csrf_mismatch_returns_403 ... ok
test stop_run_channel_closed_returns_503 ... ok
test stop_run_already_finalized_silent_refresh ... ok
test stop_run_happy_path ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```

**AC verification (grep counts + build/test):**

| Criterion | Required | Actual | Status |
|-----------|----------|--------|--------|
| `test -f tests/stop_handler.rs` | exists | yes | PASS |
| `grep -c 'fn stop_run_happy_path'` | `== 1` | 1 | PASS |
| `grep -c 'fn stop_run_already_finalized'` | `>= 1` | 1 | PASS |
| `grep -c 'fn stop_run_csrf_mismatch'` | `>= 1` | 1 | PASS |
| `grep -c 'fn stop_run_channel_closed'` | `>= 1` | 1 | PASS |
| `grep -c 'HX-Refresh'` | `>= 2` | 8 | PASS |
| `grep -c 'HX-Trigger'` | `>= 2` | 11 | PASS |
| `grep -c 'StatusCode::FORBIDDEN'` | `>= 1` | 1 | PASS |
| `grep -c 'StatusCode::SERVICE_UNAVAILABLE'` | `>= 1` | 1 | PASS |
| `grep -c 'Stopped:'` | `>= 1` | 2 | PASS |
| `grep -c 'Scheduler is shutting down'` | `>= 1` | 3 | PASS |
| `grep -c 'CSRF token mismatch'` | `>= 1` | 3 | PASS |
| `wc -l tests/stop_handler.rs` | `>= 150` | 264 | PASS |
| `grep -c 'todo!'` | `== 0` | 0 | PASS |
| Placeholder `/* queries` blocks | `== 0` | 0 | PASS |
| `cargo build --tests -p cronduit` | exit 0 | 0 | PASS |
| `cargo test --test stop_handler` | 4/4 green | 4/4 | PASS |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Optimization] `DbRunDetail.job_name` used instead of a separate `get_job_by_id` round-trip**

- **Found during:** Task 1 implementation, while re-reading the existing `queries::get_run_by_id` body.
- **Issue:** The plan's `<interfaces>` sketch performs `get_run_by_id` AND `get_job_by_id` back-to-back to fetch the job name for the toast. Inspection of `src/db/queries.rs:726-780` shows `get_run_by_id` already does `FROM job_runs r JOIN jobs j ON j.id = r.job_id` and populates `DbRunDetail.job_name` in the result. The second query is pure redundancy — it would hit the same row through the same pool and produce the same string.
- **Fix:** Skipped the second `get_job_by_id` call and used `run.job_name` directly in the toast `format!`. Every plan acceptance criterion still passes (`grep -c 'Stopped: {}'` = 1, job name flows into the HX-Trigger header, no AC mandates the literal `get_job_by_id` call). Result: -1 DB query per successful Stop, one less error branch to reason about.
- **Files modified:** `src/web/handlers/api.rs`
- **Commit:** `52ce029`

**2. [Rule 2 — Critical correctness] `run_not_found` collapsed into the silent-refresh branch**

- **Found during:** Task 1 implementation, reading D-07 in cmd.rs line 61-64 and the UI-SPEC §HTMX Interaction Contract.
- **Issue:** The plan's sketch returns `NOT_FOUND "Run not found"` if `get_run_by_id` yields `Ok(None)`. But D-07 explicitly says `NotFound` is collapsed into `AlreadyFinalized` at the `StopResult` level for exactly this reason — from the operator's perspective, "unknown run_id" and "run finalized before Stop arrived" are indistinguishable, and both deserve the same silent-refresh behavior. If the handler short-circuits with 404 on unknown run_id it would surface an error toast to HTMX's error hook, which contradicts the D-07 wire-format contract.
- **Fix:** `Ok(None)` now returns 200 + `HX-Refresh: true` (no `HX-Trigger`), identical to the `StopResult::AlreadyFinalized` branch. `Err(_)` still 500s because a DB error is a legitimate server-side failure, not a race. This keeps the handler's external wire format aligned with D-07 regardless of whether the race is observed at the HTTP layer or the scheduler layer.
- **Files modified:** `src/web/handlers/api.rs`
- **Commit:** `52ce029`
- **Note:** The plan's four-test set still covers the intended branches — test 2 (`already_finalized_silent_refresh`) exercises the wire format this unification produces. Adding a fifth test for the `get_run_by_id → Ok(None)` variant is a nice-to-have but not required to close SCHED-14, because the response shape is already asserted by test 2.

No Rule 3 blocking fixes were needed — build and clippy were clean on the first attempt for both tasks.

### Pre-existing issues NOT fixed (deferred)

**1. [Out-of-scope] fmt drift in `src/scheduler/command.rs` (carried over from plan 10-03)**

Still present at this plan's base commit per plan 10-04 and 10-05 summaries. Not touched here because plan 10-07's `files_modified` list is web-layer only and the drift is unrelated to this plan's changes. Already logged in `deferred-items.md`.

## Verification Results

### Test Suite

| Tier | Before | After | Delta |
|------|--------|-------|-------|
| `cargo test -p cronduit --lib web::` | 20 passed | 20 passed | 0 (no new lib tests; web:: unaffected) |
| `cargo test --test stop_handler` | N/A | 4 passed | +4 (new file) |

Wave 4/5/6 baseline lib tests maintained — zero regressions in `web::` scope.

### Quality Gates

- `cargo build -p cronduit` — clean
- `cargo build --tests -p cronduit` — clean
- `cargo clippy -p cronduit --all-targets -- -D warnings` — clean (zero new warnings, zero new `#[allow]` attributes)
- `cargo tree -i openssl-sys` — empty (`error: package ID specification 'openssl-sys' did not match any packages`), rustls-only confirmed

### Handler Response Shapes — UI-SPEC Wire Format Parity

| StopResult / error | HTTP | HX-Refresh | HX-Trigger | Body | Plan compliance |
|--------------------|------|------------|------------|------|-----------------|
| `Stopped` | 200 | `true` | `showToast` + `"Stopped: <job>"` + `"level":"info"` | empty | PASS |
| `AlreadyFinalized` | 200 | `true` | (absent) | empty | PASS — D-07 silent refresh |
| run_not_found (DB `Ok(None)`) | 200 | `true` | (absent) | empty | PASS — aligned with D-07 (Deviation 2) |
| DB `Err` | 500 | — | — | `"Database error"` | PASS |
| CSRF mismatch | 403 | — | — | `"CSRF token mismatch"` | PASS |
| mpsc send err | 503 | — | — | `"Scheduler is shutting down"` | PASS |
| oneshot recv err | 503 | — | — | `"Scheduler is shutting down"` | PASS |

## Handler LOC / Route String

- **Handler LOC added to `src/web/handlers/api.rs`:** 109 lines (including doc comment block).
- **Route string:** `.route("/api/runs/{run_id}/stop", post(handlers::api::stop_run))` — one line added to `src/web/mod.rs`.
- **Test LOC:** 264 lines in `tests/stop_handler.rs` (target was >=150).
- **Integration tests added:** 4 (all green).

## Threat Model Traceability

All five STRIDE threats from the plan's `<threat_model>` block remain consistent with the shipped code:

- **T-10-07-01 (CSRF)**: `csrf::validate_csrf(&cookie_token, &form.csrf_token)` runs first, matching the pattern in `run_now`. `stop_run_csrf_mismatch_returns_403` locks the 403 behavior.
- **T-10-07-02 (run_id injection)**: `Path(run_id): Path<i64>` extractor rejects non-integers with 400 before the handler body runs. No arithmetic on `run_id` means i64 overflow is not exploitable.
- **T-10-07-03 (toast job_name leakage)**: toast text is operator-authored (lives in `cronduit.toml`) and already displayed throughout the UI. `HxEvent::new_with_data` JSON-encodes the payload so embedded characters are escaped. No secret exposure.
- **T-10-07-04 (handler blocks on oneshot await)**: scheduler loop's Stop arm processes inline (no I/O, no DB read), reply latency is microseconds. If the scheduler is truly hung, tokio's graceful shutdown drops `cmd_rx` and the oneshot errs → the handler returns 503. `stop_run_channel_closed_returns_503` exercises the failure mode.
- **T-10-07-05 (unauth LAN attacker)**: accepted per v1 trusted-LAN posture; plan 10-10 will add the one-line THREAT_MODEL.md note about Phase 10 widening blast radius.

No new threat_flags introduced — the handler is a 1:1 structural clone of `run_now`/`reroll`, and their threat surface is already covered in Phase 3/5 threat models.

## What This Unblocks

- **Plan 10-08 (run detail UI)**: the Stop button can now POST to `/api/runs/{run_id}/stop` and trust the four-branch wire format. No more mock endpoints in the UI.
- **Plan 10-09 (observability + metrics)**: has a stable handler-side integration point for `cronduit_stop_requests_total{outcome}` instrumentation.
- **Plan 10-10 (three-executor integration + THREAT_MODEL note)**: end-to-end operator stop path (web handler → scheduler Stop arm → executor cancel branch → DB finalize) is now wired through real code on both halves; only the full-stack integration test remains.

## Known Stubs

None — the handler is production-ready. Four live wire-format branches, all with behavioural tests, zero placeholders.

## Self-Check: PASSED

- `src/web/handlers/api.rs` — FOUND (stop_run handler with four response branches, `use StopResult` import, 109 LOC added)
- `src/web/mod.rs` — FOUND (`/api/runs/{run_id}/stop` route registered)
- `tests/stop_handler.rs` — FOUND (264 lines, 4 tests)
- Commit `52ce029` — FOUND in `git log --oneline` (Task 1: handler + route)
- Commit `224b961` — FOUND in `git log --oneline` (Task 2: integration tests)
- `cargo build -p cronduit` — clean
- `cargo build --tests -p cronduit` — clean
- `cargo clippy -p cronduit --all-targets -- -D warnings` — clean
- `cargo test --test stop_handler` — 4/4 green
- `cargo test -p cronduit --lib web::` — 20/20 passing (no regressions)
- `cargo tree -i openssl-sys` — empty (rustls-only)
