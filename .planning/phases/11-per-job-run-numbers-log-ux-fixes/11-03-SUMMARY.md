---
phase: 11-per-job-run-numbers-log-ux-fixes
plan: 03
subsystem: database
tags: [rust, sqlx, sqlite, postgres, migration, backfill, phase-11, db-09, db-10, db-11, db-12]

# Dependency graph
requires:
  - phase: 11-02
    provides: job_runs.job_run_number (nullable) + jobs.next_run_number (NOT NULL DEFAULT 1) on both backends; migration_01_* tests green; tests/common/v11_fixtures.rs seed_null_runs helper.
provides:
  - migrations/sqlite/20260417_000002_job_run_number_backfill.up.sql — marker file 2 of 3 (SELECT 1 stub) so sqlx-tracker records file 2 as applied.
  - migrations/postgres/20260417_000002_job_run_number_backfill.up.sql — Postgres counterpart.
  - src/db/migrate_backfill.rs — NEW module with `backfill_job_run_number(pool)` orchestrator; O(1) sentinel-table fast-path on re-runs, chunked 10k-row loop with per-batch INFO logging, resync of jobs.next_run_number, final sentinel-table marker.
  - src/db/queries.rs (MODIFIED) — five new pub async fn helpers: backfill_job_run_number_batch, resync_next_run_number, count_job_runs_with_null_run_number, v11_backfill_sentinel_exists, v11_backfill_sentinel_mark_done.
  - src/db/mod.rs (MODIFIED) — DbPool::migrate now calls migrate_backfill::backfill_job_run_number after sqlx::migrate! completes (Plan 11-04 adds the second pass after file 3 lands).
  - tests/v11_runnum_migration.rs (MODIFIED) — five migration_02_* bodies land; migration_03_* stubs untouched for Plan 11-04.
affects: [11-04, 11-05, 11-06, 11-09, 11-13]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Chunked Rust-side migration orchestrator (DB-12): sqlx::migrate! applies the static marker file, then a Rust fn loops 10k-row batches of UPDATE ... FROM (SELECT ROW_NUMBER()...) with per-batch INFO progress log on target cronduit.migrate, and finally writes a sentinel table for O(1) re-run idempotency."
    - "Per-job resume offset via LEFT JOIN on MAX(job_run_number): each batch's ROW_NUMBER() is added to COALESCE((SELECT MAX over already-filled rows for this job), 0) so partial-crash restarts continue the sequence contiguously with no double-counting."
    - "SQLite 3.33+ UPDATE ... FROM form matches the Postgres arm and dodges SQLite's WHERE-pushdown optimization that breaks correlated-subquery ROW_NUMBER() by shrinking each partition to a single row."
    - "sqlx::migrate! version parsing uses splitn(2, '_') — VERSION is everything before the FIRST underscore. Each migration file MUST have a distinct first-segment integer prefix; two files with the same date prefix (e.g. 20260416_000001_* + 20260416_000002_*) collide on _sqlx_migrations.version UNIQUE."
    - "Tracing capture pattern for integration tests: CapturedWriter: MakeWriter<'a> pushing into Arc<Mutex<Vec<u8>>>, subscriber built with .with_ansi(false), future wrapped via WithSubscriber so cross-await events are captured without process-wide dispatcher install."

key-files:
  created:
    - migrations/sqlite/20260417_000002_job_run_number_backfill.up.sql
    - migrations/postgres/20260417_000002_job_run_number_backfill.up.sql
    - src/db/migrate_backfill.rs
    - .planning/phases/11-per-job-run-numbers-log-ux-fixes/11-03-SUMMARY.md
  modified:
    - src/db/queries.rs
    - src/db/mod.rs
    - tests/v11_runnum_migration.rs

