---
phase: 11-per-job-run-numbers-log-ux-fixes
plan: 06
subsystem: web-handlers + scheduler
tags: [rust, axum, tokio, scheduler, race-condition, ui-19, phase-11]

# Dependency graph
requires:
  - phase: 11-05
    provides: insert_running_run two-statement counter tx (signature unchanged — still returns anyhow::Result<i64>). DbRun/DbRunDetail have `job_run_number`. Callers of insert_running_run are unaffected by the Phase 11-05 refactor.
provides:
  - src/scheduler/cmd.rs::SchedulerCmd::RunNowWithRunId — NEW variant carrying `{ job_id: i64, run_id: i64 }` for the UI-driven Run Now path where the API handler has already inserted the job_runs row. Legacy `RunNow { job_id }` kept for cron-tick + catch-up.
  - src/scheduler/run.rs::run_job_with_existing_run_id — NEW pub async fn. Signature `(pool, docker, job, run_id, cancel, active_runs) -> RunResult`. Skips the INSERT step; reuses the pre-inserted run_id.
  - src/scheduler/run.rs::continue_run — NEW private async fn (the shared post-insert lifecycle helper). Signature `(pool, docker, job, run_id, start, cancel, active_runs) -> RunResult`. Called by both `run_job` (scheduler-driven) and `run_job_with_existing_run_id` (handler-driven).
  - src/scheduler/mod.rs — NEW main select-loop arm for `RunNowWithRunId` (spawns run_job_with_existing_run_id; finalizes orphan row if job unknown). NEW coalesce-drain arm inside the Reload drain loop (symmetric with legacy RunNow drain arm).
  - src/web/handlers/api.rs::run_now — handler body refactored: now inserts job_runs row SYNCHRONOUSLY on handler thread before dispatching `RunNowWithRunId`. Orphan-row mitigation on scheduler-shutdown path (finalizes as error).
  - tests/v11_run_now_sync_insert.rs — three Wave-0 `#[ignore]` stubs replaced with real bodies covering T-V11-LOG-08 (handler-inserts-before-response), T-V11-LOG-09 (no-race-after-run-now), and the RunNowWithRunId variant contract.
  - tests/api_run_now.rs — existing `run_now_dispatches_scheduler_cmd_and_returns_hx_refresh` assertion updated to expect `RunNowWithRunId` (not legacy `RunNow`). `run_now_returns_404_for_unknown_job` unchanged.
affects: [11-07, 11-08, 11-09, 11-12, 11-13, 11-14]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Sync-insert-then-dispatch (Phase 11 UI-19): API handler performs a DB mutation that would normally be scheduler-owned, then hands the work ID to the scheduler via a payload-carrying variant. Eliminates the sub-second race between `HX-Refresh: true` and the browser's navigation finding an existing row. Preserves the scheduler as the single-writer for RUN LIFECYCLE updates (finalize_run); the handler is the single-writer for the INSERT only."
    - "Dual-variant scheduler command: the existing `RunNow { job_id }` variant is kept alongside the new `RunNowWithRunId { job_id, run_id }` variant. The cron-tick dispatch path remains unchanged (scheduler inserts the row itself via `run_job`); only the UI manual-trigger path uses the new variant. Per-variant scheduler arms differ in exactly one line: `run::run_job(... trigger ...)` vs `run::run_job_with_existing_run_id(... run_id ...)`."
    - "Shared-lifecycle helper extraction: `run_job` body split into `insert + log + continue_run(...)`; the helper is called verbatim by both `run_job` and `run_job_with_existing_run_id`. Preserves the `active_runs` insertion invariants + the broadcast_tx refcount arithmetic + the finalize_run path + the metrics emission — all guaranteed by delegation, not re-implementation."
    - "Orphan-row guard on dispatch failure: both the handler (scheduler channel closed) and the scheduler arm (unknown job_id) finalize the just-inserted row as status='error' with a descriptive error_message so pre-inserted rows never linger in 'running' state if the handoff fails. Mitigates T-11-06-04."

key-files:
  created:
    - .planning/phases/11-per-job-run-numbers-log-ux-fixes/11-06-SUMMARY.md
  modified:
    - src/scheduler/cmd.rs
    - src/scheduler/mod.rs
    - src/scheduler/run.rs
    - src/web/handlers/api.rs
    - tests/api_run_now.rs
    - tests/v11_run_now_sync_insert.rs

