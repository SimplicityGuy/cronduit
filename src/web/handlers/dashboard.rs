//! Dashboard page and HTMX job-table partial (UI-06, UI-07, UI-13).

use askama::Template;
use askama_web::WebTemplateExt;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum_htmx::HxRequest;
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::Deserialize;
use std::str::FromStr;

use crate::db::queries::{self, DashboardJob};
use crate::web::AppState;
use crate::web::csrf;

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
pub struct DashboardParams {
    #[serde(default)]
    pub filter: String,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default = "default_order")]
    pub order: String,
}

fn default_sort() -> String {
    "name".to_string()
}
fn default_order() -> String {
    "asc".to_string()
}

// ---------------------------------------------------------------------------
// Askama templates
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "pages/dashboard.html")]
struct DashboardPage {
    jobs: Vec<DashboardJobView>,
    filter: String,
    sort: String,
    order: String,
    config_path: String,
    csrf_token: String,
}

#[derive(Template)]
#[template(path = "partials/job_table.html")]
struct JobTablePartial {
    jobs: Vec<DashboardJobView>,
    csrf_token: String,
}

// ---------------------------------------------------------------------------
// View model
// ---------------------------------------------------------------------------

pub struct DashboardJobView {
    pub id: i64,
    pub name: String,
    pub schedule: String,
    pub resolved_schedule: String,
    pub next_fire: String,
    pub last_status: String,
    pub last_status_label: String,
    pub last_run_relative: String,
}

fn to_view(job: DashboardJob, tz: Tz) -> DashboardJobView {
    let now = Utc::now();
    let now_tz = now.with_timezone(&tz);

    // Compute next fire time using croner
    let next_fire = match croner::Cron::from_str(&job.resolved_schedule) {
        Ok(cron) => match cron.find_next_occurrence(&now_tz, false) {
            Ok(next) => format_relative_future(next.with_timezone(&Utc), now),
            Err(_) => "unknown".to_string(),
        },
        Err(_) => "invalid".to_string(),
    };

    // Normalize last_status for CSS class matching (lowercase)
    let last_status = job.last_status.as_deref().unwrap_or("").to_lowercase();

    let last_status_label = if last_status.is_empty() {
        String::new()
    } else {
        last_status.to_uppercase()
    };

    // Compute relative time for last run
    let last_run_relative = match &job.last_run_time {
        Some(ts) => {
            // Try parsing as RFC 3339 / ISO 8601
            match DateTime::parse_from_rfc3339(ts) {
                Ok(dt) => format_relative_past(dt.with_timezone(&Utc), now),
                Err(_) => {
                    // Try parsing as naive datetime (SQLite format: "2026-04-10 12:34:56")
                    match chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S") {
                        Ok(ndt) => {
                            let dt = ndt.and_utc();
                            format_relative_past(dt, now)
                        }
                        Err(_) => ts.clone(),
                    }
                }
            }
        }
        None => "never".to_string(),
    };

    DashboardJobView {
        id: job.id,
        name: job.name,
        schedule: job.schedule,
        resolved_schedule: job.resolved_schedule,
        next_fire,
        last_status,
        last_status_label,
        last_run_relative,
    }
}

/// Format a future datetime as relative time (e.g., "in 4h 12m", "in 30s").
fn format_relative_future(target: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let diff = target.signed_duration_since(now);
    let total_secs = diff.num_seconds().max(0);
    format_duration_relative(total_secs, "in ")
}

/// Format a past datetime as relative time (e.g., "2m ago", "3h ago").
fn format_relative_past(target: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let diff = now.signed_duration_since(target);
    let total_secs = diff.num_seconds().max(0);
    if total_secs == 0 {
        return "just now".to_string();
    }
    format_duration_relative(total_secs, "") + " ago"
}

fn format_duration_relative(total_secs: i64, prefix: &str) -> String {
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if days > 0 {
        if hours > 0 {
            format!("{prefix}{days}d {hours}h")
        } else {
            format!("{prefix}{days}d")
        }
    } else if hours > 0 {
        if minutes > 0 {
            format!("{prefix}{hours}h {minutes}m")
        } else {
            format!("{prefix}{hours}h")
        }
    } else if minutes > 0 {
        format!("{prefix}{minutes}m")
    } else {
        format!("{prefix}{secs}s")
    }
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

pub async fn dashboard(
    HxRequest(is_htmx): HxRequest,
    State(state): State<AppState>,
    Query(params): Query<DashboardParams>,
    cookies: axum_extra::extract::CookieJar,
) -> impl IntoResponse {
    let filter = if params.filter.is_empty() {
        None
    } else {
        Some(params.filter.as_str())
    };
    let jobs = queries::get_dashboard_jobs(&state.pool, filter, &params.sort, &params.order)
        .await
        .unwrap_or_default();

    let tz: Tz = state.tz;
    let job_views: Vec<DashboardJobView> = jobs.into_iter().map(|j| to_view(j, tz)).collect();

    let csrf_token = csrf::get_token_from_cookies(&cookies);

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_relative_future() {
        let now = Utc::now();
        let target = now + chrono::Duration::hours(4) + chrono::Duration::minutes(12);
        let result = format_relative_future(target, now);
        assert!(result.starts_with("in 4h 12m"), "got: {result}");
    }

    #[test]
    fn test_format_relative_past() {
        let now = Utc::now();
        let target = now - chrono::Duration::minutes(2);
        let result = format_relative_past(target, now);
        assert_eq!(result, "2m ago");
    }

    #[test]
    fn test_format_relative_past_days() {
        let now = Utc::now();
        let target = now - chrono::Duration::days(3) - chrono::Duration::hours(5);
        let result = format_relative_past(target, now);
        assert_eq!(result, "3d 5h ago");
    }

    #[test]
    fn test_format_relative_past_just_now() {
        let now = Utc::now();
        let result = format_relative_past(now, now);
        assert_eq!(result, "just now");
    }
}
