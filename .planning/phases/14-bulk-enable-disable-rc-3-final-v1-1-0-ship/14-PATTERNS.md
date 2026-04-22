# Phase 14: Bulk Enable/Disable + rc.3 + Final v1.1.0 Ship — Pattern Map

**Mapped:** 2026-04-22
**Files analyzed:** 22 (7 new + 15 modified)
**Analogs found:** 22 / 22 — every new file has at least one in-tree template to copy

This PATTERNS.md gives the planner a per-file "what to copy + what to change" recipe.
Every code excerpt below is **verbatim** from HEAD (verified line numbers).
Executor must NOT reinvent: copy the analog, change the specific field / SQL clause / selector per the "Adaptation notes".

---

## File Classification

| File | Role | Data Flow | Closest Analog | Match Quality |
|------|------|-----------|----------------|---------------|
| `src/web/handlers/api.rs::bulk_toggle` (NEW fn) | controller (HTTP handler) | request-response (CSRF + DB-mutate + Reload + HX-Trigger) | `src/web/handlers/api.rs::stop_run` L374-463 | exact |
| `src/db/queries.rs::bulk_set_override` (NEW fn) | query helper (writer pool) | CRUD UPDATE over list | `src/db/queries.rs::disable_missing_jobs` L129-169 | exact |
| `src/db/queries.rs::get_overridden_jobs` (NEW fn) | query helper (reader pool) | CRUD SELECT list | `src/db/queries.rs::get_enabled_jobs` L172-191 | exact |
| `src/db/queries.rs::count_running_runs_for_jobs` (NEW fn, optional) | query helper (reader) | CRUD SELECT count | `src/db/queries.rs::get_enabled_jobs` (reader fanout) + AppState `active_runs` RwLock | partial (prefer AppState per Open Question 2) |
| `src/db/queries.rs::DbJob + SqliteDbJobRow + PgDbJobRow` (MOD) | struct | data model | same file L38-280 | same-file extension |
| `src/db/queries.rs::get_enabled_jobs` (MOD) | query helper (reader) | CRUD SELECT (filter change) | itself — existing shape stays | in-place |
| `src/db/queries.rs::disable_missing_jobs` (MOD) | query helper (writer) | CRUD UPDATE (SET clause extension) | itself — existing shape stays | in-place |
| `src/db/queries.rs::upsert_job` (MOD-FREEZE) | query helper (writer) | CRUD INSERT ... ON CONFLICT | **STAYS AS-IS — T-V11-BULK-01 invariant** | freeze |
| `src/web/handlers/dashboard.rs::DashboardJobView + to_view()` (MOD) | view model | transform | same file L66-172 | in-place add one field |
| `src/web/handlers/settings.rs::SettingsPage + settings()` (MOD) | view model + handler | transform + SSR | same file L17-105 | in-place add one Vec field |
| `src/web/mod.rs` (MOD) | router wiring | request-response | same file L78-81 (existing `POST /api/...` cluster) | one-line append |
| `migrations/sqlite/20260422_000004_enabled_override_add.up.sql` (NEW) | migration (SQLite DDL) | schema | `migrations/sqlite/20260416_000001_job_run_number_add.up.sql` | role-match (not 3-step dance — single ADD COLUMN NULLABLE) |
| `migrations/postgres/20260422_000004_enabled_override_add.up.sql` (NEW) | migration (Postgres DDL) | schema | `migrations/postgres/20260416_000001_job_run_number_add.up.sql` | role-match |
| `templates/pages/dashboard.html` (MOD) | template | SSR + HTMX | itself — existing thead + tbody + 3s poll | in-place insert |
| `templates/partials/job_table.html` (MOD) | template partial | SSR (per-row) | itself — existing `<tr>` shape L1-32 | in-place prepend `<td>` |
| `templates/pages/settings.html` (MOD) | template | SSR | itself — 6-card grid L14-71 | in-place append `<section>` |
| `assets/src/app.css` (MOD) | stylesheet | component CSS | `cd-btn-stop` + `cd-badge--stopped` precedent L198-284 | additive selectors only |
| `tests/v11_bulk_toggle.rs` (NEW) | integration test | axum test + sqlx | `tests/stop_handler.rs` + `tests/dashboard_jobs_pg.rs` | exact (two complementary analogs) |
| `THREAT_MODEL.md` (MOD) | documentation | prose | existing Stop-button bullet at L113 | exact parallel bullet |
| `justfile` (MOD — NEW recipes) | build/dev recipe | CLI | existing `docker-compose-up` L269-272 + `dev` L245-250 | role-match |
| `cliff.toml` (UNCHANGED) | config | release notes gen | — | no-touch |
| `MILESTONES.md` + `README.md` (MOD on final-promotion commit) | documentation | prose | existing v1.0 entry | exact parallel |

---

## Pattern Assignments

### 1. `src/web/handlers/api.rs::bulk_toggle` (NEW handler)

**Role / data flow:** HTTP POST handler — CSRF gate → validate action → dedupe → DB UPDATE → fire `SchedulerCmd::Reload` → empty 200 + `HX-Trigger` toast.

**Closest analog:** `src/web/handlers/api.rs::stop_run` at L374-463. Same shape: CSRF-first, scheduler-cmd-dispatch, HX-Trigger toast, 503 on channel closed.

**Secondary analog for the `Reload` dispatch call pattern:** `reload` handler at L146-153.

**Imports + header block pattern** (L1-24, copy verbatim, only append `axum_extra::extract::Form as ExtraForm` + `std::collections::BTreeSet`):
```rust
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum_extra::extract::CookieJar;
use axum_htmx::HxEvent;
use axum_htmx::HxResponseTrigger;
use serde::Deserialize;
use serde_json::json;

use crate::db::queries;
use crate::scheduler::cmd::{ReloadStatus, SchedulerCmd, StopResult};
use crate::web::AppState;
use crate::web::csrf;

#[derive(Deserialize)]
pub struct CsrfForm {
    csrf_token: String,
}
```

**CSRF pattern — copy verbatim from `stop_run` L380-388** (same block in `run_now` L32-40, `reload` L135-143, `reroll` L250-258):
```rust
// 1. Validate CSRF (T-10-07-01) — copy-verbatim from run_now.
let cookie_token = cookies
    .get(csrf::CSRF_COOKIE_NAME)
    .map(|c| c.value().to_string())
    .unwrap_or_default();

if !csrf::validate_csrf(&cookie_token, &form.csrf_token) {
    return (StatusCode::FORBIDDEN, "CSRF token mismatch").into_response();
}
```

**Toast-trigger pattern — copy verbatim from `stop_run` L428-438:**
```rust
// Normal path — toast + HX-Refresh (verbatim pattern from run_now).
let event = HxEvent::new_with_data(
    "showToast",
    json!({"message": format!("Stopped: {}", run.job_name), "level": "info"}),
)
.expect("toast event serialization");

let mut headers = axum::http::HeaderMap::new();
headers.insert("HX-Refresh", "true".parse().unwrap());

(HxResponseTrigger::normal([event]), headers, StatusCode::OK).into_response()
```

**Scheduler-cmd + oneshot-reply pattern — copy from `stop_run` L409-418** (Reload variant is simpler — no oneshot awaited; see Pitfall §6):
```rust
let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
match state
    .cmd_tx
    .send(SchedulerCmd::Stop {
        run_id,
        response_tx: resp_tx,
    })
    .await
{
    Ok(()) => match resp_rx.await {
        // ... success branch ...
    },
    Err(_) => (
        StatusCode::SERVICE_UNAVAILABLE,
        "Scheduler is shutting down",
    )
        .into_response(),
}
```