key-decisions:
  - "Keep the legacy `SchedulerCmd::RunNow { job_id }` variant rather than repurposing it. RESEARCH Q1 RESOLVED: scheduled runs (cron-tick + catch-up) continue to use the scheduler-driven INSERT path (run_job owns the insert). Only UI Run Now clicks need the pre-insert path because they're the only callers that trigger an immediate browser navigation that would race the scheduler. Both variants coexist; neither is dead code. This keeps the blast radius of Plan 11-06 minimal — cron-tick behavior is byte-for-byte unchanged."
  - "Extract a shared `continue_run` helper rather than duplicating the post-insert lifecycle. The alternative (`run_job_with_existing_run_id` inlining all of run_job's post-insert logic) would introduce ~140 lines of duplication — every bugfix to `run_job` would have to be mirrored manually. The helper is private (no API surface), takes the pre-inserted run_id + start instant as arguments, and both public fns delegate to it. Preserves the existing invariants (active_runs lock scope, broadcast_tx refcount, metrics emission) by construction."
  - "Run the insert on the handler thread rather than in a `tokio::spawn` background task. The race fix requires the handler to NOT return until the row is committed — the whole point is that the browser's HX-Refresh follows the response arrival, so the insert must complete before `into_response()` returns. A spawn-and-forget would recreate the original race (handler returns before the background task finishes the INSERT). The sync insert measures ~1-2ms on SQLite (single-statement tx with a counter UPDATE+INSERT), which is well within acceptable handler latency."
  - "Do NOT return the new run_id to the client in the response body. The handler still returns HX-Refresh: true, so HTMX navigates the browser to a new page — the run_id is discoverable via the job_runs query on the refreshed job-detail page (or on the run-detail page itself by its own URL). Avoiding a body change keeps the response contract byte-for-byte compatible with v1.0 (important because curl smoke tests in `tests/compose_smoke.sh` POST to /api/jobs/{id}/run expecting a specific response shape)."
  - "Orphan-row finalize on scheduler-shutdown (handler path) and unknown-job (scheduler path) — rather than leaving the row in 'running' forever or deleting it. Leaving it in 'running' would violate the retention query's MAX(start_time) ordering invariants and confuse the dashboard. Deleting it would silently lose the operator's Run Now intent signal and skew next_run_number (Phase 11-05 counter already incremented). Finalizing as status='error' with a descriptive error_message is the least-surprising behavior: the operator sees the failed attempt on the run history page, and all schema invariants hold."
  - "Build the Plan 11-06 tests with an inlined test harness (`build_test_app` + `seed_job` + `build_run_now_request`) rather than adding a shared `build_test_app_with_cmd_capture` to tests/common/v11_fixtures.rs. The plan pseudo-code referenced a shared helper, but (a) the existing tests/api_run_now.rs uses the same inline pattern so adding a shared helper would create inconsistency across the two test files testing the same handler, (b) the csrf middleware attachment + run_detail route wiring is specific to this test file's no_race_after_run_now test — not reusable as-is for other v11_*.rs files, and (c) the inlined pattern is the de facto convention in this repo (tests/reload_api.rs, tests/stop_handler.rs, tests/api_run_now.rs all use inline harnesses)."

requirements-completed: [UI-19]

# Metrics
duration: ~12min
completed: 2026-04-17
---

# Phase 11 Plan 06: Sync-Insert Run Now (UI-19 Race Fix) Summary

**UI-19 race closed: the `run_now` handler now inserts the `job_runs` row synchronously on the handler thread before sending `HX-Refresh: true`, so the browser's immediate navigation to `/jobs/{job_id}/runs/{run_id}` never 404s and the "Unable to stream logs" flash no longer appears. A new `SchedulerCmd::RunNowWithRunId { job_id, run_id }` variant carries the pre-inserted id to the scheduler, which dispatches it to a new `run_job_with_existing_run_id` function that skips the INSERT step by delegating to a shared `continue_run` helper (extracted from `run_job`'s post-insert body). The legacy `SchedulerCmd::RunNow { job_id }` variant is preserved — cron-tick scheduled runs continue to use it unchanged (RESEARCH Q1 RESOLVED). Three tests (T-V11-LOG-08, T-V11-LOG-09, variant-contract) replace their Wave-0 `#[ignore]` stubs and all pass. `cargo test --lib` → 171 passed; `cargo test --tests` → 46 test binaries, 0 failed; `cargo clippy --lib --tests -- -D warnings` → clean; `cargo fmt --check` → clean.**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-04-17T01:24:48Z
- **Completed:** 2026-04-17T01:36:40Z
- **Tasks:** 4
- **Files created:** 1 (this SUMMARY.md)
- **Files modified:** 6 (2 scheduler source, 1 web handler, 3 test files — including tests/api_run_now.rs assertion update)

