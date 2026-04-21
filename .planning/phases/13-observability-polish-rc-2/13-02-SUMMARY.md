---
phase: 13
plan: 02
subsystem: db-queries
tags: [observability, db-queries, dual-path-sql, sparkline, duration, timeline, OBS-02, OBS-03, OBS-04, OBS-05]

dependency-graph:
  requires:
    - "src/db/queries.rs::PoolRef (shipped)"
    - "src/db/queries.rs::DbPool::reader() (shipped)"
    - "idx_job_runs_start_time (migrations/{sqlite,postgres}/20260410_000000_initial.up.sql)"
    - "job_runs.job_run_number column (Phase 11 DB-11)"
    - "job_runs status lowercase literals: success, failed, timeout, cancelled, stopped, running"
  provides:
    - "queries::get_dashboard_job_sparks(pool) -> Vec<DashboardSparkRow>"
    - "queries::get_recent_successful_durations(pool, job_id, limit) -> Vec<u64>"
    - "queries::get_timeline_runs(pool, window_start_rfc3339) -> Vec<TimelineRun>"
    - "queries::DashboardSparkRow struct"
    - "queries::TimelineRun struct"
  affects:
    - "Wave 2 handlers in plan 03 (p50/p95 consumer via percentile())"
    - "Wave 2 handlers in plan 04 (dashboard sparkline hydration)"
    - "Wave 2 handlers in plan 05 (/timeline page)"

tech-stack:
  added: []
  patterns:
    - "Dual-path PoolRef match (SQLite ?N placeholders + j.enabled = 1; Postgres \\$N placeholders + j.enabled = true) — mirrors shipped get_dashboard_jobs"
    - "ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id DESC) subquery for per-job last-N window (extension of get_dashboard_jobs lr.rn = 1 single-row window)"
    - "Raw-rows-out contract: queries return Vec of typed structs; aggregation/folding happens in Rust (OBS-05 structural parity — no SQL-native percentile)"
    - "Lowercase status literals identical across SQLite/Postgres branches"
    - "Hard SQL LIMIT literal for timeline (never parameterized) per OBS-02"

key-files:
  created: []
  modified:
    - path: "src/db/queries.rs"
      exports: ["get_dashboard_job_sparks", "get_recent_successful_durations", "get_timeline_runs", "DashboardSparkRow", "TimelineRun"]
      lines-added: 233

decisions:
  - "Timeline filters on jr.start_time (not end_time) to let idx_job_runs_start_time fire on both backends; runs that started before the window but ended inside it are intentionally excluded (Research Open Question #1 resolution per plan Assumption A2)"
  - "Sparkline query runs bind-less on both backends — SQL is identical (no placeholders), so one string is shared across both PoolRef arms"
  - "Return type Vec<u64> for get_recent_successful_durations is the type-level enforcement of OBS-05: percentile computation cannot leak into SQL"
  - "j.enabled = true used on the Postgres arm of get_timeline_runs to mirror the shipped get_dashboard_jobs Postgres arm exactly (consistency over micro-optimization; migration stores enabled as BIGINT but existing code uses = true comparison)"

metrics:
  duration: "5m 39s"
  completed: "2026-04-21T17:42:23Z"
  tasks-completed: 3
  commits: 3
  files-modified: 1
  lines-added: 233
  lines-deleted: 3
  tests-added: 0
  tests-passing: 185
  tests-regressed: 0
---

# Phase 13 Plan 02: DB Queries for Observability Surfaces Summary

Three new read-only SQL queries landed in a single atomic plan — sparkline window-partitioned fetch (OBS-03), strict-success duration sample fetch (OBS-04, OBS-05), and timeline index-hitting range fetch (OBS-01, OBS-02) — all using the dual-path `PoolRef` match, lowercase status literals, and no SQL-native percentile. Consumer handlers in Wave 2+ (plans 03/04/05) can now reference these three functions directly.

## Signatures shipped

### New structs

