-- Phase 16: job_runs.config_hash backfill (FCTX-04, D-02, D-03).
--
-- BACKFILL_CUTOFF_RFC3339: 2026-04-27T00:00:00Z
-- (Marker per Phase 16 D-03; Phase 21's UI panel reads this convention to
--  distinguish backfilled rows from authentic per-run captures. Format is
--  RFC3339 UTC — matches the start_time column convention; chosen over
--  Unix epoch for human readability per RESEARCH Pitfall 7.)
--
-- File 3 of 3: best-effort bulk backfill (NOT marker-only — D-02
-- explicitly REJECTS v1.1's Rust-orchestrator pattern; homelab DBs
-- <100k rows complete this UPDATE in <1s).
--
-- Heuristic: rows where `end_time < BACKFILL_CUTOFF_RFC3339` AND
-- `config_hash IS NOT NULL` AFTER this migration are backfilled
-- (semantically suspect — reflect "config_hash at backfill time", not
-- "at run time"). Rows whose matching `jobs` row was deleted leave
-- `config_hash` NULL (orphan handling).
--
-- Pairs with migrations/postgres/20260427_000007_config_hash_backfill.up.sql.
-- Identical SQL on both backends — the correlated UPDATE shape is
-- standard SQL accepted by SQLite and Postgres (RESEARCH §G.3).
--
-- Idempotency: `WHERE config_hash IS NULL` guard makes re-runs safe;
-- sqlx _sqlx_migrations tracking provides primary idempotency.

UPDATE job_runs
   SET config_hash = (SELECT j.config_hash FROM jobs j WHERE j.id = job_runs.job_id)
 WHERE config_hash IS NULL;
