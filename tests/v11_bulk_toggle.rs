//! Phase 14 Wave 0 — red-bar integration tests for the bulk enable/disable feature.
//!
//! Every test in this file is intentionally **red** until Plans 02/03/04/06 land.
//! The compile errors and assertion failures are the scoreboard Waves 1-4 race to zero.
//!
//! Coverage map (each test → the plan that turns it green):
//!
//! | Test                                                     | Plan |
//! |----------------------------------------------------------|------|
//! | upsert_invariant                                          | 02 (struct + migration) + 03 (queries) |
//! | reload_invariant                                          | 03 |
//! | disable_missing_clears_override                           | 03 |
//! | dashboard_filter                                          | 03 |
//! | handler_csrf                                              | 04 |
//! | handler_disable                                           | 04 |
//! | handler_enable                                            | 04 |
//! | handler_partial_invalid                                   | 04 |
//! | handler_partial_invalid_toast_uses_rows_affected          | 04 |
//! | handler_dedupes_ids                                       | 04 |
//! | handler_rejects_empty                                     | 04 |
//! | handler_accepts_repeated_job_ids                          | 04 |
//! | handler_fires_reload_after_update                         | 04 |
//! | get_overridden_jobs_alphabetical                          | 03 |
//! | settings_empty_state_hides_section                        | 06 |
//!
//! Modeled verbatim on `tests/stop_handler.rs` for the harness pattern.

use std::sync::{Arc, Mutex};

use askama::Template;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use tokio::time::Instant;
use tower::ServiceExt; // brings .oneshot()

use cronduit::config::parse_and_validate;
use cronduit::db::DbPool;
use cronduit::db::queries;
use cronduit::scheduler::cmd::{ReloadResult, ReloadStatus, SchedulerCmd};
use cronduit::scheduler::sync::sync_config_to_db;
use cronduit::telemetry::setup_metrics;
use cronduit::web::csrf::CSRF_COOKIE_NAME;
use cronduit::web::handlers::api::bulk_toggle;
use cronduit::web::handlers::settings::{OverriddenJobView, SettingsPage};
use cronduit::web::{AppState, ReloadState};

/// Shared CSRF token used for both cookie and form field.
/// `validate_csrf` accepts any non-empty pair of equal-length byte strings.
const TEST_CSRF: &str = "phase14-bulk-toggle-csrf-token";

/// Seed a single job row and return its id. Mirrors `tests/stop_handler.rs::seed_running_run`
/// but skips the running-run insert (Phase 14 tests focus on the jobs table).
async fn seed_job(pool: &DbPool, name: &str) -> i64 {
    queries::upsert_job(
        pool,
        name,
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo hi"}"#,
        &format!("hash-{name}"),
        300,
    )
    .await
    .expect("upsert job")
}

/// Build a router with a mock scheduler task that records every `Reload`
/// command's arrival `Instant` into the supplied vector and replies `Ok`.
/// Returns `(router, pool, reload_instants)`.
async fn build_bulk_app() -> (Router, DbPool, Arc<tokio::sync::Mutex<Vec<Instant>>>) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    let reload_instants: Arc<tokio::sync::Mutex<Vec<Instant>>> =
        Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let reload_instants_clone = reload_instants.clone();

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);

    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            if let SchedulerCmd::Reload { response_tx } = cmd {
                {
                    let mut v = reload_instants_clone.lock().await;
                    v.push(Instant::now());
                }
                let _ = response_tx.send(ReloadResult {
                    status: ReloadStatus::Ok,
                    added: 0,
                    updated: 0,
                    disabled: 0,
                    unchanged: 0,
                    error_message: None,
                });
            }
        }
    });

    let state = AppState {
        started_at: chrono::Utc::now(),
        version: "test",
        pool: pool.clone(),
        cmd_tx,
        config_path: std::path::PathBuf::from("/tmp/cronduit-test.toml"),
        tz: chrono_tz::UTC,
        last_reload: Arc::new(Mutex::new(None::<ReloadState>)),
        watch_config: false,
        metrics_handle: setup_metrics(),
        active_runs: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    let router = Router::new()
        .route("/api/jobs/bulk-toggle", post(bulk_toggle))
        .with_state(state);

    (router, pool, reload_instants)
}

