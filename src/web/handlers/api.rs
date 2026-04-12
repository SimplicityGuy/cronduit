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
use crate::scheduler::cmd::{ReloadStatus, SchedulerCmd};
use crate::web::AppState;
use crate::web::csrf;

#[derive(Deserialize)]
pub struct CsrfForm {
    csrf_token: String,
}

// Backward-compatible alias for run_now handler.
type RunNowForm = CsrfForm;

pub async fn run_now(
    State(state): State<AppState>,
    Path(job_id): Path<i64>,
    cookies: CookieJar,
    axum::Form(form): axum::Form<RunNowForm>,
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
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    };

    // 3. Send RunNow command through scheduler channel (D-08)
    match state.cmd_tx.send(SchedulerCmd::RunNow { job_id }).await {
        Ok(()) => {
            tracing::info!(
                target: "cronduit.web",
                job_id,
                job_name = %job.name,
                "Run Now requested via API"
            );

            // 4. Return 200 with HX-Trigger for toast (D-10)
            // The showToast event carries {message, level} in the detail,
            // matching the JS listener in base.html.
            let event = HxEvent::new_with_data(
                "showToast",
                json!({"message": format!("Run queued: {}", job.name), "level": "info"}),
            )
            .expect("toast event serialization");

            (HxResponseTrigger::normal([event]), StatusCode::OK).into_response()
        }
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Scheduler is shutting down",
        )
            .into_response(),
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

                (HxResponseTrigger::normal([event]), axum::Json(json_body)).into_response()
            }
            Err(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, "Scheduler shutting down").into_response()
            }
        },
        Err(_) => {
            (StatusCode::SERVICE_UNAVAILABLE, "Scheduler shutting down").into_response()
        }
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
            Err(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, "Scheduler shutting down").into_response()
            }
        },
        Err(_) => {
            (StatusCode::SERVICE_UNAVAILABLE, "Scheduler shutting down").into_response()
        }
    }
}
