# Phase 3: Read-Only Web UI & Health Endpoint - Research

**Researched:** 2026-04-10
**Domain:** Server-rendered web UI (askama + HTMX + Tailwind CSS), ANSI log rendering, scheduler command channel, health endpoint
**Confidence:** HIGH

## Summary

Phase 3 builds the complete read-only web dashboard on top of Phases 1-2's foundation. The stack is fully locked: `askama_web 0.15` templates with `axum-0.8` feature, HTMX 2.0 vendored locally, Tailwind CSS via standalone binary, and `rust-embed 8.11` for single-binary asset embedding. The existing `AppState`, `router()`, and `serve()` in `src/web/mod.rs` are minimal scaffolds ready for extension. The scheduler's `tokio::select!` loop in `src/scheduler/mod.rs` needs a new `mpsc::Receiver<SchedulerCmd>` branch for the "Run Now" command channel. The DB schema already has `jobs`, `job_runs`, and `job_logs` tables with the indexes needed for dashboard queries; new read queries (paginated runs, paginated logs, last-run-per-job) are the main DB additions.

The primary technical risks are: (1) ANSI-to-HTML conversion must HTML-escape first to prevent XSS -- the `ansi-to-html 0.2.2` crate does this correctly out of the box; (2) askama's default HTML escaping must never be bypassed via `|safe` on log content; (3) the CSRF double-submit cookie pattern needs careful implementation without a session store. All three are well-understood patterns with clear implementation paths.

**Primary recommendation:** Build templates first (base + pages + partials), then wire HTMX polling, then add the scheduler command channel for Run Now, then ANSI rendering + XSS tests, then the health endpoint. The design system CSS custom properties from `design/DESIGN_SYSTEM.md` should be set up in `assets/src/app.css` alongside Tailwind directives as a foundation step.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Dashboard uses a dense HTML table with columns: Name, Schedule (raw), Resolved Schedule, Next Fire (relative), Last Status (badge), Last Run (relative timestamp).
- **D-02:** Inline filter (text input, HTMX + debounce) and sort (clickable column headers) above the table. State preserved in URL query params.
- **D-03:** Table auto-refreshes every 3s via HTMX polling (`hx-trigger="every 3s"`). Next-fire countdown computed server-side only.
- **D-04:** Empty state when zero jobs: "No jobs configured yet" with config file path hint.
- **D-05:** Run Detail log viewer shows interleaved stdout/stderr chronologically. Stderr lines distinguished with left-border/background tint using `--cd-status-error-bg`.
- **D-06:** ANSI SGR codes parsed server-side in Rust into sanitized `<span>` tags with CSS classes. All other content HTML-escaped by default. No `| safe` / `PreEscaped` on raw log content.
- **D-07:** Log viewer paginated -- most recent 500 lines first with "Load older lines" button (HTMX with offset param).
- **D-08:** `tokio::sync::mpsc` command channel added to `SchedulerLoop`. Web handler sends `SchedulerCmd::RunNow { job_id }`. Scheduler receives in `select!` loop and spawns run with `trigger='manual'`.
- **D-09:** `SchedulerCmd` enum designed for extensibility (Phase 5 adds `Reload`).
- **D-10:** After Run Now click, POST response includes `HX-Trigger` header for toast. Next 3s poll picks up the new row. Uses `axum-htmx` for `HxTrigger` responder.
- **D-11:** Double-submit CSRF token. Random token in `HttpOnly; SameSite=Strict` cookie + hidden form field. Server validates match.
- **D-12:** Askama template inheritance: `base.html` with `{% block content %}`, pages extend it. Partials in `partials/` subdirectory.
- **D-13:** Both dark and light mode. Default follows `prefers-color-scheme`. Toggle in nav persists to `localStorage`. `data-theme` attribute on `<html>`. ~10 lines JS.
- **D-14:** Nav: Dashboard (home) + Settings. Job Detail and Run Detail are drill-downs.
- **D-15:** `just tailwind` recipe runs standalone binary. `build.rs` ensures CSS rebuilt before `cargo build`. `just dev` runs Tailwind `--watch` alongside `cargo-watch`.
- **D-16:** HTMX vendored into `assets/vendor/htmx.min.js`, embedded via `rust-embed`. Never CDN.
- **D-17:** `rust-embed` with `debug-embed = false` (default) reads from disk in debug builds.

### Claude's Discretion

- Exact ANSI parsing crate choice (as long as server-side, sanitized, no `| safe`)
- Toast notification implementation details
- Run history pagination page size (start: 25)
- Log viewer line count per page (start: 500)
- Status badge design (text + background tint vs icon + text)
- `build.rs` Tailwind invocation approach
- `AppState` extension fields
- Health endpoint extra fields beyond spec minimum
- Template file naming conventions
- CSS custom property naming

