---
phase: 11-per-job-run-numbers-log-ux-fixes
plan: 02
subsystem: database
tags: [rust, sqlx, sqlite, postgres, migration, phase-11, db-09, db-10, schema-parity]

# Dependency graph
requires:
  - phase: 11-00
    provides: tests/v11_runnum_migration.rs Wave-0 #[ignore] stubs + tests/common/v11_fixtures.rs (setup_sqlite_with_phase11_migrations + PoolRef helpers) consumed by migration_01_* bodies.
  - phase: 11-01
    provides: cleared D-02 decision gate (Option A insert-then-broadcast path retained) — Plan 11-02 proceeds with schema additions that the Option A path will read/write.
provides:
  - migrations/sqlite/20260416_000001_job_run_number_add.up.sql — file 1 of 3 adding nullable job_runs.job_run_number + NOT NULL jobs.next_run_number DEFAULT 1.
  - migrations/postgres/20260416_000001_job_run_number_add.up.sql — Postgres counterpart using BIGINT (matches SQLite INTEGER under schema_parity INT64 normalization).
  - tests/v11_runnum_migration.rs migration_01_add_nullable_columns + migration_01_idempotent real bodies (replacing Plan 11-00 Wave-0 stubs); migration_02_* and migration_03_* remain #[ignore] for Plans 11-03 and 11-04.
affects: [11-03, 11-04, 11-05, 11-06, 11-07, 11-09, 11-10, 11-11, 11-12, 11-13]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Split-migration pattern (DB-10): add nullable column in file 1, backfill data in file 2 (Plan 11-03), tighten NOT NULL in file 3 (Plan 11-04). Never combine file 1 and file 3 — crash mid-backfill would leave rows stuck at NULL against a NOT NULL constraint."
    - "SQLite/Postgres paired migration parity: every structural change lands in both migrations/sqlite/*.up.sql and migrations/postgres/*.up.sql in the same commit and is validated by tests/schema_parity.rs (normalize_type collapses INTEGER + BIGINT + BIGSERIAL to INT64)."
    - "sqlx _sqlx_migrations tracker is the source of truth for idempotency on SQLite — ALTER TABLE ... ADD COLUMN has no IF NOT EXISTS in SQLite; re-apply is prevented by sqlx refusing to re-run recorded migrations, not by SQL-level guards."
    - "Postgres ADD COLUMN IF NOT EXISTS (9.6+) is used as belt-and-suspenders on top of sqlx tracker — cost-free, survives a hypothetical manual re-run of the .up.sql file."

key-files:
  created:
    - migrations/sqlite/20260416_000001_job_run_number_add.up.sql
    - migrations/postgres/20260416_000001_job_run_number_add.up.sql
    - .planning/phases/11-per-job-run-numbers-log-ux-fixes/11-02-SUMMARY.md
  modified:
    - tests/v11_runnum_migration.rs

key-decisions:
  - "SQLite file uses plain ALTER TABLE ADD COLUMN (no IF NOT EXISTS guard) because SQLite does not support that syntax — relying on sqlx's _sqlx_migrations table for idempotency per the plan's explicit directive."
  - "Postgres file uses ADD COLUMN IF NOT EXISTS (9.6+) as belt-and-suspenders. Zero cost; protects against a hypothetical manual re-run."
  - "BIGINT on Postgres matches SQLite's INTEGER under the schema_parity normalize_type INT64 rule. Using Postgres INTEGER (32-bit) would silently corrupt past 2.1B runs and break parity. Verified via green schema_parity test with both migrations applied."
  - "Reverted cosmetic assets/static/app.css re-minification from build.rs (same deviation as Plan 11-01 observed). Kept commit scope pure."

requirements-completed: [DB-09, DB-10]

# Metrics
duration: ~4min
completed: 2026-04-17
---

# Phase 11 Plan 02: File-1 Migration (Nullable job_run_number + next_run_number Counter) Summary

