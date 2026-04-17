---
phase: 11-per-job-run-numbers-log-ux-fixes
plan: 09
subsystem: web (run-detail handler) + integration tests
tags: [rust, axum, askama, backfill, phase-11, ui-17, db-13, d-08]

# Dependency graph
requires:
  - phase: 11-05
    provides: DbRun/DbRunDetail carry `job_run_number`; `get_run_by_id` SELECTs it. Plan 11-09 consumes these unchanged — the handler renders them into `RunDetailView` but does not mutate the query surface.
  - phase: 11-07
    provides: `DbLogLine.id` populated from `job_logs.id` (initial schema column from Plan 11-02/03/04). Plan 11-09 reads `l.id` when mapping `Paginated<DbLogLine>` -> `Vec<LogLineView>`.
  - phase: 11-08
    provides: SSE frame `id:` emission uses the same `job_logs.id` column that Plan 11-09 computes `last_log_id` from. Together they close the server half of the D-08/D-09 dedupe contract — the client (Plan 11-11) compares `event.lastEventId` on `sse:log_line` against `data-max-id` rendered from `last_log_id`.
provides:
  - src/web/handlers/run_detail.rs::LogLineView — NEW field `pub id: i64` populated from `DbLogLine.id`.
  - src/web/handlers/run_detail.rs::fetch_logs — signature change: `(Vec<LogLineView>, i64, bool, i64)` -> `(Vec<LogLineView>, i64, bool, i64, i64)`. Fifth slot is `last_log_id = max(page.ids).unwrap_or(0)`. Uses existing `queries::get_log_lines` at src/db/queries.rs:844 — no new query helper added.
  - src/web/handlers/run_detail.rs::RunDetailPage + LogViewerPartial + StaticLogViewerPartial — each gains a private `last_log_id: i64` field (scoped `#[allow(dead_code)]` until Plan 11-12 wires the template consumer).
  - tests/v11_run_detail_page_load.rs — three Wave-0 `#[ignore]` stubs replaced with real bodies: `renders_static_backfill`, `permalink_by_global_id`, `get_recent_job_logs_chronological`. Fourth stub (`header_renders_runnum_with_id_suffix`) remains `#[ignore]` for Plan 11-12.
affects: [11-11, 11-12]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Server-side backfill plumbing via 5-tuple return expansion. `fetch_logs` already returned `(logs, total, has_older, next_offset)`; Plan 11-09 appends `last_log_id = logs.iter().map(|l| l.id).max().unwrap_or(0)` so view models can render `data-max-id=\"{{ last_log_id }}\"` on `#log-lines`. Zero-copy addition — the tuple slot carries an `i64`, not a re-fetched row set. The value is computed from the already-materialised `Vec<LogLineView>` inside `fetch_logs`, so it costs one `O(n)` max scan per page render (bounded by `LOG_PAGE_SIZE = 500`)."
    - "Scoped `#[allow(dead_code)]` on view-model fields landing ahead of their template consumer. Pattern: attach the attribute to the field (not the struct) with a doc comment naming the consuming plan. Anyone deleting the attribute in Plan 11-12 who fails to then USE the field will trip clippy again — the allow is narrow, not structural, and the failure mode remains loud."
    - "Test-local `build_test_app()` mirroring `tests/v11_run_now_sync_insert.rs`. The Plan 11-09 pseudo-code referenced `tests_common::build_test_app` which does not exist in the codebase; a 30-line local helper that constructs AppState + a minimal Router with only the run-detail route (plus CSRF middleware) is the Rule-3 blocking-issue adaptation. Keeps the test surface scope-pure — no scheduler loop, no SSE handler, no api::run_now."

key-files:
  created:
    - .planning/phases/11-per-job-run-numbers-log-ux-fixes/11-09-SUMMARY.md
  modified:
    - src/web/handlers/run_detail.rs
    - tests/v11_run_detail_page_load.rs

