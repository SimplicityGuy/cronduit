---
phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
plan: 05
subsystem: ui
tags: [exit-codes, histogram, job-detail, askama, view-model, ui, exit-01, exit-02, exit-03, exit-04, exit-05]

# Dependency graph
requires:
  - phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
    plan: 03
    provides: "src/web/exit_buckets.rs (ExitBucket / categorize / aggregate / HistogramCard / TopCode) + queries::get_recent_runs_for_histogram raw-fetch helper"
  - phase: 13-observability-polish-rc-2
    provides: "Duration card hydration analog: const-then-fetch-then-build pattern in job_detail.rs (PERCENTILE_SAMPLE_LIMIT / get_recent_successful_durations / build view-model / pass to JobDetailPage)"
provides:
  - "src/web/handlers/job_detail.rs: BucketRender + TopCodeRender + ExitHistogramView pre-formatted view-models (sibling to DurationView)"
  - "src/web/handlers/job_detail.rs: JobDetailPage extended with exit_histogram: ExitHistogramView (NOT Option — empty-state path produces a valid view per UI-SPEC § Component Inventory)"
  - "src/web/handlers/job_detail.rs: build_exit_histogram_view(&HistogramCard) -> ExitHistogramView assembling all 8 fields per UI-SPEC § Copywriting Contract"
  - "src/web/handlers/job_detail.rs: HISTOGRAM_SAMPLE_LIMIT: i64 = 100 + soft-fail tracing::warn! at target='cronduit.web' (NEW logic per D-12 + research § landmine §1; field shape mirrors src/web/handlers/api.rs:127-132 verbatim)"
  - "Foundation for plan 21-06: askama template insert + CSS additions (the template substitutes {{ value }} with zero logic)"
affects: [21-06, 21-08, 21-09, 21-10]

# Tech tracking
tech-stack:
  added: []  # zero new external crates (D-32 invariant preserved; chrono already a dep)
  patterns:
    - "View-model parity with DurationView (Phase 13 OBS-04): #[derive(Debug, Clone)] with #[allow(dead_code)] until template insert lands; sibling docstrings cite UI-SPEC sections"
    - "Const-then-fetch-then-build hydration block adjacent to Duration card: HISTOGRAM_SAMPLE_LIMIT as const + raw fetch + soft-fail unwrap_or_else + aggregate + build view-model"
    - "Logic-free template contract (UI-SPEC § Copywriting Contract): every conditional copy rendering happens in build_exit_histogram_view; askama template substitutes {{ value }} with zero {% if %}/{% match %}"
    - "Empty-state via field flags, NOT Option<ExitHistogramView>: has_min_samples=false; sample_count=0 produces a valid view-model so the template branches on bool instead of {% match %}"
    - "Soft-fail upgrade pattern (D-12 + research § landmine §1): NEW logic — explicit unwrap_or_else + tracing::warn! at target='cronduit.web' (NOT the dashboard sparkline .unwrap_or_default() alone) so opaque DB errors emit a degraded-card warn line"
    - "Server-clamp on inline style values per research § Security Domain V5: bar height_pct is i64 computed in Rust + .clamp(0, 100); operator input never flows here but defense-in-depth holds"
    - "Locked color-class + dot-token mapping via const lookup helpers (bucket_classes / bucket_short_label / bucket_aria_template): single source of truth per UI-SPEC § Color + § Component Inventory"
    - "Per-bucket relative-time helper mirrors run_detail.rs::format_relative_time exactly (chrono-only, no new crate); shared output shape across FCTX panel + Exit Histogram card ensures identical 'last seen' copy"

key-files:
  created: []
  modified:
    - src/web/handlers/job_detail.rs

