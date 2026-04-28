-- Phase 16: job_runs.config_hash per-run column (FCTX-04).
--
-- Nullable TEXT, FOREVER (D-01): captured at fire time by
-- `insert_running_run` from the resolved DbJob's hash (in-memory Config
-- ⇒ compute_config_hash); pre-v1.2 rows are best-effort backfilled by
-- the paired migration `20260429_000007_config_hash_backfill.up.sql`.
--
-- Per-RUN column resolves Research-Phase Correction 2 (Option A locked
-- at requirements step) — failure-context delta needs per-RUN history.
--
-- Pairs with migrations/sqlite/20260428_000006_config_hash_add.up.sql.
-- Any structural change MUST land in both files in the same PR;
-- tests/schema_parity.rs::normalize_type collapses TEXT/VARCHAR/CHARACTER
-- VARYING/CHAR/CHARACTER to TEXT (RESEARCH §E).
--
-- Idempotency: Postgres `IF NOT EXISTS` provides re-run safety.

ALTER TABLE job_runs ADD COLUMN IF NOT EXISTS config_hash TEXT;