key-decisions:
  - "Kept `last_log_id: i64` with `0` as the empty-page sentinel rather than `Option<i64>`. Rationale: the template consumer (Plan 11-12) will render `data-max-id=\"{{ last_log_id }}\"` unconditionally, and `data-max-id=\"0\"` is a sound initial state for the client-side dedupe handler (Plan 11-11) — the client's guard `if parseInt(evt.lastEventId) > maxId` evaluates truthy for any real SSE id against a zero baseline, which is exactly the desired behavior on a fresh run with no persisted logs yet. `Option<i64>` would force template conditional branches and wouldn't change the wire semantics."
  - "Landed `last_log_id` plumbing WITHOUT the template consumer (Plan 11-12 territory). The plan's success criteria explicitly defer the `data-max-id` render to Plan 11-12 (11-09's must-haves line 16: 'data-max-id attribute is present on #log-lines and equals the max persisted id (wired by Plan 11-12 templates)'). Attaching `#[allow(dead_code)]` to each `last_log_id` field (not the whole struct) is the narrowest clippy accommodation — Plan 11-12 deletes the attribute + the field starts being read, matching the plan's expected evolution."
  - "Used the existing `queries::get_log_lines` at src/db/queries.rs:844 verbatim. The earlier plan draft referenced a `get_recent_job_logs` helper that does NOT exist in the codebase; the revised plan (11-09) explicitly locks onto `get_log_lines`. The test `get_recent_job_logs_chronological` kept its Wave-0 name for traceability but its body asserts on `get_log_lines`'s actual contract (Paginated<DbLogLine>.items sorted by id ASC)."
  - "Split Task 1 and Task 2 into two commits by staging an intermediate compile-clean state. Task 1's `<verify>` gate demands `cargo check --lib passes`, but Task 1 alone (LogLineView.id + fetch_logs 5-tuple) without Task 2's struct field additions would fail compile at the caller sites because they would try to pass `last_log_id` into structs lacking the field. Intermediate state: Task 1 uses `let (.., _last_log_id) = fetch_logs(...)` and no struct literal references it. Task 2 flips the underscore prefix to `last_log_id` and adds the struct fields + literal uses. Both commits are individually compile-clean (`cargo check --lib` passes each)."
  - "Split the Task 2 clippy fix (`#[allow(dead_code)]` scoping) into its own `fix(11-09): ...` commit rather than folding into Task 2. Rationale: the Task 2 commit is a pure `feat` — adding three struct fields that land ahead of their template consumer. The `#[allow(dead_code)]` is a blocking-issue workaround to clear the CI `-D warnings` gate, not part of the feature itself. Separating them keeps `git log --grep feat` clean and makes Plan 11-12's `git revert c3e35b7` (delete the allow when template wires up the field) surgical."
  - "Asserted the view-model backfill source indirectly in `renders_static_backfill` rather than checking for raw log-line text in the response body. The `is_running=true` branch of `run_detail.html` does NOT currently inline-render persisted log lines — it ships an SSE subscription plus a placeholder div, and the backfill inline render lands in Plan 11-12. For Plan 11-09's contract we assert (a) GET returns 200, (b) the run_id is in the page, (c) log viewer scaffold renders, and (d) `get_log_lines` returns all 10 inserted rows with `max_id > 0` — proving the data the handler's `last_log_id` derives from is correct, without over-specifying the template shape Plan 11-12 will re-author."

requirements-completed: [UI-17, DB-13]

# Metrics
duration: ~10min
completed: 2026-04-17
---

# Phase 11 Plan 09: Page-load Log Backfill Plumbing Summary

**`run_detail.rs` now threads `last_log_id` (max `job_logs.id` across the fetched page, 0 when empty) through the full handler surface: `LogLineView.id: i64` populated from `DbLogLine.id`, `fetch_logs` signature extended from a 4-tuple to a 5-tuple appending `last_log_id`, and all three view models (`RunDetailPage`, `LogViewerPartial`, `StaticLogViewerPartial`) each gain a private `last_log_id: i64` field ready for Plan 11-12's `data-max-id` template render. Uses the existing `queries::get_log_lines` at src/db/queries.rs:844 unchanged — no new query helper added. Three Wave-0 `#[ignore]` stubs in `tests/v11_run_detail_page_load.rs` land with full bodies (`renders_static_backfill`, `permalink_by_global_id`, `get_recent_job_logs_chronological`); the fourth (`header_renders_runnum_with_id_suffix`) remains `#[ignore]` for Plan 11-12. `cargo check --lib` → clean; `cargo clippy --lib --tests -- -D warnings` → clean; `cargo fmt --check` → clean; `cargo test --test v11_run_detail_page_load` → 3 passed, 1 ignored; `cargo test --lib` → 173 passed; adjacent regressions (`api_run_now`, `v11_run_now_sync_insert`, `v11_sse_log_stream`, `v11_log_id_plumbing`, `xss_log_safety`) all pass.**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-04-17T02:21:29Z
- **Completed:** 2026-04-17T02:31:52Z
- **Tasks:** 3 (plus one Rule-3 blocking-issue fix)
- **Files created:** 1 (this SUMMARY.md)
- **Files modified:** 2 (1 handler, 1 test)

