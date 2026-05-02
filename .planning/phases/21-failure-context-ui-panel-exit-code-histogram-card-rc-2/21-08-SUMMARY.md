---
phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
plan: 08
subsystem: ui
tags: [exit-codes, histogram, integration-tests, render-tests, askama, job-detail, exit-01, exit-02, exit-03, exit-04, exit-05]

# Dependency graph
requires:
  - phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
    plan: 03
    provides: "src/web/exit_buckets.rs ExitBucket / categorize / aggregate / HistogramCard / TopCode + queries::get_recent_runs_for_histogram raw-fetch helper"
  - phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
    plan: 05
    provides: "src/web/handlers/job_detail.rs ExitHistogramView + BucketRender + TopCodeRender + build_exit_histogram_view (the render-target this test file exercises)"
  - phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
    plan: 06
    provides: "templates/pages/job_detail.html cd-exit-card markup + assets/src/app.css cd-exit-* class declarations (the locked CSS class + copy contract this file asserts)"
  - phase: 13-observability-polish-rc-2
    provides: "tests/v13_duration_card.rs::seed_runs_with_duration shape (raw-SQL INSERT bypassing finalize_run for deterministic durations) and tests/v13_timeline_render.rs::build_test_app harness shape (in-memory SQLite + axum router via ServiceExt::oneshot)"
provides:
  - "tests/v12_exit_histogram.rs: 7 render-level integration tests covering EXIT-01..EXIT-05 + D-15 + D-16 empty-state branches; locks the rendered HTML contract on GET /jobs/{job_id}"
  - "Render-test parity for the Exit Histogram card: empty-state (below-N=5 + zero-run), at-threshold chart, EXIT-04 dual-classifier, EXIT-03 success-rate badge, EXIT-05 recent-codes sub-table, all 10 short labels"
affects: [21-09, 21-10, 21-11]

# Tech tracking
tech-stack:
  added: []  # zero new external crates (D-32 invariant preserved)
  patterns:
    - "Render-level integration test pattern: build_test_app() copied verbatim from tests/v13_timeline_render.rs:32-58 (in-memory SQLite + AppState + router); raw-SQL seed helper extending v13_duration_card's pattern with status + exit_code parameters; ServiceExt::oneshot() for one-request-per-test driving"
    - "Plan-locked seed shape: seed_runs_with_status_and_exit(pool, job_id, count, status: &str, exit_code: Option<i32>) — direct INSERT INTO job_runs with config_hash='seed-hash' and explicit job_run_number from a next_run_number UPDATE...RETURNING (preserves the (job_id, job_run_number) UNIQUE INDEX constraint)"
    - "Locked-copy assertion style: each test asserts on the verbatim UI-SPEC § Copywriting Contract strings (\"Need 5+ samples; have N\", \"Most frequent codes\", \"Exit Code Distribution\", \"127 (command not found)\", \"143 (SIGTERM)\", \"NOT a crash\", \"(window: 100). Hover bars for detail.\") rather than substrings — Copywriting contract is load-bearing"
    - "Locked CSS class assertion style: cd-exit-card / cd-exit-chart / cd-exit-bar / cd-exit-bar--stopped / cd-exit-bar--warn / cd-exit-empty / cd-exit-stats / cd-exit-recent / cd-exit-bucket-label / cd-exit-stat-value asserted by substring presence; class names are LOCKED per UI-SPEC § Component Inventory § 2 and changes here would require UI-SPEC + plan 21-06 + this test to all change in lockstep"
    - "Per-label bare-quote-friendly assertion structure: each of the 10 bucket short labels gets its own `let bucket_label_X = \"...\";` binding + assert! line so grep -cE '\"1\"|\"2\"|\"3-9\"|\"127\"|\"stopped\"|\"none\"' returns >= 5 matching lines (the plan's literal acceptance regex), AND so that any single-label render regression points failure output at exactly that label"

key-files:
  created:
    - tests/v12_exit_histogram.rs
  modified: []