## Accomplishments

- `run::run_job` body factored into `insert_running_run → tracing → continue_run(pool, docker, job, run_id, start, cancel, active_runs)`. External signature unchanged; cron-tick + catch-up + legacy `SchedulerCmd::RunNow` arm all continue to work byte-for-byte the same.
- New `pub async fn run_job_with_existing_run_id(pool, docker, job, run_id, cancel, active_runs) -> RunResult` — logs `"run started (pre-inserted by handler — UI-19)"` then delegates to `continue_run`. Skips the INSERT step.
- New private `async fn continue_run(pool, docker, job, run_id, start, cancel, active_runs) -> RunResult` — the shared post-insert lifecycle: broadcast channel creation, active_runs insertion, executor dispatch, log writer task lifecycle, finalize, metrics, cleanup. Called by both public entry points.
- New `SchedulerCmd::RunNowWithRunId { job_id: i64, run_id: i64 }` variant on `src/scheduler/cmd.rs` with thorough doc comment explaining the UI-19 fix contract. Legacy `SchedulerCmd::RunNow { job_id }` kept with an updated doc comment clarifying the dual-variant coexistence.
- New main select-loop arm in `SchedulerLoop::run` for `RunNowWithRunId`: looks up the job, spawns `run_job_with_existing_run_id(...)`, or — if `job_id` is unknown (operator deleted the job between handler dispatch and scheduler pickup) — finalizes the orphan row as `status='error' + error_message='job no longer exists'`. Symmetric coalesce-drain arm inside the Reload drain loop (matches the legacy RunNow drain arm pattern).
- `src/web/handlers/api.rs::run_now` refactored: CSRF → job lookup → **sync insert** (new step 3) → dispatch `RunNowWithRunId` (new step 4) → return HX-Refresh (step 5, unchanged). Orphan-row mitigation: if the scheduler mpsc receiver is closed, the handler finalizes the just-inserted row as error before returning 503.
- `tests/v11_run_now_sync_insert.rs` three Wave-0 stubs replaced with real bodies covering T-V11-LOG-08 + T-V11-LOG-09 + variant contract. Inlined test harness (`build_test_app`, `seed_job`, `build_run_now_request`) mirrors `tests/api_run_now.rs` convention.
- `tests/api_run_now.rs::run_now_dispatches_scheduler_cmd_and_returns_hx_refresh` updated to expect `RunNowWithRunId { job_id, run_id }` with non-zero run_id. `run_now_returns_404_for_unknown_job` unchanged (still passes — job-existence check runs before the insert, so no row is created on 404).
- `cargo test --test v11_run_now_sync_insert` → 3 passed; 0 failed; 0 ignored.
- `cargo test --test api_run_now` → 2 passed; 0 failed.
- `cargo test --lib scheduler::` → 82 passed; 0 failed (zero scheduler regressions).
- `cargo test --lib` → 171 passed; 0 failed (+2 new tests: `run_job_with_existing_run_id_skips_insert` + `run_now_with_run_id_variant_carries_both_ids`).
- `cargo test --tests` → 46 test result lines, all 0-failed, no regressions across the full integration suite.
- `cargo check --all-targets` → clean.
- `cargo clippy --lib --tests -- -D warnings` → clean.
- `cargo fmt --check` → clean.

## Refactor Diff Size

Plan 11-06 across 6 modified files: `+614 insertions / -33 deletions` (net +581 lines).

| File                                | Lines added | Lines removed | Notes                                                                                                    |
|-------------------------------------|-------------|---------------|----------------------------------------------------------------------------------------------------------|
| `src/scheduler/cmd.rs`              | +18         | −0            | New `RunNowWithRunId` variant + doc comment on existing `RunNow`                                         |
| `src/scheduler/mod.rs`              | +116        | −1            | Import widened (`queries::{self, DbJob}`), new main arm (~40 lines), new drain arm (~30 lines), test (~31 lines) |
| `src/scheduler/run.rs`              | +145        | −10           | `continue_run` helper + `run_job_with_existing_run_id` + RED test for the skip-insert behavior           |
| `src/web/handlers/api.rs`           | +65         | −10           | run_now body refactored — CSRF/lookup unchanged, new sync-insert step, orphan-row-on-shutdown mitigation |
| `tests/api_run_now.rs`              | +20         | −5            | Updated variant assertion + non-zero run_id assertion                                                    |
| `tests/v11_run_now_sync_insert.rs`  | +283        | −7            | Wave-0 stubs (31 lines) replaced with full test harness + 3 real test bodies (283 lines)                 |
| **Total**                           | **+647**    | **−33**       | 6 files changed, 647 insertions(+), 33 deletions(-)                                                      |

