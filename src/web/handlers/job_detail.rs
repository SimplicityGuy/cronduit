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
use crate::web::format::format_duration_ms_floor_seconds;
use crate::web::stats;

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
    /// CSRF token threaded into per-row Stop forms; re-rendered on every 2s
    /// poll so the browser always has a fresh token paired with its cookie.
    csrf_token: String,
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
    pub duration: DurationView,
}

/// View model for the Duration card on the Job Detail page (Phase 13 OBS-04).
///
/// Populated from `queries::get_recent_successful_durations` (last 100 runs
/// with `status = 'success'` only, per D-20) and folded through
/// `crate::web::stats::percentile` + `format_duration_ms_floor_seconds`.
///
/// Sample-size gating lives here per D-21: when `sample_count < 20`, both
/// p50/p95 display values are `"—"` (em dash, U+2014) and `has_min_samples`
/// is `false`; the template renders the `title="insufficient samples ..."`
/// hover copy only when `has_min_samples` is false. The subtitle matrix
/// (D-18) is pre-formatted into `sample_count_display` so the template holds
/// no logic beyond substitution.
pub struct DurationView {
    /// "1m 34s" when has_min_samples=true, else "—" (em dash, U+2014).
    pub p50_display: String,
    pub p95_display: String,
    /// True iff sample_count >= MIN_SAMPLES_FOR_PERCENTILE (20).
    pub has_min_samples: bool,
    /// Raw count of successful runs considered (0..=100).
    pub sample_count: usize,
    /// Subtitle text per UI-SPEC § Duration card subtitle matrix.
    pub sample_count_display: String,
}

pub struct RunHistoryView {
    pub id: i64,
    /// Per-job sequential run number (Phase 11 DB-11 / UI-16). Rendered as
    /// the leftmost `#N` cell in `run_history.html`; global `id` remains the
    /// URL key and the row-level hover tooltip (D-13 permalink scheme).
    pub job_run_number: i64,
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
                job_run_number: r.job_run_number,
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
        let csrf_token = csrf::get_token_from_cookies(&cookies);
        RunHistoryPartial {
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
    } else {
        // Compute cron description using croner
        let cron_description = croner::Cron::from_str(&job.resolved_schedule)
            .map(|c| c.describe())
            .unwrap_or_else(|_| "Invalid schedule".to_string());

        let has_random_schedule = job.schedule.split_whitespace().any(|f| f == "@random");

        // Duration card hydration (Phase 13 OBS-04) --------------------------
        //
        // Per D-19/D-20/D-21 the consumer enforces the N<20 threshold BEFORE
        // calling `percentile()`; the helper itself has no threshold logic.
        // Query caps at 100 rows (D-18 subtitle cap), only `status='success'`
        // contributes (D-20), durations are formatted via
        // `format_duration_ms_floor_seconds` so the Duration card emits `42s`
        // (not `42.0s`) per the UI-SPEC copywriting contract.
        const MIN_SAMPLES_FOR_PERCENTILE: usize = 20;
        const PERCENTILE_SAMPLE_LIMIT: i64 = 100;

        let durations =
            queries::get_recent_successful_durations(&state.pool, job_id, PERCENTILE_SAMPLE_LIMIT)
                .await
                .unwrap_or_default();

        let sample_count = durations.len();
        let has_min = sample_count >= MIN_SAMPLES_FOR_PERCENTILE;

        let (p50_display, p95_display) = if has_min {
            let p50 = stats::percentile(&durations, 0.5)
                .expect("non-empty when sample_count >= MIN_SAMPLES_FOR_PERCENTILE (D-21)");
            let p95 = stats::percentile(&durations, 0.95)
                .expect("non-empty when sample_count >= MIN_SAMPLES_FOR_PERCENTILE (D-21)");
            (
                format_duration_ms_floor_seconds(Some(p50 as i64)),
                format_duration_ms_floor_seconds(Some(p95 as i64)),
            )
        } else {
            ("—".to_string(), "—".to_string())
        };

        let sample_count_display = match sample_count {
            0 => "0 of 20 successful runs required".to_string(),
            1..=19 => format!("{sample_count} of 20 successful runs required"),
            20..=99 => format!("last {sample_count} successful runs"),
            _ => "last 100 successful runs".to_string(),
        };

        let duration_view = DurationView {
            p50_display,
            p95_display,
            has_min_samples: has_min,
            sample_count,
            sample_count_display,
        };

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
            duration: duration_view,
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
    cookies: axum_extra::extract::CookieJar,
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
                job_run_number: r.job_run_number,
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

    let csrf_token = csrf::get_token_from_cookies(&cookies);

    RunHistoryPartial {
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
