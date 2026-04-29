---
phase: 16-failure-context-schema-run-rs-277-bug-fix
plan: 01
subsystem: database
tags: [sqlx, sqlite, postgres, migrations, schema, fctx, image_digest, config_hash]

# Dependency graph
requires:
  - phase: 15-foundation-preamble
    provides: Cargo 1.2.0 bump + cargo-deny CI preamble + webhook delivery worker scaffolding (no schema changes)
provides:
  - job_runs.image_digest TEXT NULL column (FOUND-14, FCTX-04 storage on both backends)
  - job_runs.config_hash TEXT NULL column (FCTX-04 per-run capture storage on both backends)
  - Best-effort bulk backfill of pre-v1.2 job_runs.config_hash from jobs.config_hash
  - BACKFILL_CUTOFF_RFC3339 structured-comment marker for Phase 21 UI parser
  - Integration test asserting backfill correctness, idempotency, orphan handling, and column existence
affects:
  - 16-02 (DockerExecResult.container_id field add — uses image_digest column at finalize_run)
  - 16-04a/16-04b (queries.rs finalize_run + insert_running_run signature changes — write to these columns)
  - 16-05/16-06 (get_failure_context query — reads last_success_image_digest and last_success_config_hash)
  - 18 (webhook payload — serializes image_digest and config_hash deltas)
  - 21 (FCTX UI panel — reads BACKFILL_CUTOFF_RFC3339 marker to flag backfilled rows)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Three-file per-backend nullable-forever migration (rejects v1.1 add→backfill→NOT NULL chain — D-01)"
    - "Bulk single-statement backfill SQL (rejects v1.1 marker-only Rust orchestrator — D-02)"
    - "Structured comment markers in migration files for downstream UI consumers (BACKFILL_CUTOFF_RFC3339 — D-03)"
    - "Migration date prefixes must be unique per file (sqlx splitn(2, '_') version parsing)"

key-files:
  created:
    - migrations/sqlite/20260427_000005_image_digest_add.up.sql
    - migrations/postgres/20260427_000005_image_digest_add.up.sql
    - migrations/sqlite/20260428_000006_config_hash_add.up.sql
    - migrations/postgres/20260428_000006_config_hash_add.up.sql
    - migrations/sqlite/20260429_000007_config_hash_backfill.up.sql
    - migrations/postgres/20260429_000007_config_hash_backfill.up.sql
    - tests/v12_fctx_config_hash_backfill.rs
  modified: []

key-decisions:
  - "Rule 1 auto-fix: split the three new migrations across three distinct date prefixes (20260427, 20260428, 20260429) instead of all sharing 20260427, because sqlx parses the version via splitn(2, '_') and identical prefixes produce a UNIQUE _sqlx_migrations.version constraint failure on first migrate."

patterns-established:
  - "Phase 16 migration filename pattern: ALL six new files follow `YYYYMMDD_NNNNNN_<purpose>.up.sql` with monotonic per-file dates so sqlx version parsing remains injective."
  - "BACKFILL_CUTOFF_RFC3339 structured comment marker convention is content-not-filename — the marker timestamp may differ from the migration filename date and reflects the operator-visible cutoff for Phase 21's UI parser."

requirements-completed: [FCTX-04, FOUND-14]

# Metrics
duration: ~9min
completed: 2026-04-27
---

# Phase 16 Plan 01: Failure-Context Schema (FCTX-04 + FOUND-14) Summary

**Six new migration files (3 SQLite + 3 Postgres) add `job_runs.image_digest TEXT NULL` and `job_runs.config_hash TEXT NULL` and best-effort backfill the per-run config_hash from `jobs.config_hash`, with a `BACKFILL_CUTOFF_RFC3339: 2026-04-27T00:00:00Z` marker for Phase 21's UI parser, validated by a 4-test integration suite and the existing schema-parity + migrations-idempotent green-bar gates.**

## Performance

