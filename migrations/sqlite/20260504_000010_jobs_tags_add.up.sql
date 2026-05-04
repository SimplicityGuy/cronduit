-- Phase 22: jobs.tags JSON column (TAG-01, TAG-02).
--
-- TEXT NOT NULL DEFAULT '[]', FOREVER (TAG-02): operators may attach
-- normalized organizational tags to any job in cronduit.toml; existing
-- pre-Phase-22 rows are auto-defaulted to '[]' on column add. Old rows
-- never need backfill — empty-array is a valid in-domain value.
--
-- Pairs with migrations/postgres/20260504_000010_jobs_tags_add.up.sql.
-- Any structural change MUST land in both files in the same PR;
-- tests/schema_parity.rs::normalize_type collapses TEXT-family types
-- to TEXT, so this column passes parity with zero test edits (RESEARCH
-- §E pattern carried from P16 image_digest_add).
--
-- Idempotency: sqlx _sqlx_migrations tracking. SQLite ALTER TABLE ADD
-- COLUMN does NOT support a conditional-existence guard clause
-- (Postgres pair uses one; SQLite cannot). Re-runs are guarded by
-- sqlx's migration ledger.

ALTER TABLE jobs ADD COLUMN tags TEXT NOT NULL DEFAULT '[]';