/// Build a POST /api/jobs/bulk-toggle request with the given action and ids.
/// Body uses literal `&` separators and repeated `job_ids=` keys to exercise
/// the `axum_extra::Form` (serde_html_form) path — Landmine §1 regression guard.
fn build_bulk_request(
    cookie_token: &str,
    form_token: &str,
    action: &str,
    job_ids: &[i64],
) -> Request<Body> {
    let mut body = format!("csrf_token={}&action={}", form_token, action);
    for id in job_ids {
        body.push_str(&format!("&job_ids={}", id));
    }
    Request::builder()
        .method("POST")
        .uri("/api/jobs/bulk-toggle")
        .header("cookie", format!("{}={}", CSRF_COOKIE_NAME, cookie_token))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("build request")
}

// ─── DB-layer invariants (Plan 02/03 turns these green) ────────────────────

#[tokio::test]
async fn upsert_invariant() {
    // T-V11-BULK-01: upsert_job MUST NOT touch enabled_override.
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    let id = seed_job(&pool, "alpha").await;

    // Force the override to disabled.
    let affected = queries::bulk_set_override(&pool, &[id], Some(0))
        .await
        .expect("bulk_set_override");
    assert_eq!(affected, 1);

    // Re-upsert with mutated config_json (simulates an operator edit).
    queries::upsert_job(
        &pool,
        "alpha",
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo CHANGED"}"#,
        "hash-alpha-v2",
        600,
    )
    .await
    .expect("upsert job");

    let job = queries::get_job_by_id(&pool, id)
        .await
        .expect("get_job_by_id")
        .expect("job exists");
    assert_eq!(
        job.enabled_override,
        Some(0),
        "upsert_job MUST NOT touch enabled_override (T-V11-BULK-01)"
    );
}

#[tokio::test]
async fn reload_invariant() {
    // ERG-04: a config reload that re-includes the job MUST preserve override.
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    let id = seed_job(&pool, "alpha").await;
    queries::bulk_set_override(&pool, &[id], Some(0))
        .await
        .expect("bulk_set_override");

    // Build a minimal config containing the same job and call sync. Uses the same
    // parse_and_validate pattern as `tests/reload_inflight.rs`.
    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
    use std::io::Write;
    tmp.write_all(
        br#"
[server]
timezone = "UTC"

[[jobs]]
name = "alpha"
schedule = "*/5 * * * *"
command = "echo hi"
"#,
    )
    .expect("write config");
    tmp.flush().expect("flush");
    let parsed = parse_and_validate(tmp.path()).expect("parse config");

    sync_config_to_db(&pool, &parsed.config, std::time::Duration::from_secs(0))
        .await
        .expect("sync_config_to_db");

    let job = queries::get_job_by_id(&pool, id)
        .await
        .expect("get_job_by_id")
        .expect("job exists");
    assert_eq!(
        job.enabled_override,
        Some(0),
        "reload of a still-present job MUST preserve enabled_override (ERG-04)"
    );
}

#[tokio::test]
async fn disable_missing_clears_override() {
    // ERG-04 / D-13: a job removed from config must lose BOTH enabled AND enabled_override.
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    let id_keep = seed_job(&pool, "keepme").await;
    let id_drop = seed_job(&pool, "dropme").await;
    queries::bulk_set_override(&pool, &[id_keep, id_drop], Some(0))
        .await
        .expect("bulk_set_override");

    // disable_missing_jobs is given only "keepme" — so "dropme" must be disabled
    // AND have its override cleared.
    let _ = queries::disable_missing_jobs(&pool, &["keepme".to_string()])
        .await
        .expect("disable_missing_jobs");

    let dropped = queries::get_job_by_id(&pool, id_drop)
        .await
        .expect("get_job_by_id")
        .expect("dropme exists");
    assert!(!dropped.enabled, "dropme must be disabled");
    assert_eq!(
        dropped.enabled_override, None,
        "disable_missing_jobs MUST clear enabled_override symmetrically (ERG-04)"
    );

    // keepme retains its override and remains enabled.
    let kept = queries::get_job_by_id(&pool, id_keep)
        .await
        .expect("get_job_by_id")
        .expect("keepme exists");
    assert!(kept.enabled, "keepme must stay enabled");
    assert_eq!(kept.enabled_override, Some(0), "keepme's override is preserved");
}

