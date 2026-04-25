---
phase: 11-per-job-run-numbers-log-ux-fixes
plan: 12
subsystem: web (templates: run_detail, run_history, static_log_viewer) + view models + integration tests
tags: [frontend, askama, ui-16, phase-11, ui-17, d-04, d-05, d-08, d-09]

# Dependency graph
requires:
  - phase: 11-09
    provides: `RunDetailPage.last_log_id`, `StaticLogViewerPartial.last_log_id`, `LogViewerPartial.last_log_id` view-model fields already plumbed; handler computes `last_log_id = max(ids).unwrap_or(0)`. Plan 11-12 renders them into `data-max-id="{{ last_log_id }}"` on `#log-lines` — one attribute write per template.
  - phase: 11-11
    provides: inline `htmx:sseBeforeMessage` dedupe handler reads `logLines.dataset.maxId`; `sse:run_finished` listener swaps live→static via `htmx.ajax`. Plan 11-12's `data-max-id` attribute is the server-side fuel that makes the dedupe guard do real work — without it the cursor reads "0" and every frame is accepted.
  - phase: 11-05
    provides: `DbRun.job_run_number` + `DbRunDetail.job_run_number` are SELECTed. Plan 11-12 threads them through `RunHistoryView` and `RunDetailView` (new fields landed by THIS plan) and renders them in templates.
