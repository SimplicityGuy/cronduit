-- Phase 11: per-job run numbering (DB-12) — PostgreSQL.
--
-- File 2 of 3: chunked backfill — MARKER ONLY. See paired SQLite file for
-- the full rationale. Rust orchestrator in src/db/migrate_backfill.rs does
-- the real work, guarded by the `_v11_backfill_done` sentinel table for
-- O(1) re-run idempotency.

SELECT 1;  -- no-op
