//! Dashboard page and HTMX job-table partial (UI-06, UI-07, UI-13).

use askama::Template;
use askama_web::WebTemplateExt;
use axum::extract::State;
use axum::response::IntoResponse;
// Phase 23 TAG-06: `axum_extra::extract::Query` (uses `serde_html_form`
// internally) is the LOAD-BEARING extractor swap — `axum::extract::Query`
// silently collapses repeated `?tag=foo&tag=bar` keys to the last value,
// which is the EXACT failure mode TAG-06 forbids (RESEARCH § Pitfall 1).
// V-05 (`active_tags_parsed_from_repeated_query`) is the regression assertion.
use axum_extra::extract::Query;
use axum_htmx::HxRequest;
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};
use std::str::FromStr;

use crate::db::queries::{self, DashboardJob, DashboardSparkRow};
use crate::web::AppState;
use crate::web::csrf;
use crate::web::format::format_duration_ms_floor_seconds;

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
    /// Phase 23 TAG-06: zero-or-more active tag filters from `?tag=foo&tag=bar`.
    /// URL key is singular (`tag`), Rust field is plural (`tags`) so the URL
    /// form reads `?tag=backup&tag=weekly` per the TAG-06 lock. Deserialized via
    /// `axum_extra::extract::Query<DashboardParams>` (uses `serde_html_form`
    /// under the hood — supports repeated keys). `axum::extract::Query` would
    /// silently collapse duplicates to one — that is the EXACT failure mode
    /// TAG-06 forbids (RESEARCH § Pitfall 1).
    ///
    /// **NEVER trust this field for SQL composition** without first
    /// intersecting with the fleet-tag fold (the handler enforces this per
    /// UI-SPEC § Stale-tag URL handling — silent server-side drop). The
    /// `dashboard()` handler is the single owner of that intersection.
    #[serde(default, rename = "tag")]
    pub tags: Vec<String>,
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
    /// Phase 23 TAG-06: distinct fleet tag set, alphabetical, used to
    /// render one chip per tag in the chip strip. Empty when no job has
    /// any tags — chip strip is hidden via HTML5 `hidden` attribute (D-02).
    /// Plan 23-05 wires the template; until that plan lands the field is
    /// unread by the template (allowed because Phase 23 view-model widening
    /// must be visible to Wave 2 plans before the templates change).
    #[allow(dead_code)]
    fleet_tags: Vec<String>,
    /// Phase 23 TAG-06: post-toggle canonicalized active tag set
    /// (sorted, deduped, fleet-intersected). Drives chip active state +
    /// hidden `<input name="tag">` siblings + sort-header href tag suffix.
    /// Plan 23-05 wires the template (see `#[allow(dead_code)]` rationale
    /// on `fleet_tags`).
    #[allow(dead_code)]
    active_tags: Vec<String>,
}

#[derive(Template)]
#[template(path = "partials/job_table.html")]
struct JobTablePartial {
    jobs: Vec<DashboardJobView>,
    csrf_token: String,
    /// Phase 23 D-11: HTMX OOB swap — the partial response renders BOTH
    /// the chip strip (OOB-swapped into `#cd-tag-chip-strip` on the live
    /// page) AND the table body (target swap). Plan 23-05 wires the
    /// template; this struct carries the data both consumers need.
    #[allow(dead_code)]
    fleet_tags: Vec<String>,
    #[allow(dead_code)]
    active_tags: Vec<String>,
}

// ---------------------------------------------------------------------------
// View model
// ---------------------------------------------------------------------------

pub struct DashboardJobView {
    pub id: i64,
    pub name: String,
    pub schedule: String,
    pub resolved_schedule: String,
    pub has_random_schedule: bool,
    /// Phase 14 — true when `enabled_override == Some(0)`. Drives the inline
    /// DISABLED badge on the name column and the em-dash in `next_fire` so
    /// operators see a coherent "this job will NOT fire" signal on the
    /// dashboard, not only on `/settings` (Phase 14 UAT rc.4 gap).
    pub is_disabled: bool,
    pub next_fire: String,
    pub last_status: String,
    pub last_status_label: String,
    pub last_run_relative: String,
    /// Exactly 20 cells, oldest-to-newest left-to-right. Fewer-than-20
    /// terminal runs pad with `kind: "empty"`. Phase 13 OBS-03.
    pub spark_cells: Vec<SparkCell>,
    /// "95%" when denominator >= `MIN_SAMPLES_FOR_RATE`, else "—".
    pub spark_badge: String,
    /// Count of non-empty sparkline cells (for aria-label).
    pub spark_total: usize,
    /// Success count (for badge tooltip).
    pub spark_numerator: usize,
    /// `terminal_count - stopped_count` (for badge tooltip + threshold gate).
    pub spark_denominator: usize,
}

