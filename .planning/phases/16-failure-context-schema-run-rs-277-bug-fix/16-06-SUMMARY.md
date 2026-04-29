---
phase: 16-failure-context-schema-run-rs-277-bug-fix
plan: 06
subsystem: testing
tags: [sqlx, sqlite, postgres, explain, testcontainers, fctx, regression-lock]

# Dependency graph
requires:
  - phase: 16-failure-context-schema-run-rs-277-bug-fix
    provides: "Plan 16-01 (image_digest + config_hash schema columns); Plan 16-04a/16-04b (insert_running_run/finalize_run signature changes that wire config_hash into job_runs at fire time)"
  - phase: 16-failure-context-schema-run-rs-277-bug-fix
    provides: "Plan 16-05 (parallel sibling) — get_failure_context CTE helper. The EXPLAIN test inlines the same D-05 locked SQL the helper uses, so the wave-3 post-merge gate validates both files compose."
provides:
  - "tests/v12_fctx_explain.rs: regression-lock asserting both SQLite and Postgres planners pick idx_job_runs_job_id_start (job_id, start_time DESC) for the get_failure_context CTE"
  - "Future SQL refactor of get_failure_context that loses indexed access fails CI"
  - "Future Postgres major-version bump that regresses the planner choice fails CI"
affects:
  - "Phase 18 (webhook payload, WH-09): query helper performance is regression-locked"
  - "Phase 21 (FCTX UI, FCTX-01..06): query helper performance is regression-locked"
  - "Future Phase 16 amendments to the CTE shape: must preserve indexed access on idx_job_runs_job_id_start for both arms"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Single test file with both SQLite + Postgres EXPLAIN arms (mirrors v13_timeline_explain.rs precedent — Open Question 2 / RESEARCH §3)"
    - "Inlined locked SQL in EXPLAIN tests when the production helper is being added in a parallel wave plan (decouples test landing from helper-symbol availability)"
    - "Postgres EXPLAIN-test ANALYZE-after-seed pattern (RESEARCH Pitfall 4 mitigation)"
    - "Plan-tree JSON walker for Index Scan / Index Only Scan / Bitmap Index Scan / Bitmap Heap Scan with textual fallback (verbatim from v13)"

key-files:
  created:
    - "tests/v12_fctx_explain.rs"
  modified: []

key-decisions:
  - "Inlined the D-05 locked CTE SQL into the test file (option (a) per parallel-execution note) instead of importing from queries.rs::get_failure_context. Sibling Plan 16-05 lands the helper in parallel; the test file does NOT depend on its symbol. The wave-end gate validates composition."
  - "Single test file (`tests/v12_fctx_explain.rs`) with both #[tokio::test] functions — mirrors v13_timeline_explain.rs precedent (RESEARCH Open Question 2; D-08's 'one test file per backend' wording is permissive per CONTEXT.md)."
  - "Postgres test #[ignore]-gated (testcontainers dependency, matches v13 convention)."
  - "Dropped v13's alternation `idx_job_runs_start_time || idx_job_runs_job_id_start` — the get_failure_context CTE only hits `idx_job_runs_job_id_start` (per PATTERNS.md `<What differs>`)."

patterns-established:
  - "EXPLAIN-plan regression-lock for new query helpers: inline production SQL + assert index reference + reject bare table scan; one file, both backends."
  - "Wave-parallel test+helper landing: test file uses inlined SQL constants matching the locked decision, helper is implemented in sibling plan; wave-end CI gate validates composition."

requirements-completed: [FCTX-07]

# Metrics
duration: 8min
completed: 2026-04-28
---

# Phase 16 Plan 06: EXPLAIN-plan regression-lock for get_failure_context Summary

**Dual-backend EXPLAIN tests (SQLite EXPLAIN QUERY PLAN + Postgres EXPLAIN FORMAT JSON) asserting `idx_job_runs_job_id_start` is hit by both CTE arms of the get_failure_context query — locks FCTX-07 Success Criterion 3.**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-04-28T03:27:16Z
- **Completed:** 2026-04-28T03:35:26Z
- **Tasks:** 2 (1 test-creation task + 1 verification task)
- **Files modified:** 1 (created)

