# Phase 13: Observability Polish (rc.2) - Pattern Map

**Mapped:** 2026-04-21
**Files analyzed:** 18 (new + modified)
**Analogs found:** 17 / 18 (1 file has no close analog — `tests/v13_timeline_explain.rs` EXPLAIN-plan assertion is novel for this codebase)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/web/stats.rs` (NEW) | utility | pure-compute / transform | `src/web/format.rs` | exact |
| `src/web/handlers/timeline.rs` (NEW) | handler | request-response (GET, read-only) | `src/web/handlers/dashboard.rs` | exact |
| `templates/pages/timeline.html` (NEW) | template (page) | server-render + HTMX poll | `templates/pages/dashboard.html` | exact |
| `templates/partials/timeline_body.html` (NEW) | template (partial) | HTMX swap target | `templates/partials/job_table.html` | exact |
| `tests/v13_stats_percentile.rs` (NEW — OR inline `#[cfg(test)]`) | test (unit) | pure-compute verification | `src/web/format.rs::tests` | exact |
| `tests/v13_duration_card.rs` (NEW) | test (integration) | render-scan | `tests/job_detail_partial.rs` | exact |
| `tests/v13_sparkline_render.rs` (NEW) | test (integration) | render-scan | `tests/dashboard_render.rs` | exact |
| `tests/v13_timeline_render.rs` (NEW) | test (integration) | render-scan | `tests/dashboard_render.rs` | role-match |
| `tests/v13_timeline_explain.rs` (NEW) | test (integration, dual-backend) | EXPLAIN-plan scan | `tests/schema_parity.rs` + `tests/db_pool_postgres.rs` | partial (pattern novel for codebase) |
| `tests/v13_timeline_timezone.rs` (NEW) | test (integration) | render-scan with tz config | `tests/dashboard_render.rs` | role-match |
| `src/db/queries.rs` (MOD) — add `get_dashboard_job_sparks`, `get_recent_successful_durations`, `get_timeline_runs` | query (SELECT) | CRUD read, dual-path | existing `get_dashboard_jobs` (lines 528-653), `get_run_history` (lines 706-782) | exact |
| `src/web/handlers/dashboard.rs` (MOD) — extend `DashboardJobView` + `to_view()` | handler | request-response | (self — extend in place) | exact |
| `src/web/handlers/job_detail.rs` (MOD) — add `DurationView` | handler | request-response | (self — extend in place) | exact |
| `templates/pages/dashboard.html` (MOD) — add `<th>Recent</th>` | template (page) | static markup | existing `<th>Actions</th>` at line 86 | exact |
| `templates/partials/job_table.html` (MOD) — add `<td>` Recent cell | template (partial) | static markup | existing `<td>` at line 10-14 | exact |
| `templates/pages/job_detail.html` (MOD) — insert Duration card | template (page) | static markup | existing Configuration card block (lines 23-68) | exact |
| `templates/base.html` (MOD) — add Timeline nav link | template (layout) | static markup | existing Dashboard/Settings nav block (lines 30-37) | exact |
| `src/web/mod.rs` (MOD) — add `/timeline` route + `pub mod stats;` | config (router) | declarative | existing router builder (lines 47-87) | exact |
| `src/web/handlers/mod.rs` (MOD) — add `pub mod timeline;` | config (module) | declarative | existing `pub mod dashboard;` (line 2) | exact |
| `assets/src/app.css` (MOD) — add `cd-sparkline-*`, `cd-timeline-*`, `cd-pill-*`, `cd-tooltip-*`, `@keyframes cd-pulse`, `--cd-status-cancelled{-bg}`, `--cd-timeline-*` | config (styles) | declarative CSS | existing `@layer components` block + `cd-badge--{status}` family (lines 170-189) | exact |
| `.github/workflows/ci.yml` (MOD, possible) — grep guard for `percentile_cont` | config (CI) | policy gate | existing `- run: just openssl-check` line (per-job shell step pattern) | role-match |
| `docs/release-rc.md` (NO EDITS — reused verbatim per D-22) | doc | n/a | n/a | n/a |
| `.github/workflows/release.yml` (NO EDITS — reused verbatim per D-22) | ci | n/a | n/a | n/a |
| `Cargo.toml` (NO EDITS — already at `1.1.0` per research A7) | config | n/a | n/a | n/a |
| `.planning/REQUIREMENTS.md` (MOD, close-out) — flip OBS-01..OBS-05 checkboxes | doc | text edit | established phase close-out convention | role-match |

---

## Pattern Assignments

### `src/web/stats.rs` (utility, pure-compute)

**Analog:** `src/web/format.rs`

**Module-level structure pattern** (whole file):
```rust
// Source: src/web/format.rs:1-34 (verified)
//! Shared formatting helpers for web view models.

/// Format duration in milliseconds to human-readable string.
pub fn format_duration_ms(ms: Option<i64>) -> String {
    match ms {
        Some(ms) if ms < 1000 => format!("{ms}ms"),
        // ... more arms ...
        None => "-".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_ms() {
        assert_eq!(format_duration_ms(Some(500)), "500ms");
        // ... more assertions ...
    }
}
```

Phase 13 `stats.rs` follows the EXACT shape: a module-level doc comment, one `pub fn`, one inline `#[cfg(test)] mod tests` with several `#[test] fn test_*` cases. Research § OBS-04 provides verbatim implementation; planner should copy that implementation. Open Question #3 resolved to inline tests (no external `tests/v13_stats_percentile.rs`).

**Module wiring pattern** — how `format` is registered in `src/web/mod.rs`:
```rust
// Source: src/web/mod.rs:1-5 (verified)
pub mod ansi;
pub mod assets;
pub mod csrf;
pub mod format;
pub mod handlers;
```

Add `pub mod stats;` alphabetically between `format` and `handlers`.

---

### `src/web/handlers/timeline.rs` (handler, request-response)

**Analog:** `src/web/handlers/dashboard.rs`

**Imports pattern** (lines 1-15):
```rust
// Source: src/web/handlers/dashboard.rs:1-15 (verified)
//! Dashboard page and HTMX job-table partial (UI-06, UI-07, UI-13).

use askama::Template;
use askama_web::WebTemplateExt;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum_htmx::HxRequest;
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::Deserialize;
use std::str::FromStr;

use crate::db::queries::{self, DashboardJob};
use crate::web::AppState;
use crate::web::csrf;
```