```rust
#[derive(Debug, Clone)]
pub struct DashboardSparkRow {
    pub job_id: i64,
    pub id: i64,
    pub job_run_number: i64,
    pub status: String,
    pub duration_ms: Option<i64>,
    pub start_time: String,
    pub rn: i64,
}

#[derive(Debug, Clone)]
pub struct TimelineRun {
    pub run_id: i64,
    pub job_id: i64,
    pub job_name: String,
    pub job_run_number: i64,
    pub status: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub duration_ms: Option<i64>,
}
```

### New functions

```rust
pub async fn get_dashboard_job_sparks(
    pool: &DbPool,
) -> anyhow::Result<Vec<DashboardSparkRow>>;

pub async fn get_recent_successful_durations(
    pool: &DbPool,
    job_id: i64,
    limit: i64,
) -> anyhow::Result<Vec<u64>>;

pub async fn get_timeline_runs(
    pool: &DbPool,
    window_start_rfc3339: &str,
) -> anyhow::Result<Vec<TimelineRun>>;
```

## SQL actually shipped (verbatim — audit traceability)

### Task 1 — OBS-03 sparkline (identical SQL on both backends, no bindings)

```
SELECT job_id, id, job_run_number, status, duration_ms, start_time, rn
FROM (
    SELECT job_id, id, job_run_number, status, duration_ms, start_time,
           ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id DESC) AS rn
    FROM job_runs
    WHERE status IN ('success','failed','timeout','cancelled','stopped')
) t
WHERE rn <= 20
ORDER BY job_id ASC, rn ASC
```

The status filter intentionally excludes `'running'` — only terminal runs render as sparkline cells. `ORDER BY job_id ASC, rn ASC` yields a layout the handler can bucket by `job_id` in a single pass.

### Task 2 — OBS-04 duration (strict status = 'success' only)

SQLite arm:

```
SELECT duration_ms FROM job_runs
WHERE job_id = ?1
  AND status = 'success'
  AND duration_ms IS NOT NULL
ORDER BY id DESC
LIMIT ?2
```

Postgres arm (identical except for placeholder shape):

```
SELECT duration_ms FROM job_runs
WHERE job_id = $1
  AND status = 'success'
  AND duration_ms IS NOT NULL
ORDER BY id DESC
LIMIT $2
```

Both arms map rows with `r.get::<i64, _>("duration_ms") as u64` → `Vec<u64>`. Return type is the OBS-05 structural parity enforcement: `Vec<u64>` leaves no room for a `percentile_cont` rewrite without a type-system break.

### Task 3 — OBS-01/02 timeline (index-hitting start_time predicate + hard 10 000 cap)

SQLite arm:

```
SELECT jr.id AS run_id,
       jr.job_id,
       j.name AS job_name,
       jr.job_run_number,
       jr.status,
       jr.start_time,
       jr.end_time,
       jr.duration_ms
FROM job_runs jr
JOIN jobs j ON j.id = jr.job_id
WHERE j.enabled = 1
  AND jr.start_time >= ?1
ORDER BY j.name ASC, jr.start_time ASC
LIMIT 10000
```

Postgres arm:

```
SELECT jr.id AS run_id,
       jr.job_id,
       j.name AS job_name,
       jr.job_run_number,
       jr.status,
       jr.start_time,
       jr.end_time,
       jr.duration_ms
FROM job_runs jr
JOIN jobs j ON j.id = jr.job_id
WHERE j.enabled = true
  AND jr.start_time >= $1
ORDER BY j.name ASC, jr.start_time ASC
LIMIT 10000
```

`LIMIT 10000` is hard-coded (never parameterized) per OBS-02. Filter predicate is on `jr.start_time` (not `end_time`) so `idx_job_runs_start_time` can be used by the query planner on both backends — index-usage assertion itself is deferred to plan 06's `EXPLAIN QUERY PLAN` / `EXPLAIN ANALYZE` test (per validation strategy Wave 0 plan).

## Postgres row-mapping pattern — mirrors shipped style

Both backends store `job_runs.start_time` / `job_runs.end_time` as `TEXT` (verified at `migrations/sqlite/20260410_000000_initial.up.sql:30-31` and `migrations/postgres/20260410_000000_initial.up.sql:37-38`), so `r.get("start_time")` (into `String`) and `r.get("end_time")` (into `Option<String>`) work identically on both arms — no `DateTime<Utc>::to_rfc3339()` conversion needed. This matches the shipped pattern in:

