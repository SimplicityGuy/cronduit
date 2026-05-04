//! Integration tests for the Job Detail Exit-Code Histogram card (Phase 21
//! EXIT-01..EXIT-05). Test name: `v12_exit_histogram`.
//!
//! Bucket categorization + classifier rules (EXIT-02..EXIT-05 math) are
//! covered by the in-module unit tests in `src/web/exit_buckets.rs`
//! (plan 21-03). This file adds RENDER-LEVEL coverage only — it asserts the
//! locked CSS class names, copy strings, and DOM structure on the rendered
//! HTML body returned from `GET /jobs/{job_id}` per UI-SPEC § Component
//! Inventory § 2.
//!
//! Test harness mirrors `tests/v13_timeline_render.rs::build_test_app` (axum
//! Router + in-memory SQLite pool) and the duration-card seed pattern in
//! `tests/v13_duration_card.rs::seed_runs_with_duration` (extended with
//! `status` + `exit_code`).
//!
//! Tests:
//!   1. `empty_state_below_5_samples`               — EXIT-01 / D-15
//!   2. `empty_state_brand_new_job_zero_runs`       — EXIT-01 / D-16
//!   3. `chart_renders_with_5_or_more_samples`      — EXIT-01 / EXIT-03
//!   4. `stopped_bucket_renders_distinct_color_class` — EXIT-04
//!   5. `success_rate_badge_renders`                — EXIT-03
//!   6. `recent_codes_subtable_renders`             — EXIT-05
//!   7. `bucket_short_labels_render_under_bars`     — EXIT-02 / EXIT-04

use std::sync::{Arc, Mutex};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tower::ServiceExt; // .oneshot()

use cronduit::db::DbPool;
use cronduit::db::queries::{self, PoolRef};
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::telemetry::setup_metrics;
use cronduit::web::{AppState, ReloadState, router};

// ---------------------------------------------------------------------------
// Test app + seed helpers
// ---------------------------------------------------------------------------

async fn build_test_app() -> (axum::Router, DbPool) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    // Sink for scheduler commands — these tests only exercise GET /jobs/{id}.
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

async fn seed_test_job(pool: &DbPool, name: &str) -> i64 {
    queries::upsert_job(
        pool,
        name,
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo hello"}"#,
        &format!("hash-{name}"),
        3600,
        "[]",
    )
    .await
    .expect("upsert test job")
}

/// Insert `count` `job_runs` rows directly via raw SQL with the given
/// `status` and `exit_code`. Bypasses `insert_running_run` + `finalize_run`
/// so every column the histogram fetch reads (`status`, `exit_code`,
/// `end_time`) is fully deterministic. Mirrors the
/// `seed_runs_with_status_and_exit` shape locked in plan 21-08's
/// `<interfaces>` block.
async fn seed_runs_with_status_and_exit(
    pool: &DbPool,
    job_id: i64,
    count: usize,
    status: &str,
    exit_code: Option<i32>,
) {
    let writer = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        PoolRef::Postgres(_) => panic!("sqlite-only test fixture"),
    };

    for i in 0..count {
        // Distinct timestamps per row so ORDER BY start_time DESC is
        // deterministic across the seed. Using a tight 1-second cadence keeps
        // the relative-time formatter ("just now"/"N minutes"/"N hours")
        // stable across test runs.
        let start = chrono::Utc::now() - chrono::Duration::seconds((count - i) as i64);
        let end = start + chrono::Duration::seconds(30);
        let start_str = start.to_rfc3339();
        let end_str = end.to_rfc3339();

        // Advance the job's next_run_number counter so job_run_number stays
        // unique under the (job_id, job_run_number) UNIQUE INDEX.
        let run_number: i64 = sqlx::query_scalar(
            "UPDATE jobs SET next_run_number = next_run_number + 1 \
             WHERE id = ?1 RETURNING next_run_number - 1",
        )
        .bind(job_id)
        .fetch_one(writer)
        .await
        .expect("advance next_run_number");

        let _: i64 = sqlx::query_scalar(
            "INSERT INTO job_runs \
               (job_id, status, trigger, start_time, end_time, duration_ms, exit_code, \
                job_run_number, config_hash) \
             VALUES (?1, ?2, 'manual', ?3, ?4, 30000, ?5, ?6, 'seed-hash') \
             RETURNING id",
        )
        .bind(job_id)
        .bind(status)
        .bind(&start_str)
        .bind(&end_str)
        .bind(exit_code)
        .bind(run_number)
        .fetch_one(writer)
        .await
        .expect("insert seeded run");
    }
}

