-- Phase 11: per-job run numbering (DB-09, DB-10, DB-11) — SQLite.
--
-- File 1 of 3. Adds nullable job_runs.job_run_number plus the NOT NULL
-- jobs.next_run_number counter (DEFAULT 1). Files 2 and 3 backfill existing
-- rows and tighten to NOT NULL respectively.
--
-- Pairs with migrations/postgres/20260416_000001_job_run_number_add.up.sql.
-- Any structural change MUST land in both files in the same PR, and
-- tests/schema_parity.rs MUST remain green (normalize_type collapses
-- INTEGER + BIGINT to INT64).
--
-- Idempotency: sqlx records applied migrations in _sqlx_migrations and will
-- not re-run this file. Partial-crash recovery is handled by file 2's
-- WHERE job_run_number IS NULL guard (DB-10).

ALTER TABLE jobs ADD COLUMN next_run_number INTEGER NOT NULL DEFAULT 1;
ALTER TABLE job_runs ADD COLUMN job_run_number INTEGER;
-- job_runs.job_run_number stays nullable until file 3 (per DB-10 split migration).
