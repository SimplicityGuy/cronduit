---
phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
plan: 02
subsystem: database
tags: [sqlx, sqlite, postgres, scheduler, fctx-06, scheduled_for, signature-widen]

# Dependency graph
requires:
  - phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
    plan: 01
    provides: "job_runs.scheduled_for TEXT NULL column on sqlite + postgres (migration `20260503_000009_scheduled_for_add.up.sql`)"
provides:
  - "insert_running_run widened to 5 args with scheduled_for: Option<&str>"
  - "DbRunDetail.scheduled_for: Option<String> field"
  - "get_run_by_id reads scheduled_for on both backends"
  - "src/scheduler/run.rs::run_job widened with scheduled_for: Option<String> trailing param"
  - "Scheduler tick path writes Some(entry.fire_time.to_rfc3339())"
  - "Catch-up path writes Some(missed_time.to_rfc3339())"
  - "Run Now (web/api) writes Some(now_rfc3339) so fire skew = +0 ms by definition"
  - "Foundation for Wave 2 plan 21-04 (run_detail handler reads run.scheduled_for to compute FIRE SKEW row)"
affects: [21-03, 21-04, 21-05, 21-06, 21-07, 21-08]

# Tech tracking
tech-stack:
  added: []  # zero new external crates (D-32 invariant preserved)
  patterns:
    - "Widening discipline: #[allow(clippy::too_many_arguments)] + per-arg doc comment when the param list IS the schema (mirrors P16 finalize_run pattern)"
    - "Ownership boundary across tokio::spawn: scheduler holds Option<String>; queries.rs takes Option<&str>; convert at call site via .as_deref()"
    - "Trigger-aware semantics live at scheduler call site (D-03): tick → Some(fire_time), catch-up → Some(missed_time), Run Now → Some(now), legacy fallback → None"

key-files:
  created: []
  modified:
    - src/db/queries.rs
    - src/scheduler/run.rs
    - src/scheduler/mod.rs
    - src/web/handlers/api.rs
    - src/webhooks/dispatcher.rs
    - src/webhooks/payload.rs
    - tests/reload_inflight.rs
    - tests/v13_sparkline_render.rs
    - tests/job_detail_partial.rs
    - tests/stop_handler.rs
    - tests/v12_fctx_streak.rs
    - tests/v12_run_rs_277_bug_fix.rs
    - tests/dashboard_render.rs
    - tests/v11_runnum_counter.rs
    - tests/v11_startup_assertion.rs
    - tests/stop_race.rs
    - tests/jobs_api.rs
    - tests/v13_timeline_render.rs
    - tests/docker_executor.rs
    - tests/common/v11_fixtures.rs
    - tests/scheduler_integration.rs
    - tests/stop_executors.rs
    - tests/process_group_kill.rs
    - tests/metrics_stopped.rs

key-decisions:
  - "Widened insert_running_run to 5 args with Option<&str> trailing per D-02; used #[allow(clippy::too_many_arguments)] mirroring the P16 finalize_run pattern"
  - "run_job widened with owned Option<String> (not &str) because scheduled_for crosses the tokio::spawn boundary; .as_deref() at the queries.rs call site"
  - "Catch-up path threads m.missed_time.to_rfc3339() — operator-meaningful semantics: gap between when the slot SAID this should fire and when it actually fired post-clock-jump"
  - "Run Now path writes Some(now_rfc3339) at handler thread (api.rs:82), satisfying landmine §7 / UI-SPEC § Copywriting Contract — skew = +0 ms by definition"
  - "Both legacy SchedulerCmd::RunNow arms (primary cmd loop + drain-coalesce) pass None — landmine §9 confirms neither arm fires today; defensive None keeps them compiling"

