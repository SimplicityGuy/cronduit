---
phase: 23-job-tagging-dashboard-filter-chips-rc-3
plan: 02
subsystem: db
tags: [db, sqlx, parity, tagging, filter-sql, wave-1]

# Dependency graph
requires:
  - phase: 23-job-tagging-dashboard-filter-chips-rc-3
    plan: 01
    provides: tests/v12_tags_dashboard.rs scaffolded with V-01..V-04 todo!() stubs + seed_job_with_tags todo!() helper
  - phase: 22-job-tagging-schema-validators
    provides: jobs.tags JSON column (TEXT NOT NULL DEFAULT '[]'), sorted-canonical storage form, charset+reserved+collision validators (P22 D-09 / TAG-01..05)
provides:
  - "DashboardJob.tags: Vec<String> field deserialized from `j.tags AS tags_json` at the row-mapping site (both backend arms)"
  - "get_dashboard_jobs(active_tags: &[String]) — fifth parameter; variadic AND-chained `tags LIKE ?N`/`$N` predicates with parameterized binds"
  - "Conditional `AND tags != '[]'` clause (gated to active_tags non-empty) — TAG-07 untagged-hidden semantics"
  - "V-01..V-04 integration tests GREEN (and_filter_two_tags, untagged_hidden_when_filter_active, no_filter_shows_all_jobs, and_with_name_filter)"
  - "seed_job_with_tags helper in tests/v12_tags_dashboard.rs — sorted+deduped JSON serialization per Phase 22 D-09"
affects:
  - 23-03 (handler-side fold + active-tag intersection — fills the &[] placeholder)
  - 23-04 (axum_extra::Query extractor swap — V-05)
  - 23-05 (handler-side distinct-tag fold — V-07 + V-08..V-10)
  - 23-06 (template inserts + CSS chip primitive — V-11..V-14)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Variadic AND-chain SQL composition via format-string of `(0..N).map(|i| format!(\"AND tags LIKE ?{}\", offset+i))` — predicate count is server-controlled; values flow through bind()"
    - "JSON-quote-anchored LIKE pattern `r#\"%\\\"{}\\\"%\"#` — defense-in-depth against substring false-positives (P22 TAG-05 collision validator is the structural gate)"
    - "Conditional WHERE-clause fragment via `if !active_tags.is_empty()` — preserves default-load semantics while supporting filter-active untagged-hidden"
    - "Unified bind builder pattern — `let mut q = sqlx::query(...); if has_filter { q = q.bind(...) } for t in active_tags { q = q.bind(...) }`"

key-files:
  created: []
  modified:
    - src/db/queries.rs
    - src/web/handlers/dashboard.rs
    - src/web/handlers/api.rs
    - tests/v12_tags_dashboard.rs
    - tests/dashboard_jobs_pg.rs

key-decisions:
  - "Both backend arms widen in lockstep — SQLite bind builder + Postgres bind builder use the same shape (mutable q + conditional name bind + tag bind loop)."
  - "Caller updates touched 8 sites (1 prod handler + 1 API handler + 5 internal test fns + 2 postgres-smoke calls) — all pass `&[]` placeholder until Plan 23-03 wires the active set."
  - "P22 TAG-05 substring-collision validator at config-load is the structural security boundary; the JSON-quote-anchored LIKE is defense-in-depth, not load-bearing for correctness."
  - "Pre-existing schema_parity Docker-required test (`sqlite_and_postgres_schemas_match_structurally`) remains environmentally blocked locally — documented in `.planning/phases/23-.../deferred-items.md`. Pure-logic tests in the same file (normalize_type) pass."

patterns-established:
  - "Variadic predicate composition with bind-offset arithmetic — sets the precedent for the active-tag SQL pattern Plan 23-03 relies on."
  - "Wave-1-of-N caller update — the handler call site receives a `&[]` placeholder during the SQL-foundation plan; the next wave plan replaces it with the real active set without any signature churn."

requirements-completed: []  # TAG-07 lands GREEN end-to-end after Plan 23-03 wires the active set; this plan provides only the DB-layer half of the contract.

# Metrics
duration: ~9min
completed: 2026-05-05
---

# Phase 23 Plan 02: Wave-1 DB-Layer Tag Filter Foundation Summary

**`DashboardJob` widened to carry `tags: Vec<String>`; `get_dashboard_jobs` accepts `active_tags: &[String]` and composes variadic AND-chained `tags LIKE` predicates plus the conditional `tags != '[]'` untagged-hidden clause; V-01..V-04 GREEN.**

