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
}

// ---------------------------------------------------------------------------
// View models
// ---------------------------------------------------------------------------

pub struct JobDetailView {
    pub id: i64,
    pub name: String,
    pub schedule: String,
    pub resolved_schedule: String,
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

/// Format duration in milliseconds to human-readable string.
fn format_duration_ms(ms: Option<i64>) -> String {
    match ms {
        Some(ms) if ms < 1000 => format!("{ms}ms"),
        Some(ms) if ms < 60_000 => format!("{:.1}s", ms as f64 / 1000.0),
        Some(ms) if ms < 3_600_000 => {
            let mins = ms / 60_000;
            let secs = (ms % 60_000) / 1000;
            format!("{mins}m {secs}s")
        }
        Some(ms) => {
            let hours = ms / 3_600_000;
            let mins = (ms % 3_600_000) / 60_000;
            format!("{hours}h {mins}m")
        }
        None => "-".to_string(),
    }
}

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

    if is_htmx {
        RunHistoryPartial {
            job_id,
            runs,
            total_runs: run_result.total,
            page,
            total_pages,
        }
        .into_web_template()
        .into_response()
    } else {
        // Compute cron description using croner
        let cron_description = croner::Cron::from_str(&job.resolved_schedule)
            .map(|c| c.describe())
            .unwrap_or_else(|_| "Invalid schedule".to_string());

        let job_view = JobDetailView {
            id: job.id,
            name: job.name,
            schedule: job.schedule.clone(),
            resolved_schedule: job.resolved_schedule.clone(),
            job_type: job.job_type.clone(),
            config_json: pretty_json(&job.config_json),
            cron_description,
            timeout_display: format_timeout(job.timeout_secs),
        };

        let csrf_token = hex::encode(rand::random::<[u8; 16]>());

        JobDetailPage {
            job: job_view,
            job_id,
            runs,
            total_runs: run_result.total,
            page,
            total_pages,
            csrf_token,
        }
        .into_web_template()
        .into_response()
    }
}

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

    #[test]
    fn test_format_timeout() {
        assert_eq!(format_timeout(0), "none");
        assert_eq!(format_timeout(30), "30s");
        assert_eq!(format_timeout(90), "1m 30s");
        assert_eq!(format_timeout(3600), "1h");
        assert_eq!(format_timeout(5400), "1h 30m");
    }
}
