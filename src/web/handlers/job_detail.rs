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
use crate::web::exit_buckets;
use crate::web::exit_buckets::ExitBucket;
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
    /// Phase 21 EXIT-01..05: pre-formatted view-model for the Exit-Code
    /// Histogram card. The askama template (plan 21-06) substitutes
    /// `{{ value }}` with zero logic — every conditional copy rendering
    /// happens in `build_exit_histogram_view` per UI-SPEC § Copywriting
    /// Contract. Soft-fail produces an empty-state ExitHistogramView with
    /// `has_min_samples=false; sample_count=0` (NOT `None`) so the template
    /// branches on `has_min_samples` instead of `{% match %}` per the
    /// logic-free contract. Tolerated as `dead_code` until plan 21-06 lands
    /// the askama template insert that consumes it.
    #[allow(dead_code)]
    exit_histogram: ExitHistogramView,
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

// ---------------------------------------------------------------------------
// Phase 21 EXIT-01..05 view-model: Exit-Code Histogram card
// ---------------------------------------------------------------------------
//
// Pre-formatted view-models consumed by the askama template (plan 21-06).
// All copy strings are server-rendered per UI-SPEC § Copywriting Contract:
// the template substitutes `{{ value }}` with zero conditional rendering.
//
// Construction lives in `build_exit_histogram_view` adjacent to the existing
// Duration card hydration block. Soft-fail in the handler produces an
// empty-state view with `has_min_samples=false; sample_count=0` (not
// `Option<ExitHistogramView>`) so the template branches on `has_min_samples`
// without a `{% match %}`.

/// Per-bar render payload — one entry per ExitBucket variant in UI-SPEC
/// display order. All eight fields are pre-formatted strings or pre-clamped
/// numerics; the template injects them verbatim.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BucketRender {
    /// Short label per UI-SPEC § Component Inventory § 10 bucket short-labels:
    /// "1", "2", "3-9", "10-126", "127", "128-143", "144-254", "255",
    /// "none", "stopped".
    pub short_label: String,
    /// CSS class per UI-SPEC § Color: "err-strong" / "err-muted" / "warn"
    /// / "stopped" / "null". Maps to a `var(--cd-status-...)` token in
    /// `app.css`.
    pub color_class: String,
    /// Tooltip dot CSS variable name (e.g., "status-error", "status-stopped")
    /// — drives the small color dot in the tooltip header.
    pub dot_token: String,
    /// Bucket count (raw `usize` — template renders verbatim).
    pub count: usize,
    /// Bar height percentage, server-clamped to 0..=100 per
    /// research § Security Domain V5 (defensive; pct is computed in Rust
    /// and never derived from operator input).
    pub height_pct: i64,
    /// Pre-formatted `aria-label` per UI-SPEC § Component Inventory aria_label
    /// table, with `{N}` substituted by `count`.
    pub aria_label: String,
    /// Pre-formatted tooltip title per UI-SPEC § Copywriting Contract:
    /// `"Exit code(s): {short_label}"`.
    pub tooltip_title: String,
    /// Pre-formatted tooltip detail per UI-SPEC § Copywriting Contract:
    /// `"{count} runs · last seen {rel}"` or `"{count} runs · last seen never"`
    /// when no top-3 entry exists. BucketStopped uses the locked override
    /// copy: `"Stopped via UI — cronduit sent SIGKILL. Distinct from
    /// \"signal-killed\" (128-143) which captures external SIGTERM /
    /// SIGSEGV / etc."`.
    pub tooltip_detail: String,
}

/// Top-N entry render payload for the EXIT-05 "Most frequent codes" sub-table.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TopCodeRender {
    /// Code label per UI-SPEC § Copywriting Contract:
    /// 127 → `"127 (command not found)"`,
    /// 137 → `"137 (SIGKILL — stopped)"`,
    /// 143 → `"143 (SIGTERM)"`,
    /// otherwise → `"{code}"` (bare integer).
    pub label: String,
    pub count: usize,
    /// Relative-time render of the most-recent occurrence: e.g.
    /// `"3 hours"` / `"1 day"` / `"never"` — uses the locally-defined
    /// `format_relative_time` helper that mirrors `run_detail.rs`.
    pub last_seen_relative: String,
}

