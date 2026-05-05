---
phase: 23-job-tagging-dashboard-filter-chips-rc-3
plan: 01
subsystem: testing
tags: [tests, scaffolding, tagging, dashboard, wave-0, axum, askama]

# Dependency graph
requires:
  - phase: 22-job-tagging-schema-validators
    provides: jobs.tags JSON column, sorted-canonical storage form, charset+reserved+collision validators (P22 D-09 / TAG-01..05)
provides:
  - "tests/v12_tags_dashboard.rs scaffolded with 12 #[tokio::test] async functions in todo!() state — one per VALIDATION row V-01..V-04, V-06, V-08..V-14"
  - "src/web/handlers/dashboard.rs::tests extended with 2 #[tokio::test] async functions (V-05 + V-07) in todo!() state"
  - "Compile-gate green: cargo test --test v12_tags_dashboard --no-run AND cargo test --lib --no-run BOTH exit 0"
  - "Stable function-name surface for Wave 1-3 plans to wire implementations against"
  - "Helpers build_test_app + seed_job (verbatim from tests/dashboard_render.rs); seed_job_with_tags stub (Wave 1 fills)"
affects:
  - 23-02 (DashboardJob.tags field + SELECT widening — Wave 1)
  - 23-03 (AND-tag filter SQL — Wave 1)
  - 23-04 (axum_extra::Query extractor swap + active-tag parsing — Wave 1; lands V-05 + V-01..V-04)
  - 23-05 (handler-side fold + view-model widening — Wave 2; lands V-07 + V-08..V-10)
  - 23-06 (template inserts + CSS chip primitive + OOB swap — Wave 2; lands V-11..V-14 + V-06)
  - 23-07 (HUMAN UAT)
  - 23-08 (RC3 PREFLIGHT)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Wave-0 todo!()-body scaffold: stable function-name surface that compiles before any implementation lands"
    - "Test-helper layout for tag-aware seeding (seed_job_with_tags) co-located with the legacy seed_job"

key-files:
  created:
    - tests/v12_tags_dashboard.rs
  modified:
    - src/web/handlers/dashboard.rs

key-decisions:
  - "Function-name fidelity is load-bearing — Wave 1-3 acceptance criteria reference these names verbatim (e.g., `cargo test --test v12_tags_dashboard and_filter_two_tags`). Renaming would break the cross-plan trace."
  - "todo!() bodies (not #[ignore]) — wave-end gates surface missing implementations as panics, not silent skips."
  - "12 #[tokio::test] functions in tests/v12_tags_dashboard.rs (one per V-row), not 10 as the plan's frontmatter `must_haves.truths` and one acceptance criterion claim. The action text and the enumerated grep checks both demand 12; the `10` is a plan typo. Documented as Rule 1 deviation."
  - "Imports + helpers are exercised by a #[allow(dead_code)] async fn _wave0_compile_anchor so the file compiles without `unused import` warnings while every test body remains todo!()."

patterns-established:
  - "Wave-0 scaffold: stub all named test functions with todo!() before any impl plan touches them. Compile gate flips green from commit 1 of the phase; runtime gate flips green incrementally as Wave 1-3 lands."
  - "Repeated-test-helper pattern: copy build_test_app/seed_job verbatim from the closest sibling integration test (tests/dashboard_render.rs) rather than abstracting into a shared module — keeps each test crate self-contained per the existing v12_*.rs family convention."

requirements-completed: []  # Wave-0 SCAFFOLDING ONLY — TAG-06/07/08 land green at the end of Wave 1-3, not here.

# Metrics
duration: ~7min
completed: 2026-05-05
---

# Phase 23 Plan 01: Wave-0 Test Scaffolding Summary

**12 named integration tests + 2 named handler unit tests scaffolded with todo!() bodies — Wave 1-3 plans now have a stable, compiling test surface to land implementations against.**

## Performance

- **Duration:** ~7 min
- **Started:** 2026-05-05T01:42:34Z
- **Completed:** 2026-05-05T01:49:31Z
- **Tasks:** 2 (both `type="auto"`)
- **Files modified:** 2 (1 new, 1 extended)

## Accomplishments

- `tests/v12_tags_dashboard.rs` (NEW, 220 lines) — 12 `#[tokio::test]` async functions named per VALIDATION rows V-01..V-04, V-06, V-08..V-14. Helpers `build_test_app` + `seed_job` copied verbatim from `tests/dashboard_render.rs`; new `seed_job_with_tags` helper stubbed for Wave 1.
- `src/web/handlers/dashboard.rs::tests` extended with two new `#[tokio::test]` async functions (`active_tags_parsed_from_repeated_query` for V-05, `distinct_tag_fold_alphabetical` for V-07). Pre-existing `format_relative_*` tests stay verbatim.
- Compile gates GREEN: `cargo test --test v12_tags_dashboard --no-run` exits 0; `cargo test --lib --no-run` exits 0; full `cargo build --tests` exits 0.
- No `#[ignore]` annotations introduced — Wave 1-3 wave-end gates fail fast on `todo!()` panics, surfacing missing implementations at the wave boundary instead of at PR review.

