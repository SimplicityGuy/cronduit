---
phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
plan: 03
subsystem: ui
tags: [exit-codes, histogram, sqlx, sqlite, postgres, web, exit-01, exit-02, exit-03, exit-04, exit-05]

# Dependency graph
requires:
  - phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
    plan: 01
    provides: "job_runs.scheduled_for column (Phase 21 col-add) — not directly required, but Wave 2 ordering"
  - phase: 13-observability-polish-rc-2
    provides: "src/web/stats.rs module shape (file-doc + single pub fn + #[cfg(test)] mod tests) and queries::get_recent_successful_durations raw-fetch helper analog (per-backend split, ?N/$N placeholders, idx_job_runs_job_id_start hit)"
provides:
  - "src/web/exit_buckets.rs: 10-variant ExitBucket enum + categorize() classifier + aggregate() builder + HistogramCard struct + TopCode struct"
  - "src/web/mod.rs: pub mod exit_buckets registered alphabetically"
  - "src/db/queries.rs: get_recent_runs_for_histogram(pool, job_id, limit) returning Vec<(String, Option<i32>, Option<String>)> ordered by start_time DESC, all-statuses (no SQL-side bucketing per D-06)"
  - "Foundation for Wave 2 plans: 21-04 run_detail handler (FCTX panel) and 21-05 job_detail handler (Exit Histogram card view-model)"
affects: [21-04, 21-05, 21-08, 21-09, 21-10]

# Tech tracking
tech-stack:
  added: []  # zero new external crates (D-32 invariant preserved; only std::collections::HashMap used)
  patterns:
    - "Module-shape parity with src/web/stats.rs: file-level doc-comment cited to phase + decisions, public surface, single concern, in-module #[cfg(test)] mod tests block"
    - "Status-discriminator-wins classifier (D-08): status='success' → None; status='stopped' → BucketStopped regardless of exit_code; otherwise route by exit_code with defensive fallback for codes outside POSIX 0..=255"
    - "Owned-tuple input to pure aggregator (research §C): aggregate(&[(String, Option<i32>, Option<String>)]) — borrowed slice, owned tuples, decoupled from sqlx::Row lifetime"
    - "Top-3 determinism via lexicographic comparator: sort by count DESC then code ASC; truncate(3) post-sort"
    - "Newest-first stream → first-occurrence-wins: rows arrive ORDER BY start_time DESC so the FIRST per-code occurrence is the LATEST end_time; if entry.1.is_none() then assign — avoids re-comparing timestamps"

key-files:
  created:
    - src/web/exit_buckets.rs
  modified:
    - src/web/mod.rs
    - src/db/queries.rs

key-decisions:
  - "Used sqlx::query + r.get::<T, _> per-column extraction (matching the analog get_recent_successful_durations) instead of sqlx::query_as with tuple FromRow — keeps decode style consistent with the surrounding queries.rs convention"
  - "Defensive fallback for out-of-POSIX exit codes (Some(c) where c is negative or > 255) routes to BucketNull — operators should not see real codes outside 0..=255 from POSIX wait-status semantics, and the alternative would be to crash or silently misbucket"
  - "Top-3 tie-break is code ASC (deterministic) — avoids HashMap iteration order leaking into rendered output; documented inline + asserted in top_3_codes_last_seen test"
  - "First-seen end_time per code is kept (not last-seen by string compare) — the input contract is ORDER BY start_time DESC, so the first-encountered row IS the most recent; if we re-compared timestamps we'd waste cycles on a property the caller already guarantees"

patterns-established:
  - "When a phase introduces a new pure-Rust web/<concern>.rs module, mirror src/web/stats.rs structure exactly: file-level doc-comment cited to the phase + decision IDs, public surface, ZERO new external crates (only std), in-module #[cfg(test)] mod tests with the test cases enumerated in the plan"
  - "When a phase needs a raw-fetch DB helper for last-N rows, mirror queries::get_recent_successful_durations: per-backend match arm with ?N/$N split, hit idx_job_runs_job_id_start via WHERE job_id = ?1 ORDER BY start_time DESC LIMIT ?2, decode via sqlx::query + r.get; bucketing/aggregation lives in the consuming Rust module"