## Performance

- **Duration:** ~9 min (8min46s)
- **Started:** 2026-05-05T01:54:38Z
- **Completed:** 2026-05-05T02:03:24Z
- **Tasks:** 2 (both `type="auto"` `tdd="true"`)
- **Files modified:** 5 (no new files)
- **Commits:** 2

## Accomplishments

- **`DashboardJob` struct widened** at `src/db/queries.rs:603-610` — added `pub tags: Vec<String>` after `enabled_override` with the locked Phase 23 docstring. Field is `Vec<String>` (not `Option<...>`) per the `NOT NULL DEFAULT '[]'` schema invariant.
- **`get_dashboard_jobs` SELECT widened** in both backend arms (SQLite L851/865; Postgres L915/928) — `j.tags AS tags_json` projected after `j.enabled_override`. Edit-pair invariant honored: SQLite + Postgres land in the same commit.
- **`get_dashboard_jobs` row-map widened** in both backend arms — `tags_json` deserialized to `Vec<String>` via `serde_json::from_str(&s).unwrap_or_default()` per the P22 D-09 forgiving fallback precedent at `queries.rs:1448-1456`.
- **`get_dashboard_jobs` signature widened** with `active_tags: &[String]` as the fifth parameter.
- **Variadic AND-chain composed** via two `format!`-built strings (`tag_predicates_sqlite` for `?N` placeholders, `tag_predicates_postgres` for `$N` placeholders). Predicate COUNT is server-controlled (`active_tags.len()`); values flow through `bind()` exclusively.
- **Conditional `AND tags != '[]'` clause** gated by `if !active_tags.is_empty()` — TAG-07 untagged-hidden semantics fire only when ANY tag filter is active.
- **JSON-quote-anchored bind format** `format!(r#"%"{}"%"#, t)` applied in both backend arms. P22 TAG-05 substring-collision validator at config-load is the structural security boundary; the JSON-quote anchors are defense-in-depth.
- **All 8 callers updated** to pass `&[]` placeholder fifth argument:
  - `src/web/handlers/dashboard.rs:254` (production dashboard handler)
  - `src/web/handlers/api.rs:366` (JSON `/api/jobs` list endpoint)
  - `src/db/queries.rs:2531, 2547, 2564, 2581, 2596` (5 internal `#[cfg(test)]` regression tests)
  - `tests/dashboard_jobs_pg.rs:49, 57` (Postgres-smoke calls)
- **`seed_job_with_tags` helper** in `tests/v12_tags_dashboard.rs` filled in — sorts + dedups, then `serde_json::to_string` for the canonical column form (matches what production validators emit at config-load).
- **V-01..V-04 GREEN** — `cargo test --test v12_tags_dashboard and_filter_two_tags untagged_hidden_when_filter_active no_filter_shows_all_jobs and_with_name_filter` all exit 0 (4/4 passing).

## Task Commits

Each task committed atomically:

1. **Task 1: Widen `DashboardJob` struct + `get_dashboard_jobs` SELECT/row-map (both backend arms)** — `fc7be62` (feat)
2. **Task 2: Add `active_tags` parameter + AND-chained `tags LIKE` + caller updates + V-01..V-04 test bodies** — `07faea2` (feat)

## Files Modified

- `src/db/queries.rs` — `DashboardJob.tags` field added; `get_dashboard_jobs` signature widened with `active_tags: &[String]`; SELECT widened with `j.tags AS tags_json`; row-map widened with forgiving JSON deserialize; WHERE composition extended with variadic AND-chain + conditional untagged clause; bind sequence converted from `if/else` to a unified mutable builder pattern. Both SQLite and Postgres arms in lockstep. Five internal `#[cfg(test)]` callers updated to pass `&[]`.
- `src/web/handlers/dashboard.rs` — Production caller updated to pass `&[]` fifth argument with a comment pointing forward to Plan 23-03.
- `src/web/handlers/api.rs` — `list_jobs` JSON-API caller updated.
- `tests/v12_tags_dashboard.rs` — `seed_job_with_tags` helper body filled in (sorted+dedup + `serde_json::to_string`); V-01..V-04 test bodies filled in with concrete assertions per the `<behavior>` block in the plan.
- `tests/dashboard_jobs_pg.rs` — Two Postgres-smoke callers updated.

## Validation

