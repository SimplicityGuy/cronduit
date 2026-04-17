-- Phase 11: per-job run numbering (DB-12) — SQLite.
--
-- File 2 of 3: chunked backfill — MARKER ONLY.
-- sqlx::migrate! runs this file's static SELECT only; the actual backfill
-- is orchestrated from Rust by src/db/migrate_backfill.rs which runs AFTER
-- sqlx::migrate!'s first pass and BEFORE its second pass applies file 3
-- (Plan 11-04 adds the second pass in DbPool::migrate).
--
-- Rationale: sqlx::migrate! supports only static SQL; the 10k-row batching
-- loop + per-batch INFO progress log (D-13) lives in Rust. This marker
-- ensures sqlx-tracker records file 2 as applied so re-runs skip it cleanly
-- while the Rust orchestrator's sentinel-table (`_v11_backfill_done`) +
-- `WHERE job_run_number IS NULL` guard provides idempotent partial-crash
-- recovery.

SELECT 1;  -- no-op
