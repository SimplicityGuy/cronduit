//! Cross-job timeline page (Phase 13 OBS-01 / OBS-02).
//!
//! Renders a row-per-job gantt view over a 24h (default) or 7d window. Every
//! run within the window appears as a status-colored bar positioned by
//! `(start_time, end_time)`; running runs extend to server `now` and carry
//! the `cd-timeline-bar--pulsing` class.
//!
//! The handler executes a SINGLE SQL query (`queries::get_timeline_runs`) per
//! request — OBS-02 forbids the N+1 pattern. Alphabetical row order is
//! preserved by the query's `ORDER BY j.name ASC, jr.start_time ASC` plus the
//! handler's `BTreeMap` grouping.

use askama::Template;
use askama_web::WebTemplateExt;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum_htmx::HxRequest;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use chrono_tz::Tz;
use serde::Deserialize;
use std::collections::BTreeMap;

use crate::db::queries;
use crate::web::AppState;
use crate::web::format::format_duration_ms_floor_seconds;

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
pub struct TimelineParams {
    #[serde(default)]
    pub window: Option<String>,
}

// ---------------------------------------------------------------------------
// View models
// ---------------------------------------------------------------------------

/// A single run's positioned bar inside a job row.
///
/// `left_pct` and `width_pct` are precomputed as formatted `String`s (three
/// decimals) so the rendered CSS emits e.g. `left:12.500%` rather than
/// `left:12.499999999999998%`. askama interpolates strings verbatim.
pub struct TimelineBar {
    pub run_id: i64,
    pub job_id: i64,
    pub job_run_number: i64,
    /// lowercase status literal (success | failed | timeout | cancelled | stopped | running)
    pub status: String,
    /// uppercase status for tooltip/title text
    pub status_upper: String,
    /// Precomputed CSS percentage string, e.g. "12.500".
    pub left_pct: String,
    /// Precomputed CSS percentage string, e.g. "8.333".
    pub width_pct: String,
    /// Duration display via `format_duration_ms_floor_seconds`, e.g. "1m 34s".
    pub duration_display: String,
    /// `HH:MM:SS` in server tz.
    pub start_time_str: String,
    /// `HH:MM:SS` in server tz, or the literal `now` for running runs.
    pub end_time_str: String,
}

pub struct TimelineJobRow {
    pub id: i64,
    pub name: String,
    pub bars: Vec<TimelineBar>,
}

pub struct TimelineAxisTick {
    /// Precomputed CSS percentage string, e.g. "50.000".
    pub left_pct: String,
    pub label: String,
}

#[derive(Template)]
#[template(path = "pages/timeline.html")]
struct TimelinePage {
    window: String,
    jobs: Vec<TimelineJobRow>,
    axis_ticks: Vec<TimelineAxisTick>,
    truncated: bool,
}