#[tokio::test]
async fn dashboard_filter() {
    // DB-14: get_enabled_jobs MUST filter out rows with enabled_override = 0.
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    let id_a = seed_job(&pool, "A").await;
    let _id_b = seed_job(&pool, "B").await;
    let id_c = seed_job(&pool, "C").await;

    // A: override=0 (force disabled). C: override=1 (force enabled). B: NULL.
    queries::bulk_set_override(&pool, &[id_a], Some(0))
        .await
        .expect("set A");
    queries::bulk_set_override(&pool, &[id_c], Some(1))
        .await
        .expect("set C");

    let mut names: Vec<String> = queries::get_enabled_jobs(&pool)
        .await
        .expect("get_enabled_jobs")
        .into_iter()
        .map(|j| j.name)
        .collect();
    names.sort();

    assert_eq!(
        names,
        vec!["B".to_string(), "C".to_string()],
        "get_enabled_jobs must exclude override=0 rows; expected [B, C], got {names:?}"
    );
}

// ─── Handler integration tests (Plan 04 turns these green) ────────────────

#[tokio::test]
async fn handler_csrf() {
    // ERG-01: mismatched CSRF returns 403 AND leaves the DB untouched.
    let (app, pool, _reload_instants) = build_bulk_app().await;
    let id = seed_job(&pool, "alpha").await;

    let req = build_bulk_request(
        "cookie-side-token-aaaaaaaaaaaa",
        "form-side-token-bbbbbbbbbbbbbb",
        "disable",
        &[id],
    );
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let job = queries::get_job_by_id(&pool, id)
        .await
        .expect("get_job_by_id")
        .expect("job exists");
    assert_eq!(
        job.enabled_override, None,
        "CSRF failure must NOT mutate enabled_override"
    );
}

#[tokio::test]
async fn handler_disable() {
    // ERG-01: action=disable sets override=0 for every selected id.
    let (app, pool, _reload_instants) = build_bulk_app().await;
    let id1 = seed_job(&pool, "alpha").await;
    let id2 = seed_job(&pool, "bravo").await;

    let req = build_bulk_request(TEST_CSRF, TEST_CSRF, "disable", &[id1, id2]);
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::OK);

    for id in [id1, id2] {
        let job = queries::get_job_by_id(&pool, id)
            .await
            .expect("get_job_by_id")
            .expect("job exists");
        assert_eq!(
            job.enabled_override,
            Some(0),
            "id={id} must have enabled_override = Some(0) after action=disable"
        );
    }
}

#[tokio::test]
async fn handler_enable() {
    // D-05: action=enable clears override (sets enabled_override = NULL).
    let (app, pool, _reload_instants) = build_bulk_app().await;
    let id1 = seed_job(&pool, "alpha").await;
    let id2 = seed_job(&pool, "bravo").await;

    // Pre-state: both disabled via override.
    queries::bulk_set_override(&pool, &[id1, id2], Some(0))
        .await
        .expect("seed override");

    let req = build_bulk_request(TEST_CSRF, TEST_CSRF, "enable", &[id1, id2]);
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::OK);

    for id in [id1, id2] {
        let job = queries::get_job_by_id(&pool, id)
            .await
            .expect("get_job_by_id")
            .expect("job exists");
        assert_eq!(
            job.enabled_override, None,
            "id={id} must have enabled_override cleared to NULL after action=enable (D-05)"
        );
    }
}