requirements-completed: [EXIT-01, EXIT-02, EXIT-03, EXIT-04, EXIT-05]

# Metrics
duration: ~8min
completed: 2026-05-02
---

# Phase 21 Plan 03: Exit-Buckets Module + Histogram Raw-Fetch Helper Summary

**Pure-Rust 10-variant ExitBucket categorizer + aggregator with status-discriminator-wins classifier (D-08), success-rate-excludes-stopped formula (D-09), and top-3 last-seen aggregation (EXIT-05); plus the queries::get_recent_runs_for_histogram raw-fetch helper feeding it last-100 ALL runs.**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-05-02T20:06:09Z
- **Completed:** 2026-05-02T20:14Z (approx)
- **Tasks:** 3 (all atomic-committed)
- **Files modified:** 3 (1 created, 2 modified)

## Accomplishments

- `src/web/exit_buckets.rs` created with the locked public surface: 10-variant `ExitBucket` enum (per D-07), `categorize` classifier following D-08 status-discriminator-wins rules, `aggregate` single-pass tally producing `HistogramCard` with bucket counts + success_count + stopped_count + sample_count + has_min_samples + success_rate (D-09 formula) + top_codes (EXIT-05 top-3, ties broken by code ASC for determinism)
- 6 in-module unit tests cover EXIT-02 (10 buckets + corner exit codes 1, 2, 3, 9, 10, 126, 127, 128, 137, 143, 144, 254, 255 + defensive fallback for out-of-POSIX codes), EXIT-04 (137 dual-classifier: status='stopped'+137 → BucketStopped vs status='failed'+137 → Bucket128to143), EXIT-03/D-09 (success-rate excludes stopped from denom; None when denom == 0), EXIT-05 (top-3 last-seen + tie-break by code ASC), D-11 (sample_count=4/5/6 → false/true/true), D-15/D-16 (zero-samples brand-new job)
- `src/web/mod.rs` registers `pub mod exit_buckets;` alphabetically between `csrf` and `format` (e < f)
- `src/db/queries.rs` adds `get_recent_runs_for_histogram(pool, job_id, limit) -> Vec<(String, Option<i32>, Option<String>)>` mirroring the Phase 13 `get_recent_successful_durations` analog: per-backend `?N`/`$N` split, ORDER BY start_time DESC hits the existing `idx_job_runs_job_id_start` index, ALL-statuses WHERE clause (no SQL-side bucketing per D-06)
- Zero new external crates (D-32 invariant preserved): module uses only `std::collections::HashMap`, helper uses only the existing `sqlx` + `anyhow` + `super::DbPool` surface
- `cargo nextest run -E 'test(/exit_buckets::tests/)'` exits 0 with all 6 tests passing
- `cargo build --workspace` exits 0; `cargo build --tests --workspace` exits 0
- `cargo tree -i openssl-sys` empty (rustls-only invariant holds)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create src/web/exit_buckets.rs with ExitBucket + categorize + aggregate + HistogramCard + TopCode + tests** — `c418593` (feat)
2. **Task 2: Register `pub mod exit_buckets;` in src/web/mod.rs** — `916e728` (feat)
3. **Task 3: Add `get_recent_runs_for_histogram` raw-fetch helper to src/db/queries.rs** — `cd64c30` (feat)

**Plan metadata:** _added in the final docs commit at SUMMARY-write time_

## Files Created/Modified

**Created (1):**
- `src/web/exit_buckets.rs` — 321 lines: file-level doc-comment cited to Phase 21 / D-07..D-11 / D-15..D-16; `pub enum ExitBucket` (10 variants per D-07); `pub struct TopCode { code, count, last_seen }`; `pub struct HistogramCard { buckets, success_count, stopped_count, sample_count, has_min_samples, success_rate, top_codes }` (7 fields); `pub fn categorize(status: &str, exit_code: Option<i32>) -> Option<ExitBucket>` implementing D-08 rules; `pub fn aggregate(rows: &[(String, Option<i32>, Option<String>)]) -> HistogramCard` single-pass tally; 6 in-module tests

