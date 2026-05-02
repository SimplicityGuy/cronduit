//! Run Detail page and log viewer partial (UI-09, D-05, D-07).

use askama::Template;
use askama_web::WebTemplateExt;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum_htmx::HxRequest;
use serde::Deserialize;

use crate::db::queries;
use crate::web::AppState;
use crate::web::ansi;
use crate::web::csrf;
use crate::web::format::format_duration_ms;

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
pub struct LogPaginationParams {
    #[serde(default)]
    pub offset: i64,
}

const LOG_PAGE_SIZE: i64 = 500;

// ---------------------------------------------------------------------------
// Askama templates
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "pages/run_detail.html")]
struct RunDetailPage {
    run: RunDetailView,
    run_id: i64,
    is_running: bool,
    logs: Vec<LogLineView>,
    total_logs: i64,
    has_older: bool,
    next_offset: i64,
    csrf_token: String,
    /// Max `job_logs.id` across the server-rendered backfill (0 when empty).
    /// Rendered into `run_detail.html` via `data-max-id="{{ last_log_id }}"`
    /// on `#log-lines` so the client-side dedupe handler (Plan 11-11) can
    /// compare SSE `event.lastEventId` against it and skip already-rendered
    /// lines on reconnect (D-08 / D-09).
    last_log_id: i64,
    /// Phase 21 FCTX-01: panel visibility gate. True iff
    /// `run.status ∈ {failed, timeout}` AND `get_failure_context()` succeeded.
    /// Soft-fails to false on DB error (D-12) so the rest of the page renders.
    /// Fields tolerated as `dead_code` until plan 21-06 lands the askama
    /// template insert that consumes them.
    #[allow(dead_code)]
    show_fctx_panel: bool,
    /// Phase 21 FCTX-01..06: pre-formatted view-model for the failure-context
    /// panel. `None` when `show_fctx_panel` is false. The askama template
    /// substitutes `{{ value }}` with zero logic — every conditional copy
    /// rendering happens in `build_fctx_view` per UI-SPEC § Copywriting Contract.
    #[allow(dead_code)]
    fctx: Option<FctxView>,
}

#[derive(Template)]
#[template(path = "partials/log_viewer.html")]
struct LogViewerPartial {
    run_id: i64,
    logs: Vec<LogLineView>,
    has_older: bool,
    next_offset: i64,
    /// Max `job_logs.id` across this partial's log page — see RunDetailPage.
    /// Rendered into the log-viewer scaffolding when the partial is used as
    /// the running-run first paint include; harmless when used for pagination.
    #[allow(dead_code)]
    last_log_id: i64,
}

#[derive(Template)]
#[template(path = "partials/static_log_viewer.html")]
struct StaticLogViewerPartial {
    run_id: i64,
    logs: Vec<LogLineView>,
    total_logs: i64,
    has_older: bool,
    next_offset: i64,
    /// Max `job_logs.id` across this partial's log page. Rendered into
    /// `static_log_viewer.html` via `data-max-id="{{ last_log_id }}"` on
    /// `#log-lines`. Present even for terminal runs (no SSE) so the cursor
    /// stays consistent if/when a post-terminal stream attaches.
    last_log_id: i64,
}

// ---------------------------------------------------------------------------
// View models
// ---------------------------------------------------------------------------

