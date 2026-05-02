//! Run Detail page and log viewer partial (UI-09, D-05, D-07).

use askama::Template;
use askama_web::WebTemplateExt;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum_htmx::HxRequest;
use serde::Deserialize;

use crate::db::queries;
use crate::db::queries::FailureContext;
use crate::web::AppState;
use crate::web::ansi;
use crate::web::csrf;
use crate::web::format::{format_duration_ms, format_duration_ms_floor_seconds};
use crate::web::stats;

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
    /// Raw duration in milliseconds. Pre-formatted into `duration_display`
    /// for the metadata card; the FCTX panel DURATION row (P21 FCTX-05)
    /// needs the raw value to compute `current / p50` factor against
    /// `format_duration_ms_floor_seconds(Some(p50))` per UI-SPEC § Copywriting.
    pub duration_ms: Option<i64>,
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

/// Phase 21 FCTX-05 minimum samples threshold. UI-SPEC FCTX-05 row spec
/// fixes this at N >= 5 successful runs (NOT 20 — 20 is the v1.1 OBS-04
/// Duration card threshold, intentionally distinct).
const FCTX_MIN_DURATION_SAMPLES: usize = 5;
/// Phase 21 FCTX-05 successful-run sample window. Mirrors v1.1 OBS-04
/// `PERCENTILE_SAMPLE_LIMIT` (last-100 successful runs); the FCTX threshold
/// at 5 just gates display, not the query window.
const FCTX_DURATION_SAMPLE_LIMIT: i64 = 100;
/// Phase 21 UI-SPEC § Copywriting Contract IMAGE DIGEST row truncation.
/// Locked at 12 hex chars per the contract (`{old_12hex}… → {new_12hex}…`).
const FCTX_DIGEST_TRUNCATE_LEN: usize = 12;