Copy verbatim for `timeline.rs`; drop `csrf` (timeline is read-only, no form submission) and drop `FromStr`/`DashboardJob` imports; add `use crate::db::queries::TimelineRun;` (new struct from queries.rs).

**Query params pattern** (lines 21-36):
```rust
// Source: src/web/handlers/dashboard.rs:21-36 (verified)
#[derive(Debug, Deserialize, Default)]
pub struct DashboardParams {
    #[serde(default)]
    pub filter: String,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default = "default_order")]
    pub order: String,
}

fn default_sort() -> String { "name".to_string() }
fn default_order() -> String { "asc".to_string() }
```

Phase 13 `TimelineParams`:
```rust
#[derive(Debug, Deserialize, Default)]
pub struct TimelineParams {
    #[serde(default)]
    pub window: Option<String>, // "24h" | "7d"; None defaults to "24h"
}
```

**Template struct + handler pattern** (lines 42-58 and 180-219):
```rust
// Source: src/web/handlers/dashboard.rs:42-58 (verified)
#[derive(Template)]
#[template(path = "pages/dashboard.html")]
struct DashboardPage {
    jobs: Vec<DashboardJobView>,
    filter: String,
    sort: String,
    order: String,
    config_path: String,
    csrf_token: String,
}
```

```rust
// Source: src/web/handlers/dashboard.rs:180-219 (verified)
pub async fn dashboard(
    HxRequest(is_htmx): HxRequest,
    State(state): State<AppState>,
    Query(params): Query<DashboardParams>,
    cookies: axum_extra::extract::CookieJar,
) -> impl IntoResponse {
    let filter = if params.filter.is_empty() { None } else { Some(params.filter.as_str()) };
    let jobs = queries::get_dashboard_jobs(&state.pool, filter, &params.sort, &params.order)
        .await
        .unwrap_or_default();

    let tz: Tz = state.tz;
    let job_views: Vec<DashboardJobView> = jobs.into_iter().map(|j| to_view(j, tz)).collect();

    let csrf_token = csrf::get_token_from_cookies(&cookies);

    if is_htmx {
        JobTablePartial { jobs: job_views, csrf_token }
            .into_web_template()
            .into_response()
    } else {
        DashboardPage { jobs: job_views, /* ... */ csrf_token }
            .into_web_template()
            .into_response()
    }
}
```

Phase 13 timeline handler follows the same `HxRequest` / `State` / `Query` extractor triple + `unwrap_or_default()` error fallback + `.into_web_template().into_response()` response-building idiom. Research § OBS-01/02 provides verbatim handler body.

**Timezone threading** (line 195):
```rust
// Source: src/web/handlers/dashboard.rs:195 (verified)
let tz: Tz = state.tz;
```
Timeline handler does the same and uses `dt.with_timezone(&tz).format(...)` for every axis tick and tooltip time.

**Error fallback** (lines 191-193):
```rust
// Source: src/web/handlers/dashboard.rs:191-193 (verified)
let jobs = queries::get_dashboard_jobs(&state.pool, filter, &params.sort, &params.order)
    .await
    .unwrap_or_default();
```
`unwrap_or_default()` is the project's error-swallow pattern for read-only GET handlers; never propagate `anyhow::Error` into a 500 on the dashboard/timeline — silent fallback to empty is the correct homelab UX (per Risks #4 D-06 zero-run crash-free invariant).

---

### `src/db/queries.rs` (query, CRUD read dual-path)

**Analog:** `src/db/queries.rs::get_dashboard_jobs` (lines 528-653) — closest match for window-function + dual-path + enabled-filter. Also `get_run_history` (lines 706-782) for the simpler "limit + offset" shape (used by `get_recent_successful_durations`).

**Dual-path `PoolRef` match pattern** (lines 580-653):
```rust
// Source: src/db/queries.rs:580-653 (verified — structure shown abbreviated)
match pool.reader() {
    PoolRef::Sqlite(p) => {
        let rows = if has_filter {
            let pattern = format!("%{}%", filter.unwrap().to_lowercase());
            sqlx::query(&base_sql).bind(pattern).fetch_all(p).await?
        } else {
            sqlx::query(&base_sql).fetch_all(p).await?
        };
        Ok(rows.into_iter().map(|r| DashboardJob { id: r.get("id"), /* ... */ }).collect())
    }
    PoolRef::Postgres(p) => {
        let pg_sql = /* $1 placeholders + j.enabled = true */;
        let rows = sqlx::query(&pg_sql).bind(pattern).fetch_all(p).await?;
        Ok(rows.into_iter().map(|r| DashboardJob { /* ... */ }).collect())
    }
}
```

