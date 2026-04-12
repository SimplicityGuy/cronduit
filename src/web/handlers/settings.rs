//! Settings page handler (UI-11).

use askama::Template;
use askama_web::WebTemplateExt;
use axum::extract::State;
use axum::response::IntoResponse;
use chrono::Utc;

use crate::db::queries::PoolRef;
use crate::web::AppState;
use crate::web::csrf;

// ---------------------------------------------------------------------------
// Askama template
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "pages/settings.html")]
struct SettingsPage {
    uptime: String,
    db_status: String,
    config_path: String,
    last_reload_time: String,
    last_reload_status: String,
    last_reload_summary: String,
    watch_config: bool,
    version: String,
    csrf_token: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format uptime duration as human-readable string.
fn format_uptime(started_at: chrono::DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(started_at);
    let total_secs = diff.num_seconds().max(0);

    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m {secs}s")
    } else if hours > 0 {
        format!("{hours}h {minutes}m {secs}s")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

pub async fn settings(
    State(state): State<AppState>,
    cookies: axum_extra::extract::CookieJar,
) -> impl IntoResponse {
    let uptime = format_uptime(state.started_at);

    // Check DB connectivity with a simple SELECT 1
    let db_ok = match state.pool.reader() {
        PoolRef::Sqlite(p) => sqlx::query("SELECT 1").fetch_one(p).await.is_ok(),
        PoolRef::Postgres(p) => sqlx::query("SELECT 1").fetch_one(p).await.is_ok(),
    };
    let db_status = if db_ok {
        "ok".to_string()
    } else {
        "error".to_string()
    };

    let (last_reload_time, last_reload_status, last_reload_summary) = {
        let lr = state.last_reload.lock().unwrap();
        match lr.as_ref() {
            Some(reload) => (
                reload.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                reload.status.clone(),
                reload.summary.clone(),
            ),
            None => ("Never".to_string(), "never".to_string(), String::new()),
        }
    };

    let csrf_token = csrf::get_token_from_cookies(&cookies);

    SettingsPage {
        uptime,
        db_status,
        config_path: state.config_path.display().to_string(),
        last_reload_time,
        last_reload_status,
        last_reload_summary,
        watch_config: state.watch_config,
        version: state.version.to_string(),
        csrf_token,
    }
    .into_web_template()
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_format_uptime() {
        let now = Utc::now();

        let result = format_uptime(now - Duration::seconds(45));
        assert_eq!(result, "45s");

        let result = format_uptime(now - Duration::seconds(135));
        assert_eq!(result, "2m 15s");

        let result = format_uptime(now - Duration::seconds(8130));
        assert_eq!(result, "2h 15m 30s");

        let result = format_uptime(now - Duration::days(1) - Duration::hours(2));
        assert_eq!(result, "1d 2h 0m 0s");
    }
}