- **Duration:** ~9 min
- **Started:** 2026-04-28T02:32:04Z (UTC)
- **Completed:** 2026-04-28T02:40:37Z (UTC)
- **Tasks:** 8 (T1–T6 migration files, T7 integration test, T8 verification gate)
- **Files created:** 7 (6 migration files + 1 test file)
- **Files modified:** 0

## Accomplishments

- Both new per-run columns (`image_digest`, `config_hash`) land on SQLite + Postgres in lock-step, passing the structural-parity invariant with zero edits to `tests/schema_parity.rs` (RESEARCH §E confirmed dynamic introspection auto-covers TEXT-family columns).
- The bulk backfill UPDATE populates `config_hash` from `jobs.config_hash` for pre-existing `job_runs` rows; the `WHERE config_hash IS NULL` guard makes the migration safe to re-run.
- The `BACKFILL_CUTOFF_RFC3339: 2026-04-27T00:00:00Z` marker is deposited in both backfill files as a structured comment for Phase 21's UI parser to grep (D-03 / Pitfall 7 — RFC3339 UTC chosen for human readability over Unix epoch).
- New integration test `tests/v12_fctx_config_hash_backfill.rs` verifies four scenarios: pre-NULL row gets backfilled, re-running migration is idempotent, orphaned rows (job_id pointing at deleted jobs) stay NULL, and both new columns exist as NULLABLE after `pool.migrate()`.
- Existing `tests/migrations_idempotent.rs` and `tests/schema_parity.rs` (via `just schema-diff`) both stay green with the 6 new files.

## Task Commits

Each task was committed atomically:

1. **Task 1: SQLite image_digest_add migration** — `a738d91` (feat)
2. **Task 2: Postgres image_digest_add migration** — `a41c717` (feat)
3. **Task 3: SQLite config_hash_add migration** — `3758abc` (feat)
4. **Task 4: Postgres config_hash_add migration** — `a4777a5` (feat)
5. **Task 5: SQLite config_hash_backfill migration** — `ac8144a` (feat)
6. **Task 6: Postgres config_hash_backfill migration** — `9da673d` (feat)
7. **Rule 1 fix: rename 006/007 to unique date prefixes** — `71d1912` (fix)
8. **Task 7: integration test (4 scenarios)** — `275db7d` (test)

T8 (schema-parity + migrations-idempotent green-bar gate) is a verification step with no source changes; verified locally and contributes no commit.

## Files Created/Modified

- `migrations/sqlite/20260427_000005_image_digest_add.up.sql` — Single-statement nullable ALTER for FOUND-14 + FCTX-04 image_digest column.
- `migrations/postgres/20260427_000005_image_digest_add.up.sql` — Postgres parity using `IF NOT EXISTS` for defense-in-depth idempotency.
- `migrations/sqlite/20260428_000006_config_hash_add.up.sql` — Single-statement nullable ALTER for FCTX-04 config_hash column.
- `migrations/postgres/20260428_000006_config_hash_add.up.sql` — Postgres parity using `IF NOT EXISTS`.
- `migrations/sqlite/20260429_000007_config_hash_backfill.up.sql` — Bulk UPDATE backfill with `WHERE config_hash IS NULL` guard + `BACKFILL_CUTOFF_RFC3339: 2026-04-27T00:00:00Z` marker.
- `migrations/postgres/20260429_000007_config_hash_backfill.up.sql` — Identical correlated UPDATE on Postgres; row-level write locks only (RESEARCH §G.3).
- `tests/v12_fctx_config_hash_backfill.rs` — 4 integration scenarios (backfill_populates_config_hash_for_pre_v12_rows, backfill_is_idempotent, orphaned_rows_stay_null, columns_exist_after_full_migrate). All pass.

## Decisions Made

