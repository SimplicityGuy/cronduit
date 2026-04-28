---
phase: 260421-nn3-fix-get-dashboard-jobs-postgres-j-enable
plan: 01
subsystem: db/queries
tags: [bugfix, postgres, regression-test, quick-task]
requirements:
  - QUICK-260421-nn3
dependency_graph:
  requires:
    - src/db/queries.rs::get_dashboard_jobs (Postgres arm)
    - testcontainers-modules::postgres (already in dev-deps)
  provides:
    - Postgres-safe `get_dashboard_jobs` (BIGINT-correct `enabled` compare)
    - tests/dashboard_jobs_pg.rs regression guard
  affects:
    - Dashboard page on the Postgres backend (was silently broken pre-fix)
tech_stack:
  added: []
  patterns:
    - "Testcontainers-Postgres smoke test in default suite (no `integration` feature gate)"
key_files:
  created:
    - tests/dashboard_jobs_pg.rs
  modified:
    - src/db/queries.rs  # lines 615 + 628 — `j.enabled = true` → `j.enabled = 1`
decisions:
  - "No-error-only assertion (CONTEXT D-2 2A): `assert!(result.is_ok())` trivially proves the SQL-level bug is gone; row correctness / ORDER BY / EXPLAIN are out of scope."
  - "Fix both Postgres branches (CONTEXT D-3 3A): filtered path (line 615) and unfiltered path (line 628) had identical bug pattern; blast-radius is the same."
  - "No `#[cfg(feature = \"integration\")]` gate: mirrors tests/v13_timeline_explain.rs::explain_uses_index_postgres, which runs by default. The `integration` feature is reserved for host-Docker-daemon scenarios, not testcontainer-Postgres-only tests."
  - "No `.sqlx/` offline metadata regeneration: `get_dashboard_jobs` uses runtime `sqlx::query(&pg_sql)` strings, not the `query!` macro, so no compile-time DB check is involved."
metrics:
  duration_minutes: 4
  completed: 2026-04-22
  tasks_completed: 2
  files_changed: 2
---

# Quick Task 260421-nn3: Fix get_dashboard_jobs Postgres BIGINT Bug Summary

Closed the deferred Phase 13 item by (a) fixing the `get_dashboard_jobs` Postgres arm so `jobs.enabled` (BIGINT) is compared with integer literal `1` instead of boolean `true`, and (b) adding a Postgres integration test that fails loudly if the bug ever returns.

## What Was Done

### Task 1 — Regression test (RED step)

**File created:** `tests/dashboard_jobs_pg.rs`

Seeds one enabled job via `queries::upsert_job` against a fresh Postgres testcontainer and asserts `queries::get_dashboard_jobs` returns `Ok(_)` on both the unfiltered and filtered code paths. Mirrors `tests/v13_timeline_explain.rs::explain_uses_index_postgres` exactly — no `#[cfg(feature = "integration")]` gate, bare `#[tokio::test]`.

**Commit:** `07d81bb` — `test(queries): Postgres regression test for get_dashboard_jobs (BIGINT enabled)`

**RED proof (pre-fix failure):**
```
thread 'get_dashboard_jobs_postgres_smoke' (65334509) panicked at tests/dashboard_jobs_pg.rs:49:5:
get_dashboard_jobs (unfiltered) must succeed on Postgres; got: Some(error returned from database: operator does not exist: bigint = boolean
```
Captured in `/tmp/260421-nn3-red.log` during execution. Confirms the test correctly exercises the buggy SQL before the fix.

### Task 2 — BIGINT fix (GREEN step)

**File modified:** `src/db/queries.rs`

Two edits in the `PoolRef::Postgres(p) =>` arm of `get_dashboard_jobs`:

| Location | Before | After |
|----------|--------|-------|
| Line 615 (filtered path) | `WHERE j.enabled = true AND LOWER(j.name) LIKE $1` | `WHERE j.enabled = 1 AND LOWER(j.name) LIKE $1` |
| Line 628 (unfiltered path) | `WHERE j.enabled = true` | `WHERE j.enabled = 1` |

The SQLite arm was already correct (`j.enabled = 1` at lines 562 + 575) and was not touched. Mirrors the identical Rule-1 auto-fix applied to `get_timeline_runs` in Plan 13-06 commit `9f5e6c9`.

**Commit:** `7cb1a10` — `fix(queries): treat jobs.enabled as BIGINT in get_dashboard_jobs Postgres arm`

**GREEN proof (post-fix pass):**
```
PASS [   2.533s] (1/1) cronduit::dashboard_jobs_pg get_dashboard_jobs_postgres_smoke
Summary [   2.535s] 1 test run: 1 passed, 0 skipped
```

## Verification Matrix

| Gate | Command | Result |
|------|---------|--------|
| Regression test | `cargo nextest run --test dashboard_jobs_pg` | PASS (1/1) |
| Lib unit tests | `cargo nextest run --lib` | PASS (194/194) |
| Related Postgres suites | `cargo nextest run --test db_pool_postgres --test schema_parity --test v13_timeline_explain --test dashboard_jobs_pg` | PASS (8/8) |
| `j.enabled = true` scan | `grep -n "j\.enabled = true" src/db/queries.rs` | 0 matches |
| `j.enabled = 1` count | `grep -c "j\.enabled = 1" src/db/queries.rs` | 6 (2 SQLite-dash, 2 Postgres-dash-fixed, 1 SQLite-timeline, 1 Postgres-timeline) |
| Formatting | `cargo fmt --check` | clean |
| Linting | `cargo clippy --all-targets --all-features -- -D warnings` | clean |

## Deviations from Plan

None - plan executed exactly as written.

Both planned tasks executed in order. The RED test reproduced the expected `operator does not exist: bigint = boolean` error against a real Postgres container; the two-line fix turned the test GREEN with no surrounding code changes. No Rule 1-4 deviations encountered.

`rustfmt` collapsed a two-line `result_filtered` binding onto one line after `cargo fmt -- tests/dashboard_jobs_pg.rs` ran between Task 1 creation and commit. This is a pure formatting change, not a deviation — it reflects the project's house style before the test entered git history.

## Authentication Gates

None.

## Known Stubs

None.

## Threat Flags

None — the fix closes a pre-existing correctness bug on the Postgres backend; no new surface is introduced. The test file does not add any network-reachable endpoints (only drives existing public query functions through a test-only testcontainer pool).

## Commit Log

| Hash | Message | Files |
|------|---------|-------|
| `07d81bb` | test(queries): Postgres regression test for get_dashboard_jobs (BIGINT enabled) | tests/dashboard_jobs_pg.rs (+64) |
| `7cb1a10` | fix(queries): treat jobs.enabled as BIGINT in get_dashboard_jobs Postgres arm | src/db/queries.rs (+2/-2) |

## Branch

`gsd/quick-260421-nn3-fix-dashboard-jobs-postgres` — lands via PR per CLAUDE.md (no direct commits to main).

## Self-Check: PASSED

- Commit `07d81bb` present (`git log --oneline -5` confirms).
- Commit `7cb1a10` present.
- File `tests/dashboard_jobs_pg.rs` exists (64 lines, runs without `--features integration`).
- File `src/db/queries.rs` contains `j.enabled = 1` at both previously-buggy locations (lines 615, 628) and zero `j.enabled = true` occurrences.
- Regression test passes GREEN post-fix; `cargo fmt --check` and `cargo clippy --all-targets --all-features -- -D warnings` both clean.
- Related Postgres tests (`db_pool_postgres`, `schema_parity`, `v13_timeline_explain`) still pass — no cross-test regression.
