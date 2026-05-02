-- Phase 20 / WH-05: Dead-letter audit table for webhook deliveries that failed
-- to reach 2xx (per CONTEXT D-10). One row per FAILED delivery (no row on
-- first-attempt success). NO payload/header/signature columns (D-12 — secret/PII
-- hygiene); receivers re-derive payload from run_id if needed.
--
-- Pairs with migrations/sqlite/20260502_000008_webhook_deliveries_add.up.sql.
-- Any structural change MUST land in both files in the same PR;
-- tests/schema_parity.rs::normalize_type collapses TEXT-family types to TEXT,
-- so this table passes parity checks with zero test edits.
--
-- Idempotency: sqlx _sqlx_migrations tracking + IF NOT EXISTS guards.
-- Table starts empty; no backfill (D-13).
--
-- Index strategy (D-11): single index on `last_attempt_at` only — NO composite
-- (job_id, last_attempt_at) index in v1.2; sequential scan over a small
-- (homelab-sized, failure-bounded) table is cheaper than maintaining a second
-- index.

CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id               BIGSERIAL PRIMARY KEY,
    run_id           BIGINT  NOT NULL,
    job_id           BIGINT  NOT NULL,
    url              TEXT    NOT NULL,
    attempts         INTEGER NOT NULL,
    last_status      INTEGER,
    last_error       TEXT,
    dlq_reason       TEXT    NOT NULL,
    first_attempt_at TEXT    NOT NULL,
    last_attempt_at  TEXT    NOT NULL,
    FOREIGN KEY (run_id) REFERENCES job_runs(id),
    FOREIGN KEY (job_id) REFERENCES jobs(id)
);

CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_last_attempt
    ON webhook_deliveries (last_attempt_at);