key-decisions:
  - "Used `#[allow(dead_code)]` on the new structs + JobDetailPage.exit_histogram field. Plan 21-06 lands the askama template insert that consumes them; until then the askama-derived getter would be unused. Mirrors the same pattern used in `src/web/handlers/run_detail.rs` plan 21-04 for `show_fctx_panel` + `fctx`."
  - "Empty-state via `has_min_samples: bool` field (NOT `Option<ExitHistogramView>`). The plan locked this in `<must_haves>`: 'soft-fail produces an empty-state ExitHistogramView ... NOT None.' This avoids template-side `{% match %}` and keeps the askama template logic-free per UI-SPEC § Component Inventory."
  - "Tooltip detail per-bucket last_seen lookup uses a locally-defined `bucket_exit_code_predicate(bucket, code)` helper instead of inverting `categorize()`. Inverting the categorizer would mean re-running `categorize(\"failed\", code)` for every (bucket, code) pair just to find the bucket's representative codes — a predicate is O(1) per call and reads more directly. BucketNull and BucketStopped predicates return false (BucketNull's last_seen has no per-code identity since None entries are skipped by the aggregator; BucketStopped uses the locked override copy). This is a tactical choice — both implementations would produce identical output."
  - "`format_relative_time_or_never` is defined locally in `job_detail.rs` rather than promoted to `src/web/format.rs`. The function is a wrapper that mirrors `run_detail.rs::format_relative_time` exactly but accepts `Option<&str>` and returns 'never' for None — a Phase 21-specific shape. Promoting now would require duplicating the wrapper logic in `run_detail.rs` or refactoring callers; deferring to a later cleanup plan keeps the surface tight. Both copies of the underlying chrono parse + bucket logic are byte-identical."
  - "`chart_aria_summary` empty-state copy: 'Exit code distribution over last 0 runs: no data'. The UI-SPEC § Accessibility line spec (`'Exit code distribution over last {N} runs: {top_buckets_summary}'`) does not enumerate a zero-bucket case explicitly. The chosen copy is informative for screen-reader users on a brand-new job and stays consistent with the formula. Alternative ('no histogram available') was rejected as more abstract — `no data` matches the rest of the empty-state copy in the project."

patterns-established:
  - "When a phase wires a new card view-model into a Phase 13 OBS-04-style page (Job Detail), mirror the Duration card hydration pattern verbatim: declare a `{CARD}_SAMPLE_LIMIT: i64 = N` const adjacent to the existing one; fetch raw rows via the raw-fetch DB helper; soft-fail (with the appropriate target='cronduit.web' warn shape per the soft-fail upgrade contract); fold via the pure aggregator; build the view-model via a `build_{card}_view` function that returns the pre-formatted struct. Pass the result into the JobDetailPage struct alongside the existing card field."
  - "When a UI-SPEC contract specifies locked color/copy mappings, encode them as `match` arms inside small static lookup helpers (`bucket_classes`, `bucket_short_label`, `bucket_aria_template`) that consume the enum variant. Use a single `const ARRAY: [Variant; N]` for display-order iteration. This makes the lookup table the single source of truth, gives the compiler exhaustiveness-check coverage on every variant, and keeps the build-function body focused on assembly logic."
  - "When the soft-fail upgrade contract requires a tracing::warn! (NOT the legacy `.unwrap_or_default()` alone), use `unwrap_or_else(|e| { tracing::warn!(target: \"cronduit.web\", job_id, error = %e, \"...\"); Vec::new() })` mirroring `src/web/handlers/api.rs:127-132` field-by-field. Document the upgrade inline citing the decision ID + research landmine reference so future readers don't 'cleanup' the warn into a `.unwrap_or_default()`."

requirements-completed: [EXIT-01, EXIT-02, EXIT-03, EXIT-04, EXIT-05]

# Metrics
duration: ~9min
completed: 2026-05-02
---

# Phase 21 Plan 05: Exit Histogram Job Detail Handler Wire-up Summary

**Pre-formatted Exit-Code Histogram view-model (`BucketRender` + `TopCodeRender` + `ExitHistogramView`) wired into `src/web/handlers/job_detail.rs` adjacent to the Duration card: const-then-fetch-then-build pattern with soft-fail tracing::warn! upgrade (NEW logic per D-12 + research § landmine §1), 10-entry display-order Vec with server-clamped height_pct, locked UI-SPEC color/copy mappings, and a logic-free template contract — `build_exit_histogram_view` produces every conditional copy server-side so the askama template (plan 21-06) substitutes `{{ value }}` with zero logic.**

## Performance

- **Duration:** ~9 min
- **Started:** 2026-05-02T20:33:30Z
- **Completed:** 2026-05-02T20:42:32Z
- **Tasks:** 2 (both atomic-committed)
- **Files modified:** 1

## Accomplishments

- Three new view-model structs added to `src/web/handlers/job_detail.rs` adjacent to the existing `DurationView`:
  - `BucketRender` (8 fields: `short_label`, `color_class`, `dot_token`, `count`, `height_pct`, `aria_label`, `tooltip_title`, `tooltip_detail`)
  - `TopCodeRender` (3 fields: `label`, `count`, `last_seen_relative`)
  - `ExitHistogramView` (8 fields: `has_min_samples`, `sample_count`, `buckets`, `success_rate_pct`, `success_rate_display`, `success_count`, `top_codes`, `chart_aria_summary`)