## Task Commits

Each task was committed atomically:

1. **Task 1: Stub `tests/v12_tags_dashboard.rs` with 12 named integration tests in todo!() state** — `e5d7cdd` (test)
2. **Task 2: Extend `src/web/handlers/dashboard.rs::tests` with V-05 + V-07 unit-test stubs** — `4147af2` (test)

## Files Created/Modified

- `tests/v12_tags_dashboard.rs` (CREATED) — Wave-0 integration test scaffolding for V-01..V-04, V-06, V-08..V-14. Header doc-comment cites TAG-06/07/08 + run command. Imports + helpers borrow verbatim from `tests/dashboard_render.rs`. Each test body is `todo!("Wave N: ...")` with the specific Wave 1-3 task description embedded so the executor reading the panic message knows what to write.
- `src/web/handlers/dashboard.rs` (MODIFIED) — `#[cfg(test)] mod tests` block extended with two new `#[tokio::test]` functions (V-05 + V-07). Pre-existing four `format_relative_*` tests untouched. No `#[ignore]` added.

## Wave 1-3 Cross-Reference

Each `todo!()` body names the Wave plan that fills it in:

| Test fn | V-row | Wave | Wave plan that fills it |
|---|---|---|---|
| `and_filter_two_tags` | V-01 | 1 | 23-03 (AND-tag filter SQL) |
| `untagged_hidden_when_filter_active` | V-02 | 1 | 23-03 |
| `no_filter_shows_all_jobs` | V-03 | 1 | 23-03 |
| `and_with_name_filter` | V-04 | 1 | 23-03 |
| `active_tags_parsed_from_repeated_query` | V-05 | 1 | 23-04 (axum_extra::Query swap) |
| `stale_tag_silent_drop` | V-06 | 2 | 23-05 (handler fold + active-tag intersect) |
| `distinct_tag_fold_alphabetical` | V-07 | 2 | 23-05 |
| `chip_strip_render` | V-08 | 2 | 23-06 (template inserts) |
| `chip_active_state_class` | V-09 | 2 | 23-06 |
| `direct_url_renders_chips_active` | V-10 | 2 | 23-06 |
| `css_only_chip_no_inline_js` | V-11 | 2 | 23-06 |
| `oob_response_shape` | V-12 | 2 | 23-06 |
| `sort_header_carries_active_tags` | V-13 | 2 | 23-06 |
| `poll_hx_include_widened` | V-14 | 2 | 23-06 |

## Decisions Made

- **12 tests, not 10.** The PLAN frontmatter and the first acceptance criterion both stated "10 #[tokio::test] async functions" while every other section (action text, enumerated grep list, `<done>`) listed 12 by V-row name. Resolved to 12 — the substantive contract (one test per V-row) is the binding requirement; the `10` is a plan typo. Documented as Rule 1 below.
- **`#[allow(dead_code)] async fn _wave0_compile_anchor` exercises every imported symbol (Request/Body/StatusCode/to_bytes/ServiceExt/build_test_app/seed_job) once.** Without it, every `use` statement would generate an `unused import` warning during Wave 0 because every real test body is `todo!()` and never references the imports. The anchor function is `#[allow(dead_code)]` and never called. Wave 1-3 can leave it in place or delete it once enough real test bodies reference the imports.
- **`seed_job_with_tags` is `todo!()`, not implemented.** The plan's `<action>` block was explicit: "Wave 0: implement in Wave 1 — `serde_json::to_string` of sorted+deduped Vec<String>". Honoring that posture keeps Wave 1 plans the sole owner of the JSON-canonicalization decision.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Plan typo / internal inconsistency] PLAN counted "10" tests but enumerates 12 V-rows**

- **Found during:** Task 1 (writing `tests/v12_tags_dashboard.rs`)
- **Issue:** PLAN frontmatter `must_haves.truths` line 17 and acceptance criterion line 259 both say "Exactly 10 `#[tokio::test]` annotations". The action text on line 245 says "exactly 12 #[tokio::test] functions in the file" and lines 256-272 enumerate 12 specific test names by V-row (V-01, V-02, V-03, V-04, V-06, V-08, V-09, V-10, V-11, V-12, V-13, V-14 = 12). The `10` is a plan typo.
- **Fix:** Wrote 12 functions matching the enumerated names. The substantive contract (one test per V-row) is satisfied.
- **Files modified:** `tests/v12_tags_dashboard.rs`
- **Verification:** `grep -c '^#\[tokio::test\]' tests/v12_tags_dashboard.rs` returns `12`; every V-row name `grep -q` check passes.
- **Committed in:** `e5d7cdd` (Task 1 commit)

