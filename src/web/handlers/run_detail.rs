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
}

#[derive(Template)]
#[template(path = "partials/log_viewer.html")]
struct LogViewerPartial {
    run_id: i64,
    logs: Vec<LogLineView>,
    has_older: bool,
    next_offset: i64,
}

#[derive(Template)]
#[template(path = "partials/static_log_viewer.html")]
struct StaticLogViewerPartial {
    run_id: i64,
    logs: Vec<LogLineView>,
    total_logs: i64,
    has_older: bool,
    next_offset: i64,
}

// ---------------------------------------------------------------------------
// View models
// ---------------------------------------------------------------------------

pub struct RunDetailView {
    pub id: i64,
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
    let (logs, total_logs, has_older, next_offset, _last_log_id) =
        fetch_logs(&state.pool, run_id, offset).await;

    if is_htmx {
        LogViewerPartial {
            run_id,
            logs,
            has_older,
            next_offset,
        }
        .into_web_template()
        .into_response()
    } else {
        let status = run.status.to_lowercase();
        let status_label = status.to_uppercase();

        let run_view = RunDetailView {
            id: run.id,
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
        };

        let is_running = run_view.status == "running";

        let csrf_token = csrf::get_token_from_cookies(&cookies);

        RunDetailPage {
            run: run_view,
            run_id,
            is_running,
            logs,
            total_logs,
            has_older,
            next_offset,
            csrf_token,
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
    let (logs, _total, has_older, next_offset, _last_log_id) =
        fetch_logs(&state.pool, run_id, offset).await;

    LogViewerPartial {
        run_id,
        logs,
        has_older,
        next_offset,
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
    let (logs, total_logs, has_older, next_offset, _last_log_id) =
        fetch_logs(&state.pool, run_id, 0).await;

    StaticLogViewerPartial {
        run_id,
        logs,
        total_logs,
        has_older,
        next_offset,
    }
    .into_web_template()
    .into_response()
}
