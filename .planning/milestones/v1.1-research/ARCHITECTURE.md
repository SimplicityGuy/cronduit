# Cronduit v1.1 — Architecture Research

**Dimension:** Integration Mapping (subsequent milestone, not greenfield)
**Milestone:** v1.1 "Operator Quality of Life"
**Researched:** 2026-04-14
**Confidence:** HIGH — every file path, line number, and module shape verified by direct read of the v1.0.1 source tree.

> This document adapts the standard `research-project/ARCHITECTURE.md` template. The default template is greenfield-oriented ("what architecture should we adopt?"); for v1.1 the architecture is already shipped and the valuable research question is "how do the v1.1 features slot into the existing modules, what new data flows are required, and what is the build order?" The sections below reflect that adapted focus.

---

## 1. Executive Summary

v1.1 is a **polish-and-fix milestone on top of the shipped v1.0.1 architecture.** Every target feature slots into an existing module or adds a small sibling module. None of them require refactoring the scheduler loop, the pool split, or the template inheritance structure. The two architecturally interesting feature threads are:

1. **Stop-a-running-job**, which forces us to track a per-run control handle that the `active_runs` map currently does not carry.
2. **Per-job run numbers**, which forces a migration plus a narrow change in the write path.

Everything else is additive.

The **bulk enable/disable design question is resolvable**: option (b) — a new `jobs.enabled_override` column — is the least intrusive choice given how `sync::sync_config_to_db` currently calls `disable_missing_jobs`. The rationale is derived from reading the actual sync code, not aesthetic preference. Details in §3.7.

The **riskiest feature is stop-a-job**, and it should be the Phase-A spike. Everything else is mechanical.

---

## 2. Existing Architecture Touchpoints (verified)

The shipped modules I traced through for this research:

| Area | File | What it owns today |
|------|------|--------------------|
| Scheduler loop | `src/scheduler/mod.rs` (595 lines; main loop at L64–L371) | `tokio::select!` over sleep/join_set/cmd_rx/cancel; owns `heap`, `jobs_vec`, `join_set: JoinSet<RunResult>` |
| Command channel enum | `src/scheduler/cmd.rs` | `SchedulerCmd::{RunNow, Reload, Reroll}` + `ReloadResult` |
| Per-run task lifecycle | `src/scheduler/run.rs::run_job` (L65–L292) | Insert `running` row → spawn log writer → dispatch to executor → finalize |
| Local exec | `src/scheduler/command.rs::execute_child` (L58–L144) | `tokio::select!` over child.wait/timeout/cancel, kills via SIGKILL on process group |
| Docker exec | `src/scheduler/docker.rs::execute_docker` (L76–L369) | Pre-flight → create → start → wait + log stream → `stop_container(t=10)` on timeout/cancel → `maybe_cleanup_container` |
| Orphan reconciliation | `src/scheduler/docker_orphan.rs` | Filters `label=cronduit.run_id` at startup, stops + removes, marks rows `error`/`orphaned at restart` |
| Config sync | `src/scheduler/sync.rs::sync_config_to_db` (L102–L216) | Reads `jobs` by name, upserts by hash, then `disable_missing_jobs` for names not in config |
| Reload plumbing | `src/scheduler/reload.rs::{do_reload, do_reroll, spawn_file_watcher}` | Debounced file-watch + API + SIGHUP all funnel into `do_reload` |
| DB schema | `migrations/{sqlite,postgres}/20260410_000000_initial.up.sql` | Three tables: `jobs`, `job_runs`, `job_logs`; no per-job counter anywhere |
| Run inserts | `src/db/queries.rs::insert_running_run` (L286–L313) | `INSERT INTO job_runs (job_id, status='running', trigger, start_time) RETURNING id` |
| Run finalize | `src/db/queries.rs::finalize_run` (L316–L360) | `UPDATE job_runs SET status/exit_code/end_time/duration_ms/error_message/container_id` |
| Dashboard query | `src/db/queries.rs::get_dashboard_jobs` (L474–L597) | LEFT JOIN on a row-numbered subquery to attach latest run per job |
| Dashboard handler | `src/web/handlers/dashboard.rs` | Fetches `DashboardJob[]`, computes next-fire via croner in Rust, renders `DashboardPage` or `JobTablePartial` on HTMX |
| Job detail handler | `src/web/handlers/job_detail.rs` | Polling-friendly partial at `/partials/jobs/{id}/runs` with `hx-trigger="every 2s"` while `any_running` |
| Run detail handler | `src/web/handlers/run_detail.rs` | **No log backfill when `is_running=true`** (L64–L82 of `templates/pages/run_detail.html` — just shows a placeholder and attaches the SSE stream) |
| SSE handler | `src/web/handlers/sse.rs::sse_logs` | Subscribes to broadcast sender from `active_runs`; emits `log_line` + `run_complete` |
| Active runs registry | `src/web/mod.rs::AppState.active_runs` | `Arc<RwLock<HashMap<i64, broadcast::Sender<LogLine>>>>` — **holds broadcast sender only, no control handle** |
| API handlers | `src/web/handlers/api.rs` | `run_now`, `reload`, `reroll`, `list_jobs`, `list_job_runs` (all CSRF-gated form posts, return `HX-Trigger` for toasts) |
| Router | `src/web/mod.rs::router` | 20+ routes; clean pattern for adding new partial/API endpoints |
| Templates | `templates/{base.html, pages/*, partials/*}` | askama with `{% extends %}` inheritance; partials for `job_table`, `log_viewer`, `static_log_viewer`, `run_history`, `toast` |
| Health handler | `src/web/handlers/health.rs::health` | Returns `(StatusCode::OK, Json(json!{...}))` via axum — chunked response shape is the likely cause of the reported docker healthcheck failure (bug added to v1.1 scope mid-research) |
| **Missing today** | — | There is NO `overrides.toml`, no `jobs.enabled_override` column, no timeline view, no per-job run counter, no `stopped` status, no log-backfill for active runs, no `cronduit health` CLI subcommand. |