### Deferred Ideas (OUT OF SCOPE)

None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| UI-01 | Axum HTTP server serves embedded web UI; static assets via `rust-embed` | `rust-embed 8.11.0` with `#[folder]` attribute; `axum` `ServeDir` fallback via `tower-http` or embedded asset handler |
| UI-02 | HTML templating uses `askama_web` 0.15 with `axum-0.8` feature | Verified `askama_web 0.15.2` current; `askama 0.15.6` current; template inheritance via `{% extends %}` + `{% block %}` |
| UI-03 | Tailwind CSS built at compile time via standalone binary (no Node) | Standalone Tailwind binary via `just tailwind` recipe; `build.rs` integration for compile-time guarantee |
| UI-04 | HTMX vendored into `assets/vendor/htmx.min.js`; no CDN | HTMX 2.0.7 is latest release; vendor as static file embedded via `rust-embed` |
| UI-05 | All pages match `design/DESIGN_SYSTEM.md` terminal-green palette | Full CSS custom property system documented in DESIGN_SYSTEM.md section 6; dark/light via `data-theme` attribute |
| UI-06 | Dashboard lists all enabled jobs with name, schedule, resolved schedule, next fire, last status, last run | DB queries: join `jobs` with latest `job_runs` per job; `croner` for next-fire computation |
| UI-07 | Dashboard refreshes via HTMX polling every 3s on table partial | `hx-trigger="every 3s"` on partial endpoint; `hx-swap="innerHTML"` on table body |
| UI-08 | Job Detail shows resolved config, human-readable cron (croner), paginated run history | `croner::Cron` human-readable description API; paginated run history query with offset/limit |
| UI-09 | Run Detail shows metadata + logs with stdout/stderr distinction + ANSI color parsing | `ansi-to-html 0.2.2` for server-side ANSI-to-HTML; HTML-escapes input automatically |
| UI-10 | All log content HTML-escaped; ANSI parsing is only transformation; XSS CI test | Askama default escaping prevents XSS; `ansi-to-html` HTML-escapes before transforming; CI test for `<script>` in logs |
| UI-11 | Settings/Status page: uptime, DB status, config path, last reload, version | `AppState` fields: `started_at`, `version` (existing); add `config_path`, `db_pool` (for health check), `last_reload` |
| UI-12 | Run Now button triggers `POST /api/jobs/:id/run`; run recorded with `trigger='manual'` | `SchedulerCmd::RunNow` via `mpsc` channel; scheduler spawns run through normal pipeline |
| UI-13 | Dashboard filter (substring on name) and sort via query params | HTMX `hx-get` with query params + `hx-push-url="true"`; server-side filtering/sorting in SQL |
| UI-15 | State-changing endpoints require CSRF token | Double-submit cookie pattern; `rand` for token generation; `axum` cookie extraction + form field validation |
| OPS-01 | `GET /health` returns `{"status":"ok","db":"ok","scheduler":"running"}` | Simple axum handler; DB pool health via `sqlx::Pool::acquire()` test; scheduler status from `AppState` flag |
</phase_requirements>

## Standard Stack

### Core (Phase 3 additions to Cargo.toml)

| Library | Version | Purpose | Why Standard | Confidence |
|---------|---------|---------|--------------|------------|
| `askama` | 0.15.6 | Compile-time HTML templates | Jinja-like syntax, template inheritance, compile-time type checking. Post-Rinja merge. [VERIFIED: crates.io API] | HIGH |
| `askama_web` | 0.15.2 | Axum adapter for askama | Replaces deprecated `askama_axum`. Use `axum-0.8` feature. [VERIFIED: crates.io API] | HIGH |
| `rust-embed` | 8.11.0 | Embed static assets in binary | `debug-embed = false` enables disk reads in debug mode for hot reload. [VERIFIED: crates.io API] | HIGH |
| `axum-htmx` | 0.8.1 | HTMX request/response helpers | `HxRequest` extractor, `HxTrigger` responder for toast notifications. [VERIFIED: crates.io API] | HIGH |
| `axum-extra` | 0.12.5 | Query parsing, typed headers, cookies | `Query<T>` for filter/sort params, cookie extraction for CSRF. [VERIFIED: crates.io API] | HIGH |
| `ansi-to-html` | 0.2.2 | ANSI SGR to HTML conversion | HTML-escapes input first, then applies ANSI transformations. Produces safe `<span>` tags with CSS variables. [VERIFIED: docs.rs API docs] | HIGH |
| `rand` | 0.8.x | CSRF token generation | Already in Cargo.toml (used for `@random`). Generate 32-byte hex tokens. [ASSUMED] | HIGH |

### Vendored Assets

