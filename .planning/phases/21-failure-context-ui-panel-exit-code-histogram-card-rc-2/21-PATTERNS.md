# Phase 21: Failure-Context UI Panel + Exit-Code Histogram Card — rc.2 — Pattern Map

**Mapped:** 2026-05-01
**Files analyzed:** 13 (2 new SQL migrations, 1 new Rust module, 2 new integration test files, 8 modified Rust/HTML/CSS/justfile/test surfaces)
**Analogs found:** 13 / 13 — every Phase 21 file has a strong existing analog

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `migrations/sqlite/20260502_000009_scheduled_for_add.up.sql` | migration (DDL) | schema-add | `migrations/sqlite/20260427_000005_image_digest_add.up.sql` | exact |
| `migrations/postgres/20260502_000009_scheduled_for_add.up.sql` | migration (DDL) | schema-add | `migrations/postgres/20260427_000005_image_digest_add.up.sql` | exact |
| `src/web/exit_buckets.rs` (NEW) | utility / aggregator module | transform (stateless math) | `src/web/stats.rs` | exact (sibling) |
| `src/web/mod.rs` (modified — add `pub mod exit_buckets;`) | module index | module-declaration | `src/web/mod.rs:6` (`pub mod stats;`) | exact (in-file) |
| `src/db/queries.rs::insert_running_run` (signature widen) | DB write helper | CRUD-write | `src/db/queries.rs::finalize_run` (P16 widening pattern) | role-match (pre-start vs post-start data, but same widen-by-one-Option-arg shape) |
| `src/db/queries.rs::DbRunDetail` + `get_run_by_id` (field add) | DB row + read helper | CRUD-read | `src/db/queries.rs:617` `image_digest: Option<String>` field (P16 FOUND-14) | exact |
| `src/db/queries.rs::get_recent_runs_for_histogram` (NEW helper) | DB read helper | CRUD-read | `src/db/queries.rs::get_recent_successful_durations` (Phase 13 OBS-04) | exact (raw last-100 fetch with same idx) |
| `src/scheduler/run.rs::run_job` (signature widen) | scheduler job-runner | event-driven | `src/scheduler/run.rs:86` (current call site of `insert_running_run`) | self-modify |
| `src/web/handlers/run_detail.rs` (FCTX wire-up) | HTTP handler | request-response (read) | `src/web/handlers/job_detail.rs::job_detail` (Duration card hydration at lines 234-292) | exact (sibling handler with same fetch + view-build + soft-fail-degrade pattern) |
| `src/web/handlers/job_detail.rs` (histogram wire-up) | HTTP handler | request-response (read) | `src/web/handlers/job_detail.rs::job_detail` (Duration card existing site, lines 234-292) | exact (extends self) |
| `templates/pages/run_detail.html` (FCTX panel insert) | askama template | server-render | `templates/pages/run_detail.html:32-73` (existing metadata card chrome) | exact |
| `templates/pages/job_detail.html` (histogram card insert) | askama template | server-render | `templates/pages/job_detail.html:70-94` (existing Duration card) | exact (sibling card) |
| `assets/src/app.css` (cd-fctx-* + cd-exit-* additions) | stylesheet | static-css | `assets/src/app.css:444-490` (`.cd-tooltip*` + `.cd-timeline-bar:hover` block) | exact (same `@layer components` pattern) |
| `tests/v12_fctx_panel.rs` (NEW) | integration test (rendered HTML) | request-response | `tests/v13_duration_card.rs` + `tests/v13_timeline_render.rs` + `tests/job_detail_partial.rs` | exact (composite — sample-threshold matrix from duration_card; full router from timeline_render; minimal-router single-route from job_detail_partial) |
| `tests/v12_exit_histogram.rs` (NEW) | integration test (rendered HTML + unit) | request-response + transform | `tests/v13_duration_card.rs` (threshold + render) + `src/web/stats.rs::tests` (unit math) | exact |
| `tests/v12_fctx_explain.rs` (extension only) | integration test (EXPLAIN) | DDL-introspection | self (`tests/v12_fctx_explain.rs:115-208`) | self-extend |
| `justfile` (3 new uat-* recipes) | task runner | shell-orchestration | `justfile:337-352` (`uat-webhook-fire`/`uat-webhook-verify` family — `recipe-calls-recipe`) | exact |

## Pattern Assignments

### `migrations/sqlite/20260502_000009_scheduled_for_add.up.sql` (migration, schema-add)

**Analog:** `migrations/sqlite/20260427_000005_image_digest_add.up.sql` (entire file, 19 lines).

**Header-comment + ALTER pattern** (lines 1-18):
```sql
-- Phase 16: job_runs.image_digest per-run column (FOUND-14, FCTX-04).
--
-- Nullable TEXT, FOREVER (D-01): docker jobs populate this from
-- post-start `inspect_container().image` at finalize time; command and
-- script jobs legitimately have no image and leave the column NULL;
-- pre-v1.2 docker rows also stay NULL forever (D-04 — no backfill).
--
-- Pairs with migrations/postgres/20260427_000005_image_digest_add.up.sql.
-- Any structural change MUST land in both files in the same PR;
-- tests/schema_parity.rs::normalize_type collapses TEXT-family types to
-- TEXT, so this column passes parity with zero test edits (RESEARCH §E).
--
-- Idempotency: sqlx _sqlx_migrations tracking. SQLite ALTER TABLE ADD
-- COLUMN does NOT support a conditional-existence guard clause
-- (RESEARCH Pitfall 3 — Postgres pair uses one; SQLite cannot).
-- Re-runs are guarded by sqlx's migration ledger.

ALTER TABLE job_runs ADD COLUMN image_digest TEXT;
```

**Phase 21 divergence:** rename column `image_digest` → `scheduled_for`; update header-comment provenance to "Phase 21: job_runs.scheduled_for per-run column (FCTX-06)" with cite to P21 D-01 + D-05; mention pair file path `migrations/postgres/20260502_000009_scheduled_for_add.up.sql`. Body line is literally `ALTER TABLE job_runs ADD COLUMN scheduled_for TEXT;`. No index, no DEFAULT.

---

### `migrations/postgres/20260502_000009_scheduled_for_add.up.sql` (migration, schema-add)

**Analog:** `migrations/postgres/20260427_000005_image_digest_add.up.sql` (entire file, 17 lines).