patterns-established:
  - "Whenever DbRunDetail gains a field, ALL constructors must add the field — including test fixtures in src/webhooks/dispatcher.rs and src/webhooks/payload.rs (Rule 3 auto-fix)"
  - "Whenever a scheduler entry-point (run_job) gains a param, ALL spawn sites in scheduler/mod.rs (cron tick + catch-up + RunNow primary + RunNow drain-coalesce + the two in-module shutdown_grace tests) must thread it"

requirements-completed: [FCTX-06]

# Metrics
duration: ~22min
completed: 2026-05-02
---

# Phase 21 Plan 02: insert_running_run + run_job widen for FCTX-06 Summary

**insert_running_run + run_job + DbRunDetail widened to carry the fire-decision-time `scheduled_for` RFC3339 timestamp — scheduler tick, catch-up, and Run Now paths all persist a value; legacy fallback paths persist None; FCTX panel can now compute fire skew in plan 21-04.**

## Performance

- **Duration:** ~22 min
- **Started:** 2026-05-02T19:38Z
- **Completed:** 2026-05-02T20:00Z
- **Tasks:** 3 (all atomic-committed)
- **Files modified:** 24 (4 production source files + 20 test/fixture files)

## Accomplishments
- `insert_running_run` 5-arg signature persisted in queries.rs with `Option<&str>` Phase-21 binding; INSERT SQL extends column list on both sqlite + postgres arms
- `DbRunDetail` carries `scheduled_for: Option<String>`; `get_run_by_id` SELECTs it on both arms; constructors populate the new field
- `run_job` (8-arg now) threads `Option<String>` from spawn site to insert call via `.as_deref()`
- 5 production scheduler spawn sites updated correctly: cron tick + catch-up write the appropriate fire time (`entry.fire_time.to_rfc3339()` / `m.missed_time.to_rfc3339()`); both legacy `SchedulerCmd::RunNow` arms (primary cmd loop + drain-coalesce) pass `None` per landmine §9
- Web Run Now handler computes `chrono::Utc::now().to_rfc3339()` and passes `Some(now_rfc3339.as_str())` so fire skew = +0 ms by definition
- All 22 plan-listed test callers + 4 in-file unit tests + 2 in-module scheduler tests + 13 additional run_job/DbRunDetail call sites the plan missed updated to match the new signatures (Rule 3 auto-fix)
- `cargo build --workspace` green; `cargo nextest run --no-fail-fast` 522/531 passed (9 sandbox-Docker failures, all pre-existing testcontainer-dependent tests, identical to plan 21-01 wave-end gate)
- `cargo tree -i openssl-sys` empty (D-32 rustls-only invariant holds)
- SQLite `idx_job_runs_job_id_start` plan unchanged — `tests/v12_fctx_explain.rs` and `tests/v13_timeline_explain.rs` SQLite tests pass; the new column is invisible to those query plans (the postgres explain test fails only because no Docker daemon is available in the sandbox)

## Task Commits

Each task was committed atomically:

1. **Task 1: Widen `insert_running_run` + `DbRunDetail` + `get_run_by_id` SELECT** — `3f657a4` (feat)
2. **Task 2: Thread `scheduled_for` through `run_job` + scheduler tick/catch-up + web Run Now handler** — `1fcd738` (feat)
3. **Task 3: Bulk-update test callers + DbRunDetail fixtures** — `cd895c1` (test)

**Plan metadata:** _added in the final docs commit at SUMMARY-write time_

## Files Created/Modified

**Production source (4):**
- `src/db/queries.rs` — widened `insert_running_run` (sig + INSERT SQL on both backends + `#[allow(clippy::too_many_arguments)]` doc); extended `DbRunDetail` with `pub scheduled_for: Option<String>`; extended `get_run_by_id` SELECT + constructors on both backends; 4 in-file unit tests updated
- `src/scheduler/run.rs` — `run_job` widened with `scheduled_for: Option<String>` trailing param + `.as_deref()` at the `insert_running_run` call site; 5 in-module test callers updated; in-module insert test (line ~961) updated
- `src/scheduler/mod.rs` — 4 spawn sites updated: cron tick (`Some(entry.fire_time.to_rfc3339())`), catch-up (`Some(m.missed_time.to_rfc3339())`), legacy `SchedulerCmd::RunNow` primary arm (`None`), legacy `SchedulerCmd::RunNow` drain-coalesce arm (`None`); 2 in-module shutdown_grace tests updated
- `src/web/handlers/api.rs` — Run Now path computes `now_rfc3339` and passes `Some(now_rfc3339.as_str())` so skew = +0 ms