**Modified (2):**
- `src/web/mod.rs` — inserted `pub mod exit_buckets;` between `pub mod csrf;` and `pub mod format;` (1 line addition)
- `src/db/queries.rs` — appended new `get_recent_runs_for_histogram` async helper after `get_recent_successful_durations` (66 line addition); per-backend SQLite/Postgres arms with `?N`/`$N` placeholder split

## Decisions Made

- **`sqlx::query` + `r.get::<T, _>` per-column extraction** (not `sqlx::query_as::<_, (String, Option<i32>, Option<String>)>`): mirrors the analog `get_recent_successful_durations` style (which uses the same `query` + `r.get::<i64, _>("duration_ms")` pattern) and keeps decode-style consistency across queries.rs. Both styles compile fine; the chosen one is locally idiomatic.
- **Defensive fallback to BucketNull for out-of-POSIX exit codes** (negative or > 255 in `Some(_)` arm): operators should never see real exit codes outside 0..=255 from POSIX wait-status semantics; routing them to BucketNull preserves the histogram's invariants (every bucket has well-defined operator-meaningful semantics) without crashing or silently misbucketing into a real bucket. Asserted explicitly in `categorize_all_10_buckets` test (`Some(-1)` and `Some(999)` both → `BucketNull`).
- **Top-3 tie-break is code ASC** (lexicographic comparator: `b.count.cmp(&a.count).then(a.code.cmp(&b.code))`): without an explicit secondary sort key, HashMap iteration order would leak into the rendered top-3 output → flaky tests + non-determinism in the UI. Code-ASC tie-break is the obvious operator-friendly choice (lower codes are typically more common: exit 1 = generic failure, exit 127 = command not found). Asserted in `top_3_codes_last_seen` test where two codes (1 and 143) have count 2 and the test pins their order.
- **First-seen end_time wins per code** (not "compare and keep the larger string"): the input contract is `ORDER BY start_time DESC` from `get_recent_runs_for_histogram`, so the FIRST row encountered for a given code is the MOST RECENT one. The `if entry.1.is_none() { entry.1 = end_time.clone(); }` pattern avoids the cost of repeatedly cloning + comparing timestamps when the caller has already enforced the ordering. Asserted in `top_3_codes_last_seen` test: code 1 has rows at (`t1`, `t0`) → `last_seen` is `Some("t1")`, not `Some("t0")`.

## Deviations from Plan

None — plan executed exactly as written. The plan's `<interfaces>` block specified the public surface in full, and the `<action>` block specified the body of `categorize` and the body of `aggregate` verbatim plus the 6 test functions. All three tasks landed without auto-fix triggers:

- Task 1: file written matching the locked surface; tests pass first run.
- Task 2: alphabetical insertion landed correctly (the plan briefly considered "between format and handlers" then self-corrected to "before format because e < f"; the file lands in the correct alphabetical slot).
- Task 3: `pool.reader()` accessor existed (verified via the analog helper at line 1014); ALL-statuses WHERE clause (no `status='success'` filter) preserved per D-06; both backends decoded via the existing `sqlx::query` + `r.get` style.

The plan's `<rollback>` § preempted three potential issues (HashMap import missing, `pool.reader()` doesn't exist, tuple Encode/Decode mismatch) — none triggered because the read-first list was thorough and the analog helper was studied before writing.

## Issues Encountered

None.

## User Setup Required

None — pure-Rust module + DB helper. No new env vars, no config changes, no operator-visible surface. The `HistogramCard` consumer (job_detail handler view-model) lands in plan 21-05; the FCTX panel consumer (run_detail handler) lands in plan 21-04.

## Next Phase Readiness