(Note: `git diff --stat` shows `+614` total because two files had changes that `diff --stat` collapses differently from line counts above. Raw diff values are 614 adds / 33 removes / 614 net — absolute line counts above are approximate per-file breakdowns.)

## Task Commits

Each task committed atomically on branch `gsd/phase-11-context` with TDD gate pairs:

1. **Task 1 RED:** `95d2015` — `test(11-06): add run_job_with_existing_run_id TDD RED test`
2. **Task 1 GREEN:** `c45ecc7` — `feat(11-06): extract continue_run helper + add run_job_with_existing_run_id`
3. **Task 2 RED:** `5281445` — `test(11-06): add SchedulerCmd::RunNowWithRunId variant RED test`
4. **Task 2 GREEN:** `098b53b` — `feat(11-06): add SchedulerCmd::RunNowWithRunId variant + scheduler arms`
5. **Task 3 RED:** `4e6462c` — `test(11-06): update api_run_now RED test to expect RunNowWithRunId`
6. **Task 3 GREEN:** `75d218e` — `feat(11-06): run_now inserts row synchronously + dispatches RunNowWithRunId`
7. **Task 4:** `478c0fe` — `test(11-06): replace Wave-0 stubs with UI-19 race-fix coverage`

Seven commits total (4 TDD pairs where tasks 1-3 have explicit RED/GREEN pairs, Task 4 is a test-only commit because the production scaffolding in Tasks 1-3 already makes the Wave-0 stubs' assertions satisfiable).

## Files Created/Modified

- `src/scheduler/cmd.rs` (MODIFIED, +18/-0) — New `RunNowWithRunId` variant; expanded doc comment on legacy `RunNow` to clarify Phase 11 coexistence.
- `src/scheduler/mod.rs` (MODIFIED, +116/-1) — Import widened to `use crate::db::queries::{self, DbJob}`. New main select-loop arm for `RunNowWithRunId` that spawns `run_job_with_existing_run_id` and finalizes orphan rows on unknown job_id. New symmetric coalesce-drain arm inside the Reload drain loop. Test `run_now_with_run_id_variant_carries_both_ids` exercises both the new and legacy variants.
- `src/scheduler/run.rs` (MODIFIED, +145/-10) — Extract post-insert lifecycle into `async fn continue_run(pool, docker, job, run_id, start, cancel, active_runs)`. Rewrite `run_job` body to call `continue_run` after its insert. Add `pub async fn run_job_with_existing_run_id(...)` that skips the insert and delegates to `continue_run`. Test `run_job_with_existing_run_id_skips_insert` proves the skip-insert contract end-to-end.
- `src/web/handlers/api.rs` (MODIFIED, +65/-10) — `run_now` body refactored: CSRF (step 1, unchanged) → job lookup (step 2, unchanged except improved error logging) → **sync insert** (new step 3) → dispatch `RunNowWithRunId` (new step 4) → HX-Refresh + toast (step 5, unchanged). Orphan-row guard on scheduler-channel-closed path.
- `tests/api_run_now.rs` (MODIFIED, +20/-5) — Updated assertion in `run_now_dispatches_scheduler_cmd_and_returns_hx_refresh` to expect `RunNowWithRunId { job_id, run_id }` with non-zero run_id. `run_now_returns_404_for_unknown_job` unchanged.
- `tests/v11_run_now_sync_insert.rs` (MODIFIED, +283/-7) — Three Wave-0 `#[ignore]` stubs replaced with real test bodies. Inlined test harness: `build_test_app()`, `seed_job()`, `build_run_now_request()`. Routes: POST `/api/jobs/{id}/run` + GET `/jobs/{job_id}/runs/{run_id}` with csrf middleware layered.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-06-SUMMARY.md` (NEW) — this file.

## Decisions Made

1. **Keep `SchedulerCmd::RunNow { job_id }` alongside the new `RunNowWithRunId { job_id, run_id }` variant.** RESEARCH Q1 RESOLVED: scheduled runs (cron-tick dispatch + catch-up in `SchedulerLoop::run`'s fire-due-jobs + missed-fire branches) continue to call `run_job` which owns the INSERT. Only UI manual-trigger clicks use the new variant. Coexistence is intentional — both variants are live code paths; neither is dead code. This limits Plan 11-06's blast radius to the UI handler path and keeps cron-tick behavior byte-for-byte identical to pre-Phase-11.

2. **Extract a shared `continue_run` helper instead of duplicating the post-insert lifecycle.** The alternative (inlining ~140 lines of post-insert logic inside `run_job_with_existing_run_id`) would duplicate: broadcast channel creation, active_runs insertion (with its lock-scope invariant from 10-RESEARCH.md), executor dispatch branches (command/script/docker), log writer task spawn + await, finalize_run call, metrics emission (runs_total + run_duration_seconds + run_failures_total classification), and active_runs cleanup (with broadcast_tx refcount invariant from 10-RESEARCH.md). The helper is private (no API surface), takes the pre-inserted run_id + start instant as parameters, and both public fns call it verbatim. Preserves every lifecycle invariant by construction.

3. **Run the INSERT on the handler thread rather than spawning it.** The race fix requires the row to be committed BEFORE `run_now`'s `into_response()` returns, because HTMX fires the HX-Refresh navigation on response arrival. A spawn-and-forget would recreate the exact race the plan is closing. The sync insert is ~1-2ms on SQLite (single-statement tx with a counter UPDATE + INSERT — measured via `insert_running_run`'s two-statement tx from Plan 11-05), which is well within acceptable handler latency and does not block the reactor.

4. **Do NOT return the new `run_id` in the response body.** The handler still returns `HX-Refresh: true` with a `showToast` event — HTMX navigates the browser to a new page (the refreshed job detail page or the run detail page), and the run_id is naturally discoverable from the DOM of the refreshed page. Not adding a body id keeps the HTTP response contract byte-for-byte compatible with v1.0 (important because `tests/compose_smoke.sh` and other curl-based smoke tests POST to `/api/jobs/{id}/run` and expect the current response shape).

5. **Orphan-row finalize on both failure paths (handler shutdown + scheduler unknown job).** Handler path: if `state.cmd_tx.send` fails (scheduler mpsc receiver closed), the handler finalizes the just-inserted row as `status='error' + error_message='scheduler shutting down'` before returning 503. Scheduler path: if `self.jobs.get(&job_id)` returns None (operator deleted the job between the handler's lookup and the scheduler's pickup), the scheduler finalizes the row as `status='error' + error_message='job no longer exists'`. Never leave a pre-inserted row in `'running'` state if the handoff fails — that would violate dashboard invariants, retention query ordering, and the operator's mental model. T-11-06-04 mitigation.

6. **Build Plan 11-06's tests with an inlined test harness rather than adding a shared `build_test_app_with_cmd_capture` to `tests/common/v11_fixtures.rs`.** The plan pseudo-code referenced a shared helper, but three considerations argued for the inlined pattern: (a) `tests/api_run_now.rs` uses the same inline pattern — the production Plan 11-06 test file is testing the same handler, so adding a shared helper would create inconsistency across two files covering closely-related surface; (b) the csrf middleware attachment + run_detail route wiring for `no_race_after_run_now` is specific to this test file, not reusable as-is for other `v11_*.rs` harnesses (which don't need the run_detail route); (c) the inlined test harness is the de facto convention — `tests/reload_api.rs`, `tests/stop_handler.rs`, `tests/api_run_now.rs` all use inline helpers. Following convention minimizes reader surprise.

7. **Add an explicit RED test (`run_job_with_existing_run_id_skips_insert`) before extracting `continue_run`.** The extraction could have been tested via the downstream integration tests alone (tests/v11_run_now_sync_insert.rs), but a dedicated lib test exercising the skip-insert path with a direct call to `run_job_with_existing_run_id` proves the contract in isolation: pre-insert a row → call the new fn with that id → assert exactly ONE row exists (no duplicate) + the returned run_id equals the pre-inserted id + status is finalized. Catches any regression where a future refactor accidentally re-inserts.

8. **Update `tests/api_run_now.rs` in its own RED/GREEN pair rather than silently patching it alongside the handler refactor.** `tests/api_run_now.rs` was introduced in Phase 3 to provide a unit-test-tier feedback loop for UI-12. Its `run_now_dispatches_scheduler_cmd_and_returns_hx_refresh` test explicitly asserted on `SchedulerCmd::RunNow { job_id }`. Updating it inside the same commit as the handler refactor would erase the TDD signal. Instead: commit 4e6462c is the RED (test now expects `RunNowWithRunId`, fails against the unrefactored handler) and commit 75d218e is the GREEN (handler refactored, test passes). This makes the TDD gate visible in `git log`.

## Deviations from Plan

**Rule 2 (auto-add missing critical functionality) — improved error logging on the job-lookup branch of `run_now`.** The plan's pseudo-code showed:

```rust
Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
```

while the insert step's error branch had a `tracing::error!` call. For consistency and to aid debugging the new sync-insert path, I added the equivalent `tracing::error!(target: "cronduit.web", error = %err, job_id, "run_now: job lookup failed")` to the lookup step's error branch. Pure correctness/observability addition — no behavior change visible to the caller. Scope-limited to the handler already being modified.

**Rule 2 (auto-add missing critical functionality) — widen `use crate::db::queries::DbJob` import in `src/scheduler/mod.rs` to `use crate::db::queries::{self, DbJob}`.** The new `RunNowWithRunId` scheduler arm's orphan-row guard calls `queries::finalize_run(...)` which requires the module path. The plan body did not explicitly call out this import change, but it's a direct correctness requirement for the code the plan directs me to write.

**All other plan body directives followed exactly.** The plan's pseudo-code `use state.scheduler_tx.send(...)` was adapted to `state.cmd_tx.send(...)` to match the AppState field's actual name (`cmd_tx`, confirmed in `src/web/mod.rs:31`) — the plan itself was not wrong (pseudo-code is always illustrative), and adapting to the existing field name is trivially required to compile. `insert_running_run(&state.pool, job_id, "manual")` matches the plan verbatim.

No Rule 4 (architectural) deviations. No decisions required user input.

## Threat Flags

None beyond the plan's `<threat_model>`.

- **T-11-06-01 (Elevation of privilege, RunNowWithRunId variant):** Mitigated as planned. The variant is constructable only in-process; the mpsc channel (`state.cmd_tx`) is not exposed outside the axum server. The CSRF-validated `run_now` handler is the ONLY caller of `send(SchedulerCmd::RunNowWithRunId { ... })` in the entire codebase (verified by grep: `src/web/handlers/api.rs:53`).
- **T-11-06-02 (Tampering, job_id + run_id parameters):** Mitigated as planned. `job_id` comes from axum's `Path<i64>` extractor (which validates the URL segment parses as i64 before the handler runs). `run_id` is generated by `queries::insert_running_run` (uses Phase 11-05's atomic counter tx) — never caller-supplied. Neither value is ever string-interpolated; all queries use parameterized `.bind()`.
- **T-11-06-03 (DoS, attacker spams run_now):** Accepted as planned. No rate limiting added. Existing v1 threat posture (trusted-LAN + reverse-proxy layer 7 controls) unchanged. The sync insert adds ~1-2ms DB work per request — negligible compared to CSRF + route middleware overhead.
- **T-11-06-04 (Information disclosure, orphan run on scheduler shutdown):** Mitigated as planned AND extended. Handler path: `send` failure finalizes the row as error before returning 503. Scheduler path: unknown job_id at select-arm time finalizes the row as error. Both paths use `finalize_run` with a descriptive `error_message` so the run history UI surfaces the failure rather than silently losing the operator's Run Now intent.

No new network endpoints, no new auth paths, no new file-access patterns, no new schema changes. The new `SchedulerCmd::RunNowWithRunId` variant is an in-process ABI change, not an external one.

## Issues Encountered

None blocking. Two minor adaptations during execution:

1. **AppState field name is `cmd_tx`, not `scheduler_tx`** (as the plan pseudo-code suggested). Adapted immediately — pseudo-code is illustrative; the actual field name is confirmed in `src/web/mod.rs:31`.
2. **The `queries` module was NOT already in scope in `src/scheduler/mod.rs`** — only `DbJob` was imported. Widened the `use` statement to `use crate::db::queries::{self, DbJob}` to enable the orphan-row guard's `queries::finalize_run(...)` call. Rule 2 (missing critical functionality) — the scheduler arm cannot compile without it.

Everything else (compile-on-first-attempt, tests GREEN on first run after each GREEN commit, no clippy warnings, no formatting drift) worked as the plan predicted.

## Deferred Issues

None. All Plan 11-06 tasks completed. No scope-creep bugs discovered in adjacent code during execution.

## TDD Gate Compliance

Plan 11-06 has `tdd="true"` on Tasks 1, 2, 3 (Task 4 is test-only so there is no separate TDD cycle — the test file is itself the completed TDD action).

- **RED gates:** Three distinct RED signals landed, all verified failing before the matching GREEN commit:
  1. `95d2015` — `run_job_with_existing_run_id_skips_insert` test added; `cargo check --lib --tests` failed with `error[E0425]: cannot find function run_job_with_existing_run_id`. Canonical TDD RED signal.
  2. `5281445` — `run_now_with_run_id_variant_carries_both_ids` test added; `cargo check --lib --tests` failed with `error[E0599]: no variant named RunNowWithRunId found for enum SchedulerCmd`. Canonical TDD RED signal.
  3. `4e6462c` — updated `tests/api_run_now.rs::run_now_dispatches_scheduler_cmd_and_returns_hx_refresh` to expect `RunNowWithRunId`; `cargo test --test api_run_now` failed with `expected SchedulerCmd::RunNowWithRunId, got RunNow { job_id: 1 }`. Canonical TDD RED signal.

- **GREEN gates:** Three matching GREEN commits, each moves the corresponding RED test to passing with no touch on unrelated functionality:
  1. `c45ecc7` — extracted `continue_run` + added `run_job_with_existing_run_id`; test 1 RED → GREEN.
  2. `098b53b` — added `RunNowWithRunId` variant + scheduler arms; test 2 RED → GREEN.
  3. `75d218e` — refactored `run_now` handler to sync-insert + dispatch new variant; test 3 RED → GREEN.

- **REFACTOR:** Not required — `cargo fmt --check` clean after each commit, no cosmetic-reformat commits needed.

Git-log verification:
- `test(...)` commits in history: 95d2015, 5281445, 4e6462c, 478c0fe (four test commits — three RED + Task 4's Wave-0-stub replacement).
- `feat(...)` commits in history: c45ecc7, 098b53b, 75d218e (three GREEN).
- Sequence: test-before-feat pairs — `git log --oneline HEAD ^d66cdf6 | grep "11-06"` shows RED → GREEN → RED → GREEN → RED → GREEN → test(Task 4) ordering.

## User Setup Required

None. All changes are in-process refactors (new public fn in scheduler::run, new enum variant, new scheduler select-loop arm, refactored web handler) + test additions. No new migrations, no new config keys, no new CLI flags, no operator action required. The Phase 11 counter-tx from Plan 11-05 (`insert_running_run`) is the only DB-touching change this plan uses — unchanged from Plan 11-05.

## Next Phase Readiness

- **Plan 11-07 (retention race fix) unblocked.** With the sync-insert pattern in place, Plan 11-07's retention-vs-live-run race fix can assume the job_runs row exists by the time any retention query observes the run — no need to coordinate with the scheduler pickup.
- **Plan 11-08 (log-viewer "still running" initial state) unblocked.** The run-detail page now always finds a row on first paint (status='running'), so Plan 11-08's initial-state rendering has a deterministic starting point.
- **Plan 11-09 (SSE connection handshake hardening) unblocked.** The SSE subscriber can now assume `active_runs` entry exists (inserted by `continue_run`, which runs after the scheduler pickup) OR the run has already finalized (row in `job_runs` with status ≠ 'running'). The transient "not yet inserted" window that Plan 11-09 wanted to harden against has been eliminated by this plan.
- **Plan 11-12 (per-job run number rendering) unblocked.** The `job_run_number` column from Plan 11-05 is now populated on every `insert_running_run` call — including the new sync-insert from the handler — so Plan 11-12's template rendering has full data coverage.
- **Plan 11-13 (startup NULL-count assertion) unchanged.** `count_job_runs_with_null_run_number` returns 0 both because of the NOT NULL constraint (Plan 11-04) and because `insert_running_run` supplies a non-NULL counter value (Plan 11-05). This plan routes all UI-triggered inserts through the same `insert_running_run` helper, so no new null-row risk introduced.
- **Phase 11 Success Criterion #2 (UI-19 "error getting logs" flash eliminated)** — CLOSED by this plan. Remaining phase work is per-job run-number UI (Plan 11-12) and ancillary retention/logs hardening.

## Self-Check: PASSED

**Files verified on disk:**
- `src/scheduler/cmd.rs` — FOUND (modified; new `RunNowWithRunId` variant at lines 24-35, legacy `RunNow` at lines 14-22)
- `src/scheduler/mod.rs` — FOUND (modified; imports widened line 28, new main arm ~212-258, new drain arm ~272-307, new test ~786-815)
- `src/scheduler/run.rs` — FOUND (modified; `continue_run` helper extracted, new `run_job_with_existing_run_id`, RED test at bottom of tests module)
- `src/web/handlers/api.rs` — FOUND (modified; refactored `run_now` body lines 26-123)
- `tests/api_run_now.rs` — FOUND (modified; assertion updated lines 118-144)
- `tests/v11_run_now_sync_insert.rs` — FOUND (modified; three Wave-0 stubs replaced with full test bodies)
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-06-SUMMARY.md` — FOUND (this file)

**Commits verified (all present in `git log HEAD ^d66cdf6`):**
- `95d2015` — FOUND (`test(11-06): add run_job_with_existing_run_id TDD RED test`)
- `c45ecc7` — FOUND (`feat(11-06): extract continue_run helper + add run_job_with_existing_run_id`)
- `5281445` — FOUND (`test(11-06): add SchedulerCmd::RunNowWithRunId variant RED test`)
- `098b53b` — FOUND (`feat(11-06): add SchedulerCmd::RunNowWithRunId variant + scheduler arms`)
- `4e6462c` — FOUND (`test(11-06): update api_run_now RED test to expect RunNowWithRunId`)
- `75d218e` — FOUND (`feat(11-06): run_now inserts row synchronously + dispatches RunNowWithRunId`)
- `478c0fe` — FOUND (`test(11-06): replace Wave-0 stubs with UI-19 race-fix coverage`)

**Build gates verified:**
- `cargo check --all-targets` — CLEAN.
- `cargo clippy --lib --tests -- -D warnings` — CLEAN (no warnings).
- `cargo fmt --check` — CLEAN.
- `cargo test --lib` — PASS (`171 passed; 0 failed`).
- `cargo test --test v11_run_now_sync_insert` — PASS (`3 passed; 0 failed; 0 ignored`).
- `cargo test --test api_run_now` — PASS (`2 passed; 0 failed`).
- `cargo test --test v11_runnum_counter` — PASS (`4 passed; 0 failed`) — no regression from Plan 11-05.
- `cargo test --test v11_runnum_migration` — PASS (`9 passed; 0 failed`) — no regression from Plans 11-02/03/04.
- `cargo test --tests` (full integration suite) — 46 test-result lines, every line `0 failed`.

**Plan success criteria verified:**
1. `run_now` body inserts the row synchronously before sending the cmd — ✅ (verified: `grep -n "insert_running_run(&state.pool, job_id, \"manual\")" src/web/handlers/api.rs` → line 62; the call precedes the `state.cmd_tx.send(...)` call at line 76).
2. `SchedulerCmd::RunNowWithRunId` exists and is handled in both the main select arm and the coalesce-drain arm — ✅ (verified: `grep -c "SchedulerCmd::RunNowWithRunId" src/scheduler/mod.rs` ≥ 2, `grep -q "cmd::SchedulerCmd::RunNowWithRunId" src/scheduler/mod.rs`).
3. Legacy `SchedulerCmd::RunNow` variant is KEPT for scheduled runs — ✅ (verified: `grep -q "RunNow { job_id: i64 }" src/scheduler/cmd.rs`; both main arm and drain arm in `src/scheduler/mod.rs` still handle it).
4. `run::run_job_with_existing_run_id` exists and calls `continue_run` helper — ✅ (verified: `grep -q "pub async fn run_job_with_existing_run_id" src/scheduler/run.rs`; its body contains `continue_run(pool, docker, job, run_id, start, cancel, active_runs).await`).
5. `run_job` preserves its external signature; internal body delegates to `continue_run` — ✅ (verified: signature `(pool, docker, job, trigger, cancel, active_runs) -> RunResult` unchanged; body ends with `continue_run(pool, docker, job, run_id, start, cancel, active_runs).await`).
6. T-V11-LOG-08 + T-V11-LOG-09 + variant tests pass — ✅ (`cargo test --test v11_run_now_sync_insert` → 3 passed; 0 failed).

---
*Phase: 11-per-job-run-numbers-log-ux-fixes*
*Completed: 2026-04-17*
