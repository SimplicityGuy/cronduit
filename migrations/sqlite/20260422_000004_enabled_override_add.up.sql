-- Phase 14: jobs.enabled_override tri-state column (DB-14, ERG-04).
--
-- Nullable INTEGER: NULL = follow config `enabled` flag (no override);
-- 0 = force disabled (written by POST /api/jobs/bulk-toggle with action=disable);
-- 1 = force enabled (reserved — v1.1 UI never writes this; defensive rendering only).
--
-- Pairs with migrations/postgres/20260422_000004_enabled_override_add.up.sql.
-- Any structural change MUST land in both files in the same PR;
-- tests/schema_parity.rs normalize_type collapses INTEGER + BIGINT to INT64.
--
-- Idempotency: sqlx _sqlx_migrations tracking. No backfill needed —
-- NULL is the correct initial state for every existing row (D-13).

ALTER TABLE jobs ADD COLUMN enabled_override INTEGER;