One critical subtlety confirmed by reading `run.rs` L71 + `mod.rs` L56 + L388: the scheduler loop owns `join_set: JoinSet<RunResult>` and creates a new `child_cancel = self.cancel.child_token()` **inline at each spawn** (mod.rs L98, L122, L166, L215). **The `child_cancel` is dropped on the next loop iteration — the scheduler does NOT keep a per-run cancel handle anywhere.** This is the first architectural gap v1.1 has to close.

---

## 3. Feature-by-Feature Mapping

### 3.1 Stop a Running Job (new `stopped` status)

**Touches:**
- `src/scheduler/cmd.rs` — add `SchedulerCmd::Stop { run_id: i64, response_tx: oneshot::Sender<StopResult> }`
- `src/scheduler/mod.rs::SchedulerLoop` — **new field** `running_handles: HashMap<i64, RunControl>` where `RunControl` carries a `CancellationToken` (plus, for docker jobs, a `container_id` populated once create succeeds)
- `src/scheduler/mod.rs` main loop — new match arm for `SchedulerCmd::Stop`; must also **remove** entries from `running_handles` when `join_set.join_next()` yields a `RunResult`
- `src/scheduler/run.rs::run_job` — must register its `CancellationToken` (and later its `container_id`) into the new map. Simpler: pre-create the token in the scheduler loop and pass it to `run_job`; `run_job` is then responsible for writing its `container_id` into the map once it has one.
- `src/scheduler/command.rs::execute_child` — already handles `cancel.cancelled()` → SIGKILL path; needs a new `RunStatus::Stopped` variant distinct from `Shutdown` so `run.rs::run_job` can finalize as `"stopped"` rather than `"cancelled"`. Pass a `StopReason` enum through the cancel path.
- `src/scheduler/docker.rs::execute_docker` — same: its `cancel.cancelled()` branch (L338–L358) currently uses `stop_container(t=10)` and returns `RunStatus::Shutdown` with message `"cancelled due to shutdown"`. Must split into `RunStatus::Stopped` with a different message when triggered by operator-stop vs shutdown.
- `src/scheduler/run.rs::run_job` L238–L244 — add `RunStatus::Stopped => "stopped"` arm
- `src/web/handlers/api.rs` — new `pub async fn stop_run(job_id, run_id)` handler, CSRF-gated, sends `SchedulerCmd::Stop`. Mirrors the `run_now` handler shape.
- `src/web/mod.rs::router` — new route `POST /api/runs/{run_id}/stop`
- `templates/pages/run_detail.html` + `templates/partials/run_history.html` — a "Stop" button visible only when `status == "running"`
- `src/scheduler/run.rs` failure classification (L298–L313) — `classify_failure_reason` should map `"stopped"` → a closed variant so `cronduit_run_failures_total` cardinality stays bounded. Either reuse `FailureReason::Abandoned` or add `FailureReason::Stopped`.
- `migrations/{sqlite,postgres}` — **no schema change needed**. `status` is already `TEXT`. Just a new allowed value.

**Distinguishing cancel-by-shutdown vs cancel-by-operator:**

- **Option A (recommended):** a per-run token plus a per-run `Arc<AtomicU8>` `stop_reason`. The scheduler cancels via `token.cancel()` AND sets `stop_reason = Operator` first. The executor checks `stop_reason` after the `cancel.cancelled()` branch fires.
- Option B: two tokens (global shutdown + per-run operator). Cleaner semantically but doubles the select-arms in every executor. Rejected.

**New/modified:**
- Modified: `cmd.rs`, `mod.rs`, `run.rs`, `command.rs`, `docker.rs`, `api.rs`, `web/mod.rs`, templates
- New: a tiny `src/scheduler/control.rs` for the `RunControl` struct + `StopReason` enum + `StopResult`, to keep `mod.rs` from ballooning.

**Why this is the riskiest feature:** it touches three executors (command, script, docker) + the scheduler loop + the DB finalize path + templates + a new race surface. See §5.1 for the specific race.

