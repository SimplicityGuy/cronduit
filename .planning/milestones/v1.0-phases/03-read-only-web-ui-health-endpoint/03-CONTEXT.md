# Phase 3: Read-Only Web UI & Health Endpoint - Context

**Gathered:** 2026-04-10
**Status:** Ready for planning

<domain>
## Phase Boundary

A terminal-green, server-rendered web dashboard that:

1. Serves an HTMX-powered Dashboard page listing all enabled jobs in a dense table with name, raw schedule, resolved schedule, next fire time, last-run status badge, and last-run timestamp — auto-refreshing every 3s via HTMX partial swap
2. Provides a Job Detail page with full resolved config, human-readable cron description (via croner), and paginated run history
3. Provides a Run Detail page with run metadata and a paginated log viewer showing interleaved stdout/stderr with ANSI color codes parsed server-side into sanitized HTML spans
4. Provides a Settings/Status page showing scheduler uptime, DB connection status, config file path, last reload time, and Cronduit version
5. Implements a "Run Now" button per job that sends a command through a scheduler command channel (not bypassing the scheduler) and records runs with `trigger='manual'`
6. Serves `GET /health` returning `{"status":"ok","db":"ok","scheduler":"running"}`
7. Applies the full `design/DESIGN_SYSTEM.md` terminal-green palette with both dark and light mode support
8. Embeds all static assets (Tailwind CSS, HTMX, favicons) via `rust-embed` in a single binary
9. Protects all state-changing endpoints with a double-submit CSRF token

**Explicitly NOT in Phase 3:** SSE log streaming for in-progress runs (Phase 6, UI-14), `/metrics` Prometheus endpoint (Phase 6, OPS-02), Docker container execution (Phase 4), config reload (Phase 5), `@random` resolution (Phase 5), retention pruner (Phase 6), example `docker-compose.yml` (Phase 6).

New capabilities belong in other phases (see ROADMAP.md Phases 4-6).

</domain>

<decisions>
## Implementation Decisions

### Dashboard Layout & Behavior

- **D-01:** The Dashboard uses a **dense HTML table** with one row per job. Columns: Name, Schedule (raw), Resolved Schedule, Next Fire (relative time like "in 4h 12m"), Last Status (badge), Last Run (relative timestamp). Fits the terminal aesthetic and matches UI-06 literally.
- **D-02:** **Inline filter and sort controls** above the table. A text input for substring filtering on job name (with HTMX `hx-get` + debounce) and clickable column headers for sort. Filter/sort state preserved in URL query params. Matches UI-13.
- **D-03:** The table **auto-refreshes every 3s** via HTMX polling (`hx-trigger="every 3s"`) on a table partial endpoint. The "next fire" countdown is computed **server-side only** — no client-side JS timer. 3s granularity is sufficient for cron jobs.
- **D-04:** When zero jobs are configured, the Dashboard shows a **helpful onboarding empty state**: centered message "No jobs configured yet" with a hint pointing to the config file path (read from `AppState`).

### Log Viewer & ANSI Rendering

- **D-05:** The Run Detail log viewer shows **interleaved stdout and stderr in chronological order**. Stderr lines are visually distinguished with a subtle left-border or background tint using `--cd-status-error-bg`. Preserves the timeline of what actually happened.
- **D-06:** ANSI SGR color codes are parsed **server-side in Rust** (via a crate like `ansi-to-html` or a small hand-rolled parser) into sanitized `<span>` tags with CSS classes. No client-side JS for ANSI parsing. All other content is HTML-escaped by default — ANSI parsing is the only transformation. This is the XSS safety model (UI-10).
- **D-07:** The log viewer is **paginated** — shows the most recent 500 lines by default with a "Load older lines" button (HTMX `hx-get` with offset param). Keeps initial page load fast for jobs with large output.

### Run Now Mechanics

- **D-08:** A **`tokio::sync::mpsc` command channel** is added to `SchedulerLoop`. The web handler sends `SchedulerCmd::RunNow { job_id }` through the channel. The scheduler receives it in the `select!` loop and spawns the run through the normal fire/run pipeline with `trigger='manual'`. This satisfies UI-12's requirement that manual runs go through the scheduler, not bypass it.
- **D-09:** The `SchedulerCmd` enum is designed for extensibility — Phase 5 will add `Reload` and potentially other commands. Phase 3 ships `RunNow` only.
- **D-10:** After clicking "Run Now", the POST response includes an **`HX-Trigger` header** that fires a toast notification ("Run queued: {job_name}"). The next 3s poll picks up the new running row in the table. No redirect. Uses `axum-htmx` for the `HxTrigger` responder.
- **D-11:** All state-changing endpoints (`POST /api/jobs/:id/run`, future `POST /api/reload`) are protected by a **double-submit CSRF token**. A random token is set as an `HttpOnly; SameSite=Strict` cookie and included as a hidden form field. The server validates they match. No server-side session store needed — fits the stateless single-binary model.