/// Card-level view-model for the Exit-Code Histogram card on the Job Detail
/// page (Phase 21 EXIT-01..05). Eight fields, all pre-formatted server-side.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ExitHistogramView {
    /// True iff `sample_count >= 5` (D-11). Gates the
    /// `{% if has_min_samples %}` branch in the template.
    pub has_min_samples: bool,
    /// Raw row count fed to the aggregator. Used in the empty-state copy
    /// (`"have N"`) and the "Last N runs (window: 100)" caption.
    pub sample_count: usize,
    /// 10 entries — one per ExitBucket variant in UI-SPEC display order:
    /// Bucket1, Bucket2, Bucket3to9, Bucket10to126, Bucket127, Bucket128to143,
    /// Bucket144to254, Bucket255, BucketNull, BucketStopped.
    pub buckets: Vec<BucketRender>,
    /// Success-rate percent, 0..=100 (display-only; only meaningful when
    /// `has_min_samples` is true).
    pub success_rate_pct: u8,
    /// Pre-formatted success-rate display: `"{pct}%"` when
    /// `success_rate.is_some()`, `"—"` when `None` (denom == 0 per D-09).
    pub success_rate_display: String,
    /// Raw success count for the `{success_count}/{sample_count}` stat.
    pub success_count: usize,
    /// Top-3 code render payloads. Length 0..=3.
    pub top_codes: Vec<TopCodeRender>,
    /// One-sentence aria summary per UI-SPEC § Accessibility:
    /// `"Exit code distribution over last {N} runs: {top_buckets_summary}"`.
    pub chart_aria_summary: String,
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
// Phase 21 Exit-Code Histogram — bucket lookup helpers
// ---------------------------------------------------------------------------
//
// All copy-strings + CSS class names locked in UI-SPEC. The handler builder
// (`build_exit_histogram_view`) substitutes them into BucketRender; the
// askama template (plan 21-06) renders them verbatim per the
// "logic-free template" contract.

/// UI-SPEC § Component Inventory § display order — locked.
/// Single source of truth for the iteration order in
/// `build_exit_histogram_view`. Top-to-bottom matches the visual top-to-bottom
/// in the histogram card markup.
const EXIT_BUCKET_DISPLAY_ORDER: [ExitBucket; 10] = [
    ExitBucket::Bucket1,
    ExitBucket::Bucket2,
    ExitBucket::Bucket3to9,
    ExitBucket::Bucket10to126,
    ExitBucket::Bucket127,
    ExitBucket::Bucket128to143,
    ExitBucket::Bucket144to254,
    ExitBucket::Bucket255,
    ExitBucket::BucketNull,
    ExitBucket::BucketStopped,
];

/// Bucket → (color_class, dot_token) per UI-SPEC § Color (locked).
fn bucket_classes(bucket: ExitBucket) -> (&'static str, &'static str) {
    match bucket {
        ExitBucket::Bucket1 | ExitBucket::Bucket2 | ExitBucket::Bucket255 => {
            ("err-strong", "status-error")
        }
        ExitBucket::Bucket3to9 | ExitBucket::Bucket10to126 | ExitBucket::Bucket144to254 => {
            ("err-muted", "status-error-bg")
        }
        ExitBucket::Bucket127 | ExitBucket::Bucket128to143 => ("warn", "status-disabled"),
        ExitBucket::BucketStopped => ("stopped", "status-stopped"),
        ExitBucket::BucketNull => ("null", "status-cancelled"),
    }
}

/// Bucket → short label per UI-SPEC § Component Inventory (locked).
fn bucket_short_label(bucket: ExitBucket) -> &'static str {
    match bucket {
        ExitBucket::Bucket1 => "1",
        ExitBucket::Bucket2 => "2",
        ExitBucket::Bucket3to9 => "3-9",
        ExitBucket::Bucket10to126 => "10-126",
        ExitBucket::Bucket127 => "127",
        ExitBucket::Bucket128to143 => "128-143",
        ExitBucket::Bucket144to254 => "144-254",
        ExitBucket::Bucket255 => "255",
        ExitBucket::BucketNull => "none",
        ExitBucket::BucketStopped => "stopped",
    }
}

