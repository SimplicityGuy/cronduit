pub mod ansi;
pub mod assets;
pub mod csrf;
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

#[derive(Clone)]
pub struct AppState {
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub version: &'static str,
    pub pool: DbPool,
    pub cmd_tx: tokio::sync::mpsc::Sender<SchedulerCmd>,
    pub config_path: std::path::PathBuf,
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
            "/jobs/{job_id}/runs/{run_id}",
            get(handlers::run_detail::run_detail),
        )
        .route(
            "/partials/log-viewer/{run_id}",
            get(handlers::run_detail::log_viewer_partial),
        )
        .route("/settings", get(handlers::settings::settings))
        .route("/health", get(handlers::health::health))
        .route("/api/jobs/{id}/run", post(handlers::api::run_now))
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
