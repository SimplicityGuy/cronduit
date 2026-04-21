# Phase 13 deferred items

Out-of-scope discoveries logged per executor SCOPE BOUNDARY rule.

## Pre-existing Postgres bug in `get_dashboard_jobs`

- **File:** `src/db/queries.rs` lines 615 and 628
- **Issue:** Two `WHERE j.enabled = true` clauses in the Postgres arm of `get_dashboard_jobs`. The `jobs.enabled` column is BIGINT on Postgres (intentionally, per schema_parity normalize_type), so these clauses raise `operator does not exist: bigint = boolean` when executed. The shipped code happens to never hit this path in the test suite (no Postgres integration test of `get_dashboard_jobs`), so the bug has ridden along since an earlier phase.
- **Discovered by:** Phase 13 plan 06 Task 1 `tests/v13_timeline_explain.rs::explain_uses_index_postgres` — the identical bug pattern in `get_timeline_runs` was the Rule-1 auto-fix for this plan.
- **Scope:** Out of Phase 13 scope. Fix requires a separate phase so the change lands with its own tested regression path (and likely a Postgres integration test of `get_dashboard_jobs`).
- **Recommended action:** File as a Phase 14 bug-fix plan or a standalone hygiene commit before the v1.1.0 GA cut.
