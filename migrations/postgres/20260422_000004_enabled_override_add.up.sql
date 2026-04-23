-- Phase 14: jobs.enabled_override tri-state column (DB-14, ERG-04).
--
-- Nullable BIGINT: matches SQLite INTEGER under the INT64 normalization rule
-- in tests/schema_parity.rs. NULL = follow config; 0 = force disabled;
-- 1 = force enabled (reserved — v1.1 UI never writes this).
--
-- Pairs with migrations/sqlite/20260422_000004_enabled_override_add.up.sql.
-- Any structural change MUST land in both files in the same PR.

ALTER TABLE jobs ADD COLUMN IF NOT EXISTS enabled_override BIGINT;
