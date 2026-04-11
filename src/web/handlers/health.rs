//! GET /health endpoint (OPS-01).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::{Value, json};

use crate::db::queries::PoolRef;
use crate::web::AppState;

pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    // Attempt a simple query to verify DB is reachable.
    let db_ok = check_db(&state).await;
    let status_code = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let body = Json(json!({
        "status": if db_ok { "ok" } else { "degraded" },
        "db": if db_ok { "ok" } else { "error" },
        "scheduler": "running"
    }));

    (status_code, body)
}

async fn check_db(state: &AppState) -> bool {
    match state.pool.reader() {
        PoolRef::Sqlite(p) => sqlx::query("SELECT 1").fetch_one(p).await.is_ok(),
        PoolRef::Postgres(p) => sqlx::query("SELECT 1").fetch_one(p).await.is_ok(),
    }
}