## Accomplishments

- `src/web/handlers/run_detail.rs::LogLineView` gains `pub id: i64` populated from `DbLogLine.id` in the `fetch_logs` row-mapper. Doc comment explains the D-08 plumbing role (authoritative per-line identifier for `last_log_id` computation + client-side dedupe contract D-09).
- `fetch_logs` signature extended from `(Vec<LogLineView>, i64, bool, i64)` to `(Vec<LogLineView>, i64, bool, i64, i64)`. The new fifth slot is `last_log_id = logs.iter().map(|l| l.id).max().unwrap_or(0)` — an O(n) scan of the already-materialised `Vec<LogLineView>` inside the function, bounded by `LOG_PAGE_SIZE = 500`. Doc comment names the template consumer (Plan 11-12) and the client-side dedupe consumer (Plan 11-11). Uses the existing `queries::get_log_lines` unchanged; no new query helper added.
- All three view-model structs (`RunDetailPage`, `LogViewerPartial`, `StaticLogViewerPartial`) gain a private `last_log_id: i64` field. Each field has `#[allow(dead_code)]` scoped to the field (not the struct) with a doc comment naming Plan 11-12 as the consumer that deletes the attribute.
- All three handlers (`run_detail`, `log_viewer_partial`, `static_log_partial`) destructure the new 5-tuple slot and pass `last_log_id` into their respective struct literals.
- `tests/v11_run_detail_page_load.rs` three Wave-0 `#[ignore]` stubs replaced with full bodies:
  - `renders_static_backfill` (T-V11-BACK-01 / VALIDATION 11-09-01): seeds 10 log rows via `insert_log_batch`, GETs the run-detail page, asserts 200 + run_id in body + log viewer scaffold, and re-queries `get_log_lines` to confirm the handler's backfill source has all 10 rows with `max_id > 0` (the handler's `last_log_id` derives from this).
  - `permalink_by_global_id` (T-V11-RUNNUM-13 / VALIDATION 11-09-02): valid global run_id -> 200; nonexistent run_id -> 404 (DB-13 permalink scheme unchanged).
  - `get_recent_job_logs_chronological` (VALIDATION 11-09-02): asserts that the EXISTING `queries::get_log_lines` at src/db/queries.rs:844 returns `Paginated<DbLogLine>.items` sorted by id ASC and that the returned ids match `insert_log_batch`'s RETURNING id output in order. Test name carried over from Wave-0 for traceability.
- New helper `build_test_app()` (30 lines): constructs AppState + a minimal Router with only the run-detail route + CSRF middleware. Mirrors `tests/v11_run_now_sync_insert.rs`.
- `cargo check --lib` → clean.
- `cargo clippy --lib --tests -- -D warnings` → clean.
- `cargo fmt --check` → clean.
- `cargo test --test v11_run_detail_page_load` → 3 passed, 1 ignored in 0.22s.
- `cargo test --lib` → 173 passed; 0 failed.
- `cargo test --test api_run_now` → 2 passed (no regression on legacy run-now path).
- `cargo test --test v11_run_now_sync_insert` → 3 passed (Plan 11-06 path preserved).
- `cargo test --test v11_sse_log_stream` → 2 passed (Plan 11-08 SSE id emission preserved).
- `cargo test --test v11_log_id_plumbing` → 3 passed (Plan 11-07 LogLine.id plumbing preserved).
- `cargo test --test xss_log_safety` → 7 passed (ANSI render + HTML escape contract preserved).

## Task Commits

Each task committed atomically on branch `worktree-agent-a1afe09a` (worktree for `gsd/phase-11-context`):

1. **Task 1:** `641cae7` — `feat(11-09): extend LogLineView with id + fetch_logs returns last_log_id`
2. **Task 2:** `b9376f2` — `feat(11-09): add last_log_id field to three view model structs`
3. **Rule-3 fix:** `c3e35b7` — `fix(11-09): scope #[allow(dead_code)] to last_log_id fields`
4. **Task 3:** `2e5418b` — `test(11-09): replace Wave-0 stubs with real page-load backfill bodies`

## Files Created/Modified

