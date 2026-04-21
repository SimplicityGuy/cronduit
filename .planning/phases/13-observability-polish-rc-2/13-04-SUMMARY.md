---
phase: 13
plan: 04
subsystem: dashboard-observability
tags: [observability, sparkline, dashboard, success-rate, OBS-03]
requirements: [OBS-03]
wave: 2
depends_on: [13-01, 13-02]
dependency_graph:
  requires:
    - "CSS selector family cd-sparkline-* (shipped by 13-01)"
    - "crate::web::format::format_duration_ms_floor_seconds (shipped by 13-01)"
    - "queries::get_dashboard_job_sparks (shipped by 13-02)"
    - "queries::DashboardSparkRow (shipped by 13-02)"
  provides:
    - "pub struct SparkCell in src/web/handlers/dashboard.rs"
    - "Extended DashboardJobView with spark_cells, spark_badge, spark_total, spark_numerator, spark_denominator"
    - "Recent column rendered on the dashboard between Last Run and Actions"
    - "T-V11-SPARK-01..06 integration tests in tests/v13_sparkline_render.rs"
  affects: []
tech-stack:
  added: []
  patterns:
    - "HashMap<i64, Vec<DashboardSparkRow>> bucketing + per-job reverse + leading-empty padding (newest-rightmost layout)"
    - "Denominator = filled - stopped (D-05 stopped-exclusion)"
    - "Threshold gate MIN_SAMPLES_FOR_RATE=5 with em-dash fallback below threshold (U+2014)"
    - "Round-half-up integer percent via ((num/den) * 100.0).round() as i64"
    - "Spark hydration BEFORE HTMX/full branch split — same view-model used for GET / and /partials/job-table"
key-files:
  created:
    - "tests/v13_sparkline_render.rs (311 lines: 6 #[tokio::test] cases + test app harness)"
  modified:
    - path: "src/web/handlers/dashboard.rs"
      exports: ["SparkCell", "DashboardJobView (extended)", "MIN_SAMPLES_FOR_RATE", "SPARKLINE_SIZE"]
      lines-added: 124
    - path: "templates/pages/dashboard.html"
      purpose: "Inserted <th>Recent</th> between Last Run and Actions"
      lines-added: 2
    - path: "templates/partials/job_table.html"
      purpose: "Inserted <td> with .cd-sparkline container + 20-cell loop + .cd-sparkline-badge"
      lines-added: 9
decisions:
  - "Used Option A (extend to_view with default-initialized spark fields, then post-hydrate in a second loop). Matches plan recommendation; minimally invasive — to_view signature unchanged."
  - "Scoped below-threshold 100% guard to fragment '>100%<' rather than bare '100%' to avoid collision with inline CSS 'width:100%' on the table style attribute. Documented inline in the test."
  - "Seeded runs in tests via canonical queries::insert_running_run + queries::finalize_run pair (finalize_run computes duration from start_instant.elapsed() — no duration_override parameter exists in the shipped API). Tests assert on status-based counts and rendered strings, not specific durations."
metrics:
  duration: "~25 minutes"
  completed: "2026-04-21"
  tasks_completed: 3
  tests_added: 6
  lines_added: 446
  lines_deleted: 2
  commits: 3
---

# Phase 13 Plan 04: Dashboard Sparkline + Success-Rate Badge Summary

One-liner: Shipped the OBS-03 dashboard Recent column — a 20-cell sparkline plus success-rate badge on every job row, with oldest-to-newest left-to-right layout, stopped-exclusion denominator (D-05), N<5 em-dash gate, and 6 integration tests covering zero-run crash-safety, sub-threshold, at-threshold, mixed, stopped-excluded, and exactly-20-cells-rendered invariants.

## Scope

Plan 13-04 is the Wave 2 primary surface for Phase 13 OBS-03. It consumes the CSS selectors and duration formatter shipped by plan 13-01 and the single-query sparkline row source shipped by plan 13-02. It extends the existing dashboard handler/view-model and both templates (page + HTMX partial) so that every job row shows an at-a-glance sparkline + success-rate badge on both the full-page GET / response and the HTMX 3-second `/partials/job-table` poll response.

## Tasks Completed

### Task 1 — SparkCell struct + extended DashboardJobView + handler hydration