key-decisions:
  - "Seed helper uses direct raw-SQL INSERT (mirroring v13_duration_card.rs::seed_runs_with_duration) rather than the queries::insert_running_run + queries::finalize_run path. Raw-SQL is fully deterministic (status, exit_code, end_time all written explicitly per row); finalize_run derives duration from tokio::time::Instant::elapsed() which the histogram doesn't read but which would still produce non-deterministic timestamps. The plan's <interfaces> block locked this shape verbatim."
  - "Each row gets a distinct start_time (Utc::now() - Duration::seconds((count - i) as i64)) so ORDER BY start_time DESC is deterministic both within a single seed call AND across multiple seed calls in the same test (later seeds use later timestamps, so newest-first iteration sees the most recently seeded rows first). end_time = start + 30s gives the relative-time formatter a stable bucket (\"just now\")."
  - "next_run_number gets advanced via UPDATE...RETURNING per inserted row instead of using a fixed (i as i64) + 1 from the plan's interfaces example. The fixed-counter approach would conflict with the (job_id, job_run_number) UNIQUE INDEX when multiple seed_runs calls happen on the same job (test 4 seeds 5+5; test 7 seeds 2+2+1). The UPDATE...RETURNING pattern matches v13_duration_card.rs:118-126 verbatim and Just Works regardless of seed-call ordering."
  - "Each of the 10 bucket short labels gets its own assert! line (not a `for` loop over an array) — this satisfies the plan's literal `grep -cE '\"1\"|\"2\"|\"3-9\"|\"127\"|\"stopped\"|\"none\"' tests/v12_exit_histogram.rs returns >= 5` acceptance check (rustfmt collapses the array to a single line, so a loop would only produce 1-2 matching lines). The loop form was tried first; the per-label-let-binding form is grep-friendly without sacrificing readability and gives sharper failure output (which specific label is missing)."
  - "BucketStopped tooltip-detail copy assertion uses the substring \"NOT a crash\" rather than the full locked override sentence. The full override is `\"Stopped via UI — cronduit sent SIGKILL. Distinct from \\\"signal-killed\\\" (128-143) which captures external SIGTERM / SIGSEGV / etc.\"` (with embedded quotes). Asserting the full string would require escape-juggling; \"NOT a crash\" is the operationally meaningful copywriting contract — the rest is explanatory. The aria_label template `\"Stopped via UI (SIGKILL by cronduit, exit 137) — {N} runs. NOT a crash.\"` ALSO ends with \"NOT a crash.\" so this substring catches both surfaces in one assertion."
  - "Test 4 seeds 5+5 (10 total runs) instead of 5+5 split across two seed calls because the assertion needs both BucketStopped (status='stopped'+137) and Bucket128to143 (status='failed'+137) to render with bars — both buckets need at least 1 count to produce a non-zero `cd-exit-bar--{class}` DOM element. With 10 runs total, has_min_samples is true and both buckets contribute; with only 5 runs split 1+4, has_min_samples would still be true but the test's assertion target would shift."
  - "Recent-codes test seeds exactly 5 failed runs (1×2 + 127×1 + 143×2) hitting the N=5 threshold from below — keeps the test fast (5 inserts) AND covers the EXIT-05 top-3 ordering edge: codes 1 and 143 tie at count=2, broken by code ASC → code 1 first. The test asserts BOTH locked code labels (\"127 (command not found)\" + \"143 (SIGTERM)\") render; with only 5 rows the ordering is implicit because all three are guaranteed to appear in any rendering."