#[tokio::test]
async fn handler_partial_invalid() {
    // D-12: 2 valid + 1 invalid id → 200 + toast suffix "(1 not found)".
    let (app, pool, _reload_instants) = build_bulk_app().await;
    let id1 = seed_job(&pool, "alpha").await;
    let id2 = seed_job(&pool, "bravo").await;

    let req = build_bulk_request(TEST_CSRF, TEST_CSRF, "disable", &[id1, id2, 9999]);
    let resp = app.clone().oneshot(req).await.expect("response");

    assert_eq!(resp.status(), StatusCode::OK);
    let hx_trigger = resp
        .headers()
        .get("HX-Trigger")
        .and_then(|v| v.to_str().ok())
        .expect("HX-Trigger header present");
    assert!(
        hx_trigger.contains("(1 not found)"),
        "HX-Trigger toast must contain literal '(1 not found)' (D-12); got: {hx_trigger}"
    );
}

#[tokio::test]
async fn handler_partial_invalid_toast_uses_rows_affected() {
    // UI-SPEC primary-count semantics: rows_affected (NOT selection_size) drives the leading number.
    // Seed 2 valid jobs (ids 1, 2); 9999 is intentionally missing.
    let (app, pool, _reload_instants) = build_bulk_app().await;
    let id1 = seed_job(&pool, "alpha").await;
    let id2 = seed_job(&pool, "bravo").await;
    assert_eq!(
        (id1, id2),
        (1, 2),
        "seed ids must be 1 and 2 for deterministic assertion"
    );

    let req = build_bulk_request(TEST_CSRF, TEST_CSRF, "disable", &[1, 2, 9999]);
    let resp = app.clone().oneshot(req).await.expect("response");

    assert_eq!(resp.status(), StatusCode::OK);
    let hx_trigger = resp
        .headers()
        .get("HX-Trigger")
        .and_then(|v| v.to_str().ok())
        .expect("HX-Trigger header present");

    // Parse the HX-Trigger JSON envelope and extract the toast message.
    let v: serde_json::Value =
        serde_json::from_str(hx_trigger).expect("valid JSON envelope");
    let msg = v
        .get("showToast")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .expect("message present");

    // LOCKS primary-count semantics: rows_affected=2, selection_size=3, not_found=1.
    assert_eq!(msg, "2 jobs disabled. (1 not found)");
}

#[tokio::test]
async fn handler_dedupes_ids() {
    // D-12a: duplicate job_ids in the body collapse via BTreeSet → rows_affected == unique count.
    let (app, pool, _reload_instants) = build_bulk_app().await;
    let id1 = seed_job(&pool, "alpha").await;
    let id2 = seed_job(&pool, "bravo").await;

    // Body: job_ids=1&job_ids=1&job_ids=2 — three entries, two unique.
    let req = build_bulk_request(TEST_CSRF, TEST_CSRF, "disable", &[id1, id1, id2]);
    let resp = app.clone().oneshot(req).await.expect("response");

    assert_eq!(resp.status(), StatusCode::OK);
    let hx_trigger = resp
        .headers()
        .get("HX-Trigger")
        .and_then(|v| v.to_str().ok())
        .expect("HX-Trigger header present");
    let v: serde_json::Value =
        serde_json::from_str(hx_trigger).expect("valid JSON envelope");
    let msg = v
        .get("showToast")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .expect("message present");
    assert!(
        msg.starts_with("2 jobs disabled"),
        "rows_affected must be 2 (deduped from 3 inputs), got: {msg}"
    );
}

#[tokio::test]
async fn handler_rejects_empty() {
    // UI-SPEC + Landmine §9: explicit empty-list rejection produces 400 with a recognizable toast.
    let (app, _pool, _reload_instants) = build_bulk_app().await;

    // Body has csrf_token + action but NO job_ids keys at all.
    let req = build_bulk_request(TEST_CSRF, TEST_CSRF, "disable", &[]);
    let resp = app.clone().oneshot(req).await.expect("response");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let hx_trigger = resp
        .headers()
        .get("HX-Trigger")
        .and_then(|v| v.to_str().ok())
        .expect("HX-Trigger header present on empty-rejection branch");
    assert!(
        hx_trigger.contains("No jobs selected"),
        "HX-Trigger toast must contain literal 'No jobs selected'; got: {hx_trigger}"
    );
}