pub struct RunDetailView {
    pub id: i64,
    /// Per-job sequential run number (Phase 11 DB-11 / UI-16). Rendered as
    /// the primary `Run #N` identifier in the title, breadcrumb, and header;
    /// global `id` remains the URL key and appears as a muted `(id N)` suffix
    /// per D-05.
    pub job_run_number: i64,
    pub job_id: i64,
    pub job_name: String,
    pub status: String,
    pub status_label: String,
    pub trigger: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_display: String,
    pub exit_code: Option<i32>,
    pub error_message: Option<String>,
    /// Phase 16 FOUND-14: image digest captured post-start by
    /// `inspect_container`. `None` for command/script jobs (no image) and for
    /// pre-v1.2 docker rows. Consumed by Phase 21 FCTX panel IMAGE DIGEST row
    /// (`build_fctx_view`); the askama template never reads it directly.
    pub image_digest: Option<String>,
    /// Phase 16 FCTX-04: per-run config_hash captured at fire time by
    /// `insert_running_run`. `None` on pre-v1.2 rows. Consumed by Phase 21
    /// FCTX panel CONFIG row via the literal `run.config_hash !=
    /// last_success.config_hash` compare per D-14.
    pub config_hash: Option<String>,
    /// Phase 21 FCTX-06 (D-02 / D-04): fire-decision-time RFC3339 timestamp.
    /// `None` on pre-v1.2 rows that landed before migration
    /// `*_000009_scheduled_for_add.up.sql` AND on legacy scheduler-RunNow
    /// fallback paths. The FIRE SKEW row hides on `None` per UI-SPEC.
    pub scheduled_for: Option<String>,
}

/// Phase 21 failure-context panel pre-formatted view-model (research §H,
/// 11 fields LOCKED). Every conditional copy rendering happens in
/// `build_fctx_view` per UI-SPEC § Copywriting Contract — the askama template
/// substitutes `{{ value }}` and carries zero logic.
///
/// Field gating summary:
/// - `consecutive_failures` / `summary_meta` — always populated when the
///   panel renders (FCTX-02 / streak summary).
/// - `last_success_run_id` / `last_success_run_url` — `None` when the job
///   has never succeeded (D-13).
/// - `time_deltas_value` — locked copy varies on `last_success_run_id`
///   presence per UI-SPEC § Copywriting Contract.
/// - `is_docker_job` / `image_digest_value` — IMAGE DIGEST row hides when
///   `is_docker_job=false` (FCTX-03) or when never-succeeded (D-13);
///   `image_digest_value=None` triggers the hide.
/// - `config_changed_value` — `None` on never-succeeded (D-13); literal
///   compare per D-14 otherwise.
/// - `has_duration_samples` / `duration_value` — DURATION row gated to
///   `N >= 5` successful samples per UI-SPEC FCTX-05 (NOT 20).
/// - `fire_skew_value` — `None` when `scheduled_for IS NULL` per D-04.
///
/// Field declaration order matches research §H verbatim.
#[allow(dead_code)] // consumed by plan 21-06 (template insert + CSS)
pub struct FctxView {
    pub consecutive_failures: i64,
    pub summary_meta: String,
    pub last_success_run_id: Option<i64>,
    pub time_deltas_value: String,
    pub last_success_run_url: Option<String>,
    pub is_docker_job: bool,
    pub image_digest_value: Option<String>,
    pub config_changed_value: Option<String>,
    pub has_duration_samples: bool,
    pub duration_value: Option<String>,
    pub fire_skew_value: Option<String>,
}

pub struct LogLineView {
    /// `job_logs.id` of the persisted row. Populated from `DbLogLine.id`
    /// so the handler's `last_log_id` computation (D-08) has an authoritative
    /// per-line identifier for the template's `data-max-id` attribute and for
    /// the client-side dedupe contract (D-09) landing in Plan 11-11.
    pub id: i64,
    pub stream: String,
    pub is_stderr: bool,
    pub ts: String,
    pub html: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// Fetch log lines and build view models (shared by both handlers).
///
/// Returns a 5-tuple: `(logs, total, has_older, next_offset, last_log_id)`.
/// `last_log_id` is the max `job_logs.id` across the fetched page (0 when the
/// page is empty) — it flows into the template's `data-max-id` attribute so
/// the client-side dedupe (Plan 11-11) can compare live-stream
/// `event.lastEventId` against it. Uses the existing `queries::get_log_lines`
/// (src/db/queries.rs:844) unchanged — no new query helper added.
async fn fetch_logs(
    pool: &crate::db::DbPool,
    run_id: i64,
    offset: i64,
) -> (Vec<LogLineView>, i64, bool, i64, i64) {
    let log_result = match queries::get_log_lines(pool, run_id, LOG_PAGE_SIZE, offset).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(target: "cronduit.web", run_id, error = %e, "failed to fetch log lines");
            queries::Paginated {
                items: vec![],
                total: 0,
            }
        }
    };

    let has_older = (offset + LOG_PAGE_SIZE) < log_result.total;
    let next_offset = offset + LOG_PAGE_SIZE;
    let total = log_result.total;

    let logs: Vec<LogLineView> = log_result
        .items
        .into_iter()
        .map(|l| {
            let is_stderr = l.stream == "stderr";
            LogLineView {
                id: l.id,
                stream: l.stream,
                is_stderr,
                ts: l.ts,
                html: ansi::render_log_line(&l.line),
            }
        })
        .collect();

    let last_log_id = logs.iter().map(|l| l.id).max().unwrap_or(0);

    (logs, total, has_older, next_offset, last_log_id)
}