| V-row | Test fn | Status | Backend exercised |
|---|---|---|---|
| V-01 | `and_filter_two_tags` | GREEN | SQLite (in-memory) |
| V-02 | `untagged_hidden_when_filter_active` | GREEN | SQLite (in-memory) |
| V-03 | `no_filter_shows_all_jobs` | GREEN | SQLite (in-memory) |
| V-04 | `and_with_name_filter` | GREEN | SQLite (in-memory) |

```text
cargo test --test v12_tags_dashboard and_filter_two_tags          → 1 passed
cargo test --test v12_tags_dashboard untagged_hidden_when_filter_active → 1 passed
cargo test --test v12_tags_dashboard no_filter_shows_all_jobs     → 1 passed
cargo test --test v12_tags_dashboard and_with_name_filter         → 1 passed
```

Other v12_tags_dashboard tests (chip_strip_render, chip_active_state_class, direct_url_renders_chips_active, stale_tag_silent_drop, css_only_chip_no_inline_js, oob_response_shape, sort_header_carries_active_tags, poll_hx_include_widened) remain at `todo!()` as planned — Wave 2 fills them in.

## Compile + regression gates

- `cargo build --quiet` exits 0
- `cargo test --lib --no-run` exits 0
- `cargo test --test v12_tags_dashboard --no-run` exits 0 (compile gate intact)
- `cargo test --test dashboard_render` — 2/2 passing (no regression on existing dashboard rendering)
- Pre-existing lib-test failures in `web::handlers::dashboard::tests::active_tags_parsed_from_repeated_query` (V-05) and `distinct_tag_fold_alphabetical` (V-07) remain at `todo!()` per Plan 23-01's Wave-0 scaffold — these are owned by Plans 23-04 and 23-05 respectively and are explicitly out of scope for 23-02.

## Security gate (parameterized binds — T-23-02-01)

- Predicate COUNT comes from `active_tags.len()` — a server-controlled integer.
- Tag VALUES bind through `sqlx::query::bind()` — never string-interpolated into the SQL skeleton.
- The structural verification: `grep -F 'format!(r#' src/db/queries.rs` returns 3 hits (raw-string used for the JSON-quote-anchored bind value); the bind step is inside `for t in active_tags { q = q.bind(format!(r#"%"{}"%"#, t)); }` in both backend arms.
- The format-string composes only the predicate count and the placeholder syntax (`?N` vs `$N`); no operator-controlled value reaches the SQL string.
- The `tags != '[]'` clause is statically appended (no value to bind), gated only by `!active_tags.is_empty()` (a server-side boolean).

## Decisions Made

- **Bind builder shape: unified mutable `q` per arm.** The original code used `if has_filter { sqlx::query(...).bind(pattern) } else { sqlx::query(...) }`. Adding the variadic tag binds on top of that branching would produce four code paths. Refactored to `let mut q = sqlx::query(...); if has_filter { q = q.bind(pattern); } for t in active_tags { q = q.bind(...); }` — one path per backend, equivalent semantics, easier to read.
- **JSON-quote-anchored LIKE on `r.tags` (not on a CTE-projected unnested form).** P22 keeps the schema parity-friendly across SQLite + Postgres by storing tags as a TEXT JSON column. Using `tags LIKE '%"backup"%'` works identically across both backends without dialect-specific JSON ops. This is the recommended D-09 form per CONTEXT.
- **`unwrap_or_default()` for forgiving JSON deserialize.** Mirrors P22 D-09's posture at `queries.rs:1448-1456`. The column is `NOT NULL DEFAULT '[]'`, so corruption is structurally impossible from cronduit-controlled writes; the fallback is defense-in-depth.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Plan undercounts callers; updated 8 sites instead of the documented 1**

- **Found during:** Task 2 verification (`cargo build` would fail without these updates)
- **Issue:** The plan's `<interfaces>` block at line 114-119 named only `src/web/handlers/dashboard.rs:250` as the "only one" call site of `get_dashboard_jobs`. In reality, after the signature widening, 8 sites needed `&[]` placeholders to compile:
  1. `src/web/handlers/dashboard.rs:254` (production handler — documented)
  2. `src/web/handlers/api.rs:366` (JSON `/api/jobs` list endpoint — undocumented)
  3. `src/db/queries.rs:2531, 2547, 2564, 2581, 2596` (5 `#[cfg(test)]` regression tests — undocumented)
  4. `tests/dashboard_jobs_pg.rs:49, 57` (2 Postgres-smoke callers — undocumented)
