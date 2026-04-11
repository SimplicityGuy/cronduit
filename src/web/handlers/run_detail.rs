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
    logs: Vec<LogLineView>,
    total_logs: i64,
    has_older: bool,
    next_offset: i64,
}

#[derive(Template)]
#[template(path = "partials/log_viewer.html")]
struct LogViewerPartial {
    run_id: i64,
    logs: Vec<LogLineView>,
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
async fn fetch_logs(
    pool: &crate::db::DbPool,
    run_id: i64,
    offset: i64,
) -> (Vec<LogLineView>, i64, bool, i64) {
    let log_result = queries::get_log_lines(pool, run_id, LOG_PAGE_SIZE, offset)
        .await
        .unwrap_or(queries::Paginated {
            items: vec![],
            total: 0,
        });

    let has_older = (offset + LOG_PAGE_SIZE) < log_result.total;
    let next_offset = offset + LOG_PAGE_SIZE;
    let total = log_result.total;

    let logs: Vec<LogLineView> = log_result
        .items
        .into_iter()
        .map(|l| {
            let is_stderr = l.stream == "stderr";
            LogLineView {
                stream: l.stream,
                is_stderr,
                ts: l.ts,
                html: ansi::render_log_line(&l.line),
            }
        })
        .collect();

    (logs, total, has_older, next_offset)
}

pub async fn run_detail(
    HxRequest(is_htmx): HxRequest,
    State(state): State<AppState>,
    Path((_job_id, run_id)): Path<(i64, i64)>,
    Query(params): Query<LogPaginationParams>,
) -> impl IntoResponse {
    let run = match queries::get_run_by_id(&state.pool, run_id).await {
        Ok(Some(run)) => run,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let offset = params.offset.max(0);
    let (logs, total_logs, has_older, next_offset) = fetch_logs(&state.pool, run_id, offset).await;

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

        RunDetailPage {
            run: run_view,
            run_id,
            logs,
            total_logs,
            has_older,
            next_offset,
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
    let (logs, _total, has_older, next_offset) = fetch_logs(&state.pool, run_id, offset).await;

    LogViewerPartial {
        run_id,
        logs,
        has_older,
        next_offset,
    }
    .into_web_template()
    .into_response()
}