**Header-comment + ALTER pattern** (lines 1-17):
```sql
-- Phase 16: job_runs.image_digest per-run column (FOUND-14, FCTX-04).
--
-- Nullable TEXT, FOREVER (D-01): docker jobs populate this from
-- post-start `inspect_container().image` at finalize time; command and
-- script jobs legitimately have no image and leave the column NULL;
-- pre-v1.2 docker rows also stay NULL forever (D-04 — no backfill).
--
-- Pairs with migrations/sqlite/20260427_000005_image_digest_add.up.sql.
-- Any structural change MUST land in both files in the same PR;
-- tests/schema_parity.rs::normalize_type collapses TEXT/VARCHAR/CHARACTER
-- VARYING/CHAR/CHARACTER to TEXT, so this column passes parity with zero
-- test edits (RESEARCH §E).
--
-- Idempotency: Postgres `IF NOT EXISTS` provides re-run safety even if
-- sqlx's _sqlx_migrations ledger is somehow out of sync.

ALTER TABLE job_runs ADD COLUMN IF NOT EXISTS image_digest TEXT;
```

**Phase 21 divergence:** Postgres adds `IF NOT EXISTS` (SQLite cannot — keep these two header comments synchronized in their `Idempotency:` paragraph). Final line: `ALTER TABLE job_runs ADD COLUMN IF NOT EXISTS scheduled_for TEXT;`.

---

### `src/web/exit_buckets.rs` (NEW utility module, transform)

**Analog:** `src/web/stats.rs` (entire file, 84 lines — module shape, doc-comment block, single `pub fn`, in-file `#[cfg(test)] mod tests`).

**File-level doc-comment + `pub fn` shape** (lines 1-23):
```rust
//! Pure-Rust percentile helper (Phase 13 OBS-04 / D-19).
//!
//! Algorithm: nearest-rank, 1-indexed. Always returns an observed sample —
//! never an interpolated value that didn't occur. Matches the percentile
//! semantics documented in `.planning/phases/13-observability-polish-rc-2/13-CONTEXT.md` § D-19.
//!
//! OBS-05 structural-parity: this module is the ONLY path by which p50/p95
//! are computed, regardless of whether the backend is SQLite or Postgres.
//! Do NOT introduce a SQL-native variant on Postgres.

/// Returns the q-th percentile of `samples` using the 1-indexed nearest-rank
/// method. `q` is a fraction in `[0.0, 1.0]`. Returns `None` for empty input.
pub fn percentile(samples: &[u64], q: f64) -> Option<u64> {
    if samples.is_empty() {
        return None;
    }
    let n = samples.len();
    let mut sorted: Vec<u64> = samples.to_vec();
    sorted.sort_unstable();
    let rank = (q * n as f64).ceil() as usize;
    let idx = rank.saturating_sub(1).min(n - 1);
    Some(sorted[idx])
}
```

**Embedded test pattern** (lines 25-83):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_slice_returns_none() {
        assert_eq!(percentile(&[], 0.5), None);
        // ... edge cases
    }

    #[test]
    fn p50_p95_over_hundred_samples() {
        let samples: Vec<u64> = (1..=100).collect();
        assert_eq!(percentile(&samples, 0.5), Some(50));
        assert_eq!(percentile(&samples, 0.95), Some(95));
    }
}
```

**Phase 21 divergence:**
- File doc-comment cites Phase 21 / EXIT-01..EXIT-05 instead of Phase 13 / OBS-04.
- Public surface is **two** `pub fn`s + **one** `pub enum` (10 variants) + **one** `pub struct HistogramCard` instead of `stats.rs`'s single function (per CONTEXT D-07).
- `categorize` returns `Option<ExitBucket>` (research D resolution — `None` signals success path; aggregator routes to success-rate stat). Per research §D the aggregator filters `None`.
- `aggregate` consumes `&[(String, Option<i32>, Option<String>)]` (research §C — owned tuples, no dedicated `RawRunRow` struct; mirrors `Vec<u64>` precedent in `stats.rs`).
- Sample-threshold gating (`has_min_samples = sample_count >= 5`) lives in the aggregator's struct return, NOT the call site. Mirrors `DurationView.has_min_samples` (`src/web/handlers/job_detail.rs:104`).
- Tests cover all 10 buckets (EXIT-02), the status-discriminator-wins 137 dual-classifier (EXIT-04), success-rate-excludes-stopped (D-09), top-3 last-seen (EXIT-05), below-N=5 empty (D-15), zero-samples (D-16). The unit-test block stays in-file like `stats.rs:25-83`.

---

### `src/web/mod.rs` (one-line additive — `pub mod exit_buckets;`)

**Analog:** `src/web/mod.rs:1-6` (existing `pub mod` block).

**Insertion target** (lines 1-6, verbatim from current file):
```rust
pub mod ansi;
pub mod assets;
pub mod csrf;
pub mod format;
pub mod handlers;
pub mod stats;
```

**Phase 21 divergence:** insert `pub mod exit_buckets;` between `format` (line 4) and `handlers` (line 5) for alphabetical correctness, OR append after line 6 alongside `stats` (sibling-style — research §"D-07 verification"). Recommendation: alphabetical position (line 5), so the surface looks like `ansi → assets → csrf → exit_buckets → format → handlers → stats`. Single-line edit.

---

### `src/db/queries.rs::insert_running_run` (signature widen + SQL extend)

**Analog:** `src/db/queries.rs::finalize_run` (lines 444-491 — the P16 widening precedent).

**P16 widening pattern (header + signature + SQL bind extension)** — extracting the load-bearing 4 sub-patterns from `finalize_run`:

1. **`#[allow(clippy::too_many_arguments)]` justification block** (lines 437-442):
```rust
/// `#[allow(clippy::too_many_arguments)]`: the 8-arg shape mirrors the
/// `job_runs` row's terminal write surface (status, exit_code, end_time,
/// duration_ms, error_message, container_id, image_digest). Bundling these
/// into a struct would re-marshal data that is already in scope at every
/// caller; the param list IS the schema. Phase 16 FOUND-14 widened from 7
/// to 8 to add `image_digest` alongside `container_id`.
#[allow(clippy::too_many_arguments)]
```

2. **Signature widen — `Option<&str>` of an RFC3339 string** (lines 444-453):
```rust
pub async fn finalize_run(
    pool: &DbPool,
    run_id: i64,
    status: &str,
    exit_code: Option<i32>,
    start_instant: tokio::time::Instant,
    error_message: Option<&str>,
    container_id: Option<&str>,
    image_digest: Option<&str>, // Phase 16 FOUND-14
) -> anyhow::Result<()> {
```

3. **SQL extension via positional bind** (lines 459-470, sqlite arm):
```rust
sqlx::query(
    "UPDATE job_runs SET status = ?1, exit_code = ?2, end_time = ?3, duration_ms = ?4, error_message = ?5, container_id = ?6, image_digest = ?7 WHERE id = ?8",
)
.bind(status)
.bind(exit_code)
.bind(&now)
.bind(duration_ms)
.bind(error_message)
.bind(container_id)
.bind(image_digest) // Phase 16 FOUND-14: NEW bind, position ?7
.bind(run_id)
.execute(p)
.await?;
```

**Current `insert_running_run` shape that gets extended** (lines 372-432, condensed):
```rust
pub async fn insert_running_run(
    pool: &DbPool,
    job_id: i64,
    trigger: &str,
    config_hash: &str, // Phase 16 FCTX-04
) -> anyhow::Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    // ... reserve job_run_number via UPDATE jobs ...
    let run_id: i64 = sqlx::query_scalar(
        "INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number, config_hash) \
         VALUES (?1, 'running', ?2, ?3, ?4, ?5) RETURNING id",
    )
    .bind(job_id)
    .bind(trigger)
    .bind(&now)
    .bind(reserved)
    .bind(config_hash) // Phase 16 FCTX-04: NEW bind, position ?5
    .fetch_one(&mut *tx)
    .await?;
    // ... commit ...
}
```

**Phase 21 divergence (after the widen):**
- New trailing arg: `scheduled_for: Option<&str>` (research §A — `&str` of an RFC3339 string, NOT `Option<DateTime<X>>`; aligns with codebase convention at lines 72, 378, 454).
- INSERT column-list extends from `(job_id, status, trigger, start_time, job_run_number, config_hash)` → `(..., config_hash, scheduled_for)`; VALUES extends from `(?1, 'running', ?2, ?3, ?4, ?5)` → `(..., ?5, ?6)` for sqlite and `($1..$5, $6)` for postgres.
- New `.bind(scheduled_for)` after `.bind(config_hash)` — position `?6` / `$6`.
- Add the `#[allow(clippy::too_many_arguments)]` attribute + 5-line justification block above `pub async fn` mirroring lines 437-442 (now 5 args including `pool`, justifying the widening with cite to P21 D-02).
- Each of 22 test callers + 5 production callers gets a trailing `None` (test path) or `Some(scheduled_for_str)` (scheduler) added — research Landmine §2 enumerates all 22.

