---
phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
plan: 09
subsystem: testing
tags: [sqlx, sqlite, postgres, explain, fctx-06, scheduled_for, integration-test]

# Dependency graph
requires:
  - phase: 16-image-digest
    provides: tests/v12_fctx_explain.rs (P16 baseline EXPLAIN QUERY PLAN harness for `get_failure_context` on both backends)
  - phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
    provides: plan 21-01 — `migrations/{sqlite,postgres}/20260503_000009_scheduled_for_add.up.sql` (the additive nullable TEXT column whose index posture this plan locks)
provides:
  - "FCTX-06 invariant test guard: adding `scheduled_for TEXT NULL` does NOT shift `idx_job_runs_job_id_start` index plan for `get_failure_context` on either backend"
  - "Pattern: name-bearing post-migration guard tests in EXPLAIN harnesses (not just edits to baseline tests) so the diff/test output explicitly surfaces which schema change the assertion exists to catch"
affects: []  # leaf — no downstream Phase 21 plan reads this output

# Tech tracking
tech-stack:
  added: []  # zero new external crates (D-32 invariant preserved)
  patterns:
    - "Post-migration EXPLAIN guard test: after an additive schema change, add a separately-named `*_post_<change>` test function (not edit the baseline) so the assertion's intent is named in the test surface"
    - "VERBATIM seed-body re-use validated by research landmine §10: the explicit-column-list INSERT survives additive nullable column adds with zero edits"

key-files:
  created: []
  modified:
    - tests/v12_fctx_explain.rs

key-decisions:
  - "Add two new test functions (`explain_uses_index_sqlite_post_scheduled_for` + `explain_uses_index_postgres_post_scheduled_for`) instead of editing the existing tests — the existing tests already run against the post-migration schema by virtue of sqlx file-system ordering, but a dedicated, named test surfaces the FCTX-06 intent explicitly in test output (D-18 directive)"
  - "Re-use the existing explicit-column-list INSERT verbatim — research landmine §10 confirms `scheduled_for` defaults to NULL on both backends when omitted, so the seed body needs zero edits"
  - "Postgres post-migration test gated with `#[ignore]` exactly like the existing P16 postgres test (testcontainers-backed, Docker-required, runs in the CI Postgres lane via `--run-ignored=all`)"
  - "Inline-duplicate the `contains_index_scan` walker rather than hoist it to a private helper — preserves copy-locality with the P16 precedent and keeps each test self-contained for incremental debugging"

patterns-established:
  - "Post-migration EXPLAIN guard test naming convention: `<existing>_post_<column_or_change>` (e.g., `explain_uses_index_sqlite_post_scheduled_for`)"
  - "Header doc-comment cites the D-decision ID (D-18), the research landmine §10, and the source-of-truth migration filename so future maintainers can trace the assertion's intent without reading the plan"

requirements-completed: [FCTX-06]

# Metrics
duration: ~5min
completed: 2026-05-02
---

# Phase 21 Plan 09: FCTX-06 EXPLAIN-after-scheduled_for Tests Summary

**Two new EXPLAIN QUERY PLAN guard tests in `tests/v12_fctx_explain.rs` that lock the FCTX-06 invariant: adding the additive nullable `scheduled_for TEXT` column does NOT shift the `idx_job_runs_job_id_start` index plan for `get_failure_context` on either backend.**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-05-02T21:06:29Z
- **Completed:** 2026-05-02T21:12:00Z (approx)
- **Tasks:** 1 (single auto task)
- **Files modified:** 1 (`tests/v12_fctx_explain.rs`)