- **Commit:** `22d1b14 feat(13-04): extend dashboard handler with sparkline hydration (OBS-03)`
- **Files:** `src/web/handlers/dashboard.rs` (+124 / -2)
- **Result:**
  - New public `SparkCell { kind: String, title: String }` struct
  - Extended `DashboardJobView` with five new public fields (`spark_cells`, `spark_badge`, `spark_total`, `spark_numerator`, `spark_denominator`) — all five existing fields preserved
  - `to_view` initializes new fields to safe defaults (empty vec, `—` badge, zeros)
  - Module-private constants `MIN_SAMPLES_FOR_RATE: usize = 5` and `SPARKLINE_SIZE: usize = 20`
  - Hydration loop fetches `queries::get_dashboard_job_sparks(&state.pool)` once, buckets by `job_id` into a `HashMap`, removes each per-job row vec, reverses it (rn=1 is newest → oldest-first), pads with empty cells on the left, then folds each row into a `SparkCell` with `#{N} {STATUS} {duration} {relative}` title
  - Denominator = `filled - stopped_count` (saturating_sub); below `MIN_SAMPLES_FOR_RATE=5` renders `—` (U+2014), otherwise rounds `(success/den) * 100` half-up to integer percent
  - Hydration runs BEFORE the `if is_htmx` branch split so both full-page and HTMX-partial responses render the sparkline
- **Verification:** all task-1 acceptance greps pass; `cargo build --lib`, `cargo clippy --lib -- -D warnings`, `cargo fmt --check` all green; 194 lib tests still pass

### Task 2 — Recent column header + cell in dashboard templates

- **Commit:** `eb7b09b feat(13-04): add Recent sparkline column to dashboard templates (OBS-03)`
- **Files:** `templates/pages/dashboard.html` (+2), `templates/partials/job_table.html` (+9)
- **Result:**
  - `dashboard.html`: non-sortable `<th>Recent</th>` inserted between Last Run and Actions column headers with `min-width:180px` per UI-SPEC
  - `job_table.html`: `<td>` between Last Run cell and Actions cell containing `<div class="cd-sparkline" role="img" aria-label="...">`, a 20-iteration cell loop with conditional per-cell `title` (absent on `empty` cells per UI-SPEC), and `<span class="cd-sparkline-badge">` with conditional `title` attribute active only when denominator > 0
  - Askama compile-time template check confirms bindings match `DashboardJobView` field names
- **Verification:** all task-2 acceptance greps pass; existing `tests/dashboard_render.rs` still passes (zero regression)

### Task 3 — Integration test tests/v13_sparkline_render.rs

- **Commit:** `e07ed10 test(13-04): add 6 OBS-03 sparkline integration tests`
- **Files:** `tests/v13_sparkline_render.rs` (new, 311 lines)
- **Result:** Six `#[tokio::test]` cases exercising the full phase test map for OBS-03:
  - `zero_runs_no_crash_and_em_dash_badge` (T-V11-SPARK-01)
  - `below_threshold_shows_dash` (T-V11-SPARK-02)
  - `at_threshold_all_success_hundred_percent` (T-V11-SPARK-03)
  - `mixed_runs_integer_percent` (T-V11-SPARK-04)
  - `stopped_excluded_from_denominator` (T-V11-SPARK-05, D-05 adapted)
  - `exactly_twenty_cells_rendered` (T-V11-SPARK-06)
- Each test builds the real router via `router(state)` against an in-memory SQLite pool, seeds jobs + runs with canonical `queries::*` calls, hits `GET /` via `ServiceExt::oneshot`, and scans the rendered HTML body for UI-SPEC-locked substrings.
- **Verification:**
  ```
  cargo nextest run --test v13_sparkline_render
   6 tests run: 6 passed, 0 skipped
  ```

## Signatures shipped

### New struct

```rust
pub struct SparkCell {
    /// One of: "success" | "failed" | "timeout" | "cancelled" | "stopped" | "empty"
    pub kind: String,
    /// Per-cell tooltip; empty string when kind == "empty".
    pub title: String,
}
```

### Extended view-model

```rust
pub struct DashboardJobView {
    // --- existing fields (unchanged) ---
    pub id: i64,
    pub name: String,
    pub schedule: String,
    pub resolved_schedule: String,
    pub has_random_schedule: bool,
    pub next_fire: String,
    pub last_status: String,
    pub last_status_label: String,
    pub last_run_relative: String,
    // --- new Phase 13 OBS-03 fields ---
    pub spark_cells: Vec<SparkCell>,
    pub spark_badge: String,
    pub spark_total: usize,
    pub spark_numerator: usize,
    pub spark_denominator: usize,
}
```

### Module-private constants

```rust
const MIN_SAMPLES_FOR_RATE: usize = 5;
const SPARKLINE_SIZE: usize = 20;
```

## to_view additive check

Confirmed: the `to_view(job, tz)` function's return shape was extended by five field initializers (all safe defaults) and nothing else. No existing field was modified, removed, or renamed. Phase 3 callers of `DashboardJobView` that enumerate only the pre-existing fields are unaffected. Proof: `cargo nextest run --test dashboard_render` (the Phase 3 UI-06 integration test) is green with zero assertion changes needed.

