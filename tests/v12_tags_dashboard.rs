//! Phase 23 Integration Tests — Job Tagging Dashboard Filter Chips
//!
//! Covers TAG-06, TAG-07, TAG-08 (REQUIREMENTS.md). Validation matrix:
//! V-01..V-04, V-06, V-08..V-14 (`23-VALIDATION.md`).
//!
//! Run: `cargo test --test v12_tags_dashboard`
//! Compile gate: `cargo test --test v12_tags_dashboard --no-run`
//!
//! Wave 0: this file is initially scaffolded with `todo!()` bodies so
//! Wave 1-3 plans can wire implementations against named test functions
//! that already compile. The compile gate is what runs in Wave 0; the
//! tests THEMSELVES go green as Wave 1-3 lands.

use std::sync::{Arc, Mutex};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use cronduit::db::DbPool;
use cronduit::db::queries;
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::telemetry::setup_metrics;
use cronduit::web::{AppState, ReloadState, router};

// ---- Helpers ---------------------------------------------------------------

#[allow(dead_code)]
async fn build_test_app() -> (axum::Router, DbPool) {
    // VERBATIM copy from tests/dashboard_render.rs::build_test_app
    // (in-memory sqlite + AppState construction). The Wave 1-3 executors
    // will keep this helper as-is; only the test bodies change.
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    // Sink for scheduler commands — the dashboard tests only exercise GET /.
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);
    tokio::spawn(async move { while cmd_rx.recv().await.is_some() {} });

    let metrics_handle = setup_metrics();

    let state = AppState {
        started_at: chrono::Utc::now(),
        version: "test",
        pool: pool.clone(),
        cmd_tx,
        config_path: std::path::PathBuf::from("/tmp/cronduit-test.toml"),
        tz: chrono_tz::UTC,
        last_reload: Arc::new(Mutex::new(None::<ReloadState>)),
        watch_config: false,
        metrics_handle,
        active_runs: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    (router(state), pool)
}

#[allow(dead_code)]
async fn seed_job(pool: &DbPool, name: &str, schedule: &str) -> i64 {
    // VERBATIM copy from tests/dashboard_render.rs::seed_job — passes
    // tags_json = "[]" (post-P22 default).
    queries::upsert_job(
        pool, name, schedule, schedule, "command", "{}", "deadbeef", 300, "[]",
    )
    .await
    .expect("upsert job")
}

#[allow(dead_code)]
async fn seed_job_with_tags(pool: &DbPool, name: &str, schedule: &str, tags: &[&str]) -> i64 {
    // Phase 23: sorted-canonical JSON form per Phase 22 D-09. Sort + dedup at
    // the helper boundary so callers can pass tags in any order — the upsert
    // path's row produces the same canonical column value regardless of input
    // order, matching what production validators emit at config-load.
    let mut sorted: Vec<String> = tags.iter().map(|s| s.to_string()).collect();
    sorted.sort();
    sorted.dedup();
    let tags_json = serde_json::to_string(&sorted).expect("serialize tags");
    queries::upsert_job(
        pool, name, schedule, schedule, "command", "{}", "deadbeef", 300, &tags_json,
    )
    .await
    .expect("upsert job")
}

// ---- Tests -----------------------------------------------------------------
//
// Function names are LOAD-BEARING — the VALIDATION matrix's `cargo test`
// commands use them VERBATIM (see 23-VALIDATION.md). Each body is `todo!()`
// until the corresponding Wave 1-3 plan fills it in. The ignore attribute
// is deliberately omitted — Wave-end gates surface missing implementations
// as panics.

// V-08: Dashboard renders one chip per distinct fleet tag, alphabetical, hidden when empty
#[tokio::test]
async fn chip_strip_render() {
    todo!(
        "Wave 2: seed three jobs with [\"weekly\",\"backup\"], [\"backup\",\"prod\"], [] → \
         GET / → assert body contains all three distinct chips in alphabetical order; \
         the empty-tag job's row is still rendered"
    )
}

// V-09: Active chip class + aria-pressed
#[tokio::test]
async fn chip_active_state_class() {
    todo!(
        "Wave 2: GET /?tag=backup → assert chip for `backup` has cd-tag-chip--active + \
         aria-pressed=\"true\"; chip for `weekly` has cd-tag-chip--inactive + \
         aria-pressed=\"false\""
    )
}

// V-10: Direct URL paste renders chips active on first paint (bookmarkable)
#[tokio::test]
async fn direct_url_renders_chips_active() {
    todo!(
        "Wave 2: GET /?tag=backup&tag=weekly → first-paint response has both chips active"
    )
}

// V-01: AND-tag SQL filters correctly
#[tokio::test]
async fn and_filter_two_tags() {
    let (_app, pool) = build_test_app().await;
    let _a = seed_job_with_tags(&pool, "alpha-A", "*/5 * * * *", &["backup", "weekly"]).await;
    let _b = seed_job_with_tags(&pool, "beta-B", "*/5 * * * *", &["backup"]).await;
    let _c = seed_job_with_tags(&pool, "gamma-C", "*/5 * * * *", &["weekly"]).await;

    let active = vec!["backup".to_string(), "weekly".to_string()];
    let rows = queries::get_dashboard_jobs(&pool, None, "name", "asc", &active)
        .await
        .expect("get_dashboard_jobs");

    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert!(
        names.contains(&"alpha-A"),
        "must contain alpha-A (has both backup + weekly); got {:?}",
        names
    );
    assert!(
        !names.contains(&"beta-B"),
        "must NOT contain beta-B (missing weekly); got {:?}",
        names
    );
    assert!(
        !names.contains(&"gamma-C"),
        "must NOT contain gamma-C (missing backup); got {:?}",
        names
    );
}

