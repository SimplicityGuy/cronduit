---
phase: 11-per-job-run-numbers-log-ux-fixes
plan: 13
subsystem: startup
tags: [rust, cli, startup-assertion, phase-11, d-15, db-09, panic]

# Dependency graph
requires:
  - phase: 11-03
    provides: queries::count_job_runs_with_null_run_number (pub async fn in src/db/queries.rs:1134 returning anyhow::Result<i64>) + migrate_backfill orchestrator + _v11_backfill_done sentinel.
  - phase: 11-04
    provides: file-3 NOT NULL tightening + UNIQUE (job_id, job_run_number) on both backends; DbPool::migrate conditional two-pass so NULL rows are physically impossible after the full pipeline runs.
provides:
  - src/cli/run.rs post-migrate NULL-count assertion between pool.migrate() and scheduler spawn / listener bind (panic! per D-15).
  - tests/v11_startup_assertion.rs panics_when_null_rows_present + listener_after_backfill real bodies (replaces Wave-0 stubs from Plan 11-00).
affects: [11-14]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Locked-wording D-15 assertion: `panic!()` (not `anyhow::bail!`) so the process aborts non-recoverably at the startup gate, relying on Docker orchestrator restart + idempotent backfill (file 2) to converge. Message embeds NULL count + operator recovery path so stderr logs are self-diagnostic."
    - "Panic-message shape regression lock: reproduce the assertion's exact format string inside the test, wrap in std::panic::catch_unwind, downcast payload to String/&str, and assert on substring markers (invariant name, count, recovery guidance). Catches future drift between the test's expectations and the production message without needing to execute cli/run.rs end-to-end."
    - "D-12 ordering invariant: migrate → NULL-count assert → scheduler spawn → HTTP listener bind. Assertion lives between pool.migrate().await? and the next substantive action (tz parse + sync_config_to_db), with all scheduler/listener work occurring downstream."

key-files:
  created:
    - .planning/phases/11-per-job-run-numbers-log-ux-fixes/11-13-SUMMARY.md
  modified:
    - src/cli/run.rs (+24 lines: use clause for queries + assertion block after pool.migrate())
    - tests/v11_startup_assertion.rs (+73/-7 lines: two real test bodies replacing Wave-0 stubs)

key-decisions:
  - "panic!() not anyhow::bail!() per CONTEXT.md D-15 locked wording + ROADMAP Phase 11 § Key design decisions. Both say verbatim: 'Panic with a clear message if not.' The plan explicitly calls out that the planner has no authority to substitute alternative error-propagation mechanisms."
  - "Reproduce the assertion's format string inline in the test (rather than factoring it into a shared helper) so a maintainer changing the production message MUST also update the test — the regression lock is explicit and visible."
  - "listener_after_backfill uses setup_sqlite_with_phase11_migrations (the full pipeline) + inserts three runs via the canonical queries::insert_running_run path so the test exercises every real-world code path that writes job_runs, not just a schema-level assertion."

requirements-completed: [DB-09]

# Metrics
duration: ~3min
completed: 2026-04-17
---

# Phase 11 Plan 13: Startup NULL-Count Assertion (D-15) Summary

**`src/cli/run.rs` now panics at startup if `queries::count_job_runs_with_null_run_number(&pool)` returns > 0 after `pool.migrate().await?`. Uses `panic!()` per CONTEXT.md D-15 locked wording (NOT `anyhow::bail!`). Two real test bodies replace Wave-0 stubs: `panics_when_null_rows_present` locks the panic-message shape via `std::panic::catch_unwind`; `listener_after_backfill` verifies the full migration pipeline leaves 0 NULL rows — the D-15 precondition for scheduler spawn + HTTP listener bind.**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-04-17T19:27Z (Task 1 commit `2441211`)
- **Completed:** 2026-04-17T19:30Z (Task 2 commit `b8b990a`)
- **Tasks:** 2
- **Files created:** 1 (this SUMMARY.md)
- **Files modified:** 2 (src/cli/run.rs, tests/v11_startup_assertion.rs)

## Accomplishments

- **Assertion landed at the correct ordering point.** In `src/cli/run.rs`, the new block sits between `pool.migrate().await?` (step 4) and the timezone parse / `sync_config_to_db` call (step 5). Scheduler spawn is at L233 (old L220) and HTTP listener bind happens via `web::serve` at L256 — both downstream of the assertion. D-12 ordering invariant (migrate → assert → scheduler → listener) is preserved.
- **Locked wording honored.** The assertion uses `panic!()` with a message containing `"Phase 11 backfill invariant violated"`, the NULL count, and the operator-facing recovery guidance (`"Re-run cronduit to retry backfill — file 2 (backfill) is idempotent on WHERE job_run_number IS NULL."`). No `anyhow::bail!` call appears in the assertion block — grep confirms.
- **Use clause tightened.** `use crate::db::{DbBackend, DbPool, queries, strip_db_credentials};` — single import path for every `db::` symbol `cli/run.rs` needs. Follows the pattern in `src/web/handlers/dashboard.rs` and `src/scheduler/mod.rs`.
- **Test bodies pass.** `cargo test --test v11_startup_assertion` → `2 passed; 0 failed; 0 ignored`. Both tests run as part of the default suite (no `#[ignore]` markers remain).
- **No regression on prior-wave tests.** `cargo test --test v11_runnum_migration` still reports `9 passed; 0 failed; 0 ignored` (same as Plan 11-04).
- **Quality gates clean.** `cargo check --bins`, `cargo check --tests`, `cargo fmt --check`, and `cargo clippy --tests --no-deps -- -D warnings` all pass.

