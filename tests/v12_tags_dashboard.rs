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
    let (app, pool) = build_test_app().await;
    seed_job_with_tags(&pool, "alpha", "*/5 * * * *", &["weekly", "backup"]).await;
    seed_job_with_tags(&pool, "beta", "*/5 * * * *", &["backup", "prod"]).await;
    seed_job_with_tags(&pool, "gamma", "*/5 * * * *", &[]).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let body = std::str::from_utf8(&bytes).expect("utf-8");

    // All three distinct chips in alphabetical order. Chip text is
    // ` backup ` (with surrounding whitespace from the askama `{{ chip.tag }}`
    // expansion in the template's `<a>...</a>` block); we match the
    // unambiguous chip-anchor body via the chip `class="cd-tag-chip"`
    // attribute paired with the literal tag text occurring inside.
    assert!(
        body.contains(">\n    backup\n  </a>") || body.contains("> backup <"),
        "chip for `backup` must render inside its <a> tag; got body without that anchor"
    );
    assert!(
        body.contains(">\n    prod\n  </a>") || body.contains("> prod <"),
        "chip for `prod` must render inside its <a> tag"
    );
    assert!(
        body.contains(">\n    weekly\n  </a>") || body.contains("> weekly <"),
        "chip for `weekly` must render inside its <a> tag"
    );

    // Alphabetical order: backup < prod < weekly (in body text positions).
    let backup_pos = body
        .find(">\n    backup\n  </a>")
        .or_else(|| body.find("> backup <"))
        .expect("backup chip pos");
    let prod_pos = body
        .find(">\n    prod\n  </a>")
        .or_else(|| body.find("> prod <"))
        .expect("prod chip pos");
    let weekly_pos = body
        .find(">\n    weekly\n  </a>")
        .or_else(|| body.find("> weekly <"))
        .expect("weekly chip pos");
    assert!(
        backup_pos < prod_pos && prod_pos < weekly_pos,
        "chips must render in alphabetical order: backup({backup_pos}) < prod({prod_pos}) < weekly({weekly_pos})"
    );

    // The empty-tag job's row is still rendered (default load: no active
    // tag filter, so untagged jobs are visible per V-03).
    assert!(
        body.contains("gamma"),
        "untagged job `gamma` row must still render on default load (no active filter)"
    );

    // Chip strip wrapper is NOT hidden when the fleet has tags (D-02
    // empty-state only kicks in when fleet_tags.is_empty()).
    assert!(
        body.contains("id=\"cd-tag-chip-strip\""),
        "chip strip wrapper must render"
    );
    // Body must NOT contain a `hidden` attribute on the chip strip when
    // the fleet has at least one tag.
    let chip_strip_idx = body
        .find("id=\"cd-tag-chip-strip\"")
        .expect("chip strip idx");
    let chip_strip_open_tag_end = body[chip_strip_idx..]
        .find('>')
        .expect("chip strip open tag close");
    let chip_strip_open_tag = &body[chip_strip_idx..chip_strip_idx + chip_strip_open_tag_end];
    assert!(
        !chip_strip_open_tag.contains("hidden"),
        "chip strip wrapper must NOT carry `hidden` when fleet has tags; got: {chip_strip_open_tag}"
    );
}