// V-02: Untagged jobs hidden when active set non-empty
#[tokio::test]
async fn untagged_hidden_when_filter_active() {
    let (_app, pool) = build_test_app().await;
    let _a = seed_job_with_tags(&pool, "alpha-A", "*/5 * * * *", &["backup"]).await;
    let _b = seed_job_with_tags(&pool, "beta-B", "*/5 * * * *", &[]).await;

    let active = vec!["backup".to_string()];
    let rows = queries::get_dashboard_jobs(&pool, None, "name", "asc", &active)
        .await
        .expect("get_dashboard_jobs");

    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert!(
        names.contains(&"alpha-A"),
        "must contain alpha-A (matches backup); got {:?}",
        names
    );
    assert!(
        !names.contains(&"beta-B"),
        "must NOT contain beta-B (untagged hidden when active set non-empty); got {:?}",
        names
    );
}

// V-03: No filter → all jobs (tagged + untagged) shown (no regression on default load)
#[tokio::test]
async fn no_filter_shows_all_jobs() {
    let (_app, pool) = build_test_app().await;
    let _a = seed_job_with_tags(&pool, "alpha-A", "*/5 * * * *", &["backup"]).await;
    let _b = seed_job_with_tags(&pool, "beta-B", "*/5 * * * *", &[]).await;

    let rows = queries::get_dashboard_jobs(&pool, None, "name", "asc", &[])
        .await
        .expect("get_dashboard_jobs");

    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert!(
        names.contains(&"alpha-A"),
        "must contain alpha-A (default load shows all); got {:?}",
        names
    );
    assert!(
        names.contains(&"beta-B"),
        "must contain beta-B (default load shows untagged); got {:?}",
        names
    );
}

// V-04: Tag filter composes with name filter via AND
#[tokio::test]
async fn and_with_name_filter() {
    let (_app, pool) = build_test_app().await;
    let _a = seed_job_with_tags(&pool, "prod-backup", "*/5 * * * *", &["backup"]).await;
    let _b = seed_job_with_tags(&pool, "dev-backup", "*/5 * * * *", &["backup"]).await;
    let _c = seed_job_with_tags(&pool, "prod-cleanup", "*/5 * * * *", &["backup"]).await;

    let active = vec!["backup".to_string()];
    let rows = queries::get_dashboard_jobs(&pool, Some("prod"), "name", "asc", &active)
        .await
        .expect("get_dashboard_jobs");

    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert!(
        names.contains(&"prod-backup"),
        "must contain prod-backup (matches both prod and backup); got {:?}",
        names
    );
    assert!(
        names.contains(&"prod-cleanup"),
        "must contain prod-cleanup (matches both prod and backup); got {:?}",
        names
    );
    assert!(
        !names.contains(&"dev-backup"),
        "must NOT contain dev-backup (matches backup but not prod name filter); got {:?}",
        names
    );
}

// V-06: Stale tag (not in fleet_tags) silent-dropped at handler
#[tokio::test]
async fn stale_tag_silent_drop() {
    todo!(
        "Wave 2: seed A=[\"backup\"] → GET /?tag=backup&tag=ghost → body renders only \
         `backup` chip; `ghost` does not appear; A is rendered (not filtered out by the \
         unknown tag); response is 200 OK"
    )
}

// V-11: Chip element has correct CSS classes; no inline JS introduced
#[tokio::test]
async fn css_only_chip_no_inline_js() {
    todo!(
        "Wave 2: GET / → body contains `class=\"cd-tag-chip ...\"` for each chip; no \
         <script> tag immediately around the chip strip; chip <a> tags have no \
         `onclick=` attribute"
    )
}

// V-12: HTMX response renders BOTH `#cd-tag-chip-strip[hx-swap-oob="true"]` AND `#job-table-body` content
#[tokio::test]
async fn oob_response_shape() {
    todo!(
        "Wave 2: GET /?tag=backup with HX-Request: true header → body contains \
         id=\"cd-tag-chip-strip\" AND hx-swap-oob=\"true\" AND the table-body rows"
    )
}

// V-13: Sort-header href + hx-get both contain &tag=... for every active tag
#[tokio::test]
async fn sort_header_carries_active_tags() {
    todo!(
        "Wave 2: GET /?tag=backup&sort=name&order=desc → body's Name sort anchor href \
         AND hx-get both contain `&tag=backup`"
    )
}

// V-14: Hidden <input name="tag"> rendered for each active tag; poll hx-include lists [name='tag']
#[tokio::test]
async fn poll_hx_include_widened() {
    todo!(
        "Wave 2: GET /?tag=backup → body contains \
         <input type=\"hidden\" name=\"tag\" value=\"backup\"> AND the tbody#job-table-body \
         has hx-include attribute that contains `[name='tag']`"
    )
}

// ---- Compile-only references to silence dead-code warnings -----------------
//
// Wave 1-3 will use Request/Body/StatusCode/to_bytes/ServiceExt/build_test_app/seed_job
// inside the test bodies once they replace `todo!()`. To keep `cargo test --no-run`
// quiet about unused imports during Wave 0, this no-op function exercises each
// symbol once. It is `#[allow(dead_code)]` and never called.
#[allow(dead_code)]
async fn _wave0_compile_anchor() {
    let (app, pool) = build_test_app().await;
    let _id = seed_job(&pool, "anchor", "*/5 * * * *").await;
    let req = Request::builder()
        .method("GET")
        .uri("/")
        .body(Body::empty())
        .expect("build request");
    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
    let _bytes = to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body bytes");
}