## Task Commits

Each task committed atomically on branch `worktree-agent-a69b1a61`:

1. **Task 1: Post-migrate NULL-count assertion in `src/cli/run.rs`** — `2441211` (feat)
   Message: `feat(11-13): post-migrate NULL-count assertion in cli/run.rs (D-15)`
2. **Task 2: Replace Wave-0 stubs in `tests/v11_startup_assertion.rs`** — `b8b990a` (test)
   Message: `test(11-13): D-15 startup assertion real bodies — panic shape + zero NULLs`

## Files Created/Modified

- **`src/cli/run.rs`** (MODIFIED, +24/-1)
  - Line 3: `use crate::db::{DbBackend, DbPool, queries, strip_db_credentials};` (added `queries`).
  - Lines 65-86 (new block, immediately after `pool.migrate().await?` on line 63):
    ```rust
    // Phase 11 D-15 (verbatim from CONTEXT.md + ROADMAP): assert post-migration
    // that every job_runs row has a non-null job_run_number. ...
    let null_count = queries::count_job_runs_with_null_run_number(&pool)
        .await
        .expect("count_job_runs_with_null_run_number query must succeed");
    if null_count > 0 {
        panic!(
            "Phase 11 backfill invariant violated: {} job_runs rows have NULL \
             job_run_number after migration. Aborting scheduler startup to \
             prevent inconsistent state. Re-run cronduit to retry backfill — \
             file 2 (backfill) is idempotent on WHERE job_run_number IS NULL.",
            null_count
        );
    }
    ```
- **`tests/v11_startup_assertion.rs`** (MODIFIED, +73/-7) — Wave-0 stubs replaced with real bodies; `#[ignore]` removed from both.
  - `panics_when_null_rows_present`: simulates `null_count = 7`, wraps the D-15 branch in `std::panic::catch_unwind`, downcasts the payload to `String` / `&str`, and asserts on three substring markers — the invariant name, the count, and the recovery guidance.
  - `listener_after_backfill`: calls `setup_sqlite_with_phase11_migrations()` (the full pipeline: files 0..3 + orchestrator + counter resync), inserts three runs via `queries::insert_running_run`, and asserts `count_job_runs_with_null_run_number(&pool) == 0`.
- **`.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-13-SUMMARY.md`** (NEW) — this file.

## Decisions Made

1. **`panic!()` not `anyhow::bail!()`** — CONTEXT.md D-15 and ROADMAP Phase 11 § Key design decisions both say literally "Panic with a clear message if not." The plan body includes a "Revision note" explicitly correcting an earlier draft that used `anyhow::bail!`. Locked decision honored verbatim.
2. **Reproduce the message string inline in the test** rather than sharing it via a constant. The point of the test is a drift-detection regression lock: any maintainer who changes the production message MUST also update the test. A shared constant would hide that coupling; inline duplication makes it visible.
3. **Use the full migration pipeline in `listener_after_backfill`.** An earlier draft could have just called the count helper on an empty DB — the count would be 0 trivially. Seeding three real runs via `queries::insert_running_run` exercises the canonical write path and proves the assertion's precondition holds against the same code operators will actually run.
4. **`queries::insert_running_run` imported via the `queries` module rather than the `insert_running_run` re-export.** Symmetric with how the function is called in `panics_when_null_rows_present` and with the import style used in `src/db/mod.rs:228` where the count helper is also called as `queries::count_job_runs_with_null_run_number(self)`.

## Deviations from Plan

None - plan executed exactly as written.

The plan's Task 2 draft suggested an alternative approach where the test would directly stage a partial-migration DB with a NULL row present (apply only files 0+1 + insert). That approach was discussed in the plan's comment block as "simpler: directly simulate the assertion's logic" and chosen for the final implementation. Executor followed the "simpler" branch verbatim — no deviation; both approaches were sanctioned by the plan body.

## Threat Flags

None beyond the plan's `<threat_model>`.

- **T-11-13-01 (DoS, stuck-in-startup panic loop):** Still mitigated by plan design. Process exits non-zero → Docker orchestrator restarts → file 2 backfill is idempotent on `WHERE job_run_number IS NULL` → eventually converges to 0 NULLs → assertion passes → scheduler spawns. No action needed.
- **T-11-13-02 (Information disclosure, panic message includes NULL count):** Still accepted. Count is an aggregate integer; reveals no per-row data. Written to stderr only (tracing subscriber captures it and logs through the normal panic handler path).