/// GET /jobs/{id} and return (status, body-as-utf8-String).
async fn get_job_detail(app: axum::Router, job_id: i64) -> (StatusCode, String) {
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/jobs/{}", job_id))
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");

    let status = response.status();
    let body_bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body bytes");
    let body = String::from_utf8(body_bytes.to_vec()).expect("body utf8");
    (status, body)
}

// ---------------------------------------------------------------------------
// Test 1 — Empty state below the 5-sample threshold (EXIT-01 / D-15)
// ---------------------------------------------------------------------------

/// Seeds 4 runs (below the locked N=5 threshold). The card heading must
/// always render, the locked empty-state copy "Need 5+ samples; have 4"
/// must render, and the histogram chart + stats blocks must NOT render
/// (the template's `{% if has_min_samples %}` short-circuits).
#[tokio::test]
async fn empty_state_below_5_samples() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "below-threshold-job").await;

    seed_runs_with_status_and_exit(&pool, job_id, 4, "failed", Some(1)).await;

    let (status, body) = get_job_detail(app, job_id).await;
    assert_eq!(status, StatusCode::OK);

    // Outer card chrome + heading always render.
    assert!(
        body.contains("cd-exit-card"),
        "card chrome class must always render; body excerpt:\n{body}"
    );
    assert!(
        body.contains("Exit Code Distribution"),
        "card heading must always render"
    );

    // Locked empty-state copy with sample_count=4.
    assert!(
        body.contains("Need 5+ samples; have 4"),
        "below-N=5 must render the locked empty-state copy with the actual sample_count; \
         body excerpt:\n{body}"
    );
    assert!(
        body.contains("cd-exit-empty"),
        "empty-state container class must render"
    );

    // Chart + stats branches must NOT render below threshold.
    assert!(
        !body.contains("cd-exit-chart"),
        "chart container must be hidden in empty state (template `{{% if has_min_samples %}}`)"
    );
    assert!(
        !body.contains("cd-exit-stats"),
        "stats container must be hidden in empty state"
    );
    assert!(
        !body.contains("Most frequent codes"),
        "recent-codes sub-table must be hidden in empty state"
    );
    assert!(
        !body.contains("(window: 100). Hover bars for detail."),
        "caption must be hidden in empty state"
    );
}

// ---------------------------------------------------------------------------
// Test 2 — Brand-new job (zero runs) — D-16
// ---------------------------------------------------------------------------

/// Seeds 0 runs. Per D-16, brand-new jobs render the same empty-state path
/// as below-N=5 jobs. Locked copy is "Need 5+ samples; have 0".
#[tokio::test]
async fn empty_state_brand_new_job_zero_runs() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "brand-new-job").await;

    let (status, body) = get_job_detail(app, job_id).await;
    assert_eq!(status, StatusCode::OK);

    assert!(
        body.contains("Need 5+ samples; have 0"),
        "brand-new (zero-run) job must render \"Need 5+ samples; have 0\" per D-16; \
         body excerpt:\n{body}"
    );
    // Empty-state container present.
    assert!(
        body.contains("cd-exit-empty"),
        "empty-state container class must render for zero-run job"
    );
    // Chart + recent-codes hidden.
    assert!(
        !body.contains("cd-exit-chart"),
        "chart container must be hidden for zero-run job"
    );
    assert!(
        !body.contains("Most frequent codes"),
        "recent-codes sub-table must be hidden for zero-run job"
    );
}

