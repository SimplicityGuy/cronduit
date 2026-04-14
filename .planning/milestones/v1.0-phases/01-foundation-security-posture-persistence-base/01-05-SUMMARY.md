---
phase: 01-foundation-security-posture-persistence-base
plan: 05
subsystem: testing
tags: [testcontainers, postgres, sqlite, schema-parity, sqlx, integration-tests]

requires:
  - phase: 01-foundation-security-posture-persistence-base/01
    provides: "Cargo.toml with testcontainers + testcontainers-modules dev-deps"
  - phase: 01-foundation-security-posture-persistence-base/04
    provides: "Migration files (sqlite + postgres) and DbPool abstraction"
provides:
  - "Schema parity integration test (tests/schema_parity.rs) with type normalization whitelist"
  - "Postgres DbPool smoke test (tests/db_pool_postgres.rs) proving connect + migrate end-to-end"
affects: [01-06, 01-07]

tech-stack:
  added: []
  patterns:
    - "testcontainers AsyncRunner::start() for Postgres containers in tests"
    - "Type normalization whitelist pattern for cross-backend schema comparison"
    - "SQLite PRAGMA table_info + pk column for NOT NULL inference"

key-files:
  created:
    - tests/schema_parity.rs
    - tests/db_pool_postgres.rs
  modified:
    - migrations/postgres/20260410_000000_initial.up.sql

key-decisions:
  - "Fixed Postgres migration types (SMALLINT->BIGINT for enabled, INTEGER->BIGINT for exit_code) to achieve true structural parity with SQLite INTEGER columns"
  - "SQLite INTEGER PRIMARY KEY notnull quirk handled via pk column check rather than hardcoding column names"

patterns-established:
  - "normalize_type whitelist: unknown types panic with instructions to add to whitelist — reviewer gate"
  - "Skip Postgres auto-created indexes (_pkey, _name_key) and SQLite autoindexes during comparison"

requirements-completed: [DB-04]

duration: 8min
completed: 2026-04-10
---

# Phase 1 Plan 05: Schema Parity + Postgres Integration Tests Summary

**SQLite/Postgres structural parity test with type normalization whitelist, plus DbPool Postgres smoke test via testcontainers**

## Performance

- **Duration:** 8 min
- **Started:** 2026-04-10T04:59:43Z
- **Completed:** 2026-04-10T05:07:38Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Schema parity test introspects both backends after migration, normalizes types through a whitelist, and produces structured diffs on drift
- Unknown column types cause a panic with explicit instructions to extend the whitelist — reviewer gate for any schema change
- DbPool Postgres smoke test proves connect + migrate works end-to-end against a real container
- Idempotent migration verified (migrate called twice in both tests)

## Task Commits

Each task was committed atomically:

1. **Task 1: Schema parity harness with introspection, normalization, and structured diff** - `7c1f458` (test)
2. **Task 2: DbPool Postgres smoke test via testcontainers** - `7bd91bd` (test)

## Files Created/Modified
- `tests/schema_parity.rs` - Full parity test: SQLite in-memory + testcontainers Postgres, type normalization whitelist, structured diff on failure
- `tests/db_pool_postgres.rs` - DbPool::connect + migrate smoke test against real Postgres
- `migrations/postgres/20260410_000000_initial.up.sql` - Fixed enabled (SMALLINT->BIGINT) and exit_code (INTEGER->BIGINT) for parity with SQLite INTEGER

## normalize_type Whitelist (final)

| Raw type(s) | Normalized token | Justification |
|---|---|---|
| INTEGER, BIGINT, BIGSERIAL, INT8 | INT64 | SQLite INTEGER = Postgres BIGINT = i64 in sqlx |
| SMALLINT, INT2 | INT16 | Currently unused after migration fix; kept for future use |
| INT, INT4 | INT32 | Postgres 4-byte integer if ever needed |
| TEXT, VARCHAR, CHARACTER VARYING, CHAR, CHARACTER | TEXT | All text-ish types are semantically equivalent |

## testcontainers API Calls Used

- `Postgres::default().start().await` (via `testcontainers_modules::testcontainers::runners::AsyncRunner`)
- `container.get_host().await`
- `container.get_host_port_ipv4(5432).await`
- Connection string: `postgres://postgres:postgres@{host}:{port}/postgres`

## Decisions Made
- Fixed Postgres migration to use BIGINT for `enabled` and `exit_code` columns — SQLite's INTEGER is always 8-byte, so Postgres must match with BIGINT (not SMALLINT/INTEGER) for the parity test to be meaningful
- Handled SQLite's `INTEGER PRIMARY KEY` notnull=0 quirk by checking the `pk` column from PRAGMA table_info, rather than hardcoding column names

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed Postgres migration type mismatch for parity**
- **Found during:** Task 1 (schema parity test)
- **Issue:** Postgres migration used `SMALLINT` for `enabled` and `INTEGER` for `exit_code`, while SQLite uses `INTEGER` (always 8-byte). The parity test correctly detected this as drift.
- **Fix:** Changed both columns to `BIGINT` in the Postgres migration to match SQLite's INTEGER semantics
- **Files modified:** migrations/postgres/20260410_000000_initial.up.sql
- **Verification:** Schema parity test passes with 0 diffs
- **Committed in:** 7c1f458 (Task 1 commit)

**2. [Rule 1 - Bug] SQLite PRIMARY KEY nullability quirk**
- **Found during:** Task 1 (schema parity test)
- **Issue:** SQLite PRAGMA table_info reports `notnull=0` for `INTEGER PRIMARY KEY` columns even though they can never be NULL. Postgres BIGSERIAL PRIMARY KEY correctly reports NOT NULL.
- **Fix:** Added check for `pk > 0` in SQLite introspection to treat PK columns as implicitly NOT NULL
- **Files modified:** tests/schema_parity.rs
- **Verification:** All 3 tables' id columns now match between backends
- **Committed in:** 7c1f458 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes necessary for correctness. Migration fix improves actual schema parity. No scope creep.

## Issues Encountered
- Docker socket at non-standard path (`/Users/Robert/.rd/docker.sock` via Rancher Desktop) — requires `DOCKER_HOST` env var for testcontainers. CI runners use standard `/var/run/docker.sock` so no CI impact.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Schema parity test ready to be wired into `just schema-diff` (Plan 06)
- CI workflow (Plan 07) can run `cargo test --test schema_parity` in every matrix cell
- No unexpected type names surfaced during first run — whitelist covers the full initial schema

---
*Phase: 01-foundation-security-posture-persistence-base*
*Completed: 2026-04-10*