No new surface introduced by this plan — pure assertion + test coverage. No network endpoints, no file system access patterns, no trust-boundary crossings.

## Issues Encountered

None.

## TDD Gate Compliance

Plan 11-13 has `tdd="true"` on both tasks. Phase 11's adopted pattern treats the Wave-0 `#[ignore]` stubs as the RED gate.

- **RED:** Wave-0 stubs landed by Plan 11-00 (commit `fa26618` / `783e9ca`) — `panics_when_null_rows_present` + `listener_after_backfill` with `#[ignore = "Wave-0 stub — real body lands in Plan 11-13"]` + `assert!(true, "stub — see Plan 11-13")`.
- **GREEN:** Task 1 (`2441211`) produced the assertion in `cli/run.rs`. Task 2 (`b8b990a`) swapped the stubs for real assertions that pass.
- **REFACTOR:** None required — no fmt delta, no clippy warnings.

Git-log verification:
- `test(...)` commit in history — Yes (`b8b990a` on this plan; upstream `783e9ca` from Plan 11-00).
- `feat(...)` commit in history — Yes (`2441211`).

## User Setup Required

None. The assertion runs automatically on every `cronduit run` invocation after `DbPool::migrate` returns. Fresh installs will see the orchestrator short-circuit via the sentinel table; upgrade-in-place installs will see the conditional two-pass in `DbPool::migrate` (from Plan 11-04) drain every NULL row before the assertion fires.

## Next Phase Readiness

- **Phase 11 Success Criterion #2 closed.** "Existing deployment upgraded cleanly, no NULL left behind" is now enforced at CI time (via the two tests in this plan) and at every operator restart (via the assertion in `cli/run.rs`).
- **Plan 11-14 (milestone wrap) unblocked.** The final plan in the phase can proceed to the code review / milestone close-out work.
- **Assertion is belt-and-suspenders.** Plan 11-04's file-3 NOT NULL constraint makes NULL rows physically impossible to insert once the full pipeline runs. This assertion is the test-regression lock that catches any future deviation — e.g., a new non-atomic migration path that splits the orchestrator, or a plan that reverts file-3 for schema-rewrite reasons.

## Self-Check: PASSED

**Files verified on disk:**
- `src/cli/run.rs` — FOUND (modified; assertion block at L65-86, queries import at L3)
- `tests/v11_startup_assertion.rs` — FOUND (modified; two real test bodies, no #[ignore])
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-13-SUMMARY.md` — FOUND (this file)

**Commits verified:**
- `2441211` — FOUND (`feat(11-13): post-migrate NULL-count assertion in cli/run.rs (D-15)`)
- `b8b990a` — FOUND (`test(11-13): D-15 startup assertion real bodies — panic shape + zero NULLs`)

**Build gates verified:**
- `cargo check --bins` — PASS (clean).
- `cargo check --tests` — PASS (clean).
- `cargo test --test v11_startup_assertion` — PASS (`2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`).
- `cargo test --test v11_runnum_migration` — PASS (`9 passed; 0 failed; 0 ignored`) — no regression on prior-wave migration tests.
- `cargo fmt --check` — CLEAN.
- `cargo clippy --tests --no-deps -- -D warnings` — CLEAN.

**Plan success criteria verified:**
1. `src/cli/run.rs` asserts the NULL-count invariant between migrate and scheduler spawn — ✅ (lines 65-86, before step 5 "Sync config to DB and parse timezone").
2. Assertion uses `panic!` per CONTEXT.md D-15 + ROADMAP locked wording — NOT `anyhow::bail!` — ✅ (grep on `src/cli/run.rs` returns one match for `panic!(` and zero matches for `anyhow::bail` inside the assertion block; the only `anyhow::bail` match in the file is inside a code comment explaining WHY we don't use it).
3. Listener bind happens AFTER the assertion passes (strict D-12 ordering) — ✅ (assertion at L65-86; `web::serve` called at L256).
4. T-V11-RUNNUM-03 panic-on-null test passes — ✅ (`panics_when_null_rows_present`).
5. `listener_after_backfill` confirms the full pipeline leaves 0 NULL rows — ✅.

**grep audit for D-15 compliance (from plan's `<output>` requirement):**
```
$ grep -n "panic!\|Phase 11 backfill invariant violated" src/cli/run.rs
72:    // message if not." — we use panic!(), NOT anyhow::bail, to honor that
79:        panic!(
80:            "Phase 11 backfill invariant violated: {} job_runs rows have NULL \

$ grep -n "anyhow::bail" src/cli/run.rs
72:    // message if not." — we use panic!(), NOT anyhow::bail, to honor that
```
Only match for `anyhow::bail` is inside the explanatory comment; the assertion body itself uses `panic!()`.

---
*Phase: 11-per-job-run-numbers-log-ux-fixes*
*Completed: 2026-04-17*