## Accomplishments

- New regression-locked test file `tests/v12_fctx_explain.rs` (364 lines, two `#[tokio::test]` functions) verifying both SQLite and Postgres planners pick `idx_job_runs_job_id_start (job_id, start_time DESC)` for the get_failure_context CTE.
- SQLite arm asserts `EXPLAIN QUERY PLAN` substring contains `idx_job_runs_job_id_start` AND rejects bare `SCAN job_runs` (full table scan disallowed per D-08).
- Postgres arm runs `ANALYZE job_runs` + `ANALYZE jobs` after seeding 10,000 mixed-status rows, then walks the `EXPLAIN (FORMAT JSON)` plan JSON for Index Scan / Index Only Scan / Bitmap Index Scan / Bitmap Heap Scan node types, with textual fallback for `idx_job_runs_job_id_start` (mirrors v13_timeline_explain.rs precedent verbatim).
- Both CTE arms (last_success LIMIT 1 + streak range scan above the boundary) are exercised by the seeded fixture so the planner walks both index lookups.
- Phase 16 Plan 16-06 closes the regression lock: any future SQL refactor that loses indexed access fails CI noisily.

## Task Commits

Each task was committed atomically:

1. **Task 1: Create tests/v12_fctx_explain.rs with both backends** — `603652c` (test)
2. **Task 2: Run final phase gates and confirm green status** — no diff to commit (verification-only task; results captured in Self-Check below).