| Asset | Version | Source | Embed Method |
|-------|---------|--------|--------------|
| HTMX | 2.0.7 | GitHub release | `assets/vendor/htmx.min.js` via `rust-embed` [VERIFIED: GitHub API] |
| JetBrains Mono | v2.304 | GitHub release (WOFF2 subset) | `assets/static/fonts/` via `rust-embed` [VERIFIED: GitHub API] |
| Tailwind CSS | standalone binary | GitHub release | `bin/tailwindcss` (not embedded; build tool only) [VERIFIED: existing `just tailwind` recipe] |

### Existing Dependencies (already in Cargo.toml, used by Phase 3)

| Library | Version | Phase 3 Usage |
|---------|---------|---------------|
| `axum` | 0.8.8 | Route definitions, state extraction, JSON responses |
| `tower-http` | 0.6.8 | `TraceLayer` (existing), `ServeDir` or `CompressionLayer` if needed |
| `tokio` | 1.51 | `mpsc` channel for scheduler commands |
| `sqlx` | 0.8.6 | Read queries for dashboard, run history, log pagination |
| `chrono` | 0.4.44 | Timestamp formatting, relative time computation |
| `croner` | 3.0 | `next_after()` for dashboard "next fire" column, human-readable cron descriptions |
| `serde` | 1.0.228 | Deserialize query params for filter/sort |
| `serde_json` | 1 | Health endpoint JSON response |
| `uuid` | (add) | CSRF token generation alternative to raw `rand` [ASSUMED] |

**Installation (additions to Cargo.toml):**
```toml
# Templates
askama = { version = "0.15.6", features = ["with-axum-0.8"] }
askama_web = { version = "0.15.2", features = ["axum-0.8"] }

# Embedded assets
rust-embed = { version = "8.11.0" }

# HTMX helpers
axum-htmx = "0.8.1"

# Extra extractors (cookies, query)
axum-extra = { version = "0.12.5", features = ["cookie", "query"] }

# ANSI to HTML
ansi-to-html = "0.2.2"
```

## Architecture Patterns

### Recommended Project Structure

```
src/
  web/
    mod.rs              # AppState, router(), serve(), asset handler
    handlers/
      mod.rs
      dashboard.rs      # GET /, GET /partials/job-table
      job_detail.rs     # GET /jobs/:id, GET /partials/run-history/:id
      run_detail.rs     # GET /jobs/:job_id/runs/:run_id, GET /partials/log-viewer/:run_id
      settings.rs       # GET /settings
      api.rs            # POST /api/jobs/:id/run
      health.rs         # GET /health
    csrf.rs             # CSRF token generation + validation middleware
    ansi.rs             # ANSI-to-HTML conversion wrapper
  scheduler/
    cmd.rs              # SchedulerCmd enum (new file)
    mod.rs              # Add mpsc receiver to select! loop
  db/
    queries.rs          # Add read queries for UI

templates/
  base.html             # <html>, <head>, nav, footer, {% block content %}
  pages/
    dashboard.html      # extends base.html
    job_detail.html     # extends base.html
    run_detail.html     # extends base.html
    settings.html       # extends base.html
  partials/
    job_table.html      # HTMX-swappable table body
    run_history.html    # HTMX-swappable run list
    log_viewer.html     # HTMX-swappable log lines
    toast.html          # Toast notification element

assets/
  src/
    app.css             # @tailwind directives + CSS custom properties
  static/
    app.css             # Generated by Tailwind (gitignored)
    theme.js            # ~10 lines: dark/light toggle + localStorage
    fonts/
      JetBrainsMono-Regular.woff2
      JetBrainsMono-Bold.woff2
    favicons/           # Copied from design/favicons/
      favicon-16.svg
      favicon-32.svg
      favicon-192.svg
      favicon-512.svg
  vendor/
    htmx.min.js         # Vendored HTMX 2.0.7

tailwind.config.js      # Content: templates/**/*.html
```

### Pattern 1: AppState Extension

**What:** Extend the existing `AppState` struct with fields needed by Phase 3 handlers.
**When to use:** Every handler needs access to DB pool, scheduler command channel, config metadata.

```rust
// Source: existing src/web/mod.rs pattern, extended
use tokio::sync::mpsc;
use crate::scheduler::cmd::SchedulerCmd;
use crate::db::DbPool;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub version: &'static str,
    pub pool: DbPool,
    pub cmd_tx: mpsc::Sender<SchedulerCmd>,
    pub config_path: PathBuf,
    pub csrf_key: Arc<[u8; 32]>,  // HMAC key for CSRF tokens
}
```
[VERIFIED: existing AppState in src/web/mod.rs has started_at + version]

### Pattern 2: SchedulerCmd Channel

**What:** An `mpsc` channel bridging web handlers to the scheduler loop.
**When to use:** Run Now button and future Reload command.

```rust
// src/scheduler/cmd.rs (new file)
pub enum SchedulerCmd {
    RunNow { job_id: i64 },
    // Phase 5 adds: Reload
}
```

