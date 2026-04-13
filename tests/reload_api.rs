//! Regression test for the HX-Refresh header on the reload API handler.
//!
//! Phase 7 D-14 / D-15: asserts that POST /api/reload with valid CSRF returns
//! an `HX-Refresh: true` response header so the settings page auto-refreshes.
//! This covers the UAT-reported "reload card doesn't refresh" issue closed in
//! PR #9 (commit 8b69cb8), specifically `src/web/handlers/api.rs:175-181`.
//!
//! Unlike tests/reload_sighup.rs and tests/reload_inflight.rs -- which call
//! the library reload entry point directly -- this test exercises the HTTP
//! handler via `tower::ServiceExt::oneshot`, because the `HX-Refresh` header
//! is only inserted by the handler, not by the underlying library function.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use tower::ServiceExt; // brings .oneshot()

use cronduit::db::DbPool;
use cronduit::scheduler::cmd::{ReloadResult, ReloadStatus, SchedulerCmd};
use cronduit::telemetry::setup_metrics;
use cronduit::web::csrf::CSRF_COOKIE_NAME;
use cronduit::web::handlers::api::reload;
use cronduit::web::{AppState, ReloadState};

/// Shared CSRF token used for both the cookie header and the form field.
/// `validate_csrf` accepts any byte-equal non-empty pair of equal length.
const TEST_CSRF: &str = "phase7-reload-api-regression-test-token";

/// Build a minimal axum Router with a stubbed AppState whose scheduler
/// background task replies Ok to any Reload command.
async fn build_test_app() -> Router {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);

    // Stub scheduler: reply Ok to any Reload command that arrives.
    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            if let SchedulerCmd::Reload { response_tx } = cmd {
                let _ = response_tx.send(ReloadResult {
                    status: ReloadStatus::Ok,
                    added: 0,
                    updated: 0,
                    disabled: 0,
                    unchanged: 3,
                    error_message: None,
                });
            }
            // RunNow and Reroll are ignored in this test.
        }
    });

    // `setup_metrics()` memoizes the installed PrometheusHandle via OnceLock,
    // so repeated calls across tests in the same process are safe and return
    // a handle actually attached to the global `metrics::` facade.
    let metrics_handle = setup_metrics();

    let state = AppState {
        started_at: chrono::Utc::now(),
        version: "test",
        pool,
        cmd_tx,
        config_path: std::path::PathBuf::from("/tmp/cronduit-test.toml"),
        tz: chrono_tz::UTC,
        last_reload: Arc::new(Mutex::new(None::<ReloadState>)),
        watch_config: false,
        metrics_handle,
        active_runs: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    Router::new()
        .route("/api/reload", post(reload))
        .with_state(state)
}

#[tokio::test]
async fn reload_response_includes_hx_refresh_header() {
    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/reload")
                .header("cookie", format!("{}={}", CSRF_COOKIE_NAME, TEST_CSRF))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("csrf_token={}", TEST_CSRF)))
                .expect("build request"),
        )
        .await
        .expect("oneshot");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "reload must succeed with valid CSRF + stub scheduler replying Ok"
    );
    let hx_refresh = response
        .headers()
        .get("HX-Refresh")
        .expect("HX-Refresh header must be present on successful reload");
    assert_eq!(
        hx_refresh, "true",
        "HX-Refresh must be the string \"true\" so HTMX triggers a full-page \
         refresh and the settings Reload Config card surfaces the new Last \
         Reload timestamp without a manual refresh"
    );
}