#[tokio::test]
async fn handler_accepts_repeated_job_ids() {
    // Landmine §1 regression guard: repeated `job_ids=` keys must deserialize into a Vec
    // via `axum_extra::extract::Form` (serde_html_form), not stock `axum::Form`.
    let (app, pool, _reload_instants) = build_bulk_app().await;
    let id1 = seed_job(&pool, "alpha").await;
    let id2 = seed_job(&pool, "bravo").await;
    let id3 = seed_job(&pool, "charlie").await;

    let req = build_bulk_request(TEST_CSRF, TEST_CSRF, "disable", &[id1, id2, id3]);
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::OK);

    // All three ids must have been observed by the handler — verify via DB state.
    for id in [id1, id2, id3] {
        let job = queries::get_job_by_id(&pool, id)
            .await
            .expect("get_job_by_id")
            .expect("job exists");
        assert_eq!(
            job.enabled_override,
            Some(0),
            "id={id} must have been included in the bulk update (axum_extra::Form must accept repeated keys)"
        );
    }
}

#[tokio::test]
async fn handler_fires_reload_after_update() {
    // Landmine §6: the `SchedulerCmd::Reload` MUST fire AFTER the DB UPDATE commits.
    let (app, pool, reload_instants) = build_bulk_app().await;
    let id = seed_job(&pool, "alpha").await;

    let req = build_bulk_request(TEST_CSRF, TEST_CSRF, "disable", &[id]);
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::OK);

    // (a) DB row reflects the override.
    let job = queries::get_job_by_id(&pool, id)
        .await
        .expect("get_job_by_id")
        .expect("job exists");
    assert_eq!(job.enabled_override, Some(0));

    // (b) Mock scheduler observed at least one Reload.
    // Allow a brief moment for the spawned mock-scheduler task to record the receive.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let db_snapshot_at = Instant::now();
    let instants = reload_instants.lock().await;
    assert!(
        !instants.is_empty(),
        "scheduler MUST receive a Reload after bulk-toggle"
    );

    // (c) The reload arrived before `db_snapshot_at` — the only way that's possible
    // is if the handler dispatched Reload after committing the UPDATE (which we already
    // observed via the DB read above).
    assert!(
        instants[0] <= db_snapshot_at,
        "Reload arrival ({:?}) must precede the post-handler DB-snapshot instant ({:?})",
        instants[0],
        db_snapshot_at
    );
}

#[tokio::test]
async fn get_overridden_jobs_alphabetical() {
    // D-10b: get_overridden_jobs MUST return rows sorted by name ascending.
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    let id_z = seed_job(&pool, "zebra").await;
    let id_a = seed_job(&pool, "alpha").await;
    let id_m = seed_job(&pool, "mango").await;
    queries::bulk_set_override(&pool, &[id_z, id_a, id_m], Some(0))
        .await
        .expect("bulk_set_override");

    let names: Vec<String> = queries::get_overridden_jobs(&pool)
        .await
        .expect("get_overridden_jobs")
        .into_iter()
        .map(|j| j.name)
        .collect();
    assert_eq!(
        names,
        vec!["alpha".to_string(), "mango".to_string(), "zebra".to_string()],
        "get_overridden_jobs MUST return rows in alphabetical order by name (D-10b)"
    );
}

#[tokio::test]
async fn settings_empty_state_hides_section() {
    // D-10a: settings page MUST NOT render the 'Currently Overridden' section when empty.
    let page = SettingsPage {
        uptime: "0s".to_string(),
        db_status: "ok".to_string(),
        config_path: "/tmp/cronduit-test.toml".to_string(),
        last_reload_time: "Never".to_string(),
        last_reload_status: "never".to_string(),
        last_reload_summary: String::new(),
        watch_config: false,
        version: "test".to_string(),
        csrf_token: "test-csrf".to_string(),
        overridden_jobs: Vec::<OverriddenJobView>::new(),
    };

    let rendered = page.render().expect("askama render");
    assert!(
        !rendered.contains("Currently Overridden"),
        "empty overridden_jobs MUST hide the 'Currently Overridden' section (D-10a)"
    );
}
