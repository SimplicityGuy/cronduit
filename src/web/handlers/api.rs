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
use crate::scheduler::cmd::SchedulerCmd;
use crate::web::csrf;
use crate::web::AppState;

#[derive(Deserialize)]
pub struct RunNowForm {
    csrf_token: String,
}

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
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
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
        Err(_) => {
            (StatusCode::SERVICE_UNAVAILABLE, "Scheduler is shutting down").into_response()
        }
    }
}
