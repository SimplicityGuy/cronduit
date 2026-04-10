-- cronduit initial schema (SQLite)
--
-- Pairs with migrations/postgres/20260410_000000_initial.up.sql.
-- Any structural change MUST land in both files in the same PR,
-- and tests/schema_parity.rs (Plan 05) MUST remain green.
--
-- Design notes:
--   * jobs.config_json is TEXT (never JSONB). D-16.
--   * jobs.config_hash is SHA-256 hex of the normalized (sorted-keys,
--     stable-ordering) JSON representation of the job config. See
--     src/config/hash.rs. D-15.
--   * Timestamps are RFC3339 TEXT for SQLite portability.
--   * Partial index on job_runs(status) WHERE status='running' is
--     DEFERRED to Phase 2 per 01-RESEARCH.md S4.

CREATE TABLE IF NOT EXISTS jobs (
    id                 INTEGER PRIMARY KEY,
    name               TEXT    NOT NULL UNIQUE,
    schedule           TEXT    NOT NULL,
    resolved_schedule  TEXT    NOT NULL,
    job_type           TEXT    NOT NULL,
    config_json        TEXT    NOT NULL,
    config_hash        TEXT    NOT NULL,
    enabled            INTEGER NOT NULL DEFAULT 1,
    timeout_secs       INTEGER NOT NULL,
    created_at         TEXT    NOT NULL,
    updated_at         TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jobs_enabled ON jobs(enabled);

CREATE TABLE IF NOT EXISTS job_runs (
    id             INTEGER PRIMARY KEY,
    job_id         INTEGER NOT NULL REFERENCES jobs(id),
    status         TEXT    NOT NULL,
    trigger        TEXT    NOT NULL,
    start_time     TEXT    NOT NULL,
    end_time       TEXT,
    duration_ms    INTEGER,
    exit_code      INTEGER,
    container_id   TEXT,
    error_message  TEXT
);

CREATE INDEX IF NOT EXISTS idx_job_runs_job_id_start ON job_runs(job_id, start_time DESC);
CREATE INDEX IF NOT EXISTS idx_job_runs_start_time   ON job_runs(start_time);

CREATE TABLE IF NOT EXISTS job_logs (
    id         INTEGER PRIMARY KEY,
    run_id     INTEGER NOT NULL REFERENCES job_runs(id),
    stream     TEXT    NOT NULL,
    ts         TEXT    NOT NULL,
    line       TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_job_logs_run_id_id ON job_logs(run_id, id);