provides:
  - templates/pages/run_detail.html — `<title>`, breadcrumb tail, and `<h1>` all render `Run #{{ run.job_run_number }}` as primary (4 touchpoints including the header's new muted suffix span). `#log-lines` carries `data-max-id="{{ last_log_id }}"`. Running-run first paint: `{% if total_logs == 0 %}` placeholder / `{% else %}` `{% include "partials/log_viewer.html" %}` — closes D-08 inline-backfill (UI-17).
  - templates/partials/run_history.html — new leftmost empty-header `<th>` + `<td>` rendering `#{{ run.job_run_number }}`, `<tr title="global id: {{ run.id }}">` row-level tooltip (D-04). Stop column stays rightmost (Phase-10 D-04 interaction preserved).
  - templates/partials/static_log_viewer.html — `#log-lines` carries `data-max-id="{{ last_log_id }}"` (one attribute addition).
  - src/web/handlers/run_detail.rs::RunDetailView — NEW field `pub job_run_number: i64` populated from `DbRunDetail.job_run_number`. Drops `#[allow(dead_code)]` from `RunDetailPage.last_log_id` and `StaticLogViewerPartial.last_log_id` — templates now consume.
  - src/web/handlers/job_detail.rs::RunHistoryView — NEW field `pub job_run_number: i64` populated from `DbRun.job_run_number`. Both call sites (`job_detail` full-page + `job_runs_partial` polling-refresh) pass it through.
  - tests/v11_log_dedupe_contract.rs — `data_max_id_rendered` (VALIDATION 11-12-01) and `run_history_renders_run_number_and_title_attr` (VALIDATION 11-12-03) stubs replaced with real bodies; `#[ignore]` removed. New `build_test_app_with_job_detail()` helper wires the job-detail route so the run_history partial can be exercised through the actual HTTP stack.
  - tests/v11_run_detail_page_load.rs — `header_renders_runnum_with_id_suffix` (VALIDATION 11-12-02) stub replaced with a real body; `#[ignore]` removed.
affects: [11-13, 11-14]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Per-job run number rendering via `{{ run.job_run_number }}` interpolation in askama templates. Global `run.id` stays as the URL key (DB-13 permalink) and as the muted hover/suffix diagnostic. Template substitution only; no new CSS classes or tokens, no new JS. Sites: `title` block, breadcrumb tail, `<h1>` primary (run_detail.html); new leftmost `<td>` cell (run_history.html)."
    - "Muted `(id N)` suffix inline on the run-detail `<h1>` using an inline `<span>` with existing design tokens (`--cd-text-secondary`, `--cd-text-base`, `--cd-space-2`). UI-SPEC § Layout & Spacing Specifics Option A — no new CSS class. The span is reading order after the primary `Run #N` and margin-left gives the 8px gap to match Phase-11 spacing contract."
    - "Row-level hover tooltip via `<tr title=\"global id: {{ run.id }}\">`. Attaches to the whole row (not the `#N` cell) so the tooltip surfaces anywhere on the row's hit-target. Screen-reader accessible via the default `title` attribute semantics; no ARIA required. D-04 hover-tooltip contract."
    - "`data-max-id` attribute as the server→client dedupe handoff. The handler (Plan 11-09) computes `last_log_id = max(ids).unwrap_or(0)`; templates render it as a single attribute on `#log-lines`; the client-side dedupe (Plan 11-11) reads `logLines.dataset.maxId` on every SSE frame. Single attribute write per template — zero new markup beyond that. D-09 dedupe contract closed end-to-end on this plan."
    - "Server-rendered inline backfill via `{% include \"partials/log_viewer.html\" %}` inside the running-run `is_running=true` branch. The included partial inherits the parent scope's `logs`, `has_older`, `next_offset`, and `run_id` — askama's default include semantics — so no extra context plumbing is needed. `{% if total_logs == 0 %}` gates on the same variable the `is_running=false` branch already uses, keeping conditional shapes consistent across both branches."
    - "Handler-to-template field addition via RunDetailView.job_run_number + RunHistoryView.job_run_number. Rule-2-blocking deviation — the plan's action body references `{{ run.job_run_number }}` but the existing view-model structs didn't expose the field. Both were added with their call sites wired to DbRun*/DbRunDetail (which already SELECT the column since Plan 11-05). No query changes needed."

key-files:
  created:
    - .planning/phases/11-per-job-run-numbers-log-ux-fixes/11-12-SUMMARY.md
  modified:
    - templates/pages/run_detail.html
    - templates/partials/run_history.html
    - templates/partials/static_log_viewer.html
    - src/web/handlers/run_detail.rs
    - src/web/handlers/job_detail.rs
    - tests/v11_log_dedupe_contract.rs
    - tests/v11_run_detail_page_load.rs

key-decisions:
  - "Extended RunDetailView + RunHistoryView with job_run_number rather than doing a template-only change. The plan's action body references `{{ run.job_run_number }}` as a direct field access, but neither view model had the field — RunDetailView only had `id`, and RunHistoryView likewise. DbRunDetail and DbRun both SELECT `job_run_number` (Plan 11-05), so the fix is one field addition per view struct plus one call-site write each. Rule-2 (missing critical functionality) + Rule-3 (blocking — template compile fails without the field)."
  - "Used Option A from UI-SPEC § Layout & Spacing Specifics for the muted `(id N)` suffix: inline `<span>` with inline `style=\"...\"` using existing tokens. Option B (named `.cd-runid-suffix` class) was explicitly acceptable per the UI-SPEC but Option A is simpler when only one template uses the pattern, and UI-SPEC's own discretion note said 'single-use suggests inline'. Zero new CSS classes — matches plan `<must_haves.truths>[5]` 'No new CSS tokens or classes (UI-SPEC strict)'."
  - "Left-header column's `<th>` label intentionally empty (no `#` in the header itself). UI-SPEC § Layout & Spacing Specifics notes the empty `<th>` is acceptable and consistent with the existing rightmost empty Stop column header. The `#N` cells read as identifiers without the column needing a label."
  - "Kept `{% if total_logs == 0 %}` as the gate for the running-run placeholder path. Plan body suggested using the same conditional shape the terminal-run branch already uses, which happens to be this exact variable. Keeps the two branches visually parallel in diff review and avoids introducing a new computed condition."
  - "Dropped `#[allow(dead_code)]` from `RunDetailPage.last_log_id` and `StaticLogViewerPartial.last_log_id` since their templates now consume the field via `data-max-id=\"{{ last_log_id }}\"`. `LogViewerPartial.last_log_id` retains the allow because `log_viewer.html` itself doesn't render the attribute — the partial is included INSIDE the run-detail `#log-lines` div which already has `data-max-id` on the wrapper, so the partial doesn't need to re-emit it. The scoped-field allow remains narrow and surgical."
  - "Added a second test helper `build_test_app_with_job_detail()` in `tests/v11_log_dedupe_contract.rs` rather than reusing `build_test_app` (which only wires run-detail). The plan's `run_history_renders_run_number_and_title_attr` hits `/jobs/{job_id}` (the job-detail page which includes the run_history partial). Route wiring is per-helper; reusing the run-detail helper would have required either 404'ing the job-detail page or adding a multiplex route to the single helper. The sibling helper is 40 lines and mirrors the existing one exactly."
  - "Single plan-level feat commit strategy for the template + view-model diffs, split into three `feat(11-12)` commits (one per touched template + its wiring). Task 1: run_history (smallest, independent diff). Task 2: run_detail (four template edits + RunDetailView field). Task 3: static_log_viewer (one-line attribute add). Task 4: tests (all three stub unlocks). Keeps each commit individually compile-clean and clippy-clean."

requirements-completed: [UI-16, UI-17, DB-13]

# Metrics
duration: ~10min
completed: 2026-04-17
---

# Phase 11 Plan 12: Template Diffs (UI-16) — Per-Job `#N` Across Log Surfaces Summary

**`templates/pages/run_detail.html` now renders `Run #{{ run.job_run_number }}` as primary text in the `<title>` block (L2), breadcrumb tail (L12), and `<h1>` header (L18) with a new muted `<span>(id {{ run.id }})</span>` inline suffix on the header using existing tokens (L19). `#log-lines` gains `data-max-id="{{ last_log_id }}"` (L89) wiring Plan 11-11's dedupe cursor to real server-side data. The running-run first-paint branch now `{% if total_logs == 0 %}` renders the placeholder / `{% else %}` `{% include "partials/log_viewer.html" %}` (L96) — closing D-08 inline-backfill. `templates/partials/run_history.html` gains a leftmost `<th>`/`<td>` pair rendering `#{{ run.job_run_number }}` (L35) with a row-level `<tr title="global id: {{ run.id }}">` tooltip (L33) per D-04. `templates/partials/static_log_viewer.html` adds `data-max-id="{{ last_log_id }}"` (L9). `RunDetailView` and `RunHistoryView` gain `pub job_run_number: i64` fields populated from `DbRunDetail.job_run_number` and `DbRun.job_run_number` (Rule-2 auto-add — plan body referenced the field directly; Plan 11-05 had already SELECTed it). Three Plan-11-12 `#[ignore]` stubs (`data_max_id_rendered`, `run_history_renders_run_number_and_title_attr`, `header_renders_runnum_with_id_suffix`) now ship with full bodies exercising the rendered HTML end-to-end. `cargo check` + `cargo clippy --all-targets -- -D warnings` + `cargo fmt --check` all clean. `cargo test --test v11_log_dedupe_contract` → 6 passed 0 ignored. `cargo test --test v11_run_detail_page_load` → 4 passed 0 ignored. `cargo test --lib` → 173 passed. Adjacent regressions (`v11_sse_log_stream`, `v11_sse_terminal_event`, `v11_run_now_sync_insert`, `v11_log_id_plumbing`, `xss_log_safety`) all pass. Task 5 is a `checkpoint:human-verify` — consolidated browser UAT for Phase 11's full UI-visible surface (template diffs landed by this plan + live dedupe + graceful run_finished transition from Plan 11-11) — pending user execution.**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-04-17T02:52:26Z
- **Completed (autonomous tasks):** 2026-04-17T03:02:14Z
- **Tasks:** 4 automated + 1 checkpoint:human-verify pending
- **Files created:** 1 (this SUMMARY.md)
- **Files modified:** 7 (3 templates + 2 handlers + 2 test files)

## Accomplishments

### Task 1 — `run_history.html` leftmost `#N` cell + row tooltip (commit `c78963e`)

- `src/web/handlers/job_detail.rs::RunHistoryView` gains `pub job_run_number: i64` with a doc comment naming Phase 11 DB-11 / UI-16 as the consumer.
- Both call sites (`job_detail` full-page handler + `job_runs_partial` polling-refresh handler) wire `job_run_number: r.job_run_number` into the view struct.
- `templates/partials/run_history.html` header row L21: new leftmost `<th>` with `width:1%` matching the rightmost empty Stop `<th>` precedent (no header label per UI-SPEC discretion).
- L33: `<tr class="..." title="global id: {{ run.id }}">` — row-level tooltip.
- L35: new leftmost `<td class="py-2 px-4" style="font-size:var(--cd-text-base);color:var(--cd-text-primary);white-space:nowrap">#{{ run.job_run_number }}</td>`.
- URL permalink unchanged (still keyed on global `run.id` per DB-13).

### Task 2 — `run_detail.html` title/breadcrumb/header + data-max-id + inline backfill (commit `a7675f9`)

- `src/web/handlers/run_detail.rs::RunDetailView` gains `pub job_run_number: i64` populated from `DbRunDetail.job_run_number` at the call site (L210).
- Drops `#[allow(dead_code)]` from `RunDetailPage.last_log_id` — template now reads it.
- Template edits (4 surgical substitutions):
  - L2: `{% block title %}Run #{{ run.job_run_number }} - Cronduit{% endblock %}`.
  - L12: breadcrumb tail `<span class="text-(--cd-text-primary)">Run #{{ run.job_run_number }}</span>`.
  - L17-20: `<h1>` with `Run #{{ run.job_run_number }}` primary + inline `<span style="font-weight:400;font-size:var(--cd-text-base);color:var(--cd-text-secondary);margin-left:var(--cd-space-2)">(id {{ run.id }})</span>` muted suffix (UI-SPEC Option A — no new CSS class).
  - L79-99: `#log-lines` gains `data-max-id="{{ last_log_id }}"` (L89). Placeholder gated behind `{% if total_logs == 0 %}`; running-run first-paint now renders `{% include "partials/log_viewer.html" %}` (L96) when `total_logs > 0` — closes D-08 inline-backfill.

### Task 3 — `static_log_viewer.html` data-max-id (commit `1575454`)

- Single attribute addition: L9 `<div id="log-lines" data-max-id="{{ last_log_id }}" style="...">`.
- Drops `#[allow(dead_code)]` from `StaticLogViewerPartial.last_log_id`.
- `LogViewerPartial.last_log_id` retains the scoped allow — `log_viewer.html` doesn't render the attribute (the wrapper div does via run_detail.html).

### Task 4 — Plan-11-12 stubs unlocked (commit `36adfb6`)

- `tests/v11_log_dedupe_contract.rs::data_max_id_rendered` (VALIDATION 11-12-01): seeds 5 log lines via `insert_log_batch`, GETs run-detail, asserts body contains `data-max-id="{N}"` where N is the last inserted id.
- `tests/v11_log_dedupe_contract.rs::run_history_renders_run_number_and_title_attr` (VALIDATION 11-12-03): seeds 3 running runs, GETs job-detail, asserts body contains `#1`, `#2`, `#3` AND `title="global id:` AND per-run `global id: {run_id}` tooltips for each seeded run. Uses the new `build_test_app_with_job_detail()` helper.
- `tests/v11_run_detail_page_load.rs::header_renders_runnum_with_id_suffix` (VALIDATION 11-12-02): seeds a single run (so `job_run_number=1`), GETs run-detail, asserts body contains both `Run #1` and `(id {run_id})`.
- `build_test_app_with_job_detail()` helper (40 lines) mirrors `build_test_app` but wires the job-detail route instead of run-detail.

### Verification gates

- `cargo check` → clean.
- `cargo clippy --all-targets -- -D warnings` → clean.
- `cargo fmt --check` → clean.
- `cargo test --test v11_log_dedupe_contract` → **6 passed; 0 failed; 0 ignored** in 0.23s (was 4 passed + 2 ignored).
- `cargo test --test v11_run_detail_page_load` → **4 passed; 0 failed; 0 ignored** in 0.04s (was 3 passed + 1 ignored).
- `cargo test --lib` → **173 passed; 0 failed**.
- `cargo test --test v11_sse_log_stream` → 2 passed (Plan 11-08 preserved).
- `cargo test --test v11_sse_terminal_event` → 2 passed (Plan 11-10 preserved).
- `cargo test --test v11_run_now_sync_insert` → 3 passed (Plan 11-06 preserved).
- `cargo test --test v11_log_id_plumbing` → 3 passed (Plan 11-07 preserved).
- `cargo test --test xss_log_safety` → 7 passed (ANSI render + HTML escape contract preserved).

## Task Commits

Each task committed atomically on branch `worktree-agent-a93bb95f` (worktree for `gsd/phase-11-context`, base `f82a374`):

1. **Task 1:** `c78963e` — `feat(11-12): add #N leftmost cell + row global-id tooltip to run_history`
2. **Task 2:** `a7675f9` — `feat(11-12): render job_run_number in run_detail title/breadcrumb/header + data-max-id + inline backfill`
3. **Task 3:** `1575454` — `feat(11-12): add data-max-id to static_log_viewer.html`
4. **Task 4:** `36adfb6` — `test(11-12): unlock Plan 11-12 stubs — data-max-id, #N cells, header (id N) suffix`
5. **Task 5 (checkpoint:human-verify):** no commit — consolidated browser UAT awaiting user execution (see `## Browser UAT` below).

## Files Created/Modified

- `templates/pages/run_detail.html` (MODIFIED, +9/-4) — 4 template edits: title, breadcrumb tail, `<h1>` with muted suffix span, `#log-lines` with `data-max-id` + inline backfill include guarded by `{% if total_logs == 0 %}`.
- `templates/partials/run_history.html` (MODIFIED, +8/-1) — new leftmost `<th>` + `<td>`, row-level `title` tooltip.
- `templates/partials/static_log_viewer.html` (MODIFIED, +1/-1) — `data-max-id="{{ last_log_id }}"` attribute on `#log-lines`.
- `src/web/handlers/run_detail.rs` (MODIFIED, +11/-10) — `RunDetailView.job_run_number: i64` field + call-site wiring; drops `#[allow(dead_code)]` on `RunDetailPage.last_log_id` and `StaticLogViewerPartial.last_log_id`.
- `src/web/handlers/job_detail.rs` (MODIFIED, +7/-0) — `RunHistoryView.job_run_number: i64` field + two call-site writes.
- `tests/v11_log_dedupe_contract.rs` (MODIFIED, +122/-6) — two real test bodies + new `build_test_app_with_job_detail()` helper.
- `tests/v11_run_detail_page_load.rs` (MODIFIED, +33/-3) — one real test body.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-12-SUMMARY.md` (NEW) — this file.

## Decisions Made

1. **Extended RunDetailView + RunHistoryView with `job_run_number`** rather than doing a template-only change. Plan body referenced `{{ run.job_run_number }}` as a direct field access, but neither view model had the field. DbRunDetail and DbRun both SELECT `job_run_number` (Plan 11-05 work), so fix is one field addition per view struct plus one call-site write. Rule-2 + Rule-3 auto-add per deviation rules.
2. **UI-SPEC Option A inline `<span>`** for the muted `(id N)` suffix on the run-detail header. Zero new CSS classes, matches plan `<must_haves.truths>[5]` "No new CSS tokens or classes (UI-SPEC strict)".
3. **Empty-label leftmost `<th>`** in run_history (no "#" character). UI-SPEC § Layout & Spacing Specifics accepts this as consistent with the existing rightmost empty Stop column header.
4. **`{% if total_logs == 0 %}` gate** for the running-run placeholder path. Reuses the same condition the terminal-run branch already uses, keeping the two branches visually parallel in diff review.
5. **Dropped `#[allow(dead_code)]`** from `RunDetailPage.last_log_id` and `StaticLogViewerPartial.last_log_id` since their templates now consume the field. Kept on `LogViewerPartial.last_log_id` because `log_viewer.html` doesn't render `data-max-id` (the wrapper `#log-lines` div on the parent page has it).
6. **New `build_test_app_with_job_detail()` helper** in `tests/v11_log_dedupe_contract.rs` for the `run_history_renders_run_number_and_title_attr` test. Route wiring is per-helper; 40 lines mirroring the existing helper exactly.
7. **Four atomic commits** (one per task) keeps each commit individually compile-clean, clippy-clean, and easily revertable.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical Functionality / Rule 3 - Blocking] `RunDetailView` lacks `job_run_number`**
- **Found during:** Task 2 (authoring the template edit).
- **Issue:** The plan body writes `{{ run.job_run_number }}` into `<title>`, breadcrumb, and `<h1>`, but `RunDetailView` (src/web/handlers/run_detail.rs:88-100) only had `id`, not `job_run_number`. Template would fail to compile — askama is compile-time type-checked.
- **Fix:** Added `pub job_run_number: i64` to `RunDetailView` with a doc comment naming UI-16 as the consumer, and wired the call site with `job_run_number: run.job_run_number` (reading from `DbRunDetail.job_run_number` which Plan 11-05 added).
- **Files affected:** src/web/handlers/run_detail.rs.
- **Verification:** `cargo check --lib` → clean; `header_renders_runnum_with_id_suffix` test passes asserting `Run #1` + `(id {run_id})`.
- **Committed in:** `a7675f9` (Task 2 commit — the field addition is part of the same feature as the template render).