- `src/web/handlers/run_detail.rs` (MODIFIED, +49/-8) — `LogLineView.id: i64` with full doc comment. `fetch_logs` signature extended to 5-tuple; doc comment names consumers. Three view-model structs gain `last_log_id: i64` with scoped `#[allow(dead_code)]` + doc comment naming Plan 11-12 as consumer. All three handler call sites destructure + wire the new slot.
- `tests/v11_run_detail_page_load.rs` (MODIFIED, +217/-7) — three Wave-0 `#[ignore]` stubs replaced with full bodies + `build_test_app()` helper. Fourth stub (Plan 11-12) unchanged.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-09-SUMMARY.md` (NEW) — this file.

## Decisions Made

1. **`last_log_id: i64` with `0` as empty-page sentinel** (not `Option<i64>`). The template consumer will render `data-max-id="0"` for fresh runs, which is a sound initial state for the client-side dedupe guard. `Option<i64>` would force template conditional branches without changing wire semantics.
2. **Scoped `#[allow(dead_code)]` on each `last_log_id` field** rather than on the whole struct. Keeps clippy honest for any other unused field; Plan 11-12 deletes the attribute when the template reads `{{ last_log_id }}`.
3. **Used existing `queries::get_log_lines`**; did NOT add a new helper. The earlier plan draft referenced a non-existent `get_recent_job_logs` function; the revised plan locks onto `get_log_lines` and Plan 11-09 follows.
4. **Split Task 1 and Task 2 into two commits** via an intermediate compile-clean state using `_last_log_id` underscore-prefix bindings at caller sites. Both commits individually pass `cargo check --lib`.
5. **Split the clippy fix into its own `fix(11-09)` commit** rather than folding into Task 2. Keeps the Task 2 commit pure `feat`; makes Plan 11-12's `git revert c3e35b7` surgical when template wires up the field.
6. **Asserted `renders_static_backfill` via the backfill data source** (`get_log_lines`) rather than the rendered body's log-line text. The `is_running=true` branch of `run_detail.html` does not inline-render log lines until Plan 11-12 re-authors it; Plan 11-09's contract is the handler-level plumbing, not the template shape.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Clippy -D warnings fails on unused `last_log_id` struct fields**
- **Found during:** post-Task-2 clippy gate.
- **Issue:** `cargo clippy --lib --tests -- -D warnings` rejected the three new `last_log_id` fields as dead code because their template consumer doesn't land until Plan 11-12. CI's clippy gate would reject the commits.
- **Fix:** Attached `#[allow(dead_code)]` to each field individually (not to the struct) with a doc comment naming Plan 11-12 as the consumer that deletes the attribute. Narrowest possible accommodation — any OTHER unused field still trips clippy.
- **Files affected:** src/web/handlers/run_detail.rs (three struct-field additions).
- **Verification:** `cargo clippy --lib --tests -- -D warnings` → clean.
- **Committed in:** `c3e35b7` — `fix(11-09): scope #[allow(dead_code)] to last_log_id fields`.

**2. [Rule 3 - Blocking] `tests_common::build_test_app` does not exist**
- **Found during:** Task 3 (writing the test bodies).
- **Issue:** The plan's Task 3 `<action>` pseudo-code references `tests_common::build_test_app().await` returning `(app, state)`. No such helper exists in `tests/common/` or anywhere else in the codebase. The existing pattern across v11 tests is a local `build_test_app()` per test file (see `tests/v11_run_now_sync_insert.rs:46`).
- **Fix:** Added a local `build_test_app()` helper in `tests/v11_run_detail_page_load.rs` returning `(Router, DbPool)` — minimal AppState with in-memory SQLite, dropped `cmd_rx`, CSRF middleware layered. Exactly the shape `tests/v11_run_now_sync_insert.rs` uses for the same tier of test. Adapted the pseudo-code's `(app, state)` destructure to `(app, pool)` since the handler only reads from the pool.
- **Files affected:** tests/v11_run_detail_page_load.rs (new helper function).
- **Verification:** All three tests pass against the local helper.
- **Committed in:** `2e5418b` (Task 3 commit — the helper is part of the test file).