**2. [Rule 3 - Blocking] Comment containing the literal string `#[ignore]` tripped the no-`#[ignore]` acceptance grep**

- **Found during:** Task 1 verification (`grep -c '#\[ignore\]' tests/v12_tags_dashboard.rs` returned 1, expected 0)
- **Issue:** A doc-comment inside the file read "NO `#[ignore]` is added — Wave-end gates surface missing implementations as panics." That literal substring matches the acceptance grep, which is not anchored to a line start.
- **Fix:** Reworded the comment to "The ignore attribute is deliberately omitted — Wave-end gates surface missing implementations as panics." Same semantic content, no literal `#[ignore]` substring.
- **Files modified:** `tests/v12_tags_dashboard.rs` (pre-commit; landed in the same Task 1 commit)
- **Verification:** `grep -c '#\[ignore\]' tests/v12_tags_dashboard.rs` returns `0`.
- **Committed in:** `e5d7cdd` (Task 1 commit)

**3. [Rule 2 - Missing critical] Added `#[allow(dead_code)] async fn _wave0_compile_anchor` to keep Wave-0 imports/helpers used**

- **Found during:** Task 1 (anticipating clippy/dead-code warnings from Wave-0 todo!() bodies)
- **Issue:** Every test body is `todo!()`, so the imports `Request`, `Body`, `StatusCode`, `to_bytes`, `ServiceExt`, plus the helpers `build_test_app` and `seed_job`, would all be unused during Wave 0. The CI matrix runs `cargo clippy --all-targets --all-features -- -D warnings`; an `unused import` warning would fail clippy at every commit until Wave 1 lands. The plan's `<action>` block specifies these imports verbatim and does NOT account for the dead-code surface.
- **Fix:** Added a `#[allow(dead_code)] async fn _wave0_compile_anchor()` that exercises each imported symbol + helper once. Never called; Wave 1-3 can leave it in place or delete it.
- **Files modified:** `tests/v12_tags_dashboard.rs`
- **Verification:** `cargo build --tests` exits 0 with no warnings on the new file.
- **Committed in:** `e5d7cdd` (Task 1 commit)

---

**Total deviations:** 3 auto-fixed (1 plan typo, 1 blocking grep mismatch, 1 missing critical to keep clippy green)
**Impact on plan:** All deviations preserve the substantive contract (one test function per V-row, all in `todo!()` state, no `#[ignore]`, compile gates green). No scope creep — every change is a minimal, defensible patch to make the plan executable as written.

## Issues Encountered

None.

## User Setup Required

None — pure test scaffolding; no external service configuration.

## TDD Gate Compliance

Plan frontmatter is `type: execute`, not `type: tdd`, so the plan-level RED→GREEN→REFACTOR gate sequence does not apply. Both task commits are `test(...)` per project commit convention. Wave 1-3 will land `feat(...)` commits implementing the tests; once those land, the cross-plan picture is RED (this plan) → GREEN (Wave 1-3) per the per-V-row contract.

## Next Phase Readiness

- **Wave 1 (plans 23-02, 23-03, 23-04) is unblocked.** Each plan can wire its acceptance criteria against test functions that already exist and compile. The `cargo test --test v12_tags_dashboard --no-run` gate is GREEN from this commit forward; the per-V-row runtime gates flip green as each Wave 1-3 plan lands.
- **No blockers.** PLAN's success criteria all satisfied.
- **Open question for Wave 1:** the `seed_job_with_tags` helper body is still `todo!()` — Wave 1-02 (`DashboardJob.tags` field) is the natural place to fill it in alongside the `j.tags AS tags_json` projection landing.

## Self-Check: PASSED

- `tests/v12_tags_dashboard.rs` exists on disk
- `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-01-SUMMARY.md` exists on disk
- Commit `e5d7cdd` (Task 1) found in `git log --oneline --all`
- Commit `4147af2` (Task 2) found in `git log --oneline --all`
- `src/web/handlers/dashboard.rs` contains `fn active_tags_parsed_from_repeated_query` (V-05)
- `src/web/handlers/dashboard.rs` contains `fn distinct_tag_fold_alphabetical` (V-07)

---

*Phase: 23-job-tagging-dashboard-filter-chips-rc-3*
*Completed: 2026-05-05*