**Note on `finalize_run`:** does NOT change. Per research §D-03 verified, `scheduled_for` is set ONCE at insert; finalize never updates it.

---

### `src/db/queries.rs::DbRunDetail` (field add) + `get_run_by_id` (SELECT extend)

**Analog:** `src/db/queries.rs::DbRunDetail` field block (lines 615-622) — the P16 FOUND-14 + FCTX-04 precedent.

**Field-add pattern** (lines 615-622):
```rust
/// Phase 16 FOUND-14: image digest from post-start `inspect_container`. NULL for
/// command/script jobs (no image), pre-v1.2 docker rows (capture site landed in v1.2).
pub image_digest: Option<String>,
/// Phase 16 FCTX-04: per-run config_hash captured at fire time by
/// `insert_running_run`. NULL for pre-v1.2 rows whose backfill found no matching
/// `jobs.config_hash`. See migration `*_000007_config_hash_backfill.up.sql` for
/// the BACKFILL_CUTOFF_RFC3339 marker (D-03).
pub config_hash: Option<String>,
```

**SELECT-extend pattern** (`get_run_by_id` lines 1300-1336, both backends):
```rust
let sql_sqlite = r#"
    SELECT r.id, r.job_id, r.job_run_number, j.name AS job_name, r.status, r.trigger,
           r.start_time, r.end_time, r.duration_ms, r.exit_code, r.error_message,
           r.image_digest, r.config_hash
    FROM job_runs r
    JOIN jobs j ON j.id = r.job_id
    WHERE r.id = ?1
"#;
// ... mirror sql_postgres with $1 ...
Ok(row.map(|r| DbRunDetail {
    id: r.get("id"),
    // ... all existing fields ...
    image_digest: r.get("image_digest"), // Phase 16 FOUND-14
    config_hash: r.get("config_hash"),   // Phase 16 FCTX-04
}))
```

**Phase 21 divergence:**
- New field on `DbRunDetail`: `pub scheduled_for: Option<String>,` after `config_hash` with doc-comment citing P21 FCTX-06 / D-01 + the NULL-on-legacy-rows posture (D-04).
- Both SQL strings (sqlite + postgres) get `, r.scheduled_for` appended to the SELECT column list before `FROM`.
- Both `.map(|r| DbRunDetail { ... })` constructions get `scheduled_for: r.get("scheduled_for"),` appended after `config_hash`.

---

### `src/db/queries.rs::get_recent_runs_for_histogram` (NEW raw-fetch helper)

**Analog:** `queries::get_recent_successful_durations` — referenced in `src/web/handlers/job_detail.rs:246` and consumed at line 250. This helper is the v1.1 OBS-04 raw-fetch precedent that Phase 21's histogram fetch mirrors in shape (last-100, ORDER BY start_time DESC, single SELECT, NO bucketing in SQL).

**Call-site pattern** (`src/web/handlers/job_detail.rs:243-250`):
```rust
const PERCENTILE_SAMPLE_LIMIT: i64 = 100;

let durations =
    queries::get_recent_successful_durations(&state.pool, job_id, PERCENTILE_SAMPLE_LIMIT)
        .await
        .unwrap_or_default();

let sample_count = durations.len();
```

**Phase 21 divergence:**
- Phase 21 helper signature: `pub async fn get_recent_runs_for_histogram(pool: &DbPool, job_id: i64, limit: i64) -> anyhow::Result<Vec<(String, Option<i32>, Option<String>)>>` — returning owned `(status, exit_code, end_time)` tuples per research §C.
- WHERE clause is **all statuses** (NOT `status='success'` only — EXIT-01 covers ALL last-100 runs including stopped/cancelled/timeout).
- ORDER BY same `start_time DESC`; LIMIT same parameter.
- Both backend arms (`PoolRef::Sqlite(p)` / `PoolRef::Postgres(p)`) following the lines 735-754 `get_failure_context` arm pattern.
- Soft-fail via `.unwrap_or_default()` in the consuming handler — same as line 248. **Plus** a new `tracing::warn!` per landmine §1 (see api.rs analog below).

---

### `src/scheduler/run.rs::run_job` (signature widen + scheduler/mod.rs caller update)

**Analog:** `src/scheduler/run.rs:71-122` (current `run_job` body — self-modify).

**Current call-site** (line 86):
```rust
let run_id = match insert_running_run(&pool, job.id, &trigger, &job.config_hash).await {
    Ok(id) => id,
    Err(e) => {
        tracing::error!(
            target: "cronduit.run",
            job = %job.name,
            error = %e,
            "failed to insert running run"
        );
        return RunResult { run_id: 0, status: "error".to_string() };
    }
};
```

