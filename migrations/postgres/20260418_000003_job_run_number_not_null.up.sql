-- Phase 11: per-job run numbering (DB-10) — PostgreSQL.
--
-- File 3 of 3: flip job_runs.job_run_number to NOT NULL + add UNIQUE index.
-- Postgres supports straight ALTER COLUMN ... SET NOT NULL; no table rewrite.
--
-- Pairs with migrations/sqlite/20260418_000003_job_run_number_not_null.up.sql.
-- Any structural change MUST land in both files in the same PR, and
-- tests/schema_parity.rs MUST remain green.
--
-- Preconditions (enforced by DbPool::migrate's conditional two-pass strategy):
--   1. File 1 (20260416_000001) added the nullable column. ✓
--   2. src/db/migrate_backfill.rs filled every row BEFORE this file runs,
--      OR this file runs on a fresh install where job_runs is empty.
--
-- Idempotency: sqlx records applied migrations in _sqlx_migrations and will
-- not re-run this file. `IF NOT EXISTS` on the index is belt-and-suspenders.

ALTER TABLE job_runs ALTER COLUMN job_run_number SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_job_runs_job_id_run_number
    ON job_runs(job_id, job_run_number);