## Files Touched

| File                                      | Status   | Lines (delta) | Purpose                                                       |
|-------------------------------------------|----------|---------------|---------------------------------------------------------------|
| `src/web/handlers/dashboard.rs`           | Modified | +124 / -2     | SparkCell struct; DashboardJobView extension; hydration loop  |
| `templates/pages/dashboard.html`          | Modified | +2            | Non-sortable `<th>Recent</th>` header                         |
| `templates/partials/job_table.html`       | Modified | +9            | `<td>` with sparkline container + 20-cell loop + badge        |
| `tests/v13_sparkline_render.rs`           | Created  | +311          | 6 integration tests covering OBS-03 behavior matrix           |

Total: 1 file created, 3 files modified. +446 lines, 2 deletions.

## Verification Commands Run

### Task-level verification

All plan-required verify greps pass:

```
grep -q 'pub struct SparkCell'                    src/web/handlers/dashboard.rs  # OK
grep -q 'pub spark_cells: Vec<SparkCell>'         src/web/handlers/dashboard.rs  # OK
grep -q 'get_dashboard_job_sparks'                src/web/handlers/dashboard.rs  # OK
grep -q 'MIN_SAMPLES_FOR_RATE'                    src/web/handlers/dashboard.rs  # OK
grep -q 'SPARKLINE_SIZE: usize = 20'              src/web/handlers/dashboard.rs  # OK
grep -q 'rows.reverse()'                          src/web/handlers/dashboard.rs  # OK
grep -q 'saturating_sub(stopped_count)'           src/web/handlers/dashboard.rs  # OK
grep -q 'format!("{pct}%")'                       src/web/handlers/dashboard.rs  # OK
grep -q '"—"'                                     src/web/handlers/dashboard.rs  # OK

grep -q '>Recent</th>'                            templates/pages/dashboard.html  # OK
grep -q 'min-width:180px'                         templates/pages/dashboard.html  # OK
grep -q 'cd-sparkline-cell--{{ cell.kind }}'      templates/partials/job_table.html  # OK
grep -q 'role="img"'                              templates/partials/job_table.html  # OK
grep -q 'job.spark_badge'                         templates/partials/job_table.html  # OK
grep -q 'job.spark_cells'                         templates/partials/job_table.html  # OK
```

### Test runs (final pass)

```
$ cargo nextest run --test v13_sparkline_render
Summary 6 tests run: 6 passed, 0 skipped

    PASS zero_runs_no_crash_and_em_dash_badge
    PASS below_threshold_shows_dash
    PASS at_threshold_all_success_hundred_percent
    PASS mixed_runs_integer_percent
    PASS stopped_excluded_from_denominator
    PASS exactly_twenty_cells_rendered

$ cargo nextest run --test dashboard_render
Summary 2 tests run: 2 passed, 0 skipped  # Phase 3 regression — green
    PASS dashboard_empty_state_when_no_jobs
    PASS dashboard_renders_all_jobs_with_six_required_fields

$ cargo nextest run --lib
Summary 194 tests run: 194 passed, 0 skipped  # full lib suite — no regression
```

### Build + lint

```
$ cargo build --lib
Finished dev profile (success)

$ cargo clippy --lib --tests -- -D warnings
Finished dev profile (zero warnings)

$ cargo fmt --check
(clean)
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Integration test guard `!body.contains("100%")` collided with inline CSS**

- **Found during:** Task 3 first run of `cargo nextest run --test v13_sparkline_render`
- **Issue:** `below_threshold_shows_dash` test asserted `!body.contains("100%")` as a guard that the N<5 gate didn't fall through to rendering a percent badge. The rendered dashboard HTML contains `style="width:100%;border-collapse:collapse"` on the `<table>` tag, so the naive substring match fired a false positive.
- **Fix:** Scoped the guard to the fragment `">100%<"` which only appears inside a rendered badge (the `cd-sparkline-badge` closes with `</span>` so the value text is delimited by `>` and `<`). Documented inline in the test.
- **Files modified:** `tests/v13_sparkline_render.rs`
- **Commit:** `e07ed10` (the fix landed in the same task-3 commit; the RED discovery → fix cycle happened within the same task execution)

### No other deviations

- Task 1 implementation matches the plan's verbatim code block; only fmt-mandated reformatting of the nested match statement was applied (consistent with plan 13-01's style precedent).
- Task 2 template additions match the UI-SPEC-referenced HTML verbatim.
- Task 3 test names match the plan's exact `snake_case` specification.
- No auth gates hit (plan has no authentication surface — all changes are read-only dashboard rendering code).
- No architectural Rule-4 decisions encountered.

## Design Fidelity Check

- All 6 sparkline cell `kind` values route to the shipped `.cd-sparkline-cell--{kind}` selectors from plan 13-01: `success`, `failed`, `timeout`, `cancelled`, `stopped`, `empty`. No one-off colors; no new selectors added.
- The new `<th>Recent</th>` style mirrors the existing Actions column header's non-sortable style block verbatim, with only `min-width:180px` added per UI-SPEC.
- Sparkline container markup uses `role="img"` + `aria-label="Last {total} runs for {name}: {num} succeeded of {den} non-stopped"` verbatim from the UI-SPEC § Sparkline column copywriting contract.
- Badge `title` attribute renders `"{num} of {den} non-stopped runs"` only when denominator > 0 (avoids misleading "0 of 0" tooltip on zero-run jobs).
- Cell `title` format: `#{N} {STATUS_UPPERCASE} {duration_display} {relative_time}` — space-separated, status uppercase, duration via `format_duration_ms_floor_seconds` (plan 13-01 helper), relative via existing `format_relative_past` module-private fn (same module, no visibility change needed).
- Em-dash uses the exact U+2014 character (`—`), not a hyphen-minus or en-dash, as locked by UI-SPEC D-03.