## Accomplishments
- `explain_uses_index_sqlite_post_scheduled_for` — runs against in-memory SQLite with the full Phase 21 migration set applied (including `migrations/sqlite/20260503_000009_scheduled_for_add.up.sql`); seeds 100 mixed-status rows; asserts `idx_job_runs_job_id_start` referenced AND no bare `SCAN job_runs` without `USING INDEX`.
- `explain_uses_index_postgres_post_scheduled_for` (#[ignore]'d, Docker-gated) — same assertion via testcontainers Postgres + 10,000 row seed + ANALYZE (RESEARCH Pitfall 4 mitigation), accepts either an Index Scan node OR a textual `idx_job_runs_job_id_start` reference per the v13 / P16 precedent.
- Existing tests (`explain_uses_index_sqlite`, `explain_uses_index_postgres`) untouched — still green per research landmine §10 (the explicit-column-list INSERT omits `scheduled_for` and SQLite/Postgres default it to NULL).
- Verified locally: `cargo nextest run --test v12_fctx_explain -E 'not test(postgres)'` passes 2/2 sqlite tests; postgres tests deferred to CI (Docker-backed lane).

## Task Commits

Single auto task, single feat-style commit (using `test(...)` per the conventional-commit `test:` type for test-only changes):

1. **Task 1: Add two `*_post_scheduled_for` EXPLAIN tests** — `10a131b` (test)

_No metadata commit yet — this SUMMARY commit is a separate test/docs commit per Phase 21 wave-3 conventions._

## Files Created/Modified
- `tests/v12_fctx_explain.rs` — added two test functions (266 lines added; existing 365-line file preserved verbatim)

## Decisions Made
- **Add named tests rather than edit baseline tests** (D-18 directive): The baseline `explain_uses_index_sqlite` already runs against the post-Phase-21 schema thanks to sqlx applying every migration in `migrations/sqlite/`, but adding a separately-named `*_post_scheduled_for` test surfaces the FCTX-06 intent in the test surface — when a future schema change touches `job_runs`, the named guard appears in CI diffs as the explicit "this assertion exists to catch column-additions changing the index plan" alarm.
- **VERBATIM seed-body re-use** (research landmine §10): The existing explicit-column-list INSERT was copy-pasted into both new tests with zero edits. SQLite and Postgres both default the omitted `scheduled_for` column to NULL, so the seed body survives the migration with zero modification — the column being NULL is also what pre-v1.2 rows look like in production (D-04: no backfill ever).
- **Inline-duplicate `contains_index_scan` walker** rather than extracting a private helper: keeps each test fully self-contained for incremental debugging and preserves copy-locality with the P16 precedent. The walker is six lines; abstraction cost > duplication cost.
- **Postgres test `#[ignore]`** matches the existing P16 postgres test: testcontainers-backed, requires a live Docker daemon, runs in CI's Postgres lane via `cargo nextest run --run-ignored=all`. SQLite tests run unconditionally on every developer machine.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] `cargo fmt` collapsed trailing comments on out-of-scope files**
- **Found during:** Task 1 verify step (post-implementation `cargo fmt -- tests/v12_fctx_explain.rs` invocation, which the rustfmt config applied workspace-wide rather than to a single file).
- **Issue:** Although the `cargo fmt -- tests/v12_fctx_explain.rs` invocation was scoped to one file, the project-level `rustfmt.toml` triggered `cargo fmt` to also reformat unrelated files: `src/db/queries.rs`, `src/web/exit_buckets.rs`, `src/web/handlers/job_detail.rs`, `src/web/handlers/run_detail.rs`, `tests/jobs_api.rs` — these all had trailing comments collapsed onto single lines after structure-field declarations and similar surface edits. NONE of these files are in plan 21-09's scope (which is testing-only, modifying `tests/v12_fctx_explain.rs` ONLY).
- **Fix:** `git checkout HEAD --` on each out-of-scope file restored them to their pre-fmt state. Only `tests/v12_fctx_explain.rs` was committed by this plan.
- **Files modified:** N/A (the fix REVERTED out-of-scope edits)
- **Verification:** `git status --short` post-fix shows ONLY ` M tests/v12_fctx_explain.rs`; the staged commit `10a131b` has 1 file changed / 266 insertions / 0 deletions, matching plan 21-09's stated scope.
- **Committed in:** No fix commit — the revert happened pre-stage, so the final commit `10a131b` cleanly contains only the in-scope test changes.

**Plan invariant note:** This deviation surfaces a workspace-level fmt-config fact worth documenting for future parallel-wave plans: `cargo fmt -- <single_file>` will still apply project-level rustfmt rules workspace-wide if the rustfmt.toml settings are repository-scoped. Single-file format invocations should be followed by a `git status` check to ensure only the targeted file was modified, restoring any spurious touches with `git checkout HEAD -- <unrelated_path>`.

---

**Total deviations:** 1 auto-fixed (1 scope-bleed bug)
**Impact on plan:** Zero scope creep; the deviation actively _reduces_ scope by restoring out-of-scope files. All four other Wave 3 plans (21-07, 21-08, 21-10, 21-11) running in parallel worktrees are unaffected.

## Issues Encountered

- **Pre-existing clippy `doc_lazy_continuation` warnings on `src/web/handlers/job_detail.rs:450` and `src/web/handlers/run_detail.rs:220`:** These warnings exist on `9d0ef42` (the worktree base, post-Phase-21-06 merge) BEFORE plan 21-09 made any changes. Per scope-boundary rule (only auto-fix issues directly caused by current task's changes), these are out of scope for plan 21-09. Logged here for visibility — should be addressed by the wave that introduced them (21-04 or 21-05 / 21-06) or a follow-on cleanup plan; not by plan 21-09. The `cargo clippy --tests` output on `tests/v12_fctx_explain.rs` itself is clean.

## User Setup Required

None — test code only, no environment / configuration / external service touches.

## Next Phase Readiness

- **Wave 3 sibling plans (21-07, 21-08, 21-10, 21-11)** are unaffected by this plan — they run in parallel worktrees and modify orthogonal surfaces (panel template, histogram card, just recipes, HUMAN-UAT).
- **Wave 4 (CI gate / RC-2 release prep)** can rely on the FCTX-06 invariant being mechanically locked — any future structural migration that accidentally regresses the index plan will trip both `explain_uses_index_sqlite_post_scheduled_for` (in every PR's sqlite lane) and `explain_uses_index_postgres_post_scheduled_for` (in the CI Postgres lane).
- **Postgres lane validation:** the post-scheduled_for postgres test runs only on CI machines with Docker; the planning team should confirm it lights up green in the next CI Postgres run before treating FCTX-06 as fully verified end-to-end. The sqlite branch is locally green.

## Threat Flags

None — read-only EXPLAIN against in-memory SQLite (test 3) or testcontainers Postgres (test 4); no new auth/network/file-access surface; no schema change at any trust boundary; no production code touched. Threat register T-21-09-01 (`accept` disposition for the EXPLAIN test) remains valid as written.

## Self-Check: PASSED

- `tests/v12_fctx_explain.rs` — FOUND (post-edit; +266 lines committed in `10a131b`)
- Commit `10a131b` — FOUND in `git log --oneline`
- `grep -c "async fn explain_uses_index_sqlite_post_scheduled_for" tests/v12_fctx_explain.rs` — returns `1` (acceptance criterion 1)
- `grep -c "async fn explain_uses_index_postgres_post_scheduled_for" tests/v12_fctx_explain.rs` — returns `1` (acceptance criterion 2)
- `grep -c "idx_job_runs_job_id_start" tests/v12_fctx_explain.rs` — returns `18` (acceptance criterion 3 ≥ 4)
- `grep -E "async fn explain_uses_index_(sqlite|postgres)\b" tests/v12_fctx_explain.rs` — finds both baseline functions (acceptance criterion 4)
- `cargo nextest run --test v12_fctx_explain -E 'not test(postgres)'` — exits 0, 2/2 passed (postgres tests skipped per `#[ignore]`)
- `git status --short` — clean (only the one in-scope file changed and committed)
- Postgres post-migration test: deferred to CI Docker lane (D-18 sibling-precedent — the existing P16 postgres test uses the identical gating)

---
*Phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2*
*Plan: 09*
*Completed: 2026-05-02*