// V-09: Active chip class + aria-pressed
#[tokio::test]
async fn chip_active_state_class() {
    let (app, pool) = build_test_app().await;
    seed_job_with_tags(&pool, "alpha", "*/5 * * * *", &["backup", "weekly"]).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/?tag=backup")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let body = std::str::from_utf8(&bytes).expect("utf-8");

    // Active chip for `backup` carries cd-tag-chip--active and
    // aria-pressed="true". Inactive chip for `weekly` carries
    // cd-tag-chip--inactive and aria-pressed="false".
    assert!(
        body.contains("cd-tag-chip--active"),
        "body must contain cd-tag-chip--active class for the active backup chip"
    );
    assert!(
        body.contains("cd-tag-chip--inactive"),
        "body must contain cd-tag-chip--inactive class for the inactive weekly chip"
    );
    assert!(
        body.contains("aria-pressed=\"true\""),
        "active chip must carry aria-pressed=\"true\""
    );
    assert!(
        body.contains("aria-pressed=\"false\""),
        "inactive chip must carry aria-pressed=\"false\""
    );

    // Verify that the active class is correctly attached to the BACKUP
    // anchor specifically — find the backup chip block and assert its
    // class string includes --active.
    let backup_anchor_text = ">\n    backup\n  </a>";
    let backup_idx = body.find(backup_anchor_text).expect("backup chip pos");
    // Look back ~500 chars to find the corresponding <a class="..."> tag.
    let scan_start = backup_idx.saturating_sub(500);
    let backup_block = &body[scan_start..backup_idx];
    assert!(
        backup_block.contains("cd-tag-chip--active"),
        "backup chip's <a> opening tag must carry cd-tag-chip--active; window: {backup_block}"
    );
}

// V-10: Direct URL paste renders chips active on first paint (bookmarkable)
#[tokio::test]
async fn direct_url_renders_chips_active() {
    let (app, pool) = build_test_app().await;
    seed_job_with_tags(&pool, "alpha", "*/5 * * * *", &["backup", "weekly"]).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/?tag=backup&tag=weekly")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let body = std::str::from_utf8(&bytes).expect("utf-8");

    // Both chips active on first paint (bookmarkable URL state).
    // Two `cd-tag-chip--active` occurrences total — one per active tag —
    // and zero `cd-tag-chip--inactive` since the only fleet tags are
    // both active.
    let active_count = body.matches("cd-tag-chip--active").count();
    let inactive_count = body.matches("cd-tag-chip--inactive").count();
    assert_eq!(
        active_count, 2,
        "direct paste of `?tag=backup&tag=weekly` must render BOTH chips active on first paint; \
         got {active_count} active occurrences. Both fleet tags are in the active set."
    );
    assert_eq!(
        inactive_count, 0,
        "no inactive chip when every fleet tag is in the active set; got {inactive_count} \
         inactive occurrences"
    );

    // Hidden inputs round-trip the active set for HTMX poll preservation
    // (V-14 covers that explicitly; this test asserts both inputs render).
    assert!(
        body.contains("<input type=\"hidden\" name=\"tag\" value=\"backup\">"),
        "hidden input for backup must render so the 3s poll's hx-include can pick it up"
    );
    assert!(
        body.contains("<input type=\"hidden\" name=\"tag\" value=\"weekly\">"),
        "hidden input for weekly must render"
    );
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
    let (app, pool) = build_test_app().await;
    seed_job_with_tags(&pool, "alpha", "*/5 * * * *", &["backup", "weekly"]).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let body = std::str::from_utf8(&bytes).expect("utf-8");

    // Extract the chip strip block (from id="cd-tag-chip-strip" up to the
    // matching </div>). We scan from the wrapper open to the first </div>
    // that closes it — the chip strip contains only chip <a> tags + hidden
    // <input> tags; no nested <div>s.
    let strip_start = body
        .find("id=\"cd-tag-chip-strip\"")
        .expect("chip strip wrapper");
    let strip_end_rel = body[strip_start..]
        .find("</div>")
        .expect("chip strip close </div>");
    let chip_strip_block = &body[strip_start..strip_start + strip_end_rel];

    // Each chip <a> tag must carry the cd-tag-chip class.
    assert!(
        chip_strip_block.contains("class=\"cd-tag-chip"),
        "chip strip block must render <a class=\"cd-tag-chip ...\"> per UI-SPEC; got: \
         {chip_strip_block}"
    );

    // No inline JS: no `onclick=` attribute on any chip anchor.
    assert!(
        !chip_strip_block.contains("onclick"),
        "chip <a> tags MUST NOT carry onclick=; CSS-only toggle per TAG-08. Found inline JS in \
         chip strip block: {chip_strip_block}"
    );

    // No <script> tag inside the chip strip block itself.
    assert!(
        !chip_strip_block.contains("<script"),
        "chip strip MUST NOT include any <script> tag; CSS-only per TAG-08. Block: \
         {chip_strip_block}"
    );
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
    let (app, pool) = build_test_app().await;
    seed_job_with_tags(&pool, "alpha", "*/5 * * * *", &["backup"]).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/?tag=backup&sort=name&order=desc")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let body = std::str::from_utf8(&bytes).expect("utf-8");

    // Sort-header href carries the active tag — RESEARCH § Pitfall 3.
    // The askama `urlencode` filter in the template emits `backup` verbatim
    // (alphanumeric — no escape needed). All four sortable columns + both
    // attributes (href + hx-get) per column = 8 occurrences of `&tag=backup`
    // in the body. The chip strip itself uses Rust-side `form_urlencoded`
    // which emits a single `tag=backup` (no leading `&`, since it's the
    // first param after `filter` / `sort` / `order`).
    let sort_header_carries = body.matches("&tag=backup").count();
    assert!(
        sort_header_carries >= 8,
        "Sort-header anchors must carry `&tag=backup` in BOTH href and hx-get for ALL FOUR \
         sortable columns (Name / Next Fire / Status / Last Run) — expected ≥ 8 occurrences \
         of `&tag=backup` (4 columns × 2 attributes); got {sort_header_carries}. RESEARCH § \
         Pitfall 3 regression lock."
    );

    // The body's Name sort anchor specifically must round-trip the tag.
    // Anchor-uniquely identifiable substring: `sort=name&order=` appears
    // ONLY in the Name sort header (twice: once in href, once in hx-get).
    let name_sort_idx = body
        .find("sort=name&order=")
        .expect("Name sort anchor href");
    // The active-tag suffix `&tag=backup` follows the href value (after
    // `&order=desc` since URL is ?sort=name&order=desc → next click flips
    // to asc; either way `&tag=backup` is appended in the active_tags
    // for-loop). Scan ~200 chars after the sort=name marker.
    let scan_end = (name_sort_idx + 300).min(body.len());
    let name_block = &body[name_sort_idx..scan_end];
    assert!(
        name_block.contains("&tag=backup"),
        "Name sort header anchor block must include `&tag=backup` in its href; window from \
         `sort=name&order=` was: {name_block}"
    );
}

