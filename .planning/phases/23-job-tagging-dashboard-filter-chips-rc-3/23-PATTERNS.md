# Phase 23: Job Tagging Dashboard Filter Chips — rc.3 — Pattern Map

**Mapped:** 2026-05-04
**Files analyzed:** 11 (8 modified Rust/HTML/CSS/justfile + 3 new artifacts)
**Analogs found:** 10 / 11 (one site — `hx-swap-oob` — has no in-tree precedent and inherits its contract from HTMX 2.0 docs + UI-SPEC § Component Inventory)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/db/queries.rs` (`DashboardJob` struct widening at L590) | model (DB row) | CRUD-read | `DbRunDetail.tags` field at `queries.rs:659-664` (Phase 22 added) | exact (sibling struct, identical Phase 22 idiom) |
| `src/db/queries.rs` (`get_dashboard_jobs` SELECT widening at L818-942) | service (DB read) | request-response (parameterized SELECT, parity-pair) | self-extension; `j.tags AS tags_json` projection at `queries.rs:1412, 1421` (P22 in `get_run_by_id`) | exact (P22 added the column to a sibling read) |
| `src/db/queries.rs` (`get_dashboard_jobs` WHERE composition at L818-942) | service (DB read) | request-response (variadic AND-chain) | self (existing `has_filter` branching at L837-865; format-string `ORDER BY` whitelist at L825-835) | role-match (extends the same parity-pair function with a variadic predicate set) |
| `src/web/handlers/dashboard.rs::DashboardParams` (L23-31, field add) | model (HTTP query params) | URL→struct deserialize | sibling fields `filter`/`sort`/`order` on the same struct + `#[serde(default = "...")]` idiom at L25-30 | exact (self-extension) |
| `src/web/handlers/dashboard.rs::dashboard` (L242 extractor swap + L254-337 handler body) | controller (HTTP handler) | request-response (HTMX or full page) | sparkline hydration at `dashboard.rs:262-337` (P13 OBS-03 — handler-side aggregation pattern) | exact (P13 fold pattern; D-08 explicitly mirrors this) |
| `src/web/handlers/dashboard.rs::DashboardPage`/`JobTablePartial` (L46-60, view-model widening) | model (template view) | server-render | Phase 14 `is_disabled: bool` field on `DashboardJobView` (`dashboard.rs:76`) | exact (sibling-template pattern) |
| `templates/pages/dashboard.html` (chip strip insert above L19; sort-header widen at L88-128; poll `hx-include` widen at L138-141) | component (askama template) | server-render + HTMX URL state | `cd-bulk-action-bar` `hidden`-until-relevant at `dashboard.html:46`; sort-header anchors at L88-128; existing 3s poll at L136-141 | exact (D-02 mirrors L46; D-13 widens L88-128; D-12 widens L140) |
| `assets/src/app.css` (`cd-tag-chip-*` family added in `@layer components`) | utility (stylesheet) | static-css | P21 `cd-fctx-*` block at `app.css:557-575`; P21 `cd-exit-*` block at `app.css:577-605`; reduced-motion block at `app.css:431-434`; print block at `app.css:607-610` | exact (D-04 explicitly mirrors P21's cd-fctx/cd-exit namespacing precedent) |
| `tests/v12_tags_dashboard.rs` (NEW) | test (integration) | request-response | `tests/dashboard_render.rs` (full router + oneshot + body substring asserts); `tests/v12_tags_validators.rs` (P22 v12_tags_* family naming + harness shape) | exact (composite — router/oneshot from `dashboard_render.rs`, file-naming family from `v12_tags_validators.rs`) |
| `justfile` (3 new `uat-chips-*` recipes in `[group('uat')]`) | task runner | shell-orchestration | `justfile:1182-1224` (`uat-tags-persist`); `justfile:1234-1370` (`uat-tags-validators`); `justfile:1370-1428` (`uat-tags-webhook`) — P22 `uat-tags-*` family | exact (D-17 explicitly mirrors P22 family) |
| `23-RC3-PREFLIGHT.md` (NEW) | docs (release runbook) | maintainer-checklist | `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-RC2-PREFLIGHT.md` (entire 206-line file) | exact (D-15 explicitly mirrors P21 D-22..D-26 verbatim; substitution rc.2→rc.3, P21→P23, FCTX→tag-chips) |
| `23-HUMAN-UAT.md` (NEW) | docs (UAT plan) | maintainer-checklist | `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-HUMAN-UAT.md` | exact (autonomous=false maintainer plan precedent) |

## Pattern Assignments

### `src/db/queries.rs` — `DashboardJob.tags` field add (modify L590-604, model)

**Analog:** `DbRunDetail.tags` field added by Phase 22 at `src/db/queries.rs:659-664`.

**Existing pattern (verbatim from `queries.rs:659-664`):**

```rust
/// Phase 22 TAG-01 / WH-09: tags from the joined `jobs.tags` JSON column,
/// deserialized to `Vec<String>` at the row-mapping site. Empty Vec when
/// the job has no tags (column is NOT NULL DEFAULT '[]'); never None
/// — schema guarantees a value. Sorted-canonical order per the upsert
/// path's `serde_json::to_string` of a sorted+deduped Vec.
pub tags: Vec<String>,
```

**Phase 23 minimal delta:**

- Append to `DashboardJob` after the existing `enabled_override: Option<i64>` at `queries.rs:603`:
  ```rust
  /// Phase 23 TAG-06: tags from the joined `jobs.tags` JSON column,
  /// deserialized to `Vec<String>` at the row-mapping site. Empty Vec when
  /// the job has no tags (column is NOT NULL DEFAULT '[]' — schema guarantees
  /// a value). Sorted-canonical order per Phase 22 D-09. Consumed by the
  /// dashboard chip strip fleet-tag fold (D-08) and the chip strip render
  /// (D-01..D-04 + UI-SPEC § Component Inventory).
  pub tags: Vec<String>,
  ```
- NOT `Option<Vec<String>>` — the column is `NOT NULL DEFAULT '[]'`, so the read site always produces a (possibly empty) Vec. Distinguishes from `enabled_override` which is genuinely nullable.

---

### `src/db/queries.rs` — `get_dashboard_jobs` SELECT widening (modify L818-942, service)

**Analog:** P22's `get_run_by_id` SELECT projection at `queries.rs:1412, 1421` (already projects `j.tags AS tags_json`) + P22's row-map at L1448-1456, L1479-1483 (already deserializes JSON).

**Existing P22 SELECT projection pattern (verbatim from `queries.rs:1408-1422`):**

```rust
let sql_sqlite = r#"
    SELECT r.id, r.job_id, r.job_run_number, j.name AS job_name, r.status, r.trigger,
           r.start_time, r.end_time, r.duration_ms, r.exit_code, r.error_message,
           r.image_digest, r.config_hash, r.scheduled_for,
           j.tags AS tags_json
    FROM job_runs r
    JOIN jobs j ON j.id = r.job_id
    WHERE r.id = ?1
"#;
```

**Existing P22 row-map JSON-deserialize pattern (verbatim from `queries.rs:1448-1456`):**

```rust
tags: {
    // Phase 22 TAG-01: forgiving on corrupt JSON — column is NOT NULL
    // DEFAULT '[]' so corruption is structurally impossible from
    // cronduit-controlled writes; if a future writer ever bugs and
    // stores invalid JSON, fall back to Vec::new() rather than
    // panicking and breaking webhook delivery.
    let s: String = r.get("tags_json");
    serde_json::from_str(&s).unwrap_or_default()
},
```

**Existing `get_dashboard_jobs` SELECT shape (verbatim from `queries.rs:840-848`):**

```rust
format!(
    r#"SELECT j.id, j.name, j.schedule, j.resolved_schedule, j.job_type, j.timeout_secs, j.enabled_override,
              lr.status AS last_status, lr.start_time AS last_run_time, lr.trigger AS last_trigger
       FROM jobs j
       LEFT JOIN ( ... ) lr ON lr.job_id = j.id AND lr.rn = 1
       WHERE j.enabled = 1 AND LOWER(j.name) LIKE ?1
       {order_clause}"#
)
```

**Existing `get_dashboard_jobs` row-map (verbatim from `queries.rs:875-889`):**

```rust
Ok(rows
    .into_iter()
    .map(|r| DashboardJob {
        id: r.get("id"),
        name: r.get("name"),
        schedule: r.get("schedule"),
        resolved_schedule: r.get("resolved_schedule"),
        job_type: r.get("job_type"),
        timeout_secs: r.get("timeout_secs"),
        last_status: r.get("last_status"),
        last_run_time: r.get("last_run_time"),
        last_trigger: r.get("last_trigger"),
        enabled_override: r.try_get("enabled_override").ok().flatten(),
    })
    .collect())
```

**Phase 23 minimal delta:**

- BOTH SQLite (L840-851) and Postgres (L894-905) SELECT lists: append `j.tags AS tags_json` after `j.enabled_override`. Identical column name across both backends; `tests/schema_parity.rs::normalize_type` already absorbs `TEXT NOT NULL DEFAULT '[]'` into the TEXT family.
- BOTH row-map arms (L877-889 sqlite; L928-940 postgres): append after `enabled_override`:
  ```rust
  tags: {
      // Phase 23 TAG-06: forgiving on corrupt JSON — see Phase 22's
      // get_run_by_id row-map for the same pattern. Column is NOT NULL
      // DEFAULT '[]' so corruption is structurally impossible.
      let s: String = r.get("tags_json");
      serde_json::from_str(&s).unwrap_or_default()
  },
  ```
- Edit-pair invariant: SQLite + Postgres branches must change in the same commit. Postgres SELECT uses `$1` placeholders — already in place; we only add the projected column.

---

### `src/db/queries.rs` — `get_dashboard_jobs` WHERE / variadic AND-chain (modify L818-942, service)

**Analog:** the existing `has_filter` branching at `queries.rs:837-865` + the format-string `ORDER BY` whitelist at `queries.rs:825-835` (server-controlled SQL composition; never user-input string interpolation).

**Existing format-string ORDER BY whitelist (verbatim from `queries.rs:825-835`):**

```rust
// Build ORDER BY from whitelist — never interpolate user input into SQL.
let order_clause = match (sort, order) {
    ("name", "desc") => "ORDER BY j.name DESC",
    ("name", _) => "ORDER BY j.name ASC",
    ("last_run", "desc") => "ORDER BY lr.start_time DESC NULLS LAST",
    ("last_run", _) => "ORDER BY lr.start_time ASC NULLS LAST",
    ("status", "desc") => "ORDER BY lr.status DESC NULLS LAST",
    ("status", _) => "ORDER BY lr.status ASC NULLS LAST",
    ("next_run", _) => "ORDER BY j.name ASC", // placeholder
    (_, "desc") => "ORDER BY j.name DESC",
    _ => "ORDER BY j.name ASC",
};
```

**Existing has_filter branching + bind site (verbatim from `queries.rs:837, 867-873`):**

```rust
let has_filter = filter.is_some_and(|f| !f.is_empty());
// ...
match pool.reader() {
    PoolRef::Sqlite(p) => {
        let rows = if has_filter {
            let pattern = format!("%{}%", filter.unwrap().to_lowercase());
            sqlx::query(&base_sql).bind(pattern).fetch_all(p).await?
        } else {
            sqlx::query(&base_sql).fetch_all(p).await?
        };
```

**Phase 23 minimal delta (D-09 — implements the AND-chain on top of the existing has_filter branch):**

- Signature: append `active_tags: &[String]` argument after `order: &str` (server-controlled — handler has already filtered against the fleet-tag fold before calling, per UI-SPEC § Stale-tag URL handling + RESEARCH § Pattern 3).
- Compute the bind offset ONCE at the top of the function:
  ```rust
  let has_filter = filter.is_some_and(|f| !f.is_empty());
  let tag_bind_start = if has_filter { 2 } else { 1 };
  ```
- Build the variadic predicate string from the count (count, not values — values bind separately):
  ```rust
  // SQLite uses ?N; Postgres uses $N — branch the placeholder shape after
  // the has_filter / pool.reader() switch, the same way the existing
  // sqlite/postgres SELECT bodies branch.
  let tag_predicates_sqlite: String = (0..active_tags.len())
      .map(|i| format!("AND tags LIKE ?{}", tag_bind_start + i))
      .collect::<Vec<_>>()
      .join(" ");
  let tag_predicates_postgres: String = (0..active_tags.len())
      .map(|i| format!("AND tags LIKE ${}", tag_bind_start + i))
      .collect::<Vec<_>>()
      .join(" ");
  let untagged_clause = if !active_tags.is_empty() { "AND tags != '[]'" } else { "" };
  ```
- Splice both fragments into the existing `WHERE j.enabled = 1 [AND LOWER(j.name) LIKE ?1] {tag_predicates} {untagged_clause} {order_clause}` shape inside both backend arms (SQLite at L840-851, Postgres at L894-905). The `format!()` already handles named substitution.
- Bind sequence after the existing name filter (verbatim shape): for each tag in `active_tags`, `q = q.bind(format!(r#"%"{}"%"#, tag))` — the `%"…"%` LIKE pattern ensures structural matching against the JSON-quoted element. P22 D-03 substring-collision validator at config-load already prevents `back`/`backup` ambiguity, so the LIKE is safe.
- Caller-side update: `src/web/handlers/dashboard.rs:250` (only call site of `get_dashboard_jobs`) — pass `&active_tags` as the new fifth arg.

**Confidence note:** The variadic predicate count (`active_tags.len()`) comes from a server-controlled set (the active set has already been intersected with `fleet_tags` per RESEARCH Pattern 3 / UI-SPEC stale-tag handling). NO user-controlled SQL string interpolation. Bind values use the parameterized `?N`/`$N` form per existing convention.

---

### `src/web/handlers/dashboard.rs::DashboardParams` (L23-31, model)

**Analog:** sibling fields `filter`/`sort`/`order` on the same struct (`dashboard.rs:23-31`).

**Existing pattern (verbatim from `dashboard.rs:23-31`):**

```rust
#[derive(Debug, Deserialize, Default)]
pub struct DashboardParams {
    #[serde(default)]
    pub filter: String,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default = "default_order")]
    pub order: String,
}
```

**Phase 23 minimal delta (D-10):**

- Append after `order`:
  ```rust
  /// Phase 23 TAG-06: zero-or-more active tag filters from `?tag=foo&tag=bar`.
  /// Renamed from `tag` (singular URL key, plural Rust field) so the URL form
  /// reads `?tag=backup&tag=weekly` per TAG-06 lock. Deserialized via
  /// `axum_extra::extract::Query<DashboardParams>` (uses `serde_html_form`
  /// under the hood — supports repeated keys). `axum::extract::Query` would
  /// silently collapse duplicates to one — that is the exact failure mode
  /// TAG-06 forbids. NEVER trust this field for SQL composition without
  /// first intersecting with the fleet-tag fold (handler enforces this
  /// per UI-SPEC § Stale-tag URL handling / silent server-side drop).
  #[serde(default, rename = "tag")]
  pub tags: Vec<String>,
  ```
- Imports: change `use axum::extract::{Query, State};` (L5) to `use axum::extract::State;` and add `use axum_extra::extract::Query;` (axum-extra is already in tree per Cargo.toml L120-123 with `query` feature).
- Extractor swap at L242: the `Query(params): Query<DashboardParams>,` line is unchanged textually — only the import points to `axum_extra::extract::Query` now.

---

### `src/web/handlers/dashboard.rs::dashboard` body (modify L239-360, controller)

**Analog:** the sparkline hydration loop at `dashboard.rs:262-337` (P13 OBS-03 — handler-side aggregation pattern; D-08 explicitly mirrors this).

**Existing hydration shape (verbatim from `dashboard.rs:255-337`, abbreviated):**

```rust
let mut job_views: Vec<DashboardJobView> = jobs.into_iter().map(|j| to_view(j, tz)).collect();

// Phase 13 OBS-03: hydrate 20-cell sparkline + success-rate badge. Single
// query covers every job; bucket rows by job_id, reverse per-job, pad with
// empty cells on the left ...
let spark_rows = queries::get_dashboard_job_sparks(&state.pool)
    .await
    .unwrap_or_default();

let mut spark_by_job: HashMap<i64, Vec<DashboardSparkRow>> = HashMap::new();
for row in spark_rows {
    spark_by_job.entry(row.job_id).or_default().push(row);
}

// ...for each job_view: bucket -> compute -> annotate...
```

**Existing handler branch on `is_htmx` (verbatim from `dashboard.rs:341-359`):**

```rust
if is_htmx {
    JobTablePartial {
        jobs: job_views,
        csrf_token,
    }
    .into_web_template()
    .into_response()
} else {
    DashboardPage {
        jobs: job_views,
        filter: params.filter,
        sort: params.sort,
        order: params.order,
        config_path: state.config_path.display().to_string(),
        csrf_token,
    }
    .into_web_template()
    .into_response()
}
```

**Phase 23 minimal delta (D-08 + UI-SPEC § Component Inventory):**

- Add `use std::collections::BTreeSet;` to imports.
- After the existing `let mut job_views: Vec<DashboardJobView> = ...` (L255), BEFORE the sparkline hydration block (L262), insert the fleet-tag fold + active-tag filter. Use `DashboardJob.tags` directly (NOT `DashboardJobView.tags` — the view is a render-time projection; the fold operates on the row data). Refactor: capture the raw `Vec<DashboardJob>` BEFORE the `into_iter().map(to_view)` consumes it, OR re-project tags onto the view (planner picks; the view-projection is cleaner and matches `is_disabled` precedent at `dashboard.rs:115` `to_view`).
- Suggested concrete shape (mirrors RESEARCH § Pattern 3):
  ```rust
  // Phase 23 D-08: fleet-tag fold (BTreeSet -> Vec preserves alphabetical sort).
  // Disabled jobs are excluded by `WHERE j.enabled = 1` upstream — fleet_tags
  // is "tags from the rendered row set" (CONTEXT § Claude's Discretion default).
  let fleet_tags: Vec<String> = jobs
      .iter()
      .flat_map(|j| j.tags.iter().cloned())
      .collect::<BTreeSet<String>>()
      .into_iter()
      .collect();

  // Active-tag set: dedup + canonicalize alphabetical (UI-SPEC URL canonicalization)
  // + intersect with fleet so stale URL tags are silently dropped (UI-SPEC
  // § Stale-tag URL handling).
  let mut active_tags: Vec<String> = params.tags.clone();
  active_tags.sort();
  active_tags.dedup();
  active_tags.retain(|t| fleet_tags.contains(t));
  ```
- Pass `&active_tags` into the `get_dashboard_jobs` call at L250 (the SQL widening from the previous section consumes it).
- **Re-fetch ordering caveat (call out in PLAN.md):** The fold needs `jobs` BEFORE filtering. Either (a) fetch jobs once with `active_tags = &[]` to compute the fleet, then re-fetch with the filter applied — TWO queries; OR (b) run the SQL filter on the first fetch and accept that `fleet_tags` reflects the filtered subset (operators see only chips for tags currently in the visible set — confusing UX when narrowing). **Recommended: (a)** — two cheap reads per dashboard request. Document the choice. Discretion item per CONTEXT § Claude's Discretion ("disabled-job tags" + "Whether the active-tag URL is canonicalized"). Planner picks.
- Both template structs (DashboardPage at L46-53, JobTablePartial at L55-60) gain `fleet_tags: Vec<String>` and `active_tags: Vec<String>` fields. JobTablePartial gains them too because the OOB swap response renders the chip strip alongside the table body (D-11).
- The handler's `if is_htmx` branch at L341-359 stays structurally identical; both arms now receive `fleet_tags` + `active_tags` in the constructor.

---

### `src/web/handlers/dashboard.rs::DashboardJobView` (L66-92, model — optional widening)

**Analog:** Phase 14 `is_disabled: bool` field at `dashboard.rs:76` (per-row data added to view-model after introduction in `to_view` — sibling extension precedent).

**Existing extension pattern (verbatim from `dashboard.rs:72-76`):**

```rust
/// Phase 14 — true when `enabled_override == Some(0)`. Drives the inline
/// DISABLED badge on the name column and the em-dash in `next_fire` so
/// operators see a coherent "this job will NOT fire" signal on the
/// dashboard, not only on `/settings` (Phase 14 UAT rc.4 gap).
pub is_disabled: bool,
```

**Phase 23 minimal delta (per CONTEXT § canonical_refs L223; planner discretion):**

- The fold consumes `DashboardJob.tags` directly and does NOT require `DashboardJobView.tags`. UI-SPEC ships zero per-row tag rendering in v1.2 (the chip strip is fleet-level; rows do not show tag chips per CONTEXT deferred ideas — "Tag chips on `/jobs/{id}` (job detail) page — deferred").
- **Recommendation:** SKIP adding `tags: Vec<String>` to `DashboardJobView` — keep view-model lean. The fold runs on `Vec<DashboardJob>` (raw rows) before `to_view` consumes them.

---

### `templates/pages/dashboard.html` — chip strip insert (NEW block above L19)

**Analog:** `cd-bulk-action-bar` `hidden`-until-relevant pattern at `dashboard.html:46-72`.

**Existing hidden-until-relevant pattern (verbatim from `dashboard.html:46-47`):**

```html
<!-- Sticky bulk-action bar (ERG-01 D-02); hidden until at least one row checkbox is ticked. -->
<div id="cd-bulk-action-bar" class="cd-bulk-bar" hidden>
  <span class="cd-bulk-bar-count"><strong id="cd-bulk-count">0</strong> selected</span>
```

**Phase 23 minimal delta (D-01 + D-02 + UI-SPEC § Component Inventory § 1):**

- Insert immediately AFTER the dashboard `<h1>` row at `dashboard.html:5-7`, BEFORE the existing filter row at L19 (per D-01: dedicated row above the name-filter input):
  ```html
  <!-- Phase 23 TAG-06: dashboard tag filter chip strip (UI-SPEC § Component Inventory).
       Hidden via HTML5 `hidden` attribute when fleet has zero tagged jobs (D-02 —
       mirrors cd-bulk-action-bar at L46). flex-wrap reflows on narrow viewports
       (D-03). Class namespace cd-tag-chip-* (D-04). -->
  <div id="cd-tag-chip-strip"
       class="cd-tag-chip-strip"
       {% if fleet_tags.is_empty() %}hidden{% endif %}
       role="group"
       aria-label="Filter jobs by tag">
    {% for tag in fleet_tags %}
    <a class="cd-tag-chip {% if active_tags.contains(tag) %}cd-tag-chip--active{% else %}cd-tag-chip--inactive{% endif %}"
       href="?{{ chip_href_for(tag) }}"
       hx-get="?{{ chip_href_for(tag) }}"
       hx-target="#job-table-body"
       hx-push-url="true"
       {% if active_tags.contains(tag) %}aria-pressed="true"{% else %}aria-pressed="false"{% endif %}
       aria-label="Tag filter: {{ tag }}{% if active_tags.contains(tag) %} (active — click to remove){% else %} (inactive — click to add){% endif %}">
      {{ tag }}
    </a>
    {% endfor %}

    {% for tag in active_tags %}
    <input type="hidden" name="tag" value="{{ tag }}">
    {% endfor %}
  </div>
  ```
- `chip_href_for(tag)` is a server-side template helper (askama macro/filter — planner picks). It emits the post-toggle URL query string with `filter`, `sort`, `order` re-emitted from current params plus the new active-tag set. UI-SPEC § Component Inventory codifies the contract.
- **NB on OOB:** UI-SPEC § Component Inventory § OOB swap contract specifies that the OOB `hx-swap-oob="true"` lives on the OUTER wrapper `<div id="cd-tag-chip-strip">` in the **HTMX response body**, NOT in this base template. The base template renders the chip strip without `hx-swap-oob`; the partial-response template (see "OOB swap response composition" below) wraps the same markup with `hx-swap-oob="true"` for the response body.

---

### `templates/pages/dashboard.html` — sort-header href widening (modify L88-128)

**Analog:** four existing sortable column anchors at L88-128 (Name / Next Fire / Status / Last Run).

**Existing pattern (verbatim from `dashboard.html:90-96`, Name column):**

```html
<a href="?filter={{ filter }}&sort=name&order={% if sort == "name" && order == "asc" %}desc{% else %}asc{% endif %}"
   class="no-underline hover:text-(--cd-text-accent){% if sort == "name" %} text-(--cd-text-accent){% else %} text-(--cd-text-secondary){% endif %}"
   hx-get="/partials/job-table?filter={{ filter }}&sort=name&order={% if sort == "name" && order == "asc" %}desc{% else %}asc{% endif %}"
   hx-target="#job-table-body"
   hx-push-url="true">
  Name{% if sort == "name" %}{% if order == "asc" %} &#9650;{% else %} &#9660;{% endif %}{% endif %}
</a>
```

**Phase 23 minimal delta (D-13):**

- Both `href` and `hx-get` attributes on each of the four sort anchors (L90, L92, L102, L104, L112, L114, L122, L124) gain a trailing active-tag suffix. The suffix repeats `&tag={{ t|urlencode }}` for every active tag.
- **Inline option (planner discretion per CONTEXT):**
  ```html
  href="?filter={{ filter }}&sort=name&order={% if ... %}...{% endif %}{% for t in active_tags %}&tag={{ t|urlencode }}{% endfor %}"
  ```
- **Macro/filter option (preferred per CONTEXT § Specifics § Sort-header readability):**
  ```html
  href="?{{ build_sort_href("name", filter, sort, order, active_tags) }}"
  ```
  with the helper defined as an askama macro in a partials file or a custom filter registered on the templating engine. The four anchors then collapse to single-attribute calls.
- `tag|urlencode`: askama 0.15 ships `urlencode` filter out of the box (RESEARCH § Pattern 5). P22 charset regex `^[a-z0-9][a-z0-9_-]{0,30}$` (RESEARCH-cited) precludes structural escape (no `&`, `=`, `?`, `#`, `<`, `>`, `&`, `'`, `"` chars), so the urlencode is defense-in-depth.

---

### `templates/pages/dashboard.html` — 3s poll `hx-include` widening (modify L138-141)

**Analog:** existing `<tbody id="job-table-body" ... hx-include="...">` block at L136-141.

**Existing pattern (verbatim from `dashboard.html:136-141`):**

```html
<tbody id="job-table-body"
       hx-get="/partials/job-table"
       hx-trigger="every 3s"
       hx-swap="innerHTML"
       hx-include="[name='filter'],[name='sort'],[name='order']">
  {% include "partials/job_table.html" %}
</tbody>
```

**Phase 23 minimal delta (D-12):**

- Single-character widening at L140:
  ```html
  hx-include="[name='filter'],[name='sort'],[name='order'],[name='tag']"
  ```
- The matching hidden inputs `<input type="hidden" name="tag" value="X">` are rendered inside `#cd-tag-chip-strip` per the chip strip insert above. The `[name='tag']` selector is a sibling-finder, not scoped to the chip strip — but the only place `name='tag'` appears in the document is inside the chip strip, so collisions are structurally impossible.

---

### `src/web/handlers/dashboard.rs` — OOB swap response composition (NEW pattern in this codebase)

**Analog:** NONE in the codebase — `grep -rn "hx-swap-oob" templates/` returns empty (RESEARCH § Section 4 confirmed). The contract is inherited verbatim from HTMX 2.0 docs (https://htmx.org/attributes/hx-swap-oob/) + UI-SPEC § Component Inventory § OOB swap contract.

**UI-SPEC contract (verbatim from `23-UI-SPEC.md` § Component Inventory § OOB swap):**

> partial responses for chip toggles render `<div id="cd-tag-chip-strip" hx-swap-oob="true">…</div>` immediately followed by the table-body markup, in the same response body, in that order.

**Phase 23 minimal delta:**

- Either:
  - **Option A — single composite partial template.** Extend `templates/partials/job_table.html` to render the OOB chip strip wrapper FIRST (only when the partial is the HTMX response body — gate via a `{% if include_oob_chip_strip %}` template variable that the handler sets to `true`), then the existing `<tr>` rows. Pass new template fields `fleet_tags` + `active_tags` + `include_oob_chip_strip: bool`. This is the cleanest single-template approach.
  - **Option B — two partials concatenated by the handler.** Add `templates/partials/chip_strip_oob.html` (renders the chip strip with `hx-swap-oob="true"` on the wrapper div). The handler in the `if is_htmx` arm renders both partials and concatenates the response bodies. axum/askama supports this via the standard `Response::builder().body(Body::from(format!("{}{}", oob_partial, table_partial)))` shape.
- **Recommendation:** Option A — keeps the partial response shape encapsulated in one askama template; matches how `JobTablePartial` already drives the partial render path.
- The `hx-swap-oob="true"` attribute lives on the outer `<div id="cd-tag-chip-strip">` ONLY, not on each chip anchor. RESEARCH § Pattern 4 explicitly calls out the failure mode of putting OOB on each chip.

---

### `assets/src/app.css` — `cd-tag-chip-*` family addition (modify `@layer components`)

**Analog:** P21 `cd-fctx-*` block at `app.css:557-575` and P21 `cd-exit-*` block at `app.css:577-605`. D-04 explicitly mirrors P21 namespacing precedent.

**Existing P21 `@layer components` pattern (verbatim from `app.css:558-563`):**

```css
.cd-fctx-panel { background: var(--cd-bg-surface); border: 1px solid var(--cd-border); border-radius: 8px; overflow: hidden; }
.cd-fctx-summary { display: flex; align-items: center; gap: var(--cd-space-3); padding: var(--cd-space-4) var(--cd-space-6); background: var(--cd-bg-surface-raised); cursor: pointer; font-size: var(--cd-text-xl); font-weight: 700; letter-spacing: -0.02em; list-style: none; user-select: none; }
.cd-fctx-summary::-webkit-details-marker { display: none; }
.cd-fctx-summary:hover { background: var(--cd-bg-hover); }
.cd-fctx-summary:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--cd-green-dim); border-color: var(--cd-border-focus); }
```

**Existing reduced-motion pattern (verbatim from `app.css:431-434`):**

```css
@media (prefers-reduced-motion: reduce) {
  .cd-timeline-bar--pulsing { animation: none; opacity: 1; }
  .cd-fctx-summary-caret { transition: none; }
}
```

**Existing print-mode pattern (verbatim from `app.css:607-610`):**

```css
@media print {
  details.cd-fctx-panel { open: open; }
}
```

**Phase 23 minimal delta (UI-SPEC § Component Inventory § CSS contract):**

- Add a new block after the P21 `cd-exit-*` block (i.e., after L605, inside the same `@layer components { ... }` block that closes at L611):
  ```css
  /* === Phase 23 tag filter chip strip (UI-SPEC § Component Inventory) === */
  .cd-tag-chip-strip { display: flex; flex-wrap: wrap; gap: var(--cd-space-2); align-items: center; margin-bottom: var(--cd-space-4); }
  .cd-tag-chip-strip[hidden] { display: none; }
  .cd-tag-chip { display: inline-flex; align-items: center; min-height: var(--cd-space-10); padding: var(--cd-space-2) var(--cd-space-3); border-radius: var(--cd-radius-full); font-size: var(--cd-text-sm); font-family: var(--font-mono); letter-spacing: 0; text-decoration: none; cursor: pointer; transition: background 0.1s ease, border-color 0.1s ease, color 0.1s ease; user-select: none; }
  .cd-tag-chip--inactive { background: var(--cd-bg-surface-raised); border: 1px solid var(--cd-border-subtle); color: var(--cd-text-secondary); font-weight: 400; }
  .cd-tag-chip--inactive:hover { background: var(--cd-bg-hover); border-color: var(--cd-border); color: var(--cd-text-primary); }
  .cd-tag-chip--inactive:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--cd-green-dim); background: var(--cd-bg-hover); border-color: var(--cd-border); color: var(--cd-text-primary); }
  .cd-tag-chip--active { background: var(--cd-green-dim); border: 1px solid var(--cd-text-accent); color: var(--cd-text-accent); font-weight: 700; }
  .cd-tag-chip--active:hover { filter: brightness(1.1); }
  .cd-tag-chip--active:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--cd-green-dim); border-color: var(--cd-text-accent); }
  ```
- EXTEND the existing `@media (prefers-reduced-motion: reduce)` block at L431-434:
  ```css
  @media (prefers-reduced-motion: reduce) {
    .cd-timeline-bar--pulsing { animation: none; opacity: 1; }
    .cd-fctx-summary-caret { transition: none; }
    .cd-tag-chip { transition: none; }   /* Phase 23 */
  }
  ```
- EXTEND the existing `@media print` block at L607-610:
  ```css
  @media print {
    details.cd-fctx-panel { open: open; }
    .cd-tag-chip-strip { display: none; }   /* Phase 23 */
  }
  ```
- **Zero new tokens** — every value resolves to existing `--cd-*` custom property per UI-SPEC § Tokens — Existing Reuse Verified.

---

### `tests/v12_tags_dashboard.rs` (NEW, integration test)

**Analog:** composite — full router/oneshot harness from `tests/dashboard_render.rs:26-180` + file-naming family + tempfile validate-then-router shape from `tests/v12_tags_validators.rs:41-80`.

**Imports + harness pattern (verbatim from `tests/dashboard_render.rs:1-52`):**

```rust
use std::sync::{Arc, Mutex};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tower::ServiceExt; // brings .oneshot()

use cronduit::db::DbPool;
use cronduit::db::queries;
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::telemetry::setup_metrics;
use cronduit::web::{AppState, ReloadState, router};

async fn build_test_app() -> (axum::Router, DbPool) {
    let pool = DbPool::connect("sqlite::memory:").await.expect("...");
    pool.migrate().await.expect("run migrations");
    // ... AppState construction ...
    (router(state), pool)
}

async fn seed_job(pool: &DbPool, name: &str, schedule: &str) -> i64 {
    queries::upsert_job(
        pool, name, schedule, schedule, "command", "{}", "deadbeef", 300, "[]",
    )
    .await
    .expect("upsert job")
}
```

**Existing oneshot + body-substring assert pattern (verbatim from `tests/dashboard_render.rs:96-122`):**

```rust
let response = app
    .oneshot(
        Request::builder()
            .method("GET")
            .uri("/")
            .body(Body::empty())
            .expect("build request"),
    )
    .await
    .expect("oneshot");

assert_eq!(response.status(), StatusCode::OK, "GET / must return 200");

let bytes = to_bytes(response.into_body(), usize::MAX).await.expect("body");
let body = std::str::from_utf8(&bytes).expect("utf-8");

assert!(body.contains("alpha-backup"), "must contain alpha-backup");
```

**Phase 23 minimal delta (D-17):**

- File header doc-comment per `v12_tags_validators.rs:1-21` shape (cite Phase 23 / TAG-06..08 + RESEARCH §, give run command).
- Reuse `build_test_app` + `seed_job` exactly. **Note:** existing `seed_job` already passes `"[]"` as the `tags_json` arg to `upsert_job` (post-P22). Add a parallel `seed_job_with_tags(pool, name, schedule, tags: &[&str]) -> i64` helper that builds the JSON tag string via `serde_json::to_string(&Vec::from_iter(tags.iter().map(|s| s.to_string())))`.
- Test cases (one per acceptance criterion from CONTEXT § §1-7):
  - `chip_strip_renders_distinct_fleet_tags_alphabetical` — seed three jobs with `["weekly","backup"]`, `["backup","prod"]`, `[]` → GET / → body contains all three distinct chips in alphabetical order; the empty-tag job's row is still rendered.
  - `chip_strip_hidden_when_no_jobs_have_tags` — seed two jobs with `[]` each → GET / → body contains `<div id="cd-tag-chip-strip" ... hidden>` (or `display:none` via `hidden` attribute + CSS).
  - `and_filter_sql_intersects_active_tags` — seed jobs A=["backup","weekly"], B=["backup"], C=["weekly"] → GET `/?tag=backup&tag=weekly` → body contains A, NOT B, NOT C.
  - `untagged_jobs_hidden_when_filter_active_TAG07` — seed jobs A=["backup"], B=[] → GET `/?tag=backup` → body contains A, NOT B.
  - `chip_filter_composes_with_name_filter_via_AND` — seed jobs `prod-backup`=["backup"], `dev-backup`=["backup"], `prod-cleanup`=["backup"] → GET `/?filter=prod&tag=backup` → body contains `prod-backup` + `prod-cleanup`, NOT `dev-backup`.
  - `repeated_tag_url_param_parses_to_vec_TAG06` — GET `/?tag=foo&tag=bar` → handler receives `Vec::from(["foo","bar"])` (assert via body content reflecting two active chips). This locks the `axum_extra::Query` swap.
  - `sort_header_href_round_trips_active_tags_D13` — GET `/?tag=backup&sort=name&order=desc` → body's Name sort anchor `href` contains `&tag=backup`.
  - `htmx_partial_response_contains_oob_chip_strip_D11` — GET `/?tag=backup` with `HX-Request: true` header → body contains `id="cd-tag-chip-strip"` AND `hx-swap-oob="true"` AND the table rows.
  - `stale_tag_in_url_silently_dropped` — seed job A=["backup"] → GET `/?tag=backup&tag=ghost` → body renders only the `backup` chip; `ghost` does not appear; A is rendered (not filtered out by the unknown tag).
  - `active_tags_canonicalized_alphabetical_in_chip_hrefs` — GET `/?tag=zebra&tag=alpha` → chip hrefs serialize the active set as `tag=alpha&tag=zebra` (canonical sort).

---

### `justfile` — three new `uat-chips-*` recipes

**Analog:** P22 `uat-tags-*` family at `justfile:1182-1428` (`uat-tags-persist`, `uat-tags-validators`, `uat-tags-webhook`). D-17 explicitly mirrors this family.

**Existing pattern (verbatim from `justfile:1180-1224`, abbreviated):**

```just
[group('uat')]
[doc('Phase 22 — TAG-02 persistence spot-check (operator validates jobs.tags column shape)')]
uat-tags-persist:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▶ Phase 22 UAT: TAG-02 persistence spot-check"
    echo ""
    echo "Step 1: Build cronduit."
    just build
    echo ""
    echo "Step 2: Reset dev DB (so the upsert path runs cleanly)."
    just db-reset
    # ...
    cat > .tmp/uat-tags-persist.toml <<'TOML_EOF'
    [server]
    bind = "127.0.0.1:8080"
    timezone = "UTC"

    [[jobs]]
    name = "uat-tags-persist-demo"
    schedule = "*/5 * * * *"
    command = "true"
    tags = ["weekly", "backup", "prod"]
    TOML_EOF
    just check-config .tmp/uat-tags-persist.toml
    # ... operator-driven steps with read prompts ...
    echo "Maintainer: ... Claude does NOT mark this passed."
```

**Phase 23 minimal delta (D-17 — three recipes per CONTEXT § decisions D-17):**

- `uat-chips-render` — seed a multi-tag fleet (3-5 distinct tags spread across 4-6 jobs, NO substring-collisions) → start cronduit → walk operator to dashboard → confirm chip strip renders with every distinct tag, alphabetical, then test the empty-fleet hidden case by reverting to a no-tags TOML.
- `uat-chips-and-filter` — multi-tag fleet → walk operator to toggle two chips → confirm AND semantics (only jobs with both tags appear) + untagged-hidden + composes with name-filter via AND (TAG-07 verification).
- `uat-chips-share-url` — multi-tag fleet → operator pastes a shareable URL `http://127.0.0.1:8080/?tag=backup&tag=weekly` directly into a fresh tab → confirm chips render in active state on page load + URL push round-trips on toggle (active set changes update the URL bar).
- All three end with the ritual `Maintainer: confirm ... Claude does NOT mark this passed.` per memory feedback.
- All recipes use `just check-config` + `just build` + `just db-reset` + (in another terminal) `cargo run -- run --config .tmp/...toml` per the `uat-tags-persist` recipe-calls-recipe pattern.

---

### `23-RC3-PREFLIGHT.md` (NEW, autonomous=false, maintainer plan)

**Analog:** `21-RC2-PREFLIGHT.md` (entire 206-line file, including frontmatter). D-15 explicitly mirrors this verbatim modulo substitution rc.2→rc.3, P21→P23, FCTX→tag-chips.

**Existing frontmatter pattern (verbatim from `21-RC2-PREFLIGHT.md:1-9`):**

```yaml
---
phase: 21
plan: 11
type: rc-preflight
autonomous: false
rc_tag: v1.2.0-rc.2
created: 2026-05-02
status: pending-maintainer-execution
---
```

**Existing tag-command pattern (verbatim from `21-RC2-PREFLIGHT.md:120-133`):**

```bash
git checkout main
git pull --ff-only origin main
git tag -a -s v1.2.0-rc.2 -m "v1.2.0-rc.2 — FCTX UI panel + exit-code histogram (P21)"
git push origin v1.2.0-rc.2
```

**Existing `:latest` invariant verification block (verbatim from `21-RC2-PREFLIGHT.md:142-155`):**

```bash
LATEST_DIGEST=$(docker manifest inspect ghcr.io/simplicityguy/cronduit:latest | sha256sum | awk '{print $1}')
V1_1_0_DIGEST=$(docker manifest inspect ghcr.io/simplicityguy/cronduit:1.1.0 | sha256sum | awk '{print $1}')
[[ "$LATEST_DIGEST" == "$V1_1_0_DIGEST" ]] && echo "OK: :latest invariant verified" || echo "FAIL: :latest was promoted to rc.2"
```

**Phase 23 minimal delta (D-15 + D-16):**

- Frontmatter substitutions: `phase: 23`, `plan: NN` (planner-determined; final wave plan number), `rc_tag: v1.2.0-rc.3`, `created: 2026-05-04`.
- Section headers: `Phase 21` → `Phase 23`; `v1.2.0-rc.2` → `v1.2.0-rc.3`; `FCTX UI panel + exit-code histogram` → `dashboard tag filter chips`.
- Section 1 (PR-merged checklist) — replace P21 plans 01-10 with P23 plans 01-NN.
- Section 5 (cardinality discipline verification) — replace EXIT-06 grep with the analogous Phase 23 invariant: confirm tags do NOT appear as a Prometheus label per CONTEXT deferred ideas:
  ```bash
  grep -rn 'tags' src/telemetry.rs                              # MUST return empty (or no Prometheus label use)
  grep -rn 'cronduit_runs_total.*tags' src/                     # MUST return empty
  ```
- Section 8 (tag command, copy-paste verbatim per D-15):
  ```bash
  git tag -a -s v1.2.0-rc.3 -m "v1.2.0-rc.3 — dashboard tag filter chips (P23)"
  ```
- Section 9 (post-publish verification) — substitute `rc.2` → `rc.3` in every digest-comparison reference; the `:latest` invariant assertion still references `cronduit:1.1.0` (NOT `1.2.0`) per D-15 (latest stays at 1.1.0).
- Out-of-scope section — keep verbatim per D-16: NO modifications to `release.yml` / `cliff.toml` / `docs/release-rc.md`.
- Cross-reference footer: `21-RC2-PREFLIGHT.md` → `20-RC1-PREFLIGHT.md → 21-RC2-PREFLIGHT.md` chain extended with `23-RC3-PREFLIGHT.md`.

---

### `23-HUMAN-UAT.md` (NEW, autonomous=false maintainer plan)

**Analog:** `21-HUMAN-UAT.md` (16,445 bytes, autonomous=false, maintainer-validated UAT scenarios).

**Phase 23 minimal delta (D-17):**

- Frontmatter `autonomous: false`, `phase: 23`.
- Scenarios (per CONTEXT § decisions D-17):
  - The 3 `uat-chips-*` recipes invoked end-to-end.
  - Mobile viewport (chip strip wraps below 640px).
  - Light-mode rendering (theme switch — chip strip honors `[data-theme="light"]` automatically per UI-SPEC).
  - Keyboard navigation (Tab onto chips; Enter/Space toggles).
  - Screen-reader narration of active state (`aria-pressed` + `aria-label` per UI-SPEC § Accessibility Contract).
  - End-to-end with v1.0 name-filter combined with active chips (TAG-07 verification).
- Sign-off block per project memory `feedback_uat_user_validates.md`: maintainer fills the table; Claude does NOT mark passed.

---

## Shared Patterns

### sqlx parity-pair widening (SQLite + Postgres in lockstep)
**Source:** `src/db/queries.rs:62-138` (`upsert_job` — P22 widening), `src/db/queries.rs:1389-1487` (`get_run_by_id` — P22 row-map), the existing `get_dashboard_jobs` itself at L818-942.
**Apply to:** `get_dashboard_jobs` SELECT + WHERE widening.

- Both backend arms widen identically; only differ in `?N` vs `$N` placeholder syntax and `excluded`/`EXCLUDED` capitalization.
- The `match pool.reader() { PoolRef::Sqlite(p) => ..., PoolRef::Postgres(p) => ... }` shape is the existing pattern; add the new column projection + AND-chain composition + bind sequence in BOTH arms.
- `tests/schema_parity.rs::normalize_type` already collapses TEXT-family types — `j.tags AS tags_json` passes parity automatically (P22 verified).

### Server-controlled SQL composition (whitelist + format-string + ?N binds)
**Source:** `src/db/queries.rs:825-835` (existing ORDER BY whitelist) + `src/db/queries.rs:837-873` (existing has_filter branching).
**Apply to:** the variadic `AND tags LIKE ?N` chain.

- Predicate count comes from a SERVER-controlled set (`active_tags.len()` after the handler intersects with the fleet-tag fold). NEVER user-controlled.
- Each tag value flows through `bind()` — the format string is parameter-only.
- P22 substring-collision validator at config-load (`check_tag_substring_collision`) prevents `back`/`backup` LIKE ambiguity at the runtime layer.

### Handler-side aggregation over fetched rows (P13/P21 OBS-* precedent)
**Source:** `src/web/handlers/dashboard.rs:262-337` (sparkline hydration from raw `Vec<DashboardSparkRow>`).
**Apply to:** the fleet-tag fold + active-tag intersection.

- Pattern: handler runs ONE DB read, then `BTreeSet`/`HashMap` folds in Rust to produce derived view-model fields.
- Cheap at homelab scale (sub-millisecond for fleets up to thousands of jobs).
- Keeps queries.rs pure (returns rows, not aggregates).

### Hidden-until-relevant DOM block (P14 ERG-01 precedent)
**Source:** `templates/pages/dashboard.html:46` (`<div id="cd-bulk-action-bar" class="cd-bulk-bar" hidden>`).
**Apply to:** the chip strip empty-state (D-02).

- HTML5 `hidden` attribute handled at the askama template level via `{% if fleet_tags.is_empty() %}hidden{% endif %}`.
- CSS `[hidden]` rule in `cd-tag-chip-strip[hidden] { display: none; }` reinforces for older UAs that don't honor `hidden` in `display:flex` contexts (UI-SPEC § Component Inventory § CSS contract).

### `cd-*` namespacing in `@layer components` (P21 precedent)
**Source:** `assets/src/app.css:557-575` (`cd-fctx-*`); `app.css:577-605` (`cd-exit-*`).
**Apply to:** the new `cd-tag-chip-*` family.

- All new component CSS lives inside `@layer components { ... }` block at L187-611.
- One-line header comment per family: `/* === Phase X feature name === */`.
- Existing `@media (prefers-reduced-motion: reduce)` (L431) and `@media print` (L607-610) blocks are EXTENDED rather than duplicated.

### Reused token vocabulary (UI-SPEC § Tokens — Existing Reuse Verified)
**Source:** `assets/src/app.css` `:root` token declarations (L1-186) + `[data-theme="light"]` block.
**Apply to:** every chip CSS value.

- Tokens consumed: `--cd-bg-primary`, `--cd-bg-surface`, `--cd-bg-surface-raised`, `--cd-bg-hover`, `--cd-border`, `--cd-border-subtle`, `--cd-text-primary`, `--cd-text-secondary`, `--cd-text-accent`, `--cd-text-sm`, `--cd-space-1..--cd-space-10`, `--cd-radius-full`, `--cd-green-dim`, `--font-mono`.
- Zero new tokens introduced (UI-SPEC § Tokens lock).
- Light-mode mirroring is automatic via `[data-theme="light"]` block — zero new light-mode work.

### Test-file naming family `tests/v12_<feature>_<scenario>.rs`
**Source:** `tests/v12_labels_merge.rs`, `tests/v12_fctx_panel.rs`, `tests/v12_tags_validators.rs`, etc.
**Apply to:** `tests/v12_tags_dashboard.rs`.

- Family stays consistent through v1.2.
- Header doc-comment names the requirement IDs covered + gives the cargo test command.

### Recipe-calls-recipe `just` family (P22 `uat-tags-*` precedent)
**Source:** `justfile:1182-1428` (`uat-tags-persist`, `uat-tags-validators`, `uat-tags-webhook`).
**Apply to:** the three new `uat-chips-*` recipes.

- Each recipe is in `[group('uat')]` with `[doc(...)]` annotations.
- Recipes call `just build`, `just check-config`, `just db-reset`, sometimes `just uat-webhook-mock`/`uat-webhook-fire`/`uat-webhook-verify` as composition primitives.
- Steps use `read` prompts to gate operator transitions between terminals.
- Every recipe ends with `Maintainer: ... Claude does NOT mark this passed.` (project memory `feedback_uat_user_validates.md`).

### rc-cut runbook reuse (P20 D-30 / P21 D-22..D-26 precedent)
**Source:** `.planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-RC1-PREFLIGHT.md`; `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-RC2-PREFLIGHT.md`; `docs/release-rc.md` (UNCHANGED).
**Apply to:** `23-RC3-PREFLIGHT.md`.

- Per D-15 / D-16: NO modifications to `release.yml`, `cliff.toml`, or `docs/release-rc.md` in this phase. Maintainer-discovered runbook gaps become hotfix PRs BEFORE tagging.
- The `:latest` digest invariant must equal the `:1.1.0` digest post-publish — same gate as P21.
- `git-cliff` output is authoritative for the GitHub Release body; no hand-edits post-publish.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| OOB swap response composition (lives in either `JobTablePartial` template or a new sibling partial) | template (composite partial) | server-render | `grep -rn "hx-swap-oob" templates/` empty (RESEARCH § Section 4 confirmed). Phase 23 introduces `hx-swap-oob` to the codebase for the first time. **Inheritance source:** HTMX 2.0 docs (https://htmx.org/attributes/hx-swap-oob/) + UI-SPEC § Component Inventory § OOB swap contract. **Failure mode to avoid:** putting `hx-swap-oob="true"` on each chip anchor instead of on the outer wrapper `<div id="cd-tag-chip-strip">` — RESEARCH § Pattern 4 calls this out explicitly. |

The OOB pattern is well-documented externally; UI-SPEC codifies the contract for future codebase reuse. Planner should call this out in the relevant PLAN.md so the executor reads UI-SPEC § Component Inventory § OOB swap contract (and HTMX 2.0 docs) before the template work.

---

## Metadata

**Analog search scope:**
- `src/db/queries.rs` — `DashboardJob` struct (L590-604), `get_dashboard_jobs` (L818-942), `DbRunDetail` + `get_run_by_id` (L631-665, L1390-1487) for P22 sibling row-map.
- `src/web/handlers/dashboard.rs` — `DashboardParams` (L23-31), `dashboard()` handler (L239-360), sparkline hydration loop (L262-337) for P13 OBS-03 fold precedent, `is_disabled` view-model precedent (L72-76).
- `templates/pages/dashboard.html` — entire 196-line file (filter row L19-36, bulk-action-bar L46-72, sort-header anchors L88-128, 3s poll tbody L136-141).
- `assets/src/app.css` — `@layer components` (L187-611), `cd-fctx-*` block (L557-575), `cd-exit-*` block (L577-605), reduced-motion block (L431-434), print block (L607-610).
- `tests/dashboard_render.rs` — full router/oneshot harness (L26-180).
- `tests/v12_tags_validators.rs` — P22 sibling test family naming + tempfile validate-then-router shape (L1-200).
- `justfile` — P22 `uat-tags-*` recipe family (L1182-1428).
- `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-RC2-PREFLIGHT.md` — entire 206-line file for `23-RC3-PREFLIGHT.md` template.
- `.planning/phases/22-job-tagging-schema-validators/22-PATTERNS.md` — analog reference for the schema layer Phase 23 reads from.

**Files scanned:** 9 source/template/CSS files + 3 in-tree test/justfile analogs + 2 cross-phase preflight templates = 14 in-tree analogs read; ranges non-overlapping per the no-re-read constraint.

**Pattern extraction date:** 2026-05-04