```rust
// In scheduler select! loop, add new branch:
cmd = cmd_rx.recv() => {
    match cmd {
        Some(SchedulerCmd::RunNow { job_id }) => {
            if let Some(job) = self.jobs.get(&job_id) {
                let child_cancel = self.cancel.child_token();
                join_set.spawn(run::run_job(
                    self.pool.clone(),
                    job.clone(),
                    "manual".to_string(),
                    child_cancel,
                ));
                tracing::info!(target: "cronduit.scheduler", job_id, "manual run triggered");
            }
        }
        None => break, // channel closed
    }
}
```
[VERIFIED: existing scheduler loop structure in src/scheduler/mod.rs]

### Pattern 3: Askama Template with HTMX Partial

**What:** Full page templates extend `base.html`; partials return HTML fragments for HTMX swaps.
**When to use:** Every page + its HTMX-refreshable section.

```rust
// Handler that serves full page OR partial based on HX-Request header
use axum_htmx::HxRequest;

async fn dashboard(
    HxRequest(is_htmx): HxRequest,
    State(state): State<AppState>,
    Query(params): Query<DashboardParams>,
) -> impl IntoResponse {
    let jobs = queries::get_dashboard_jobs(&state.pool, &params).await?;
    if is_htmx {
        // Return only the table partial
        JobTablePartial { jobs }.into_response()
    } else {
        // Return full page with base layout
        DashboardPage { jobs, params }.into_response()
    }
}
```
[CITED: axum-htmx README pattern for HxRequest extractor]

### Pattern 4: CSRF Double-Submit Cookie

**What:** Stateless CSRF protection via matching cookie and form field.
**When to use:** All POST endpoints (`/api/jobs/:id/run`).

```rust
// Generate token on first page load (set cookie if not present)
fn generate_csrf_token() -> String {
    use rand::Rng;
    let token: [u8; 32] = rand::thread_rng().gen();
    hex::encode(token)
}

// Validate: extract cookie + form field, compare
fn validate_csrf(cookie_token: &str, form_token: &str) -> bool {
    // Use constant-time comparison to prevent timing attacks
    use subtle::ConstantTimeEq;
    cookie_token.as_bytes().ct_eq(form_token.as_bytes()).into()
}
```
[ASSUMED: pattern is standard; `subtle` crate for constant-time comparison recommended but optional for v1]

### Pattern 5: ANSI Log Rendering

**What:** Convert raw log lines with ANSI codes to safe HTML spans.
**When to use:** Run Detail log viewer.

```rust
// src/web/ansi.rs
pub fn render_log_line(raw: &str) -> String {
    // ansi-to-html::convert HTML-escapes input FIRST,
    // then wraps ANSI-styled segments in <span> tags.
    // Safe by construction -- no raw HTML injection possible.
    ansi_to_html::convert(raw).unwrap_or_else(|_| {
        // Fallback: just HTML-escape without ANSI processing
        askama::filters::escape(raw, askama::filters::Html).unwrap()
    })
}
```
[VERIFIED: ansi-to-html docs.rs confirms HTML-escaping before ANSI transformation]

### Anti-Patterns to Avoid

- **`| safe` or `PreEscaped` on log content:** NEVER. This is the single most dangerous thing in the entire phase. Askama's default escaping prevents XSS. The only place raw HTML is allowed is the output of `ansi-to-html::convert()` which has already HTML-escaped the input.
- **Client-side ANSI parsing:** Moves the XSS surface to JavaScript. Server-side only.
- **Bypassing the scheduler for Run Now:** Creating a separate code path that inserts `job_runs` directly. All runs go through `SchedulerCmd::RunNow` -> scheduler -> `run_job()`.
- **Loading HTMX or fonts from CDN:** Breaks the single-binary promise and offline/air-gapped homelabs.
- **Using `askama_axum`:** This crate is deprecated. Use `askama_web` with `axum-0.8` feature.
- **Full page reload on HTMX poll:** Only swap the table body partial, not the entire page. Preserves filter state, scroll position, and focus.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| ANSI to HTML | Custom regex-based ANSI stripper/converter | `ansi-to-html 0.2.2` | Handles 3/4/8/24-bit color, bold/italic/underline, nested styles, and HTML-escapes input. A regex approach will miss edge cases and risk XSS. |
| HTMX request detection | Manual `HX-Request` header parsing | `axum-htmx 0.8.1` `HxRequest` extractor | Type-safe, handles missing header correctly, provides `HxTrigger` responder for toast. |
| Template rendering | Hand-built HTML strings in Rust | `askama 0.15` templates | Compile-time checked, auto-escaping, template inheritance, separation of concerns. |
| CSS framework | Hand-written CSS from scratch | Tailwind CSS standalone binary | Utility-first, tree-shakes unused classes, works without Node. |
| Static asset embedding | `include_str!` or `include_bytes!` | `rust-embed 8.11` | Handles MIME types, debug-mode disk reads, directory traversal, Content-Type headers. |
| Cookie handling | Manual `Set-Cookie` header construction | `axum-extra` cookie feature | Handles encoding, `HttpOnly`, `SameSite`, `Secure` flags correctly. |