**Phase 21 divergence:**
- `run_job` signature gains `scheduled_for: Option<String>` (owned `String` so it can cross the `tokio::spawn` await boundary in `scheduler/mod.rs:152` without lifetime gymnastics).
- The line-86 call becomes `insert_running_run(&pool, job.id, &trigger, &job.config_hash, scheduled_for.as_deref()).await`.
- Caller in `src/scheduler/mod.rs:152-160` (the `for entry in &due` loop, where `entry: &fire::FireEntry` has `entry.fire_time: DateTime<chrono_tz::Tz>` in scope) passes `Some(entry.fire_time.to_rfc3339())` as the new arg.
- Caller in `src/scheduler/mod.rs:194-211` (legacy `RunNow` cmd arm) passes `None` — research landmine §9 confirms this arm doesn't fire today (api.rs uses `RunNowWithRunId` per Phase 11 UI-19 fix); kept as defensive fallback.
- `src/web/handlers/api.rs:82` (the live Run Now path) passes `Some(now_rfc3339_string.as_str())` so `scheduled_for == start_time` for skew=0ms by definition (research landmine §7).

---

### `src/web/handlers/run_detail.rs` (FCTX panel wire-up + soft-fail)

**Analog (degradation pattern):** `src/web/handlers/job_detail.rs:234-292` — the existing Duration card hydration block on the sibling handler. This is the **closest** existing pattern: a fetch → soft-fail-degrade → view-build → assemble-into-page-context flow.

**Duration-card hydration pattern** (lines 234-292):
```rust
const MIN_SAMPLES_FOR_PERCENTILE: usize = 20;
const PERCENTILE_SAMPLE_LIMIT: i64 = 100;

let durations =
    queries::get_recent_successful_durations(&state.pool, job_id, PERCENTILE_SAMPLE_LIMIT)
        .await
        .unwrap_or_default();

let sample_count = durations.len();
let has_min = sample_count >= MIN_SAMPLES_FOR_PERCENTILE;

let (p50_display, p95_display) = if has_min {
    let p50 = stats::percentile(&durations, 0.5)
        .expect("non-empty when sample_count >= MIN_SAMPLES_FOR_PERCENTILE (D-21)");
    let p95 = stats::percentile(&durations, 0.95)
        .expect("non-empty when sample_count >= MIN_SAMPLES_FOR_PERCENTILE (D-21)");
    (
        format_duration_ms_floor_seconds(Some(p50 as i64)),
        format_duration_ms_floor_seconds(Some(p95 as i64)),
    )
} else {
    ("—".to_string(), "—".to_string())
};

let duration_view = DurationView {
    p50_display,
    p95_display,
    has_min_samples: has_min,
    sample_count,
    sample_count_display,
};
```

**Analog (`tracing::warn!` field shape — REQUIRED per research landmine §1):** `src/web/handlers/api.rs:127-132` — the codebase's only handler-side `tracing::warn!` with `target: "cronduit.web"` + structured fields:
```rust
tracing::warn!(
    target: "cronduit.web",
    job_id,
    run_id,
    "run_now: scheduler channel closed — finalizing pre-inserted row as error"
);
```

**Anti-pattern to AVOID (research landmine §1):** the dashboard sparkline soft-fail at `src/web/handlers/dashboard.rs:262-264` uses `.unwrap_or_default()` ALONE — no warn. CONTEXT D-12 explicitly **upgrades** the pattern; the planner must NOT copy dashboard.rs verbatim.

**Phase 21 divergence (concrete shape per research §H):**
- Extend `RunDetailView` (lines 85-102) with three new fields: `image_digest: Option<String>`, `config_hash: Option<String>`, `scheduled_for: Option<String>` — pulled from the widened `DbRunDetail` (already provides `image_digest` and `config_hash` since P16; `scheduled_for` is new from the SELECT-extend above).
- Extend `RunDetailPage` (lines 33-50) with `show_fctx_panel: bool` and `fctx: Option<FctxView>`.
- New `FctxView` struct per research §H lines 290-300 (10 fields, all pre-formatted strings/bools so the askama template holds zero logic).
- Gating logic: `show_fctx_panel = matches!(run.status.as_str(), "failed" | "timeout")` per FCTX-01 — research landmine §11 explicitly says NOT to include `"error"`.
- Soft-fail wrapper around `get_failure_context()` call:
  ```rust
  let (show_fctx_panel, fctx) = if matches!(run.status.as_str(), "failed" | "timeout") {
      match queries::get_failure_context(&state.pool, run.job_id).await {
          Ok(ctx) => (true, Some(build_fctx_view(&run, ctx, &state.pool).await)),
          Err(e) => {
              tracing::warn!(
                  target: "cronduit.web",
                  job_id = run.job_id,
                  run_id = run.id,
                  error = %e,
                  "fctx panel: get_failure_context failed — hiding panel"
              );
              (false, None)
          }
      }
  } else {
      (false, None)
  };
  ```
- Research landmine §12: do NOT short-circuit the handler; the FCTX panel hides but the rest of the page renders normally. Mirrors the `fetch_logs` soft-fail at `run_detail.rs:137-146` which uses `tracing::error!` for the load-bearing log fetch and degrades to an empty `Paginated`.

---

### `src/web/handlers/job_detail.rs` (histogram card wire-up + soft-fail)

**Analog:** `src/web/handlers/job_detail.rs:234-292` (the existing Duration card hydration on the same handler — this becomes a **siblings on the same handler** wire-up, where the planner literally adds an analogous block immediately after the Duration card block).

**Duration-card → histogram-card adjacency pattern** — the new histogram block lives at lines ~292-300 (right after `let duration_view = DurationView { ... };`) and produces an `ExitHistogramView` field that gets attached to the existing `JobDetailView` (line 84).

**Concrete extension shape (mirrors lines 245-264 verbatim with histogram-specific bindings):**
```rust
const HISTOGRAM_SAMPLE_LIMIT: i64 = 100;

let raw_runs = queries::get_recent_runs_for_histogram(
    &state.pool,
    job_id,
    HISTOGRAM_SAMPLE_LIMIT,
)
.await
.unwrap_or_else(|e| {
    tracing::warn!(
        target: "cronduit.web",
        job_id,
        error = %e,
        "exit histogram: query failed — degraded card"
    );
    Vec::new()
});

let histogram = exit_buckets::aggregate(&raw_runs);
let exit_view = ExitHistogramView::from(&histogram);
```