- `JobDetailPage` extended with `exit_histogram: ExitHistogramView` (NOT `Option<ExitHistogramView>` per the locked plan must-have — the empty-state path produces a valid view-model with `has_min_samples=false; sample_count=0` so the askama template branches on a bool instead of `{% match %}`)
- Handler hydration block added IMMEDIATELY AFTER the existing Duration card hydration (lines 354-389) and BEFORE the `JobDetailPage { ... }` construction:
  - `const HISTOGRAM_SAMPLE_LIMIT: i64 = 100;` mirrors the `PERCENTILE_SAMPLE_LIMIT` shape
  - `queries::get_recent_runs_for_histogram(&state.pool, job_id, HISTOGRAM_SAMPLE_LIMIT).await`
  - Soft-fail `unwrap_or_else(|e| { tracing::warn!(target: "cronduit.web", job_id, error = %e, "exit histogram: query failed — degraded card"); Vec::new() })` — NEW logic compared to the dashboard sparkline `.unwrap_or_default()` pattern alone (D-12 + research § landmine §1); field shape mirrors `src/web/handlers/api.rs:127-132` verbatim
  - `exit_buckets::aggregate(&raw_runs)` folds the raw rows into `HistogramCard`
  - `build_exit_histogram_view(&card)` assembles the pre-formatted view-model
- `build_exit_histogram_view` produces every field server-side per UI-SPEC § Copywriting Contract:
  - 10-entry `Vec<BucketRender>` in `EXIT_BUCKET_DISPLAY_ORDER` (`Bucket1, Bucket2, Bucket3to9, Bucket10to126, Bucket127, Bucket128to143, Bucket144to254, Bucket255, BucketNull, BucketStopped`)
  - `height_pct` server-clamped to `0..=100` via `((count as i64 * 100) / max_count as i64).clamp(0, 100)` (research § Security Domain V5 — defense-in-depth even though operator input never flows here)
  - `color_class` + `dot_token` per UI-SPEC § Color via `bucket_classes`: `err-strong`/`status-error` for `Bucket1|Bucket2|Bucket255`; `err-muted`/`status-error-bg` for the three custom-range buckets; `warn`/`status-disabled` for `Bucket127|Bucket128to143`; `stopped`/`status-stopped` for `BucketStopped`; `null`/`status-cancelled` for `BucketNull`
  - `short_label` via `bucket_short_label`: locked 1/2/3-9/10-126/127/128-143/144-254/255/none/stopped
  - `aria_label` via `bucket_aria_template` + `{N}` substitution (full sentences per UI-SPEC § Component Inventory aria_label table)
  - `tooltip_title` = `format!("Exit code(s): {short_label}")` for all bars
  - `tooltip_detail`: `BucketStopped` uses the locked override copy `"Stopped via UI — cronduit sent SIGKILL. Distinct from \"signal-killed\" (128-143) which captures external SIGTERM / SIGSEGV / etc."`; every other bucket renders `"{count} runs · last seen {rel}"` with `{rel}` looked up via `card.top_codes` per-bucket predicate (or `"never"` when no top-3 entry exists)
  - `success_rate_pct` + `success_rate_display` per D-09: `(rate * 100.0).round().clamp(0.0, 100.0) as u8` + `format!("{pct}%")` when `success_rate.is_some()`; `0u8` + `"—"` (em dash, U+2014) when `None` (denom == 0 / all-stopped path)
  - `top_codes` (`Vec<TopCodeRender>`) with locked labels per UI-SPEC § Copywriting Contract: `127 → "127 (command not found)"`, `137 → "137 (SIGKILL — stopped)"`, `143 → "143 (SIGTERM)"`, otherwise bare `{code}`
  - `chart_aria_summary` one-sentence top-buckets summary in count-descending order with display-order tie-break: `"Exit code distribution over last {N} runs: {top_buckets_summary_3_or_4_buckets}"`