**Key insight:** The XSS safety of the log viewer depends entirely on never bypassing HTML escaping. Using `ansi-to-html` (which escapes first) + askama default escaping (which escapes template variables) creates defense-in-depth. Hand-rolling either layer introduces XSS risk.

## Common Pitfalls

### Pitfall 1: XSS via Log Content (Pitfall 13 from PITFALLS.md)

**What goes wrong:** A job writes `<script>alert(1)</script>` to stdout. If the log viewer uses `| safe` or `PreEscaped` to render log lines, the script executes in the operator's browser.
**Why it happens:** Developers use `| safe` to render ANSI-transformed HTML, not realizing it also passes through any injected HTML from log content.
**How to avoid:** (1) `ansi-to-html::convert()` HTML-escapes input before ANSI transformation. (2) The output of `convert()` is safe HTML (contains only `<span>` and `<b>` tags). (3) In askama templates, the ANSI-converted output is the ONLY thing that may use `| safe` -- and only because the crate guarantees safety. (4) CI test: insert `<script>alert(1)</script>` as a log line, render the template, assert the output contains `&lt;script&gt;` not `<script>`.
**Warning signs:** Any use of `| safe` or `PreEscaped` on variables derived from `job_logs.line`.

### Pitfall 2: Run Now Bypasses Scheduler (Pitfall 16 from PITFALLS.md)

**What goes wrong:** The Run Now handler creates a `job_runs` row directly and calls `run_job()` itself, bypassing the scheduler. This means the scheduler's `JoinSet` doesn't track the run, graceful shutdown doesn't wait for it, and future concurrency policies won't apply.
**Why it happens:** It's simpler to call `run_job()` directly than to set up an mpsc channel.
**How to avoid:** The web handler ONLY sends `SchedulerCmd::RunNow { job_id }` through the channel. The scheduler's `select!` loop receives it and spawns the run through the same `run_job()` path as scheduled runs. The handler returns 204 immediately (fire-and-forget from the web perspective).
**Warning signs:** `run_job()` called from anywhere outside `src/scheduler/mod.rs`.

### Pitfall 3: rust-embed Debug vs Release Behavior (Pitfall 23 from PITFALLS.md)

**What goes wrong:** In debug mode, `rust-embed` reads assets from disk (fast iteration). In release mode, assets are compiled into the binary. If the `#[folder]` path is wrong or assets are missing at build time, the release binary ships with no CSS/JS.
**Why it happens:** The `#[folder]` attribute is relative to `Cargo.toml`, not the source file. And `debug-embed = false` (the default) means dev and release read from different sources.
**How to avoid:** (1) CI builds in release mode and runs a smoke test that fetches `/` and asserts CSS/JS are served. (2) `build.rs` runs `just tailwind` to ensure CSS exists before `rust-embed` compiles. (3) The `#[folder]` path is verified to exist in CI.
**Warning signs:** `cargo build --release` succeeds but the web UI has no styling.

### Pitfall 4: HTMX Polling Breaks Filter/Sort State

**What goes wrong:** The HTMX poll replaces the entire table. If the user has typed a filter or changed sort order, the poll response doesn't include those params, so the table reverts to the unfiltered/unsorted default.
**How to avoid:** The poll target (`hx-get`) must include current filter/sort params. Use `hx-include` to include the filter input value, or use `hx-push-url="true"` and read params from the URL on the server side. The partial endpoint reads `?filter=&sort=&order=` from query params.
**Warning signs:** Filter resets every 3 seconds.

### Pitfall 5: Askama Template Path Configuration

**What goes wrong:** Askama looks for templates relative to `$CARGO_MANIFEST_DIR/templates` by default. If templates are in a different directory, the build fails with a confusing error about missing template files.
**How to avoid:** Either place templates in `templates/` at the repo root (askama default), or add an `askama.toml` at the repo root with `dirs = ["templates"]`. The `#[template(path = "pages/dashboard.html")]` attribute is relative to the configured template directory.
**Warning signs:** `error: template not found` during `cargo build`.

### Pitfall 6: CSRF Cookie Not Set on First Visit

**What goes wrong:** The Run Now button sends a POST with a CSRF form field, but the cookie hasn't been set yet because the user navigated directly to a job detail page. The POST fails with a CSRF error.
**How to avoid:** Set the CSRF cookie in the `base.html` template render path -- every page load checks for the cookie and sets it if missing. Use axum middleware (or a `tower` layer) that runs on every GET response to ensure the cookie is always present.
**Warning signs:** Run Now fails on first visit, works after a page refresh.