// ---------------------------------------------------------------------------
// Test 3 — Chart renders when N >= 5 (EXIT-01 / EXIT-03)
// ---------------------------------------------------------------------------

/// Seeds 5 mixed-status runs (3 success, 2 failed exit=1). At threshold:
/// chart container, at least one bar, the SUCCESS stat label, and the
/// caption must all render.
#[tokio::test]
async fn chart_renders_with_5_or_more_samples() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "at-threshold-job").await;

    seed_runs_with_status_and_exit(&pool, job_id, 3, "success", Some(0)).await;
    seed_runs_with_status_and_exit(&pool, job_id, 2, "failed", Some(1)).await;

    let (status, body) = get_job_detail(app, job_id).await;
    assert_eq!(status, StatusCode::OK);

    // Chart container + at least one bar.
    assert!(
        body.contains("cd-exit-chart"),
        "chart container must render at N>=5; body excerpt:\n{body}"
    );
    assert!(
        body.contains("cd-exit-bar"),
        "at least one bar element must render at N>=5"
    );

    // Stats block + SUCCESS label.
    assert!(
        body.contains("cd-exit-stats"),
        "stats container must render at N>=5"
    );
    assert!(
        body.contains("SUCCESS"),
        "SUCCESS stat label must render at N>=5"
    );

    // Locked caption copy.
    assert!(
        body.contains("(window: 100). Hover bars for detail."),
        "locked caption copy must render at N>=5; body excerpt:\n{body}"
    );

    // The empty-state copy must NOT render at threshold.
    assert!(
        !body.contains("Need 5+ samples; have"),
        "empty-state copy must NOT render at N>=5"
    );
}

// ---------------------------------------------------------------------------
// Test 4 — Stopped bucket renders distinct color class (EXIT-04)
// ---------------------------------------------------------------------------

/// EXIT-04 dual-classifier (RENDER level — math is in plan 21-03 unit
/// tests). Seeds 5 status='stopped' runs with exit_code=137 + 5
/// status='failed' runs with exit_code=137. Both buckets render with
/// distinct color classes (`cd-exit-bar--stopped` for BucketStopped,
/// `cd-exit-bar--warn` for Bucket128to143). The BucketStopped tooltip
/// detail carries the locked override copy with the substring
/// "NOT a crash".
#[tokio::test]
async fn stopped_bucket_renders_distinct_color_class() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "exit-137-dual-job").await;

    // Status discriminator wins: stopped+137 → BucketStopped.
    seed_runs_with_status_and_exit(&pool, job_id, 5, "stopped", Some(137)).await;
    // Status='failed'+137 → Bucket128to143 (signal-killed; NOT operator-stop).
    seed_runs_with_status_and_exit(&pool, job_id, 5, "failed", Some(137)).await;

    let (status, body) = get_job_detail(app, job_id).await;
    assert_eq!(status, StatusCode::OK);

    // Both bucket modifier classes must render — proves the dual classifier
    // emitted DIFFERENT buckets for the same exit code.
    assert!(
        body.contains("cd-exit-bar--stopped"),
        "BucketStopped color class must render for status='stopped'+137; \
         body excerpt:\n{body}"
    );
    assert!(
        body.contains("cd-exit-bar--warn"),
        "Bucket128to143 color class (`warn`) must render for status='failed'+137; \
         body excerpt:\n{body}"
    );

    // BucketStopped's tooltip-detail override copy must mention "NOT a crash"
    // — UI-SPEC § Copywriting Contract for the BucketStopped tooltip / aria.
    assert!(
        body.contains("NOT a crash"),
        "BucketStopped aria/tooltip must contain the locked \"NOT a crash\" \
         distinguisher copy; body excerpt:\n{body}"
    );
}

// ---------------------------------------------------------------------------
// Test 5 — Success-rate stat badge (NOT a histogram bar) — EXIT-03
// ---------------------------------------------------------------------------