**Webhook fixtures (2 — Rule 3 auto-fix; missed by plan):**
- `src/webhooks/dispatcher.rs` — DbRunDetail constructor in `#[cfg(test)]` golden-fixture block extended with `scheduled_for: None`
- `src/webhooks/payload.rs` — `fixture_run_detail` helper in `#[cfg(test)]` block extended with `scheduled_for: None`

**Test files plan-listed (14):** see Task 3 commit message for per-file call-count breakdown.

**Test files plan-missed (4 — Rule 3 auto-fix):**
- `tests/scheduler_integration.rs` — 7 `run_job` call sites updated
- `tests/stop_executors.rs` — 3 `run_job` call sites updated
- `tests/process_group_kill.rs` — 2 `run_job` call sites updated
- `tests/metrics_stopped.rs` — 1 `run_job` call site updated

## Decisions Made

- **`Option<&str>` at the queries.rs boundary; `Option<String>` at the scheduler boundary; convert via `.as_deref()`** — D-02 + research §A. The borrowed `&str` in queries.rs avoids forcing the writer pool transaction to own a String; the owned `Option<String>` in `run_job` is required because the value crosses the `tokio::spawn` boundary inside `scheduler/mod.rs`.
- **Catch-up path uses `m.missed_time.to_rfc3339()`** — the plan didn't explicitly call this out, but the operator-meaningful semantics (gap between when the slot SAID this should fire and when it actually fired post-clock-jump) align exactly with FIRE SKEW's UI purpose. Fits the trigger-aware-semantics-at-scheduler discipline of D-03.
- **`#[allow(clippy::too_many_arguments)]` for both `insert_running_run` AND `run_job`** — `insert_running_run` is the param-list-is-the-schema case (mirrors P16 `finalize_run`); `run_job` reaches 8 args because the new trailing `scheduled_for` joins 7 pre-existing scheduler-context args. Documented inline.
- **Both legacy `SchedulerCmd::RunNow` arms (primary cmd loop + drain-coalesce loop) pass `None`** — landmine §9 confirms neither arm fires today (the UI Run Now path uses `RunNowWithRunId` instead). Defensive `None` keeps the arms compiling and the FIRE SKEW row hidden if either ever does fire in the future.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] DbRunDetail constructors in webhooks/{dispatcher,payload}.rs missed by plan's `<files>` list**

- **Found during:** Task 3 (after running `cargo build --tests --workspace`, two `error[E0063]: missing field scheduled_for in initializer of queries::DbRunDetail` errors fired in `src/webhooks/dispatcher.rs:507` and `src/webhooks/payload.rs:124`)
- **Issue:** Adding the new `scheduled_for: Option<String>` field to `DbRunDetail` makes ALL existing struct constructors miss the field. The plan's `<files>` list called out the 2 constructors inside `src/db/queries.rs::get_run_by_id` (lines ~1348 + ~1370) but not the 2 test-fixture constructors in the webhooks module.
- **Fix:** Added `scheduled_for: None, // Phase 21 FCTX-06: test fixture` to both constructors. Both are inside `#[cfg(test)]` blocks; runtime behavior unaffected.
- **Files modified:** `src/webhooks/dispatcher.rs`, `src/webhooks/payload.rs`
- **Verification:** `cargo build --tests --workspace` clears the E0063 errors; downstream webhook payload tests (`v12_webhook_*`) all pass.
- **Committed in:** `cd895c1` (Task 3 bulk commit)