**SQLite and Postgres migration files land the first of three paired files for DB-09/DB-10/DB-11: adds `job_runs.job_run_number` (nullable) and `jobs.next_run_number` (NOT NULL DEFAULT 1); schema-parity validated against both backends; migration_01_add_nullable_columns + migration_01_idempotent tests pass.**

## Performance

- **Duration:** ~4 min
- **Started:** 2026-04-17T00:13:37Z
- **Completed:** 2026-04-17T00:17:45Z (approx.)
- **Tasks:** 3
- **Files created:** 3 (2 migration files + this SUMMARY.md)
- **Files modified:** 1 (tests/v11_runnum_migration.rs — stubs swapped for real bodies)

## Accomplishments

- File 1 of the DB-10 split migration landed on both backends. Pure additive ALTER TABLE ADD COLUMN on each — no structural rewrites, no data movement, no lock escalation beyond Postgres 11+'s metadata-only ADD COLUMN fast path.
- BIGINT on Postgres chosen deliberately to match SQLite's INTEGER under the parity test's INT64 normalization. Verified by running tests/schema_parity.rs against both backends with the new files applied — PASS.
- Wave-0 stubs for `migration_01_add_nullable_columns` and `migration_01_idempotent` swapped for real PRAGMA-based shape assertions. Plans 11-03 (migration_02_*) and 11-04 (migration_03_*) stubs untouched, carrying forward `#[ignore]` with plan-name ownership annotations.
- `cargo test --test v11_runnum_migration migration_01` prints `2 passed; 0 failed; 7 ignored` — the exact pre/post state this plan is supposed to produce.
- `cargo clippy --tests -- -D warnings` clean.
- `cargo test --test schema_parity` clean (Postgres testcontainer + SQLite both migrated, all columns + indexes aligned).

## PRAGMA Outputs (Post-Migration, SQLite In-Memory)

```
jobs.next_run_number:      INTEGER  notnull=1  default='1'
job_runs.job_run_number:   INTEGER  notnull=0  default=NULL
```

Captured via a freshly-created on-disk SQLite DB after applying both initial (`20260410_000000_initial.up.sql`) and file-1 (`20260416_000001_job_run_number_add.up.sql`) migrations in sequence. Matches test expectations:

- `jobs.next_run_number`: INTEGER, NOT NULL, DEFAULT 1 — ready to seed the per-job counter at row creation without needing an explicit INSERT value.
- `job_runs.job_run_number`: INTEGER, nullable, no default — waiting for Plan 11-03's backfill + Plan 11-04's NOT NULL tightening.

## Task Commits

Each task committed atomically:

1. **Task 1: SQLite migration file 1 — add nullable job_run_number + next_run_number counter** — `b2b3bff` (feat)
2. **Task 2: Postgres migration file 1 — same schema intent, BIGINT types** — `ae149bb` (feat)
3. **Task 3: tests/v11_runnum_migration.rs — swap Wave-0 stubs for real file-1 assertions** — `c9c1494` (test)

## Files Created/Modified

