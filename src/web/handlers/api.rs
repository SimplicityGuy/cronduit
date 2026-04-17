//! API handlers for state-changing operations.
//!
//! D-08: Run Now sends SchedulerCmd::RunNow through mpsc channel.
//! D-10: Response includes HX-Trigger header for toast notification.
//! UI-12: Manual runs recorded with trigger='manual'.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum_extra::extract::CookieJar;
use axum_htmx::HxEvent;
use axum_htmx::HxResponseTrigger;
use serde::Deserialize;
use serde_json::json;

use crate::db::queries;
use crate::scheduler::cmd::{ReloadStatus, SchedulerCmd, StopResult};
use crate::web::AppState;
use crate::web::csrf;

#[derive(Deserialize)]
pub struct CsrfForm {
    csrf_token: String,
}

pub async fn run_now(
    State(state): State<AppState>,
    Path(job_id): Path<i64>,
    cookies: CookieJar,
    axum::Form(form): axum::Form<CsrfForm>,
) -> impl IntoResponse {
    // 1. Validate CSRF token (T-03-14)
    let cookie_token = cookies
        .get(csrf::CSRF_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .unwrap_or_default();

    if !csrf::validate_csrf(&cookie_token, &form.csrf_token) {
        return (StatusCode::FORBIDDEN, "CSRF token mismatch").into_response();
    }

    // 2. Verify job exists (T-03-16)
    let job = match queries::get_job_by_id(&state.pool, job_id).await {
        Ok(Some(job)) => job,
        Ok(None) => return (StatusCode::NOT_FOUND, "Job not found").into_response(),
        Err(err) => {
            tracing::error!(target: "cronduit.web", error = %err, job_id, "run_now: job lookup failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // 3. Phase 11 UI-19 fix (Plan 11-06): insert the running `job_runs` row
    // SYNCHRONOUSLY on the handler thread BEFORE dispatching to the
    // scheduler. This eliminates the sub-second race where the browser's
    // HX-Refresh navigation to `/jobs/{job_id}/runs/{run_id}` would
    // otherwise hit a 404 (and the log-viewer would flash "Unable to
    // stream logs") because the scheduler task hadn't yet inserted the
    // row. Row is reserved here; scheduler reuses this id.
    let run_id = match queries::insert_running_run(&state.pool, job_id, "manual").await {
        Ok(id) => id,
        Err(err) => {
            tracing::error!(target: "cronduit.web", error = %err, job_id, "run_now: insert failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // 4. Dispatch the new variant carrying both ids. Scheduler's arm calls
    // `run::run_job_with_existing_run_id` which skips the INSERT step.
    match state
        .cmd_tx
        .send(SchedulerCmd::RunNowWithRunId { job_id, run_id })
        .await
    {
        Ok(()) => {
            tracing::info!(
                target: "cronduit.web",
                job_id,
                run_id,
                job_name = %job.name,
                "run_now: row inserted + scheduler notified (UI-19)"
            );

            // 5. Return 200 with HX-Trigger for toast (D-10) + HX-Refresh so the
            // Job Detail page reloads and the newly queued run appears in the
            // run list without a manual refresh. Matches the reload/reroll
            // handler pattern in this module.
            let event = HxEvent::new_with_data(
                "showToast",
                json!({"message": format!("Run queued: {}", job.name), "level": "info"}),
            )
            .expect("toast event serialization");

            let mut headers = axum::http::HeaderMap::new();
            headers.insert("HX-Refresh", "true".parse().unwrap());

            (HxResponseTrigger::normal([event]), headers, StatusCode::OK).into_response()
        }
        Err(_) => {
            // Scheduler mpsc receiver closed (shutting down). Finalize the
            // just-inserted row as error so it doesn't linger in 'running'
            // forever. T-11-06-04 mitigation.
            tracing::warn!(
                target: "cronduit.web",
                job_id,
                run_id,
                "run_now: scheduler channel closed — finalizing pre-inserted row as error"
            );
            let _ = queries::finalize_run(
                &state.pool,
                run_id,
                "error",
                None,
                tokio::time::Instant::now(),
                Some("scheduler shutting down"),
                None,
            )
            .await;
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "Scheduler is shutting down",
            )
                .into_response()
        }
    }
}

/// POST /api/reload -- trigger config reload (RELOAD-02, D-01, D-02, D-03).
///
/// Returns JSON diff summary with HTMX toast. CSRF-protected (T-05-08).
pub async fn reload(
    State(state): State<AppState>,
    cookies: CookieJar,
    axum::Form(form): axum::Form<CsrfForm>,
) -> impl IntoResponse {
    // 1. Validate CSRF (T-05-08)
    let cookie_token = cookies
        .get(csrf::CSRF_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .unwrap_or_default();

    if !csrf::validate_csrf(&cookie_token, &form.csrf_token) {
        return (StatusCode::FORBIDDEN, "CSRF token mismatch").into_response();
    }

    // 2. Send Reload command (D-09: through same SchedulerCmd channel)
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    match state
        .cmd_tx
        .send(SchedulerCmd::Reload {
            response_tx: resp_tx,
        })
        .await
    {
        Ok(()) => match resp_rx.await {
            Ok(result) => {
                // D-03: JSON diff summary
                let status_str = match result.status {
                    ReloadStatus::Ok => "ok",
                    ReloadStatus::Error => "error",
                };
                let json_body = json!({
                    "status": status_str,
                    "added": result.added,
                    "updated": result.updated,
                    "disabled": result.disabled,
                    "unchanged": result.unchanged,
                    "message": result.error_message,
                });

                // D-01/D-02: Toast with diff summary or error
                let (toast_msg, toast_level, toast_duration) = match result.status {
                    ReloadStatus::Ok => {
                        let msg = format!(
                            "Config reloaded: {} added, {} updated, {} disabled",
                            result.added, result.updated, result.disabled
                        );
                        (msg, "success", 5000u32)
                    }
                    ReloadStatus::Error => {
                        let msg = format!(
                            "Reload failed: {}",
                            result.error_message.as_deref().unwrap_or("unknown error")
                        );
                        (msg, "error", 0u32) // 0 = persist until dismissed (D-02)
                    }
                };

                // Update last_reload state for settings page
                {
                    let summary = match result.status {
                        ReloadStatus::Ok => format!(
                            "{} added, {} updated, {} disabled",
                            result.added, result.updated, result.disabled
                        ),
                        ReloadStatus::Error => result
                            .error_message
                            .as_deref()
                            .unwrap_or("unknown error")
                            .to_string(),
                    };
                    let mut lr = state.last_reload.lock().unwrap();
                    *lr = Some(crate::web::ReloadState {
                        timestamp: chrono::Utc::now(),
                        status: status_str.to_string(),
                        summary,
                    });
                }

                tracing::info!(
                    target: "cronduit.web",
                    status = status_str,
                    added = result.added,
                    updated = result.updated,
                    disabled = result.disabled,
                    "config reload requested via API"
                );

                let event = HxEvent::new_with_data(
                    "showToast",
                    json!({"message": toast_msg, "level": toast_level, "duration": toast_duration}),
                )
                .expect("toast event serialization");

                // HX-Refresh: true so settings page auto-refreshes with new reload state
                let mut headers = axum::http::HeaderMap::new();
                headers.insert("HX-Refresh", "true".parse().unwrap());

                (
                    HxResponseTrigger::normal([event]),
                    headers,
                    axum::Json(json_body),
                )
                    .into_response()
            }
            Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "Scheduler shutting down").into_response(),
        },
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "Scheduler shutting down").into_response(),
    }
}

/// POST /api/jobs/{id}/reroll -- re-roll @random schedule (D-06).
///
/// Returns HTMX toast with result. CSRF-protected (T-05-09).
pub async fn reroll(
    State(state): State<AppState>,
    Path(job_id): Path<i64>,
    cookies: CookieJar,
    axum::Form(form): axum::Form<CsrfForm>,
) -> impl IntoResponse {
    // 1. Validate CSRF (T-05-09)
    let cookie_token = cookies
        .get(csrf::CSRF_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .unwrap_or_default();

    if !csrf::validate_csrf(&cookie_token, &form.csrf_token) {
        return (StatusCode::FORBIDDEN, "CSRF token mismatch").into_response();
    }

    // 2. Verify job exists before sending command
    let job = match queries::get_job_by_id(&state.pool, job_id).await {
        Ok(Some(job)) => job,
        Ok(None) => return (StatusCode::NOT_FOUND, "Job not found").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    };

    // 3. Send Reroll command
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    match state
        .cmd_tx
        .send(SchedulerCmd::Reroll {
            job_id,
            response_tx: resp_tx,
        })
        .await
    {
        Ok(()) => match resp_rx.await {
            Ok(result) => {
                let (toast_msg, toast_level, toast_duration) = match result.status {
                    ReloadStatus::Ok => (
                        format!("Schedule re-rolled for {}", job.name),
                        "success",
                        5000u32,
                    ),
                    ReloadStatus::Error => {
                        let msg = result
                            .error_message
                            .as_deref()
                            .unwrap_or("This job has no @random schedule");
                        (msg.to_string(), "error", 0u32)
                    }
                };

                tracing::info!(
                    target: "cronduit.web",
                    job_id,
                    job_name = %job.name,
                    status = ?result.status,
                    "schedule reroll requested via API"
                );

                let event = HxEvent::new_with_data(
                    "showToast",
                    json!({"message": toast_msg, "level": toast_level, "duration": toast_duration}),
                )
                .expect("toast event serialization");

                // HX-Refresh: true to reload the job detail page with new resolved schedule
                let mut headers = axum::http::HeaderMap::new();
                if result.status == ReloadStatus::Ok {
                    headers.insert("HX-Refresh", "true".parse().unwrap());
                }

                (HxResponseTrigger::normal([event]), headers, StatusCode::OK).into_response()
            }
            Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "Scheduler shutting down").into_response(),
        },
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "Scheduler shutting down").into_response(),
    }
}

/// GET /api/jobs — list all configured jobs as JSON.
///
/// Read-only, no CSRF required. Returns the same shape the HTML dashboard
/// renders so external tooling (scripts, Prometheus sidecars, CI smoke tests)
/// can resolve job id from name without scraping HTML. Added in Phase 8 to
/// unblock the compose-smoke CI matrix (08-04) which needs deterministic
/// name → id resolution to trigger Run Now on each example job.
pub async fn list_jobs(State(state): State<AppState>) -> impl IntoResponse {
    match queries::get_dashboard_jobs(&state.pool, None, "name", "asc").await {
        Ok(jobs) => {
            let body: Vec<_> = jobs
                .iter()
                .map(|j| {
                    json!({
                        "id": j.id,
                        "name": j.name,
                        "schedule": j.schedule,
                        "resolved_schedule": j.resolved_schedule,
                        "type": j.job_type,
                        "timeout_secs": j.timeout_secs,
                        "last_status": j.last_status,
                        "last_run_time": j.last_run_time,
                        "last_trigger": j.last_trigger,
                    })
                })
                .collect();
            (StatusCode::OK, axum::Json(body)).into_response()
        }
        Err(err) => {
            tracing::error!(target: "cronduit.web", error = %err, "GET /api/jobs query failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

/// POST /api/runs/{run_id}/stop -- stop an in-flight run (SCHED-14).
///
/// CSRF-gated (T-10-07-01). Dispatches `SchedulerCmd::Stop` with a oneshot
/// reply so the scheduler loop can look up the `RunEntry` in `active_runs`,
/// fire `control.stop(StopReason::Operator)`, and report back whether the
/// run was stopped or had already finalized (race case).
///
/// Response contract (UI-SPEC §HTMX Interaction Contract):
/// - `StopResult::Stopped`       → 200 + `HX-Trigger: showToast` + `HX-Refresh: true`
/// - `StopResult::AlreadyFinalized` → 200 + `HX-Refresh: true` (D-07 silent refresh,
///   NO `HX-Trigger` so the UI stays quiet and a page reload surfaces the truth)
/// - channel send / oneshot recv err → 503 "Scheduler is shutting down"
/// - CSRF mismatch                → 403 "CSRF token mismatch"
///
/// The handler does NOT write to job_runs — all terminal-status updates flow
/// through the executor's cancel branch (PITFALLS §1.5, single-writer
/// invariant). The run lookup is read-only and only used for the toast text.
pub async fn stop_run(
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
    cookies: CookieJar,
    axum::Form(form): axum::Form<CsrfForm>,
) -> impl IntoResponse {
    // 1. Validate CSRF (T-10-07-01) — copy-verbatim from run_now.
    let cookie_token = cookies
        .get(csrf::CSRF_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .unwrap_or_default();

    if !csrf::validate_csrf(&cookie_token, &form.csrf_token) {
        return (StatusCode::FORBIDDEN, "CSRF token mismatch").into_response();
    }

    // 2. Look up the run to recover the job name for the toast. DbRunDetail
    //    already joins jobs.name so a single query suffices — no separate
    //    get_job_by_id needed. If the run doesn't exist at all, reply as if
    //    it had already finalized (D-07 collapses "unknown" and "finalized"
    //    into the same silent-refresh response because the refreshed page
    //    will show the truth either way).
    let run = match queries::get_run_by_id(&state.pool, run_id).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            let mut headers = axum::http::HeaderMap::new();
            headers.insert("HX-Refresh", "true".parse().unwrap());
            return (headers, StatusCode::OK).into_response();
        }
        Err(err) => {
            tracing::error!(target: "cronduit.web", error = %err, run_id, "stop_run: run lookup failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // 3. Dispatch Stop command with oneshot reply (analog: reroll L222-235).
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    match state
        .cmd_tx
        .send(SchedulerCmd::Stop {
            run_id,
            response_tx: resp_tx,
        })
        .await
    {
        Ok(()) => match resp_rx.await {
            Ok(StopResult::Stopped) => {
                tracing::info!(
                    target: "cronduit.web",
                    run_id,
                    job_name = %run.job_name,
                    "stop requested via API"
                );

                // Normal path — toast + HX-Refresh (verbatim pattern from run_now).
                let event = HxEvent::new_with_data(
                    "showToast",
                    json!({"message": format!("Stopped: {}", run.job_name), "level": "info"}),
                )
                .expect("toast event serialization");

                let mut headers = axum::http::HeaderMap::new();
                headers.insert("HX-Refresh", "true".parse().unwrap());

                (HxResponseTrigger::normal([event]), headers, StatusCode::OK).into_response()
            }
            Ok(StopResult::AlreadyFinalized) => {
                // D-07: silent refresh, NO HX-Trigger header so no toast fires.
                tracing::debug!(
                    target: "cronduit.web",
                    run_id,
                    "stop_run: run already finalized (race case) — replying with silent refresh"
                );
                let mut headers = axum::http::HeaderMap::new();
                headers.insert("HX-Refresh", "true".parse().unwrap());
                (headers, StatusCode::OK).into_response()
            }
            Err(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Scheduler is shutting down",
            )
                .into_response(),
        },
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Scheduler is shutting down",
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct RunsQuery {
    pub limit: Option<i64>,
}

/// GET /api/jobs/{id}/runs?limit=N — list recent runs for a job as JSON.
///
/// Read-only, no CSRF required. Default limit is 50, capped at 500. Returns
/// most recent first (same ordering as `get_run_history`). Added in Phase 8
/// so external pollers can observe run terminal status without HTML scraping.
pub async fn list_job_runs(
    State(state): State<AppState>,
    Path(job_id): Path<i64>,
    axum::extract::Query(query): axum::extract::Query<RunsQuery>,
) -> impl IntoResponse {
    // Verify job exists so a 404 distinguishes missing job from empty history.
    match queries::get_job_by_id(&state.pool, job_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return (StatusCode::NOT_FOUND, "Job not found").into_response(),
        Err(err) => {
            tracing::error!(target: "cronduit.web", error = %err, job_id, "GET /api/jobs/:id/runs job lookup failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    }

    let limit = query.limit.unwrap_or(50).clamp(1, 500);
    match queries::get_run_history(&state.pool, job_id, limit, 0).await {
        Ok(paginated) => {
            let body: Vec<_> = paginated
                .items
                .iter()
                .map(|r| {
                    json!({
                        "id": r.id,
                        "job_id": r.job_id,
                        "status": r.status,
                        "trigger": r.trigger,
                        "start_time": r.start_time,
                        "end_time": r.end_time,
                        "duration_ms": r.duration_ms,
                        "exit_code": r.exit_code,
                        "error_message": r.error_message,
                    })
                })
                .collect();
            (StatusCode::OK, axum::Json(body)).into_response()
        }
        Err(err) => {
            tracing::error!(target: "cronduit.web", error = %err, job_id, "GET /api/jobs/:id/runs history query failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}