**2. [Rule 3 — Blocking] `run_job` callers in 4 test files + 2 in-module scheduler tests missed by plan's `<files>` list**

- **Found during:** Task 3 (after running `cargo build --tests --workspace`, `error[E0061]: this function takes 8 arguments but 7 arguments were supplied` fired across `tests/scheduler_integration.rs` (7 sites), `tests/stop_executors.rs` (3 sites), `tests/process_group_kill.rs` (2 sites), `tests/metrics_stopped.rs` (1 site), and `src/scheduler/mod.rs` (2 in-module shutdown_grace tests)).
- **Issue:** Widening `run_job` from 7 to 8 args breaks every existing caller. The plan focused on the `insert_running_run` test callers and listed `tests/common/v11_fixtures.rs:101` + `src/scheduler/run.rs:938` as the only `run_job`-adjacent test callers, but the actual `grep -rn 'run_job\b' tests/` enumerated 13 additional call sites. The plan's Action D explicitly anticipates this: "If grep finds additional production callers not listed in research §'Codebase Map', update them."
- **Fix:** Added trailing `None, // Phase 21 FCTX-06: test passes None` argument at every additional call site. All call sites are tests; runtime behavior unaffected.
- **Files modified:** `tests/scheduler_integration.rs`, `tests/stop_executors.rs`, `tests/process_group_kill.rs`, `tests/metrics_stopped.rs`, `src/scheduler/mod.rs`
- **Verification:** `cargo build --tests --workspace` clears the E0061 errors; the affected scheduler integration tests all pass under `cargo nextest run` (e.g., `scheduler_integration::*`, `stop_executors::*`, `metrics_stopped::*`).
- **Committed in:** `cd895c1` (Task 3 bulk commit)

**3. [Rule 3 — Blocking] Plan listed 22 test callers, actual count is 26 (per-file enumeration sums to 26)**