## Code Examples

### Dashboard DB Query (Paginated Jobs with Last Run)

```sql
-- Get all enabled jobs with their most recent run status
-- Source: derived from existing schema in migrations/sqlite/
SELECT
    j.id, j.name, j.schedule, j.resolved_schedule, j.job_type,
    j.config_json, j.timeout_secs,
    lr.status AS last_status,
    lr.start_time AS last_run_time,
    lr.trigger AS last_trigger
FROM jobs j
LEFT JOIN (
    SELECT job_id, status, start_time, trigger,
           ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY start_time DESC) AS rn
    FROM job_runs
) lr ON lr.job_id = j.id AND lr.rn = 1
WHERE j.enabled = 1
ORDER BY j.name ASC
```
[VERIFIED: schema matches migrations/sqlite/20260410_000000_initial.up.sql]

Note: `ROW_NUMBER() OVER` works in both SQLite (3.25+) and PostgreSQL. The existing codebase pattern uses separate query strings per backend but the SQL is identical for this query.

### Run History Query (Paginated)

```sql
SELECT id, status, trigger, start_time, end_time, duration_ms, exit_code, error_message
FROM job_runs
WHERE job_id = ?1
ORDER BY start_time DESC
LIMIT ?2 OFFSET ?3
```

### Log Lines Query (Reverse-Paginated from End)

```sql
-- Get total count for pagination
SELECT COUNT(*) as total FROM job_logs WHERE run_id = ?1;

-- Get page of logs (most recent first, then reverse for display)
SELECT id, stream, ts, line
FROM job_logs
WHERE run_id = ?1
ORDER BY id DESC
LIMIT ?2 OFFSET ?3
```

### Health Endpoint Handler

```rust
// Source: OPS-01 requirement
use axum::Json;
use serde_json::json;

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    let db_ok = match state.pool.reader() {
        PoolRef::Sqlite(p) => sqlx::query("SELECT 1").fetch_one(p).await.is_ok(),
        PoolRef::Postgres(p) => sqlx::query("SELECT 1").fetch_one(p).await.is_ok(),
    };
    Json(json!({
        "status": "ok",
        "db": if db_ok { "ok" } else { "error" },
        "scheduler": "running"
    }))
}
```
[VERIFIED: PoolRef pattern matches existing src/db/queries.rs]

### Askama Template with HTMX (base.html skeleton)

```html
{# templates/base.html #}
<!DOCTYPE html>
<html lang="en" data-theme="dark">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{% block title %}Cronduit{% endblock %}</title>
    <link rel="icon" type="image/svg+xml" href="/static/favicons/favicon-32.svg">
    <link rel="stylesheet" href="/static/app.css">
    <script src="/static/theme.js"></script>
    <script src="/vendor/htmx.min.js"></script>
</head>
<body class="bg-[var(--cd-bg-primary)] text-[var(--cd-text-primary)] font-mono">
    <nav><!-- nav bar with Dashboard + Settings links + theme toggle --></nav>
    <main>{% block content %}{% endblock %}</main>
    <div id="toast-container"></div>
</body>
</html>
```
[CITED: askama template syntax from askama.rs/template_syntax.html]

### HTMX Polling Table Partial

```html
{# templates/partials/job_table.html #}
{% for job in jobs %}
<tr class="hover:bg-[var(--cd-bg-hover)] border-b border-[var(--cd-border-subtle)]">
    <td><a href="/jobs/{{ job.id }}" class="text-[var(--cd-text-accent)]">{{ job.name }}</a></td>
    <td>{{ job.schedule }}</td>
    <td>{{ job.resolved_schedule }}</td>
    <td>{{ job.next_fire }}</td>
    <td><span class="cd-badge cd-badge--{{ job.last_status }}">{{ job.last_status_label }}</span></td>
    <td>{{ job.last_run_relative }}</td>
    <td>
        <form hx-post="/api/jobs/{{ job.id }}/run" hx-swap="none">
            <input type="hidden" name="csrf_token" value="{{ csrf_token }}">
            <button type="submit" class="cd-btn-primary">Run Now</button>
        </form>
    </td>
</tr>
{% endfor %}
```

### Dashboard Page with HTMX Polling

