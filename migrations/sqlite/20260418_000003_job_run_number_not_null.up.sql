-- Phase 11: per-job run numbering (DB-10) — SQLite.
--
-- File 3 of 3: flip job_runs.job_run_number to NOT NULL + add UNIQUE index.
-- SQLite does not support ALTER COLUMN ... SET NOT NULL, so we use the
-- canonical 12-step table-rewrite per https://www.sqlite.org/lang_altertable.html
--
-- Pairs with migrations/postgres/20260418_000003_job_run_number_not_null.up.sql.
-- Any structural change MUST land in both files in the same PR, and
-- tests/schema_parity.rs MUST remain green.
--
-- Preconditions (enforced by DbPool::migrate's conditional two-pass strategy):
--   1. File 1 (20260416_000001) added job_run_number as nullable. ✓
--   2. src/db/migrate_backfill.rs filled every row BEFORE this file runs,
--      OR this file runs on a fresh install where job_runs is empty.
--
-- Idempotency: sqlx records applied migrations in _sqlx_migrations and will
-- not re-run this file.

PRAGMA foreign_keys = OFF;

CREATE TABLE job_runs_new (
    id                INTEGER PRIMARY KEY,
    job_id            INTEGER NOT NULL REFERENCES jobs(id),
    job_run_number    INTEGER NOT NULL,
    status            TEXT    NOT NULL,
    trigger           TEXT    NOT NULL,
    start_time        TEXT    NOT NULL,
    end_time          TEXT,
    duration_ms       INTEGER,
    exit_code         INTEGER,
    container_id      TEXT,
    error_message     TEXT
);

INSERT INTO job_runs_new (
    id, job_id, job_run_number, status, trigger, start_time, end_time,
    duration_ms, exit_code, container_id, error_message
)
SELECT
    id, job_id, job_run_number, status, trigger, start_time, end_time,
    duration_ms, exit_code, container_id, error_message
FROM job_runs;

DROP TABLE job_runs;
ALTER TABLE job_runs_new RENAME TO job_runs;

CREATE INDEX IF NOT EXISTS idx_job_runs_job_id_start ON job_runs(job_id, start_time DESC);
CREATE INDEX IF NOT EXISTS idx_job_runs_start_time   ON job_runs(start_time);

CREATE UNIQUE INDEX IF NOT EXISTS idx_job_runs_job_id_run_number
    ON job_runs(job_id, job_run_number);

PRAGMA foreign_key_check;
PRAGMA foreign_keys = ON;