patterns-established:
  - "When a phase ships a new pre-formatted view-model card (here: ExitHistogramView with builder pattern), the integration tests live in a tests/{phase-prefix}_{feature}.rs file (here: tests/v12_exit_histogram.rs — `v12` prefix is conventional for phase 12+ test suites in this repo despite this being phase 21 work; see tests/v12_fctx_*) and assert on the rendered HTML body of GET /jobs/{job_id} via ServiceExt::oneshot, using the locked UI-SPEC § Copywriting Contract strings as substring assertion targets. View-model unit tests + categorize / aggregate unit tests stay in src/web/<module>.rs::tests; render tests stay in tests/."
  - "When a render test needs deterministic per-row data (status, exit_code, end_time), bypass the canonical queries::insert_running_run + queries::finalize_run path and INSERT directly via raw SQL. The raw-SQL helper signature mirrors v13_duration_card.rs::seed_runs_with_duration: `seed_runs_with_status_and_exit(pool, job_id, count, status: &str, exit_code: Option<i32>)`. Advance `jobs.next_run_number` via UPDATE...RETURNING per insert to preserve the UNIQUE INDEX(job_id, job_run_number) constraint regardless of seed-call ordering."
  - "When a bucket / variant / label set is LOCKED per UI-SPEC and would change in lockstep with handler + template + CSS code, render-test it by asserting EVERY locked variant individually (one assert! per label). Grep-friendly regex acceptance checks (`grep -cE '\"label1\"|\"label2\"|...' >= N`) only match individual lines; rustfmt-aware code structure must produce one line per asserted variant for the regex to be useful as a quality gate."

requirements-completed: [EXIT-01, EXIT-02, EXIT-03, EXIT-04, EXIT-05]

# Metrics
duration: ~11min
completed: 2026-05-02
---

# Phase 21 Plan 08: Exit Histogram Render-Level Integration Tests Summary

**One new test file `tests/v12_exit_histogram.rs` (558 lines) with 7 render-level integration tests locking the rendered Exit-Code Histogram card behavior on `GET /jobs/{job_id}`: empty-state below-N=5 + brand-new-job (D-15/D-16), at-threshold chart + caption + stats, EXIT-04 dual-classifier (status='stopped'+137 → BucketStopped color class vs status='failed'+137 → Bucket128to143 warn class), EXIT-03 success-rate badge with locked "SUCCESS" label + percentage display + meta line, EXIT-05 recent-codes sub-table with locked "Most frequent codes" heading + locked code labels ("127 (command not found)" / "143 (SIGTERM)"), and all 10 locked bucket short labels rendering under bar columns. Categorization math is covered by in-module unit tests in plan 21-03; this plan is RENDER-LEVEL only.**

## Performance

- **Duration:** ~11 min
- **Started:** 2026-05-02T21:06:11Z
- **Completed:** 2026-05-02T21:17:26Z
- **Tasks:** 1 (atomic-committed)
- **Files modified:** 1 (created)

## Accomplishments

