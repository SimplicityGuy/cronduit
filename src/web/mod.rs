pub mod assets;

use axum::{Router, http::StatusCode, routing::get};
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub version: &'static str,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/static/{*path}", get(assets::static_handler))
        .route("/vendor/{*path}", get(assets::vendor_handler))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

async fn index() -> (StatusCode, &'static str) {
    (
        StatusCode::OK,
        "cronduit is running — no scheduler yet (Phase 1 placeholder)\n",
    )
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