/// Bucket → aria-label template with `{N}` placeholder per UI-SPEC §
/// Component Inventory (locked). Caller substitutes `{N}` with the count.
fn bucket_aria_template(bucket: ExitBucket) -> &'static str {
    match bucket {
        ExitBucket::Bucket1 => "Exit code 1: general error — {N} runs",
        ExitBucket::Bucket2 => "Exit code 2: shell builtin misuse — {N} runs",
        ExitBucket::Bucket3to9 => "Exit codes 3 through 9: custom range — {N} runs",
        ExitBucket::Bucket10to126 => "Exit codes 10 through 126: custom range — {N} runs",
        ExitBucket::Bucket127 => "Exit code 127: command not found — {N} runs",
        ExitBucket::Bucket128to143 => {
            "Exit codes 128 through 143: signal-killed (e.g. SIGTERM, SIGSEGV) — {N} runs"
        }
        ExitBucket::Bucket144to254 => "Exit codes 144 through 254: custom range — {N} runs",
        ExitBucket::Bucket255 => "Exit code 255: out of range — {N} runs",
        ExitBucket::BucketNull => {
            "No exit code recorded — {N} runs (e.g., timeout or stopped without code captured)"
        }
        ExitBucket::BucketStopped => {
            "Stopped via UI (SIGKILL by cronduit, exit 137) — {N} runs. NOT a crash."
        }
    }
}

/// Returns the set of raw exit_codes that fall into the given bucket. Used by
/// `build_exit_histogram_view` to look up a bucket's last_seen timestamp from
/// `card.top_codes` (the per-code ledger) for the tooltip detail line. Codes
/// outside POSIX 0..=255 always route to BucketNull per the categorizer's
/// defensive fallback (D-08 documentation), so this list is operator-meaningful
/// only — full-range fallbacks are not enumerated.
fn bucket_exit_code_predicate(bucket: ExitBucket, code: i32) -> bool {
    match bucket {
        ExitBucket::Bucket1 => code == 1,
        ExitBucket::Bucket2 => code == 2,
        ExitBucket::Bucket3to9 => (3..=9).contains(&code),
        ExitBucket::Bucket10to126 => (10..=126).contains(&code),
        ExitBucket::Bucket127 => code == 127,
        ExitBucket::Bucket128to143 => (128..=143).contains(&code),
        ExitBucket::Bucket144to254 => (144..=254).contains(&code),
        ExitBucket::Bucket255 => code == 255,
        // BucketNull holds None or out-of-POSIX codes; not addressable via
        // `card.top_codes` (which only tracks Some(code) entries — the
        // aggregator skips None per exit_buckets.rs §`aggregate`).
        ExitBucket::BucketNull => false,
        // BucketStopped has its own tooltip-detail override; per-code
        // last_seen is not used.
        ExitBucket::BucketStopped => false,
    }
}

/// Format a top-3 code label per UI-SPEC § Copywriting Contract (locked).
fn format_top_code_label(code: i32) -> String {
    match code {
        127 => "127 (command not found)".to_string(),
        137 => "137 (SIGKILL — stopped)".to_string(),
        143 => "143 (SIGTERM)".to_string(),
        c => c.to_string(),
    }
}