- `tests/v12_exit_histogram.rs` created with 558 lines covering 7 render-level integration tests on `GET /jobs/{job_id}`:
  - **`empty_state_below_5_samples`** — 4 failed/exit=1 runs seeded; asserts the card heading "Exit Code Distribution" + outer `cd-exit-card` chrome class + locked empty-state copy "Need 5+ samples; have 4" + `cd-exit-empty` container class render, AND that `cd-exit-chart` / `cd-exit-stats` / "Most frequent codes" / "(window: 100). Hover bars for detail." caption are HIDDEN (template `{% if has_min_samples %}` short-circuit).
  - **`empty_state_brand_new_job_zero_runs`** — 0 runs seeded (D-16); asserts "Need 5+ samples; have 0" + `cd-exit-empty` render, chart + recent-codes hidden.
  - **`chart_renders_with_5_or_more_samples`** — 3 success + 2 failed/exit=1 runs (5 total = at threshold); asserts `cd-exit-chart` + at least one `cd-exit-bar` + `cd-exit-stats` + "SUCCESS" label + locked caption "(window: 100). Hover bars for detail." render, AND the empty-state copy is HIDDEN.
  - **`stopped_bucket_renders_distinct_color_class`** — 5 stopped/exit=137 + 5 failed/exit=137 runs (EXIT-04 dual-classifier RENDER level); asserts BOTH `cd-exit-bar--stopped` AND `cd-exit-bar--warn` color modifier classes render — proves the categorize() classifier emits DIFFERENT buckets for the same exit code based on status. Also asserts BucketStopped's locked tooltip / aria distinguisher copy substring "NOT a crash".
  - **`success_rate_badge_renders`** — 8 success + 2 failed/exit=1 runs; asserts "SUCCESS" stat label + `cd-exit-stat-value` class + "80%" rounded display + "8 of 10" meta line render. Asserts NO `cd-exit-bar--success` class anywhere (D-07: success has no histogram bucket — it's reported via the stat badge).
  - **`recent_codes_subtable_renders`** — 2 failed/exit=1 + 1 failed/exit=127 + 2 failed/exit=143 (5 runs, 3 codes; tie between code 1 and code 143 broken by code ASC → 1 first); asserts "Most frequent codes" heading + `cd-exit-recent` table class + locked code labels "127 (command not found)" + "143 (SIGTERM)" + standard column headers ">Code<" / ">Count<" / ">Last seen<" all render.
  - **`bucket_short_labels_render_under_bars`** — 2 failed/exit=1 + 2 failed/exit=127 + 1 stopped/exit=137 (5 runs); asserts the `cd-exit-bucket-label` class renders + each of the 10 locked short labels (`1`, `2`, `3-9`, `10-126`, `127`, `128-143`, `144-254`, `255`, `none`, `stopped`) appears inside a `<div class="cd-exit-bucket-label">{label}</div>` wrapper — proves all 10 buckets render columns even at 0 count per UI-SPEC § Component Inventory.
- Test harness `build_test_app()` mirrors `tests/v13_timeline_render.rs:32-58` verbatim: `DbPool::connect("sqlite::memory:")` → `migrate()` → `AppState` with sink `cmd_tx` → `router(state)`. Each test gets its own fresh in-memory pool (no cross-test fixture leakage).
- Seed helper `seed_runs_with_status_and_exit(pool, job_id, count, status, exit_code)` matches the plan's locked `<interfaces>` shape: raw-SQL `INSERT INTO job_runs (...)` with `config_hash='seed-hash'`, distinct per-row timestamps via `chrono::Utc::now() - Duration::seconds((count - i) as i64)`, and `next_run_number` advanced via `UPDATE...RETURNING` per insert (preserves the `(job_id, job_run_number)` UNIQUE INDEX constraint regardless of seed-call ordering).
- Each test runs in 0.03-0.08s on the local sandbox; full suite of 7 tests completes in ~0.1s after compilation. `cargo nextest run --test v12_exit_histogram` exits 0 on every run.
- Full test suite: 535 passed / 9 failed where all 9 failures are the pre-existing `SocketNotFoundError("/var/run/docker.sock")` Postgres testcontainer sandbox limitation (verified: `dashboard_jobs_pg`, `db_pool_postgres`, `schema_parity::sqlite_and_postgres_schemas_match_structurally`, all `v11_bulk_toggle_pg::*`, `v13_timeline_explain::explain_uses_index_postgres` — same set as plans 21-02 / 21-04 / 21-05 / 21-06 wave-end gates). Not regressions.
- `cargo tree -i openssl-sys` empty (D-32 rustls-only invariant holds).
- Zero new external crates added (test file uses only `axum::body`, `axum::http`, `chrono`, `tower::ServiceExt`, `tokio`, `sqlx::query_scalar`, `cronduit::*` — all already project deps).

## Task Commits

1. **Task 1: Create tests/v12_exit_histogram.rs with 7 render-level integration tests** — `2329b8b` (test)

## Files Created/Modified

**Created (1):**
- `tests/v12_exit_histogram.rs` — 558 lines: file-doc header citing UI-SPEC + Phase 21 plan IDs; `build_test_app()` harness verbatim from `v13_timeline_render.rs`; `seed_test_job(pool, name)` helper using `queries::upsert_job` for command-type jobs; `seed_runs_with_status_and_exit(pool, job_id, count, status, exit_code)` direct-INSERT helper; `get_job_detail(app, job_id)` request driver; 7 `#[tokio::test]` functions.

**Modified (0):** none.

## Decisions Made

- **Seed helper uses direct raw-SQL INSERT (mirroring `v13_duration_card.rs::seed_runs_with_duration`) rather than the canonical `queries::insert_running_run` + `queries::finalize_run` path.** Raw-SQL is fully deterministic — `status`, `exit_code`, `end_time` are all written explicitly per row. The canonical path's `finalize_run` derives `duration_ms` from `tokio::time::Instant::elapsed()` which the histogram doesn't read directly, but the elapsed-time computation produces non-deterministic intermediate timestamps that could complicate any future debugging. The plan's `<interfaces>` block locked this shape verbatim.
- **Each row gets a distinct `start_time` via `Utc::now() - Duration::seconds((count - i) as i64)`** so `ORDER BY start_time DESC` is deterministic both within a single seed call AND across multiple seed calls in the same test (later seeds get later timestamps because `Utc::now()` advances; newest-first iteration sees the most recently seeded rows first). `end_time = start + 30s` gives the relative-time formatter a stable bucket (`"just now"`).
- **`next_run_number` advances via `UPDATE...RETURNING` per inserted row** instead of using a fixed `(i as i64) + 1` from the plan's interfaces example. The fixed-counter approach would conflict with the `(job_id, job_run_number)` UNIQUE INDEX when multiple `seed_runs_with_status_and_exit` calls happen on the same `job_id` (test 4 seeds 5+5; test 7 seeds 2+2+1; test 5 seeds 8+2). The `UPDATE...RETURNING` pattern matches `v13_duration_card.rs:118-126` verbatim and works regardless of seed-call ordering.
- **Each of the 10 bucket short labels gets its own `assert!` line (not a `for` loop over an array)** — this satisfies the plan's literal `grep -cE '"1"|"2"|"3-9"|"127"|"stopped"|"none"' tests/v12_exit_histogram.rs returns >= 5` acceptance check. The loop form was tried first but rustfmt collapses the inline array `["1", "2", "3-9", ...]` to a single line, so the grep would only match 2-3 lines (false positives on `seed_runs_with_status_and_exit(..., "stopped", ...)` calls). The per-label-let-binding form (`let bucket_label_1 = "1";` + `assert!(...)`) is grep-friendly AND gives sharper failure output: any single-label render regression points failure output at exactly that label, not at a generic "labels missing" message.
- **BucketStopped tooltip-detail copy assertion uses the substring `"NOT a crash"` rather than the full locked override sentence.** The full override is `"Stopped via UI — cronduit sent SIGKILL. Distinct from \\\"signal-killed\\\" (128-143) which captures external SIGTERM / SIGSEGV / etc."` (with embedded escaped quotes). Asserting the full string requires escape-juggling that obscures the test intent. `"NOT a crash"` is the operationally meaningful copywriting contract — the rest is explanatory. The `bucket_aria_template` for BucketStopped also ends with `"NOT a crash."`, so this single substring catches both the aria-label surface AND the tooltip surface — broader coverage at lower assertion cost.
- **Test 4 seeds 10 total runs (5 stopped + 5 failed) instead of a smaller mix** because the assertion needs both `BucketStopped` (status='stopped'+137) AND `Bucket128to143` (status='failed'+137) to render with bars. Each bucket needs at least 1 count to produce a non-zero `cd-exit-bar--{class}` DOM element. With 10 runs and `has_min_samples=true`, both buckets contribute; with only 5 split 1+4, `has_min_samples` would still be true but the test's assertion target would shift (the smaller bucket might still render as a 0% height bar but the visual contract for "distinct color classes" needs both bars to be visibly present).
- **Recent-codes test seeds exactly 5 failed runs hitting N=5 from below** — keeps the test fast (5 inserts) AND covers the EXIT-05 top-3 ordering edge: codes 1 and 143 tie at count=2, broken by code ASC → code 1 first. With only 5 rows the ordering is implicit (all three codes are guaranteed to appear in any top-3 rendering), so the test asserts only the locked code labels render, NOT the order — order assertion lives in `exit_buckets::tests::top_3_codes_last_seen` (plan 21-03).
- **Bucket label assertion uses `format!(">{label}</div>")`** as the search needle rather than asserting on the bare label string. The bare strings would false-positive on incidental matches (e.g., the digit "1" appears in `"cd-exit-bar"`'s substring search context); the `>{label}</div>` shape forces the label to be inside a `<div>` close-tag, which is exactly the rendered template fragment per `templates/pages/job_detail.html:129` (`<div class="cd-exit-bucket-label">{{ bucket.short_label }}</div>`).

## Deviations from Plan

None — plan executed exactly as written.

The plan's `<action>` block specified all 7 test functions verbatim (with their seed shapes, assertion targets, and gating conditions); the plan's `<interfaces>` block locked the seed helper signature; the plan's `<read_first>` list pointed at the right reference files (`v13_duration_card.rs` for seed pattern, `v13_timeline_render.rs:32-58` for `build_test_app`, UI-SPEC § Component Inventory § 2 for class names + copy). The single task landed without auto-fix triggers:

- File written matching the locked layout; first compile + first test run pass.
- Acceptance grep checks: file exists ✓, 7 test functions defined (count=7) ✓, `exit_code` occurrences (count=8 ≥ 5) ✓, locked CSS class names (count=18 ≥ 6) ✓, locked copy (count=9 ≥ 5) ✓, file line count (558 ≥ 150) ✓, `v12_exit_histogram` substring present ✓, `oneshot|jobs/` references present ✓, bucket short labels (count=8 ≥ 5) ✓.

The bucket-short-labels grep initially returned 3 (with a `for` loop over an inline array) — this was the only acceptance check that needed iteration. Switching from the loop form to the per-label `let` + `assert!` form preserved test behavior, satisfied the literal grep regex (count=8), AND improved failure-output diagnostics. The plan's grep regex was clearly written assuming a per-label assertion structure; the loop form was the wrong-shape implementation. Tracked here as a tactical refactor, not a deviation from intent.

## Issues Encountered

- **Pre-existing clippy errors in unrelated files** — `cargo clippy --tests -- -D warnings` reports two errors on this base (`9d0ef42`):
  - `src/web/handlers/job_detail.rs:450` — "doc list item without indentation" on the line `///   `truncate(3)`.`
  - `src/web/handlers/run_detail.rs:220` — "doc list item without indentation" on the line `/// depends on (`Cargo.toml` line ~).`

  These are pre-existing on the wave-3 base and are NOT introduced by this plan. Verified by inspection of the file blame at HEAD~1 (`9d0ef42`). Per the deviation rules' SCOPE BOUNDARY, these are out-of-scope (not directly caused by this task's changes) and should be deferred to a dedicated cleanup plan or fixed by whichever future plan touches those files. Logging here for the verifier's reference.

- **Pre-existing `cargo fmt --check` diff in `src/db/queries.rs`** — Line 390 has a trailing-comment formatting drift unrelated to plan 21-08. Same out-of-scope category as the clippy issues; pre-existing on `9d0ef42`.

- **Postgres testcontainer tests cannot run in this sandbox** — same 9 tests that failed at plans 21-02 / 21-04 / 21-05 / 21-06 wave-end gates fail again here with `Client(Init(SocketNotFoundError("/var/run/docker.sock")))`. They require `testcontainers-modules::postgres::Postgres` which spins up a Postgres container via the host Docker daemon — the sandbox has no Docker daemon. All other 535 tests (including the 7 new `v12_exit_histogram` tests) pass. Postgres parity verifies on CI where Docker is available.

## User Setup Required

None — pure test-code addition, no operator-visible surface, no new env vars, no config changes. The existing `cargo nextest run --test v12_exit_histogram` (or `cargo test --test v12_exit_histogram`) commands run all 7 tests against an in-memory SQLite pool with zero external dependencies.

## Next Phase Readiness

- **Plan 21-09 (rc.2 tag cut + UAT prep)** — the histogram card's render contract is now locked at integration-test level. Any future template edit that changes a class name, copy string, or DOM structure will surface as a failed assertion in this file — making the rc.2 release boundary defensible.
- **Plan 21-10 / 21-11 (UAT + final docs)** — UAT can reference these tests as the canonical "what does the histogram render?" spec. The locked copy strings (`"Need 5+ samples; have N"`, `"Most frequent codes"`, `"127 (command not found)"`, `"143 (SIGTERM)"`, `"NOT a crash"`, `"(window: 100). Hover bars for detail."`) appear verbatim in both this file and `templates/pages/job_detail.html`; any UAT-time discovery that copy needs adjustment would require simultaneous edits to template + view-model + this test file.
- **Future histogram extensions** (e.g., adding a new bucket variant) — the test pattern established here scales: add a new `assert!(body.contains(">{new_label}</div>"))` line under `bucket_short_labels_render_under_bars` and a new render-level test if the new bucket needs distinct color or copy coverage.

## Threat Flags

None — the plan's `<threat_model>` enumerates one threat (`T-21-08-01`, n/a, accept) which remains valid as written: test code uses an in-memory SQLite pool, no production trust surface, no new operator-visible endpoints. The new test file:

- Reads only from `cronduit::*` public surfaces already shipped by plans 21-03 / 21-05 / 21-06.
- Writes only to an in-memory pool that is dropped at test end.
- Adds no new authentication / authorization / network surface.
- Adds no new dependencies (zero crate-graph impact).

No new security-relevant surface beyond what the threat model enumerates.

## Self-Check: PASSED

- File `tests/v12_exit_histogram.rs` — FOUND (558 lines)
- Commit `2329b8b` (Task 1) — FOUND in `git log --oneline -3`
- 7 test functions defined: `grep -cE "async fn (empty_state_below_5_samples|empty_state_brand_new_job_zero_runs|chart_renders_with_5_or_more_samples|stopped_bucket_renders_distinct_color_class|success_rate_badge_renders|recent_codes_subtable_renders|bucket_short_labels_render_under_bars)" tests/v12_exit_histogram.rs` returns 7
- `cargo nextest run --test v12_exit_histogram` — 7 tests run, 7 passed, 0 failed
- `exit_code` occurrences: `grep -c "exit_code" tests/v12_exit_histogram.rs` returns 8 (≥ 5)
- Locked CSS class names: `grep -c "cd-exit-card\|cd-exit-bar\|cd-exit-chart\|cd-exit-empty\|cd-exit-stats\|cd-exit-recent" tests/v12_exit_histogram.rs` returns 18 (≥ 6)
- Locked copy: `grep -c '"Need 5+ samples; have"\|"Most frequent codes"\|"Exit Code Distribution"\|"NOT a crash"\|"(window: 100)"' tests/v12_exit_histogram.rs` returns 9 (≥ 5)
- Bucket short labels: `grep -cE '"1"|"2"|"3-9"|"127"|"stopped"|"none"' tests/v12_exit_histogram.rs` returns 8 (≥ 5)
- File line count: 558 (≥ 150)
- File contains `v12_exit_histogram`: `grep -c "v12_exit_histogram" tests/v12_exit_histogram.rs` returns 1 (in the file-doc header)
- `oneshot|jobs/` references present: 5 matches (file-doc reference + ServiceExt import + AppState comment + GET helper docstring + the actual `.oneshot(` call site)
- `cargo build --tests --workspace` exits 0
- `cargo nextest run --no-fail-fast` — 535 passed / 9 failed (all 9 = `SocketNotFoundError("/var/run/docker.sock")`; sandbox limitation; same set as plans 21-02 / 21-04 / 21-05 / 21-06 wave-end gates; not regressions)
- `cargo tree -i openssl-sys` — returns "package ID specification ... did not match any packages" (D-32 invariant)
- `rustfmt --edition 2024 --check tests/v12_exit_histogram.rs` exits 0 (file properly formatted)

---
*Phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2*
*Plan: 08*
*Completed: 2026-05-02*