- **BACKFILL_CUTOFF_RFC3339 value:** `2026-04-27T00:00:00Z` (UTC midnight on the planned migration day, per Pitfall 7's RFC3339-UTC mandate; format chosen over Unix epoch for human readability).
- **Migration date prefixes:** the three new files use `20260427`, `20260428`, and `20260429` rather than the plan's literal `20260427_*` triple — see Deviations §1 below.
- **Test seeding strategy:** the integration test seeds rows via raw `INSERT` against the post-migrate schema, then UPDATEs `config_hash = NULL` to simulate pre-v1.2 state, then re-runs the bulk UPDATE. This avoids needing a custom "stop-before-007" fixture in `tests/common/v11_fixtures.rs`. The plan's `<read_first>` block listed the alternative but explicitly tagged it as planner discretion.
- **`tests/schema_parity.rs` not edited:** verified via `git diff --quiet tests/schema_parity.rs`. RESEARCH §E pre-confirmed that `normalize_type` already collapses TEXT-family columns to `"TEXT"`, so the new columns require no whitelist update.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Rename migrations 006 and 007 to use unique date prefixes**

- **Found during:** Task 7 (integration test first run)
- **Issue:** The plan specified all three new migrations with the date prefix `20260427` (`20260427_000005_*`, `20260427_000006_*`, `20260427_000007_*`). sqlx parses the migration version via `splitn(2, '_')` and uses `parts[0]` as the i64 version. Three files with the same date prefix all map to version `20260427`, producing `UNIQUE constraint failed: _sqlx_migrations.version` on the first call to `pool.migrate()`. This caused all four T7 tests to fail at the `setup_sqlite_with_phase11_migrations` fixture call. The plan's `must_haves` artifacts list and CONTEXT.md's illustrative file layout disagree on this — CONTEXT.md showed `20260427`, `20260428`, `20260429` (three distinct dates) which is the correct shape; the PLAN.md task descriptions collapsed them to a single date.
- **Fix:** `git mv` renamed `20260427_000006` → `20260428_000006` and `20260427_000007` → `20260429_000007` on both backends; updated each renamed file's `Pairs with migrations/<other>/<filename>` header and the inline reference (`paired migration ..._000007_config_hash_backfill.up.sql` → full new path) to reflect the new filenames.
- **Files modified:** all four files renamed via git-mv (history preserved, ~80–95% similarity); inline header path references updated.
- **Verification:** `cargo test --test v12_fctx_config_hash_backfill` advanced from 0/4 → 1/4 pass after rename, then 4/4 pass after Rule 1 fix #2 below; `cargo test --test migrations_idempotent` green; `just schema-diff` green.
- **Committed in:** `71d1912` (separate atomic commit between T6 and T7 since the bug bridges both — the 6 migration files conceptually own the filename layout, but the discovery happened in T7).

**2. [Rule 1 - Bug] Add `job_run_number` to seed_job_run / orphan-insert SQL in the new test**

- **Found during:** Task 7 (second test run, after Rule 1 fix #1)
- **Issue:** Phase 11 made `job_runs.job_run_number` NOT NULL (file 3 of the v1.1 three-file chain). My initial test `seed_job_run` and `orphaned_rows_stay_null` orphan-insert helpers omitted that column from the column list, hitting `NOT NULL constraint failed: job_runs.job_run_number` on three of the four scenarios. The plan's `<action>` SQL excerpt for `seed_job` mentioned `next_run_number` (a `jobs` column) but did not flag the `job_run_number` (a `job_runs` column) NOT NULL requirement.
- **Fix:** Added `job_run_number` to the column list and bound a constant `1` in both `seed_job_run` and the orphan INSERT statement (each test seeds at most one run per job, so a constant is sufficient — mirrors the v11 fixtures' approach for tests that don't exercise the counter).
- **Files modified:** `tests/v12_fctx_config_hash_backfill.rs` only.
- **Verification:** All four tests pass; `cargo build --tests` clean.
- **Committed in:** `275db7d` (folded into the T7 test commit since the test was not yet in any commit at the time of fix).

**3. [Rule 1 - Bug] Reword the SQLite-005 idempotency comment to remove the literal phrase `IF NOT EXISTS`**

- **Found during:** Task 1 (verify command)
- **Issue:** The plan's exact `<action>` content for the SQLite migration file 005 included a comment block that mentioned `IF NOT EXISTS` (in a sentence explaining that SQLite cannot use it). The plan's verify shell expression includes `! grep -q 'IF NOT EXISTS'`, which is a literal grep over the entire file content — so the very comment the plan dictated also failed the plan's own verification. The semantic intent (SQLite DDL must not use `IF NOT EXISTS`) is intact, but the literal grep cannot distinguish DDL from comment text.
- **Fix:** Reworded the SQLite file 005's idempotency comment to phrase the constraint without using the literal token (`SQLite ALTER TABLE ADD COLUMN does NOT support a conditional-existence guard clause`). The same wording was used for the SQLite file 006 comment proactively to avoid the same false positive.
- **Files modified:** `migrations/sqlite/20260427_000005_image_digest_add.up.sql` and `migrations/sqlite/20260428_000006_config_hash_add.up.sql` (proactive consistency with file 005).
- **Verification:** Both SQLite migration files pass the plan's automated grep verification.
- **Committed in:** Folded into the original `a738d91` (T1) and `3758abc` (T3) commits — the fix happened before either file was committed.

---

**Total deviations:** 3 auto-fixed (3 × Rule 1 — Bug)
**Impact on plan:** All three are mechanical corrections to the plan's prescribed text/file-naming. The semantic shape (one nullable ALTER per backend per requirement; one bulk-UPDATE backfill per backend with the BACKFILL_CUTOFF_RFC3339 marker) is preserved exactly. Schema parity, idempotency, and integration-test coverage all green. No scope creep.

## Issues Encountered

- The plan's task count (8 tasks) intentionally includes T8 as a no-source-change verification gate. The committed history therefore shows 8 commits (one per task) plus one additional commit (`71d1912`) for the Rule 1 file-rename fix, which spanned tasks T1–T6 conceptually but was discovered during T7. T8 itself contributes no source commit; it is verified by the local green-bar runs of `cargo test --test migrations_idempotent` and `just schema-diff` recorded above.

## Threat Surface Scan

No new threat surfaces introduced beyond those already documented in the plan's `<threat_model>` (T-16-01-01..T-16-01-04, all `accept` disposition with severity `low`). The migrations introduce no new network endpoints, no new auth paths, no new file access patterns, and no new schema columns at trust boundaries beyond what the plan's threat register already covers.

## Self-Check: PASSED

All 7 created files verified present on disk: 6 migration files (3 SQLite + 3 Postgres) under `migrations/{sqlite,postgres}/` and `tests/v12_fctx_config_hash_backfill.rs`. All 8 commits verified in `git log` (`a738d91`, `a41c717`, `3758abc`, `a4777a5`, `ac8144a`, `9da673d`, `71d1912`, `275db7d`). `cargo test --test v12_fctx_config_hash_backfill` reports 4/4 pass. `cargo test --test migrations_idempotent` reports 1/1 pass. `just schema-diff` reports 3/3 pass (sqlite_and_postgres_schemas_match_structurally + 2 normalize_type unit tests). `git diff --quiet tests/schema_parity.rs` exits 0 (no edits to that file).

## User Setup Required

None — no external service configuration. All artifacts are static SQL files and one Rust integration test; the migrations apply automatically on next `pool.migrate()` (i.e., on next Cronduit startup).

## Next Phase Readiness

- **16-02 (DockerExecResult.container_id field add):** has the `image_digest` column waiting for it on both backends — `finalize_run` can write to the new column once Plan 16-04a/b lands the signature change.
- **16-04a/16-04b (queries.rs signature changes):** both new columns are present and NULLABLE; `insert_running_run` can bind `config_hash` and `finalize_run` can bind `image_digest` without further migration work.
- **16-05/16-06 (get_failure_context query + EXPLAIN tests):** `last_success_image_digest` and `last_success_config_hash` are real columns now; the CTE per D-05 can SELECT them directly.
- **Phase 21 (FCTX UI panel):** the `BACKFILL_CUTOFF_RFC3339: 2026-04-27T00:00:00Z` marker is in place in both backfill files for Phase 21's UI parser.

No blockers.

---
*Phase: 16-failure-context-schema-run-rs-277-bug-fix*
*Plan: 16-01*
*Completed: 2026-04-27 (PT) / 2026-04-28 (UTC)*
