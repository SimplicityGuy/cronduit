-- cronduit initial schema (PostgreSQL)
--
-- Pairs with migrations/sqlite/20260410_000000_initial.up.sql. Keep in sync.
-- D-15: config_hash column exists; Phase 1 does not yet populate.
-- D-16: config_json is TEXT on both backends (never JSONB).

CREATE TABLE IF NOT EXISTS jobs (
    id                 BIGSERIAL PRIMARY KEY,
    name               TEXT     NOT NULL UNIQUE,
    schedule           TEXT     NOT NULL,
    resolved_schedule  TEXT     NOT NULL,
    job_type           TEXT     NOT NULL,
    config_json        TEXT     NOT NULL,
    config_hash        TEXT     NOT NULL,
    enabled            BIGINT   NOT NULL DEFAULT 1,
    timeout_secs       BIGINT   NOT NULL,
    created_at         TEXT     NOT NULL,
    updated_at         TEXT     NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jobs_enabled ON jobs(enabled);

CREATE TABLE IF NOT EXISTS job_runs (
    id             BIGSERIAL PRIMARY KEY,
    job_id         BIGINT   NOT NULL REFERENCES jobs(id),
    status         TEXT     NOT NULL,
    trigger        TEXT     NOT NULL,
    start_time     TEXT     NOT NULL,
    end_time       TEXT,
    duration_ms    BIGINT,
    exit_code      BIGINT,
    container_id   TEXT,
    error_message  TEXT
);

CREATE INDEX IF NOT EXISTS idx_job_runs_job_id_start ON job_runs(job_id, start_time DESC);
CREATE INDEX IF NOT EXISTS idx_job_runs_start_time   ON job_runs(start_time);

CREATE TABLE IF NOT EXISTS job_logs (
    id         BIGSERIAL PRIMARY KEY,
    run_id     BIGINT   NOT NULL REFERENCES job_runs(id),
    stream     TEXT     NOT NULL,
    ts         TEXT     NOT NULL,
    line       TEXT     NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_job_logs_run_id_id ON job_logs(run_id, id);