**2. [Rule 2 - Missing Critical Functionality / Rule 3 - Blocking] `RunHistoryView` lacks `job_run_number`**
- **Found during:** Task 1 (authoring the template edit).
- **Issue:** The plan body writes `#{{ run.job_run_number }}` into the new leftmost `<td>`, but `RunHistoryView` (src/web/handlers/job_detail.rs:84-92) did not have the field. Same compile-time failure mode as #1.
- **Fix:** Added `pub job_run_number: i64` to `RunHistoryView` with a doc comment, wired both call sites (`job_detail` full-page handler at L167 + `job_runs_partial` polling handler at L272) with `job_run_number: r.job_run_number` from `DbRun.job_run_number` (Plan 11-05).
- **Files affected:** src/web/handlers/job_detail.rs.
- **Verification:** `cargo check --lib` → clean; `run_history_renders_run_number_and_title_attr` test passes asserting `#1`, `#2`, `#3` rendered.
- **Committed in:** `c78963e` (Task 1 commit).

**3. [Rule 3 - Blocking] `tests_common::build_test_app` variant for job-detail route does not exist**
- **Found during:** Task 4 (writing `run_history_renders_run_number_and_title_attr`).
- **Issue:** The test needs to hit `/jobs/{job_id}` (the job-detail page) to exercise the included `run_history.html` partial. The existing local `build_test_app()` in `tests/v11_log_dedupe_contract.rs` (added by Plan 11-11) only wires `/jobs/{job_id}/runs/{run_id}`. Plan pseudo-code didn't address which route to wire for this new test.
- **Fix:** Added a sibling helper `build_test_app_with_job_detail()` in the same file (40 lines). Identical shape to `build_test_app` but with `.route("/jobs/{job_id}", get(handlers::job_detail::job_detail))` instead. Reusing the existing helper would have required multi-route wiring that would slow all other tests unnecessarily.
- **Files affected:** tests/v11_log_dedupe_contract.rs (new helper function).
- **Verification:** `run_history_renders_run_number_and_title_attr` passes — all three `#N` cells and all three per-run `global id: {N}` tooltips detected in the response body.
- **Committed in:** `36adfb6` (Task 4 commit).