**Phase 21 divergence:**
- New view-model `ExitHistogramView` adjacent to `DurationView` (lines 99-109) — pre-formats all bucket bars (height_pct, color_class, aria_label, tooltip_title, tooltip_detail per UI-SPEC § Component Inventory bucket-table), the success-rate stat, the recent-codes top-3 table, and the `has_min_samples` flag for the `{% if has_min_samples %}` template gate.
- `JobDetailPage` (lines 39-50) gains `exit_histogram: ExitHistogramView` adjacent to existing fields. (Note: `JobDetailView.duration` is the existing Duration card field at line 84 — Phase 21's `exit_histogram` lives at the **page level** rather than nested inside `JobDetailView`, since the histogram is the page-level second-tier section, mirroring `runs: Vec<RunHistoryView>` at line 44 which is also page-level.)
- Inline `tracing::warn!` is **inside** the `unwrap_or_else` closure rather than via a `match`. This shape is the single-handler-call equivalent of the run_detail.rs match block.
- Bar-height inline `style` attribute is computed in `ExitHistogramView::from` — pct is server-computed `i64` clamped to 0..100 BEFORE template render (research § Security Domain V5).

---

### `templates/pages/run_detail.html` (FCTX panel insert — between metadata card and Log Viewer)

**Analog:** the existing metadata card block `templates/pages/run_detail.html:32-73` — outer chrome that the new panel replicates, plus the existing card-after-card flow (metadata card → Log Viewer; Phase 21 inserts a third card between them).

**Existing metadata card chrome** (lines 32-33):
```html
<!-- Metadata card -->
<div style="background:var(--cd-bg-surface);border:1px solid var(--cd-border);border-radius:8px;padding:var(--cd-space-6)" class="mb-6">
```

**Existing match/option-render pattern** (lines 60-72, the closest askama precedent for the `{% match %}` + `{% when Some/None %}` + conditional row rendering Phase 21 needs):
```html
<div style="font-size:var(--cd-text-base);color:var(--cd-text-primary);margin-top:2px">
  {% match run.exit_code %}
    {% when Some with (code) %}{{ code }}{% when None %}N/A{% endmatch %}
</div>
```

**Insertion point** (between lines 73 and 75 — verbatim from research code-examples §"Existing askama template insert hook line"):
```html
  {% endmatch %}
</div>

<!-- Log Viewer -->
```

**Phase 21 divergence:**
- The new block uses the markup contract from UI-SPEC § Component Inventory § 1 (Failure-Context Panel) — `<details class="cd-fctx-panel mb-6">` + `<summary class="cd-fctx-summary">` + `<div class="cd-fctx-body">` + 5 conditionally-rendered rows.
- Top-level guard is `{% if show_fctx_panel %}{% if let Some(fctx) = fctx %} ... {% endif %}{% endif %}`.
- IMAGE DIGEST row is wrapped in `{% if fctx.is_docker_job %}` (FCTX-03 hide-on-non-docker).
- DURATION row is wrapped in `{% if fctx.has_duration_samples %}` (FCTX-05 hide-below-N=5).
- FIRE SKEW row is wrapped in `{% match fctx.fire_skew_value %}{% when Some with (v) %} ... {% when None %}{% endmatch %}` to hide cleanly on legacy NULL `scheduled_for` (D-04).
- All values are pre-formatted in `FctxView` (research §H), so the template carries no logic beyond `{{ ... }}` substitution + the conditional gates.
- NO `|safe` filters anywhere (UI-SPEC § Output Escaping & XSS).
- NO new `<script>` blocks (D-17 / UI-SPEC).

---

### `templates/pages/job_detail.html` (Histogram card insert — between Duration card and Run History)

**Analog:** the existing Duration card `templates/pages/job_detail.html:70-94` — sibling-card chrome the new histogram replicates verbatim per UI-SPEC § Layout & Surfaces "sibling card to Duration".

**Existing Duration card** (lines 70-94, full block):
```html
<!-- Duration (OBS-04) -->
<div style="background:var(--cd-bg-surface);border:1px solid var(--cd-border);border-radius:8px;padding:var(--cd-space-6)" class="mb-6">
  <h2 style="font-size:var(--cd-text-xl);font-weight:700;letter-spacing:-0.02em;margin-bottom:var(--cd-space-4)">Duration</h2>

  <div style="display:flex;gap:var(--cd-space-6);align-items:baseline">
    <div>
      <span style="font-size:var(--cd-text-xs);color:var(--cd-text-secondary);text-transform:uppercase;letter-spacing:0.1em;font-weight:700">p50</span>
      <div style="font-size:var(--cd-text-xl);color:var(--cd-text-primary);font-weight:700;letter-spacing:-0.02em;margin-top:2px"
           {% if !job.duration.has_min_samples %}title="insufficient samples: need 20 successful runs, currently have {{ job.duration.sample_count }}"{% endif %}>
        {{ job.duration.p50_display }}
      </div>
    </div>
    <div>
      <span style="...p95 stat...">p95</span>
      <div style="...p95 value...">{{ job.duration.p95_display }}</div>
    </div>
  </div>

  <div style="font-size:var(--cd-text-sm);color:var(--cd-text-secondary);margin-top:var(--cd-space-2)">
    {{ job.duration.sample_count_display }}
  </div>
</div>
```

**Insertion point** — between lines 94 and 96 (verbatim from research):
```html
  </div>
</div>

<!-- Run History -->
```

**Phase 21 divergence:**
- The new block uses the markup contract from UI-SPEC § Component Inventory § 2 (Exit-Code Histogram Card) — `<div class="cd-exit-card mb-6">` outer; `<h2 class="cd-exit-card-title">Exit Code Distribution</h2>`; success-rate stat; histogram chart (`<div class="cd-exit-chart" role="img">` with `{% for bucket in exit_histogram.buckets %}` loop); recent-codes table (`{% if !exit_histogram.top_codes.is_empty() %}`); empty-state branch under `{% else %}`.
- Top-level guard: `{% if exit_histogram.has_min_samples %} ... {% else %} ... {% endif %}` — D-15 below-N=5 empty state.
- **Bar-height inline `style="height:{{ bucket.height_pct }}%"` is the divergence from Duration**: Duration has zero `style=` interpolation; the histogram has one per bar (server-clamped numeric per research § Security V5).
- Bucket color uses `class="cd-exit-bar cd-exit-bar--{{ bucket.color_class }}"` — `bucket.color_class` is one of {`err-strong`, `err-muted`, `warn`, `stopped`, `null`} (UI-SPEC § Component Inventory CSS contract table maps each of the 10 buckets to one of these 5 modifier classes).
- Tooltip uses the existing `.cd-tooltip` / `.cd-tooltip-row` / `.cd-tooltip-dot` classes from Phase 13 — UI-SPEC explicitly says "REUSES Phase 13 `.cd-tooltip` rule".
- `{% if !exit_histogram.has_min_samples %}` empty-state copy is the locked `—` em-dash + `Need 5+ samples; have {{ exit_histogram.sample_count }}` substitution per UI-SPEC § Copywriting Contract.

---

### `assets/src/app.css` (cd-fctx-* + cd-exit-* additions)

**Analog:** `assets/src/app.css:444-490` — the existing `.cd-tooltip*` block inside `@layer components`, which Phase 21's new classes sit immediately adjacent to (UI-SPEC explicitly reuses these tooltip classes verbatim).

**Existing tooltip + hover-trigger pattern** (lines 444-490):
```css
.cd-tooltip {
  visibility: hidden;
  opacity: 0;
  position: absolute;
  bottom: calc(100% + 8px);
  left: 50%;
  transform: translateX(-50%);
  min-width: 220px;
  max-width: 320px;
  padding: var(--cd-space-2) var(--cd-space-3);
  background: var(--cd-bg-surface-raised);
  border: 1px solid var(--cd-border);
  border-radius: var(--cd-radius-md);
  /* ... */
}
.cd-timeline-bar:hover .cd-tooltip,
.cd-timeline-bar:focus-visible .cd-tooltip {
  visibility: visible;
  opacity: 1;
}
.cd-tooltip-row { display: block; }
.cd-tooltip-row + .cd-tooltip-row { margin-top: var(--cd-space-1); }
.cd-tooltip-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 9999px;
  margin-right: var(--cd-space-2);
  vertical-align: middle;
}
.cd-tooltip::after {
  content: "";
  position: absolute;
  top: 100%;
  left: 50%;
  transform: translateX(-50%);
  border: 6px solid transparent;
  border-top-color: var(--cd-border);
}
```

**Phase 21 divergence:**
- Phase 21 ADDS one new selector `.cd-exit-bar:hover .cd-tooltip, .cd-exit-bar:focus-visible .cd-tooltip { visibility: visible; opacity: 1; }` — a parallel rule to the existing `.cd-timeline-bar:hover .cd-tooltip` rule (anchor element changes from `.cd-timeline-bar` to `.cd-exit-bar`; tooltip inside is the same).
- All other Phase 21 classes (`.cd-fctx-panel`, `.cd-fctx-summary`, `.cd-fctx-body`, `.cd-fctx-row`, `.cd-fctx-row-*`, `.cd-fctx-mono-digest`, `.cd-exit-card`, `.cd-exit-card-title`, `.cd-exit-stats`, `.cd-exit-stat*`, `.cd-exit-chart`, `.cd-exit-bucket`, `.cd-exit-bar*`, `.cd-exit-bucket-label`, `.cd-exit-caption`, `.cd-exit-subhead`, `.cd-exit-recent`, `.cd-exit-empty*`) are NEW declarations, all property values from UI-SPEC § Component Inventory CSS contract tables — every value is `var(--cd-*)`, `calc(var(--cd-*) * N)`, or `0`. NO new tokens; NO bare px literals (UI-SPEC § Spacing § Exceptions: none).
- Insertion point: at the end of the existing `@layer components` block, after the Phase 13 timeline declarations (around current line 500+). Group them into 3 sub-blocks with header comments: `/* === Phase 21 FCTX panel === */`, `/* === Phase 21 exit-code histogram === */`, `/* === Phase 21 reduced-motion extension === */` (the last extends the existing `@media (prefers-reduced-motion: reduce)` block to include `.cd-fctx-summary-caret { transition: none }` per UI-SPEC § Interaction Contract).

---

### `tests/v12_fctx_panel.rs` (NEW integration test — panel render + gating + soft-fail)

**Analog (composite — three closest existing test files):**
1. **`tests/v13_duration_card.rs`** (lines 1-100, then full file ~400 lines) — closest for the **sample-threshold matrix + rendered-HTML byte-exact substring assertions** pattern. Phase 21's panel has a parallel matrix (status ∈ {failed, timeout} renders; success/cancelled/running/stopped hides; below-5 hides duration row).
2. **`tests/v13_timeline_render.rs`** (lines 1-100) — closest for the **full-router via `cronduit::web::router(state)` + `tower::ServiceExt::oneshot` + scan body for substrings** harness. Phase 21 needs the full router because it renders `/jobs/{job_id}/runs/{run_id}` which is a real route.
3. **`tests/v12_fctx_streak.rs`** (full file, 230+ lines) — closest for the **seed_run helper for raw INSERT with explicit column list** pattern. Phase 21 reuses this exact `seed_run` shape (line 67-84) so tests can write `scheduled_for` values directly.

**Test-app harness pattern from `v13_timeline_render.rs:32-58` (reusable verbatim — same harness for v12_fctx_panel.rs):**
```rust
async fn build_test_app() -> (axum::Router, DbPool) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);
    tokio::spawn(async move { while cmd_rx.recv().await.is_some() {} });

    let metrics_handle = setup_metrics();

    let state = AppState {
        started_at: chrono::Utc::now(),
        version: "test",
        pool: pool.clone(),
        cmd_tx,
        config_path: std::path::PathBuf::from("/tmp/cronduit-test.toml"),
        tz: chrono_tz::UTC,
        last_reload: Arc::new(Mutex::new(None::<ReloadState>)),
        watch_config: false,
        metrics_handle,
        active_runs: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    (router(state), pool)
}
```

**Seed pattern from `v12_fctx_streak.rs:67-84` (reusable — extend with `scheduled_for` arg):**
```rust
async fn seed_run(pool: &DbPool, job_id: i64, status: &str, time_index: i64) {
    let p = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let start_time = format!("2026-04-27T00:{:02}:00Z", time_index);
    sqlx::query(
        "INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number, image_digest, config_hash) \
         VALUES (?1, ?2, 'manual', ?3, ?4, NULL, 'seed-hash')",
    )
    .bind(job_id)
    .bind(status)
    .bind(&start_time)
    .bind(time_index)
    .execute(p)
    .await
    .expect("seed run");
}
```

**Phase 21 divergence:**
- New `seed_run_with_scheduled_for(pool, job_id, status, time_index, scheduled_for)` helper that extends the column list to `(..., config_hash, scheduled_for)` and binds the new column (Some/None) — covers FCTX-06 fire-skew test cases.
- Test scenarios per research § Validation Architecture lines 411-427:
  - `panel_renders_gated_on_failed_timeout` (FCTX-01)
  - `panel_hidden_on_non_failure_status` (FCTX-01 negative)
  - `time_deltas_row_renders` (FCTX-02)
  - `image_digest_row_hidden_on_command_job` (FCTX-03)
  - `duration_row_hidden_below_5_samples` (FCTX-05)
  - `fire_skew_row_hidden_on_null_scheduled_for` (FCTX-06 / D-04)
  - `fire_skew_row_renders_skew_ms` (FCTX-06)
  - `run_now_skew_zero` (FCTX-06 / Run Now writes scheduled_for == start_time)
  - `soft_fail_hides_panel_emits_warn` (D-12)
- Soft-fail assertion uses a tracing layer subscriber to capture the warn — pattern not yet in codebase; planner may need to add a small helper. Alternative: assert the panel section is absent from rendered HTML when the underlying row is unreachable (simpler shape; still proves the soft-fail).

---

### `tests/v12_exit_histogram.rs` (NEW integration + unit test — buckets + render + threshold)

**Analog (composite):**
1. **`src/web/stats.rs::tests`** (lines 25-83, embedded `#[cfg(test)] mod tests`) — closest for the **pure-function bucket categorization unit tests** (10 buckets × the dual-classifier 137 rule × top-3 last-seen × success-rate-excludes-stopped). These can live IN `src/web/exit_buckets.rs` itself per the `stats.rs:25-83` precedent — research §"Validation Architecture" maps EXIT-02..EXIT-05 to **`cargo test -p cronduit --lib`** unit tests, not integration tests.
2. **`tests/v13_duration_card.rs`** (full file) — closest for the **rendered-card-from-`/jobs/{id}` integration tests** that assert the histogram card chrome + below-N=5 empty state + bar height attributes appear in the body bytes.

**Phase 21 divergence:**
- The unit tests for `categorize`/`aggregate` go INSIDE `src/web/exit_buckets.rs` (`#[cfg(test)] mod tests`), mirroring `stats.rs:25-83`. NOT in `tests/v12_exit_histogram.rs`.
- `tests/v12_exit_histogram.rs` integration tests cover ONLY the rendered-page assertions (EXIT-01 below-N=5 empty state visible; histogram chart present when N≥5; bucket-class names present per UI-SPEC table; empty-state copy `—` + `Need 5+ samples; have N`).
- `seed_runs_with_status_and_exit(pool, job_id, count, status, exit_code)` helper extends the existing `seed_runs_with_duration` shape from `v13_duration_card.rs:97-` with `exit_code` and `status` params (research § Validation Architecture says "planner adds in 21-07/21-08").

---

### `tests/v12_fctx_explain.rs` (extend with index-plan assertion for new column)

**Analog:** the existing tests in this file (lines 115-208 for sqlite, 214+ for postgres) — Phase 21 only **extends** these.

**Existing assertion pattern** (lines 189-207, sqlite branch):
```rust
// Primary assertion (D-08): the plan references idx_job_runs_job_id_start.
// Both CTE arms should hit this (job_id, start_time DESC) covering index.
assert!(
    plan_text.contains("idx_job_runs_job_id_start"),
    "expected EXPLAIN QUERY PLAN to use idx_job_runs_job_id_start; got:\n{plan_text}"
);

// Secondary assertion (D-08): the plan must NOT show a bare SCAN job_runs
// (full table scan).
assert!(
    !plan_text.contains("SCAN job_runs") || plan_text.contains("USING INDEX"),
    "EXPLAIN must not show a bare SCAN job_runs (would mean full table scan); got:\n{plan_text}"
);
```

**Insert site for column list** (lines 149-151 — the existing INSERT bind list):
```rust
let insert_sql = "INSERT INTO job_runs \
    (job_id, job_run_number, status, trigger, start_time, image_digest, config_hash) \
    VALUES (?, ?, ?, 'manual', ?, NULL, 'seed-hash')";
```

**Phase 21 divergence:**
- Per research D-18 verification: **the existing INSERT does NOT need extension** — adding `scheduled_for TEXT NULL` to the schema leaves these inserts valid (SQLite/Postgres default the omitted column to NULL). Research landmine §10 confirms.
- The Phase 21 extension is a **new test function** (one per backend) that re-runs the same EXPLAIN QUERY PLAN assertion AFTER the migration with `scheduled_for` lands. The assertion is identical: `plan_text.contains("idx_job_runs_job_id_start")` + the negative SCAN assertion.
- Test names: `explain_uses_index_sqlite_post_scheduled_for` and `explain_uses_index_postgres_post_scheduled_for`.

---

### `justfile` (3 new uat-* recipes)

**Analog:** `justfile:337-352` — the `uat-webhook-fire`/`uat-webhook-verify` family showing the canonical `recipe-calls-recipe` pattern (Phase 18 D-25 precedent), plus `justfile:267-273` (`uat-fctx-bugfix-spot-check`) showing the raw `sqlite3` query pattern for inspection without a fixture seeder.

**`uat-webhook-fire` recipe shape (recipe-calls-recipe pattern)** (lines 335-342):
```just
[group('uat')]
[doc('Phase 18 — force Run Now on a webhook-configured job (operator-supplied JOB_NAME)')]
uat-webhook-fire JOB_NAME:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▶ UAT: triggering run for {{JOB_NAME}} — watch the receiver and the cronduit log"
    JOB_ID=$(just api-job-id "{{JOB_NAME}}")
    just api-run-now "$JOB_ID"
```

**`uat-fctx-bugfix-spot-check` raw-sqlite3 inspect pattern** (lines 265-273):
```just
[group('db')]
[doc('Phase 16 FOUND-14 spot check — container_id MUST NOT start with sha256:')]
uat-fctx-bugfix-spot-check:
    @echo "Phase 16 / FOUND-14 spot check"
    @echo "Most recent job_run container_id (must NOT start with 'sha256:'):"
    @sqlite3 cronduit.dev.db "SELECT id, job_id, status, container_id, image_digest FROM job_runs ORDER BY id DESC LIMIT 1;"
    @echo ""
    @echo "Expected: container_id is a real Docker container ID (or NULL for non-docker runs)."
    @echo "FAIL if: container_id starts with 'sha256:' (would indicate the bug regressed)."
```

**Existing primitives that the 3 new recipes compose from** (research landmine §4):
- `dev` (line 853 — start cronduit in dev mode)
- `db-reset` (line 237 — wipe `cronduit.dev.db`)
- `api-job-id JOB_NAME` (line 312 — name → id resolver)
- `api-run-now JOB_ID` (line 291 — CSRF-aware Run Now POST)
- raw `sqlite3 cronduit.dev.db "SELECT ..."` for inspection

**Phase 21 divergence — three new recipes:**
- `uat-fctx-panel` — composes `db-reset` → `dev` (background) → `api-job-id` → seed N failures via raw `sqlite3` INSERT (cite `v12_fctx_streak.rs::seed_run` as the schema reference) → echo URL `http://127.0.0.1:8080/jobs/{id}/runs/{id}` for maintainer to walk.
- `uat-exit-histogram` — composes `db-reset` → `dev` → seed mixed exit-code rows via raw `sqlite3` INSERT (status='success', 'failed' with exit_code 1/127/137-with-status='stopped', 'failed' with exit_code 137 via status='failed' for EXIT-04 dual-classifier) → echo URL `http://127.0.0.1:8080/jobs/{id}` for maintainer.
- `uat-fire-skew` — research §F resolution: slow-start container approach. Recipe seeds an `[[jobs]]` block in dev config with `image = "alpine:latest" command = ["sh","-c","sleep 30 && echo done"] schedule = "* * * * *"`, calls `db-reset` + `dev`, waits one full minute, prints the latest run's `scheduled_for` and `start_time` from `sqlite3 cronduit.dev.db`, echoes the URL for maintainer to confirm `+30000ms` skew.

All three recipes use the `[group('uat')]` + `[doc('Phase 21 — ...')]` attribute decorations exactly as `uat-webhook-fire` does. All three follow the `#!/usr/bin/env bash` + `set -euo pipefail` shebang convention.

---

## Shared Patterns

### Soft-fail with `tracing::warn!` (Phase 21 D-12 — UPGRADED from existing dashboard pattern)

**Source:** `src/web/handlers/api.rs:127-132` (the codebase's only handler-side `tracing::warn!` with `target: "cronduit.web"` + structured fields + `error = %e` + final string message).

**Apply to:** Both new wire-up sites (`run_detail.rs` for FCTX panel; `job_detail.rs` for histogram card).

```rust
tracing::warn!(
    target: "cronduit.web",
    job_id,
    run_id,        // <-- include only when in-scope; histogram drops this field
    error = %e,
    "<context-specific message>"
);
```

**Concrete shape per surface (research §E):**
- run_detail FCTX: `target: "cronduit.web", job_id = run.job_id, run_id = run.id, error = %e, "fctx panel: get_failure_context failed — hiding panel"`
- job_detail histogram: `target: "cronduit.web", job_id, error = %e, "exit histogram: query failed — degraded card"`

**Anti-pattern (research landmine §1):** `src/web/handlers/dashboard.rs:262-264` uses `.unwrap_or_default()` ALONE. Do NOT copy verbatim — Phase 21 D-12 explicitly upgrades the pattern by adding the warn.

---

### `Option<&str>` of RFC3339 string at queries.rs boundary

**Source:** `src/db/queries.rs::finalize_run` (lines 444-453) — the canonical widening pattern for adding optional time-related parameters.

**Apply to:** `insert_running_run` widening (Phase 21 P21 D-02).

Every timestamp param at the queries.rs boundary is `&str` or `Option<&str>` of an RFC3339 string. NEVER `chrono::DateTime<X>`. Conversion happens at the call boundary via `.to_rfc3339()` (e.g., `entry.fire_time.to_rfc3339()` in `scheduler/mod.rs`).

---

### Sibling-card outer chrome (matches Duration card on job_detail; metadata card on run_detail)

**Source:** `templates/pages/run_detail.html:33` and `templates/pages/job_detail.html:71`.

**Apply to:** both new template inserts (FCTX panel chrome + histogram card chrome).

```html
<div style="background:var(--cd-bg-surface);border:1px solid var(--cd-border);border-radius:8px;padding:var(--cd-space-6)" class="mb-6">
```

The FCTX panel uses this verbatim on its `<details>` parent (with `padding: 0` overridden because `<summary>` carries its own padding); the histogram card uses it verbatim on `.cd-exit-card`. UI-SPEC § Layout & Surfaces names this rule "non-negotiable for design coherence."

---

### Pre-formatted view-model (template carries no logic)

**Source:** `src/web/handlers/job_detail.rs::DurationView` (lines 99-109) — a flat struct of pre-formatted strings + booleans, populated by the handler before render.

**Apply to:** `FctxView` (run_detail) and `ExitHistogramView` (job_detail).

```rust
pub struct DurationView {
    pub p50_display: String,           // "1m 34s" or "—"
    pub p95_display: String,
    pub has_min_samples: bool,         // gates `title=...` attribute
    pub sample_count: usize,
    pub sample_count_display: String,  // pre-formatted subtitle
}
```

**Why it matters:** the askama templates hold ZERO logic beyond `{{ value }}` substitution + `{% if %}` / `{% match %}` gates. All conditional copy ("First failure: {ts} ago • {N} consecutive failures • [view last successful run]" vs "...• No prior successful run") is computed in Rust and stored as the same field on the view-model. UI-SPEC § Copywriting Contract is the source of truth for the strings; the handler picks the right one at render time.

---

### `#[cfg(test)] mod tests` colocated with module (sibling to `pub fn`)

**Source:** `src/web/stats.rs:25-83` — embedded test block with multiple `#[test] fn` inside the same `.rs` file.

**Apply to:** `src/web/exit_buckets.rs` unit tests for EXIT-02..EXIT-05 (per research § Validation Architecture: these run via `cargo test -p cronduit --lib exit_buckets::tests::...`).

The integration-test files in `tests/` cover render-level scenarios (EXIT-01 empty state visibility, FCTX-01 panel-render gating); the math/categorization tests live INSIDE `exit_buckets.rs` next to the code they test. Mirrors the `stats.rs:25-83` precedent exactly.

---

## No Analog Found

Files / behaviors with no close match in the existing codebase (planner should anchor to UI-SPEC.md / RESEARCH.md content here, since no Cronduit precedent exists):

| File / Behavior | Reason |
|------|--------|
| Soft-fail with `tracing::warn!` capture in an integration test | No existing test in `tests/` captures tracing output. Research § Validation Architecture mentions this assertion but no helper exists. Planner may need to add a small `tracing_subscriber::fmt::Layer` capture helper, OR substitute a behavioral assertion (panel section absent in body) for the warn-capture assertion. |
| `<details>` / `<summary>` markup in any template | None of the existing 5 page templates use native `<details>`. Phase 21 introduces the pattern. UI-SPEC § Component Inventory § 1 is the contract. |
| Pure-CSS bar chart with inline `style="height:{pct}%"` | None of the existing chart surfaces (sparkline, timeline) use this exact shape. Sparkline uses CSS-grid colored cells (no height variance); timeline uses `width:{pct}%`. UI-SPEC § Component Inventory § 2 is the contract; research §"Code Examples" notes "matches the v1.1 `.cd-sparkline` precedent (CSS grid + colored cells). No SVG, no canvas, no JS." |

## Metadata

**Analog search scope:** `src/web/`, `src/db/queries.rs`, `src/scheduler/`, `migrations/{sqlite,postgres}/`, `templates/pages/`, `assets/src/app.css`, `tests/v12_*.rs`, `tests/v13_*.rs`, `tests/job_detail_partial.rs`, `tests/dashboard_render.rs`, `justfile`.

**Files scanned:** 19 (verified via Read tool, non-overlapping ranges).

**Pattern extraction date:** 2026-05-01.

**Linked references:**
- `21-CONTEXT.md` § Decisions D-01..D-33 (33 locked items)
- `21-RESEARCH.md` § Codebase Map (exact `file:line` table for every reusable surface)
- `21-RESEARCH.md` § Discretion Resolutions §A-§H (8 decisions resolved)
- `21-RESEARCH.md` § Landmines (12 items — landmines §1, §2, §4, §11, §12 are most consequential for pattern selection)
- `21-UI-SPEC.md` § Component Inventory + § Color + § Copywriting Contract (CSS class contracts + locked copy)
