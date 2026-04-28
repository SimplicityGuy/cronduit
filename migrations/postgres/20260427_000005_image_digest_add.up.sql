-- Phase 16: job_runs.image_digest per-run column (FOUND-14, FCTX-04).
--
-- Nullable TEXT, FOREVER (D-01): docker jobs populate this from
-- post-start `inspect_container().image` at finalize time; command and
-- script jobs legitimately have no image and leave the column NULL;
-- pre-v1.2 docker rows also stay NULL forever (D-04 — no backfill).
--
-- Pairs with migrations/sqlite/20260427_000005_image_digest_add.up.sql.
-- Any structural change MUST land in both files in the same PR;
-- tests/schema_parity.rs::normalize_type collapses TEXT/VARCHAR/CHARACTER
-- VARYING/CHAR/CHARACTER to TEXT, so this column passes parity with zero
-- test edits (RESEARCH §E).
--
-- Idempotency: Postgres `IF NOT EXISTS` provides re-run safety even if
-- sqlx's _sqlx_migrations ledger is somehow out of sync.

ALTER TABLE job_runs ADD COLUMN IF NOT EXISTS image_digest TEXT;