**All other plan body directives followed exactly.** No Rule 1 (bugs) or Rule 4 (architectural) deviations.

## Threat Flags

None beyond the plan's `<threat_model>`.

- **T-11-12-01 (XSS via job name in run_history):** Mitigated as planned. Existing askama auto-escape handles `{{ run.job_name }}`. `job_run_number` + `run.id` are `i64`; their decimal-ASCII renderings contain no HTML metacharacters, so no escape path is even exercised.
- **T-11-12-02 (Information disclosure via (id N) suffix + title tooltip):** Accepted as planned. Exposing the global `id` is existing UI behavior (URLs contain it); the inline `(id N)` suffix + `title="global id: N"` tooltip make it copy-pasteable for support discussions. Same trust class as the existing URL scheme.

No new network endpoints, no new auth paths, no new file-access patterns introduced. The `#log-lines` wrapper now carries an extra attribute (`data-max-id`) that exposes the max `job_logs.id` — a monotonic rowid, not sensitive.

## Issues Encountered

Three Rule-2/3 auto-fixed issues (all documented under Deviations from Plan):

1. `RunDetailView` lacked `job_run_number` → added field + wired call site.
2. `RunHistoryView` lacked `job_run_number` → added field + wired both call sites.
3. `build_test_app` for job-detail route didn't exist → added `build_test_app_with_job_detail()` sibling helper.