- **Plan 21-04 (run_detail handler + FCTX panel)** can import `crate::web::exit_buckets` if it ever needs to bucket a single run's exit_code for display (the FCTX panel itself reads scheduled_for from `DbRunDetail`, not the histogram, so this dependency is theoretical for plan 21-04).
- **Plan 21-05 (job_detail handler + Exit Histogram card)** is the primary consumer:
  - Call `queries::get_recent_runs_for_histogram(&state.pool, job_id, 100).await?` to fetch the raw rows.
  - Pass the result to `crate::web::exit_buckets::aggregate(&rows)` to get a `HistogramCard`.
  - Map `HistogramCard` → askama template view-model with the 10 bucket counts (per UI-SPEC `21-UI-SPEC.md` § Component Inventory § "10 bucket short-labels (locked)" lines 351-363) + success_rate badge (EXIT-03) + top-3 codes sub-table (EXIT-05) + has_min_samples → "Not enough data yet" empty state (D-11 / D-15 / D-16).
- **Plan 21-09 / 21-10 integration tests** can seed runs with mixed statuses + exit codes via direct SQL and assert against `aggregate` output; the in-module unit tests already lock the math, so integration tests focus on the SQL → Rust → askama pipeline, not the bucket math.

## Threat Flags

None — the new helper queries an existing table with no new operator-visible surface; the new module is a pure-Rust function with no I/O. Threat register entries T-21-03-01 (Tampering on `categorize`, accept), T-21-03-02 (Information Disclosure on `TopCode.last_seen`, accept), and T-21-03-03 (Denial of Service on `aggregate` cost, accept) all remain valid as written: bucketed input is server-typed (`&str` / `Option<i32>` / `Option<String>`), no operator parsing surface; aggregate is O(N) over fixed N=100 LIMIT-bounded rows; HashMap of at most 10 buckets + ~100 unique codes.

## Self-Check: PASSED

- File `src/web/exit_buckets.rs` — FOUND (321 lines)
- File `src/web/mod.rs` — FOUND (modified; `pub mod exit_buckets;` registered at line 4)
- File `src/db/queries.rs` — FOUND (modified; `get_recent_runs_for_histogram` added)
- Commit `c418593` (Task 1) — FOUND in `git log`
- Commit `916e728` (Task 2) — FOUND in `git log`
- Commit `cd64c30` (Task 3) — FOUND in `git log`
- `grep -c "^    Bucket" src/web/exit_buckets.rs` returns 10 (10 enum variants in the block)
- `grep -c "pub fn categorize(status: &str, exit_code: Option<i32>) -> Option<ExitBucket>" src/web/exit_buckets.rs` returns 1
- `grep -c "pub fn aggregate(rows: &\[(String, Option<i32>, Option<String>)\]) -> HistogramCard" src/web/exit_buckets.rs` returns 1
- `grep -c "pub struct HistogramCard" src/web/exit_buckets.rs` returns 1
- `grep -c "pub struct TopCode" src/web/exit_buckets.rs` returns 1
- `grep -c "^#\[cfg(test)\]" src/web/exit_buckets.rs` returns 1
- `grep -cE "^\s*fn (categorize_all_10_buckets|status_discriminator_wins_137|success_rate_excludes_stopped|top_3_codes_last_seen|below_min_samples_threshold|zero_samples_brand_new_job)" src/web/exit_buckets.rs` returns 6
- `grep -cE "^\s*Success," src/web/exit_buckets.rs` returns 0 (no Success variant per EXIT-03 / D-07)
- `grep -c "^pub mod exit_buckets;$" src/web/mod.rs` returns 1
- `grep -c "pub async fn get_recent_runs_for_histogram(" src/db/queries.rs` returns 1
- `grep -c "Vec<(String, Option<i32>, Option<String>)>" src/db/queries.rs` returns 1
- WHERE clauses: `WHERE job_id = ?1` (sqlite arm) + `WHERE job_id = $1` (postgres arm) — both present, neither filters by status (D-06 compliance)
- ORDER BY start_time DESC + LIMIT placeholders present on both arms
- `cargo nextest run -E 'test(/exit_buckets::tests/)'` — 6 tests run, 6 passed, 559 skipped
- `cargo build --workspace` — exits 0
- `cargo build --tests --workspace` — exits 0
- `cargo tree -i openssl-sys` — returns "package ID specification ... did not match any packages" (D-32 invariant)

---
*Phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2*
*Plan: 03*
*Completed: 2026-05-02*