/// A single cell in the dashboard sparkline (Phase 13 OBS-03).
///
/// `kind` is one of: `success`, `failed`, `timeout`, `cancelled`, `stopped`,
/// `empty`. `title` carries the per-cell tooltip (`#42 SUCCESS 1m 34s 2h ago`)
/// for filled cells, or is empty for `empty` padding cells.
pub struct SparkCell {
    /// One of: `success` | `failed` | `timeout` | `cancelled` | `stopped` | `empty`
    pub kind: String,
    /// Per-cell tooltip; empty string when `kind == "empty"`.
    pub title: String,
}

/// Minimum non-stopped terminal runs required before the success-rate badge
/// renders as an integer percent. Below this threshold, the badge renders as
/// `—` (U+2014 em dash). Phase 13 D-03.
const MIN_SAMPLES_FOR_RATE: usize = 5;

/// Exact number of sparkline cells rendered per job row. Shorter histories pad
/// with `empty` kind cells on the left so the newest run is always rightmost.
const SPARKLINE_SIZE: usize = 20;

fn to_view(job: DashboardJob, tz: Tz) -> DashboardJobView {
    let now = Utc::now();
    let now_tz = now.with_timezone(&tz);

    // Phase 14: a Some(0) override forces the job disabled regardless of the
    // config-level `enabled` flag. The scheduler already honors this by not
    // firing; the dashboard must honor it visually — otherwise operators see
    // a `Next Fire: in 41s` on a job that will never actually fire.
    let is_disabled = job.enabled_override == Some(0);

    // Compute next fire time. Skip cron evaluation entirely for
    // override-disabled jobs and render an em-dash: the reality is "never",
    // and the Settings "Currently Overridden" audit carries the authoritative
    // DISABLED state per UI-SPEC.
    let next_fire = if is_disabled {
        "—".to_string()
    } else {
        match croner::Cron::from_str(&job.resolved_schedule) {
            Ok(cron) => match cron.find_next_occurrence(&now_tz, false) {
                Ok(next) => format_relative_future(next.with_timezone(&Utc), now),
                Err(_) => "unknown".to_string(),
            },
            Err(_) => "invalid".to_string(),
        }
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

    let has_random_schedule = job.schedule.split_whitespace().any(|f| f == "@random");

    DashboardJobView {
        id: job.id,
        name: job.name,
        schedule: job.schedule,
        resolved_schedule: job.resolved_schedule,
        has_random_schedule,
        is_disabled,
        next_fire,
        last_status,
        last_status_label,
        last_run_relative,
        // Filled by the sparkline hydration loop in `dashboard()`.
        spark_cells: Vec::new(),
        spark_badge: "—".to_string(),
        spark_total: 0,
        spark_numerator: 0,
        spark_denominator: 0,
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
    // Phase 23 D-08 / RESEARCH § Pattern 3: TWO-fetch sequence so the fleet-tag
    // fold reflects the UNFILTERED fleet (chips render every tag in the fleet,
    // not only tags surviving the active AND-filter). The first fetch is
    // unfiltered (active_tags=&[]); we fold its `Vec<DashboardJob>` into
    // `fleet_tags`. The second fetch applies the active-tag intersection for
    // the table body. Both reads are cheap at homelab scale (sub-millisecond
    // over a few hundred jobs); RESEARCH Open Question 1 + PATTERNS.md L296-318
    // document the deliberate trade-off vs. the single-fetch alternative
    // (which would hide chips for tags whose only job is filtered out by
    // another active chip).
    let unfiltered_jobs = queries::get_dashboard_jobs(
        &state.pool,
        None, // no name-filter for fleet-tag fold
        &params.sort,
        &params.order,
        &[], // no tag-filter — fold over the WHOLE fleet
    )
    .await
    .unwrap_or_default();

    // Phase 23 D-08 / D-07: fleet-tag fold. `BTreeSet<String>` -> `Vec<String>`
    // preserves alphabetical sort and dedupes. Disabled jobs are excluded by
    // `WHERE j.enabled = 1` upstream — `fleet_tags` is "tags from the rendered
    // row set" (CONTEXT § Claude's Discretion default).
    let fleet_tags: Vec<String> = unfiltered_jobs
        .iter()
        .flat_map(|j| j.tags.iter().cloned())
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect();

    // Phase 23 / UI-SPEC § URL canonicalization + § Stale-tag URL handling:
    // dedup + canonicalize alphabetical (so `/?tag=zebra&tag=alpha` and
    // `/?tag=alpha&tag=zebra` produce the same shareable URL) + intersect
    // with fleet so stale URL tags are silently dropped (RESEARCH § Pitfall 4
    // / threat T-23-03-01). The retain step is the security boundary: any
    // operator-supplied tag NOT in the fleet is dropped BEFORE reaching SQL.
    let mut active_tags: Vec<String> = params.tags.clone();
    active_tags.sort();
    active_tags.dedup();
    active_tags.retain(|t| fleet_tags.contains(t));

    // Now apply the active-tag filter to the actual rendered fleet. The
    // `unfiltered_jobs` binding is consumed by the fold and goes out of scope
    // here; the new `jobs` shadows it and keeps the existing role downstream
    // (sparkline hydration + view-model construction).
    let jobs = queries::get_dashboard_jobs(
        &state.pool,
        filter,
        &params.sort,
        &params.order,
        &active_tags,
    )
    .await
    .unwrap_or_default();

    let tz: Tz = state.tz;
    let mut job_views: Vec<DashboardJobView> = jobs.into_iter().map(|j| to_view(j, tz)).collect();

    // Phase 13 OBS-03: hydrate 20-cell sparkline + success-rate badge. Single
    // query covers every job; bucket rows by job_id, reverse per-job (rn=1 is
    // newest → we need oldest-first for display), pad with empty cells on the
    // left so the newest run is always rightmost, and compute the denominator
    // EXCLUDING stopped runs (D-05).
    let spark_rows = queries::get_dashboard_job_sparks(&state.pool)
        .await
        .unwrap_or_default();

    let mut spark_by_job: HashMap<i64, Vec<DashboardSparkRow>> = HashMap::new();
    for row in spark_rows {
        spark_by_job.entry(row.job_id).or_default().push(row);
    }

    let now = Utc::now();
    for job_view in &mut job_views {
        // Query returns `rn ASC` where rn=1 is the newest run. Reverse so the
        // oldest run is at index 0 and the newest is last.
        let mut rows = spark_by_job.remove(&job_view.id).unwrap_or_default();
        rows.reverse();

        let filled = rows.len();
        let mut success_count: usize = 0;
        let mut stopped_count: usize = 0;
        let mut cells: Vec<SparkCell> = Vec::with_capacity(SPARKLINE_SIZE);

        // Leading empty cells so the newest filled cell is always rightmost.
        for _ in 0..SPARKLINE_SIZE.saturating_sub(filled) {
            cells.push(SparkCell {
                kind: "empty".to_string(),
                title: String::new(),
            });
        }

        for r in &rows {
            let status_lower = r.status.to_lowercase();
            if status_lower == "success" {
                success_count += 1;
            }
            if status_lower == "stopped" {
                stopped_count += 1;
            }

            let duration_display = format_duration_ms_floor_seconds(r.duration_ms);
            let relative = match DateTime::parse_from_rfc3339(&r.start_time) {
                Ok(dt) => format_relative_past(dt.with_timezone(&Utc), now),
                Err(_) => {
                    match chrono::NaiveDateTime::parse_from_str(&r.start_time, "%Y-%m-%d %H:%M:%S")
                    {
                        Ok(ndt) => format_relative_past(ndt.and_utc(), now),
                        Err(_) => r.start_time.clone(),
                    }
                }
            };

            cells.push(SparkCell {
                kind: status_lower.clone(),
                title: format!(
                    "#{} {} {} {}",
                    r.job_run_number,
                    status_lower.to_uppercase(),
                    duration_display,
                    relative,
                ),
            });
        }

        let denominator = filled.saturating_sub(stopped_count);
        let badge = if denominator < MIN_SAMPLES_FOR_RATE {
            "—".to_string()
        } else {
            let pct = ((success_count as f64 / denominator as f64) * 100.0).round() as i64;
            format!("{pct}%")
        };

        job_view.spark_cells = cells;
        job_view.spark_badge = badge;
        job_view.spark_total = filled;
        job_view.spark_numerator = success_count;
        job_view.spark_denominator = denominator;
    }

    let csrf_token = csrf::get_token_from_cookies(&cookies);

    if is_htmx {
        JobTablePartial {
            jobs: job_views,
            csrf_token,
            fleet_tags,
            active_tags,
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
            fleet_tags,
            active_tags,
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

    // V-05 (unit / handler): `?tag=backup&tag=weekly` deserializes to Vec<String> length 2
    // via axum_extra::Query. axum::Query would silently collapse duplicates to one — this
    // is the EXACT failure mode TAG-06 forbids (RESEARCH § Pitfall 1).
    //
    // We use `Query::try_from_uri` (axum-extra 0.12 public API at
    // `axum_extra::extract::query::Query::try_from_uri`) which calls
    // `serde_html_form::from_str` on the URI's query string — the same path
    // `from_request_parts` takes when the extractor runs in a real request.
    // Testing this directly avoids the axum `FromRequestParts<S>` state-type
    // dance and exercises the load-bearing property: a `Vec<String>` field
    // receives every occurrence of a repeated `?tag=` key.
    #[tokio::test]
    async fn active_tags_parsed_from_repeated_query() {
        use axum::http::Uri;
        use axum_extra::extract::Query as AxumExtraQuery;

        let uri: Uri = "/?tag=backup&tag=weekly"
            .parse()
            .expect("parse test URI with repeated tag keys");

        let AxumExtraQuery(params): AxumExtraQuery<DashboardParams> =
            AxumExtraQuery::try_from_uri(&uri)
                .expect("axum_extra::Query::try_from_uri deserializes repeated keys");

        assert_eq!(
            params.tags,
            vec!["backup".to_string(), "weekly".to_string()],
            "DashboardParams MUST deserialize repeated `tag=` keys into a 2-element \
             Vec<String>. axum::Query would collapse to vec![\"weekly\"] — that is the \
             regression this test prevents (RESEARCH § Pitfall 1; TAG-06 lock)."
        );
    }

    // V-07 (unit / handler): distinct-tag fold from Vec<DashboardJob> produces
    // sorted alphabetical Vec<String> for chip strip (CONTEXT D-08, RESEARCH § Pattern 3).
    //
    // The fold lives inline in the `dashboard()` handler (BTreeSet<String> ->
    // Vec<String> via flat_map + collect chain). This unit test exercises the
    // exact same pattern against a hand-built `Vec<DashboardJob>` so we can
    // assert the load-bearing property (alphabetical-distinct) without
    // standing up a database or routing layer.
    #[tokio::test]
    async fn distinct_tag_fold_alphabetical() {
        use crate::db::queries::DashboardJob;
        use std::collections::BTreeSet;

        fn mk_job(name: &str, tags: Vec<&str>) -> DashboardJob {
            DashboardJob {
                id: 0,
                name: name.to_string(),
                schedule: "*/5 * * * *".to_string(),
                resolved_schedule: "*/5 * * * *".to_string(),
                job_type: "command".to_string(),
                timeout_secs: 300,
                last_status: None,
                last_run_time: None,
                last_trigger: None,
                enabled_override: None,
                tags: tags.into_iter().map(String::from).collect(),
            }
        }

        let jobs = [
            mk_job("a", vec!["weekly", "backup"]),
            mk_job("b", vec!["backup", "prod"]),
            mk_job("c", vec![]),
        ];

        let fleet_tags: Vec<String> = jobs
            .iter()
            .flat_map(|j| j.tags.iter().cloned())
            .collect::<BTreeSet<String>>()
            .into_iter()
            .collect();

        assert_eq!(
            fleet_tags,
            vec![
                "backup".to_string(),
                "prod".to_string(),
                "weekly".to_string()
            ],
            "BTreeSet -> Vec MUST yield distinct alphabetical fleet tags. Duplicates \
             across jobs (`backup` in jobs 'a' and 'b') MUST collapse to one chip; \
             empty-tag jobs MUST contribute nothing; output MUST be alphabetical."
        );
    }
}