### Template & Asset Pipeline

- **D-12:** Template inheritance uses **askama's `{% extends "base.html" %}` pattern**. A single `base.html` contains `<head>`, nav, footer, and a `{% block content %}` slot. Each page (`dashboard.html`, `job_detail.html`, `run_detail.html`, `settings.html`) extends `base.html`. HTMX-swappable partials live in a `partials/` subdirectory (`job_table.html`, `run_history.html`, `log_viewer.html`).
- **D-13:** The UI ships with **both dark and light mode**. Default follows `prefers-color-scheme` media query. A toggle button in the nav overrides it and persists the choice in `localStorage`. CSS custom properties switch based on a `data-theme` attribute on `<html>`. A small JS snippet (~10 lines) beyond HTMX handles the toggle and localStorage persistence.
- **D-14:** **Navigation** has two top-level items: Dashboard (home/job list) and Settings/Status. Job Detail and Run Detail are drill-down pages reached by clicking rows, not top-level nav items. The theme toggle sits in the nav bar.
- **D-15:** Tailwind integration uses a **`just tailwind` recipe** that runs the standalone Tailwind binary (`tailwindcss -i assets/src/app.css -o assets/static/app.css --minify`). A `build.rs` ensures CSS is rebuilt before `cargo build`. `just dev` runs Tailwind in `--watch` mode alongside `cargo-watch` for the inner dev loop.
- **D-16:** HTMX is vendored into `assets/vendor/htmx.min.js` and embedded via `rust-embed`. Never loaded from a CDN. Favicons from `design/favicons/` are also embedded.
- **D-17:** `rust-embed` with `debug-embed = false` (default) reads assets from disk in debug builds, enabling Tailwind edit-refresh without `cargo build`. Release builds embed everything into the binary.

### Claude's Discretion

The planner / researcher may decide the following without re-asking:

- Exact ANSI parsing crate choice (`ansi-to-html`, `cansi`, or hand-rolled) — as long as it's server-side Rust, produces sanitized HTML, and doesn't use `| safe` / `PreEscaped` on raw log content
- Exact toast notification implementation (inline HTML element + CSS animation, or a small JS helper triggered by HX-Trigger event)
- Run history pagination page size (suggested starting point: 25 runs per page)
- Log viewer line count per page (500 is the starting point; tunable)
- Exact status badge design (text + background tint, or icon + text) as long as it uses the DESIGN_SYSTEM.md status color tokens
- Whether `build.rs` calls Tailwind directly or checks for a prebuilt CSS file
- Exact `AppState` extensions (what fields are added for scheduler handle, config path, version, etc.)
- Whether the health endpoint includes additional fields beyond the spec minimum
- Template file naming conventions (e.g., `dashboard.html` vs `pages/dashboard.html`)
- Exact CSS custom property naming for theme tokens (as long as they map to DESIGN_SYSTEM.md)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-Level Contracts

- `CLAUDE.md` — Locked tech stack with version pins. Especially: `askama_web 0.15` with `axum-0.8` feature (NOT `askama_axum`), `axum-htmx 0.8.1`, `rust-embed 8.11.0`, `tower-http 0.6.8`. Full HTMX integration approach section. `## What NOT to Use` table (askama_axum, CDN loading, Node-based Tailwind).
- `.planning/PROJECT.md` — Vision, locked constraints, out-of-scope (no SPA, no CDN, no auth in v1).
- `.planning/REQUIREMENTS.md` — Phase 3 requirements: UI-01 through UI-13, UI-15, OPS-01 (15 requirements total). Success criteria in ROADMAP.md.
- `.planning/ROADMAP.md` section "Phase 3: Read-Only Web UI & Health Endpoint" — Phase goal, 6 success criteria, pitfalls to address (13, 16, 23).

### Specification & Research

- `docs/SPEC.md` section "Web UI" — Authoritative v1 spec for all UI pages, endpoints, and behaviors.
- `.planning/research/ARCHITECTURE.md` section "AppState" (Pattern 1) — How axum state is structured; Phase 3 extends this with scheduler command channel.
- `.planning/research/PITFALLS.md` section 13 (log XSS / ANSI / binary rendering), section 16 (Run Now bypasses scheduler), section 23 (rust-embed hot reload / binary size).
- `.planning/research/STACK.md` — Version pins for `askama_web 0.15.2`, `axum-htmx 0.8.1`, `rust-embed 8.11.0`, `htmx 2.0.x`.

### Design & Brand

- `design/DESIGN_SYSTEM.md` — **CRITICAL for this phase.** Full color token system (sections 2.1-2.6), typography (section 3), component patterns (sections 4-8), dark/light mode token pairs. Every UI element must reference these tokens.
- `design/showcase.html` — HTML reference implementation of the brand look. Extract layout patterns and component styles from this.
- `design/favicons/` — SVG favicons at 16, 32, 192, 512px. Embed all four via rust-embed.
- `design/logos/` — Square logo variants for nav branding.

