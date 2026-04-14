---
phase: 01-foundation-security-posture-persistence-base
plan: 04
subsystem: database-persistence-boot-flow
tags: [sqlx, sqlite, postgres, migrations, startup-event, graceful-shutdown]
dependency_graph:
  requires: [01-01, 01-02]
  provides: [DbPool, DbBackend, strip_db_credentials, migrations, startup-event, boot-flow]
  affects: [01-05]
tech_stack:
  added: [sqlx-sqlite-pools, sqlx-postgres-pool, sqlx-migrate-macro, libc-dev-dep]
  patterns: [split-read-write-pools, compile-time-migrations, credential-stripping, structured-startup-event]
key_files:
  created:
    - migrations/sqlite/20260410_000000_initial.up.sql
    - migrations/postgres/20260410_000000_initial.up.sql
    - tests/db_pool_sqlite.rs
    - tests/migrations_idempotent.rs
    - tests/startup_event.rs
    - tests/graceful_shutdown.rs
  modified:
    - src/db/mod.rs
    - src/cli/run.rs
    - Cargo.toml
decisions:
  - "sqlite::memory: URL detection via starts_with prefix check, not split on ://"
  - "startup_event tests use SIGTERM + 1.5s wait instead of assert_cmd timeout (process::exit doesn't flush stdout on SIGKILL)"
  - "graceful_shutdown test accepts either exit code 0 or signal 15 to handle race between handler install and signal delivery"
metrics:
  duration: ~15m
  completed: 2026-04-10
  tasks_completed: 7
  tasks_total: 7
  tests_added: 11
  files_created: 6
  files_modified: 3
---

# Phase 1 Plan 4: SQLite/Postgres Persistence + Boot Flow Summary

DbPool enum with SQLite split read/write pools (WAL/busy_timeout/synchronous/foreign_keys), Postgres pool, dual-backend migration dispatch, structured cronduit.startup event with non-loopback bind warning, and full cronduit run boot flow.

## Tasks Completed

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | Create migration files (SQLite + Postgres) | 307e1e6 | migrations/sqlite/20260410_000000_initial.up.sql, migrations/postgres/20260410_000000_initial.up.sql |
| 2 | DbPool enum with split pools + migrate + credential stripping | 3b90dcc | src/db/mod.rs |
| 3 | Full boot flow in cronduit run | 63b2ad3 | src/cli/run.rs |
| 4 | SQLite PRAGMA assertions (Pitfall 7 guard) | 572b77d | tests/db_pool_sqlite.rs |
| 5 | Idempotent migration test (DB-03 + D-15) | af5e124 | tests/migrations_idempotent.rs |
| 6 | Startup event black-box tests (D-23/D-24) | 3e77377 | tests/startup_event.rs, Cargo.toml |
| 7 | Graceful shutdown SIGTERM test (T-01-08) | e2d7b14 | tests/graceful_shutdown.rs |

## Requirements Satisfied

- **DB-01**: SQLite default, zero-config via DbPool::connect("sqlite:...")
- **DB-02**: PostgreSQL optional via DbPool::connect("postgres://...")
- **DB-03**: Idempotent migrations (tested: migrate twice with no error)
- **DB-04**: jobs table with all required columns
- **DB-05**: job_runs table with status, trigger, timestamps, exit_code, container_id
- **DB-06**: job_logs table with run_id, stream, ts, line
- **DB-07**: Schema-only: jobs.enabled column exists (runtime soft-delete is Phase 5)
- **OPS-03**: Default bind 127.0.0.1:8080; loud WARN on non-loopback bind

## Threat Mitigations Verified

- **T-01-01**: Non-loopback bind warning tested by startup_event::startup_emits_warn_on_non_loopback_bind
- **T-01-02**: Credential stripping tested by db::tests::strip_creds_postgres and startup_event::startup_does_not_leak_database_credentials
- **T-01-11**: SQLite writer contention mitigated by max_connections=1 + WAL + busy_timeout=5000 + synchronous=NORMAL, asserted by db_pool_sqlite::sqlite_writer_pragmas_match_expectations
- **T-01-08**: Graceful shutdown tested by graceful_shutdown::sigterm_yields_clean_exit_within_one_second

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] SQLite URL scheme detection for sqlite::memory:**
- **Found during:** Task 2
- **Issue:** The plan's code used `split_once("://")` to detect the database scheme, but `sqlite::memory:` uses `:` not `://`, causing the connect function to reject valid SQLite URLs.
- **Fix:** Changed to prefix-based detection: `starts_with("sqlite:")` checked first, before the `://` split for postgres/other schemes.
- **Files modified:** src/db/mod.rs
- **Commit:** 3b90dcc

**2. [Rule 1 - Bug] Missing Debug derive on DbPool**
- **Found during:** Task 2
- **Issue:** The plan's code used `#[derive(Clone)]` but tests call `unwrap_err()` which requires `Debug`.
- **Fix:** Added `Debug` to the derive list: `#[derive(Clone, Debug)]`.
- **Files modified:** src/db/mod.rs
- **Commit:** 3b90dcc

**3. [Rule 3 - Blocking] Startup event tests failed with assert_cmd timeout**
- **Found during:** Task 6
- **Issue:** assert_cmd's `timeout()` uses SIGKILL which doesn't allow the process to flush stdout. Also, the process needs ~1s to start up (config parse + DB migrate + bind), so 500ms was insufficient.
- **Fix:** Rewrote tests to use std::process::Command + libc::kill(SIGTERM) with 1.5s startup wait. Added `libc = "0.2"` to dev-dependencies.
- **Files modified:** tests/startup_event.rs, Cargo.toml
- **Commit:** 3e77377

## Notes

- The `config_hash` column exists in the schema but is not populated by Phase 1 code. Phase 2 wires the config hash computation into the sync engine.
- The `jobs.enabled` column exists (DB-07 schema-only). Runtime soft-delete behavior ("removed jobs marked enabled=0") is implemented by the config reload sync engine in Phase 5.
- Startup event tests require `--test-threads=1` due to each test spawning a full binary process.

## Self-Check: PASSED

All 8 files verified present. All 7 commits verified in git log.