/// Truncate a hex-ish string to the first `n` characters. Used for the
/// IMAGE DIGEST row per UI-SPEC § Copywriting Contract: `{old_12hex}… →
/// {new_12hex}…`. Bounded by `take(n)` so no panic on short input — a
/// 4-char digest renders as itself with the trailing "…" supplied by the
/// caller's format string.
fn truncate_hex(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Render an RFC3339 timestamp as a coarse relative-time string ("3 hours
/// ago", "5 minutes ago"). Used by the TIME DELTAS row per UI-SPEC §
/// Copywriting Contract. No external crate added — uses `chrono::Utc::now()`
/// + `chrono::DateTime::parse_from_rfc3339` which the project already
/// depends on (`Cargo.toml` line ~).
///
/// Returns "just now" for negative or sub-minute durations (defensive
/// against clock skew between web request and DB row write). Anything older
/// than 30 days renders as "{N} days ago" rather than a more granular unit.
fn format_relative_time(rfc3339: &str) -> String {
    let parsed = match chrono::DateTime::parse_from_rfc3339(rfc3339) {
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
        return if mins == 1 { "1 minute".to_string() } else { format!("{mins} minutes") };
    }
    let hours = delta.num_hours();
    if hours < 24 {
        return if hours == 1 { "1 hour".to_string() } else { format!("{hours} hours") };
    }
    let days = delta.num_days();
    if days == 1 { "1 day".to_string() } else { format!("{days} days") }
}

/// Phase 21 FCTX panel pre-formatted view-model assembly (UI-SPEC §
/// Copywriting Contract).
///
/// Decisions honored:
/// - **D-13 never-succeeded:** TIME DELTAS row substitutes "No prior
///   successful run" suffix (no link); IMAGE DIGEST + CONFIG rows hide
///   (image_digest_value/config_changed_value = None); DURATION hides
///   on N<5 successful samples (`percentile()` returns None on empty).
/// - **D-14 config-hash compare:** literal `run.config_hash !=
///   last_success.config_hash`; both are per-run snapshots.
/// - **D-04 NULL scheduled_for:** FIRE SKEW row hides
///   (fire_skew_value = None).
/// - **FCTX-03 docker-only IMAGE DIGEST:** is_docker_job=false suppresses
///   the row even when the digest values exist (job-type lookup happens
///   here Rust-side via `queries::get_job_by_id`).
/// - **FCTX-05 N>=5 threshold (NOT 20):** UI-SPEC FCTX-05 fixes the
///   threshold at 5 — distinct from the v1.1 OBS-04 N>=20 Duration card.
///
/// Soft-fails locally on `get_job_by_id` Err (treats as non-docker) and
/// on `get_recent_successful_durations` Err (treats as no samples). The
/// caller already performs the upstream soft-fail on `get_failure_context`
/// per D-12; this helper assumes a successful FailureContext fetch.
async fn build_fctx_view(
    run: &RunDetailView,
    ctx: FailureContext,
    pool: &crate::db::DbPool,
) -> FctxView {
    // 1. Streak summary copy (UI-SPEC § Copywriting Contract — collapsed summary).
    //    `summary_meta` carries the meta line shown next to "Failure context"
    //    when the panel is collapsed. >1 → "{N} consecutive failures";
    //    ==1 → "1 failure (no streak)".
    let summary_meta = if ctx.consecutive_failures > 1 {
        format!("{} consecutive failures", ctx.consecutive_failures)
    } else {
        "1 failure (no streak)".to_string()
    };

    // 2. Job-type lookup (FCTX-03 docker-only IMAGE DIGEST row gating).
    //    Local soft-fail to non-docker on lookup error — the IMAGE DIGEST
    //    row hides defensively rather than rendering with stale/missing
    //    job context. The handler-level soft-fail (D-12) already covers
    //    the upstream FailureContext fetch; this is a same-pattern guard.
    let is_docker_job = queries::get_job_by_id(pool, run.job_id)
        .await
        .ok()
        .flatten()
        .map(|j| j.job_type == "docker")
        .unwrap_or(false);

    // 3. last_success_run_url — only when a prior success exists.
    //    `[view last successful run]` is the LINK TEXT in UI-SPEC; the
    //    template wraps it in <a href="{last_success_run_url}"> so the
    //    Rust string carries the URL only. D-13 never-succeeded path
    //    leaves both the URL and the link out of the rendered copy.
    let last_success_run_url = ctx
        .last_success_run_id
        .map(|id| format!("/jobs/{}/runs/{}", run.job_id, id));

    // 4. TIME DELTAS row (UI-SPEC § Copywriting Contract).
    //    With prior success: "First failure: {ts_relative} ago • {N}
    //    consecutive failures" (template appends [view last successful run]
    //    via last_success_run_url).
    //    No prior success: "First failure: {ts_relative} ago • {N}
    //    consecutive failures • No prior successful run" (D-13).
    let ts_relative = format_relative_time(&run.start_time);
    let time_deltas_value = if ctx.last_success_run_id.is_some() {
        format!(
            "First failure: {} ago • {} consecutive failures",
            ts_relative, ctx.consecutive_failures
        )
    } else {
        format!(
            "First failure: {} ago • {} consecutive failures • No prior successful run",
            ts_relative, ctx.consecutive_failures
        )
    };

    // 5. IMAGE DIGEST row (UI-SPEC § Copywriting Contract).
    //    - Non-docker job → hide (FCTX-03).
    //    - Never-succeeded → hide (D-13).
    //    - Same digest as last success → "unchanged".
    //    - Different digest → "{old_12hex}… → {new_12hex}…" (12-char lock).
    //    - Current run has no captured digest → defensive "unchanged"
    //      (avoids implying a change against an absent value).
    let image_digest_value = if !is_docker_job {
        None
    } else {
        match ctx.last_success_image_digest.as_deref() {
            None => None, // D-13 never-succeeded → hide
            Some(last) => match run.image_digest.as_deref() {
                None => Some("unchanged".to_string()),
                Some(cur) if cur == last => Some("unchanged".to_string()),
                Some(cur) => Some(format!(
                    "{}… → {}…",
                    truncate_hex(last, FCTX_DIGEST_TRUNCATE_LEN),
                    truncate_hex(cur, FCTX_DIGEST_TRUNCATE_LEN)
                )),
            },
        }
    };

    // 6. CONFIG row (UI-SPEC § Copywriting Contract; D-14 literal compare).
    //    - Never-succeeded (last_success.config_hash IS NULL) → hide (D-13).
    //    - Both NOT NULL: literal `run.config_hash != last_success.config_hash`
    //      drives "Yes" / "No" copy.
    //    - Current run has no config_hash but last_success does → treat
    //      as changed (the snapshot couldn't be captured, which IS a
    //      meaningful operator-visible difference).
    let config_changed_value = match ctx.last_success_config_hash.as_deref() {
        None => None, // D-13 never-succeeded → hide
        Some(last) => {
            let changed = run.config_hash.as_deref() != Some(last);
            Some(if changed {
                "Config changed since last success: Yes".to_string()
            } else {
                "Config changed since last success: No".to_string()
            })
        }
    };

    // 7. DURATION row (UI-SPEC § Copywriting Contract; FCTX-05 N>=5).
    //    Reuses Phase 13 OBS-04 `get_recent_successful_durations` +
    //    `stats::percentile` exactly. Threshold at 5 (NOT 20 — distinct
    //    from v1.1 Duration card per UI-SPEC FCTX-05).
    //    Copy: "{this}; typical p50 is {p50} ({factor}× {longer|shorter}
    //    than usual)".
    let durations =
        queries::get_recent_successful_durations(pool, run.job_id, FCTX_DURATION_SAMPLE_LIMIT)
            .await
            .unwrap_or_default();
    let has_duration_samples = durations.len() >= FCTX_MIN_DURATION_SAMPLES;
    let duration_value = if has_duration_samples {
        let p50 = stats::percentile(&durations, 0.5)
            .expect("non-empty when has_duration_samples is true (FCTX_MIN_DURATION_SAMPLES >= 1)");
        let cur_ms = run.duration_ms.unwrap_or(0);
        // factor = current / p50; render as "Nx longer" when >=1, "Nx
        // shorter" otherwise. Defensive against p50==0 (would be a
        // sub-millisecond cohort, unlikely but cheap to guard).
        let (factor_display, direction) = if p50 == 0 {
            (1.0_f64, "longer")
        } else if cur_ms as u64 >= p50 {
            (cur_ms as f64 / p50 as f64, "longer")
        } else if cur_ms == 0 {
            // current 0 → effectively infinitely shorter; pick a finite
            // display so we don't emit "infx shorter".
            (1.0_f64, "shorter")
        } else {
            (p50 as f64 / cur_ms as f64, "shorter")
        };
        Some(format!(
            "{}; typical p50 is {} ({:.1}× {} than usual)",
            format_duration_ms_floor_seconds(Some(cur_ms)),
            format_duration_ms_floor_seconds(Some(p50 as i64)),
            factor_display,
            direction,
        ))
    } else {
        None
    };

    // 8. FIRE SKEW row (UI-SPEC § Copywriting Contract; D-04).
    //    - scheduled_for IS NULL → hide (legacy pre-v1.2 rows + scheduler
    //      legacy fallback paths).
    //    - Unparseable timestamps → hide defensively.
    //    - Otherwise: "Scheduled: {hh:mm:ss} • Started: {hh:mm:ss}
    //      (+{skew} ms)" — operator sees fire-decision-time vs
    //      actually-started time. Run Now writes scheduled_for==start_time
    //      so skew == 0ms by definition.
    let fire_skew_value = match run.scheduled_for.as_deref() {
        None => None,
        Some(sched) => {
            let sched_dt = chrono::DateTime::parse_from_rfc3339(sched).ok();
            let start_dt = chrono::DateTime::parse_from_rfc3339(&run.start_time).ok();
            match (sched_dt, start_dt) {
                (Some(s), Some(t)) => {
                    let skew_ms = (t - s).num_milliseconds();
                    Some(format!(
                        "Scheduled: {} • Started: {} (+{} ms)",
                        s.format("%H:%M:%S"),
                        t.format("%H:%M:%S"),
                        skew_ms
                    ))
                }
                _ => None,
            }
        }
    };

    FctxView {
        consecutive_failures: ctx.consecutive_failures,
        summary_meta,
        last_success_run_id: ctx.last_success_run_id,
        time_deltas_value,
        last_success_run_url,
        is_docker_job,
        image_digest_value,
        config_changed_value,
        has_duration_samples,
        duration_value,
        fire_skew_value,
    }
}

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
            duration_ms: run.duration_ms,
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

        // Phase 21 FCTX panel gating + fetch + soft-fail (FCTX-01 / D-12).
        //
        // Gating: panel renders ONLY for status ∈ {failed, timeout}. The
        // `error` status is intentionally excluded (research landmine §11) —
        // `error` is the executor-error orphan path; the FCTX panel is the
        // diagnostic-rich operator surface for actual job failures.
        //
        // Soft-fail: get_failure_context Err → hide panel + emit warn.
        // Field shape mirrors `src/web/handlers/api.rs:127-132` verbatim
        // (target: "cronduit.web", structured fields, error = %e Display).
        // Per research landmine §12 we do NOT short-circuit the handler:
        // log fetch + page render proceed unaffected so a transient FCTX
        // query failure still shows the operator the run + its logs.
        let (show_fctx_panel, fctx) =
            if matches!(run_view.status.as_str(), "failed" | "timeout") {
                match queries::get_failure_context(&state.pool, run_view.job_id).await {
                    Ok(ctx) => {
                        let view = build_fctx_view(&run_view, ctx, &state.pool).await;
                        (true, Some(view))
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: "cronduit.web",
                            job_id = run_view.job_id,
                            run_id = run_view.id,
                            error = %e,
                            "fctx panel: get_failure_context failed — hiding panel"
                        );
                        (false, None)
                    }
                }
            } else {
                (false, None)
            };

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