- `migrations/sqlite/20260416_000001_job_run_number_add.up.sql` (NEW, 18 lines) — two ALTER TABLE statements + DB-09/DB-10/DB-11 header comment referencing the Postgres counterpart and the schema_parity invariant.
- `migrations/postgres/20260416_000001_job_run_number_add.up.sql` (NEW, 14 lines) — same intent with BIGINT types and `IF NOT EXISTS` guards. Header comment references the SQLite counterpart and the schema_parity INT64 rule.
- `tests/v11_runnum_migration.rs` (MODIFIED) — removed `#[ignore]` from `migration_01_add_nullable_columns` + `migration_01_idempotent`; replaced their `assert!(true, ...)` stub bodies with real PRAGMA-table-info assertions. Added `use common::v11_fixtures::setup_sqlite_with_phase11_migrations` and `use cronduit::db::queries::PoolRef` imports. Every `migration_02_*` and `migration_03_*` stub left untouched with its plan-ownership `#[ignore]` + `assert!(true)` body.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-02-SUMMARY.md` (NEW) — this file.

## Decisions Made

1. **SQLite file uses unguarded ALTER TABLE ADD COLUMN.** The plan explicitly directs against `IF NOT EXISTS` or `PRAGMA user_version` guards on SQLite: SQLite doesn't support `ADD COLUMN IF NOT EXISTS`, and layering manual guards complicates partial-crash recovery without adding safety on top of sqlx's `_sqlx_migrations` tracker. Retained the plan's specified text verbatim.

2. **Postgres file uses `ADD COLUMN IF NOT EXISTS`.** Postgres supports this since 9.6 — it's a free belt-and-suspenders guard that survives a hypothetical manual re-run of the `.up.sql` file. Retained the plan's specified text verbatim.

3. **BIGINT on Postgres, INTEGER on SQLite.** BIGINT on Postgres is necessary so `schema_parity::normalize_type` collapses both to INT64 — using Postgres INTEGER (32-bit) would have broken the parity test and silently corrupted the counter past 2.1B runs. (Cronduit won't realistically hit that, but the parity invariant matters more.)

4. **Reverted cosmetic `assets/static/app.css` regeneration** from the build.rs Tailwind step (same thing Plan 11-01 observed). Kept the test commit scope to `tests/v11_runnum_migration.rs` only. Any developer running `just tailwind` regenerates the identical output.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] sqlx test binary cached old migration list — required `touch src/db/mod.rs`**
- **Found during:** Task 3 first test run
- **Issue:** `cargo test --test v11_runnum_migration migration_01` reported failures even though migration files were already on disk and committed. Root cause: `sqlx::migrate!("./migrations/sqlite")` is a compile-time macro; the test binary had a stale embedded migration list from a previous compilation. `cargo` didn't detect that the migrations directory had new files because the test binary's direct source deps (`src/db/mod.rs`) hadn't changed.
- **Fix:** `touch src/db/mod.rs` to force a rebuild of the dep that calls the macro. After rebuild both tests passed (`2 passed; 0 failed; 7 ignored`).
- **Files modified:** None committed (touch only invalidates the cargo cache).
- **Verification:** `cargo test --test v11_runnum_migration migration_01` passes.
- **Committed in:** N/A — no lasting changes.
- **Note for Plans 11-03 and 11-04:** When adding files 2 and 3, the same workaround applies if a test run sees stale migration state. `touch src/db/mod.rs` (or any file the test binary's dep chain includes) forces sqlx's compile-time macro to re-embed the updated migration set.

**2. [Rule 3 - Blocking] rustfmt disagreed with the plan's draft test formatting**
- **Found during:** Task 3 `cargo fmt --check`
- **Issue:** The plan's draft body for `migration_01_add_nullable_columns` has the first `sqlx::query_as` call split across multiple lines with argument on its own line, but rustfmt reformats it to put the SQL string inline on the same line as the call. Without reformatting, `cargo fmt --check` fails and a CI gate would reject.
- **Fix:** Ran `cargo fmt -- tests/v11_runnum_migration.rs`. The reformat is purely cosmetic — same assertions, same imports, same test bodies. Re-ran tests post-reformat to confirm `2 passed; 0 failed; 7 ignored`.
- **Files modified:** `tests/v11_runnum_migration.rs` (final reformatted version).
- **Verification:** `cargo fmt --check -- tests/v11_runnum_migration.rs` clean; both file-1 tests still green.
- **Committed in:** `c9c1494` (Task 3 commit).

---

**Total deviations:** 2 auto-fixed (both Rule 3 - Blocking).
**Impact on plan:** Both deviations are pure tooling/infra glitches — neither changed semantics, scope, or the plan's decision boundary. Migration file content and test assertion logic are exactly as the plan specified.

## Threat Flags

None — File 1 is pure DDL (`ALTER TABLE ADD COLUMN`) with literal constants. No new network endpoints, no auth paths, no file-access patterns, no schema changes at trust boundaries beyond what the plan's `<threat_model>` already documented (T-11-02-01 mitigated by static-DDL-only constraint; T-11-02-02 accepted because migration runs pre-listener per D-12).

## Issues Encountered

None beyond the two auto-fixed deviations above.

## TDD Gate Compliance

The plan has `tdd="true"` on Task 3, and Phase 11's adopted pattern (per Plan 11-00 Wave-0 stub convention and Plan 11-01 precedent) treats the Wave-0 `#[ignore]` stub as the RED gate.