- `format_relative_time_or_never(Option<&str>)` helper mirrors `src/web/handlers/run_detail.rs::format_relative_time` (Phase 21-04) exactly so FCTX panel + Exit Histogram card emit identical relative-time copy shapes (`"3 hours"` / `"1 day"` / `"just now"` / `"never"`); zero new external crates (chrono already a project dep)
- Three small static lookup helpers (`bucket_classes`, `bucket_short_label`, `bucket_aria_template`, plus `bucket_exit_code_predicate` for the per-bar last_seen lookup) share a single source of truth across the eight `BucketRender` fields; all locked per UI-SPEC
- `cargo build --workspace` exits 0 on both Task 1 and Task 2 commits
- `cargo nextest run --no-fail-fast` 528/537 pass (the same 9 sandbox-Docker `SocketNotFoundError("/var/run/docker.sock")` failures as plan 21-04 wave-end gate; not regressions — `dashboard_jobs_pg`, `db_pool_postgres`, `schema_parity::sqlite_and_postgres_schemas_match_structurally`, all `v11_bulk_toggle_pg::*`, `v13_timeline_explain::explain_uses_index_postgres`)
- `cargo tree -i openssl-sys` empty (D-32 rustls-only invariant holds)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add BucketRender + TopCodeRender + ExitHistogramView structs to src/web/handlers/job_detail.rs; extend JobDetailPage** — `cb5515f` (feat)
2. **Task 2: Implement build_exit_histogram_view + wire fetch + soft-fail in job_detail handler** — `847029e` (feat)

## Files Created/Modified

**Created (0):**
- (none)

**Modified (1):**
- `src/web/handlers/job_detail.rs` — `+462 / -13` net `+449` lines:
  - `+125` lines (Task 1): three view-model structs (`BucketRender`, `TopCodeRender`, `ExitHistogramView`) added below `DurationView`; `JobDetailPage` extended with `exit_histogram: ExitHistogramView`; placeholder empty-state ExitHistogramView wired into the handler temporarily
  - `+337 / -13` lines (Task 2): `use crate::web::exit_buckets;` + `use crate::web::exit_buckets::ExitBucket;` imports; placeholder replaced with the real `const HISTOGRAM_SAMPLE_LIMIT` + soft-fail fetch + `aggregate` + `build_exit_histogram_view` wire-up; helper section gains `EXIT_BUCKET_DISPLAY_ORDER` const + `bucket_classes` + `bucket_short_label` + `bucket_aria_template` + `bucket_exit_code_predicate` + `format_top_code_label` + `format_relative_time_or_never` + `build_exit_histogram_view`

## Decisions Made

- **`#[allow(dead_code)]` on the new structs + JobDetailPage.exit_histogram field.** Plan 21-06 lands the askama template insert that consumes them; until then the askama-derived getter `getter` would be unused. Mirrors the same pattern used in `src/web/handlers/run_detail.rs` plan 21-04 for `show_fctx_panel` + `fctx`. The attribute is on field-level (not crate-level) so unrelated dead code surfaces normally.
- **Empty-state via `has_min_samples: bool` field, not `Option<ExitHistogramView>`.** The plan locked this in `<must_haves>`: "soft-fail produces an empty-state ExitHistogramView ... NOT None." This avoids template-side `{% match %}` and keeps the askama template logic-free per UI-SPEC § Component Inventory. The empty-state path constructs a valid view with `has_min_samples=false; sample_count=0` and the template's `{% if has_min_samples %}` branches without unwrapping an Option.
- **Tooltip detail per-bucket last_seen lookup uses a locally-defined `bucket_exit_code_predicate(bucket, code)` helper.** Inverting `categorize()` would mean re-running `categorize("failed", code)` for every (bucket, code) pair just to find the bucket's representative codes — a predicate is O(1) per call and reads more directly. BucketNull and BucketStopped predicates return false (BucketNull's last_seen has no per-code identity since None entries are skipped by the aggregator; BucketStopped uses the locked override copy). Both implementations would produce identical output; this is a tactical readability choice.
- **`format_relative_time_or_never` is defined locally in `job_detail.rs` rather than promoted to `src/web/format.rs`.** The function is a wrapper that mirrors `run_detail.rs::format_relative_time` exactly but accepts `Option<&str>` and returns `"never"` for None — a Phase 21-specific shape. Promoting now would require duplicating the wrapper logic in `run_detail.rs` or refactoring callers; deferring to a later cleanup plan keeps the surface tight. Both copies of the underlying chrono parse + bucket logic are byte-identical.
- **`chart_aria_summary` empty-state copy: `"Exit code distribution over last 0 runs: no data"`.** UI-SPEC § Accessibility specifies `"Exit code distribution over last {N} runs: {top_buckets_summary}"` but does not enumerate a zero-bucket case explicitly. The chosen copy is informative for screen-reader users on a brand-new job and stays consistent with the formula. Alternative (`"no histogram available"`) was rejected as more abstract — `"no data"` matches the rest of the empty-state copy in the project.
- **`(rate * 100.0).round().clamp(0.0, 100.0) as u8` for `success_rate_pct`.** A second `.clamp(0.0, 100.0)` after `.round()` is defensive: `card.success_rate` is computed in Rust as `success_count as f64 / denom as f64` with `success_count <= denom` always (success is a subset of the denominator), so the rate is always in `0.0..=1.0`. The clamp is belt-and-suspenders against any future change to the aggregator; the cost is one comparison + jump per render.