/// EXIT-03 success-rate stat as a separate badge (D-07: no Success enum
/// variant in ExitBucket — success is reported via the stat row, NOT as a
/// histogram bar). Seeds 8 successes + 2 failures (exit=1). Expected
/// success_rate = 8 / (10 - 0) = 0.8 → "80%".
///
/// Asserts: the SUCCESS label + the "80%" rendered display value render in
/// the stats row, AND the rendered HTML does NOT contain a
/// `cd-exit-bar--success` class (no Success bucket exists per D-07).
#[tokio::test]
async fn success_rate_badge_renders() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "success-rate-job").await;

    seed_runs_with_status_and_exit(&pool, job_id, 8, "success", Some(0)).await;
    seed_runs_with_status_and_exit(&pool, job_id, 2, "failed", Some(1)).await;

    let (status, body) = get_job_detail(app, job_id).await;
    assert_eq!(status, StatusCode::OK);

    // SUCCESS label + 80% value render in the stats row.
    assert!(
        body.contains("SUCCESS"),
        "SUCCESS stat label must render in the stats row"
    );
    assert!(
        body.contains("cd-exit-stat-value"),
        "stats row value class must render"
    );
    assert!(
        body.contains("80%"),
        "success_rate_display must round to \"80%\" for 8 success / 2 failed; \
         body excerpt:\n{body}"
    );
    // Meta line: "{success_count} of {sample_count}" → "8 of 10".
    assert!(
        body.contains("8 of 10"),
        "stats meta must render \"{{success_count}} of {{sample_count}}\" → \"8 of 10\"; \
         body excerpt:\n{body}"
    );

    // No Success-bucket bar — per D-07 success is the stat badge, not a
    // histogram bar.
    assert!(
        !body.contains("cd-exit-bar--success"),
        "no `cd-exit-bar--success` class must render — success has no bucket per D-07; \
         body excerpt:\n{body}"
    );
}

// ---------------------------------------------------------------------------
// Test 6 — Recent-codes sub-table (EXIT-05)
// ---------------------------------------------------------------------------

/// EXIT-05 top-3 codes with last-seen. Seeds 5 failed runs with mixed exit
/// codes (1, 1, 127, 143, 143). After aggregation: code 1 (count 2),
/// code 143 (count 2; tied with 1, broken by code ASC), code 127 (count
/// 1) → all three should render in the sub-table.
///
/// Asserts: the "Most frequent codes" heading, the `cd-exit-recent` table
/// class, and the locked code labels (`127 (command not found)` and
/// `143 (SIGTERM)`) render in the rendered HTML.
#[tokio::test]
async fn recent_codes_subtable_renders() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "recent-codes-job").await;

    // 5 runs, all failed, mixed codes — exact distribution: 1×2, 127×1, 143×2.
    seed_runs_with_status_and_exit(&pool, job_id, 2, "failed", Some(1)).await;
    seed_runs_with_status_and_exit(&pool, job_id, 1, "failed", Some(127)).await;
    seed_runs_with_status_and_exit(&pool, job_id, 2, "failed", Some(143)).await;

    let (status, body) = get_job_detail(app, job_id).await;
    assert_eq!(status, StatusCode::OK);

    // Heading + table class.
    assert!(
        body.contains("Most frequent codes"),
        "EXIT-05 sub-table heading must render; body excerpt:\n{body}"
    );
    assert!(
        body.contains("cd-exit-recent"),
        "EXIT-05 sub-table class must render"
    );

    // Locked code labels per UI-SPEC § Copywriting Contract.
    assert!(
        body.contains("127 (command not found)"),
        "code 127 must render with the locked label \"127 (command not found)\"; \
         body excerpt:\n{body}"
    );
    assert!(
        body.contains("143 (SIGTERM)"),
        "code 143 must render with the locked label \"143 (SIGTERM)\"; \
         body excerpt:\n{body}"
    );

    // Standard column headers.
    assert!(
        body.contains(">Code<"),
        "table must render Code column header"
    );
    assert!(
        body.contains(">Count<"),
        "table must render Count column header"
    );
    assert!(
        body.contains(">Last seen<"),
        "table must render Last seen column header"
    );
}