```html
{# templates/pages/dashboard.html #}
{% extends "base.html" %}
{% block title %}Dashboard - Cronduit{% endblock %}
{% block content %}
<div>
    <h1>Dashboard</h1>
    <input type="text" name="filter" value="{{ filter }}"
           placeholder="Filter by name..."
           hx-get="/partials/job-table"
           hx-trigger="keyup changed delay:300ms"
           hx-target="#job-table-body"
           hx-include="[name='sort'],[name='order']"
           hx-push-url="true">
    <table>
        <thead><!-- sortable column headers --></thead>
        <tbody id="job-table-body"
               hx-get="/partials/job-table?filter={{ filter }}&sort={{ sort }}&order={{ order }}"
               hx-trigger="every 3s"
               hx-swap="innerHTML">
            {% include "partials/job_table.html" %}
        </tbody>
    </table>
</div>
{% endblock %}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `askama_axum` crate | `askama_web` with `axum-0.8` feature | askama 0.13+ (2024) | Must use `askama_web`, not `askama_axum` (deprecated) [VERIFIED: crates.io] |
| HTMX 1.x | HTMX 2.0.7 | 2024 | Breaking: `hx-on` syntax changed; `htmx.ajax()` API changed. Use 2.0 docs. [VERIFIED: GitHub API] |
| Tailwind via npm | Tailwind standalone binary | 2022+ | No Node dependency; download from GitHub releases [VERIFIED: existing justfile recipe] |
| `askama` + `rinja` (fork) | `askama` 0.15 (merged) | 2024 | Rinja merged back into askama; use `askama` 0.15+ [VERIFIED: crates.io] |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `rand` 0.8.x already in Cargo.toml or easily addable for CSRF token generation | Standard Stack | LOW -- `rand` is ubiquitous; may need to add explicitly |
| A2 | `uuid` crate useful for CSRF tokens | Standard Stack | LOW -- can use raw `rand` bytes instead |
| A3 | `subtle` crate recommended for constant-time CSRF comparison | Code Examples | LOW -- timing attacks on CSRF are theoretical; can use simple `==` for v1 |
| A4 | `askama` 0.15 template directory defaults to `$CARGO_MANIFEST_DIR/templates` | Pitfalls | LOW -- well-documented default; verify in askama docs |
| A5 | `ansi-to-html` output is safe to use with `\| safe` in askama because it HTML-escapes input first | Architecture Patterns | MEDIUM -- verified via docs.rs but should be confirmed with XSS test in CI |

## Open Questions (RESOLVED)

1. **JetBrains Mono font licensing for embedding**
   - What we know: JetBrains Mono is OFL-1.1 licensed (open source). v2.304 is latest.
   - **RESOLVED:** OFL-1.1 requires the license notice to accompany redistributed fonts. Include `OFL.txt` in `assets/static/fonts/` with the full OFL-1.1 text from the JetBrains Mono repository. Plan 03-01 Task 1 step 7 already includes this. [DECISION: include OFL.txt]

2. **Tailwind CSS version for standalone binary**
   - What we know: npm registry shows Tailwind 4.2.2. The standalone binary may lag behind npm releases. Tailwind v4 has breaking config changes (CSS-based config instead of `tailwind.config.js`).
   - **RESOLVED:** Lock to Tailwind v3.4.17 (latest v3 release). The `tailwind.config.js` format used in Plan 03-01 is v3-specific (`module.exports`, `content`, `theme.extend`). Tailwind v4 uses a completely different CSS-based config and would break the build. The `just tailwind` recipe MUST download from a pinned v3.4.17 release URL, not `latest` (which resolves to v4). [DECISION: lock to v3.4.17 in justfile download URL]

3. **`ansi-to-html` CSS variable naming**
   - What we know: docs.rs shows it outputs `<span style='color:var(--red,#a00)'>` with CSS variables and inline fallback colors.
   - **RESOLVED:** The CSS variable names (`--red`, `--green`, etc.) do NOT conflict with Cronduit's design system variables (which use `--cd-` prefix). Plan 03-01 Task 1 already defines these ANSI color CSS variables in `app.css` under a separate `:root` block with both dark and light mode values. The fallback colors in `ansi-to-html` output ensure rendering even if CSS vars are missing. [DECISION: define `--red`, `--green`, etc. in app.css; no conflict with `--cd-*` tokens]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | Everything | Yes | 1.94.1 (edition 2024) | -- |
| Tailwind standalone binary | CSS build | No (not yet downloaded) | -- | `just tailwind` downloads automatically |
| HTMX JS file | Web UI | No (not yet vendored) | -- | Must download HTMX 2.0.7 and commit to `assets/vendor/` |
| JetBrains Mono WOFF2 | Typography | No (not yet downloaded) | -- | Must download and commit to `assets/static/fonts/` |
| Favicon SVGs | Branding | Yes | -- | Already in `design/favicons/` -- copy to `assets/static/favicons/` |

**Missing dependencies with no fallback:**
- HTMX 2.0.7 JS file must be downloaded and vendored before UI can function.
- JetBrains Mono WOFF2 files must be self-hosted (no CDN per project constraints).

**Missing dependencies with fallback:**
- Tailwind standalone binary is auto-downloaded by `just tailwind` recipe (no manual step).

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test + cargo nextest (already configured) |
| Config file | `.config/nextest.toml` or default |
| Quick run command | `cargo test --lib` |
| Full suite command | `just nextest` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| UI-10 | XSS prevention: `<script>` in logs renders as escaped text | integration | `cargo test --test xss_log_safety` | No -- Wave 0 |
| UI-12 | Run Now goes through scheduler command channel | unit | `cargo test web::handlers::api::tests` | No -- Wave 0 |
| UI-15 | CSRF token validation rejects mismatched tokens | unit | `cargo test web::csrf::tests` | No -- Wave 0 |
| OPS-01 | Health endpoint returns correct JSON | integration | `cargo test --test health_endpoint` | No -- Wave 0 |
| UI-07 | HTMX partial returns valid HTML fragment | integration | `cargo test --test dashboard_partial` | No -- Wave 0 |
| UI-06 | Dashboard lists enabled jobs with correct columns | integration | `cargo test --test dashboard_render` | No -- Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test --lib`
- **Per wave merge:** `just nextest`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `tests/xss_log_safety.rs` -- covers UI-10 (XSS prevention CI test)
- [ ] `tests/health_endpoint.rs` -- covers OPS-01
- [ ] `src/web/csrf.rs` unit tests -- covers UI-15
- [ ] `src/web/handlers/api.rs` unit tests -- covers UI-12

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | v1 ships unauthenticated (documented constraint) |
| V3 Session Management | No | No sessions -- stateless CSRF via cookies |
| V4 Access Control | No | No RBAC in v1 |
| V5 Input Validation | Yes | Askama default HTML escaping; `ansi-to-html` escapes input; query param validation via serde |
| V6 Cryptography | Partial | CSRF token uses `rand` CSPRNG; no other crypto in Phase 3 |

