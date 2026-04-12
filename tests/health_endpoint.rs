//! Integration test for GET /health endpoint (OPS-01).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

/// Build a test router with a real (in-memory SQLite) DB pool.
async fn test_app() -> axum::Router {
    use cronduit::db::DbPool;
    use cronduit::web::{AppState, router};

    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    pool.migrate().await.unwrap();

    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::channel(32);

    let metrics_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .build_recorder()
        .handle();

    let state = AppState {
        started_at: chrono::Utc::now(),
        version: "test",
        pool,
        cmd_tx,
        config_path: std::path::PathBuf::from("/test/config.toml"),
        tz: chrono_tz::Tz::UTC,
        last_reload: std::sync::Arc::new(std::sync::Mutex::new(None)),
        metrics_handle,
        watch_config: false,
        active_runs: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    router(state)
}

#[tokio::test]
async fn health_returns_200_with_ok_status() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
    assert_eq!(json["db"], "ok");
    assert_eq!(json["scheduler"], "running");
}

#[tokio::test]
async fn health_returns_json_content_type() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        content_type.contains("application/json"),
        "expected JSON content type, got: {content_type}"
    );
}