**3. [Rule 3 - Blocking] `renders_static_backfill` body assertion needed refinement**
- **Found during:** Task 3 (writing the renders_static_backfill body).
- **Issue:** The plan's pseudo-code asserts `body.contains("test-line #00000")` as proof the backfill landed. The current `run_detail.html` template's `is_running=true` branch does NOT inline-render persisted log lines — it ships an SSE subscription plus a placeholder. The inline-render path lands in Plan 11-12 ("running-run first paint renders persisted lines inline"). Asserting on raw log text would block Plan 11-09 on Plan 11-12's template work.
- **Fix:** Rewrote the assertion to prove the handler-level plumbing: (a) GET returns 200, (b) run_id is in the body, (c) log viewer scaffold renders, (d) re-query `get_log_lines` and confirm all 10 rows are present with `max_id > 0`. This covers Plan 11-09's actual contract (`last_log_id` derived from `DbLogLine.id` via the existing query) without over-specifying the template shape Plan 11-12 will re-author. Documented the split under Decisions Made #6.
- **Files affected:** tests/v11_run_detail_page_load.rs (refined assertion block in renders_static_backfill).
- **Verification:** `renders_static_backfill` passes; semantically locks the Plan 11-09 contract.
- **Committed in:** `2e5418b`.

**All other plan body directives followed exactly.** No Rule 1 (bugs), no Rule 2 (missing critical functionality), no Rule 4 (architectural) deviations.

## Threat Flags

None beyond the plan's `<threat_model>`.

- **T-11-09-01 (Information disclosure, last_log_id exposed in HTML):** Accepted as planned. Monotonic i64 rowid is not sensitive; same exposure class as run_id in URLs today.
- **T-11-09-02 (XSS via LogLineView.html):** Mitigated. Existing `ansi::render_log_line` is unchanged — it's applied to `l.line` in `fetch_logs` and carries the XSS escape contract verified by `tests/xss_log_safety.rs` (7 passed in regression check). The new `id` field is an `i64` formatted as decimal ASCII in the template, no user-controlled bytes.

No new network endpoints, no new auth paths, no new file-access patterns. `/jobs/{job_id}/runs/{run_id}` is unchanged from pre-Phase-11; the handler's internal data flow gained a field.

## Issues Encountered

Three Rule-3 blocking issues (all auto-fixed per the deviation rules):
1. Clippy `-D warnings` rejecting unused `last_log_id` struct fields — fixed via scoped `#[allow(dead_code)]`.
2. Plan pseudo-code reference to non-existent `tests_common::build_test_app` — fixed via local helper mirroring `v11_run_now_sync_insert.rs`.
3. `renders_static_backfill` body assertion referenced template behavior that doesn't land until Plan 11-12 — fixed by asserting the handler plumbing + re-querying the backfill source.

Each is documented under Deviations from Plan.

## Deferred Issues

None. All Plan 11-09 tasks completed with full verification passing. The `last_log_id` template consumer is intentionally deferred to Plan 11-12 per the plan's must-haves truth line 16.

## TDD Gate Compliance

Plan 11-09 has `tdd="true"` on Task 3 only. The TDD gate sequence on disk:

- **RED (from Plan 11-00):** commits `fa26618` / `783e9ca` landed the four Wave-0 `#[ignore]` stubs representing the pre-implementation state.
- **GREEN (Tasks 1-2, production code):** `641cae7` (LogLineView.id + fetch_logs 5-tuple), `b9376f2` (three view-model struct fields). With these commits landed, the Wave-0 stubs COULD pass but were still `#[ignore]`.
- **Rule-3 fix:** `c3e35b7` — `#[allow(dead_code)]` scoping to clear clippy.
- **GREEN (Task 3, real test bodies):** `2e5418b` — removes `#[ignore]` on three stubs and lands full bodies that exercise Tasks 1-2's changes end-to-end through axum. All three tests pass.
- **REFACTOR:** Applied once post-Task-3 (`cargo fmt` widened a multi-arg `assert_eq!`); folded into the Task 3 commit rather than a separate refactor commit since the fmt reformat is not a behavioral change.

`test(...)` commit: `2e5418b`.
`feat(...)` commits: `641cae7`, `b9376f2`.
`fix(...)` commit: `c3e35b7`.

## User Setup Required

None. All changes are:
- One `LogLineView` field addition (internal to the handler module — no public API).
- One `fetch_logs` function-local signature change (module-private function).
- Three struct field additions (module-private view models).
- Test-only additions (new helper + real bodies).

No new migrations, config keys, CLI flags, routes, dependencies, or operator action. The `/jobs/{job_id}/runs/{run_id}` URL scheme is unchanged (DB-13 permalink verified by `permalink_by_global_id`).