### Known Threat Patterns for Server-Rendered Web UI

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| XSS via log content injection | Tampering | Askama default escaping + `ansi-to-html` HTML-escapes input before ANSI transformation; CI XSS test |
| CSRF on Run Now endpoint | Elevation of Privilege | Double-submit cookie pattern; `HttpOnly; SameSite=Strict` cookie + form field match |
| Information disclosure via settings page | Information Disclosure | Settings page shows config path and DB status -- acceptable for single-operator tool; no secrets exposed |
| Log content exfiltration | Information Disclosure | Mitigated by default loopback bind; operators front with auth proxy for LAN exposure |

## Project Constraints (from CLAUDE.md)

- **Tech stack locked:** Rust backend, `bollard` for Docker, `askama_web 0.15` (NOT `askama_axum`), Tailwind CSS standalone binary, HTMX vendored locally.
- **No CDN:** HTMX, fonts, and CSS must be embedded via `rust-embed`. No external network requests at page load.
- **Single binary:** All assets compiled into binary for release; disk reads in debug mode via `rust-embed` default.
- **TOML config:** Phase 3 does not add config options but reads `config_path` for display on Settings page.
- **rustls only:** No new dependencies may pull `openssl-sys`.
- **Quality gates:** `cargo fmt --check`, `cargo clippy -D warnings`, `just nextest` must pass.
- **Branching:** All changes via PR on feature branch; no direct commits to `main`.
- **Diagrams:** Mermaid only, no ASCII art.
- **Design fidelity:** Must match `design/DESIGN_SYSTEM.md` terminal-green brand.

## Sources

### Primary (HIGH confidence)
- crates.io API (2026-04-10) -- version verification for `askama 0.15.6`, `askama_web 0.15.2`, `rust-embed 8.11.0`, `axum-htmx 0.8.1`, `axum-extra 0.12.5`, `ansi-to-html 0.2.2`
- docs.rs `ansi-to-html 0.2.2` -- API documentation confirming HTML-escaping behavior and output format
- GitHub API -- HTMX latest release `v2.0.7`, JetBrains Mono latest release `v2.304`
- Existing codebase -- `src/web/mod.rs`, `src/scheduler/mod.rs`, `src/db/queries.rs`, `migrations/sqlite/`, `Cargo.toml`, `justfile`, `design/DESIGN_SYSTEM.md`

### Secondary (MEDIUM confidence)
- [Askama template syntax documentation](https://askama.rs/en/stable/template_syntax.html) -- template inheritance (`extends`, `block`), escaping behavior (auto-escapes HTML by default)

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all crate versions verified against crates.io on 2026-04-10
- Architecture: HIGH -- patterns derived from existing codebase structure and locked decisions
- Pitfalls: HIGH -- pitfalls 13, 16, 23 documented in project PITFALLS.md; mitigations verified against crate APIs

**Research date:** 2026-04-10
**Valid until:** 2026-05-10 (30 days -- stable ecosystem, locked versions)