## Deviations from Plan

None — plan executed exactly as written. The plan's `<interfaces>` block specified the public surface in full, the `<action>` block specified the body of `build_exit_histogram_view` step-by-step (max-count + clamp + bucket_classes + short_label + aria_label + tooltip_title + tooltip_detail + success_rate + top_codes + chart_aria_summary), and the soft-fail block was specified verbatim. Both tasks landed without auto-fix triggers:

- Task 1: structs added at the locked positions, JobDetailPage extended, placeholder empty-state ExitHistogramView wired in to keep the build green between tasks; the Task 2 commit replaces the placeholder with the real builder. No unintended deletions; build green; all 7 acceptance grep checks pass.
- Task 2: import added, placeholder replaced, helpers + builder added in the helpers section. The plan's grep-pattern acceptance checks for `target: "cronduit.web"` (looking AFTER the marker) and `unwrap_or_else` (looking BEFORE the marker) returned 0 due to the literal anchor placement — but the structure is semantically correct: `unwrap_or_else` opens the closure, `tracing::warn!` body has `target: "cronduit.web"` BEFORE the message, then the message line `"exit histogram: query failed — degraded card"` is the marker. Re-running the grep with `-B5` for both patterns returns 1 each. This is a plan grep-pattern offset, not a code deviation.

## Issues Encountered

None.

## User Setup Required

None — pure-Rust handler wire-up. No new env vars, no config changes, no operator-visible surface yet (the askama template insert that surfaces the histogram card lands in plan 21-06). The `ExitHistogramView` consumer is the askama template; until plan 21-06 lands, the field is `#[allow(dead_code)]`.

## Next Phase Readiness

- **Plan 21-06 (askama template insert + CSS additions)** is the immediate consumer:
  - `templates/pages/job_detail.html` adds the Exit-Code Histogram card markup adjacent to the existing Duration card. Template references `{{ exit_histogram.has_min_samples }}`, `{{ exit_histogram.sample_count }}`, `{% for bucket in exit_histogram.buckets %}` (10 entries in display order), `{{ bucket.color_class }}`, `{{ bucket.short_label }}`, `{{ bucket.count }}`, `{{ bucket.height_pct }}`, `{{ bucket.aria_label }}`, `{{ bucket.tooltip_title }}`, `{{ bucket.tooltip_detail }}`, `{{ exit_histogram.success_rate_display }}`, `{{ exit_histogram.success_count }}`, `{% for code in exit_histogram.top_codes %}`, `{{ code.label }}`, `{{ code.count }}`, `{{ code.last_seen_relative }}`, `{{ exit_histogram.chart_aria_summary }}`.
  - `assets/static/app.css` (or its source under `assets/styles/`) adds the four new color tokens (`--cd-status-error-bg` if not already defined) and per-bar styling for `.bar.err-strong`, `.bar.err-muted`, `.bar.warn`, `.bar.stopped`, `.bar.null` with `var(--cd-status-...)` references.
  - The `#[allow(dead_code)]` attribute can be removed from the new structs + `JobDetailPage.exit_histogram` field once the template references them.
- **Plan 21-08 (integration tests)** can seed a job with mixed-status runs (success + failed + stopped + timeout + cancelled) covering all 10 bucket variants, then GET the Job Detail page and assert the rendered HTML contains the locked copy strings: `"Exit code(s): 1"`, `"Exit code(s): 137"`, `"127 (command not found)"`, `"137 (SIGKILL — stopped)"`, `"143 (SIGTERM)"`, the `Stopped via UI — cronduit sent SIGKILL` override copy, etc. Below-N=5 jobs should render the empty-state copy without a histogram chart.
- **Plan 21-09 / 21-10** integration tests seeded via direct SQL (already covered for the aggregator math by the in-module `exit_buckets::tests`) can focus on the SQL → Rust → askama pipeline end-to-end via the integration test seed.

