-- Phase 11: per-job run numbering (DB-09, DB-10, DB-11) — PostgreSQL.
--
-- File 1 of 3. Adds nullable job_runs.job_run_number plus the NOT NULL
-- jobs.next_run_number counter (DEFAULT 1). Files 2 and 3 backfill existing
-- rows and tighten to NOT NULL respectively.
--
-- Pairs with migrations/sqlite/20260416_000001_job_run_number_add.up.sql.
-- BIGINT here matches SQLite's INTEGER under the INT64 normalization rule
-- in tests/schema_parity.rs. Any structural change MUST land in both files
-- in the same PR.

ALTER TABLE jobs ADD COLUMN IF NOT EXISTS next_run_number BIGINT NOT NULL DEFAULT 1;
ALTER TABLE job_runs ADD COLUMN IF NOT EXISTS job_run_number BIGINT;
-- job_runs.job_run_number stays nullable until file 3 (per DB-10 split migration).
