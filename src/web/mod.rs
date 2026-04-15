pub mod ansi;
pub mod assets;
pub mod csrf;
pub mod format;
pub mod handlers;

use axum::{
    Router, middleware,
    routing::{get, post},
};
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;
use tower_http::trace::TraceLayer;

use crate::db::DbPool;
use crate::scheduler::cmd::SchedulerCmd;

/// Tracks the result of the most recent config reload for the settings page.
#[derive(Clone)]
pub struct ReloadState {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status: String,
    pub summary: String,
}

#[derive(Clone)]
pub struct AppState {
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub version: &'static str,
    pub pool: DbPool,
    pub cmd_tx: tokio::sync::mpsc::Sender<SchedulerCmd>,
    pub config_path: std::path::PathBuf,
    pub tz: chrono_tz::Tz,
    pub last_reload: std::sync::Arc<std::sync::Mutex<Option<ReloadState>>>,
    pub watch_config: bool,
    /// Prometheus metrics handle for rendering /metrics endpoint (OPS-02).
    pub metrics_handle: metrics_exporter_prometheus::PrometheusHandle,
    /// Per-run authoritative records keyed by run_id (D-01 merged map).
    /// SSE handlers subscribe to `entry.broadcast_tx` for real-time log streaming
    /// (UI-14); plan 10-07's stop_run handler will look up `entry.control` to
    /// fire `RunControl::stop(StopReason::Operator)` (SCHED-10).
    pub active_runs: std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<i64, crate::scheduler::RunEntry>>,
    >,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(handlers::dashboard::dashboard))
        .route("/partials/job-table", get(handlers::dashboard::dashboard))
        .route("/jobs/{id}", get(handlers::job_detail::job_detail))
        .route(
            "/partials/run-history/{id}",
            get(handlers::job_detail::job_detail),
        )
        .route(
            "/partials/jobs/{job_id}/runs",
            get(handlers::job_detail::job_runs_partial),
        )
        .route(
            "/jobs/{job_id}/runs/{run_id}",
            get(handlers::run_detail::run_detail),
        )
        .route(
            "/partials/log-viewer/{run_id}",
            get(handlers::run_detail::log_viewer_partial),
        )
        .route(
            "/partials/runs/{run_id}/logs",
            get(handlers::run_detail::static_log_partial),
        )
        .route("/settings", get(handlers::settings::settings))
        .route("/health", get(handlers::health::health))
        .route("/api/jobs", get(handlers::api::list_jobs))
        .route("/api/jobs/{id}/runs", get(handlers::api::list_job_runs))
        .route("/api/jobs/{id}/run", post(handlers::api::run_now))
        .route("/api/reload", post(handlers::api::reload))
        .route("/api/jobs/{id}/reroll", post(handlers::api::reroll))
        .route("/api/runs/{run_id}/stop", post(handlers::api::stop_run))
        .route("/metrics", get(handlers::metrics::metrics_handler))
        .route("/events/runs/{run_id}/logs", get(handlers::sse::sse_logs))
        .route("/static/{*path}", get(assets::static_handler))
        .route("/vendor/{*path}", get(assets::vendor_handler))
        .with_state(state)
        .layer(middleware::from_fn(csrf::ensure_csrf_cookie))
        .layer(TraceLayer::new_for_http())
}

pub async fn serve(
    bind: SocketAddr,
    state: AppState,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(%bind, "listening");

    axum::serve(listener, router(state).into_make_service())
        .with_graceful_shutdown(async move { shutdown.cancelled().await })
        .await?;

    Ok(())
}