Each is documented under Deviations from Plan.

## Deferred Issues

- **Consolidated browser UAT (Task 5)** — `checkpoint:human-verify` pending user execution. This is the single UI gate for Phase 11's visible surface: verifies both THIS plan's template diffs (per-job `#N`, muted `(id N)` suffix, hover tooltips, `data-max-id` attribute) AND Plan 11-11's live dedupe + graceful `run_finished` transition (which needs `data-max-id` emitted by this plan to be meaningful). See `## Browser UAT` below.

## TDD Gate Compliance

Plan 11-12 is `type: execute` (not `type: tdd`). No explicit RED/GREEN/REFACTOR gate sequence required at the plan level. However:

- **RED (from Plan 11-00):** the three stubs `data_max_id_rendered`, `run_history_renders_run_number_and_title_attr`, `header_renders_runnum_with_id_suffix` were already present with `#[ignore]` markers (Wave-0 scaffolding).
- **GREEN (Tasks 1-3):** production template + view-model changes landed first in `c78963e`, `a7675f9`, `1575454`. After these, the three stubs COULD pass but were still `#[ignore]`.
- **GREEN (Task 4, tests):** `36adfb6` removes the `#[ignore]` markers and lands real test bodies that exercise the production changes end-to-end through axum.
- **REFACTOR:** not required; `cargo fmt --check` clean throughout.