// V-14: Hidden <input name="tag"> rendered for each active tag; poll hx-include lists [name='tag']
#[tokio::test]
async fn poll_hx_include_widened() {
    let (app, pool) = build_test_app().await;
    seed_job_with_tags(&pool, "alpha", "*/5 * * * *", &["backup"]).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/?tag=backup")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let body = std::str::from_utf8(&bytes).expect("utf-8");

    // Hidden input rendered for each active tag (D-12).
    assert!(
        body.contains("<input type=\"hidden\" name=\"tag\" value=\"backup\">"),
        "Hidden `<input name=\"tag\" value=\"backup\">` must render inside the chip strip so the \
         3s poll's hx-include selector picks it up"
    );

    // The 3s poll on #job-table-body has hx-include widened with [name='tag'].
    assert!(
        body.contains("[name='tag']"),
        "tbody#job-table-body's hx-include must include `[name='tag']` so polling preserves \
         the active filter set per D-12 / RESEARCH § Pitfall 3"
    );

    // Confirm the hx-include selector list contains all four name selectors,
    // i.e., the widening did not REPLACE the existing list.
    let tbody_idx = body
        .find("id=\"job-table-body\"")
        .expect("tbody job-table-body");
    let tbody_block_end_rel = body[tbody_idx..].find('>').expect("tbody open close");
    let tbody_block = &body[tbody_idx..tbody_idx + tbody_block_end_rel + 1];
    assert!(
        tbody_block.contains("[name='filter']")
            && tbody_block.contains("[name='sort']")
            && tbody_block.contains("[name='order']")
            && tbody_block.contains("[name='tag']"),
        "tbody hx-include must list all four [name='X'] selectors (filter/sort/order/tag) — \
         widening must EXTEND not REPLACE the existing list. Got: {tbody_block}"
    );
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