### Phase 1 & 2 Foundation (already built)

- `.planning/phases/01-foundation-security-posture-persistence-base/01-CONTEXT.md` — D-05 (single crate), D-09/D-10 (justfile + just-only CI), D-11 (`just tailwind` recipe defined but placeholder CSS).
- `.planning/phases/02-scheduler-core-command-script-executor/02-CONTEXT.md` — D-01 (scheduler select! loop), D-08 (scheduler in `src/scheduler/`). Phase 3 adds command channel to this loop.
- `src/web/mod.rs` — Current `AppState` (has `started_at`, `version`), `router()`, `serve()`. Phase 3 extends all three.
- `src/scheduler/mod.rs` — `SchedulerLoop` struct. Phase 3 adds `mpsc::Receiver<SchedulerCmd>` to the select! loop.
- `src/db/queries.rs` — `DbJob`, `get_enabled_jobs()`, query helpers. Phase 3 adds read queries for job runs and logs.
- `assets/src/app.css` — Placeholder file waiting for Tailwind directives.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`src/web/mod.rs` — `AppState`, `router()`, `serve()`**: Extend `AppState` with `DbPool`, `cmd_tx: mpsc::Sender<SchedulerCmd>`, `config_path`, etc. Add routes to `router()`. `serve()` graceful shutdown already wired.
- **`src/db/queries.rs` — `DbJob`, `get_enabled_jobs()`**: Reuse `DbJob` for dashboard queries. Add queries for `job_runs` and `job_logs` tables.
- **`src/scheduler/mod.rs` — `SchedulerLoop`**: Add `mpsc::Receiver<SchedulerCmd>` as a new branch in the `tokio::select!` loop. Minimal change to existing scheduler code.
- **`design/DESIGN_SYSTEM.md`**: Full color token table ready to map to CSS custom properties. Dark and light mode values defined for every token.
- **`design/showcase.html`**: Reference HTML for the terminal-green brand look. Extract nav, table, badge, and card patterns.
- **`design/favicons/`**: SVG favicons at all required sizes, ready to embed.

### Established Patterns

- **Just-only CI** (Phase 1 D-09/D-10): All new recipes (`just tailwind`, `just dev`) go through the justfile. CI calls `just` exclusively.
- **Split migration directories** (Phase 1 D-13): Any new migration goes into both `migrations/sqlite/` and `migrations/postgres/`.
- **`tracing` structured logging** (Phase 1): JSON-to-stdout with `RUST_LOG` env filter. Web request logging via `tower-http::TraceLayer` already in `router()`.
- **`DbPool` read/write split** (Phase 1 D-05): Dashboard queries should use the read pool. Run Now inserts go through the scheduler (which uses the write pool).

### Integration Points

- **`src/cli/run.rs`**: After scheduler spawn, before `serve()`, construct the extended `AppState` with the command channel sender and pass it to the router.
- **`Cargo.toml`**: Add `askama_web`, `askama` (with `with-axum-0.8` feature), `rust-embed`, `axum-htmx`, `axum-extra` (for typed headers/query), `uuid` (for CSRF tokens), `rand` (for CSRF token generation).
- **`assets/src/app.css`**: Replace placeholder with `@tailwind base; @tailwind components; @tailwind utilities;` plus Cronduit custom properties from DESIGN_SYSTEM.md.
- **`tailwind.config.js`**: Create at repo root, configured to scan `templates/**/*.html` for class usage.
- **`templates/`**: New directory at repo root for askama templates.

</code_context>

<specifics>
## Specific Ideas

- The user chose **both dark and light mode** with a system-preference default + manual toggle in the nav. This means DESIGN_SYSTEM.md's full dark/light token table must be implemented as CSS custom properties switching on a `data-theme` attribute. A small (~10 line) JS snippet handles the toggle and localStorage persistence — this is the only custom JS beyond HTMX.
- The user wants the **scheduler command channel** pattern (`SchedulerCmd` enum with `RunNow` variant) designed for extensibility. Phase 5 will add `Reload` to this enum. The channel sender lives in `AppState` so web handlers can send commands.
- The user chose **head-drop** for the log channel in Phase 2 (D-10). The log viewer pagination (showing most recent 500 lines first with "Load older" button) aligns with this — users see the tail end of output first, which is the most diagnostic part.
- The user explicitly chose **interleaved** stdout/stderr (not tabs or side-by-side), with stderr lines visually tinted. This preserves temporal context of what actually happened during execution.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

### Reviewed Todos (not folded)

*(None — no pending todos matched Phase 3 at discussion time.)*

</deferred>

---

*Phase: 03-read-only-web-ui-health-endpoint*
*Context gathered: 2026-04-10*