`feat(...)` commits: `c78963e`, `a7675f9`, `1575454`.
`test(...)` commit: `36adfb6`.

## User Setup Required

None for the autonomous tasks. Browser UAT (Task 5) needs:
- Local dev server running against a populated SQLite DB with at least one job having > 5 historical runs.
- User executes the 5-part verification script in the plan's Task 5 `<how-to-verify>`.

## Browser UAT

**Task 5 is a `checkpoint:human-verify` — STOPPED and awaiting user verification.** This is the consolidated UI gate for Phase 11 per the plan's autonomy resolution.

Scope of the consolidated UAT:
- **Template changes (THIS plan):** Run-history `#N` leftmost cell + global-id tooltip; run-detail `Run #N` in title/breadcrumb/header + muted `(id N)` suffix; `#log-lines[data-max-id]` attribute present on running-run and static-run paths.
- **Dedupe + run_finished behavior (Plan 11-11 + THIS plan together):** Client-side dedupe drops SSE frames with `id <= data-max-id` (requires `data-max-id` from THIS plan to do real work); `sse:run_finished` listener swaps live view to static partial on run completion; no transient "error getting logs" flash on immediate navigation after Run Now (Plan 11-06 UI-19 fix).

The plan's `<how-to-verify>` section (Task 5) contains five test scripts covering:
1. Run history `#N` + global-id tooltip (UI-16).
2. Run detail header `Run #N` + muted `(id N)` (UI-16).
3. `data-max-id` attribute on `#log-lines` (Plan 11-12 + 11-11 wired together).
4. Live dedupe + run_finished transition (Plan 11-11 behavior, verifiable only now).
5. No "error getting logs" race (UI-19, Plan 11-06).

