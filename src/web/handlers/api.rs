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

/// POST /api/reload -- Trigger a config reload (D-01, RELOAD-02).
pub async fn reload(
    State(state): State<AppState>,
    cookies: CookieJar,
    axum::Form(form): axum::Form<CsrfForm>,
) -> impl IntoResponse {
    // Validate CSRF token (T-05-14)
    let cookie_token = cookies
        .get(csrf::CSRF_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .unwrap_or_default();

    if !csrf::validate_csrf(&cookie_token, &form.csrf_token) {
        return (StatusCode::FORBIDDEN, "CSRF token mismatch").into_response();
    }

    // Send Reload command through scheduler channel
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    match state
        .cmd_tx
        .send(SchedulerCmd::Reload {
            response_tx: resp_tx,
        })
        .await
    {
        Ok(()) => {
            // Wait for reload result
            match resp_rx.await {
                Ok(result) => {
                    let (msg, level, duration) = match result.status {
                        ReloadStatus::Ok => {
                            let summary = format!(
                                "{} added, {} updated, {} disabled",
                                result.added, result.updated, result.disabled
                            );
                            (
                                format!("Config reloaded: {summary}"),
                                "success",
                                5000,
                            )
                        }
                        ReloadStatus::Error => {
                            let err_msg = result
                                .error_message
                                .as_deref()
                                .unwrap_or("unknown error");
                            (format!("Reload failed: {err_msg}"), "error", 0)
                        }
                    };

                    // Update last_reload state for settings page
                    {
                        let status_str = match result.status {
                            ReloadStatus::Ok => "ok",
                            ReloadStatus::Error => "error",
                        };
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
                        level,
                        "config reload completed via API"
                    );

                    let event = HxEvent::new_with_data(
                        "showToast",
                        json!({"message": msg, "level": level, "duration": duration}),
                    )
                    .expect("toast event serialization");

                    (HxResponseTrigger::normal([event]), StatusCode::OK).into_response()
                }
                Err(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Reload response lost",
                )
                    .into_response(),
            }
        }
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Scheduler is shutting down",
        )
            .into_response(),
    }
}

/// POST /api/jobs/{id}/reroll -- Re-roll @random schedule for a job (D-06).
pub async fn reroll(
    State(state): State<AppState>,
    Path(job_id): Path<i64>,
    cookies: CookieJar,
    axum::Form(form): axum::Form<CsrfForm>,
) -> impl IntoResponse {
    // Validate CSRF token (T-05-13)
    let cookie_token = cookies
        .get(csrf::CSRF_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .unwrap_or_default();

    if !csrf::validate_csrf(&cookie_token, &form.csrf_token) {
        return (StatusCode::FORBIDDEN, "CSRF token mismatch").into_response();
    }

    // Look up job name for toast message
    let job_name = match queries::get_job_by_id(&state.pool, job_id).await {
        Ok(Some(job)) => job.name,
        Ok(None) => return (StatusCode::NOT_FOUND, "Job not found").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    };

    // Send Reroll command through scheduler channel
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
                let (msg, level, duration) = match result.status {
                    ReloadStatus::Ok => (
                        format!("Schedule re-rolled for {job_name}"),
                        "success",
                        5000,
                    ),
                    ReloadStatus::Error => {
                        let err_msg = result
                            .error_message
                            .as_deref()
                            .unwrap_or("This job has no @random schedule");
                        (err_msg.to_string(), "error", 0)
                    }
                };

                tracing::info!(
                    target: "cronduit.web",
                    job_id,
                    job_name = %job_name,
                    level,
                    "schedule reroll completed via API"
                );

                let event = HxEvent::new_with_data(
                    "showToast",
                    json!({"message": msg, "level": level, "duration": duration}),
                )
                .expect("toast event serialization");

                (HxResponseTrigger::normal([event]), StatusCode::OK).into_response()
            }
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Reroll response lost",
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