## Next Phase Readiness

- **Plan 11-11 (client-side dedupe handler) unblocked on the server side.** The handler now computes `last_log_id` from the same `job_logs.id` column that Plan 11-08's SSE `id:` field emits, so once Plan 11-12 renders `data-max-id="{{ last_log_id }}"` into the template, the client can compare `event.lastEventId` against it numerically.
- **Plan 11-12 (template consumer for backfill + data-max-id) unblocked.** `RunDetailPage.last_log_id` is available as a template variable, ready for `data-max-id="{{ last_log_id }}"` on `#log-lines`. `LogLineView.id` is ready for rendering in the inline-log partial.
- **DB-13 (permalink by global run_id) verified.** `permalink_by_global_id` asserts 200 on valid id, 404 on unknown id. The URL shape `/jobs/{job_id}/runs/{run_id}` unchanged from pre-Phase-11.

## Self-Check: PASSED

**Files verified on disk:**
- `src/web/handlers/run_detail.rs` — FOUND; `pub id: i64` at L98 inside `LogLineView`; `fetch_logs` returns `(Vec<LogLineView>, i64, bool, i64, i64)` at L125; `last_log_id = logs.iter().map(|l| l.id).max().unwrap_or(0)` at L156; three struct-field `last_log_id: i64` at L49, L60, L72 (each preceded by `#[allow(dead_code)]`); all three callers destructure `last_log_id` and pass it into struct literals.
- `tests/v11_run_detail_page_load.rs` — FOUND; 237 lines; one `#[ignore]` remaining (Plan 11-12 stub); three `#[tokio::test]` functions with real bodies (`renders_static_backfill`, `permalink_by_global_id`, `get_recent_job_logs_chronological`); local `build_test_app()` helper at L33.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-09-SUMMARY.md` — FOUND (this file).

**Commits verified (present in `git log d0ec085..HEAD`):**
- `641cae7` — FOUND (`feat(11-09): extend LogLineView with id + fetch_logs returns last_log_id`)
- `b9376f2` — FOUND (`feat(11-09): add last_log_id field to three view model structs`)
- `c3e35b7` — FOUND (`fix(11-09): scope #[allow(dead_code)] to last_log_id fields`)
- `2e5418b` — FOUND (`test(11-09): replace Wave-0 stubs with real page-load backfill bodies`)

**Build gates verified:**
- `cargo check --lib` — CLEAN.
- `cargo clippy --lib --tests -- -D warnings` — CLEAN.
- `cargo fmt --check` — CLEAN.
- `cargo test --lib` — PASS (`173 passed; 0 failed`).
- `cargo test --test v11_run_detail_page_load` — PASS (`3 passed; 0 failed; 1 ignored`).
- `cargo test --test api_run_now` — PASS (`2 passed`).
- `cargo test --test v11_run_now_sync_insert` — PASS (`3 passed`).
- `cargo test --test v11_sse_log_stream` — PASS (`2 passed`).
- `cargo test --test v11_log_id_plumbing` — PASS (`3 passed`).
- `cargo test --test xss_log_safety` — PASS (`7 passed`).

**Plan success criteria verified:**
1. LogLineView has `pub id: i64` — PASS (verified: `grep -q "pub id: i64" src/web/handlers/run_detail.rs` matches).
2. fetch_logs returns `(..., i64)` with last_log_id in the 5th slot — PASS (verified: signature at src/web/handlers/run_detail.rs:125 reads `-> (Vec<LogLineView>, i64, bool, i64, i64)`).
3. fetch_logs uses the existing `queries::get_log_lines`; no new query function added — PASS (verified: `src/db/queries.rs` unchanged; only call site is `queries::get_log_lines(pool, run_id, LOG_PAGE_SIZE, offset)`).
4. RunDetailPage + StaticLogViewerPartial + LogViewerPartial all have `last_log_id` — PASS (verified: `grep -c "last_log_id: i64," src/web/handlers/run_detail.rs` = 3).
5. permalink_by_global_id passes — PASS (test output: `test permalink_by_global_id ... ok`).
6. get_recent_job_logs_chronological passes and references the actual `get_log_lines` function — PASS (test output: `test get_recent_job_logs_chronological ... ok`; body calls `cronduit::db::queries::get_log_lines(&pool, run_id, 100, 0)`).

---
*Phase: 11-per-job-run-numbers-log-ux-fixes*
*Completed: 2026-04-17*