- **Found during:** Task 3 (the plan's prose says "Total: 22 call sites updated → expected total `, None` adds = 22"; the per-file checklist sums to 1+1+2+1+3+4+2+5+1+1+1+2+1+1 = 26).
- **Issue:** The plan template enumerated each file's callers correctly but the prose total is off by 4. No structural impact — the per-file checklist was always the source of truth; updating "all enumerated sites" works regardless of the wrong sum.
- **Fix:** Updated all 26 sites the per-file checklist enumerates.
- **Files modified:** none beyond the plan-listed files; the per-file fix counts match the per-file enumeration.
- **Verification:** `grep -rn 'insert_running_run' tests/ | grep -v '//' | grep -c ', None)'` returns 26 (matches per-file enumeration).
- **Committed in:** `cd895c1` (Task 3 bulk commit)

---

**Total deviations:** 3 auto-fixed (3 blocking — all Rule 3 caller-update follow-throughs the plan understated)
**Impact on plan:** All 3 are mechanical signature-propagation: any code that constructs `DbRunDetail` or calls `run_job` had to gain the new field/arg or fail to compile. No scope creep, no behavioral change, no logic change. Production semantics align exactly with the plan's intent (`Some(fire_time)` for tick/catch-up, `Some(now_rfc3339)` for Run Now, `None` for legacy fallback).

## Issues Encountered

- **Postgres testcontainer tests cannot run in this sandbox:** the same 9 tests that failed at plan 21-01's wave-end gate (`dashboard_jobs_pg`, `db_pool_postgres`, `schema_parity::sqlite_and_postgres_schemas_match_structurally`, all `v11_bulk_toggle_pg::*`, `v13_timeline_explain::explain_uses_index_postgres`) fail again here with `Client(Init(SocketNotFoundError("/var/run/docker.sock")))`. They require `testcontainers-modules::postgres::Postgres` which spins up a Postgres container via the host Docker daemon — the sandbox has no Docker daemon. All other 522 tests pass, including the SQLite explain tests for both Phase 16 (`v12_fctx_explain::explain_uses_index_sqlite`) and Phase 13 (`v13_timeline_explain::explain_uses_index_sqlite`) — confirming the new `scheduled_for` column doesn't shift the existing `idx_job_runs_job_id_start` index plans on SQLite. Postgres explain parity verifies on CI where Docker is available.

## User Setup Required

None — schema-only column write site landed in plan 21-01; this plan only adds Rust code that writes the column. No new env vars, no config changes, no operator-visible surface (the FIRE SKEW UI row lands in plan 21-04).

## Next Phase Readiness

- **Wave 2 plan 21-04 (run_detail handler wire-up + FCTX panel template)** can now read `run.scheduled_for: Option<String>` from `DbRunDetail` after `get_run_by_id` and pass it to the askama template context for fire-skew computation. The FIRE SKEW row should hide on `None` per UI-SPEC + D-04.
- **Plan 21-08 / 21-09 integration tests** can seed `scheduled_for` values via direct SQL (the `seed_run_with_scheduled_for` helper the plan calls out for Wave 3 plans 21-07 / 21-08) without extending the shared `tests/common/v11_fixtures.rs::seed_running_run` (which now passes `None` per the plan's note on the v11_fixtures helper).
- **Postgres-backend integration tests** that depend on the new column (e.g., a future "scheduled_for round-trips on postgres" test) must run on CI with Docker available; the sandbox's `SocketNotFoundError` is environmental, not a plan defect.

## Threat Flags

None — the new write site is internal-only. `chrono::Utc::now().to_rfc3339()` (handler thread) and `entry.fire_time.to_rfc3339()` (scheduler tick) are both system-time-derived, not operator-supplied. The threat register's T-21-02-01 (Tampering, accept) and T-21-02-02 (Information Disclosure, accept) remain valid as written. ASVS V5 input-validation: `scheduled_for` is never read from operator input.

## Self-Check: PASSED

- Commit `3f657a4` (Task 1) — FOUND in `git log`
- Commit `1fcd738` (Task 2) — FOUND in `git log`
- Commit `cd895c1` (Task 3) — FOUND in `git log`
- `src/db/queries.rs` — `insert_running_run` signature has `scheduled_for: Option<&str>` (line 394); `DbRunDetail` has `pub scheduled_for: Option<String>` field; `get_run_by_id` SELECTs `r.scheduled_for` on both backends; constructors populate the field on both backends
- `src/scheduler/run.rs::run_job` — signature has `scheduled_for: Option<String>` trailing param; insert call passes `.as_deref()`
- `src/scheduler/mod.rs` — `fire_time.to_rfc3339()` thread present (cron tick path); `missed_time.to_rfc3339()` thread present (catch-up path); both legacy RunNow arms pass `None`
- `src/web/handlers/api.rs:82-91` — `let now_rfc3339 = chrono::Utc::now().to_rfc3339();` followed by `Some(now_rfc3339.as_str())` in the insert_running_run call
- `src/webhooks/dispatcher.rs` and `src/webhooks/payload.rs` — DbRunDetail fixtures include `scheduled_for: None`
- `cargo build --workspace` — exits 0
- `cargo build --tests --workspace` — exits 0
- `cargo nextest run --no-fail-fast` — 522 passed, 9 failed (all 9 = `SocketNotFoundError("/var/run/docker.sock")`; sandbox limitation; verified by `grep -E "SocketNotFound|FAIL\b"`)
- `cargo tree -i openssl-sys` — returns "package ID specification ... did not match any packages" (D-32 invariant)
- `grep -rn 'insert_running_run' src/ tests/` — 0 four-arg calls remain (every call site has the trailing arg)
- `grep -rn 'run_job(' src/ tests/` — 0 seven-arg calls remain in test/production code

---
*Phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2*
*Plan: 02*
*Completed: 2026-05-02*
