//! Prometheus /metrics endpoint handler (OPS-02, D-08).
//!
//! Returns Prometheus text format with Content-Type `text/plain; version=0.0.4`.
//! Unauthenticated by design -- consistent with v1 no-auth stance.

use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;

use crate::web::AppState;

pub async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    let body = state.metrics_handle.render();
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}
