# Quick Task 260421-nn3: Fix get_dashboard_jobs Postgres j.enabled BIGINT bug + add Postgres integration test — Context

**Gathered:** 2026-04-22
**Status:** Ready for planning

<domain>
## Task Boundary

Two-line source fix in `src/db/queries.rs` — the Postgres arm of `get_dashboard_jobs` has `WHERE j.enabled = true` at lines 615 and 628, but `jobs.enabled` is BIGINT on Postgres (per schema_parity `normalize_type`). This raises `operator does not exist: bigint = boolean` when executed against a real Postgres DB. Fix: change both to `WHERE j.enabled = 1`, mirroring the Plan 13-06 Rule-1 auto-fix for `get_timeline_runs`. Plus: add a Postgres integration test for `get_dashboard_jobs` so the bug can never silently return.

Origin: Phase 13 `deferred-items.md` — deferred out of Phase 13 scope so the fix can land with its own regression test.

</domain>

<decisions>
## Implementation Decisions

### 1. Test framework parity
- **Locked: 1A — exact mirror of existing Phase 13 pattern.** Reuse `testcontainers-modules::postgres::Postgres`, follow the feature-gate and harness rhythm of `tests/v13_timeline_explain.rs::explain_uses_index_postgres`. No new test infrastructure; zero new patterns. Keeps review surface minimal.

### 2. Test scope
- **Locked: 2A — no-error-only assertion.** The bug is specifically `operator does not exist: bigint = boolean`. The test seeds a Postgres DB with at least one enabled job, calls `get_dashboard_jobs`, and asserts `Ok(..)`. That trivially proves the bug is gone. Row correctness and EXPLAIN assertions are explicitly out of scope — they belong in a query-perf-focused phase, not this fix.

### 3. Search-filter branch
- **Locked: 3A — fix both lines (615 and 628).** Both have the identical bug pattern. Blast-radius is the same; no reason to split. The test's `None` (no search filter) path covers line 628 directly; line 615 can be optionally exercised via a search-filter path in the same test, or left to follow-up if the test's assertion is strictly "call without error on the default path."

### Claude's Discretion
- Exact test file name: default to `tests/v13a_dashboard_jobs_pg.rs` or a non-v13-prefixed name like `tests/dashboard_jobs_pg.rs` (this isn't part of Phase 13 — don't claim the v13 namespace). Planner picks the most consistent name.
- Whether to optionally include a trivial search-filter invocation in the test to cover line 615. Planner decides based on minimal-surface principle.
- Postgres image tag for testcontainers: match what `v13_timeline_explain.rs` already uses — don't bikeshed.

</decisions>

<specifics>
## Specific Ideas

- Mirror commit pattern: `fix(queries): treat jobs.enabled as BIGINT in get_dashboard_jobs Postgres arm` + `test(queries): Postgres regression test for get_dashboard_jobs (BIGINT enabled)`.
- Reference the precedent: Plan 13-06 auto-fixed the identical bug in `get_timeline_runs` (commit `9f5e6c9`). This quick task closes the remaining instance per `.planning/phases/13-observability-polish-rc-2/deferred-items.md`.
- Validation command to run locally before committing: `cargo nextest run --test <new-test-file> --features integration` (or the project's existing feature flag for testcontainers tests).

</specifics>

<canonical_refs>
## Canonical References

- `.planning/phases/13-observability-polish-rc-2/deferred-items.md` — origin of the issue.
- `src/db/queries.rs` lines 615 + 628 — buggy lines.
- `tests/v13_timeline_explain.rs` — harness precedent for `testcontainers-modules::postgres::Postgres`.
- Commit `9f5e6c9 test(13-06): add dual-backend EXPLAIN QUERY PLAN tests for timeline query (OBS-02)` — the inline Rule-1 fix for the same bug in `get_timeline_runs`.

</canonical_refs>