## Threat Model Coverage

Plan 13-04's threat register (4 rows) had no `mitigate` dispositions at file level — all four threats are `accept`:

- **T-13-04-01 Info disclosure via disabled-job history:** Accepted. Sparkline query returns rows for ALL jobs with terminal runs, but the hydration loop only iterates `job_views` (built from the enabled-filtered `get_dashboard_jobs`). Disabled-job rows fall out naturally via `spark_by_job.remove(&job_view.id)` semantics — no view, no hydration. Unauthenticated web UI is a pre-existing v1 acceptance per `THREAT_MODEL.md`.
- **T-13-04-02 XSS via cell title:** Accepted (askama auto-escapes `{{ }}` interpolations). Cell title content is built from integers (`job_run_number`), known-lowercase enum strings (`status`), and a duration-formatter output — all safe.
- **T-13-04-03 DoS on large job_runs table:** Accepted. Single query with `ROW_NUMBER()` partitioned scan bounded by `rn <= 20`. Phase 6 retention prunes old rows.
- **T-13-04-04 Dashboard-poll query load:** Accepted. The additional query is bounded to `rn <= 20` and runs against the READER pool — no write contention.

No mitigation code required.

## Known Stubs

None. Every new field is wired end-to-end from the SQL query (plan 13-02) through the handler hydration loop to the askama templates. No placeholder text, no TODO/FIXME markers, no hardcoded empty data flows to the UI.

## Threat Flags

None — plan 04 adds only read-only view-model hydration + template rendering. No new network endpoints, no new auth paths, no file access, no schema changes at trust boundaries. All four threats covered by the plan's `<threat_model>` register above.

## Commits

| Task | Hash       | Message                                                                   |
| ---- | ---------- | ------------------------------------------------------------------------- |
| 1    | `22d1b14`  | `feat(13-04): extend dashboard handler with sparkline hydration (OBS-03)` |
| 2    | `eb7b09b`  | `feat(13-04): add Recent sparkline column to dashboard templates (OBS-03)` |
| 3    | `e07ed10`  | `test(13-04): add 6 OBS-03 sparkline integration tests`                    |

## Self-Check: PASSED

**Files verified:**

```
[ -f src/web/handlers/dashboard.rs ]    FOUND
[ -f templates/pages/dashboard.html ]   FOUND
[ -f templates/partials/job_table.html ] FOUND
[ -f tests/v13_sparkline_render.rs ]    FOUND
```

**Commits verified:**

```
$ git log --oneline | grep -E '22d1b14|eb7b09b|e07ed10'
e07ed10 test(13-04): add 6 OBS-03 sparkline integration tests
eb7b09b feat(13-04): add Recent sparkline column to dashboard templates (OBS-03)
22d1b14 feat(13-04): extend dashboard handler with sparkline hydration (OBS-03)
FOUND: 22d1b14
FOUND: eb7b09b
FOUND: e07ed10
```

All three task commits present on HEAD. All acceptance greps pass. `cargo build --lib`, `cargo clippy --lib --tests -- -D warnings`, `cargo fmt --check`, `cargo nextest run --test v13_sparkline_render` (6 pass), `cargo nextest run --test dashboard_render` (2 pass — no regression), and `cargo nextest run --lib` (194 pass) all green.

Plan 13-04 complete.