- **Fix:** Updated all 8 sites to pass `&[]` as the placeholder fifth argument. Each site is functionally identical to its pre-P23 behavior because the empty active set produces zero `AND tags LIKE` predicates and no `tags != '[]'` clause.
- **Files modified:** `src/web/handlers/api.rs`, `src/db/queries.rs` (test block), `tests/dashboard_jobs_pg.rs`
- **Verification:** `cargo build --quiet` exits 0; `cargo test --lib --no-run` exits 0; `cargo test --test dashboard_render` 2/2 passing.
- **Committed in:** `07faea2` (Task 2 commit)
- **Documented for downstream:** Plan 23-03 will replace the `&[]` in `src/web/handlers/dashboard.rs:254` with the real active set; the other 7 callers stay at `&[]` because they are tag-agnostic regression-coverage paths that don't need active-tag filtering.

### Out-of-Scope

**Pre-existing schema_parity Docker-required test (environmental)**

- The `sqlite_and_postgres_schemas_match_structurally` test in `tests/schema_parity.rs` requires Docker for `testcontainers`. Local machine has no Docker socket. The same test panics identically on `main` before any 23-02 commit — the failure is environmental, not regression.
- Logged in `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/deferred-items.md`. Pure-logic tests in the same file (`known_types_normalize_correctly`, `unknown_type_panics`) pass locally.
- CI runs Docker, so this test gate is verified in CI as part of the Wave-1 PR.

---

**Total deviations:** 1 auto-fixed (Rule 3 — blocking caller count mismatch). 0 architectural decisions required (Rule 4).
**Impact on plan:** Substantive contract preserved end-to-end. The 7 additional caller updates are minimal and tag-agnostic (`&[]`), explicitly out of Plan 23-03's scope (which only replaces the production handler call site with the real active set).

## Issues Encountered

None — pre-existing schema_parity Docker dependency is environmental and was not introduced or affected by this plan.

## User Setup Required

None.

## TDD Gate Compliance

Plan frontmatter is `type: execute` and individual tasks are marked `tdd="true"`. The Wave-0 plan (23-01) provided the failing-test surface (V-01..V-04 in `todo!()` state). This plan filled in the implementation, flipping V-01..V-04 from `todo!()` panic to GREEN. Both task commits are `feat(...)` per project convention because the new code is feature implementation, not a fresh test-write step (Plan 23-01 owned the `test(...)` commits).

The cross-plan picture for V-01..V-04: RED (Plan 23-01 — `todo!()` stubs land) → GREEN (Plan 23-02 — implementation lands).

## Next Phase Readiness

- **Plan 23-03 is unblocked.** The DB layer accepts `active_tags: &[String]`; Plan 23-03 wires the handler-side fold + active-tag intersection and replaces the `&[]` placeholder at `src/web/handlers/dashboard.rs:254` with the real fleet-intersected set.
- **Plan 23-04 is unblocked.** The `axum_extra::extract::Query<DashboardParams>` swap and the `tags: Vec<String>` field on `DashboardParams` (V-05) can land independently of 23-02 (no SQL surface dependency).
- **Plans 23-05..23-08 unaffected.** This plan is purely DB-layer; the handler-side fold (23-05), template inserts (23-06), HUMAN UAT (23-07), and RC3 PREFLIGHT (23-08) are downstream and untouched.

## Self-Check: PASSED

- `src/db/queries.rs` exists and contains `pub tags: Vec<String>` (2 occurrences total: 1 in `DbRunDetail`, 1 in `DashboardJob`)
- `src/db/queries.rs` contains `active_tags: &[String]` parameter
- `src/db/queries.rs` contains `tag_predicates_sqlite` and `tag_predicates_postgres`
- `src/db/queries.rs` contains the literal `AND tags != '[]'`
- `src/db/queries.rs` contains `active_tags.is_empty()` gate
- `src/db/queries.rs` contains `format!(r#` (3 hits — JSON-quote-anchored bind format)
- `src/web/handlers/dashboard.rs` contains `&[]` placeholder argument (2 hits — including pre-existing `&[] }`)
- Commit `fc7be62` (Task 1) found in `git log --oneline --all`
- Commit `07faea2` (Task 2) found in `git log --oneline --all`
- `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-02-SUMMARY.md` exists on disk

---

*Phase: 23-job-tagging-dashboard-filter-chips-rc-3*
*Completed: 2026-05-05*