/// Render an RFC3339 timestamp as a coarse relative-time string for the
/// "last seen" copy in BucketRender.tooltip_detail and TopCodeRender.
/// last_seen_relative. Mirrors the helper in `run_detail.rs` exactly so
/// FCTX panel + Exit Histogram card emit identical relative-time copy
/// shapes ("3 hours" / "1 day" / "just now"). Returns "never" for None
/// inputs; `unknown` for unparseable strings (defensive).
fn format_relative_time_or_never(rfc3339: Option<&str>) -> String {
    let s = match rfc3339 {
        Some(s) => s,
        None => return "never".to_string(),
    };
    let parsed = match chrono::DateTime::parse_from_rfc3339(s) {
        Ok(dt) => dt.with_timezone(&chrono::Utc),
        Err(_) => return "unknown".to_string(),
    };
    let now = chrono::Utc::now();
    let delta = now.signed_duration_since(parsed);
    let secs = delta.num_seconds();
    if secs < 60 {
        return "just now".to_string();
    }
    let mins = delta.num_minutes();
    if mins < 60 {
        return if mins == 1 {
            "1 minute".to_string()
        } else {
            format!("{mins} minutes")
        };
    }
    let hours = delta.num_hours();
    if hours < 24 {
        return if hours == 1 {
            "1 hour".to_string()
        } else {
            format!("{hours} hours")
        };
    }
    let days = delta.num_days();
    if days == 1 {
        "1 day".to_string()
    } else {
        format!("{days} days")
    }
}