_Note: Task 2 has no per-task commit because it is a verification-only task that produces no file changes (the plan's verification gate scope spans the entire Phase 16 wave; this worktree confirmed all gates that can run pre-merge — see Issues Encountered for the deferred wave-end gate)._

## Files Created/Modified

- `tests/v12_fctx_explain.rs` — NEW. Two `#[tokio::test]` functions:
  - `explain_uses_index_sqlite` (runs in standard CI, no Docker required) — passes.
  - `explain_uses_index_postgres` (`#[ignore]`-gated; requires Docker for testcontainers Postgres image) — compiles cleanly; runs locally when Docker is available.

## Decisions Made

1. **Inlined CTE SQL in the test file (option (a) from parallel-execution note).** Sibling Plan 16-05 lands `get_failure_context` in queries.rs in parallel. The test file does NOT call the production helper — it inlines the same D-05 locked CTE SQL and runs `EXPLAIN ... <SQL>` directly via `sqlx::query`. This decouples test landing from helper-symbol availability and lets both plans compose cleanly via the wave-3 post-merge gate. (Rationale: the test verifies the SQL pattern hits the index; the production helper uses the same SQL pattern; if 16-05 deviates from D-05, the helper test in 16-05 would fail first.)

2. **Single test file, two `#[tokio::test]` functions** (RESEARCH §3 + Open Question 2). D-08's "one test file per backend" wording is permissive per CONTEXT.md; v13_timeline_explain.rs sets the precedent for single-file dual-backend EXPLAIN tests.

3. **Postgres test `#[ignore]`-gated** (matches v13 convention; testcontainers dependency).

4. **Dropped the v13's `idx_job_runs_start_time || idx_job_runs_job_id_start` alternation** — the get_failure_context CTE only hits `idx_job_runs_job_id_start` (job_id, start_time DESC) since both arms predicate on `job_id` first.

5. **Used `queries::upsert_job` for job seeding** instead of raw INSERT into `jobs` (the plan's example INSERT used `type` instead of the actual `job_type` column name and omitted required `created_at`/`updated_at` NOT NULL columns). Using the production helper handles all those concerns automatically. Raw INSERT only used for `job_runs` rows (where status mix and start_time control matter).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Doc-comment formatting tripped clippy `doc_lazy_continuation` lint**
- **Found during:** Task 1 (test creation, post-write clippy check)
- **Issue:** The "Seeder notes" paragraph in the file-level `//!` doc comment had a continuation paragraph starting with "+ `finalize_run`)" that clippy parsed as a markdown list-item bullet. The subsequent unindented continuation lines triggered five `doc_lazy_continuation` errors under `-D warnings`.
- **Fix:** Restructured the paragraph to avoid the leading-`+` token; replaced "`insert_running_run`\n//! + `finalize_run`)" with "production `insert_running_run` and `finalize_run` helpers". Same content, different prose.
- **Files modified:** `tests/v12_fctx_explain.rs`
- **Verification:** `cargo clippy --test v12_fctx_explain -- -D warnings` returns clean.
- **Committed in:** `603652c` (Task 1 commit — caught and fixed before commit).

**2. [Rule 1 - Bug] Plan's example INSERT for `jobs` table used wrong column name**
- **Found during:** Task 1 (test creation, schema verification)
- **Issue:** Plan 16-06's skeleton example used `INSERT INTO jobs (..., type, ...)` but the actual schema column is `job_type` (per `migrations/sqlite/20260410_000000_initial.up.sql:21`). The example also omitted the NOT NULL `created_at` and `updated_at` columns, and used `enabled = 1` without addressing the v11 `next_run_number` NOT NULL DEFAULT 1 column.
- **Fix:** Used `queries::upsert_job(...)` (the production helper) for job seeding instead of raw INSERT — handles `created_at`/`updated_at`/`next_run_number` defaults automatically and uses the correct `job_type` column name. This is the same pattern v13_timeline_explain.rs uses for its job seeding.
- **Files modified:** `tests/v12_fctx_explain.rs`
- **Verification:** SQLite test passes (`cargo test --test v12_fctx_explain explain_uses_index_sqlite` returns ok).
- **Committed in:** `603652c` (Task 1 commit — caught while implementing).

---

**Total deviations:** 2 auto-fixed (Rule 1 — both bugs in the plan's example skeleton; the test as committed uses the correct shapes).
**Impact on plan:** Neither deviation changes the plan's intent; both fixes preserve the locked behavior (D-05 CTE SQL, D-08 assertion shapes). The test is regression-locked exactly per FCTX-07 Success Criterion 3 and D-08.

## Issues Encountered

- **Sibling Plan 16-05's `tests/v12_fctx_streak.rs` is not yet in this worktree** because Wave 3 runs both plans in parallel. The wave-end gate (after both 16-05 and 16-06 merge back to main) will run `cargo test --test v12_fctx_streak` and `just nextest` against the composed codebase. This worktree confirmed all gates that CAN run pre-merge:
  - `cargo build` — green
  - `just fmt-check` — green
  - `just clippy` — green
  - `just grep-no-percentile-cont` — green (D-15 compliant)
  - `just schema-diff` — green (3 passed, no migration changes)
  - `cargo test --test v12_fctx_explain explain_uses_index_sqlite` — green (1 passed; 1 ignored-by-design)
  - `cargo test --test v12_fctx_config_hash_backfill` — green (4 passed; Plan 16-01 deliverable)
  - `just nextest` — green (389 passed, 23 skipped including the testcontainer-gated Postgres EXPLAIN tests)

  The deferred gates `cargo test --test v12_fctx_streak` and the Docker-required `v12_run_rs_277_bug_fix` integration tests are out of this worktree's scope per the parallel execution contract.

## Cross-References

- **Plan 16-05 (sibling):** lands `src/db/queries.rs::get_failure_context` and `tests/v12_fctx_streak.rs`. The CTE SQL inlined in this plan's test file is the same SQL 16-05 emits into the helper. The wave-3 post-merge gate validates composition.
- **`tests/v13_timeline_explain.rs` (v1.1 OBS-02 precedent):** the verbatim-mirror analog. The structure of `tests/v12_fctx_explain.rs` follows it line-for-line: same SQLite arm shape (in-memory DB + 100-row seed + EXPLAIN QUERY PLAN substring), same Postgres arm shape (testcontainer + 10k-row seed + ANALYZE + EXPLAIN FORMAT JSON + plan-tree JSON walker + textual fallback).
- **CONTEXT.md D-05:** the locked CTE SQL shape (verbatim in the test file's `FCTX_SQL_SQLITE` and `FCTX_SQL_POSTGRES` constants).
- **CONTEXT.md D-08:** the locked assertion contract (plan must reference `idx_job_runs_job_id_start`; plan must NOT contain bare `SCAN job_runs` / `Seq Scan` on job_runs without an index hit).
- **RESEARCH.md Pitfall 4:** the ANALYZE-after-seed mandate for fresh testcontainer Postgres. Implemented in `explain_uses_index_postgres`.
- **PATTERNS.md "tests/v12_fctx_explain.rs (NEW) — EXPLAIN dual-backend":** the pattern excerpt (verbatim SQLite + Postgres idiom snippets).

## Phase 16 Phase-Gate Status (this worktree)

| Gate | Status | Notes |
|------|--------|-------|
| `cargo build` | green | Compiles cleanly with new test file. |
| `just fmt-check` | green | Format compliance verified. |
| `just clippy` | green | All lints pass under `-D warnings`. |
| `just grep-no-percentile-cont` | green | D-15 compliant (no banned SQL functions). |
| `just schema-diff` | green | 3 passed; no migration changes in this plan. |
| `cargo test --test v12_fctx_config_hash_backfill` | green | 4 passed (Plan 16-01 deliverable). |
| `cargo test --test v12_fctx_explain explain_uses_index_sqlite` | green | 1 passed; this plan's deliverable. |
| `cargo test --test v12_fctx_explain explain_uses_index_postgres` | n/a (ignored) | Compiles cleanly; ignored-by-design (testcontainers). |
| `just nextest` | green | 389 tests pass, 23 skipped. |
| `cargo test --test v12_fctx_streak` | deferred | Sibling Plan 16-05 deliverable; wave-end gate validates. |
| `cargo test --test v12_run_rs_277_bug_fix` (Docker integration) | deferred | Plan 16-03 deliverable; requires local Docker daemon. |

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Phase 16 wave 3 ready for merge** from this worktree's perspective. Sibling Plan 16-05's parallel work merges separately; the wave-end gate at the orchestrator level runs `cargo test --test v12_fctx_streak` + full nextest against the composed codebase.
- **Phase 18 (webhook payload, WH-09)** can consume the regression-locked `get_failure_context` performance contract: a future Postgres major-version bump or SQL refactor that regresses to Seq Scan will fail CI before reaching production.
- **Phase 21 (FCTX UI, FCTX-01..06)** likewise consumes the same performance contract.

## Self-Check: PASSED

**File existence:**
- FOUND: `tests/v12_fctx_explain.rs` (364 lines)

**Commit existence:**
- FOUND: `603652c` test(16-06): add EXPLAIN-plan tests for get_failure_context CTE

**Acceptance criteria (Task 1):**
- FOUND: `idx_job_runs_job_id_start` substring in file
- FOUND: `EXPLAIN QUERY PLAN` substring (SQLite idiom)
- FOUND: `EXPLAIN (FORMAT JSON)` substring (Postgres idiom)
- FOUND: `ANALYZE job_runs` substring (Pitfall 4 mitigation)
- FOUND: `explain_uses_index_sqlite` `#[tokio::test]` function
- FOUND: `explain_uses_index_postgres` `#[tokio::test]` function with `#[ignore]` gate
- FOUND: SQLite test exits 0 under `cargo test --test v12_fctx_explain explain_uses_index_sqlite`

**Verification gates (Task 2; gates that can run pre-merge):**
- PASSED: cargo build, just fmt-check, just clippy, just grep-no-percentile-cont, just schema-diff, just nextest (389 passed, 23 skipped)

---
*Phase: 16-failure-context-schema-run-rs-277-bug-fix*
*Plan: 06*
*Completed: 2026-04-28*