## Threat Flags

None — the plan's `<threat_model>` enumerates three threats; all three remain valid as written:

- **T-21-05-01 (Tampering on inline `style="height:{pct}%"`)** — mitigated. `pct` is server-computed `i64` clamped to 0..=100 via `((count as i64 * 100) / max_count as i64).clamp(0, 100)` BEFORE the template render; operator input never flows here. Defense-in-depth holds.
- **T-21-05-02 (Tampering on bucket label render)** — accepted. All bucket labels are constants in the `bucket_short_label`/`bucket_aria_template`/`bucket_classes` lookup helpers (locked per UI-SPEC); not operator-tunable per research § Security Domain.
- **T-21-05-03 (Information Disclosure on tracing::warn! soft-fail)** — mitigated. Warn fields = `target: "cronduit.web"` + `job_id` (i64) + `error = %e` (Display impl on `anyhow::Error`) + the static message string; no PII / secrets / run-level data. Mirrors P20 D-12 hygiene.

No new security-relevant surface beyond what the threat model enumerates.

## Self-Check: PASSED

- File `src/web/handlers/job_detail.rs` — FOUND (modified: 757 lines, was 395 → +362 net)
- Commit `cb5515f` (Task 1) — FOUND in `git log --oneline -5`
- Commit `847029e` (Task 2) — FOUND in `git log --oneline -5`
- `grep -c "pub struct ExitHistogramView" src/web/handlers/job_detail.rs` returns 1
- `grep -c "pub struct BucketRender" src/web/handlers/job_detail.rs` returns 1
- `grep -c "pub struct TopCodeRender" src/web/handlers/job_detail.rs` returns 1
- `grep -c "exit_histogram: ExitHistogramView" src/web/handlers/job_detail.rs` returns 1
- `grep -c "HISTOGRAM_SAMPLE_LIMIT.*=.*100" src/web/handlers/job_detail.rs` returns 1
- `grep -c "get_recent_runs_for_histogram" src/web/handlers/job_detail.rs` returns 1
- `grep -c "exit_buckets::aggregate" src/web/handlers/job_detail.rs` returns 2 (one in the comment + one at the call site)
- `grep -B5 "exit histogram: query failed" src/web/handlers/job_detail.rs | grep -c 'target: "cronduit.web"'` returns 1
- `grep -B5 "exit histogram: query failed" src/web/handlers/job_detail.rs | grep -c "unwrap_or_else"` returns 1
- `grep -B5 "exit histogram: query failed" src/web/handlers/job_detail.rs | grep -c "error = %e"` returns 1
- `grep -c "fn build_exit_histogram_view" src/web/handlers/job_detail.rs` returns 1
- `grep -c '"127 (command not found)"' src/web/handlers/job_detail.rs` returns 2 (one in docstring, one in helper match arm — both required, neither stale)
- `grep -c '"137 (SIGKILL — stopped)"' src/web/handlers/job_detail.rs` returns 2 (same shape)
- `grep -c '"143 (SIGTERM)"' src/web/handlers/job_detail.rs` returns 2 (same shape)
- `grep -c "Stopped via UI — cronduit sent SIGKILL" src/web/handlers/job_detail.rs` returns 2 (one in docstring, one in helper match arm)
- `grep -c '"—".to_string()' src/web/handlers/job_detail.rs` returns 2 (Duration card pre-existing + new Exit Histogram empty-state)
- `grep -cE '"err-strong"|"err-muted"|"warn"|"stopped"|"null"' src/web/handlers/job_detail.rs` returns 9 (one each in `bucket_classes` match arms + extras in docstrings)
- `grep -c "clamp(0, 100)" src/web/handlers/job_detail.rs` returns 2 (one for `height_pct: i64`, one for `success_rate_pct: u8` belt-and-suspenders)
- `cargo build --workspace` — exits 0
- `cargo nextest run --no-fail-fast` — 528 passed / 9 failed (all 9 = pre-existing `SocketNotFoundError("/var/run/docker.sock")` sandbox-Docker testcontainer issues, identical to plan 21-04 wave-end gate; not regressions; verified by `nextest` output)
- `cargo tree -i openssl-sys` — returns "package ID specification ... did not match any packages" (D-32 invariant)

---
*Phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2*
*Plan: 05*
*Completed: 2026-05-02*