/// Partial rendered for the 30s HTMX poll. Matches the inner contents of
/// `#timeline-body` only — no outer nav / heading / pills. Without this,
/// `hx-get="/timeline"` returned the full page and nested the whole layout
/// inside the timeline region on every poll (Phase 14 UAT rc.3 gap 2).
#[derive(Template)]
#[template(path = "partials/timeline_body.html")]
struct TimelinePartial {
    window: String,
    jobs: Vec<TimelineJobRow>,
    axis_ticks: Vec<TimelineAxisTick>,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

pub async fn timeline(
    HxRequest(is_htmx): HxRequest,
    State(state): State<AppState>,
    Query(params): Query<TimelineParams>,
) -> impl IntoResponse {
    // Strict allow-list for window param. Default and any unexpected value
    // fall back to 24h (T-13-05-06 mitigation).
    let window = match params.window.as_deref() {
        Some("7d") => "7d",
        _ => "24h",
    };

    let (window_duration, tick_step, tick_count, daily_ticks): (
        ChronoDuration,
        ChronoDuration,
        usize,
        bool,
    ) = match window {
        "7d" => (ChronoDuration::days(7), ChronoDuration::days(1), 7, true),
        _ => (
            ChronoDuration::hours(24),
            ChronoDuration::hours(2),
            12,
            false,
        ),
    };

    let now_utc: DateTime<Utc> = Utc::now();
    let window_start_utc: DateTime<Utc> = now_utc - window_duration;
    let total_seconds = window_duration.num_seconds() as f64;
    let tz: Tz = state.tz;

    // Single query per request (OBS-02 load-bearing).
    let runs = queries::get_timeline_runs(&state.pool, &window_start_utc.to_rfc3339())
        .await
        .unwrap_or_default();

    let truncated = runs.len() == 10_000;

    // Group runs by job name. Query already orders by `j.name ASC` so the
    // BTreeMap ordering matches the natural grouping pass.
    let mut by_job: BTreeMap<String, TimelineJobRow> = BTreeMap::new();
    for run in runs {
        let job_name = run.job_name.clone();
        let row = by_job
            .entry(job_name.clone())
            .or_insert_with(|| TimelineJobRow {
                id: run.job_id,
                name: job_name,
                bars: Vec::new(),
            });

        let start_utc = parse_db_timestamp(&run.start_time).unwrap_or(now_utc);
        // Guard: runs that started before window_start should have been
        // filtered by SQL, but clamp anyway in case of clock skew.
        let start_clamped = start_utc.max(window_start_utc);

        // Terminal runs use end_time; running runs extend to now_utc.
        let end_utc = match run.end_time.as_deref() {
            Some(s) => parse_db_timestamp(s).unwrap_or(now_utc),
            None => now_utc,
        };
        // Never render past the right edge of the window.
        let end_clamped = end_utc.min(now_utc);

        let left_secs = (start_clamped - window_start_utc).num_seconds() as f64;
        let width_secs = (end_clamped - start_clamped).num_seconds() as f64;

        let left_pct_f = (left_secs / total_seconds * 100.0).clamp(0.0, 100.0);
        let raw_width_pct = width_secs / total_seconds * 100.0;
        // Width clamp: never extend past right edge (left_pct + width_pct <= 100).
        let width_pct_f = raw_width_pct.clamp(0.0, 100.0 - left_pct_f);

        let status_lower = run.status.to_lowercase();
        let duration_display = format_duration_ms_floor_seconds(run.duration_ms);

        let start_local = start_utc.with_timezone(&tz);
        let end_local = end_utc.with_timezone(&tz);

        let start_time_str = start_local.format("%H:%M:%S").to_string();
        let end_time_str = if run.end_time.is_some() {
            end_local.format("%H:%M:%S").to_string()
        } else {
            "now".to_string()
        };

        row.bars.push(TimelineBar {
            run_id: run.run_id,
            job_id: run.job_id,
            job_run_number: run.job_run_number,
            status_upper: status_lower.to_uppercase(),
            status: status_lower,
            left_pct: format!("{left_pct_f:.3}"),
            width_pct: format!("{width_pct_f:.3}"),
            duration_display,
            start_time_str,
            end_time_str,
        });
    }

    let jobs: Vec<TimelineJobRow> = by_job.into_values().collect();

    // Axis ticks — DST-aware via chrono_tz. The UTC tick position is fixed;
    // the formatted label reflects whatever local time chrono_tz maps it to.
    let mut axis_ticks: Vec<TimelineAxisTick> = Vec::with_capacity(tick_count);
    for i in 0..tick_count {
        let tick_utc = window_start_utc + tick_step * (i as i32);
        let tick_local = tick_utc.with_timezone(&tz);
        let tick_secs = (tick_utc - window_start_utc).num_seconds() as f64;
        let left_pct_f = (tick_secs / total_seconds) * 100.0;
        let label = if daily_ticks {
            tick_local.format("%a").to_string()
        } else {
            tick_local.format("%H:00").to_string()
        };
        axis_ticks.push(TimelineAxisTick {
            left_pct: format!("{left_pct_f:.3}"),
            label,
        });
    }

    // HTMX polls (every 30s) must receive the partial only — returning the
    // full page here nests `<nav>` + `<h1>` + `<div id="timeline-body">`
    // inside the existing `#timeline-body` div, and each subsequent poll
    // nests again (Phase 14 UAT rc.3 gap 2). Direct browser navigation still
    // gets the full page.
    if is_htmx {
        TimelinePartial {
            window: window.to_string(),
            jobs,
            axis_ticks,
        }
        .into_web_template()
        .into_response()
    } else {
        TimelinePage {
            window: window.to_string(),
            jobs,
            axis_ticks,
            truncated,
        }
        .into_web_template()
        .into_response()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a DB timestamp stored as either RFC3339 or `%Y-%m-%d %H:%M:%S`.
///
/// Mirrors the dashboard handler's fallback idiom. The second form is what
/// SQLite's `CURRENT_TIMESTAMP` writes when a column gets a DB-side default.
fn parse_db_timestamp(s: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(ndt.and_utc());
    }
    None
}