key-decisions:
  - "Renamed file-2 markers from 20260416_000002_* to 20260417_000002_* so each migration has a distinct VERSION prefix (first segment before `_`). sqlx splits on splitn(2, '_'); co-date-prefix with Plan 11-02's 20260416_000001_* would collide on _sqlx_migrations.version UNIQUE constraint."
  - "Switched SQLite backfill from the planned correlated scalar subquery form to the UPDATE ... FROM form (SQLite 3.33+) matching the Postgres arm. SQLite's optimizer pushes the outer s.id = job_runs.id equality INTO the inner subquery, shrinking each ROW_NUMBER() partition to a single row (rn always = 1). UPDATE ... FROM materializes the derived table BEFORE the join, preserving partition scope."
  - "Added LEFT JOIN on prev(job_id, max_filled) + `s.rn + COALESCE(prev.max_filled, 0)` expression so a partial-crash restart continues the per-job sequence contiguously. Without it, a second batch on 10k remaining NULL rows would re-number from 1 and collide with the first batch's 1..10k assignments."
  - "Manual ceiling divide `(total + BATCH_SIZE - 1) / BATCH_SIZE` instead of `i64::div_ceil` — the latter is still on the unstable `int_roundings` feature in Rust 1.94 (stable)."

requirements-completed: [DB-09, DB-10, DB-11, DB-12]

# Metrics
duration: ~23min
completed: 2026-04-17
---

# Phase 11 Plan 03: Chunked Rust-Orchestrated Backfill (File 2 of 3) Summary

**Rust-side `backfill_job_run_number` orchestrator loops 10k-row batches of ROW_NUMBER()-computed UPDATEs with D-13 INFO logging per batch, resyncs `jobs.next_run_number = MAX+1` per job, and marks a `_v11_backfill_done` sentinel table for O(1) re-run idempotency. Five migration_02_* tests cover backfill completion, progress logging, partial-crash resume, counter reseed, and id-ASC numbering.**

## Performance

- **Duration:** ~23 min
- **Started:** 2026-04-17T00:22:14Z
- **Completed:** 2026-04-17T00:45:45Z (approx.)
- **Tasks:** 4
- **Files created:** 4 (2 marker migration files + src/db/migrate_backfill.rs + this SUMMARY.md)
- **Files modified:** 3 (src/db/queries.rs, src/db/mod.rs, tests/v11_runnum_migration.rs)

## Accomplishments

- Marker files land on both backends with unique VERSION prefixes — sqlx-tracker records file 2 as applied without a UNIQUE-constraint collision against Plan 11-02's `20260416_000001_*`.
- `src/db/migrate_backfill.rs` (NEW) adds the chunked orchestrator with the exact step sequence the plan specified: sentinel fast-path → NULL count → chunked UPDATE loop with per-batch INFO log on target `cronduit.migrate` → resync counter → mark sentinel → final completion log.
- Five query helpers landed in `src/db/queries.rs` (`backfill_job_run_number_batch`, `resync_next_run_number`, `count_job_runs_with_null_run_number`, `v11_backfill_sentinel_exists`, `v11_backfill_sentinel_mark_done`) — all pair-tested across SQLite + Postgres code paths with static SQL only (T-11-03-01 threat mitigated).
- `DbPool::migrate` now invokes the orchestrator after `sqlx::migrate!` completes. Plan 11-04 will add the second `sqlx::migrate!` pass once file 3 lands.
- All five `migration_02_*` integration tests pass: backfill_completes, logs_progress, resume_after_crash, counter_reseed, row_number_order_by_id. Log-progress test captures tracing output via a `CapturedWriter: MakeWriter<'a>` + `WithSubscriber` dispatch and verifies the D-13 field shape (`batch=`, `rows_done=`, `rows_total=`, `elapsed_ms=`).
- Per-job resume offset (`s.rn + COALESCE(prev.max_filled, 0)`) makes partial-crash restarts safe: a second batch on remaining NULL rows continues the sequence contiguously instead of colliding with the first batch's 1..N assignments (verified by `migration_02_resume_after_crash`).

## Task Commits

Each task committed atomically:

1. **Task 1: Marker SQL files (file 2 stubs)** — `3279a43` (feat) + rename fix `60a7a1b` (fix) to avoid sqlx version collision.
2. **Task 2: queries.rs helpers** — `c6b74f4` (feat), later fixed for SQL correctness by `0faec8f` (fix) after migration_02_* tests exercised the bug.
3. **Task 3: src/db/migrate_backfill.rs orchestrator + DbPool::migrate wiring** — `44e0462` (feat).
4. **Task 4: migration_02_* test bodies** — `5c9ff9b` (test).