**Adaptation notes:**
- **CRITICAL correction to CONTEXT D-11:** swap `axum::Form<CsrfForm>` (used by every analog — `run_now` L30, `reload` L133, `reroll` L248, `stop_run` L378) for `axum_extra::extract::Form<BulkToggleForm>`. Stock `axum::Form` uses `serde_urlencoded` which does NOT support `Vec<T>` from repeated keys. `axum_extra::Form` uses `serde_html_form` and does. RESEARCH §axum 0.8 Form Handler Pattern.
- Add `#[axum::debug_handler]` above the fn (Landmine §8 — readable extractor-order errors). None of the existing analogs use it; this handler is the first to need it because of the mixed `FromRequestParts` + `FromRequest` extractors.
- **Extractor order (enforced by axum 0.8):** `State` → `CookieJar` → `ExtraForm` (body-consuming last).
- Use `HX-Trigger` ONLY (no `HX-Refresh`) per CONTEXT D-12b — the 3s dashboard poll picks up DB state on next cycle. This diverges from `stop_run` / `reload` / `reroll` which all `HX-Refresh: true`.
- For the `Reload` dispatch: use the `reload` handler L146-153 shape BUT **do not await `resp_rx`** — drop the receiver, return immediately after `send` succeeds. Rationale: we don't need reload status in the toast; the toast reports `rows_affected` from the UPDATE.
- Add `BulkToggleForm { csrf_token: String, action: String, #[serde(default)] job_ids: Vec<i64> }` alongside `CsrfForm` at L21. `#[serde(default)]` is load-bearing — without it `serde_html_form` rejects missing `job_ids` key with a 400 before the handler runs (Landmine §9).
- Dedupe via `BTreeSet<i64>` after deserialize (D-12a).
- Empty-ids rejection: explicit `if form.job_ids.is_empty()` → 400 + error toast "No jobs selected." (Claude's-Discretion resolution in UI-SPEC).
- Running-count for verbose toast: Research Open Question 2 recommends reading `state.active_runs` RwLock (no new query). Planner may use either; recommend RwLock.
- **Do NOT** look up per-job names or render the full list in the toast (deferred — see CONTEXT `<deferred>` "Bulk action with per-job name list").

---

### 2. `src/db/queries.rs::bulk_set_override` (NEW fn)

**Role / data flow:** writer-pool UPDATE that sets `enabled_override = ?` for a list of job ids.

**Closest analog:** `disable_missing_jobs` L129-169 — IDENTICAL SQL-backend-split shape: SQLite builds `IN (?1..?N)` placeholder list; Postgres uses `ANY($1)` array bind.

**Verbatim excerpt (L129-169):**
```rust
/// Disable all jobs whose names are NOT in `active_names`.
/// Returns the count of rows that were disabled.
pub async fn disable_missing_jobs(pool: &DbPool, active_names: &[String]) -> anyhow::Result<u64> {
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            if active_names.is_empty() {
                let result = sqlx::query("UPDATE jobs SET enabled = 0 WHERE enabled = 1")
                    .execute(p)
                    .await?;
                return Ok(result.rows_affected());
            }
            // SQLite doesn't support array binds; build a parameterized IN list.
            let placeholders: Vec<String> =
                (1..=active_names.len()).map(|i| format!("?{i}")).collect();
            let sql = format!(
                "UPDATE jobs SET enabled = 0 WHERE enabled = 1 AND name NOT IN ({})",
                placeholders.join(", ")
            );
            let mut query = sqlx::query(&sql);
            for name in active_names {
                query = query.bind(name);
            }
            let result = query.execute(p).await?;
            Ok(result.rows_affected())
        }
        PoolRef::Postgres(p) => {
            if active_names.is_empty() {
                let result = sqlx::query("UPDATE jobs SET enabled = 0 WHERE enabled = 1")
                    .execute(p)
                    .await?;
                return Ok(result.rows_affected());
            }
            // Postgres supports ANY($1) with array bind.
            let result = sqlx::query(
                "UPDATE jobs SET enabled = 0 WHERE enabled = 1 AND NOT (name = ANY($1))",
            )
            .bind(active_names)
            .execute(p)
            .await?;
            Ok(result.rows_affected())
        }
    }
}
```

**Adaptation notes:**
- Signature: `pub async fn bulk_set_override(pool: &DbPool, ids: &[i64], new_override: Option<i64>) -> anyhow::Result<u64>`.
- Empty-ids early return: handler already rejects empty with 400; defensive `if ids.is_empty() { return Ok(0); }` matches the analog's early return.
- SQLite placeholder layout — **shift by one because we bind `new_override` first** (the analog has no UPDATE SET parameter to bind): `?1` is the override value; ids use `?2..?(N+1)`. Build placeholder list like:
  ```rust
  let placeholders: Vec<String> = (2..=ids.len()+1).map(|i| format!("?{i}")).collect();
  let sql = format!(
      "UPDATE jobs SET enabled_override = ?1 WHERE id IN ({})",
      placeholders.join(", ")
  );
  let mut q = sqlx::query(&sql).bind(new_override);  // Option<i64> → NULL when None
  for id in ids { q = q.bind(id); }
  ```
- Postgres — single UPDATE with `ANY($2)` and `.bind(ids)` as `&[i64]` → `BIGINT[]`:
  ```rust
  let result = sqlx::query("UPDATE jobs SET enabled_override = $1 WHERE id = ANY($2)")
      .bind(new_override)  // Option<i64>
      .bind(ids)           // &[i64]
      .execute(p).await?;
  ```
- Postgres column-type note: `enabled_override` is `BIGINT NULL`; binding `Option<i64>` is correct and mirrors the existing `enabled` column handling in the SQLite↔Postgres normalization below.
- SQLite `Option<i64>` binds as `NULL` when `None` — standard sqlx behavior; no special handling.

---

### 3. `src/db/queries.rs::get_overridden_jobs` (NEW fn)

**Role / data flow:** reader-pool SELECT of every job with `enabled_override IS NOT NULL`, alphabetical.

**Closest analog:** `get_enabled_jobs` L172-191 + `get_job_by_name` L194-215. Same fanout over `reader()`, same `SqliteDbJobRow` / `PgDbJobRow` mapping, same `FromRow` hydration.

**Verbatim excerpt (`get_enabled_jobs`, L172-191):**
```rust
/// Fetch all enabled jobs from the database.
pub async fn get_enabled_jobs(pool: &DbPool) -> anyhow::Result<Vec<DbJob>> {
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let rows = sqlx::query_as::<_, SqliteDbJobRow>(
                "SELECT id, name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, timeout_secs, created_at, updated_at FROM jobs WHERE enabled = 1",
            )
            .fetch_all(p)
            .await?;
            Ok(rows.into_iter().map(|r| r.into()).collect())
        }
        PoolRef::Postgres(p) => {
            let rows = sqlx::query_as::<_, PgDbJobRow>(
                "SELECT id, name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, timeout_secs, created_at, updated_at FROM jobs WHERE enabled = 1",
            )
            .fetch_all(p)
            .await?;
            Ok(rows.into_iter().map(|r| r.into()).collect())
        }
    }
}
```

**Adaptation notes:**
- Signature: `pub async fn get_overridden_jobs(pool: &DbPool) -> anyhow::Result<Vec<DbJob>>`.
- SELECT list gains `enabled_override` AND the struct change below adds the column to both row types. The SELECT literal becomes:
  ```sql
  SELECT id, name, schedule, resolved_schedule, job_type, config_json, config_hash,
         enabled, enabled_override, timeout_secs, created_at, updated_at
  FROM jobs
  WHERE enabled_override IS NOT NULL
  ORDER BY name ASC
  ```
- Same literal SQL for both backends — `ORDER BY name ASC` is dialect-neutral (D-10b).
- `get_enabled_jobs` (existing, MOD): apply the same `enabled_override` addition to SELECT columns AND change the WHERE filter to `WHERE enabled = 1 AND (enabled_override IS NULL OR enabled_override = 1)` (RESEARCH §sqlx patterns).

---

### 4. `src/db/queries.rs::DbJob` struct (MOD)

**Role / data flow:** shared data transfer type between queries and view models.

**Closest analog:** itself. The existing struct (L38-52):
```rust
/// A row from the `jobs` table.
#[derive(Debug, Clone)]
pub struct DbJob {
    pub id: i64,
    pub name: String,
    pub schedule: String,
    pub resolved_schedule: String,
    pub job_type: String,
    pub config_json: String,
    pub config_hash: String,
    pub enabled: bool,
    pub timeout_secs: i64,
    pub created_at: String,
    pub updated_at: String,
}
```

**Adaptation notes:**
- Add `pub enabled_override: Option<i64>,` — CONTEXT places it after `enabled`. Field order should match SELECT column order to keep mental alignment.
- The compiler will error on every hydrator missing this field — use that as the task checklist (Landmine §11).

---

### 5. `src/db/queries.rs::SqliteDbJobRow + PgDbJobRow` FromRow impls (MOD)

**Role / data flow:** sqlx `FromRow` wrappers for per-backend column-type splits (SQLite INTEGER-as-bool, Postgres native bool).

**Closest analog:** itself. Existing L217-280 (verbatim):
```rust
// Internal row types for sqlx::FromRow mapping (SQLite uses i32/i64 for booleans).

#[derive(FromRow)]
struct SqliteDbJobRow {
    id: i64,
    name: String,
    schedule: String,
    resolved_schedule: String,
    job_type: String,
    config_json: String,
    config_hash: String,
    enabled: i32,
    timeout_secs: i64,
    created_at: String,
    updated_at: String,
}

impl From<SqliteDbJobRow> for DbJob {
    fn from(r: SqliteDbJobRow) -> Self {
        DbJob {
            id: r.id,
            name: r.name,
            schedule: r.schedule,
            resolved_schedule: r.resolved_schedule,
            job_type: r.job_type,
            config_json: r.config_json,
            config_hash: r.config_hash,
            enabled: r.enabled != 0,
            timeout_secs: r.timeout_secs,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(FromRow)]
struct PgDbJobRow {
    id: i64,
    name: String,
    schedule: String,
    resolved_schedule: String,
    job_type: String,
    config_json: String,
    config_hash: String,
    enabled: bool,
    timeout_secs: i64,
    created_at: String,
    updated_at: String,
}
```

**Adaptation notes:**
- `SqliteDbJobRow`: add `enabled_override: Option<i32>` (SQLite INTEGER NULL → `Option<i32>`).
- `PgDbJobRow`: add `enabled_override: Option<i64>` (Postgres BIGINT NULL → `Option<i64>`).
- In `From<SqliteDbJobRow> for DbJob`: normalize via `enabled_override: r.enabled_override.map(|v| v as i64)` — widens `i32` → `i64` to match the shared `DbJob` field type. Precedent: `enabled: r.enabled != 0` at L244 collapses SQLite i32 to bool.
- In `From<PgDbJobRow> for DbJob`: pass-through `enabled_override: r.enabled_override` (already `Option<i64>`).

---

### 6. `src/db/queries.rs::upsert_job` (MOD — FREEZE)

**Role / data flow:** writer-pool INSERT ... ON CONFLICT (name) DO UPDATE.

**This function STAYS EXACTLY AS-IS.** T-V11-BULK-01 asserts the invariant.

**Verbatim excerpt of the locked-frozen block (L57-125) — DO NOT TOUCH:**
```rust
pub async fn upsert_job(
    pool: &DbPool,
    name: &str,
    schedule: &str,
    resolved_schedule: &str,
    job_type: &str,
    config_json: &str,
    config_hash: &str,
    timeout_secs: i64,
) -> anyhow::Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            let row = sqlx::query(
                r#"INSERT INTO jobs (name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, timeout_secs, created_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?8)
                   ON CONFLICT(name) DO UPDATE SET
                       schedule = excluded.schedule,
                       resolved_schedule = excluded.resolved_schedule,
                       job_type = excluded.job_type,
                       config_json = excluded.config_json,
                       config_hash = excluded.config_hash,
                       enabled = 1,
                       timeout_secs = excluded.timeout_secs,
                       updated_at = excluded.updated_at
                   RETURNING id"#,
            )
            // ... (Postgres branch identical modulo $N / EXCLUDED)
```

**Adaptation notes:**
- `enabled_override` does NOT appear in the INSERT column list.
- `enabled_override` does NOT appear in the ON CONFLICT DO UPDATE SET clause.
- New rows get NULL (schema default for nullable column). Existing rows keep whatever `enabled_override` already had — which is the whole point of the reload-preserves-override invariant (ERG-04).
- `tests/v11_bulk_toggle.rs::upsert_invariant` asserts this by: seed a job → `bulk_set_override(&pool, &[id], Some(0))` → call `upsert_job` again with same-or-different config fields → assert `enabled_override` still `Some(0)`.

---

### 7. `src/db/queries.rs::disable_missing_jobs` (MOD)

**Role / data flow:** writer-pool UPDATE that disables jobs not in the active config list.

**Closest analog:** itself. Existing function L129-169 (already excerpted in Pattern #2 above).

**Adaptation notes:**
- Extend the SET clause on BOTH the empty-list and non-empty-list paths. Four `UPDATE jobs SET enabled = 0` strings in the function body (L133, L141-143 inside format!, L154, L160-162) ALL become `UPDATE jobs SET enabled = 0, enabled_override = NULL`.
- This is the ERG-04 reload-symmetry rule: a job that leaves the config loses BOTH its enabled flag AND any override.
- Dialect-neutral SET clause — no placeholder changes.
- `tests/v11_bulk_toggle.rs::disable_missing_clears_override` asserts this.

---

### 8. `src/web/handlers/api.rs::BulkToggleForm` struct (NEW, sibling of `CsrfForm`)

**Role / data flow:** serde-Deserialize form body.

**Closest analog:** `CsrfForm` at L21-24 (verbatim):
```rust
#[derive(Deserialize)]
pub struct CsrfForm {
    csrf_token: String,
}
```

**Adaptation notes:**
- Append near `CsrfForm`:
  ```rust
  #[derive(Deserialize)]
  pub struct BulkToggleForm {
      csrf_token: String,
      action: String,
      #[serde(default)]
      job_ids: Vec<i64>,
  }
  ```
- `#[serde(default)]` is mandatory (Landmine §9). Without it, a POST body lacking any `job_ids` key returns a 400 from the extractor before the handler runs; with it, the empty Vec deserializes cleanly and the handler's explicit-rejection path fires.
- Field visibility: the analog uses default-private (crate-visible) for the struct fields. Match that (no `pub` on fields).

---

### 9. `src/web/mod.rs` router wiring (MOD)

**Role / data flow:** axum Router builder chain.

**Closest analog:** lines L76-81 of the existing `/api/...` cluster (verbatim):
```rust
.route("/api/jobs", get(handlers::api::list_jobs))
.route("/api/jobs/{id}/runs", get(handlers::api::list_job_runs))
.route("/api/jobs/{id}/run", post(handlers::api::run_now))
.route("/api/reload", post(handlers::api::reload))
.route("/api/jobs/{id}/reroll", post(handlers::api::reroll))
.route("/api/runs/{run_id}/stop", post(handlers::api::stop_run))
```

**Adaptation notes:**
- Append ONE line immediately after the `stop_run` line:
  ```rust
  .route("/api/jobs/bulk-toggle", post(handlers::api::bulk_toggle))
  ```
- No `{id}` path param — `job_ids` come from the form body. This is the only POST in the cluster without an id in the path.
- No middleware changes. `csrf::ensure_csrf_cookie` layer at L87 already covers the new route.

---

### 10. `src/web/handlers/dashboard.rs::DashboardJobView + to_view()` (MOD)

**Role / data flow:** view-model struct + builder for the dashboard table.

**Closest analog:** itself — the existing struct at L66-87 and `to_view()` at L110-172 (excerpts above). Uses `DashboardJob` from `queries::DashboardJob` as input.

**Adaptation notes:**
- Add `pub enabled_override: Option<i64>,` to `DashboardJobView` somewhere with the other metadata fields (after `last_run_relative` is natural).
- **Important dependency:** `DashboardJobView` hydrates from `DashboardJob` (not `DbJob`). Grep `src/db/queries.rs::DashboardJob` (a different struct from `DbJob`) to confirm whether the dashboard-specific query also needs `enabled_override`. Per Context L177 and Research L839 the field flows through — verify during implementation and add to `DashboardJob` + `get_dashboard_jobs` if missing.
- `to_view()` pipes `enabled_override: job.enabled_override` through. The field is carried but NOT necessarily rendered — CONTEXT L177 marks this "optional flag for the planner" and Research Open Question 1 recommends NOT surfacing on the dashboard in v1.1. Keep it plumbed for templates that may opt in.
- Do NOT break any existing dashboard sort/filter queries. `get_dashboard_jobs` is the hot-path query recently fixed for the Postgres BIGINT-vs-bool bug (quick task 260421-nn3). Touch it only to add the new column to the SELECT list.

---

### 11. `src/web/handlers/settings.rs::SettingsPage + settings()` (MOD)

**Role / data flow:** view-model struct + handler for the settings page.

**Closest analog:** itself — existing struct at L17-29 and handler at L61-105 (excerpts above).

**Adaptation notes:**
- Add a new field to `SettingsPage`:
  ```rust
  overridden_jobs: Vec<OverriddenJobView>,
  ```
- Define `OverriddenJobView` inside `settings.rs` (keep per-handler view models co-located — same pattern as `DashboardJobView` in `dashboard.rs`):
  ```rust
  pub struct OverriddenJobView {
      pub id: i64,
      pub name: String,
      pub enabled_override: i64,  // non-null guaranteed by the query filter
  }
  ```
  UI-SPEC Design-System-Extension-Summary confirms this shape (`{ id, name, enabled_override }`).
- Hydrate in `settings()` AFTER the existing `last_reload` block:
  ```rust
  let overridden_jobs: Vec<OverriddenJobView> = queries::get_overridden_jobs(&state.pool)
      .await
      .unwrap_or_else(|err| {
          tracing::error!(target: "cronduit.web", error = %err, "settings: get_overridden_jobs failed");
          Vec::new()
      })
      .into_iter()
      .filter_map(|j| j.enabled_override.map(|ov| OverriddenJobView {
          id: j.id, name: j.name, enabled_override: ov,
      }))
      .collect();
  ```
  Fallback-on-err shape mirrors the existing error-swallowing pattern in `dashboard.rs`. Planner may prefer to 500 on error — either works; non-fatal degradation is kinder for a settings audit page.
- The template at `templates/pages/settings.html` references the struct fields directly via askama (compile-time checked); missing field = build error. Use this as the "did I remember to wire it?" gate.

---

### 12. `templates/pages/dashboard.html` (MOD)

**Role / data flow:** askama SSR template with HTMX 3s poll.

**Closest analog:** itself. Four insertion points:

**(a) The filter bar that closes at L36 + the overflow-wrapper that opens at L39** (verbatim):
```html
<!-- Filter bar -->
<div class="flex items-center gap-4 mb-6">
  ...
  <input type="hidden" name="sort" value="{{ sort }}">
  <input type="hidden" name="order" value="{{ order }}">
</div>

<!-- Job table -->
<div class="overflow-x-auto">
```

**(b) The `<thead><tr>` at L42 with its first `<th>Name</th>` at L44-52 (verbatim):**
```html
<tr style="background:var(--cd-bg-surface-raised)">
  <!-- Name (sortable) -->
  <th class="text-left py-2 px-4" style="font-size:var(--cd-text-xs);font-weight:700;text-transform:uppercase;letter-spacing:0.1em{% if sort == "name" %};color:var(--cd-text-accent){% endif %}">
    ...
  </th>
```

**(c) The existing HTMX 3s poll `<tbody>` at L91-95 (verbatim):**
```html
<tbody id="job-table-body"
       hx-get="/partials/job-table"
       hx-trigger="every 3s"
       hx-swap="innerHTML"
       hx-include="[name='filter'],[name='sort'],[name='order']">
  {% include "partials/job_table.html" %}
</tbody>
```

**(d) Close `{% endblock %}` at L101 — inline `<script>` helpers go BEFORE this line.**

**Adaptation notes:**
- Insert the `<div id="cd-bulk-action-bar" class="cd-bulk-bar" hidden>` AS A SIBLING between the filter-bar `</div>` at L36 and the `<div class="overflow-x-auto">` at L39 — **NOT inside the overflow wrapper** (Research Landmine §4: position:sticky + overflow-x: auto interaction; keep the bar OUTSIDE the overflow parent so sticky attaches to the viewport, not the table wrapper).
- Insert a new `<th>` BEFORE the "Name" `<th>` at L44 for the select-all checkbox. Mirror the column-header style but with `min-width:44px;width:44px` for WCAG touch target.
- Leave the 3s poll `<tbody>` block unchanged — the row checkboxes rely on `hx-preserve` to survive the swap (Landmine §3).
- Add the inline `<script>` block per UI-SPEC Surface A (lines 270-303) BEFORE `{% endblock %}` — `__cdBulkSelectAll`, `__cdBulkOnRowChange`, `__cdBulkUpdateIndeterminate`, `__cdBulkUpdateBar`, `__cdBulkClearSelection`, plus `htmx:afterSwap` listener.
- Template variable to pass from handler: `csrf_token` already in scope (existing struct field at `dashboard.rs:52`). No additional view-model change required for dashboard.html.

---

### 13. `templates/partials/job_table.html` (MOD)

**Role / data flow:** per-row `<tr>` partial included by the polled `<tbody>`.

**Closest analog:** itself. Existing file (whole file, verbatim, L1-32):
```html
{% for job in jobs %}
<tr class="hover:bg-(--cd-bg-hover) border-b border-(--cd-border-subtle)">
  <td class="py-2 px-4">
    <a href="/jobs/{{ job.id }}" class="text-(--cd-text-accent) hover:underline font-bold">{{ job.name }}</a>
  </td>
  <td class="py-2 px-4 text-(--cd-text-secondary)" style="font-size:var(--cd-text-base)">
    {{ job.resolved_schedule }}{% if job.has_random_schedule %} <span class="cd-badge cd-badge--random">@random</span>{% endif %}
  </td>
  ...
  <td class="py-2 px-4">
    <form hx-post="/api/jobs/{{ job.id }}/run" hx-swap="none">
      <input type="hidden" name="csrf_token" value="{{ csrf_token }}">
      <button type="submit" class="cd-btn-primary text-sm py-1 px-3" style="min-height:36px">Run Now</button>
    </form>
  </td>
</tr>
{% endfor %}
```

**Adaptation notes:**
- Prepend ONE new `<td>` INSIDE the `<tr>` at L2, BEFORE the existing L3 `<td>` Name cell:
  ```html
  <td class="py-2 px-4" style="width:44px">
    <input type="checkbox"
           id="cd-row-cb-{{ job.id }}"
           class="cd-row-checkbox"
           name="job_ids"
           value="{{ job.id }}"
           aria-label="Select {{ job.name }}"
           hx-preserve="true"
           onclick="__cdBulkOnRowChange()">
  </td>
  ```
- **Critical:** `id="cd-row-cb-{{ job.id }}"` is load-bearing for `hx-preserve` (Research Landmine §3). Without a stable unique id, `hx-preserve` silently fails and every 3s poll wipes selection state.
- `name="job_ids"` (NOT `job_ids[]`) — HTMX's form-encoder emits repeated keys (`job_ids=1&job_ids=2`) which `serde_html_form` collects into `Vec<i64>`.
- Existing per-row CSRF hidden input at L27 stays inside the Run Now form; the bulk-bar uses its OWN hidden CSRF input (rendered from dashboard.html) + `hx-include="[name='csrf_token']"` to pick it up.

---

### 14. `templates/pages/settings.html` (MOD)

**Role / data flow:** askama SSR for the settings page.

**Closest analog:** itself. The 6-card grid closes at L71 with a single `</div>`. One card (verbatim L29-40) shows the aesthetic contract:
```html
<!-- Config Watcher (Component 7) -->
<div style="background:var(--cd-bg-surface);border:1px solid var(--cd-border);border-radius:8px;padding:var(--cd-space-6)">
  <span style="font-size:var(--cd-text-xs);color:var(--cd-text-secondary);text-transform:uppercase;letter-spacing:0.1em">Config Watcher</span>
  <div style="margin-top:4px">
    {% if watch_config %}
    <span class="cd-badge cd-badge--success">WATCHING</span>
    {% else %}
    <span class="cd-badge cd-badge--disabled">DISABLED</span>
    {% endif %}
  </div>
  <div style="font-size:var(--cd-text-sm);color:var(--cd-text-secondary);margin-top:2px">{{ config_path }}</div>
</div>
```

The grid closing `</div>` is at L71, and `{% endblock %}` is at L73.

**Adaptation notes:**
- Insert a new `<section>` between the grid closing `</div>` at L71 and the `{% endblock %}` at L73. Verbatim shape per UI-SPEC Surface D (lines 515-568) — a `<h2>Currently Overridden</h2>` + description `<p>` + `<div class="overflow-x-auto"><table>` with three columns (Name / Override State / Clear).
- Wrap the ENTIRE new `<section>` in `{% if !overridden_jobs.is_empty() %}...{% endif %}` (D-10a: empty state hides the section).
- Column-header styling is copy-paste from dashboard.html L44 (verbatim same inline `font-size:var(--cd-text-xs);font-weight:700;text-transform:uppercase;letter-spacing:0.1em;color:var(--cd-text-secondary)`).
- Badge render uses SHIPPED `cd-badge--disabled` class for `enabled_override == 0` rows; NEW `cd-badge--forced` class (see Pattern §17) for the defensive `== 1` case.
- Per-row Clear button wraps in its own `<form hx-post="/api/jobs/bulk-toggle" hx-swap="none">` with three hidden inputs (`csrf_token`, `action=enable`, `job_ids={{ job.id }}`) — this mirrors the per-row Run Now form shape at `partials/job_table.html:26-29`.
- Heading font-size is `--cd-text-lg` (NOT `--cd-text-xl` — D-09 + UI-SPEC Typography § explicitly drop one tier for the subordinate `<h2>`).

---

### 15. `assets/src/app.css` (MOD — additive selectors only)

**Role / data flow:** Tailwind v4 `@layer components` stylesheet.

**Closest analog:** existing `cd-badge--stopped` at L206 + `cd-btn-stop:hover` at L258-262 (verbatim):
```css
.cd-badge--stopped { color: var(--cd-status-stopped); background: var(--cd-status-stopped-bg); }

.cd-btn-stop:hover {
  background: var(--cd-status-stopped-bg);
  border-color: var(--cd-status-stopped);
  color: var(--cd-status-stopped);
}
```

The shipped `cd-badge` block spans L188-206 with ten `cd-badge--*` variants; `cd-btn-stop` at L245-284 demonstrates the secondary-button-with-status-hover-tint pattern.

**Adaptation notes:**
- Append after the `cd-btn-stop--compact` block (ends ~L284) inside the same `@layer components` block. All SIX new selectors per UI-SPEC Design-System-Extension-Summary (lines 658-666):
  - `.cd-row-checkbox` + `.cd-row-checkbox:focus-visible` (UI-SPEC Surface A CSS).
  - `#cd-select-all` (inherits from `.cd-row-checkbox`).
  - `.cd-bulk-bar` + `.cd-bulk-bar[hidden]` + `.cd-bulk-bar-count` (UI-SPEC Surface B CSS).
  - `.cd-btn-secondary.cd-btn-disable-hint:hover` + `.cd-btn-secondary.cd-btn-disable-hint:active` (UI-SPEC Color § destructive-leaning hover tint).
  - `.cd-badge--forced` — parallel to the existing `cd-badge--stopped` one-liner at L206:
    ```css
    .cd-badge--forced { color: var(--cd-status-running); background: var(--cd-status-running-bg); }
    ```
- **Zero new tokens.** `--cd-status-running`, `--cd-status-running-bg`, `--cd-status-disabled`, `--cd-status-disabled-bg`, `--cd-bg-surface-raised`, `--cd-border`, `--cd-space-*`, `--cd-radius-md`, `--cd-text-accent` all shipped.
- **Zero modifications to existing selectors.** Additive only.
- The `cd-btn-stop:hover` at L258-262 is the IDIOM for the new `cd-btn-disable-hint:hover` — both are "secondary-button base + status-color hover tint".

---

### 16. `migrations/sqlite/20260422_000004_enabled_override_add.up.sql` (NEW)

**Role / data flow:** forward-only SQLite migration (ALTER TABLE).

**Closest analog:** `migrations/sqlite/20260416_000001_job_run_number_add.up.sql` (whole file, verbatim, L1-19):
```sql
-- Phase 11: per-job run numbering (DB-09, DB-10, DB-11) — SQLite.
--
-- File 1 of 3. Adds nullable job_runs.job_run_number plus the NOT NULL
-- jobs.next_run_number counter (DEFAULT 1). Files 2 and 3 backfill existing
-- rows and tighten to NOT NULL respectively.
--
-- Pairs with migrations/postgres/20260416_000001_job_run_number_add.up.sql.
-- Any structural change MUST land in both files in the same PR, and
-- tests/schema_parity.rs MUST remain green (normalize_type collapses
-- INTEGER + BIGINT to INT64).
--
-- Idempotency: sqlx records applied migrations in _sqlx_migrations and will
-- not re-run this file. Partial-crash recovery is handled by file 2's
-- WHERE job_run_number IS NULL guard (DB-10).

ALTER TABLE jobs ADD COLUMN next_run_number INTEGER NOT NULL DEFAULT 1;
ALTER TABLE job_runs ADD COLUMN job_run_number INTEGER;
-- job_runs.job_run_number stays nullable until file 3 (per DB-10 split migration).
```

**Adaptation notes:**
- **Single-step migration**, NOT the 3-file dance from Phase 11. D-13 justifies: new column is nullable end-to-end, no backfill needed (NULL is the correct initial state).
- File path: `migrations/sqlite/20260422_000004_enabled_override_add.up.sql` (filename preserves phase-11 `_add` convention).
- Body: ONE statement.
  ```sql
  -- Phase 14: jobs.enabled_override tri-state column (DB-14, ERG-04).
  --
  -- Nullable INTEGER: NULL = follow config `enabled` flag (no override);
  -- 0 = force disabled (written by POST /api/jobs/bulk-toggle with action=disable);
  -- 1 = force enabled (reserved — v1.1 UI never writes this; defensive rendering only).
  --
  -- Pairs with migrations/postgres/20260422_000004_enabled_override_add.up.sql.
  -- Any structural change MUST land in both files in the same PR;
  -- tests/schema_parity.rs normalize_type collapses INTEGER + BIGINT to INT64.
  --
  -- Idempotency: sqlx _sqlx_migrations tracking. No backfill needed —
  -- NULL is the correct initial state for every existing row (D-13).

  ALTER TABLE jobs ADD COLUMN enabled_override INTEGER;
  ```
- `INTEGER` (NOT `INTEGER NULL` — SQLite treats absent-NOT-NULL as NULL-allowed; same convention as the analog L17 `ALTER TABLE job_runs ADD COLUMN job_run_number INTEGER;`).
- No index (D-13a — jobs table is tiny).

---

### 17. `migrations/postgres/20260422_000004_enabled_override_add.up.sql` (NEW)

**Role / data flow:** forward-only Postgres migration (ALTER TABLE).

**Closest analog:** `migrations/postgres/20260416_000001_job_run_number_add.up.sql` (whole file, verbatim, L1-14):
```sql
-- Phase 11: per-job run numbering (DB-09, DB-10, DB-11) — PostgreSQL.
--
-- File 1 of 3. Adds nullable job_runs.job_run_number plus the NOT NULL
-- jobs.next_run_number counter (DEFAULT 1). Files 2 and 3 backfill existing
-- rows and tighten to NOT NULL respectively.
--
-- Pairs with migrations/sqlite/20260416_000001_job_run_number_add.up.sql.
-- BIGINT here matches SQLite's INTEGER under the INT64 normalization rule
-- in tests/schema_parity.rs. Any structural change MUST land in both files
-- in the same PR.

ALTER TABLE jobs ADD COLUMN IF NOT EXISTS next_run_number BIGINT NOT NULL DEFAULT 1;
ALTER TABLE job_runs ADD COLUMN IF NOT EXISTS job_run_number BIGINT;
-- job_runs.job_run_number stays nullable until file 3 (per DB-10 split migration).
```

**Adaptation notes:**
- `BIGINT` for Postgres (matches the existing `enabled` column handling — SQLite INTEGER ↔ Postgres BIGINT both normalize to INT64 in `tests/schema_parity.rs`).
- `ADD COLUMN IF NOT EXISTS` — the Postgres analog uses this for idempotency (SQLite doesn't support `IF NOT EXISTS` on ADD COLUMN; the sqlx migration tracker handles it on SQLite).
- Body:
  ```sql
  -- Phase 14: jobs.enabled_override tri-state column (DB-14, ERG-04).
  --
  -- Nullable BIGINT: matches SQLite INTEGER under the INT64 normalization rule
  -- in tests/schema_parity.rs. NULL = follow config; 0 = force disabled;
  -- 1 = force enabled (reserved — v1.1 UI never writes this).
  --
  -- Pairs with migrations/sqlite/20260422_000004_enabled_override_add.up.sql.
  -- Any structural change MUST land in both files in the same PR.

  ALTER TABLE jobs ADD COLUMN IF NOT EXISTS enabled_override BIGINT;
  ```
- No DEFAULT clause (NULL is the implicit default for nullable-without-default columns).
- No `.down.sql` pair (Research Open Question 4 recommends forward-only; matches Phase 11/10 convention).

---

### 18. `tests/v11_bulk_toggle.rs` (NEW)

**Role / data flow:** integration test covering the T-V11-BULK-01 invariants + ERG-01..04 handler behaviors.

**Closest analogs (TWO complementary templates):**
1. `tests/stop_handler.rs` (whole file, L1-175+) — axum test server harness with mock scheduler mpsc.
2. `tests/dashboard_jobs_pg.rs` (whole file, L1-65) — testcontainers Postgres parity fixture.

**Harness pattern — verbatim from `tests/stop_handler.rs` L55-94:**
```rust
async fn build_app_with_scheduler_reply(reply: StopResult) -> (Router, DbPool, i64) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    let run_id = seed_running_run(&pool, "stop-handler-test").await;

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);

    // Mock scheduler: for every SchedulerCmd::Stop that arrives, reply with
    // the canned `reply` value. Other variants are ignored (tests only
    // dispatch Stop commands).
    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            if let SchedulerCmd::Stop { response_tx, .. } = cmd {
                let _ = response_tx.send(reply);
            }
        }
    });

    let state = AppState {
        started_at: chrono::Utc::now(),
        version: "test",
        pool: pool.clone(),
        cmd_tx,
        config_path: std::path::PathBuf::from("/tmp/cronduit-test.toml"),
        tz: chrono_tz::UTC,
        last_reload: Arc::new(Mutex::new(None::<ReloadState>)),
        watch_config: false,
        metrics_handle: setup_metrics(),
        active_runs: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    let router = Router::new()
        .route("/api/runs/{run_id}/stop", post(stop_run))
        .with_state(state);

    (router, pool, run_id)
}
```

**Request-construction + assertion pattern — verbatim from `tests/stop_handler.rs` L96-155:**
```rust
fn build_stop_request(run_id: i64, cookie_token: &str, form_token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/api/runs/{}/stop", run_id))
        .header("cookie", format!("{}={}", CSRF_COOKIE_NAME, cookie_token))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(format!("csrf_token={}", form_token)))
        .expect("build request")
}

#[tokio::test]
async fn stop_run_happy_path() {
    let (app, _pool, run_id) = build_app_with_scheduler_reply(StopResult::Stopped).await;
    let response = app.oneshot(build_stop_request(run_id, TEST_CSRF, TEST_CSRF)).await.expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK, "happy path must return 200");
    let headers = response.headers();
    let hx_trigger = headers.get("HX-Trigger").expect("HX-Trigger header must be present")
        .to_str().expect("HX-Trigger header must be valid UTF-8");
    assert!(hx_trigger.contains("showToast"), "HX-Trigger must carry a showToast event");
    assert!(hx_trigger.contains("\"level\":\"info\""));
}
```

**Postgres fixture pattern — verbatim from `tests/dashboard_jobs_pg.rs` L16-45:**
```rust
#[tokio::test]
async fn get_dashboard_jobs_postgres_smoke() {
    let container = Postgres::default().start().await.expect("start postgres container");
    let host = container.get_host().await.expect("container host");
    let port = container.get_host_port_ipv4(5432).await.expect("container port");
    let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");
    let pool = DbPool::connect(&url).await.expect("DbPool::connect");
    assert_eq!(pool.backend(), DbBackend::Postgres);
    pool.migrate().await.expect("run migrations");

    // Seed one enabled job via the production upsert path.
    let _job_id = queries::upsert_job(
        &pool, "dash-pg-smoke", "*/5 * * * *", "*/5 * * * *",
        "command", r#"{"command":"echo hi"}"#, "hash-dash-pg", 3600,
    ).await.expect("upsert job");

    // ... test body ...
    pool.close().await;
}
```

**Adaptation notes (test file `tests/v11_bulk_toggle.rs`):**
- File naming: `v11_*.rs` convention (all Phase 11 tests use it; Phase 14 is closing out the v1.1 milestone → same prefix).
- Imports: copy verbatim from `tests/stop_handler.rs` L14-27; swap `stop_run` for `bulk_toggle`, swap `SchedulerCmd::Stop` matcher for `SchedulerCmd::Reload`.
- Mock scheduler: Research §6 mandates Reload fires AFTER UPDATE. Mock loop:
  ```rust
  while let Some(cmd) = cmd_rx.recv().await {
      if let SchedulerCmd::Reload { response_tx } = cmd {
          let _ = response_tx.send(ReloadResult { status: ReloadStatus::Ok, ... });
      }
  }
  ```
- Body-construction: form-urlencoded with repeated `job_ids=`:
  ```rust
  Body::from(format!("csrf_token={csrf}&action=disable&job_ids={a}&job_ids={b}", ...))
  ```
- Test cases per Research §Validation Architecture (13+ cases):
  - `upsert_invariant` — seed + bulk_set_override + upsert_job + assert override preserved.
  - `reload_invariant` — sync_config_to_db with job still present; assert override preserved.
  - `disable_missing_clears_override` — remove job from config names; assert both columns updated.
  - `dashboard_filter` — 3 jobs (2 override=0, 1 NULL); `get_enabled_jobs` returns only the NULL one.
  - `handler_csrf` — mismatched token → 403 + DB untouched.
  - `handler_disable` — `action=disable` sets override=0 for all ids.
  - `handler_enable` — `action=enable` sets override=NULL.
  - `handler_partial_invalid` — 2 valid + 1 invalid id → 200 + toast "(1 not found)".
  - `handler_dedupes_ids` — duplicate ids in body → single UPDATE per id.
  - `handler_rejects_empty` — empty job_ids → 400 + error toast.
  - `handler_accepts_repeated_job_ids` — confirms `axum_extra::Form` deserializes `job_ids=1&job_ids=2&job_ids=3` → `Vec<i64>` (Landmine §1 regression guard).
  - `handler_fires_reload_after_update` — assert mpsc received Reload AND DB state matches.
  - `get_overridden_jobs_alphabetical` — seed 3 jobs with overrides; assert alphabetical name order.
  - `settings_empty_state_hides_section` — askama render of settings page with empty `overridden_jobs` asserts the `<section>` is absent.
- Postgres parity: either fold `#[cfg(feature = "pg-integration")]`-gated tests into the same file, or create `tests/v11_bulk_toggle_pg.rs` mirroring `dashboard_jobs_pg.rs` L16-64. Precedent: `tests/dashboard_jobs_pg.rs` is standalone.

---

### 19. `THREAT_MODEL.md` (MOD)

**Role / data flow:** prose documentation of threat posture.

**Closest analog:** existing Stop-button bullet at L113 (verbatim):
```markdown
**Stop button (v1.1+ blast radius):** The Stop button added in v1.1 lets anyone with Web UI access terminate any running job via `POST /api/runs/{id}/stop`. This widens the blast radius of an unauthenticated UI compromise — previously an attacker could trigger or view runs, now they can also interrupt them mid-execution. The mitigation posture is unchanged from the rest of the v1 Web UI: keep Cronduit on loopback or front it with a reverse proxy that enforces authentication. Web UI authentication (including differentiated Stop authorization) is deferred to v2 (AUTH-01 / AUTH-02).
```

**Section heading:** under `## Threat Model 2: Untrusted Client` → `### Residual Risk` (the Stop-button bullet is appended to this subsection at L113; Phase 14 parallel bullet goes immediately after it).

**Adaptation notes:**
- Append ONE new bold-prefixed paragraph after the Stop-button paragraph at L113. Research §Security Domain provides the suggested wording (verbatim — planner may refine):
  ```markdown
  **Bulk toggle (v1.1 blast radius):** The bulk-toggle endpoint added in v1.1 lets anyone with Web UI access disable every configured job in a single `POST /api/jobs/bulk-toggle` request. This further widens the blast radius of an unauthenticated UI compromise — an attacker can now silently stop the entire schedule without terminating any running execution. Running jobs are NOT terminated by bulk disable (D-02 / ERG-02), so an in-flight attacker-triggered run continues to completion even after all jobs are bulk-disabled. Mitigation posture is identical to the rest of the v1 Web UI: loopback default or reverse-proxy auth. Bulk-action authorization (including a per-action confirmation step) is deferred to v2 (AUTH-01 / AUTH-02).
  ```
- Exact wording is the planner's call; the invariant is: **one paragraph, parallel structure to the Stop bullet, same AUTH-01/02 deferral reference**.

---

### 20. `justfile` (MOD — TWO new recipes)

**Role / data flow:** build/dev/release recipes.

**Closest analog A:** `docker-compose-up` at L269-272 (verbatim):
```make
# Bring up the full compose stack from examples/
[group('dev')]
docker-compose-up:
    docker compose -f examples/docker-compose.yml up
```

**Closest analog B:** `release` at L23-28 (verbatim — shape for version-scoped release recipes):
```make
# The actual image build and push happens in CI via docker/build-push-action@v6.
[group('meta')]
[doc('Tag and push a release. Usage: just release 1.0.0')]
release version:
    @echo "Creating release v{{version}}..."
    git tag -a "v{{version}}" -m "Release v{{version}}"
    git push origin "v{{version}}"
    @echo "Release v{{version}} tagged and pushed. CI will build and publish."
```

**Closest analog C:** `dev` at L245-250 (verbatim — shape for multi-line shell recipe):
```make
# Single-process dev loop (readable text logs, trace level for cronduit)
[group('dev')]
dev:
    RUST_LOG=debug,cronduit=trace cargo run -- run \
        --config examples/cronduit.toml \
        --log-format text
```

**Adaptation notes:**
- Add a new `[group('release')]` section or extend `[group('dev')]`. Research §Missing-Just-Recipes suggests a dedicated `release` group.
- Recipe 1 — `compose-up-rc3`:
  ```make
  # Bring up the full compose stack pinned to the v1.1.0-rc.3 image for
  # HUMAN-UAT validation. Phase 14 D-17 / feedback_uat_use_just_commands.
  [group('release')]
  [doc('Bring up the compose stack pinned to v1.1.0-rc.3 for HUMAN-UAT')]
  compose-up-rc3:
      CRONDUIT_IMAGE=ghcr.io/simplicityguy/cronduit:1.1.0-rc.3 \
      docker compose -f examples/docker-compose.yml up -d
  ```
  **Precondition (Research Landmine §13):** `examples/docker-compose.yml` must honor `${CRONDUIT_IMAGE:-...}`. Verify pre-plan via `grep 'image:' examples/docker-compose.yml`; if hard-coded, the planner adds a one-line env-var support change in the same PR.
- Recipe 2 — `reload`:
  ```make
  # Trigger a config reload of the running cronduit by SIGHUP.
  # HUMAN-UAT Step 4 + 7 per D-17.
  [group('release')]
  [doc('Send SIGHUP to the running cronduit process (config reload)')]
  reload:
      #!/usr/bin/env bash
      set -euo pipefail
      if docker ps --format '{{.Names}}' | grep -q '^cronduit$'; then
          docker kill -s HUP cronduit
          echo "SIGHUP sent to cronduit container"
      else
          pkill -HUP cronduit && echo "SIGHUP sent to cronduit process" \
              || { echo "no running cronduit found"; exit 1; }
      fi
  ```
- Justfile-specific gotcha: `{{.Names}}` inside the bash script clashes with just's `{{ ... }}` template syntax — inside the `#!/usr/bin/env bash` block just is content-verbatim so the shell-quoting issue is handled by just itself; verify with `just --list` after the change.
- Both recipes honor `feedback_uat_use_just_commands.md` (every UAT step maps to a real `just` recipe).

---

### 21. `cliff.toml` (UNCHANGED — reference only)

**Role / data flow:** git-cliff release-notes generator config.

**No edits.** D-15 (rc.3 delta) and D-19 (v1.1.0 cumulative) both rely on default grouping. Same stance as Phase 12 D-12 and Phase 13 D-23.

**Adaptation notes (for planner awareness only):**
- The `commit_preprocessors` at `cliff.toml:35-38` already rewrite `Phase N:` squash-merge titles to `feat: Phase N:` so they appear in cumulative v1.1.0 notes.
- If the rc.3 or v1.1.0 cumulative output reads awkwardly (Research Claude's Discretion), plan a FOLLOW-UP PR to `cliff.toml`, NOT a hand-edit of the GitHub Release body.

---

### 22. `MILESTONES.md` + `README.md` (MOD on final-promotion commit)

**Role / data flow:** prose documentation.

**Closest analog:** existing v1.0 entry in `MILESTONES.md` (not read here — follow its shape exactly per D-20).

**Adaptation notes:**
- `MILESTONES.md` gets a new v1.1 entry following the shape of the v1.0 entry: title, ship date, one-paragraph summary, pointers to `.planning/milestones/v1.1-ROADMAP.md` + `.planning/milestones/v1.1-REQUIREMENTS.md` + `.planning/milestones/v1.1-MILESTONE-AUDIT.md` (latter three created by `/gsd-complete-milestone`, not by this phase).
- `README.md` "Current State" paragraph updates: v1.1.0 is current stable (was v1.0.1).
- Both edits land on the final-promotion commit (D-20), NOT on the Phase 14 feature commit. Research Caveat: D-16 prefers the MILESTONES.md update as a FOLLOW-UP commit on main AFTER the v1.1.0 tag is pushed (so the tagged commit remains bit-identical to rc.3).

---

## Shared Patterns

### Authentication (CSRF — ALL mutation handlers)

**Source:** `src/web/handlers/api.rs::run_now` L32-40 (run_now), L135-143 (reload), L250-258 (reroll), L380-388 (stop_run) — four verbatim copies.

**Apply to:** `bulk_toggle` (Pattern §1).

**Verbatim idiom:**
```rust
let cookie_token = cookies
    .get(csrf::CSRF_COOKIE_NAME)
    .map(|c| c.value().to_string())
    .unwrap_or_default();

if !csrf::validate_csrf(&cookie_token, &form.csrf_token) {
    return (StatusCode::FORBIDDEN, "CSRF token mismatch").into_response();
}
```

Every mutation handler in the project MUST open with this block BEFORE touching the DB. The outer `csrf::ensure_csrf_cookie` middleware (`src/web/mod.rs:87`) guarantees the cookie is present on every response; the form field must match it.

---

### Error Handling (scheduler channel closed)

**Source:** `stop_run` L451-462 (verbatim):
```rust
Err(_) => (
    StatusCode::SERVICE_UNAVAILABLE,
    "Scheduler is shutting down",
)
    .into_response(),
```

**Apply to:** `bulk_toggle` on `cmd_tx.send(Reload)` failure. UI-SPEC Copywriting § specifies the user-facing toast variant: `"Scheduler is shutting down — try again shortly."` with `level=error`, `duration=0` (sticky).

---

### Validation (CSRF form + typed extractor)

**Source:** `CsrfForm` struct at `api.rs:21-24`:
```rust
#[derive(Deserialize)]
pub struct CsrfForm {
    csrf_token: String,
}
```

**Apply to:** Phase 14's `BulkToggleForm` (Pattern §8). Same derive, crate-visible fields, no `pub` on fields. Key addition: `#[serde(default)]` on the `Vec<i64>` field (Landmine §9).

---

### SQL Backend Split (SQLite `?N` vs Postgres `$N` / `ANY`)

**Source:** `disable_missing_jobs` L129-169 (already excerpted).

**Apply to:** `bulk_set_override` (Pattern §2). **Every new multi-row write query MUST follow this split** — it's the only supported cross-backend "update a list of rows" shape.

---

### HX-Trigger Toast Envelope (ALL user-facing mutation feedback)

**Source:** `run_now` L87-91, `reload` L218-222, `reroll` L302-306, `stop_run` L429-433. Canonical shape:
```rust
let event = HxEvent::new_with_data(
    "showToast",
    json!({"message": format!("..."), "level": "info"}),
)
.expect("toast event serialization");
// ...
(HxResponseTrigger::normal([event]), headers, StatusCode::OK).into_response()
```

**Apply to:** `bulk_toggle` (Pattern §1). Copy verbatim — only the `message` + `level` + `duration` values change per UI-SPEC Copywriting § Toast Copy table.

---

### Schema Parity (SQLite INTEGER ↔ Postgres BIGINT = INT64)

**Source:** `tests/schema_parity.rs` normalize_type rule (not read here) + every existing migration pair.

**Apply to:** `migrations/sqlite/20260422_000004_enabled_override_add.up.sql` uses `INTEGER`; `migrations/postgres/20260422_000004_enabled_override_add.up.sql` uses `BIGINT`. Both normalize to INT64 → `tests/schema_parity.rs` stays green.

Same discipline drives the `SqliteDbJobRow.enabled_override: Option<i32>` (fits SQLite INTEGER) ↔ `PgDbJobRow.enabled_override: Option<i64>` (fits Postgres BIGINT) split; converter widens to `Option<i64>` on the shared `DbJob`.

---

### Askama View-Model Threading

**Source:** `dashboard.rs` — `DashboardJobView` struct at L66-87 + askama template at `dashboard.html` with compile-time field access (e.g., `{{ job.enabled_override }}` would build-error if the field is missing).

**Apply to:** every Phase 14 view-model change. The compiler + askama build-pass are your checklist: if you add a template reference to `{{ overridden_jobs }}` without adding the Rust field, the build fails. Use this as a forcing function.

---

## No Analog Found

None. Every new Phase 14 file has at least one strong in-tree template.

---

## Metadata

**Analog search scope:** `src/web/handlers/`, `src/db/`, `templates/`, `migrations/sqlite/`, `migrations/postgres/`, `tests/`, `assets/src/`, `justfile`, `THREAT_MODEL.md`.
**Files scanned:** 40+ (templates, handlers, queries, migrations, tests, CSS, justfile, threat model).
**Pattern extraction date:** 2026-04-22.
**Line-number verification:** all line numbers in this document were read against HEAD on the `gsd/quick-260421-nn3-fix-dashboard-jobs-postgres` branch (clean working tree).

**Key pattern invariants carried to planner:**
1. **CSRF is always first** in a mutation handler (5 existing handlers + 1 new).
2. **Writer-pool + SQLite `?N` placeholder build + Postgres `ANY($N)` array bind** is the ONLY supported cross-backend "update list of rows" shape.
3. **`axum_extra::extract::Form`** for any handler needing `Vec<T>` from repeated keys (CORRECTS CONTEXT D-11).
4. **`hx-preserve="true"` requires a stable `id` attribute** — `id="cd-row-cb-{{ job.id }}"`.
5. **Sticky action-bar must be SIBLING (not child) of `<div class="overflow-x-auto">`.**
6. **`upsert_job` is FROZEN** — T-V11-BULK-01 asserts no touch to the INSERT columns or ON CONFLICT SET.
7. **Migration is single-file per backend** (D-13 — nullable column, no backfill, no 3-step dance).
8. **Additive CSS only** — zero existing selectors or tokens modified.
9. **No hand-edit of release notes** — `git-cliff` is authoritative (D-15, D-19).
10. **MILESTONES.md + README.md updates land on the final-promotion commit (or post-tag follow-up)** — NOT on the Phase 14 feature commit.