**Resume signal:** user types "verified" if all five tests pass; "issue: [description]" otherwise.

## Next Phase Readiness

- **UI-16 fully closed** end-to-end. Per-job `#N` renders in every run-surface template (run-history table + run-detail title/breadcrumb/header). Global `id` stays as URL key (DB-13 permalink scheme preserved) + muted hover/suffix diagnostic.
- **UI-17 closed** — page-load backfill inline-renders when `total_logs > 0` in the running-run branch of run_detail.html. Operator sees persisted lines on first paint instead of `Waiting for output...` + an SSE race to the first live frame.
- **D-08 + D-09 (server+client dedupe contract) fully closed** end-to-end: server emits `id:` per SSE frame (Plan 11-08) + computes `last_log_id` in handler (Plan 11-09) + renders `data-max-id` in template (Plan 11-12) + client dedupe guard (Plan 11-11) compares lastEventId against dataset.maxId.
- **D-04 (row-level global-id tooltip) closed** via `<tr title="global id: {N}">` in run_history.
- **D-05 (muted id suffix on run-detail header) closed** via inline `<span>(id {N})</span>`.
- **Plan 11-13 (whatever remains in Wave 12+) unblocked.** No dependency chain blocked by Plan 11-12 since the template diffs are terminal consumers of Plan 11-09's view-model plumbing.
- **Plan 11-14 (if applicable) unblocked.** All rendering sites have the data-max-id cursor wired, so any future backfill/reconnect-dedupe plan can plug directly into the existing cursor.

## Self-Check: PASSED

