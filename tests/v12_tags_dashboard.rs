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
    // NEW for Phase 23: serializes `tags` to sorted-canonical JSON via
    // serde_json::to_string and passes it as the tags_json arg to upsert_job.
    // Matches Phase 22 D-09 sorted-canonical storage form. Wave 1 fills
    // this in.
    let _ = (pool, name, schedule, tags);
    todo!("Wave 0: implement in Wave 1 — serde_json::to_string of sorted+deduped Vec<String>")
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
    todo!(
        "Wave 1: seed A=[\"backup\",\"weekly\"], B=[\"backup\"], C=[\"weekly\"] → \
         GET /?tag=backup&tag=weekly → body contains A, NOT B, NOT C"
    )
}

// V-02: Untagged jobs hidden when active set non-empty
#[tokio::test]
async fn untagged_hidden_when_filter_active() {
    todo!(
        "Wave 1: seed A=[\"backup\"], B=[] → GET /?tag=backup → body contains A, NOT B"
    )
}

// V-03: No filter → all jobs (tagged + untagged) shown (no regression on default load)
#[tokio::test]
async fn no_filter_shows_all_jobs() {
    todo!(
        "Wave 1: seed A=[\"backup\"], B=[] → GET / → body contains BOTH A AND B"
    )
}

// V-04: Tag filter composes with name filter via AND
#[tokio::test]
async fn and_with_name_filter() {
    todo!(
        "Wave 1: seed prod-backup=[\"backup\"], dev-backup=[\"backup\"], \
         prod-cleanup=[\"backup\"] → GET /?filter=prod&tag=backup → \
         body contains prod-backup + prod-cleanup, NOT dev-backup"
    )
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