### 3.2 Per-Job Sequential Run Number

**Touches:**
- `migrations/sqlite/20260411_000000_job_run_number.up.sql` (new) — `ALTER TABLE job_runs ADD COLUMN job_run_number INTEGER` (nullable, so the backfill UPDATE doesn't block other writers). Plus a paired migration for Postgres.
- `migrations/sqlite/20260411_000001_job_run_number_backfill.up.sql` (new) — idempotent backfill using a window function: `UPDATE job_runs SET job_run_number = rn FROM (SELECT id, ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id) rn FROM job_runs) s WHERE job_runs.id = s.id AND job_runs.job_run_number IS NULL`. SQLite 3.33+ supports this.
- `migrations/sqlite/20260411_000002_job_run_number_not_null.up.sql` (new) — add `NOT NULL` constraint once backfill completes.
- `src/db/queries.rs::insert_running_run` — **the design decision.** Two options:

  **Option A — subquery at insert time.** Single statement with `COALESCE((SELECT MAX … WHERE job_id = $1), 0) + 1`. On SQLite this is atomic within the single write connection — cronduit already uses a separate writer pool with WAL + busy_timeout, so only one writer runs at a time. On Postgres the subquery is NOT atomic; needs a unique constraint + retry loop OR serializable isolation OR a dedicated counter.

  **Option B (recommended) — dedicated counter column on `jobs`.** `ALTER TABLE jobs ADD COLUMN next_run_number BIGINT NOT NULL DEFAULT 1`. `insert_running_run` becomes a two-statement transaction: `UPDATE jobs SET next_run_number = next_run_number + 1 WHERE id = $1 RETURNING next_run_number - 1 AS assigned; INSERT INTO job_runs (…, job_run_number) VALUES (…, $assigned)`. Works identically on both backends because `UPDATE … RETURNING` takes a row lock on the `jobs` row (Postgres) / is serialized by the single writer (SQLite).

  **Recommendation: Option B.** Reasons derived from the existing write path:
  1. Cronduit already uses `ON CONFLICT DO UPDATE … RETURNING id` in `upsert_job` (queries.rs L71–L123) — the team is comfortable with `RETURNING`-on-UPDATE across both backends.
  2. Option B gives a consistent locking story on both backends without a unique-index retry loop.
  3. Option A relies on a backend-specific reasoning ("SQLite has one writer") that will NOT survive the first time someone enables statement-level connection pooling for a hypothetical future multi-writer SQLite path.

- `src/db/queries.rs::DbRun` + `DbRunDetail` — add `job_run_number: i64` field, propagate through both `SELECT` statements.
- `src/web/handlers/run_detail.rs::RunDetailView` — surface `job_run_number`.
- `templates/pages/run_detail.html` — replace `Run #{{ run.id }}` in the breadcrumb/title with `Run #{{ run.job_run_number }}` (keep global id visible as a hint). **Keep the `/jobs/{job_id}/runs/{run_id}` URL on the global id**; don't rekey by `job_run_number` or the orphan-reconciliation path and the SSE handler both break.
- `templates/partials/run_history.html` — show `#{{ run.job_run_number }}` as the primary identifier.

**Backfill safety:** the migration runs at startup before `scheduler::spawn` is called (confirmed by reading `src/cli/run.rs` — see §5.2). No race against new inserts.

**New/modified:** two new migration files per backend; modifications to `queries.rs` insert path, view models, and two templates. No new modules.

### 3.3 Log Backfill on Navigation

**Current behavior** (confirmed by reading `templates/pages/run_detail.html` L64–L82): when `is_running` is true, the template renders an **empty** `#log-lines` div with `sse-connect` attached. No prior log lines are loaded. This is the described bug: lines persisted before the user opened the page are invisible until the run finishes.

**Fix — follows the existing "static then live" pattern that already exists for `run_complete`:**

- In `src/web/handlers/run_detail.rs::run_detail`, when `is_running == true`, fetch the existing log lines from the DB (using the same `fetch_logs` helper already at L97–L132) and pass them to the page template alongside the `is_running` flag.
- `templates/pages/run_detail.html` L64–L82 — render the existing `logs` via the `{% include "partials/log_viewer.html" %}` form **inside** the `#log-lines` div (before the placeholder), then leave the SSE attach as-is.
- **Dedupe:** the SSE stream will replay lines that were in the broadcast channel's ring buffer (capacity 256, per `run.rs` L101), and for a brand-new subscriber those are lines sent **after** the last slot index.

**Edge case:** if the backfill read happens at log-id 500 and the SSE subscribe happens milliseconds later, and a burst of lines arrives in between, lines 501–N could briefly double-appear. The **correct fix** is to:

1. Fetch the DB snapshot first to get `max(id)`.
2. Subscribe to SSE.
3. Discard any SSE line whose id is ≤ the last backfilled id.

Because the broadcast `LogLine` struct (`log_pipeline.rs`) currently has only `stream`, `ts`, `line` — no id — we must add `id: Option<i64>` to `LogLine` and have the broadcast send only **after** the DB insert, with the assigned id populated.

**Touches:**
- `src/scheduler/log_pipeline.rs` — add `id: Option<i64>` to `LogLine`
- `src/db/queries.rs::insert_log_batch` — `RETURNING id` so each emitted `LogLine` gets its assigned id back
- `src/scheduler/run.rs::log_writer_task` — reorder: insert first, then fan-out with populated ids
- `src/web/handlers/run_detail.rs::run_detail` — pass `logs` and `max_backfill_id` when `is_running`
- `templates/pages/run_detail.html` — render backfill inside `#log-lines`; a small JS line that records the max id and drops SSE events with lower ids
- `src/web/handlers/sse.rs` — emit the id as a data-attribute on the rendered div

**Also fixes** the "transient error getting logs" race and the "lines out of order after job completes" bug. With ids, the static swap is deterministic.

### 3.4 Run Timeline (Gantt) View

**Location decision: new `/timeline` page, NOT on the dashboard.**

Justification: the dashboard already does non-trivial work per request (`get_dashboard_jobs` with a subquery, croner next-fire computation, HTMX polling via `hx-trigger="every 2s"`). Adding a gantt render on top would make the dashboard the hot path for three unrelated query shapes. A dedicated page keeps the dashboard query tight and lets the timeline have its own cache semantics.

**Touches:**
- `src/web/handlers/timeline.rs` (new file) — handler producing `TimelinePage` + `TimelinePartial` for HTMX window-toggle (24h / 7d)
- `src/web/handlers/mod.rs` — `pub mod timeline;`
- `src/web/mod.rs::router` — `.route("/timeline", get(handlers::timeline::timeline))` + `.route("/partials/timeline", get(handlers::timeline::timeline_partial))`
- `templates/pages/timeline.html` (new) — extends `base.html`, renders rows as horizontal bars using plain CSS grid with `grid-template-columns` driven by data attributes. No JS library, no canvas.
- `src/db/queries.rs::get_timeline_runs(since: DateTime, until: DateTime) -> Vec<TimelineRun>` (new) — single query fetching `(job_id, job_name, status, start_time, end_time_or_now)` for runs whose `end_time >= $since OR status = 'running'`.
- `templates/base.html` — new nav link "Timeline"
- Polling cadence: `hx-trigger="every 5s"` on the timeline partial only while the window contains running rows.

### 3.5 Sparkline + Success-Rate Badge on Dashboard Cards

**Recommendation:** a separate query `get_dashboard_job_sparks(job_ids: &[i64]) -> HashMap<i64, SparkData>` called once with all ids from `get_dashboard_jobs`. One extra round-trip, but the query is trivially parallelizable and avoids dialect subtleties from adding a CTE to the already-complex `get_dashboard_jobs`.

**Touches:**
- `src/db/queries.rs` — new `get_dashboard_job_sparks` function
- `src/web/handlers/dashboard.rs::to_view` — populate `success_rate: String` and `sparkline: Vec<&'static str>` (20 entries)
- `templates/partials/job_table.html` — render a fixed 20-cell inline SVG or `<span>` per status. Recommend pure HTML spans with CSS classes keyed to `cd-status-*` variables for consistency with the design system.
- Refresh: dashboard polling is already in place. No new polling.

### 3.6 Duration Trend p50/p95 on Job Detail

**The dialect split:** SQLite has no built-in `percentile_cont`; Postgres has it since 9.4. Three options:

- **Option A (recommended) — compute in Rust.** Fetch the last N `duration_ms` values (already indexed on `(job_id, start_time DESC)`), sort in-memory, pick indices `[0.5 * len]` and `[0.95 * len]`. For N=100 this is microseconds. Same code path works on both backends.
- Option B: Postgres-native + custom SQLite — more code, more tests, no payoff.

**Rationale for A:** cronduit's structural-parity constraint exists precisely to avoid dialect-specific code outside migration files.

**Touches:**
- `src/db/queries.rs::get_recent_durations(job_id, limit) -> Vec<i64>` (new, trivial one-column select; reuses existing index)
- `src/web/handlers/job_detail.rs` — call `get_recent_durations(job_id, 100)`, compute p50/p95 in a small helper
- Add `p50_display`, `p95_display` to `JobDetailView`
- `templates/pages/job_detail.html` — new stat card
- Optional: a `src/web/stats.rs` helper module holding `percentile(samples: &mut [i64], q: f64) -> i64` with its own tests. Recommended for testability.

### 3.7 Bulk Enable/Disable — the Open Design Question (RESOLVED)

**Reading** `src/scheduler/sync.rs::sync_config_to_db` L102–L216 and `src/db/queries.rs::disable_missing_jobs` L129–L169 confirms the semantics:

- **On every reload**, `sync_config_to_db` computes `active_names = [every job in the config file]` and calls `disable_missing_jobs(pool, &active_names)`, which sets `enabled=0` on every row whose name is NOT in that list.
- **On every upsert** (`queries::upsert_job` L57), the `ON CONFLICT DO UPDATE` clause **hardcodes `enabled = 1`**. So any job present in the config file gets re-enabled on every sync.

**Option (a) — edit `cronduit.toml` in place.** Rejected.
- Violates the v1 constraint "Config file mounted read-only".
- Would require the web process to own write access to an operator-managed file, expanding the blast radius against the documented threat model.
- Would create a config-write → file-watch → reload → sync loop that races the writer against itself.

**Option (b) — a DB-only flag.** The cleanest instantiation is **a new column `enabled_override` (nullable tri-state: NULL / 0 / 1) on `jobs`**, NOT repurposing `enabled`.

- Semantics:
  - `enabled_override IS NULL` → behavior follows the config file (current semantics preserved)
  - `enabled_override = 0` → forced disabled regardless of config
  - `enabled_override = 1` → forced enabled (not strictly required for v1.1; can be left for a future milestone)
- `get_enabled_jobs` becomes `WHERE enabled = 1 AND (enabled_override IS NULL OR enabled_override = 1)`.
- `disable_missing_jobs` continues to set `enabled = 0` on rows whose names left the config — no change.
- `upsert_job` continues to set `enabled = 1` — no change, because the new override column is untouched by config sync.
- **Does NOT break reload semantics**: a reload still correctly re-enables a job whose name reappears in the config file, and a reload still disables a job whose name vanishes. The override is orthogonal.
- **One new rule**: when the operator deletes a job from the config and the row then gets `enabled = 0`, we should also **clear** the `enabled_override` so a later re-add-from-config does the expected thing. One-line addition to `disable_missing_jobs`.

**Option (c) — separate `overrides.toml`.** Rejected.
- Creates a second source of truth that every reader (dashboard, scheduler, api) must consult.
- The write path for the bulk-disable UI would still need to modify this file, and the file watcher would trigger a reload loop.
- Bigger surface area than option (b), with no upside.

**Recommendation: Option (b).** Specifically:
1. New migration adds `enabled_override INTEGER` (SQLite) / `BIGINT` (Postgres), nullable, default NULL.
2. `get_enabled_jobs` and `get_dashboard_jobs` filter on the composite predicate above.
3. `disable_missing_jobs` clears the override when disabling.
4. New DB functions `set_enabled_override_bulk(job_ids: &[i64], override: Option<bool>)` and corresponding API handler `POST /api/jobs/bulk-toggle` (CSRF-gated, form post with multi-select `job_ids[]` + `action=enable|disable`).
5. **After the bulk update, the API handler must fire `SchedulerCmd::Reload`** so the scheduler rebuilds its heap and newly-disabled jobs stop firing. This requires **no scheduler change** — `do_reload` already fetches enabled jobs via `get_enabled_jobs`, so changing the filter is enough.
6. Dashboard partial gets checkboxes + a sticky action bar; template touches only.
7. Settings page should show the current overrides for operator visibility ("3 jobs forced-disabled: foo, bar, baz").

**Touches:**
- Migrations (both backends)
- `src/db/queries.rs`: `get_enabled_jobs`, `get_dashboard_jobs`, `disable_missing_jobs` (clear override on path), new `set_enabled_override_bulk`
- `src/web/handlers/api.rs`: new `bulk_toggle` handler
- `src/web/mod.rs::router`
- `templates/pages/dashboard.html` + `templates/partials/job_table.html`: checkboxes, action bar, CSRF token
- `templates/pages/settings.html`: optional override list display

**New/modified:** zero new modules; additions to queries, one new API handler, template changes, two migration files.

### 3.8 Docker Healthcheck "unhealthy" Bug (added mid-research)

**Symptom:** `docker ps` reports the cronduit container as `Up 2 hours (unhealthy)` while `curl http://localhost:8080/health` inside the container returns a correct 200 JSON response.

**Root cause hypothesis** (to be confirmed at fix time): busybox `wget --spider` in the alpine:3 base image misparses HTTP responses that axum sends with `Transfer-Encoding: chunked`. Axum's `Json` responder emits the body chunked when the content length isn't pre-computed; busybox `wget --spider` has historical issues with chunked responses that surface as `exit 1` even on 200 OK status.

**Fix-path recommendation** (phase-plan time picks the specific path):

1. **(Preferred) Ship a `cronduit health` CLI subcommand** — a new top-level clap subcommand that performs a local `GET http://$bind/health`, parses the JSON, and exits 0 on `status=ok`, 1 otherwise. Healthcheck becomes `CMD ["/cronduit", "health"]`. Self-hosted, zero external tool dependency, image stays small.
2. **Embed `HEALTHCHECK` in the Dockerfile** so the shipped image has a known-good default independent of the compose file the operator writes. Recommended regardless of which test expression wins.
3. **Change the axum health handler to force a known Content-Length** — return `Response::builder().header(CONTENT_LENGTH, body.len()).body(...)` instead of `Json(...)`. Fragile; doesn't help operators who use a different healthcheck tool in the future.

Option 1 + 2 together is the most cronduit-flavored solution. It closes an entire category of "operator's healthcheck tool behaves weirdly" bugs by making the healthcheck live inside the binary.

**Touches:**
- `src/cli/mod.rs` + `src/cli/health.rs` (new) — clap `HealthCmd` with `--bind` / `--config` args; calls the running server's `/health` over HTTP via `reqwest` or a minimal hand-rolled hyper client
- `Dockerfile` — add `HEALTHCHECK CMD ["/cronduit", "health"]`
- `examples/docker-compose.yml` — replace the wget healthcheck stanza with the CMD form that invokes `cronduit health`
- `examples/docker-compose.secure.yml` — same
- `README.md` — healthcheck troubleshooting section (if any)

**New/modified:** one new CLI subcommand module, Dockerfile addition, two compose-file updates.

**Phase placement:** rc.1 bug-fix block (SCHED/OPS category). Small and independent of the scheduler/log/run-number work.

---

## 4. Suggested Build Order

**Recommended: three rcs, in this order.**

### rc.1 — Bug-Fix Block (Phase A)

Spike-and-land the riskiest feature first.

1. **Stop-a-running-job** (§3.1) — riskiest, touches three executors. Spike it alone to de-risk the `RunControl` abstraction. Lands first because (a) every later feature can assume the `stopped` status exists, (b) if it's late, we can still cut a usable rc.1 with log-backfill only.
2. **Per-job run number** (§3.2) — lands next because the timeline view (§3.4), the run-history partial, and the sparkline (§3.5) all want to display `#N` instead of `#123456`. Landing this before observability polish means we only change the templates once.
3. **Log backfill + out-of-order fix + transient-error fix** (§3.3) — lands last in rc.1. Requires the `LogLine.id` shape change but touches only the SSE + run_detail path, so it can ship independently of the rest.
4. **Docker healthcheck fix** (§3.8) — independent, small, can land in parallel. Gate: rc.1 should ship with the healthcheck fixed so external adopters trying rc.1 don't immediately hit the `(unhealthy)` problem.

### rc.2 — Observability Polish (Phase B)

All three features share no code; any internal order works. Suggest:

5. **Duration trend p50/p95** (§3.6) — smallest, most mechanical. Warm-up.
6. **Sparkline + success-rate** (§3.5) — new query, template work.
7. **Timeline view** (§3.4) — new page, new query, new template. Largest in this block.

### rc.3 — Ergonomics (Phase C)

8. **Bulk enable/disable** (§3.7) — new migration, new handler, biggest template changes on the dashboard. Lands last because the selection UI is the most visible regression risk and we want a clean dashboard for rc.1 / rc.2 screenshots.

### Strict dependencies

- §3.2 (run numbers) must land before §3.4 (timeline) and §3.5 (sparkline) — otherwise you render `#{global_id}` in the new views and rewrite them later.
- §3.3 (log backfill) is independent of everything but benefits from landing early because every rc matters for operator experience.
- §3.1 (stop) is independent but is the highest-risk spike — land it first so late-breaking regressions have maximum fix time.
- §3.7 (bulk toggle) is independent and should land last because it changes the most visible surface.
- §3.8 (healthcheck) is fully independent; fit it wherever convenient in rc.1.

### Spike recommendation

Before cutting rc.1, do a short spike on §3.1 specifically validating:

- `RunControl` + `StopReason::Operator` round-trip on all three executors
- The race in §5.1 is covered by a test
- `mark_run_orphaned` / orphan-reconciliation still works when a run is `stopped` mid-execution and the process is killed

---

## 5. Integration Gotchas (→ Test Cases)

### 5.1 Stop-a-Job Races with Natural Completion

**The race:** `SchedulerCmd::Stop { run_id: 42 }` arrives in the scheduler loop at time T. At T+1μs, `join_set.join_next()` yields `RunResult { run_id: 42, status: "success" }` because the job finished naturally. The `Stop` handler then tries to cancel a token whose task is already gone, and — worse — the DB row is already `success`, so overwriting it to `stopped` would be a lie.

**Correct ordering:**

1. `Stop` handler first checks `running_handles.get(&run_id)`.
2. If present, it cancels the token AND **does NOT touch the DB.** The executor's cancel branch is the only place that writes `status = "stopped"`.
3. If absent, it either returns `StopResult::AlreadyFinished { final_status }` by reading `job_runs` by id, or returns `StopResult::Unknown` if not in `running_handles` and not in `job_runs`.
4. `run.rs::run_job`, when finalizing, must `finalize_run` first (takes the writer lock briefly), then `running_handles.write().await.remove(&run_id)`.

**Invariant:** the executor finalizes as `stopped` if and only if it observed `cancel.cancelled()` with `stop_reason = Operator`; otherwise the natural-completion status wins.

**Test cases:**

- Stop during a running job → DB row ends `stopped`, container removed.
- Stop arrives within 1ms of natural exit → DB row ends in the natural exit status (no `stopped` overwrite). Use `tokio::time::pause` to make the race deterministic.
- Stop for an unknown `run_id` → 404 from API handler, no DB touch.
- Stop for a completed `run_id` → handler responds `AlreadyFinished` toast, no DB touch.

### 5.2 Per-Job-Run-Number Backfill Races Startup

**The concern:** "backfill races the scheduler writing new rows." Reading `src/cli/run.rs` and `src/db/mod.rs::migrate` confirms the current ordering:

1. `DbPool::connect()` → `pool.migrate()` runs migrations (before scheduler spawn)
2. `docker_orphan::reconcile_orphans()` runs (before scheduler spawn)
3. `sync_config_to_db()` runs
4. `scheduler::spawn()` starts the loop — only now can new `insert_running_run` calls happen

So the backfill migration runs **strictly before** any new `job_runs` row is inserted by the running scheduler. **No race.** The only remaining concerns:

- The backfill migration must be idempotent (rerun safe) because migrations may be applied partially on crash.
- The `NOT NULL` constraint must be added in a SEPARATE migration after the backfill migration so a partial backfill doesn't leave an unsatisfiable constraint. Standard two-step-migration practice.
- On **very large** `job_runs` tables the backfill UPDATE could take seconds. Acceptable for a v1.1 upgrade.

**Test cases:**

- Start with an empty DB → run Insert → `job_run_number` is 1.
- Start with a populated DB (no column) → migrate → every row has a `job_run_number` matching `ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id)`.
- Start with a partially backfilled DB (simulating a crashed prior migration) → migrate → all nulls get filled.
- Backfill migration is rerun on an already-complete DB → no-op (idempotent).

### 5.3 SSE Backfill Dedupe

Covered in §3.3. Summary:

- **Mechanism:** assign `id` in `insert_log_batch`, propagate into `LogLine`, emit via SSE, JS-side drop events with `id <= max_backfill_id`.
- **Test cases:**
  - Navigate to a running job after 100 lines are persisted → see 100 lines, then live lines as they arrive, no duplicates at the transition.
  - Navigate during a log burst (100 lines persisted, 50 more in the broadcast ring) → see 100 from DB + 50 from SSE, none duplicated.
  - Navigate to a job with 10k lines → pagination still works for the static view (existing `has_older` / `next_offset` shape unchanged).

### 5.4 Bulk Disable + Running Jobs

**Concern:** operator bulk-disables 5 jobs, 2 of them are currently running. The `SchedulerCmd::Reload` that follows the bulk toggle will rebuild the heap WITHOUT those jobs — but the in-flight runs hold cloned `DbJob` values (see `do_reload` L60 comment) and will complete naturally. **This is the correct behavior**: bulk disable stops *future* fires, not running jobs. The operator who wants to stop a running job uses the Stop button from §3.1. Document this in the UI (toast: "3 jobs disabled; 2 currently-running jobs will complete").

### 5.5 `stopped` Status and Orphan Reconciliation

**Concern:** a `stopped` run leaves a container that's in the process of being force-killed. If cronduit crashes between `container_kill` and `finalize_run`, the orphan reconciler at next startup will find the container with `cronduit.run_id=42`, stop it (already stopped), remove it, and mark run 42 as `error`/`orphaned at restart`. That's wrong — the run was explicitly stopped, not orphaned.

**Fix:** change `mark_run_orphaned` to `UPDATE … WHERE id = $1 AND status = 'running'`. Strict improvement on today's behavior, orthogonal to v1.1's Stop feature. Test with: pre-seed a row `status = 'stopped'`, run reconciliation against a matching container, assert row remains `stopped`.

### 5.6 Healthcheck Fix Does Not Break Existing Deployments

**Concern:** operators may have custom healthcheck stanzas in their own compose files that use the old wget pattern. Fix must be backward-compatible.

**Resolution:**

- The Dockerfile `HEALTHCHECK` directive is a new default; operators who override it in their compose file keep their override (compose overrides win over Dockerfile).
- The `cronduit health` subcommand is additive — adding it to the CLI does not change any existing behavior.
- Docs should call out the new default + the recommended compose-file pattern so operators opt in.

---

## 6. Proposed New Modules / Files

| Path | Purpose | Scope |
|------|---------|-------|
| `src/scheduler/control.rs` | `RunControl`, `StopReason`, `StopResult` types + the `HashMap<run_id, RunControl>` extension | ~60 LOC |
| `src/web/handlers/timeline.rs` | Timeline handler + view model | ~120 LOC |
| `src/web/stats.rs` | `percentile()` helper with tests | ~40 LOC |
| `src/cli/health.rs` | `cronduit health` subcommand | ~60 LOC |
| `templates/pages/timeline.html` | Timeline page | ~80 LOC |
| `migrations/{sqlite,postgres}/20260415_000000_job_run_number.up.sql` | Add column | — |
| `migrations/{sqlite,postgres}/20260415_000001_job_run_number_backfill.up.sql` | Backfill | — |
| `migrations/{sqlite,postgres}/20260415_000002_job_run_number_not_null.up.sql` | Add NOT NULL | — |
| `migrations/{sqlite,postgres}/20260415_000010_enabled_override.up.sql` | Bulk disable column | — |

**No new modules beyond those.** Everything else is additions inside existing files.

---

## 7. What NOT to Change During v1.1

Explicit non-goals to keep the milestone tight and avoid the "refactor-while-we're-here" anti-pattern:

1. **Do not refactor the scheduler loop.** Add the `Stop` match arm; don't split the loop into functions.
2. **Do not change the writer/reader pool split.** Every query in §3 uses the existing `PoolRef::{Sqlite,Postgres}` pattern.
3. **Do not add a new templating library.** All new partials are askama templates extending `base.html`.
4. **Do not introduce a metrics registry change.** The `metrics` facade is already wired; new metrics (e.g. `cronduit_runs_stopped_total`) use the existing bounded-cardinality label scheme in `run.rs::classify_failure_reason`.
5. **Do not introduce a JS framework for the timeline or sparkline.** Plain inline HTML/CSS only. HTMX is already vendored.
6. **Do not change the config file schema.** All new state lives in the DB. (Per §3.7 resolution.)
7. **Do not rework `mark_run_orphaned`** beyond the targeted `status = 'running'` WHERE clause (§5.5).

---

## 8. Confidence Assessment

| Area | Level | Basis |
|------|-------|-------|
| File/module paths | HIGH | Every path verified by direct Read; line numbers included where nontrivial |
| Bulk-disable recommendation | HIGH | Derived from reading `sync.rs` + `queries.rs::disable_missing_jobs` + `queries.rs::upsert_job` end-to-end |
| Stop-a-job design | HIGH | Derived from reading the scheduler loop + all three executors; race analysis grounded in the existing `tokio::select!` structure |
| Per-job run number option B | HIGH | Derived from the existing `UPDATE … RETURNING` pattern already in `upsert_job` |
| Log backfill dedupe via id | MEDIUM-HIGH | Requires touching `log_pipeline::LogLine` and the log writer task — both are small and well-isolated, but the dedupe-via-id pattern isn't in the code today |
| p50/p95 in Rust | HIGH | Cronduit's structural-parity constraint explicitly rejects dialect-specific SQL outside migrations |
| Timeline as new page | MEDIUM | Design judgment; a reasonable alternative is a collapsible dashboard section |
| Build order | HIGH | Dependency graph derived from the templating/schema touches, not aesthetic |
| Orphan/stopped interaction | HIGH | Behavior confirmed by reading `docker_orphan.rs::mark_run_orphaned` |
| Healthcheck fix (§3.8) | MEDIUM | Root-cause hypothesis needs confirmation by reproducing the chunked-encoding busybox wget failure, but the `cronduit health` subcommand fix path is sound regardless of the exact root cause |

---

## 9. Notes for the Roadmap Consumer

- **Every v1.1 feature has a specific file/module path.** No hand-wavy "somewhere in the scheduler".
- **The bulk-disable open question is resolved:** Option (b), new `enabled_override` column, with justification derived from reading the sync code. Phase plan does not need to relitigate.
- **Build order is justified by strict dependencies**: run numbers before timeline/sparkline (template reuse), stop before everything (risk-front-load), log backfill independent.
- **Three release candidates map to the three thematic blocks**: rc.1 = bug-fix block (stop + run numbers + log backfill + healthcheck), rc.2 = observability (trend + sparkline + timeline), rc.3 = ergonomics (bulk toggle).
- **The one architectural pattern v1.1 introduces** that isn't in v1.0 is the `running_handles` map carrying `RunControl`. Keep it minimal and colocated in `src/scheduler/control.rs`.

---

## 10. Source Files Read

- `/Users/Robert/Code/public/cronduit/.planning/PROJECT.md`
- `src/scheduler/mod.rs`
- `src/scheduler/cmd.rs`
- `src/scheduler/run.rs`
- `src/scheduler/reload.rs`
- `src/scheduler/sync.rs`
- `src/scheduler/command.rs`
- `src/scheduler/docker.rs`
- `src/scheduler/docker_orphan.rs`
- `src/db/queries.rs` (structural scan + L40–L215, L286–L800)
- `src/web/mod.rs`
- `src/web/handlers/api.rs`
- `src/web/handlers/dashboard.rs`
- `src/web/handlers/job_detail.rs`
- `src/web/handlers/run_detail.rs`
- `src/web/handlers/sse.rs`
- `src/web/handlers/health.rs`
- `templates/pages/run_detail.html`
- `migrations/sqlite/20260410_000000_initial.up.sql`
- `migrations/postgres/20260410_000000_initial.up.sql`
- `Dockerfile`
- `examples/cronduit.toml`
- `examples/docker-compose.yml`