- `get_dashboard_jobs` (lines 588-601 / 638-651): `last_run_time: r.get("last_run_time")` — Postgres `last_run_time` is `Option<String>`, same as SQLite.
- `get_run_history` (lines 731-739 / 765-774): `start_time: r.get("start_time"), end_time: r.get("end_time")` — both arms identical.

The three new queries follow the exact same pattern — no new row-conversion logic introduced.

## Verification results

### Automated (plan-required)

```
$ cargo nextest run --lib -E 'test(db::queries::tests)'
21 tests run: 21 passed, 164 skipped

$ cargo nextest run --lib
185 tests run: 185 passed, 0 skipped

$ cargo build --lib
Finished `dev` profile in 7.90s

$ cargo clippy --lib -- -D warnings
Finished `dev` profile in 2.95s (zero warnings)

$ cargo fmt --check
(clean)
```

Green baseline — no regressions across the full library test suite.

### Structural checks (plan verification section)

- Three new fns present: `grep -c 'pub async fn get_dashboard_job_sparks\|pub async fn get_recent_successful_durations\|pub async fn get_timeline_runs'` → **3** (matches expected).
- No SQL-native percentile: `grep -E 'percentile_cont|percentile_disc|PERCENTILE'` matches only in a doc comment explicitly declaring these are NOT used. No match in any SQL string.
- `ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id DESC)` literal present at line 685 (Task 1 acceptance).
- `jr.start_time >= ?1` / `$1` present at lines 830 / 864 (Task 3 acceptance).
- `Vec<u64>` return type on `get_recent_successful_durations` (Task 2 OBS-05 structural enforcement).

## Deviations from Plan

None — plan executed exactly as written. No Rule 1/2/3 auto-fixes needed; the three queries compiled clean on first build. No architectural decisions (Rule 4) encountered.

One minor formatting adjustment landed as part of Task 3's commit: `cargo fmt` collapsed the multi-line signature of `get_dashboard_job_sparks` (originally written across multiple lines) onto a single line to pass `cargo fmt --check`. This is cosmetic and does not affect the plan's acceptance criteria.

## Commits

| Task | Hash      | Message                                                                          |
| ---- | --------- | -------------------------------------------------------------------------------- |
| 1    | `ead488a` | `feat(13-02): add DashboardSparkRow + get_dashboard_job_sparks query (OBS-03)`   |
| 2    | `c672b8b` | `feat(13-02): add get_recent_successful_durations query (OBS-04, OBS-05)`        |
| 3    | `2b4d0a9` | `feat(13-02): add TimelineRun + get_timeline_runs query (OBS-01, OBS-02)`        |

## Known Stubs

None. All three functions return live query results from the reader pool; no hard-coded empty arrays, no placeholder text, no TODO/FIXME markers, no components wired to mock data.

## Threat Flags

None — plan 02 adds only read-only `SELECT` queries against existing trust boundaries. No new network endpoints, auth paths, file access patterns, or schema changes at trust boundaries. The plan's `<threat_model>` already captures all five relevant threats (T-13-02-01 through T-13-02-05) with disposition `mitigate` or `accept` per the pre-existing design.

## Self-Check: PASSED

**Files verified:**

```
$ ls -la src/db/queries.rs
-rw-r--r--@ 1 Robert  staff  76073 Apr 21 17:40 src/db/queries.rs
FOUND: src/db/queries.rs
```

**Commits verified:**

```
$ git log --oneline | grep -E 'ead488a|c672b8b|2b4d0a9'
2b4d0a9 feat(13-02): add TimelineRun + get_timeline_runs query (OBS-01, OBS-02)
c672b8b feat(13-02): add get_recent_successful_durations query (OBS-04, OBS-05)
ead488a feat(13-02): add DashboardSparkRow + get_dashboard_job_sparks query (OBS-03)
FOUND: ead488a
FOUND: c672b8b
FOUND: 2b4d0a9
```

All three task commits present on HEAD. Functions and structs grep clean. Build + clippy + fmt + db::queries::tests + full lib test suite all green. Plan 13-02 complete.
