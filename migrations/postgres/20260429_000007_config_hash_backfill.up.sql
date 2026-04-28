-- Phase 16: job_runs.config_hash backfill (FCTX-04, D-02, D-03).
--
-- BACKFILL_CUTOFF_RFC3339: 2026-04-27T00:00:00Z
-- (Marker per Phase 16 D-03; Phase 21's UI panel reads this convention.
--  Format is RFC3339 UTC — matches start_time column convention.)
--
-- File 3 of 3: best-effort bulk backfill (NOT marker-only — D-02
-- explicitly REJECTS v1.1's Rust-orchestrator pattern). Postgres MVCC
-- semantics: row-level write locks only, no table lock; default
-- statement_timeout = 0; <100k-row homelab DBs complete in <1s
-- (RESEARCH §G.3). v1.3 introduces chunked-loop if scaling pain.
--
-- Heuristic: rows where `end_time < BACKFILL_CUTOFF_RFC3339` AND
-- `config_hash IS NOT NULL` AFTER this migration are backfilled
-- (semantically suspect — reflect "config_hash at backfill time").
-- Rows whose matching `jobs` row was deleted leave `config_hash` NULL.
--
-- Pairs with migrations/sqlite/20260429_000007_config_hash_backfill.up.sql.
-- Identical SQL on both backends.
--
-- Idempotency: `WHERE config_hash IS NULL` guard makes re-runs safe.

UPDATE job_runs
   SET config_hash = (SELECT j.config_hash FROM jobs j WHERE j.id = job_runs.job_id)
 WHERE config_hash IS NULL;
