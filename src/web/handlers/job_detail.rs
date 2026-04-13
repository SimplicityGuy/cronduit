//! Job Detail page and run history partial (UI-08).

use askama::Template;
use askama_web::WebTemplateExt;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum_htmx::HxRequest;
use serde::Deserialize;
use std::str::FromStr;

use crate::db::queries;
use crate::web::AppState;
use crate::web::csrf;
use crate::web::format::format_duration_ms;

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: i64,
}

fn default_page() -> i64 {
    1
}

const RUNS_PER_PAGE: i64 = 25;

// ---------------------------------------------------------------------------
// Askama templates
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "pages/job_detail.html")]
struct JobDetailPage {
    job: JobDetailView,
    job_id: i64,
    runs: Vec<RunHistoryView>,
    total_runs: i64,
    page: i64,
    total_pages: i64,
    any_running: bool,
    csrf_token: String,
}

#[derive(Template)]
#[template(path = "partials/run_history.html")]
struct RunHistoryPartial {
    job_id: i64,
    runs: Vec<RunHistoryView>,
    total_runs: i64,
    page: i64,
    total_pages: i64,
    /// When true, the wrapper carries `hx-trigger="every 2s"` so the Run History
    /// card auto-refreshes while any run is in the RUNNING state. Once all runs
    /// are terminal (SUCCESS/FAILED/TIMEOUT/CANCELLED), the wrapper re-renders
    /// without the trigger and polling stops naturally via HTMX outerHTML swap.
    any_running: bool,
}

// ---------------------------------------------------------------------------
// View models
// ---------------------------------------------------------------------------

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

pub struct RunHistoryView {
    pub id: i64,
    pub status: String,
    pub status_label: String,
    pub trigger: String,
    pub start_time: String,
    pub duration_display: String,
    pub exit_code: Option<i32>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format timeout seconds to human-readable string.
fn format_timeout(secs: i64) -> String {
    if secs <= 0 {
        return "none".to_string();
    }
    if secs < 60 {
        return format!("{secs}s");
    }
    if secs < 3600 {
        let m = secs / 60;
        let s = secs % 60;
        if s > 0 {
            return format!("{m}m {s}s");
        }
        return format!("{m}m");
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if m > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{h}h")
    }
}

/// Pretty-print JSON config for display.
fn pretty_json(raw: &str) -> String {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| raw.to_string())
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

pub async fn job_detail(
    HxRequest(is_htmx): HxRequest,
    State(state): State<AppState>,
    Path(job_id): Path<i64>,
    Query(params): Query<PaginationParams>,
    cookies: axum_extra::extract::CookieJar,
) -> impl IntoResponse {
    let job = match queries::get_job_by_id(&state.pool, job_id).await {
        Ok(Some(job)) => job,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let page = params.page.max(1);
    let offset = (page - 1) * RUNS_PER_PAGE;

    let run_result = queries::get_run_history(&state.pool, job_id, RUNS_PER_PAGE, offset)
        .await
        .unwrap_or(queries::Paginated {
            items: vec![],
            total: 0,
        });

    let total_pages = ((run_result.total as f64) / RUNS_PER_PAGE as f64).ceil() as i64;
    let total_pages = total_pages.max(1);

    let runs: Vec<RunHistoryView> = run_result
        .items
        .into_iter()
        .map(|r| {
            let status = r.status.to_lowercase();
            let status_label = status.to_uppercase();
            RunHistoryView {
                id: r.id,
                status,
                status_label,
                trigger: r.trigger,
                start_time: r.start_time,
                duration_display: format_duration_ms(r.duration_ms),
                exit_code: r.exit_code,
            }
        })
        .collect();

    let any_running = runs.iter().any(|r| r.status == "running");

    if is_htmx {
        RunHistoryPartial {
            job_id,
            runs,
            total_runs: run_result.total,
            page,
            total_pages,
            any_running,
        }
        .into_web_template()
        .into_response()
    } else {
        // Compute cron description using croner
        let cron_description = croner::Cron::from_str(&job.resolved_schedule)
            .map(|c| c.describe())
            .unwrap_or_else(|_| "Invalid schedule".to_string());

        let has_random_schedule = job.schedule.split_whitespace().any(|f| f == "@random");

        let job_view = JobDetailView {
            id: job.id,
            name: job.name,
            schedule: job.schedule.clone(),
            resolved_schedule: job.resolved_schedule.clone(),
            has_random_schedule,
            job_type: job.job_type.clone(),
            config_json: pretty_json(&job.config_json),
            cron_description,
            timeout_display: format_timeout(job.timeout_secs),
        };

        let csrf_token = csrf::get_token_from_cookies(&cookies);

        JobDetailPage {
            job: job_view,
            job_id,
            runs,
            total_runs: run_result.total,
            page,
            total_pages,
            any_running,
            csrf_token,
        }
        .into_web_template()
        .into_response()
    }
}

/// GET /partials/jobs/:job_id/runs
///
/// Always renders the Run History partial (regardless of HX-Request header),
/// so HTMX can poll this URL for live updates of run status transitions. The
/// partial response carries `hx-trigger="every 2s"` on its wrapper element
/// only while at least one run is in the RUNNING state, so an idle Job Detail
/// page does not poll indefinitely (closes the Phase 6 UAT issue filed against
/// Test 4 of `.planning/phases/06-.../06-UAT.md`).
pub async fn job_runs_partial(
    State(state): State<AppState>,
    Path(job_id): Path<i64>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    // Ensure the job exists — return 404 for unknown IDs so polling against a
    // deleted job cleanly terminates rather than rendering an empty table.
    match queries::get_job_by_id(&state.pool, job_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let page = params.page.max(1);
    let offset = (page - 1) * RUNS_PER_PAGE;

    let run_result = queries::get_run_history(&state.pool, job_id, RUNS_PER_PAGE, offset)
        .await
        .unwrap_or(queries::Paginated {
            items: vec![],
            total: 0,
        });

    let total_pages = ((run_result.total as f64) / RUNS_PER_PAGE as f64).ceil() as i64;
    let total_pages = total_pages.max(1);

    let runs: Vec<RunHistoryView> = run_result
        .items
        .into_iter()
        .map(|r| {
            let status = r.status.to_lowercase();
            let status_label = status.to_uppercase();
            RunHistoryView {
                id: r.id,
                status,
                status_label,
                trigger: r.trigger,
                start_time: r.start_time,
                duration_display: format_duration_ms(r.duration_ms),
                exit_code: r.exit_code,
            }
        })
        .collect();

    let any_running = runs.iter().any(|r| r.status == "running");

    RunHistoryPartial {
        job_id,
        runs,
        total_runs: run_result.total,
        page,
        total_pages,
        any_running,
    }
    .into_web_template()
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timeout() {
        assert_eq!(format_timeout(0), "none");
        assert_eq!(format_timeout(30), "30s");
        assert_eq!(format_timeout(90), "1m 30s");
        assert_eq!(format_timeout(3600), "1h");
        assert_eq!(format_timeout(5400), "1h 30m");
    }
}