// ---------------------------------------------------------------------------
// Test 7 — All 10 bucket short labels render under bars (EXIT-02 / EXIT-04)
// ---------------------------------------------------------------------------

/// EXIT-02 + EXIT-04 (RENDER level). Seeds 5+ mixed runs covering at least
/// 3 distinct buckets. Per UI-SPEC § Component Inventory, all 10 bucket
/// short labels render under their columns even when the count is 0 — the
/// view-model builder produces a 10-entry display-order Vec regardless of
/// per-bucket counts.
///
/// Asserts: every one of the 10 locked short labels appears in the
/// rendered HTML (`1`, `2`, `3-9`, `10-126`, `127`, `128-143`, `144-254`,
/// `255`, `none`, `stopped`).
#[tokio::test]
async fn bucket_short_labels_render_under_bars() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "all-buckets-job").await;

    // Seed enough runs to clear N>=5 and cover 3 distinct buckets.
    seed_runs_with_status_and_exit(&pool, job_id, 2, "failed", Some(1)).await;
    seed_runs_with_status_and_exit(&pool, job_id, 2, "failed", Some(127)).await;
    seed_runs_with_status_and_exit(&pool, job_id, 1, "stopped", Some(137)).await;

    let (status, body) = get_job_detail(app, job_id).await;
    assert_eq!(status, StatusCode::OK);

    // Every short label per UI-SPEC § Component Inventory § "10 bucket
    // short-labels (locked)". Each label renders inside a
    // `<div class="cd-exit-bucket-label">{label}</div>` even at count=0.
    let bucket_label_class = "cd-exit-bucket-label";
    assert!(
        body.contains(bucket_label_class),
        "bucket label class must render"
    );

    // One assertion per locked short label — line-per-label keeps each label
    // independently traceable in failure output and lets greps pin the test
    // to specific buckets. Each `let` binding holds the locked short-label
    // string verbatim (UI-SPEC § Component Inventory § "10 bucket
    // short-labels (locked)") so the assertion target is grep-friendly.
    let bucket_label_1 = "1";
    let bucket_label_2 = "2";
    let bucket_label_3_9 = "3-9";
    let bucket_label_10_126 = "10-126";
    let bucket_label_127 = "127";
    let bucket_label_128_143 = "128-143";
    let bucket_label_144_254 = "144-254";
    let bucket_label_255 = "255";
    let bucket_label_none = "none";
    let bucket_label_stopped = "stopped";
    assert!(
        body.contains(&format!(">{bucket_label_1}</div>")),
        "short label `1` must render inside cd-exit-bucket-label"
    );
    assert!(
        body.contains(&format!(">{bucket_label_2}</div>")),
        "short label `2` must render inside cd-exit-bucket-label"
    );
    assert!(
        body.contains(&format!(">{bucket_label_3_9}</div>")),
        "short label `3-9` must render inside cd-exit-bucket-label"
    );
    assert!(
        body.contains(&format!(">{bucket_label_10_126}</div>")),
        "short label `10-126` must render inside cd-exit-bucket-label"
    );
    assert!(
        body.contains(&format!(">{bucket_label_127}</div>")),
        "short label `127` must render inside cd-exit-bucket-label"
    );
    assert!(
        body.contains(&format!(">{bucket_label_128_143}</div>")),
        "short label `128-143` must render inside cd-exit-bucket-label"
    );
    assert!(
        body.contains(&format!(">{bucket_label_144_254}</div>")),
        "short label `144-254` must render inside cd-exit-bucket-label"
    );
    assert!(
        body.contains(&format!(">{bucket_label_255}</div>")),
        "short label `255` must render inside cd-exit-bucket-label"
    );
    assert!(
        body.contains(&format!(">{bucket_label_none}</div>")),
        "short label `none` must render inside cd-exit-bucket-label"
    );
    assert!(
        body.contains(&format!(">{bucket_label_stopped}</div>")),
        "short label `stopped` must render inside cd-exit-bucket-label"
    );
}