## Files Created/Modified

- `migrations/sqlite/20260417_000002_job_run_number_backfill.up.sql` (NEW, 16 lines) — SELECT 1 marker; header references DB-12 + `_v11_backfill_done` sentinel + pair with Postgres file.
- `migrations/postgres/20260417_000002_job_run_number_backfill.up.sql` (NEW, 7 lines) — same intent.
- `src/db/migrate_backfill.rs` (NEW, ~113 lines) — pub async fn `backfill_job_run_number(pool)` with sentinel fast-path, NULL count, chunked loop, resync, final sentinel-write, six structured INFO log lines (start + N batches + complete + the short-circuit/no-rows paths).
- `src/db/queries.rs` (MODIFIED) — five new pub async fn items at the end of the module (before `#[cfg(test)] mod tests`); total +208 lines minus fmt delta. Post-fix SQL uses `UPDATE ... FROM (SELECT ROW_NUMBER()) LEFT JOIN prev` form on both SQLite and Postgres.
- `src/db/mod.rs` (MODIFIED) — declares `pub mod migrate_backfill;` and calls `migrate_backfill::backfill_job_run_number(self).await?` in `migrate()` after `sqlx::migrate!` returns; docstring on `migrate()` updated to explain the phase-11 ordering invariants and the Plan 11-04 second-pass hook.
- `tests/v11_runnum_migration.rs` (MODIFIED) — removed `#[ignore]` + replaced five `assert!(true)` bodies with real assertions; added shared `reset_sentinel` helper + `CapturedWriter` struct + relevant imports. Module docstring updated: Plan 11-03 is now ACTIVE; Plan 11-02 → COMPLETE.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-03-SUMMARY.md` (NEW) — this file.

## Decisions Made

1. **Rename file-2 markers to `20260417_000002_*` (date bumped by one day).** sqlx 0.8 parses migration filenames with `splitn(2, '_')` — the VERSION (i64) is everything BEFORE the first underscore. Plan 11-02's file is `20260416_000001_*` (version `20260416`); the originally-proposed Plan 11-03 name `20260416_000002_*` would have parsed to the SAME version `20260416` and blown up `_sqlx_migrations.version UNIQUE`. Bumping the date prefix to `20260417_` gives file 2 version `20260417` — distinct i64, correct chronological sort, no collision.

2. **Switch SQLite from correlated-subquery form to `UPDATE ... FROM` form.** The plan prescribed a correlated scalar subquery for SQLite to avoid the (at-the-time assumed) unavailability of `UPDATE ... FROM` on SQLite. But SQLite's optimizer pushes the outer `WHERE s.id = job_runs.id` equality INTO the inner `SELECT ... WHERE job_run_number IS NULL`, which reduces the `ROW_NUMBER() OVER (PARTITION BY job_id)` computation to a single-row partition (rn = 1 for every row). `UPDATE ... FROM (derived table)` is supported on SQLite 3.33+ (released 2020-08) and matches the Postgres arm exactly; both execute the window function over the full NULL set before joining.

3. **Add per-job offset via `LEFT JOIN prev(job_id, max_filled)`.** Without it, the `WHERE job_run_number IS NULL` guard that makes the query idempotent has a secondary effect: after a partial-crash first batch fills 10k rows, the second batch's ROW_NUMBER() sees only the remaining 10k NULL rows and numbers them 1..10k — colliding directly with the first batch's 1..10k. `SET job_run_number = s.rn + COALESCE(prev.max_filled, 0)` gives each batch the correct offset so 1..20000 stays contiguous across any number of restarts.

4. **`.with_ansi(false)` on the tracing test subscriber + manual `batch=` substring checks.** Default `tracing_subscriber::fmt()` emits ANSI escape codes between the field name and `=` (e.g. `batch\x1b[0m\x1b[2m=\x1b[0m1`), which makes `output.contains("batch=")` fail even when the field is semantically present. Disabling ANSI gives us plain `batch=1` text and lets the substring assertions succeed.

5. **Manual ceiling divide `(total + BATCH_SIZE - 1) / BATCH_SIZE` instead of `i64::div_ceil(BATCH_SIZE)`.** `div_ceil` is still gated behind the unstable `int_roundings` feature on Rust 1.94 (the project's pinned stable toolchain). The open-coded expression is semantically identical, compiles on stable, and matches the pattern the plan originally specified.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] sqlx version collision between Plan 11-02's file-1 and Plan 11-03's file-2**
- **Found during:** Task 1 verification (after committing the markers)
- **Issue:** Plan 11-03's specified filename `20260416_000002_job_run_number_backfill.up.sql` shared a VERSION prefix with Plan 11-02's `20260416_000001_job_run_number_add.up.sql` under sqlx's `splitn(2, '_')` rule. Running `pool.migrate()` twice (or on a DB where any prior application tried to insert both) triggered `UNIQUE constraint failed: _sqlx_migrations.version`. All db-tests (lib `db::queries::tests::*`, integration `v11_runnum_migration::migration_01_*`, etc.) broke.
- **Fix:** `git mv` both files to `20260417_000002_*`. Distinct VERSION → UNIQUE constraint satisfied; chronological order preserved; no change needed to Plan 11-02's filename.
- **Files modified:** migrations/sqlite/20260417_000002_job_run_number_backfill.up.sql, migrations/postgres/20260417_000002_job_run_number_backfill.up.sql (rename).
- **Verification:** `cargo test --lib db::` → 25/25 pass; `cargo test --test v11_runnum_migration` → 7/7 pass + 2 ignored.
- **Committed in:** `60a7a1b`.
- **Heads-up for Plan 11-04:** The file-3 marker MUST use a third distinct VERSION prefix (e.g. `20260418_000003_job_run_number_not_null.up.sql`). Same date-bump pattern.

**2. [Rule 1 - Bug] SQLite correlated-subquery ROW_NUMBER broken by optimizer pushdown**
- **Found during:** Task 4 first run of `migration_02_backfill_completes`
- **Issue:** Plan-prescribed SQLite SQL:
  ```sql
  UPDATE job_runs SET job_run_number = (
    SELECT rn FROM (SELECT id, ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id ASC) AS rn
                    FROM job_runs WHERE job_run_number IS NULL) s
    WHERE s.id = job_runs.id)
  WHERE id IN (SELECT id FROM job_runs WHERE job_run_number IS NULL ORDER BY id ASC LIMIT ?1)
  ```
  SQLite's optimizer pushed `s.id = job_runs.id` INTO the inner subquery's WHERE clause, reducing each partition to a single row. Every row got job_run_number = 1, so the first 10 rows for job-one all received 1.
- **Fix:** Use SQLite 3.33+ `UPDATE ... FROM (derived table)` form (same shape as Postgres arm). The derived table is materialized first, THEN joined — partition scope is preserved.
- **Files modified:** src/db/queries.rs (backfill_job_run_number_batch SQLite arm).
- **Verification:** `migration_02_backfill_completes` passes with per-job numbering 1..10 / 1..8 / 1..7 exactly.
- **Committed in:** `0faec8f`.

**3. [Rule 1 - Bug] Partial-crash restart re-numbers from 1 — second batch collides with first**
- **Found during:** Task 4 run of `migration_02_resume_after_crash`
- **Issue:** After the fix in deviation #2, `migration_02_backfill_completes` passed but `migration_02_resume_after_crash` (20000 rows in a single job, two batches) failed at index 10000 (expected 10001, got 1). Root cause: each batch's ROW_NUMBER() scans only rows where `job_run_number IS NULL`, so after the first batch fills 1..10000 the second batch sees only the remaining 10k NULLs and numbers them 1..10k — overwriting-friendly if the first-batch values weren't already in the column, but guaranteed to produce duplicate `(job_id, job_run_number)` pairs because the first batch IS persisted.
- **Fix:** Add a LEFT JOIN to a subquery computing `MAX(job_run_number)` per job over already-filled rows, then `SET job_run_number = s.rn + COALESCE(prev.max_filled, 0)`. Each batch gets the correct per-job offset. Postgres arm updated identically for symmetry and schema parity invariants.
- **Files modified:** src/db/queries.rs (both arms of backfill_job_run_number_batch).
- **Verification:** `migration_02_resume_after_crash` passes with `nums = 1..=20000` contiguous; `migration_02_backfill_completes` still green.
- **Committed in:** `0faec8f` (same commit as deviation #2 — both SQL corrections land together).

**4. [Rule 3 - Blocking] `i64::div_ceil` is still unstable on Rust 1.94**
- **Found during:** Task 3 first `cargo check`
- **Issue:** Initial draft of migrate_backfill.rs used `total.div_ceil(BATCH_SIZE)` to estimate batch count for the INFO log. Rust stable 1.94 (project pin) still gates this method behind `int_roundings` feature.
- **Fix:** Replace with manual `(total + BATCH_SIZE - 1) / BATCH_SIZE` + comment. Semantically identical; the plan's own draft code used this form.
- **Files modified:** src/db/migrate_backfill.rs.
- **Verification:** `cargo check --lib` clean.
- **Committed in:** `44e0462` (rolled into Task 3 commit, since the bug existed only in the uncommitted draft).

**5. [Rule 3 - Blocking] Default tracing_subscriber::fmt emits ANSI escapes that break substring checks**
- **Found during:** Task 4 run of `migration_02_logs_progress`
- **Issue:** Captured writer received `batch\x1b[0m\x1b[2m=\x1b[0m1` style lines (ANSI dim + reset around the `=`). Plain `output.contains("batch=")` returned false, but semantically the field IS present in every batch line.
- **Fix:** Add `.with_ansi(false)` to the test's `tracing_subscriber::fmt()` builder. Output becomes plain `batch=1`, `rows_done=10000`, etc.
- **Files modified:** tests/v11_runnum_migration.rs (CapturedWriter subscriber builder).
- **Verification:** `migration_02_logs_progress` passes with all four D-13 field assertions.
- **Committed in:** `5c9ff9b`.

---

**Total deviations:** 5 auto-fixed (3 Rule 1 — schema/SQL correctness bugs; 2 Rule 3 — tooling/infra blockers).
**Impact on plan:** All five are real correctness bugs that would have shipped without the test bodies to exercise them. The plan's Q3 RESOLVED note (portable `UPDATE ... WHERE id IN (SELECT ... LIMIT N)` form) was itself partially correct — that form IS portable for the basic shape, but the ROW_NUMBER() PARTITION + resume-offset correctness required the `UPDATE ... FROM (derived table) LEFT JOIN prev` form on both backends.

## Threat Flags

None beyond the plan's `<threat_model>`.
- T-11-03-01 (Tampering, backfill SQL): still mitigated — all static string literals; only `BATCH_SIZE` compile-time const is `.bind()`-parameterized.
- T-11-03-02 (DoS, writer pool): still accepted — backfill runs before HTTP listener binds per D-12.
- T-11-03-03 (Information Disclosure, tracing logs): still accepted — log shape contains only counts + durations + batch numbers; no job names, no row contents. ANSI fix is a test-only concern.
- T-11-03-04 (Tampering, sentinel table): still mitigated — CHECK (id = 1) constraint; re-running orchestrator is idempotent.

## Issues Encountered

None beyond the five auto-fixed deviations above.

## TDD Gate Compliance

Plan 11-03 has `tdd="true"` on Tasks 2, 3, and 4. Phase 11's adopted pattern (per Plan 11-00 Wave-0 stubs + Plans 11-01/11-02 precedent) treats the Wave-0 `#[ignore]` stubs as the RED gate.

- **RED:** Wave-0 stubs landed by Plan 11-00 (`fa26618` + `783e9ca`) — five `migration_02_*` stubs with `#[ignore = "Wave-0 stub — real body lands in Plan 11-03"]` + `assert!(true, "stub — see Plan 11-03")`.
- **GREEN:** Plan 11-03 Tasks 1–3 (`3279a43`, `60a7a1b`, `c6b74f4`, `0faec8f`, `44e0462`) produced the migrations + orchestrator + query helpers. Task 4 (`5c9ff9b`) swapped the stubs for real assertions that pass.
- **REFACTOR:** `cargo fmt` reformatted a single query-scalar chain in the test file (cosmetic only — same assertions, same shape). No standalone refactor commit.

Git-log verification:
- `test(...)` commit in history — Yes (`5c9ff9b` on this plan; upstream `783e9ca` from Plan 11-00).
- `feat(...)` commits in history — Yes (`3279a43`, `c6b74f4`, `44e0462`).
- `fix(...)` commits in history — Yes (`60a7a1b`, `0faec8f`).

## User Setup Required

None — no external service configuration required. Downstream plans pick up the new orchestrator via in-memory SQLite + testcontainers-backed Postgres as before.

## Next Phase Readiness

- **Plan 11-04 unblocked.** Once Plan 11-04 lands file 3 (NOT NULL tightening), it can:
  1. Add the file 3 marker at version `20260418_000003_*` (or similar distinct VERSION).
  2. Add a SECOND `sqlx::migrate!` call in `DbPool::migrate` AFTER the `migrate_backfill::backfill_job_run_number` call so file 3 runs only after every NULL has been filled.
  3. Implement migration_03_sqlite_table_rewrite + migration_03_sqlite_indexes_preserved + migration_03_postgres_not_null (integration-gated).
- **Plan 11-05 (counter tests) unblocked.** `jobs.next_run_number` counter is now resynced to MAX+1 per job after backfill; Plan 11-05 can land its insert_running_run two-statement transaction tests without worrying about stale counter values.
- **Plan 11-13 (startup assertion, D-15)** has the helper it needs: `queries::count_job_runs_with_null_run_number(&pool)` returns the exact i64 the startup assertion will check against zero.

## Self-Check: PASSED

**Files verified on disk:**
- migrations/sqlite/20260417_000002_job_run_number_backfill.up.sql — FOUND
- migrations/postgres/20260417_000002_job_run_number_backfill.up.sql — FOUND
- src/db/migrate_backfill.rs — FOUND
- src/db/queries.rs — FOUND (modified; +5 pub async fn)
- src/db/mod.rs — FOUND (modified; pub mod migrate_backfill + migrate() wiring)
- tests/v11_runnum_migration.rs — FOUND (modified; migration_02_* bodies)
- .planning/phases/11-per-job-run-numbers-log-ux-fixes/11-03-SUMMARY.md — FOUND (this file)

**Commits verified:**
- 3279a43 — FOUND (Task 1 marker migrations)
- 60a7a1b — FOUND (Task 1 rename fix)
- c6b74f4 — FOUND (Task 2 query helpers)
- 44e0462 — FOUND (Task 3 orchestrator + wiring)
- 0faec8f — FOUND (SQL correctness fixes for deviations #2 + #3)
- 5c9ff9b — FOUND (Task 4 test bodies)

**Build gates verified:**
- `cargo test --test v11_runnum_migration` — PASS (`7 passed; 0 failed; 2 ignored`) — all 5 migration_02_* tests green; migration_03_* remain `#[ignore]` for Plan 11-04.
- `cargo test --test schema_parity` — PASS (`3 passed`) — Postgres testcontainer + SQLite schemas still aligned with all three file-2 markers applied.
- `cargo test --lib db::` — PASS (`25 passed; 0 failed`).
- `cargo clippy --tests -- -D warnings` — CLEAN.
- `cargo fmt --check` — CLEAN.

**Plan success criteria verified:**
1. Both marker `.up.sql` files exist (renamed to `20260417_` version); `sqlx::migrate!` records file 2 as applied without UNIQUE collision — ✅.
2. `src/db/migrate_backfill.rs` exists with `backfill_job_run_number` orchestrator + sentinel guard — ✅.
3. All five query helpers exported from `queries` module — ✅.
4. `DbPool::migrate` calls the orchestrator after `sqlx::migrate!` completes — ✅ (Plan 11-04 will add the second pass).
5. All five migration_02_* tests pass — ✅.
6. Sentinel table `_v11_backfill_done` exists after successful backfill and makes re-runs O(1) — ✅ (verified in `migration_02_resume_after_crash` + `migration_02_backfill_completes` flows).

---
*Phase: 11-per-job-run-numbers-log-ux-fixes*
*Completed: 2026-04-17*
