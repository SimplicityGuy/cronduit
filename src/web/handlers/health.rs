//! GET /health endpoint (OPS-01).

use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::db::queries::PoolRef;
use crate::web::AppState;

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    // Attempt a simple query to verify DB is reachable.
    let db_status = match check_db(&state).await {
        true => "ok",
        false => "error",
    };

    Json(json!({
        "status": "ok",
        "db": db_status,
        "scheduler": "running"
    }))
}

async fn check_db(state: &AppState) -> bool {
    match state.pool.reader() {
        PoolRef::Sqlite(p) => sqlx::query("SELECT 1").fetch_one(p).await.is_ok(),
        PoolRef::Postgres(p) => sqlx::query("SELECT 1").fetch_one(p).await.is_ok(),
    }
}