pub async fn run_detail(
    HxRequest(is_htmx): HxRequest,
    State(state): State<AppState>,
    Path((_job_id, run_id)): Path<(i64, i64)>,
    Query(params): Query<LogPaginationParams>,
    cookies: axum_extra::extract::CookieJar,
) -> impl IntoResponse {
    let run = match queries::get_run_by_id(&state.pool, run_id).await {
        Ok(Some(run)) => run,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let offset = params.offset.max(0);
    let (logs, total_logs, has_older, next_offset, last_log_id) =
        fetch_logs(&state.pool, run_id, offset).await;

    if is_htmx {
        LogViewerPartial {
            run_id,
            logs,
            has_older,
            next_offset,
            last_log_id,
        }
        .into_web_template()
        .into_response()
    } else {
        let status = run.status.to_lowercase();
        let status_label = status.to_uppercase();

        let run_view = RunDetailView {
            id: run.id,
            job_run_number: run.job_run_number,
            job_id: run.job_id,
            job_name: run.job_name,
            status,
            status_label,
            trigger: run.trigger,
            start_time: run.start_time,
            end_time: run.end_time.unwrap_or_else(|| "still running".to_string()),
            duration_display: format_duration_ms(run.duration_ms),
            exit_code: run.exit_code,
            error_message: run.error_message,
            // Phase 16 + 21: pass-through from DbRunDetail. Populated for the
            // FCTX panel (build_fctx_view in plan 21-04 task 2/3); the template
            // does not read these fields directly.
            image_digest: run.image_digest,
            config_hash: run.config_hash,
            scheduled_for: run.scheduled_for,
        };

        let is_running = run_view.status == "running";

        let csrf_token = csrf::get_token_from_cookies(&cookies);

        // Phase 21 FCTX wire-up lands in plan 21-04 task 2 (gating + soft-fail)
        // and task 3 (build_fctx_view). Task 1 ships the struct shape only;
        // the panel stays hidden on every render until task 2 wires the gate.
        let show_fctx_panel = false;
        let fctx: Option<FctxView> = None;

        RunDetailPage {
            run: run_view,
            run_id,
            is_running,
            logs,
            total_logs,
            has_older,
            next_offset,
            csrf_token,
            last_log_id,
            show_fctx_panel,
            fctx,
        }
        .into_web_template()
        .into_response()
    }
}

/// HTMX partial handler for log viewer pagination (single run_id path param).
pub async fn log_viewer_partial(
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
    Query(params): Query<LogPaginationParams>,
) -> impl IntoResponse {
    let offset = params.offset.max(0);
    let (logs, _total, has_older, next_offset, last_log_id) =
        fetch_logs(&state.pool, run_id, offset).await;

    LogViewerPartial {
        run_id,
        logs,
        has_older,
        next_offset,
        last_log_id,
    }
    .into_web_template()
    .into_response()
}

/// HTMX partial handler that returns the static log viewer for a completed run.
/// Used by the SSE `run_complete` event to swap from live to static view (D-04).
pub async fn static_log_partial(
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
) -> impl IntoResponse {
    let (logs, total_logs, has_older, next_offset, last_log_id) =
        fetch_logs(&state.pool, run_id, 0).await;

    StaticLogViewerPartial {
        run_id,
        logs,
        total_logs,
        has_older,
        next_offset,
        last_log_id,
    }
    .into_web_template()
    .into_response()
}