- **RED:** `fa26618` + `783e9ca` (Plan 11-00 landing the `#[ignore]` stubs for migration_01_*).
- **GREEN:** `b2b3bff` + `ae149bb` (Tasks 1 & 2 landed the migration files that make the real assertions in Task 3 satisfiable) + `c9c1494` (Task 3 swapped stub bodies for real assertions, which pass).
- **REFACTOR:** none required.

Git-log verification:
- `test(...)` commit in history — Yes (`c9c1494` on this plan, plus `783e9ca` upstream from Plan 11-00).
- `feat(...)` commit after `test(...)` on this plan — Yes (`b2b3bff`, `ae149bb` precede `c9c1494`, satisfying the gate since the RED state is already upstream of this plan).

This matches the phase design pattern.

## User Setup Required

None — no external service configuration required. Downstream plans pick up the new schema via in-memory SQLite + testcontainers-backed Postgres as before.

## Next Phase Readiness

- **Plan 11-03 unblocked.** The `job_run_number` column is now nullable and present on both backends. Plan 11-03 authors file 2 (`20260416_000002_job_run_number_backfill.up.sql`) containing the `UPDATE ... FROM (SELECT ... ROW_NUMBER() OVER (...))` backfill scoped by `WHERE job_run_number IS NULL`, exposes progress logging per DB-10, and lands the `migration_02_backfill_completes` + `migration_02_logs_progress` + `migration_02_resume_after_crash` + `migration_02_counter_reseed` + `migration_02_row_number_order_by_id` tests.
- **Plan 11-04 unblocked.** Once Plan 11-03 lands the backfill, Plan 11-04 authors file 3 (`20260416_000003_job_run_number_not_null.up.sql`) for the SQLite table rewrite and Postgres ALTER COLUMN SET NOT NULL, plus the `migration_03_*` test bodies.
- **Plan 11-05 (counter tests) unblocked.** `jobs.next_run_number` counter column is live; Plan 11-05 can now land the `insert_running_run` two-statement transaction tests against the new schema.

## Self-Check: PASSED

**Files verified on disk:**
- migrations/sqlite/20260416_000001_job_run_number_add.up.sql — FOUND
- migrations/postgres/20260416_000001_job_run_number_add.up.sql — FOUND
- tests/v11_runnum_migration.rs — FOUND (modified)
- .planning/phases/11-per-job-run-numbers-log-ux-fixes/11-02-SUMMARY.md — FOUND (this file)

**Commits verified:**
- b2b3bff — FOUND (Task 1 SQLite migration)
- ae149bb — FOUND (Task 2 Postgres migration)
- c9c1494 — FOUND (Task 3 test bodies)

**Build gates verified:**
- `cargo test --test v11_runnum_migration migration_01` — PASS (`2 passed; 0 failed; 7 ignored`)
- `cargo test --test v11_runnum_migration` — PASS (`2 passed; 0 failed; 7 ignored`) — confirms migration_02_* and migration_03_* remain `#[ignore]`
- `cargo clippy --tests -- -D warnings` — CLEAN
- `cargo fmt --check -- tests/v11_runnum_migration.rs` — CLEAN
- `cargo test --test schema_parity` — PASS (Postgres testcontainer + SQLite schemas aligned with both new files applied)

**Plan success criteria verified:**
- Both migration files exist with correct ALTER statements (INTEGER on SQLite, BIGINT on Postgres).
- `migration_01_add_nullable_columns` and `migration_01_idempotent` pass.
- `schema_parity` test continues to pass — no structural drift.

---
*Phase: 11-per-job-run-numbers-log-ux-fixes*
*Completed: 2026-04-17*