Phase 13's three new queries (`get_dashboard_job_sparks`, `get_recent_successful_durations`, `get_timeline_runs`) all clone this shape. Key invariants:
- SQLite arm uses `?1`/`?2`/`?3` placeholders and `j.enabled = 1`.
- Postgres arm uses `$1`/`$2`/`$3` placeholders and `j.enabled = true`.
- Status literals are lowercase (`'success'`, `'failed'`, `'timeout'`, `'cancelled'`, `'stopped'`, `'running'`) — **identical on both arms** (see Risks #1: uppercase drift is a silent bug).
- Row mapping via `.into_iter().map(|r| Struct { field: r.get("col"), ... }).collect()`.

**Window-function (`ROW_NUMBER OVER PARTITION BY`) pattern** (lines 557-575):
```rust
// Source: src/db/queries.rs:557-575 (verified)
LEFT JOIN (
    SELECT job_id, status, start_time, trigger,
           ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY start_time DESC) AS rn
    FROM job_runs
) lr ON lr.job_id = j.id AND lr.rn = 1
```

Sparkline query clones this with `rn <= 20` (not `rn = 1`) and filters on `status IN (...)`. Research § OBS-03 provides verbatim SQL.

**Simple `LIMIT` query pattern** (lines 720-727):
```rust
// Source: src/db/queries.rs:720-727 (verified)
let rows = sqlx::query(
    "SELECT id, job_id, job_run_number, status, trigger, start_time, end_time, duration_ms, exit_code, error_message FROM job_runs WHERE job_id = ?1 ORDER BY start_time DESC LIMIT ?2 OFFSET ?3",
)
.bind(job_id)
.bind(limit)
.bind(offset)
.fetch_all(p)
.await?;
```

`get_recent_successful_durations` clones this shape, dropping `OFFSET` and adding `AND status = 'success' AND duration_ms IS NOT NULL` to the WHERE clause. Research § OBS-04 provides verbatim SQL.

**Column-fetch pattern** (lines 588-601):
```rust
// Source: src/db/queries.rs:588-601 (verified)
Ok(rows
    .into_iter()
    .map(|r| DashboardJob {
        id: r.get("id"),
        name: r.get("name"),
        // ... named column fetches ...
    })
    .collect())
```

For raw `Vec<u64>` return (percentile input): `rows.into_iter().map(|r| r.get::<i64, _>("duration_ms") as u64).collect()` per Research § OBS-04.

---

### `src/web/handlers/dashboard.rs` (MOD — extend view-model)

**Analog:** self (extend in place)

**View-model struct extension pattern** (lines 64-74):
```rust
// Source: src/web/handlers/dashboard.rs:64-74 (verified)
pub struct DashboardJobView {
    pub id: i64,
    pub name: String,
    pub schedule: String,
    pub resolved_schedule: String,
    pub has_random_schedule: bool,
    pub next_fire: String,
    pub last_status: String,
    pub last_status_label: String,
    pub last_run_relative: String,
}
```

Phase 13 appends new fields: `spark_cells: Vec<SparkCell>`, `spark_badge: String`, `spark_total: usize`, `spark_numerator: usize`, `spark_denominator: usize`. Additive — no existing fields touched.

**`to_view` hydration pattern** (lines 76-132):
```rust
// Source: src/web/handlers/dashboard.rs:76-132 (verified — structure shown abbreviated)
fn to_view(job: DashboardJob, tz: Tz) -> DashboardJobView {
    let now = Utc::now();
    let now_tz = now.with_timezone(&tz);

    let next_fire = match croner::Cron::from_str(&job.resolved_schedule) { /* ... */ };

    let last_status = job.last_status.as_deref().unwrap_or("").to_lowercase();
    let last_status_label = if last_status.is_empty() { String::new() } else { last_status.to_uppercase() };

    let last_run_relative = match &job.last_run_time {
        Some(ts) => { /* parse RFC3339, then NaiveDateTime fallback */ }
        None => "never".to_string(),
    };

    DashboardJobView { /* ... all fields ... */ }
}
```

The handler's extension: after `let job_views: Vec<DashboardJobView> = jobs.into_iter().map(|j| to_view(j, tz)).collect();`, fetch `spark_rows`, bucket by job_id into a `HashMap<i64, Vec<DashboardSparkRow>>`, then iterate `&mut job_views` to populate the 5 new fields. Research § OBS-03 provides verbatim handler code.

**RFC3339 + naive-datetime fallback parse idiom** (lines 102-113):
```rust
// Source: src/web/handlers/dashboard.rs:102-113 (verified)
match DateTime::parse_from_rfc3339(ts) {
    Ok(dt) => format_relative_past(dt.with_timezone(&Utc), now),
    Err(_) => {
        match chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S") {
            Ok(ndt) => {
                let dt = ndt.and_utc();
                format_relative_past(dt, now)
            }
            Err(_) => ts.clone(),
        }
    }
}
```

**Reuse verbatim** for sparkline cell `relative_time` tooltip and timeline bar `start_time`/`end_time` rendering. Both SQLite text-storage formats are handled. Research § "Don't Hand-Roll" calls this out explicitly.

**`format_relative_past` helper** (lines 142-149):
```rust
// Source: src/web/handlers/dashboard.rs:142-149 (verified)
fn format_relative_past(target: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let diff = now.signed_duration_since(target);
    let total_secs = diff.num_seconds().max(0);
    if total_secs == 0 { return "just now".to_string(); }
    format_duration_relative(total_secs, "") + " ago"
}
```

Sparkline cell tooltip uses this helper for the "2h ago" component of `title="#42 SUCCESS 1m 34s 2h ago"`. Either promote to `pub` and cross-import from `timeline.rs`, or duplicate (3 lines) — planner decides. Research A2 notes: current visibility is module-private; consider making `pub(super)` via `handlers/mod.rs`.

---

### `src/web/handlers/job_detail.rs` (MOD — add Duration section)

**Analog:** self (extend in place) + existing `RunHistoryView` hydration for `duration_display`

**`format_duration_ms` reuse** (imports line 15 + lines 178):
```rust
// Source: src/web/handlers/job_detail.rs:15 (verified)
use crate::web::format::format_duration_ms;

// Source: src/web/handlers/job_detail.rs:178 (verified)
duration_display: format_duration_ms(r.duration_ms),
```

Phase 13 Duration card reuses `format_duration_ms` for `p50_display` / `p95_display`. **Caveat (Research A3 / Open Question #2 / Risks #12):** current formatter emits `"1.2s"` for sub-minute durations. UI-SPEC copywriting says `"42s"`. Planner resolves via one of:
- Add `format_duration_ms_floor_seconds` in `src/web/format.rs` (recommended — no regression to shipped Run History display).
- Mutate shared formatter (simpler but changes Run History column across all pages).

Use the Phase-13-only variant to preserve backward-compat. Inline test in `format.rs::tests` block (same pattern as existing `test_format_duration_ms`).

**View-model extension pattern** (lines 72-82):
```rust
// Source: src/web/handlers/job_detail.rs:72-82 (verified)
pub struct JobDetailView {
    pub id: i64,
    pub name: String,
    pub schedule: String,
    pub resolved_schedule: String,
    pub has_random_schedule: bool,
    pub job_type: String,
    pub config_json: String,
    pub cron_description: String,
    pub timeout_display: String,
}
```

Phase 13 appends `pub duration: DurationView,` — a new nested struct (fields: `p50_display`, `p95_display`, `has_min_samples`, `sample_count`, `sample_count_display`). Research § OBS-04 (cont.) provides verbatim struct + subtitle-matrix match expression.

**Full-page render branch pattern** (lines 199-233):
```rust
// Source: src/web/handlers/job_detail.rs:199-233 (verified)
// Compute cron description using croner
let cron_description = croner::Cron::from_str(&job.resolved_schedule)
    .map(|c| c.describe())
    .unwrap_or_else(|_| "Invalid schedule".to_string());

let has_random_schedule = job.schedule.split_whitespace().any(|f| f == "@random");

let job_view = JobDetailView {
    id: job.id,
    name: job.name,
    // ...
    timeout_display: format_timeout(job.timeout_secs),
};

JobDetailPage { job: job_view, /* ... */ csrf_token }
    .into_web_template()
    .into_response()
```

Add the duration hydration block (fetch durations → call `percentile()` → format → build `DurationView`) BEFORE the `JobDetailView { ... }` literal, then include `duration: duration_view` in the struct literal. Research § OBS-04 (cont.) provides verbatim code.

---

### `templates/pages/job_detail.html` (MOD — insert Duration card)

**Analog:** self — Configuration card outer shape at lines 22-68

**Card outer-shape pattern** (line 23):
```html
<!-- Source: templates/pages/job_detail.html:23-24 (verified) -->
<div style="background:var(--cd-bg-surface);border:1px solid var(--cd-border);border-radius:8px;padding:var(--cd-space-6)" class="mb-6">
  <h2 style="font-size:var(--cd-text-lg);font-weight:700;margin-bottom:var(--cd-space-4)">Configuration</h2>
  <!-- ... -->
</div>
```

Duration card copies this envelope EXACTLY — same `background:var(--cd-bg-surface);border:1px solid var(--cd-border);border-radius:8px;padding:var(--cd-space-6)` + `class="mb-6"`.

**CRITICAL DIVERGENCE — UI-SPEC locks `--cd-text-xl` for the Duration heading, NOT `--cd-text-lg`.** UI-SPEC collapsed typography from 5 sizes to 4 per checker Dimension 4; `--cd-text-lg` is not used by Phase 13 (see UI-SPEC § Typography). The existing Configuration card heading at line 24 stays at `--cd-text-lg` (additive-only — do NOT edit it). The new Duration card heading renders at `--cd-text-xl`. Visual separation inside Duration card is carried by weight + chip labels + flex layout, not a size jump.

**Insertion point:** between the Configuration card closing `</div>` at line 68 and the Run History opening `<div class="mb-6">` at line 71.

**Inner-chip label pattern** (line 28):
```html
<!-- Source: templates/pages/job_detail.html:28-29 (verified) -->
<span style="font-size:var(--cd-text-xs);color:var(--cd-text-secondary);text-transform:uppercase;letter-spacing:0.1em">Type</span>
<div style="font-size:var(--cd-text-base);color:var(--cd-text-primary);margin-top:2px">{{ job.job_type }}</div>
```

Duration-card `p50` / `p95` label + value follows this "uppercase xs label + primary value below with 2px margin-top" pattern. UI-SPEC § "Surface B" provides verbatim HTML.

---

### `templates/partials/job_table.html` (MOD — add Recent cell)

**Analog:** self — existing status-badge `<td>` at lines 10-14

**Status-badge cell pattern** (lines 10-14):
```html
<!-- Source: templates/partials/job_table.html:10-14 (verified) -->
<td class="py-2 px-4">
  {% if !job.last_status.is_empty() %}
  <span class="cd-badge cd-badge--{{ job.last_status }}">{{ job.last_status_label }}</span>
  {% endif %}
</td>
```

**Key reuse pattern:** `cd-badge--{{ job.last_status }}` — **class-name derived from lowercase status string**. Phase 13 sparkline cells follow the exact same convention: `cd-sparkline-cell--{{ cell.kind }}`. Research § "Code Patterns to Follow" row "cd-badge--{status} class derivation" locks this as the phase pattern.

**Insertion point:** between line 15 (`Last Run`) and line 16 (`Actions`). UI-SPEC § "Surface A" provides verbatim HTML.

---

### `templates/pages/dashboard.html` (MOD — add `<th>Recent</th>`)

**Analog:** self — existing non-sortable `<th>Actions</th>` at line 86

**Non-sortable column-header pattern** (line 86):
```html
<!-- Source: templates/pages/dashboard.html:86 (verified) -->
<th class="text-left py-2 px-4" style="font-size:var(--cd-text-xs);font-weight:700;text-transform:uppercase;letter-spacing:0.1em;color:var(--cd-text-secondary)">Actions</th>
```

Recent column header clones this shape (no inner `<a hx-get ...>` wrapper — not sortable). UI-SPEC § "Surface A" locks `min-width:180px` addition for sparkline-cell sizing.

**Insertion point:** between the Last Run `<th>` closing at line 84 and the Actions `<th>` opening at line 86.

**HTMX poll wrapper pattern** (lines 89-95):
```html
<!-- Source: templates/pages/dashboard.html:89-95 (verified) -->
<tbody id="job-table-body"
       hx-get="/partials/job-table"
       hx-trigger="every 3s"
       hx-swap="innerHTML"
       hx-include="[name='filter'],[name='sort'],[name='order']">
  {% include "partials/job_table.html" %}
</tbody>
```

**Timeline mirrors this shape** at 30s cadence with a single `[name='window']` include. The pill toggle + hidden `<input>` must live OUTSIDE the swap target (UI-SPEC § "Surface C") so the `outerHTML` swap doesn't destroy state. No changes to dashboard's existing 3s poll.

---

### `templates/pages/timeline.html` (NEW — page skeleton)

**Analog:** `templates/pages/dashboard.html`

**Template-inheritance pattern** (lines 1-7):
```html
<!-- Source: templates/pages/dashboard.html:1-7 (verified) -->
{% extends "base.html" %}
{% block title %}Dashboard - Cronduit{% endblock %}
{% block nav_dashboard_active %}border-b-2 border-(--cd-text-accent) text-(--cd-text-primary){% endblock %}
{% block content %}
<div class="flex items-center justify-between mb-6">
  <h1 style="font-size:var(--cd-text-xl);font-weight:700;letter-spacing:-0.02em">Dashboard</h1>
</div>
```

Phase 13 timeline page clones this exactly with `{% block title %}Timeline - Cronduit{% endblock %}` and `{% block nav_timeline_active %}border-b-2 border-(--cd-text-accent) text-(--cd-text-primary){% endblock %}` (new block — requires `base.html` edit). `<h1>Timeline</h1>` at `--cd-text-xl` weight 700 letter-spacing `-0.02em`.

**Empty-state inline-block pattern** (lines 9-16):
```html
<!-- Source: templates/pages/dashboard.html:9-16 (verified) -->
{% if jobs.is_empty() %}
<div class="flex flex-col items-center justify-center py-16 text-center">
  <h2 style="font-size:var(--cd-text-lg);font-weight:700;color:var(--cd-text-secondary)">No jobs configured</h2>
  <p style="font-size:var(--cd-text-base);color:var(--cd-text-secondary)" class="mt-2">
    Add jobs to your config file at <code class="text-(--cd-text-accent)">{{ config_path }}</code> and restart Cronduit.
  </p>
</div>
{% else %}
```

Timeline uses `.cd-timeline-empty` instead (UI-SPEC-locked selector). Different visual (axis stays visible per D-14) but same `{% if ... %}{% else %}...{% endif %}` branching shape.

UI-SPEC § "Surface C: Timeline page" provides the full verbatim template.

---

### `templates/partials/timeline_body.html` (NEW — HTMX swap target)

**Analog:** `templates/partials/job_table.html`

**Partial-shape pattern** (whole file):
```html
<!-- Source: templates/partials/job_table.html (verified) -->
{% for job in jobs %}
<tr class="hover:bg-(--cd-bg-hover) border-b border-(--cd-border-subtle)">
  <td class="py-2 px-4">
    <a href="/jobs/{{ job.id }}" class="text-(--cd-text-accent) hover:underline font-bold">{{ job.name }}</a>
  </td>
  <!-- ... cells ... -->
</tr>
{% endfor %}
```

Partials contain NO `{% extends %}` — they are `{% include %}`-ed from their page. Phase 13 `timeline_body.html` is structurally similar (loop over jobs, nested per-bar loop) but uses `<div class="cd-timeline-row">` containers instead of `<tr>`. UI-SPEC § "Surface C" provides verbatim HTML.

**Job-name link pattern** (line 4):
```html
<!-- Source: templates/partials/job_table.html:4 (verified) -->
<a href="/jobs/{{ job.id }}" class="text-(--cd-text-accent) hover:underline font-bold">{{ job.name }}</a>
```

Timeline row-label `<a>` uses `.cd-timeline-row-label` class (new) with identical `href="/jobs/{{ job.id }}"` pattern.

---

### `templates/base.html` (MOD — add Timeline nav link)

**Analog:** self — existing Dashboard/Settings nav links at lines 30-37

**Nav-link pattern** (lines 30-37):
```html
<!-- Source: templates/base.html:30-37 (verified) -->
<a href="/"
   class="text-(--cd-text-secondary) hover:text-(--cd-text-primary) no-underline text-sm py-1 {% block nav_dashboard_active %}{% endblock %}">
  Dashboard
</a>
<a href="/settings"
   class="text-(--cd-text-secondary) hover:text-(--cd-text-primary) no-underline text-sm py-1 {% block nav_settings_active %}{% endblock %}">
  Settings
</a>
```

**Insertion point:** between the Dashboard `<a>` closing `</a>` at line 33 and the Settings `<a>` opening at line 34. New block name: `{% block nav_timeline_active %}{% endblock %}`. UI-SPEC § "Surface C" Template insertion locks the exact HTML.

---

### `src/web/mod.rs` (MOD — register route + module)

**Analog:** self — existing `pub mod` declarations (lines 1-5) + router builder (lines 47-87)

**Module declaration pattern** (lines 1-5):
```rust
// Source: src/web/mod.rs:1-5 (verified)
pub mod ansi;
pub mod assets;
pub mod csrf;
pub mod format;
pub mod handlers;
```

Add `pub mod stats;` — alphabetically fits between `format` and `handlers`.

**Route-registration pattern** (lines 47-84):
```rust
// Source: src/web/mod.rs:47-84 (verified — abbreviated)
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(handlers::dashboard::dashboard))
        .route("/partials/job-table", get(handlers::dashboard::dashboard))
        .route("/jobs/{id}", get(handlers::job_detail::job_detail))
        // ... many more routes ...
        .route("/settings", get(handlers::settings::settings))
        .route("/health", get(handlers::health::health))
        // ...
        .with_state(state)
        .layer(middleware::from_fn(csrf::ensure_csrf_cookie))
        .layer(TraceLayer::new_for_http())
}
```

Insert `.route("/timeline", get(handlers::timeline::timeline))` — placement near the top alongside the other page routes (after `/partials/run-history/{id}` or before `/settings` reads cleanly). Single-line addition. No partial route needed per UI-SPEC (the `outerHTML` swap re-enters the same `/timeline` handler).

---

### `src/web/handlers/mod.rs` (MOD — expose module)

**Analog:** self (lines 1-8)

**Handler-module pattern** (whole file):
```rust
// Source: src/web/handlers/mod.rs:1-8 (verified)
pub mod api;
pub mod dashboard;
pub mod health;
pub mod job_detail;
pub mod metrics;
pub mod run_detail;
pub mod settings;
pub mod sse;
```

Add `pub mod timeline;` — alphabetically between `sse` and (nothing). New line: `pub mod timeline;`.

---

### `assets/src/app.css` (MOD — add sparkline + timeline + pill + tooltip selectors)

**Analog:** existing `@layer components` block (lines 170-259+) + `cd-badge--{status}` family (lines 181-189)

**`@layer components` scoping pattern** (line 170):
```css
/* Source: assets/src/app.css:170 (verified) */
@layer components {
  .cd-badge {
    font-size: var(--cd-text-xs);
    font-weight: 700;
    /* ... */
  }
  /* ... */
}
```

All new Phase 13 selectors (`cd-sparkline-*`, `cd-timeline-*`, `cd-pill-*`, `cd-tooltip-*`, `@keyframes cd-pulse`) go INSIDE the `@layer components { }` block. UI-SPEC § "Surface A/B/C" provides verbatim CSS.

**Status-variant-per-class pattern** (lines 181-189):
```css
/* Source: assets/src/app.css:181-189 (verified) */
.cd-badge--success { color: var(--cd-status-active); background: var(--cd-status-active-bg); }
.cd-badge--failed { color: var(--cd-status-error); background: var(--cd-status-error-bg); }
.cd-badge--running { color: var(--cd-status-running); background: var(--cd-status-running-bg); }
.cd-badge--timeout { color: var(--cd-status-disabled); background: var(--cd-status-disabled-bg); }
.cd-badge--stopped { color: var(--cd-status-stopped); background: var(--cd-status-stopped-bg); }
```

Sparkline cells + timeline bars clone this `--{status}` variant family exactly:
```css
.cd-sparkline-cell--success   { background: var(--cd-status-active); }
.cd-sparkline-cell--failed    { background: var(--cd-status-error); }
/* ... etc. for six statuses ... */

.cd-timeline-bar--success   { background: var(--cd-status-active); }
.cd-timeline-bar--failed    { background: var(--cd-status-error); }
/* ... etc. for six statuses including --running for the pulsing bar ... */
```

**Focus-ring pattern** (lines 252-255, from `cd-btn-stop`):
```css
/* Source: assets/src/app.css:252-255 (verified) */
.cd-btn-stop:focus-visible {
    outline: none;
    border-color: var(--cd-border-focus);
    box-shadow: 0 0 0 2px var(--cd-green-dim);
}
```

Every new interactive element in Phase 13 (`.cd-pill`, `.cd-timeline-bar`) clones this focus-ring pattern exactly for keyboard-navigation consistency.

**Token-addition pattern:** UI-SPEC § "New --cd-timeline-* token additions" + "New --cd-status-cancelled color tokens" locks the 7 new CSS custom properties — 5 layout-scalar (identical values across themes) + 2 color tokens (dark + light + auto-detect). Planner adds to `:root` (dark defaults around lines 25-81), `[data-theme="light"]` (around lines 84-114), and `@media (prefers-color-scheme: light)` (around lines 117-145) per Risks #8.

---

### `tests/v13_stats_percentile.rs` OR inline `src/web/stats.rs::tests` (test, unit)

**Analog:** `src/web/format.rs` lines 22-34

**Inline-test pattern** (lines 22-34):
```rust
// Source: src/web/format.rs:22-34 (verified)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_ms() {
        assert_eq!(format_duration_ms(Some(500)), "500ms");
        assert_eq!(format_duration_ms(Some(1200)), "1.2s");
        assert_eq!(format_duration_ms(Some(135_000)), "2m 15s");
        assert_eq!(format_duration_ms(Some(7_260_000)), "2h 1m");
        assert_eq!(format_duration_ms(None), "-");
    }
}
```

Phase 13 `percentile` helper tests live inline per Research Open Question #3 resolution. Research § OBS-04 provides verbatim test vectors (9 `#[test] fn`s covering T-V11-DUR-01..04 plus boundary cases).

---

### `tests/v13_duration_card.rs` (test, integration)

**Analog:** `tests/job_detail_partial.rs`

**Imports + test-app builder pattern** (lines 14-69):
```rust
// Source: tests/job_detail_partial.rs:14-69 (verified — abbreviated)
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use axum::routing::get;
use tower::ServiceExt;

use cronduit::db::DbPool;
use cronduit::db::{finalize_run, insert_running_run, upsert_job};
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::telemetry::setup_metrics;
use cronduit::web::handlers::job_detail::job_runs_partial;
use cronduit::web::{AppState, ReloadState};

async fn build_test_app() -> (Router, DbPool) {
    let pool = DbPool::connect("sqlite::memory:").await.expect("in-memory sqlite pool");
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

    let router = Router::new()
        .route("/partials/jobs/{job_id}/runs", get(job_runs_partial))
        .with_state(state);

    (router, pool)
}
```

**Full AppState construction idiom is load-bearing** — every integration test in the codebase reproduces this 10-field struct literal. Phase 13 tests clone verbatim (only changing the route registered on the test router + any seeded fixtures).

**Seed-helper pattern** (lines 72-96):
```rust
// Source: tests/job_detail_partial.rs:72-96 (verified)
async fn seed_job(pool: &DbPool, name: &str) -> i64 {
    upsert_job(pool, name, "*/5 * * * *", "*/5 * * * *", "command",
               r#"{"command":"echo hello"}"#, &format!("hash-{name}"), 3600)
        .await.expect("upsert test job")
}

async fn seed_success_run(pool: &DbPool, job_id: i64) -> i64 {
    let run_id = insert_running_run(pool, job_id, "scheduled").await.expect("insert running run");
    let start = tokio::time::Instant::now();
    finalize_run(pool, run_id, "success", Some(0), start, None, None).await.expect("finalize run as success");
    run_id
}
```

**Reuse verbatim** for Duration-card tests — seed 0 / 1 / 19 / 20 / 42 / 100 / 150 success runs and assert rendered HTML matches the UI-SPEC Copywriting contract's subtitle matrix. Alternative: import `tests/common/v11_fixtures.rs::setup_sqlite_with_phase11_migrations` + `seed_test_job` instead of re-defining the helpers.

---

### `tests/v13_sparkline_render.rs` (test, integration)

**Analog:** `tests/dashboard_render.rs`

**Render-scan pattern** (lines 62-80):
```rust
// Source: tests/dashboard_render.rs:62-80 (verified — abbreviated)
#[tokio::test]
async fn dashboard_renders_all_jobs_with_six_required_fields() {
    let (app, pool) = build_test_app().await;

    let alpha_id = seed_job(&pool, "alpha-backup", "*/10 * * * *").await;
    let beta_id = seed_job(&pool, "beta-sync", "0 */2 * * *").await;

    let start = tokio::time::Instant::now();
    let alpha_run = queries::insert_running_run(&pool, alpha_id, "manual").await.expect("insert alpha running run");
    queries::finalize_run(&pool, alpha_run, "success", Some(0), start, None, None)
        .await.expect("finalize alpha run");

    // ... hit GET / via app.oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
    // ... scan response body with .contains("alpha-backup") etc.
}
```

Sparkline tests follow the same shape: build test app, seed jobs + terminal runs (5/15/20/zero), GET `/partials/job-table` (or GET `/`), scan HTML for exactly 20 `<span class="cd-sparkline-cell` matches per job row and the badge text (`"95%"` / `"—"` / `"94%"` for stopped-denominator test). Assertions per UI-SPEC Copywriting § "Sparkline column".

---

### `tests/v13_timeline_render.rs` / `tests/v13_timeline_timezone.rs` (test, integration)

**Analog:** `tests/dashboard_render.rs` (same test-app pattern)

Seed jobs + terminal and running runs across a 24h window, GET `/timeline?window=24h`, scan HTML for one `<div class="cd-timeline-row">` per enabled job in alphabetical order, 6 status-class variants, `cd-timeline-bar--pulsing` class only on running-status bars, empty-window message text per UI-SPEC Copywriting.

Timezone test overrides `tz: chrono_tz::UTC` to `chrono_tz::Tz::America_Los_Angeles` in the AppState construction and asserts the UTC-10:00Z run renders as "03:00" in tick labels + tooltip (per Risks #7 DST pitfall).

---

### `tests/v13_timeline_explain.rs` (test, integration — dual-backend)

**Analogs:** `tests/schema_parity.rs` (Postgres testcontainer + dual-backend shape) + `tests/db_pool_postgres.rs` (minimal Postgres start-up)

**Postgres testcontainer startup pattern** (lines 8-19 of `db_pool_postgres.rs`):
```rust
// Source: tests/db_pool_postgres.rs:8-19 (verified)
#[tokio::test]
async fn db_pool_connects_and_migrates_against_postgres() {
    let container = Postgres::default().start().await.expect("start postgres");
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");

    let pool = DbPool::connect(&url).await.expect("DbPool::connect");
    assert_eq!(pool.backend(), DbBackend::Postgres);
    pool.migrate().await.expect("first migrate");
    pool.migrate().await.expect("second migrate (idempotent)");
    pool.close().await;
}
```

**Reuse verbatim** for the Postgres half of the timeline-explain test. Then run `EXPLAIN (FORMAT JSON) SELECT ...` via `sqlx::query` against the pool and assert `"Index Scan"` appears in the plan JSON (pattern novel for this codebase — no prior `EXPLAIN` test exists to copy).

SQLite half: `sqlx::query("EXPLAIN QUERY PLAN " + sql).fetch_all(pool)` and assert a row contains `"USING INDEX idx_job_runs_start_time"` or `"USING INDEX idx_job_runs_job_id_start"`.

**Imports pattern** (lines 18-24 of `schema_parity.rs`):
```rust
// Source: tests/schema_parity.rs:18-24 (verified)
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{PgPool, Row, SqlitePool};
use std::collections::{BTreeMap, BTreeSet};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
```

---

### `.github/workflows/ci.yml` (MOD — OBS-05 grep guard, optional)

**Analog:** existing lint-job step at lines 34-40

**Per-job `run:` step pattern** (lines 34-40):
```yaml
# Source: .github/workflows/ci.yml:34-40 (verified)
- run: just fmt-check
- run: just clippy
# `just openssl-check` depends on `just install-targets` and loops over
# native + amd64-musl + arm64-musl in a single run. One lint-job invocation
# covers every target CI ships (Pitfall 14 -- 01-RESEARCH.md S14).
- run: just openssl-check
```

Add either a `just grep-no-percentile-cont` recipe (invoke via `- run: just grep-no-percentile-cont` in lint job) OR a direct `- run:` shell step with `! rg -q 'percentile_cont|percentile_disc' src/` gate. Research Open Question #4 recommends the CI-step approach (lower cognitive load than a reflection-style Rust test).

Phase 13 Claude's-Discretion item D-22 restricts `release.yml` edits — `ci.yml` is NOT mentioned, so this addition is within scope.

---

## Shared Patterns

### Dual-path SQL with `PoolRef` match (MANDATORY for every new query)

**Source:** `src/db/queries.rs:580-653` (`get_dashboard_jobs`)

**Apply to:** `get_dashboard_job_sparks`, `get_recent_successful_durations`, `get_timeline_runs` (all three new queries).

**Concrete excerpt:**
```rust
match pool.reader() {
    PoolRef::Sqlite(p) => {
        // ?1 placeholders, j.enabled = 1
        let rows = sqlx::query(&sqlite_sql).bind(arg1).fetch_all(p).await?;
        Ok(rows.into_iter().map(|r| /* struct literal with r.get("col") */).collect())
    }
    PoolRef::Postgres(p) => {
        // $1 placeholders, j.enabled = true
        let rows = sqlx::query(&pg_sql).bind(arg1).fetch_all(p).await?;
        Ok(rows.into_iter().map(|r| /* ... */).collect())
    }
}
```

**`PoolRef` enum defined at:** `src/db/queries.rs:15` (`pub enum PoolRef<'a>`). `DbPool::reader()` at line 30.

Status literals MUST be lowercase and identical across branches (Risks #1). All phase 13 queries use the READER pool — no writes (CLAUDE.md separate-read/write-pools constraint).

---

### View-model hydration in `to_view()` / handler body

**Source:** `src/web/handlers/dashboard.rs:76-132` (`to_view`) + lines 180-219 (handler)

**Apply to:** all three handlers (dashboard extension, job_detail extension, new timeline handler).

**Contract:**
- Handler fetches rows from `queries::*` with `.unwrap_or_default()` fallback (never returns a 500 for a read-only GET at homelab scale).
- Raw DB types (`DashboardJob`, `DbRun`, `TimelineRun`) are converted to typed view structs (`DashboardJobView`, `JobDetailView`, etc.) via `to_view(row, tz)`.
- Templates consume ONLY view structs — never raw DB rows.
- All timestamps pass through `state.tz` via `dt.with_timezone(&tz)`.

**Concrete excerpt** (handler body skeleton):
```rust
let rows = queries::get_*(&state.pool, /* args */).await.unwrap_or_default();
let tz: Tz = state.tz;
let views: Vec<View> = rows.into_iter().map(|r| to_view(r, tz)).collect();
if is_htmx {
    Partial { /* ... */ }.into_web_template().into_response()
} else {
    Page { /* ... */ }.into_web_template().into_response()
}
```

---

### Status → CSS class derivation

**Source:** `templates/partials/job_table.html:12` (`cd-badge--{{ job.last_status }}`)

**Apply to:**
- Sparkline cells: `<span class="cd-sparkline-cell cd-sparkline-cell--{{ cell.kind }}">`
- Timeline bars: `<a class="cd-timeline-bar cd-timeline-bar--{{ bar.status }}{% if bar.status == "running" %} cd-timeline-bar--pulsing{% endif %}">`

**Concrete excerpt:**
```html
<!-- Source: templates/partials/job_table.html:12 (verified) -->
<span class="cd-badge cd-badge--{{ job.last_status }}">{{ job.last_status_label }}</span>
```

**Pattern:** the template interpolates the lowercase status string directly into the class name. The view-model MUST guarantee `last_status` / `cell.kind` / `bar.status` is pre-lowered and is one of the six canonical statuses (`success|failed|timeout|cancelled|stopped|running`) plus the phase-13-only `empty` kind for placeholder cells. Keep this property in view-model hydration per `dashboard.rs:90` (`let last_status = job.last_status.as_deref().unwrap_or("").to_lowercase();`).

---

### HTMX outer-HTML poll with external hidden `<input>` for state

**Source:** `templates/pages/dashboard.html:89-95`

**Apply to:** `templates/pages/timeline.html`.

**Concrete excerpt:**
```html
<!-- Source: templates/pages/dashboard.html:89-95 (verified) -->
<tbody id="job-table-body"
       hx-get="/partials/job-table"
       hx-trigger="every 3s"
       hx-swap="innerHTML"
       hx-include="[name='filter'],[name='sort'],[name='order']">
  {% include "partials/job_table.html" %}
</tbody>
```

Timeline's variant at 30s cadence:
```html
<!-- hidden input OUTSIDE the swap target so pill navigation + 30s poll don't destroy state -->
<input type="hidden" name="window" value="{{ window }}">

<div id="timeline-body"
     hx-get="/timeline"
     hx-trigger="every 30s"
     hx-swap="outerHTML"
     hx-include="[name='window']">
  {% include "partials/timeline_body.html" %}
</div>
```

**Key invariant:** the hidden `<input name="...">` that carries state across polls MUST live OUTSIDE the swap target (Risks #13). Dashboard achieves this via `innerHTML` swap (hidden inputs at line 34-35 are in the parent, not `<tbody>`). Timeline uses `outerHTML` and puts the hidden input above `#timeline-body`.

---

### Integration test harness: in-memory SQLite + full AppState construction

**Source:** `tests/dashboard_render.rs:26-52` + `tests/job_detail_partial.rs:38-69`

**Apply to:** all Phase 13 integration tests (`v13_sparkline_render.rs`, `v13_duration_card.rs`, `v13_timeline_render.rs`, `v13_timeline_timezone.rs`).

**Concrete excerpt:**
```rust
// Source: tests/dashboard_render.rs:26-52 (verified)
async fn build_test_app() -> (axum::Router, DbPool) {
    let pool = DbPool::connect("sqlite::memory:").await.expect("in-memory sqlite pool");
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

**Load-bearing:** the 10-field `AppState` literal must match exactly. Timezone tests override `tz:` to `chrono_tz::Tz::America_Los_Angeles`; all other fields stay constant across Phase 13 tests.

**Preferred alternative:** `tests/common/v11_fixtures.rs::setup_sqlite_with_phase11_migrations` already bundles the DB-setup half. Phase 13 tests can import it to save boilerplate; but the AppState construction must still be inlined per test file (no shared helper exists yet).

---

### Focus-visible ring convention (keyboard navigation)

**Source:** `assets/src/app.css:252-255` (`cd-btn-stop:focus-visible`)

**Apply to:** `.cd-pill:focus-visible`, `.cd-timeline-bar:focus-visible` (every new interactive element).

**Concrete excerpt:**
```css
/* Source: assets/src/app.css:252-255 (verified) */
.cd-btn-stop:focus-visible {
    outline: none;
    border-color: var(--cd-border-focus);
    box-shadow: 0 0 0 2px var(--cd-green-dim);
}
```

Universal project convention — every interactive element uses this exact three-line focus treatment. Phase 13 extends without modification.

---

### rc.2 release mechanics reuse (no new release engineering)

**Source:** `.github/workflows/release.yml:107-135` (tag-gating patches) + `docs/release-rc.md` (maintainer runbook) + `scripts/verify-latest-retag.sh`

**Apply to:** Phase 13 close-out tag cut.

**Concrete excerpt** (release.yml tag-gating, shown abbreviated):
```yaml
# Source: .github/workflows/release.yml:130-135 (verified)
tags: |
  type=semver,pattern={{version}}
  type=semver,pattern={{major}}.{{minor}},enable=${{ !contains(github.ref, '-') }}
  type=semver,pattern={{major}},enable=${{ !contains(github.ref, '-') }}
  type=raw,value=latest,enable=${{ !contains(github.ref, '-') }}
  type=raw,value=rc,enable=${{ contains(github.ref, '-rc.') }}
```

**Zero edits to release.yml or docs/release-rc.md** per D-22. Phase 13 close-out:
1. Merge all Phase 13 PRs.
2. Run `scripts/verify-latest-retag.sh 1.0.1` (Phase 12.1).
3. Preview: `git cliff --unreleased --tag v1.1.0-rc.2 -o /tmp/rc2-preview.md`.
4. Local tag: `git tag -a -s v1.1.0-rc.2 -m "v1.1.0-rc.2 — release candidate"` (OR `-a` unsigned fallback).
5. Push: `git push origin v1.1.0-rc.2` → release.yml fires.
6. Close-out commit flips `.planning/REQUIREMENTS.md` OBS-01..OBS-05 from `[ ]` to `[x]`.

Tag format is `v1.1.0-rc.2` (dot before `rc.N`) per Risks #14 — metadata-action's `contains(github.ref, '-rc.')` gate depends on the dot.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `tests/v13_timeline_explain.rs` | test (integration, EXPLAIN-plan scan) | plan-JSON introspection | No existing test in the codebase runs `EXPLAIN QUERY PLAN` (SQLite) or `EXPLAIN (FORMAT JSON)` (Postgres) and asserts index usage. Closest analogs (`schema_parity.rs` + `db_pool_postgres.rs`) cover testcontainer setup + dual-backend shape, but the EXPLAIN-parsing assertions are novel. Planner writes this from scratch following Research § Validation Architecture. |

All other files have an exact or role-match analog.

---

## Metadata

**Analog search scope:**
- `src/db/queries.rs`
- `src/web/` (handlers, format, stats-to-be)
- `templates/` (pages + partials + base)
- `assets/src/app.css` (`@layer components` block)
- `tests/` (all integration fixtures + common helpers)
- `.github/workflows/` (release.yml + ci.yml)
- `scripts/`
- `migrations/sqlite/` (schema reference for index verification)

**Files scanned:** 21 source/template/test/config files read + grep sweeps across `src/`, `templates/`, `tests/`, `assets/`, `.github/`.

**Pattern extraction date:** 2026-04-21

**Downstream contract:** Planner references these analogs verbatim in each plan's action section; each new file has a concrete source-file + line-range to copy from.

## PATTERN MAPPING COMPLETE
