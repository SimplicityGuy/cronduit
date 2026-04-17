---
phase: 11-per-job-run-numbers-log-ux-fixes
plan: 00
subsystem: testing
tags: [rust, cargo-test, nyquist-rule, test-harness, sqlx, sqlite, phase-11]

# Dependency graph
requires:
  - phase: 10-stop-executors
    provides: existing tests/ integration test infra (cargo test, #[tokio::test]) and shared DbPool / PoolRef abstractions in src/db.
provides:
  - 10 Wave-0 test harness files (tests/v11_*.rs) landed as compiling #[ignore] stubs, one file per downstream Phase-11 plan.
  - tests/common/v11_fixtures.rs + tests/common/mod.rs — shared Phase 11 fixtures (setup_sqlite_with_phase11_migrations, seed_test_job, seed_running_run, make_test_batch, seed_null_runs).
  - Nyquist-rule satisfaction — every Phase-11 test exists on disk before its owning plan executes.
affects: [11-01, 11-02, 11-03, 11-04, 11-05, 11-06, 11-07, 11-08, 11-09, 11-10, 11-11, 11-12, 11-13]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Wave-0 stub convention: harness files land pre-implementation with `#[ignore = \"Wave-0 stub — real body lands in Plan XX\"]` attrs; owners remove `#[ignore]` and replace the `assert!(true, ...)` body with real assertions."
    - "Crate-level `#![allow(clippy::assertions_on_constants)]` at harness top permits `assert!(true, \"stub — see Plan XX\")` bodies to record ownership inline without tripping clippy `-D warnings`."
    - "Shared fixture module: tests/common/ holds cross-test helpers, included via `mod common;` + `use common::v11_fixtures::*;` in each integration test file."

key-files:
  created:
    - tests/common/v11_fixtures.rs
    - tests/common/mod.rs
    - tests/v11_runnum_migration.rs
    - tests/v11_run_now_sync_insert.rs
    - tests/v11_sse_log_stream.rs
    - tests/v11_sse_terminal_event.rs
    - tests/v11_run_detail_page_load.rs
    - tests/v11_startup_assertion.rs
    - tests/v11_log_dedupe_benchmark.rs
    - tests/v11_log_id_plumbing.rs
    - tests/v11_runnum_counter.rs
    - tests/v11_log_dedupe_contract.rs
  modified: []

key-decisions:
  - "Adapted seed_test_job fixture to bind the actual initial-migration column set (name, schedule, resolved_schedule, job_type, config_json, config_hash, timeout_secs, created_at, updated_at) instead of the plan's draft (cron, kind, command) which referenced columns that do not exist in migrations/sqlite/20260410_000000_initial.up.sql."
  - "Added crate-level `#![allow(clippy::assertions_on_constants)]` to every stub file so `assert!(true, \"stub — see Plan XX\")` compiles under `cargo clippy --tests -- -D warnings`. This preserves the ownership-in-assertion-message pattern the plan specified."
  - "VALIDATION.md frontmatter + checklist were already at the Wave-0-complete target state (nyquist_compliant: true, wave_0_complete: true, all ✅ W0 / - [x]) when this plan began, reflecting planner pre-population. Task 3 is a no-op; no VALIDATION.md edit was needed."

patterns-established:
  - "Wave-0 stub pattern: #[tokio::test] + #[ignore = \"Wave-0 stub — real body lands in Plan XX\"] + assert!(true, \"stub — see Plan XX\"); swapped to real bodies by the owning plan."
  - "Fixture sqlite-only panic-guard: match on PoolRef::Sqlite(_) vs PoolRef::Postgres(_) with explicit panic on the Postgres arm for sqlite-only fixtures."
  - "Ownership-in-assertion-message: every stub body names its owning plan in the panic message so grep surfaces drift if a downstream plan skips its stub."

requirements-completed: [DB-09, DB-10, DB-11, DB-12, DB-13, UI-16, UI-17, UI-18, UI-19, UI-20]

# Metrics
duration: ~15min
completed: 2026-04-16
---

# Phase 11 Plan 00: Wave-0 Test Harness Pre-Landing Summary

**10 Phase-11 test harness files landed as compiling `#[ignore]` stubs + shared `v11_fixtures` module, restoring Nyquist-rule compliance before any Wave-1 plan runs.**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-04-16T23:42:00Z (approx.)
- **Completed:** 2026-04-16T23:58:08Z
- **Tasks:** 3 (Task 3 no-op — VALIDATION.md already at target state)
- **Files created:** 12 (2 fixture files + 10 harness files)
- **Files modified:** 0

## Accomplishments

- Every Phase-11 test file named in `11-VALIDATION.md § Wave 0 Requirements` exists on disk as a compiling `#[ignore]` stub. Downstream plans APPEND real bodies instead of creating new files.
- 37 stub tests across 10 harness files, each attributed with `#[ignore = "Wave-0 stub — real body lands in Plan XX"]` so the owning plan is machine-discoverable via `cargo test -- --ignored`.
- Shared `tests/common/v11_fixtures.rs` module provides the canonical SQLite-in-memory setup + 4 seed helpers used by every downstream test body.
- `cargo check --tests`, `cargo clippy --tests -- -D warnings`, and `cargo test --tests` all run clean. Full suite: 169 lib-crate tests pass + 36 Phase-11 v11 stubs ignored + all pre-existing integration tests pass with 0 failures.

## Task Commits

Each task committed atomically:

1. **Task 1: Shared Phase 11 test fixtures** — `fa26618` (test)
2. **Task 2: 10 harness files as compiling stubs** — `783e9ca` (test)
3. **Task 3: VALIDATION.md Wave-0 markers** — NO-OP (planner pre-populated; see Deviations)

_No TDD cycle: this plan is a scaffolding-only setup plan; test bodies land in owning plans._

## Files Created/Modified

- `tests/common/v11_fixtures.rs` (NEW, 106 lines) — Phase 11 shared fixtures: `setup_sqlite_with_phase11_migrations`, `seed_test_job`, `seed_running_run`, `make_test_batch`, `seed_null_runs`.
- `tests/common/mod.rs` (NEW, 6 lines) — module registration for `v11_fixtures`.
- `tests/v11_runnum_migration.rs` (NEW, 10 stubs) — Plans 11-02/03/04 migration tests (`migration_01_*`, `migration_02_*`, `migration_03_*`).
- `tests/v11_run_now_sync_insert.rs` (NEW, 3 stubs) — Plan 11-06 UI-19 race fix.
- `tests/v11_sse_log_stream.rs` (NEW, 2 stubs) — Plan 11-08 SSE id-line emission.
- `tests/v11_sse_terminal_event.rs` (NEW, 2 stubs) — Plan 11-10 `run_finished` terminal event.
- `tests/v11_run_detail_page_load.rs` (NEW, 4 stubs) — Plans 11-09/12 page-load backfill + URL compat.
- `tests/v11_startup_assertion.rs` (NEW, 2 stubs) — Plan 11-13 D-15 startup assertion.
- `tests/v11_log_dedupe_benchmark.rs` (NEW, 1 stub) — Plan 11-01 T-V11-LOG-02 benchmark gate.
- `tests/v11_log_id_plumbing.rs` (NEW, 3 stubs) — Plan 11-07 LogLine id plumbing (T-V11-LOG-01).
- `tests/v11_runnum_counter.rs` (NEW, 4 stubs) — Plan 11-05 per-job counter tests (DB-11).
- `tests/v11_log_dedupe_contract.rs` (NEW, 6 stubs) — Plans 11-11/12 D-09/D-10 contract tests.

## Decisions Made

1. **Fixture schema matches current initial migration exactly.** The plan's draft `seed_test_job` bound `cron, kind, command` — columns that do not exist in `migrations/sqlite/20260410_000000_initial.up.sql`. The plan's own action block flagged this with an explicit directive: "executor MUST grep the actual schema ... and match column names exactly." I bound `name, schedule, resolved_schedule, job_type, config_json, config_hash, timeout_secs, created_at, updated_at` verbatim. `enabled` is omitted because it has a DEFAULT.

2. **`#![allow(clippy::assertions_on_constants)]` crate-level attribute on every stub file.** `cargo clippy --tests -- -D warnings` rejects `assert!(true, ...)` as an always-true assertion. I preserved the plan's prescribed stub body pattern (`assert!(true, "stub — see Plan XX")`) because the plan-name-in-message is explicitly part of the hand-off contract and should survive verbatim through the downstream plans. A file-scoped allow is the minimal intrusion.

3. **Task 3 is a no-op.** The planner pre-populated `11-VALIDATION.md` with `nyquist_compliant: true`, `wave_0_complete: true`, `✅ W0` on every row, `- [x]` on every Wave 0 Requirements checklist line, and `approved 2026-04-16` on the sign-off. Verified via `grep -c "❌ W0"` = 0 before starting. No edit needed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixture schema mismatch with actual initial migration**
- **Found during:** Task 1 (v11_fixtures.rs creation)
- **Issue:** The plan's draft `seed_test_job` INSERT referenced columns `cron`, `kind`, `command` which don't exist in `migrations/sqlite/20260410_000000_initial.up.sql`. Real columns are `name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, timeout_secs, created_at, updated_at`.
- **Fix:** Rewrote the INSERT to bind the correct NOT-NULL columns: `(name, schedule, resolved_schedule, job_type, config_json, config_hash, timeout_secs, created_at, updated_at) VALUES (?1, '* * * * *', '* * * * *', 'command', '{}', '0', 60, ?2, ?2)`.
- **Files modified:** `tests/common/v11_fixtures.rs`
- **Verification:** `cargo check --tests` clean; fixture only referenced from stubs that are #[ignore]d so real execution deferred to downstream plans — but compile-time reference to `cronduit::db::queries::PoolRef` + `cronduit::db::queries::insert_running_run` validates the API surface is correct.
- **Committed in:** `fa26618` (Task 1).
- **Note:** The plan explicitly anticipated this — its closing note in Task 1 directed the executor to grep the actual schema and match it.

**2. [Rule 3 - Blocking] Clippy rejects `assert!(true)` under `-D warnings`**
- **Found during:** Task 2 (first harness file clippy check)
- **Issue:** `clippy::assertions_on_constants` flags `assert!(true, "...")` as always-true. Plan's stub bodies use this pattern to embed ownership in the panic message; removing the pattern would break the hand-off contract.
- **Fix:** Added `#![allow(clippy::assertions_on_constants)]` at the top of each of the 10 harness files. Kept stub bodies exactly as the plan specified.
- **Files modified:** All 10 `tests/v11_*.rs` harness files.
- **Verification:** `cargo clippy --tests -- -D warnings` passes cleanly.
- **Committed in:** `783e9ca` (Task 2).

**3. [Rule 1 - Bug] `seed_null_runs` cannot bind non-existent `job_run_number` column**
- **Found during:** Task 1 (v11_fixtures.rs creation)
- **Issue:** Plan's draft `seed_null_runs` INSERT bound `job_run_number = NULL` explicitly. That column does not exist until Plan 11-02 adds it. Including it in a Wave-0 fixture would fail to compile against today's schema.
- **Fix:** Removed the `job_run_number` column from the INSERT. Comment in the fixture explains that once Plan 11-02 lands the nullable column, downstream tests (Plan 11-03 backfill) update the column via targeted UPDATEs rather than relying on the fixture to bind it at insert time.
- **Files modified:** `tests/common/v11_fixtures.rs`
- **Verification:** `cargo check --tests` clean.
- **Committed in:** `fa26618` (Task 1).

---

**Total deviations:** 3 auto-fixed (2 Rule 1 bugs — schema mismatches against current code; 1 Rule 3 blocker — clippy policy conflict).
**Impact on plan:** All three fixes are correctness-preserving and small-scoped. The plan's Task-1 close explicitly anticipated Deviation #1 (schema-grep directive), and Deviation #2 preserves the plan's semantic choice of embedding ownership in assertion messages. No scope creep. No task names renamed.

## Issues Encountered

None beyond the three auto-fixed deviations above.

## TDD Gate Compliance

Not applicable — this plan is scaffolding only. Owning plans execute TDD cycles when they swap stubs for real bodies.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Wave 1 (Plan 11-01) unblocked — can now open `tests/v11_log_dedupe_benchmark.rs`, remove `#[ignore]` from `p95_under_50ms`, replace the `assert!(true)` body with the real benchmark, and commit.
- Every downstream Phase-11 plan's "create test file" step becomes "edit existing file + remove `#[ignore]`" per plan design.
- VALIDATION.md Per-Task Verification Map rows remain `⬜ pending` — owning plans flip these to `✅ green` as they land real bodies.

## Self-Check: PASSED

**Files verified on disk:**
- tests/common/v11_fixtures.rs — FOUND
- tests/common/mod.rs — FOUND
- tests/v11_runnum_migration.rs — FOUND
- tests/v11_run_now_sync_insert.rs — FOUND
- tests/v11_sse_log_stream.rs — FOUND
- tests/v11_sse_terminal_event.rs — FOUND
- tests/v11_run_detail_page_load.rs — FOUND
- tests/v11_startup_assertion.rs — FOUND
- tests/v11_log_dedupe_benchmark.rs — FOUND
- tests/v11_log_id_plumbing.rs — FOUND
- tests/v11_runnum_counter.rs — FOUND
- tests/v11_log_dedupe_contract.rs — FOUND

**Commits verified:**
- fa26618 — FOUND (Task 1 fixtures)
- 783e9ca — FOUND (Task 2 harness stubs)

**Build gates verified:**
- `cargo check --tests` — CLEAN (0 errors, 0 warnings)
- `cargo clippy --tests -- -D warnings` — CLEAN
- `cargo test --tests` — 0 failed across all test binaries; all v11 stubs ignored as expected

---
*Phase: 11-per-job-run-numbers-log-ux-fixes*
*Completed: 2026-04-16*
