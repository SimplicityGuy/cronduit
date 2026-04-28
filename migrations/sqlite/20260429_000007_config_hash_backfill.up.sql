-- Phase 16: job_runs.config_hash backfill (FCTX-04, D-02, D-03).
--
-- BACKFILL_CUTOFF_RFC3339: 2026-04-28T00:00:00Z
-- (Marker per Phase 16 D-03. Bumped from 2026-04-27 to 2026-04-28 per
--  code-review WR-03 so it sits strictly AFTER any v1.1 finish time on
--  every plausible deploy day — leaves no cutoff-hour false-negative
--  window for Phase 21's heuristic. Format is RFC3339 UTC — matches the
--  start_time column convention; chosen over Unix epoch for human
--  readability per RESEARCH Pitfall 7.)
--
-- File 3 of 3: best-effort bulk backfill (NOT marker-only — D-02
-- explicitly REJECTS v1.1's Rust-orchestrator pattern; homelab DBs
-- <100k rows complete this UPDATE in <1s).
--
-- WR-04 / cross-phase contract (NOT a SQL filter):
--   This SQL does NOT filter on end_time. It backfills EVERY row where
--   config_hash IS NULL — the BACKFILL_CUTOFF_RFC3339 marker above is a
--   forward-looking documentation token for downstream consumers, not a
--   predicate on this UPDATE.
--
--   Phase 21's UI panel will read this marker and apply a
--   "presumed backfilled" heuristic on the QUERY side:
--       end_time < BACKFILL_CUTOFF_RFC3339 AND config_hash IS NOT NULL
--   Rows matching that predicate are flagged as semantically suspect
--   (reflect "config_hash at backfill time", not "at run time"). Rows
--   whose matching `jobs` row was deleted leave `config_hash` NULL
--   (orphan handling).
--
-- Pairs with migrations/postgres/20260429_000007_config_hash_backfill.up.sql.
-- Identical SQL on both backends — the correlated UPDATE shape is
-- standard SQL accepted by SQLite and Postgres (RESEARCH §G.3).
--
-- Idempotency: `WHERE config_hash IS NULL` guard makes re-runs safe;
-- sqlx _sqlx_migrations tracking provides primary idempotency.

UPDATE job_runs
   SET config_hash = (SELECT j.config_hash FROM jobs j WHERE j.id = job_runs.job_id)
 WHERE config_hash IS NULL;
