-- Phase 22: jobs.tags JSON column (TAG-01, TAG-02).
--
-- TEXT NOT NULL DEFAULT '[]', FOREVER (TAG-02): operators may attach
-- normalized organizational tags to any job in cronduit.toml; existing
-- pre-Phase-22 rows are auto-defaulted to '[]' on column add. Old rows
-- never need backfill — empty-array is a valid in-domain value.
--
-- Pairs with migrations/sqlite/20260504_000010_jobs_tags_add.up.sql.
-- Any structural change MUST land in both files in the same PR;
-- tests/schema_parity.rs::normalize_type collapses TEXT-family types
-- to TEXT, so this column passes parity with zero test edits (RESEARCH
-- §E pattern carried from P16 image_digest_add).
--
-- Idempotency: Postgres `IF NOT EXISTS` provides re-run safety even if
-- sqlx's _sqlx_migrations ledger is somehow out of sync.

ALTER TABLE jobs ADD COLUMN IF NOT EXISTS tags TEXT NOT NULL DEFAULT '[]';