**Files verified on disk:**
- `templates/pages/run_detail.html` — FOUND. Lines 2, 12, 18 render `Run #{{ run.job_run_number }}`. Line 19 renders muted `(id {{ run.id }})` span. Line 89 has `data-max-id="{{ last_log_id }}"`. Line 96 has `{% include "partials/log_viewer.html" %}` inside the running-run `{% else %}` branch.
- `templates/partials/run_history.html` — FOUND. Line 21 new leftmost empty `<th width:1%>`. Line 33 `<tr title="global id: {{ run.id }}">`. Line 35 `#{{ run.job_run_number }}` cell.
- `templates/partials/static_log_viewer.html` — FOUND. Line 9 `<div id="log-lines" data-max-id="{{ last_log_id }}" style="...">`.
- `src/web/handlers/run_detail.rs` — FOUND. `RunDetailView.job_run_number: i64` field + call-site write. No remaining `#[allow(dead_code)]` on `RunDetailPage.last_log_id` or `StaticLogViewerPartial.last_log_id`.
- `src/web/handlers/job_detail.rs` — FOUND. `RunHistoryView.job_run_number: i64` field + both call sites writing `job_run_number: r.job_run_number`.
- `tests/v11_log_dedupe_contract.rs` — FOUND. `data_max_id_rendered` and `run_history_renders_run_number_and_title_attr` no longer `#[ignore]`. New `build_test_app_with_job_detail()` helper.
- `tests/v11_run_detail_page_load.rs` — FOUND. `header_renders_runnum_with_id_suffix` no longer `#[ignore]`.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-12-SUMMARY.md` — FOUND (this file).

**Commits verified (present in `git log f82a374..HEAD`):**
- `c78963e` — FOUND (`feat(11-12): add #N leftmost cell + row global-id tooltip to run_history`)
- `a7675f9` — FOUND (`feat(11-12): render job_run_number in run_detail title/breadcrumb/header + data-max-id + inline backfill`)
- `1575454` — FOUND (`feat(11-12): add data-max-id to static_log_viewer.html`)
- `36adfb6` — FOUND (`test(11-12): unlock Plan 11-12 stubs — data-max-id, #N cells, header (id N) suffix`)

**Build gates verified:**
- `cargo check --lib` — CLEAN.
- `cargo clippy --all-targets -- -D warnings` — CLEAN.
- `cargo fmt --check` — CLEAN.
- `cargo test --lib` — PASS (`173 passed; 0 failed`).
- `cargo test --test v11_log_dedupe_contract` — PASS (`6 passed; 0 failed; 0 ignored`). Was 4 passed + 2 ignored before this plan.
- `cargo test --test v11_run_detail_page_load` — PASS (`4 passed; 0 failed; 0 ignored`). Was 3 passed + 1 ignored before this plan.
- `cargo test --test v11_sse_log_stream` — PASS (`2 passed`).
- `cargo test --test v11_sse_terminal_event` — PASS (`2 passed`).
- `cargo test --test v11_run_now_sync_insert` — PASS (`3 passed`).
- `cargo test --test v11_log_id_plumbing` — PASS (`3 passed`).
- `cargo test --test xss_log_safety` — PASS (`7 passed`).

**Plan success criteria verified:**
1. Run-history rows render `#{{ run.job_run_number }}` with row-level global-id tooltip — PASS (line 35 + line 33 of run_history.html; `run_history_renders_run_number_and_title_attr` asserts).
2. Run-detail title / breadcrumb / header all use job_run_number; header has muted (id X) suffix — PASS (lines 2, 12, 18, 19 of run_detail.html; `header_renders_runnum_with_id_suffix` asserts).
3. `#log-lines` on both running-run and static-run paths carry `data-max-id` — PASS (run_detail.html L89 + static_log_viewer.html L9; `data_max_id_rendered` asserts the running-run path).
4. No new CSS tokens or classes added (UI-SPEC strict) — PASS (grep of `assets/static/app.css` and templates shows zero new class definitions; inline styles reuse existing `--cd-*` tokens exclusively).
5. Browser UAT confirms visual correctness AND live dedupe AND graceful run_finished transition — PENDING user execution (Task 5 `checkpoint:human-verify`).

---
*Phase: 11-per-job-run-numbers-log-ux-fixes*
*Completed (autonomous tasks): 2026-04-17*
*Browser UAT: PENDING (Task 5 checkpoint:human-verify)*