/// Build the pre-formatted `ExitHistogramView` from an
/// `exit_buckets::HistogramCard`. Always produces a valid view-model — the
/// template branches on `has_min_samples` for the empty-state copy
/// (D-15/D-16) so the constructor is non-Option per the logic-free template
/// contract.
///
/// Field-by-field decisions:
/// - `has_min_samples`/`sample_count`/`success_count`: forwarded from the card.
/// - `buckets`: 10 entries in `EXIT_BUCKET_DISPLAY_ORDER`. `height_pct`
///   server-clamped to 0..=100 per research § Security Domain V5; pct is an
///   `i64` computed in Rust + `.clamp(0, 100)`. Operator input never flows
///   here.
/// - `success_rate_pct` + `success_rate_display`: D-09 — when
///   `card.success_rate.is_none()` (denom == 0, all-stopped path), display is
///   `"—"` (em dash, U+2014, matching the Duration card empty-state copy).
/// - `top_codes`: `card.top_codes` mapped 1:1 through `format_top_code_label`
///   + `format_relative_time_or_never`. Length 0..=3 per `aggregate`'s
///   `truncate(3)`.
/// - `chart_aria_summary`: composes a one-sentence summary of the top 3-4
///   non-zero buckets in count-descending order (UI-SPEC § Accessibility).
fn build_exit_histogram_view(card: &exit_buckets::HistogramCard) -> ExitHistogramView {
    // 1. Find max bucket count for the height_pct denominator. When the card
    //    is empty (sample_count == 0 / brand-new job), max_count is 0 and
    //    every bar pct collapses to 0 — which is fine because the template
    //    short-circuits on `has_min_samples=false` and renders the empty
    //    state instead of the bars.
    let max_count: usize = card.buckets.values().copied().max().unwrap_or(0);

    // 2. Build the 10-entry Vec<BucketRender> in display order.
    let buckets: Vec<BucketRender> = EXIT_BUCKET_DISPLAY_ORDER
        .iter()
        .map(|bucket| {
            let count = card.buckets.get(bucket).copied().unwrap_or(0);

            // Height percent: server-clamped to 0..=100. When max_count is 0
            // (zero non-success buckets), pct is 0 for every bar.
            let height_pct: i64 = if max_count == 0 {
                0
            } else {
                ((count as i64 * 100) / max_count as i64).clamp(0, 100)
            };

            let (color_class, dot_token) = bucket_classes(*bucket);
            let short_label = bucket_short_label(*bucket).to_string();
            let aria_label = bucket_aria_template(*bucket)
                .replace("{N}", &count.to_string());
            let tooltip_title = format!("Exit code(s): {short_label}");

            // Tooltip detail: BucketStopped uses the locked override copy;
            // every other bucket renders "{count} runs · last seen {rel}",
            // where {rel} is looked up from card.top_codes if any of the
            // bucket's codes appears there, else "never".
            let tooltip_detail = if matches!(bucket, ExitBucket::BucketStopped) {
                "Stopped via UI — cronduit sent SIGKILL. Distinct from \"signal-killed\" \
                 (128-143) which captures external SIGTERM / SIGSEGV / etc."
                    .to_string()
            } else {
                let last_seen = card
                    .top_codes
                    .iter()
                    .find(|tc| bucket_exit_code_predicate(*bucket, tc.code))
                    .and_then(|tc| tc.last_seen.as_deref());
                let rel = format_relative_time_or_never(last_seen);
                format!("{count} runs · last seen {rel}")
            };

            BucketRender {
                short_label,
                color_class: color_class.to_string(),
                dot_token: dot_token.to_string(),
                count,
                height_pct,
                aria_label,
                tooltip_title,
                tooltip_detail,
            }
        })
        .collect();

    // 3. Success-rate stat (D-09).
    let (success_rate_pct, success_rate_display) = match card.success_rate {
        Some(rate) => {
            let pct = (rate * 100.0).round().clamp(0.0, 100.0) as u8;
            (pct, format!("{pct}%"))
        }
        None => (0, "—".to_string()),
    };

    // 4. Top-3 code render entries (UI-SPEC § Copywriting Contract).
    let top_codes: Vec<TopCodeRender> = card
        .top_codes
        .iter()
        .map(|tc| TopCodeRender {
            label: format_top_code_label(tc.code),
            count: tc.count,
            last_seen_relative: format_relative_time_or_never(tc.last_seen.as_deref()),
        })
        .collect();

    // 5. chart_aria_summary — one-sentence top-buckets summary in
    //    count-descending order. Uses up to 4 non-zero buckets; ties broken by
    //    display order for determinism. Empty-state ("Exit code distribution
    //    over last 0 runs: no data") is acceptable because the template
    //    surfaces `has_min_samples=false` empty-state copy alongside.
    let mut nonzero: Vec<(ExitBucket, usize)> = EXIT_BUCKET_DISPLAY_ORDER
        .iter()
        .filter_map(|b| {
            let c = card.buckets.get(b).copied().unwrap_or(0);
            if c > 0 { Some((*b, c)) } else { None }
        })
        .collect();
    // Stable sort preserves display-order tie-break.
    nonzero.sort_by(|a, b| b.1.cmp(&a.1));
    let top_segments: Vec<String> = nonzero
        .iter()
        .take(4)
        .map(|(b, c)| format!("{} ({} runs)", bucket_short_label(*b), c))
        .collect();
    let chart_aria_summary = if top_segments.is_empty() {
        format!(
            "Exit code distribution over last {} runs: no data",
            card.sample_count
        )
    } else {
        format!(
            "Exit code distribution over last {} runs: {}",
            card.sample_count,
            top_segments.join(", ")
        )
    };

    ExitHistogramView {
        has_min_samples: card.has_min_samples,
        sample_count: card.sample_count,
        buckets,
        success_rate_pct,
        success_rate_display,
        success_count: card.success_count,
        top_codes,
        chart_aria_summary,
    }
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

        // Exit-Code Histogram card hydration (Phase 21 EXIT-01..05) -----------
        //
        // Window: last-100 ALL runs (no SQL-side bucketing per D-06; the
        // `exit_buckets::aggregate` function does the work in Rust).
        //
        // Soft-fail (D-12 + research § landmine §1): NEW logic compared to the
        // dashboard sparkline path. The dashboard pattern uses
        // `.unwrap_or_default()` ALONE — no warn. Phase 21 explicitly upgrades
        // the contract: log a degraded-card warn so an opaque DB error never
        // silently produces a "no data" empty state. Field shape mirrors
        // `src/web/handlers/api.rs:127-132` verbatim for log-aggregator parity.
        const HISTOGRAM_SAMPLE_LIMIT: i64 = 100;

        let raw_runs = queries::get_recent_runs_for_histogram(
            &state.pool,
            job_id,
            HISTOGRAM_SAMPLE_LIMIT,
        )
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(
                target: "cronduit.web",
                job_id,
                error = %e,
                "exit histogram: query failed — degraded card"
            );
            Vec::new()
        });

        let card = exit_buckets::aggregate(&raw_runs);
        let exit_histogram = build_exit_histogram_view(&card);

        JobDetailPage {
            job: job_view,
            job_id,
            runs,
            total_runs: run_result.total,
            page,
            total_pages,
            any_running,
            csrf_token,
            exit_histogram,
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
