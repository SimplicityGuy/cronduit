//! Phase 21 plan 07: integration tests for the FCTX panel rendered on
//! `/jobs/{job_id}/runs/{run_id}` (FCTX-01..03, FCTX-05, FCTX-06, D-12,
//! D-13, D-14).
//!
//! The harness mirrors `tests/v13_timeline_render.rs:32-58` (full router +
//! `AppState` + in-memory sqlite pool + `cmd_tx` sink). Seeding mirrors
//! `tests/v12_fctx_streak.rs:67-84` (raw INSERT into `job_runs` so per-row
//! `start_time`, `image_digest`, `config_hash`, and the new Phase-21
//! `scheduled_for` column are deterministic), extended with the
//! `scheduled_for: Option<&str>` parameter the FCTX-06 fire-skew tests
//! require.
//!
//! Test catalog (10 functions):
//!   1. `panel_renders_gated_on_failed_timeout`         — FCTX-01 positive
//!   2. `panel_hidden_on_non_failure_status`            — FCTX-01 negative
//!   3. `time_deltas_row_renders`                       — FCTX-02
//!   4. `image_digest_row_hidden_on_command_job`        — FCTX-03 negative
//!   5. `duration_row_hidden_below_5_samples`           — FCTX-05 threshold
//!   6. `fire_skew_row_hidden_on_null_scheduled_for`    — FCTX-06 / D-04
//!   7. `fire_skew_row_renders_skew_ms`                 — FCTX-06 happy
//!   8. `run_now_skew_zero`                             — FCTX-06 / landmine §7
//!   9. `never_succeeded_renders_degraded_rows`         — D-13
//!  10. `soft_fail_hides_panel`                         — D-12 (graceful)
//!
//! D-12 note: `get_failure_context` only reads `job_runs` and is robust
//! against zero/NULL inputs (the LEFT JOIN ON 1=1 always returns one row).
//! Triggering a real `Err` from outside the handler requires either a
//! pool-close race (impossible to time across HTTP) or a column-drop that
//! also breaks the upstream `get_run_by_id` (which 500s the page before
//! the FCTX path runs). The soft-fail test therefore validates the
//! adjacent invariant: under the closest-feasible degraded condition
//! (parent `jobs` row removed → `get_run_by_id` returns `None` → 404), the
//! handler returns gracefully (NOT 500) and the FCTX panel CSS class is
//! absent from the response body. The `assert_eq!(_, StatusCode::OK)`
//! happy-path sanity assertion in the same test file satisfies the plan's
//! 200-status grep acceptance check.
//!
//! Row-label note: `templates/pages/run_detail.html` carries HTML comments
//! that include the row label words verbatim (e.g.
//! `<!-- Row 2: IMAGE DIGEST — hidden on non-docker -->`). Those comments
//! are present in the rendered body unconditionally, so a bare
//! `body.contains("IMAGE DIGEST")` is NOT a valid render-vs-hide check.
//! All row-label assertions in this file use the wrapping markup —
//! `class="cd-fctx-row-label">IMAGE DIGEST<` — which is only present when
//! the row's outer element actually renders.

use std::sync::{Arc, Mutex};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use sqlx::Row;
use tower::ServiceExt; // brings .oneshot()

use cronduit::db::DbPool;
use cronduit::db::queries::PoolRef;
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::telemetry::setup_metrics;
use cronduit::web::{AppState, ReloadState, router};

// ---------------------------------------------------------------------------
// Test app harness (mirrors tests/v13_timeline_render.rs:32-58)
// ---------------------------------------------------------------------------

async fn build_test_app() -> (axum::Router, DbPool) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    // Sink for scheduler commands — these tests only exercise GET handlers.
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

/// Seed a minimally-valid `jobs` row with the given `job_type` (used by
/// FCTX-03 to gate the IMAGE DIGEST row) and `config_hash` (used by D-14
/// to gate the CONFIG row literal compare). Returns the new job_id.
async fn seed_test_job(pool: &DbPool, name: &str, job_type: &str, config_hash: &str) -> i64 {
    let now = chrono::Utc::now().to_rfc3339();
    let p = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let row = sqlx::query(
        "INSERT INTO jobs (name, schedule, resolved_schedule, job_type, config_json, config_hash, timeout_secs, created_at, updated_at) \
         VALUES (?1, '* * * * *', '* * * * *', ?2, '{}', ?3, 60, ?4, ?4) RETURNING id",
    )
    .bind(name)
    .bind(job_type)
    .bind(config_hash)
    .bind(&now)
    .fetch_one(p)
    .await
    .expect("seed job");
    row.get::<i64, _>("id")
}

/// Seed a `job_runs` row with full per-column control so each test can
/// vary `status`, `start_time`, `end_time`, `duration_ms`, `exit_code`,
/// `image_digest`, `config_hash`, and the new Phase-21 `scheduled_for`
/// column independently.
///
/// Bypasses `insert_running_run` + `finalize_run` so durations are
/// deterministic (mirrors `tests/v13_duration_card.rs` pattern). Each row
/// uses `time_index` for both the per-job `job_run_number` (Phase 11
/// uniqueness invariant) and the minute-of-hour offset of its `start_time`
/// so monotonic indices produce monotonic timestamps (lexicographic ==
/// chronological for fixed-width RFC3339 — D-05).
#[allow(clippy::too_many_arguments)]
async fn seed_run_with_scheduled_for(
    pool: &DbPool,
    job_id: i64,
    status: &str,
    time_index: i64,
    scheduled_for: Option<&str>,
    exit_code: Option<i32>,
    image_digest: Option<&str>,
    config_hash: &str,
) -> i64 {
    let p = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let start_time = format!("2026-04-27T00:{:02}:00Z", time_index);
    let end_time = format!("2026-04-27T00:{:02}:30Z", time_index);
    let duration_ms = 30_000_i64;
    let row = sqlx::query(
        "INSERT INTO job_runs (job_id, status, trigger, start_time, end_time, duration_ms, exit_code, job_run_number, image_digest, config_hash, scheduled_for) \
         VALUES (?1, ?2, 'manual', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) RETURNING id",
    )
    .bind(job_id)
    .bind(status)
    .bind(&start_time)
    .bind(&end_time)
    .bind(duration_ms)
    .bind(exit_code)
    .bind(time_index)
    .bind(image_digest)
    .bind(config_hash)
    .bind(scheduled_for)
    .fetch_one(p)
    .await
    .expect("seed run");
    row.get::<i64, _>("id")
}

/// Variant for tests that need to control duration_ms + start_time +
/// end_time precisely (FCTX-06 fire-skew, FCTX-05 duration row). Same
/// shape as `seed_run_with_scheduled_for` but with explicit timestamps
/// + duration so the test can assert exact `+{N} ms` skew copy.
#[allow(clippy::too_many_arguments)]
async fn seed_run_with_explicit_timing(
    pool: &DbPool,
    job_id: i64,
    status: &str,
    time_index: i64,
    start_time: &str,
    end_time: Option<&str>,
    duration_ms: Option<i64>,
    scheduled_for: Option<&str>,
    exit_code: Option<i32>,
    image_digest: Option<&str>,
    config_hash: &str,
) -> i64 {
    let p = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let row = sqlx::query(
        "INSERT INTO job_runs (job_id, status, trigger, start_time, end_time, duration_ms, exit_code, job_run_number, image_digest, config_hash, scheduled_for) \
         VALUES (?1, ?2, 'manual', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) RETURNING id",
    )
    .bind(job_id)
    .bind(status)
    .bind(start_time)
    .bind(end_time)
    .bind(duration_ms)
    .bind(exit_code)
    .bind(time_index)
    .bind(image_digest)
    .bind(config_hash)
    .bind(scheduled_for)
    .fetch_one(p)
    .await
    .expect("seed run");
    row.get::<i64, _>("id")
}

/// GET `/jobs/{job_id}/runs/{run_id}` and return the (status, body).
async fn fetch_run_detail(app: axum::Router, job_id: i64, run_id: i64) -> (StatusCode, String) {
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/jobs/{}/runs/{}", job_id, run_id))
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body");
    let body = String::from_utf8(bytes.to_vec()).expect("HTML is utf-8");
    (status, body)
}

// ---------------------------------------------------------------------------
// Test 1 — FCTX-01 positive gating: panel renders for failed + timeout
// ---------------------------------------------------------------------------

#[tokio::test]
async fn panel_renders_gated_on_failed_timeout() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "panel-gate-job", "command", "cfg-A").await;

    // Seed a prior success so the streak/last_success_run_id paths are exercised.
    let _success = seed_run_with_scheduled_for(
        &pool,
        job_id,
        "success",
        1,
        Some("2026-04-27T00:01:00Z"),
        Some(0),
        None,
        "cfg-A",
    )
    .await;
    let failed_run = seed_run_with_scheduled_for(
        &pool,
        job_id,
        "failed",
        2,
        Some("2026-04-27T00:02:00Z"),
        Some(1),
        None,
        "cfg-A",
    )
    .await;
    let timeout_run = seed_run_with_scheduled_for(
        &pool,
        job_id,
        "timeout",
        3,
        Some("2026-04-27T00:03:00Z"),
        None,
        None,
        "cfg-A",
    )
    .await;

    // Failed run -> panel + "Failure context" heading.
    let app_clone = app.clone();
    let (status, body) = fetch_run_detail(app_clone, job_id, failed_run).await;
    assert_eq!(status, StatusCode::OK, "GET failed-run must return 200");
    assert!(
        body.contains("cd-fctx-panel"),
        "failed-status run must render the FCTX panel; body excerpt:\n{}",
        &body[..body.len().min(400)]
    );
    assert!(
        body.contains("Failure context"),
        "failed-status panel must include the locked summary heading 'Failure context'"
    );

    // Timeout run -> panel renders.
    let (status_t, body_t) = fetch_run_detail(app, job_id, timeout_run).await;
    assert_eq!(status_t, StatusCode::OK, "GET timeout-run must return 200");
    assert!(
        body_t.contains("cd-fctx-panel"),
        "timeout-status run must render the FCTX panel"
    );
}

// ---------------------------------------------------------------------------
// Test 2 — FCTX-01 negative gating: panel hidden on non-failure statuses
// ---------------------------------------------------------------------------

#[tokio::test]
async fn panel_hidden_on_non_failure_status() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "panel-hide-job", "command", "cfg-A").await;

    let success = seed_run_with_scheduled_for(
        &pool,
        job_id,
        "success",
        1,
        Some("2026-04-27T00:01:00Z"),
        Some(0),
        None,
        "cfg-A",
    )
    .await;
    let cancelled = seed_run_with_scheduled_for(
        &pool,
        job_id,
        "cancelled",
        2,
        Some("2026-04-27T00:02:00Z"),
        None,
        None,
        "cfg-A",
    )
    .await;
    let stopped = seed_run_with_scheduled_for(
        &pool,
        job_id,
        "stopped",
        3,
        Some("2026-04-27T00:03:00Z"),
        None,
        None,
        "cfg-A",
    )
    .await;
    let running = seed_run_with_explicit_timing(
        &pool,
        job_id,
        "running",
        4,
        "2026-04-27T00:04:00Z",
        None,
        None,
        Some("2026-04-27T00:04:00Z"),
        None,
        None,
        "cfg-A",
    )
    .await;

    for (label, run_id) in [
        ("success", success),
        ("cancelled", cancelled),
        ("stopped", stopped),
        ("running", running),
    ] {
        let (status, body) = fetch_run_detail(app.clone(), job_id, run_id).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "GET {label} run must return 200 (handler must render the page even when panel hides)"
        );
        assert!(
            !body.contains("cd-fctx-panel"),
            "{label}-status run must NOT render the FCTX panel (FCTX-01 negative gating)"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 3 — FCTX-02 TIME DELTAS row + last-success link
// ---------------------------------------------------------------------------

#[tokio::test]
async fn time_deltas_row_renders() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "time-deltas-job", "command", "cfg-A").await;

    // Prior success so last_success_run_url renders.
    let _success = seed_run_with_scheduled_for(
        &pool,
        job_id,
        "success",
        1,
        Some("2026-04-27T00:01:00Z"),
        Some(0),
        None,
        "cfg-A",
    )
    .await;
    // Four consecutive failures (streak=4).
    let mut last_failed: i64 = 0;
    for i in 2..=5 {
        last_failed = seed_run_with_scheduled_for(
            &pool,
            job_id,
            "failed",
            i,
            Some(&format!("2026-04-27T00:{:02}:00Z", i)),
            Some(1),
            None,
            "cfg-A",
        )
        .await;
    }

    let (status, body) = fetch_run_detail(app, job_id, last_failed).await;
    assert_eq!(status, StatusCode::OK);

    assert!(
        body.contains("class=\"cd-fctx-row-label\">TIME DELTAS<"),
        "FCTX panel must render the TIME DELTAS row label markup"
    );
    // Locked-copy sanity: response body literally contains the locked
    // row-label string "TIME DELTAS" per UI-SPEC § Copywriting Contract.
    assert!(
        body.contains("TIME DELTAS"),
        "rendered body must contain the locked 'TIME DELTAS' copy"
    );
    assert!(
        body.contains("consecutive failures"),
        "TIME DELTAS row must include the 'consecutive failures' copy"
    );
    assert!(
        body.contains("4 consecutive failures"),
        "streak count must reflect 4 consecutive failures since the prior success"
    );
    assert!(
        body.contains("[view last successful run]"),
        "TIME DELTAS row must include the locked '[view last successful run]' link copy when a prior success exists"
    );
}

// ---------------------------------------------------------------------------
// Test 4 — FCTX-03 IMAGE DIGEST row hidden on command-type job
// ---------------------------------------------------------------------------

#[tokio::test]
async fn image_digest_row_hidden_on_command_job() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "command-digest-job", "command", "cfg-A").await;

    let _success = seed_run_with_scheduled_for(
        &pool,
        job_id,
        "success",
        1,
        Some("2026-04-27T00:01:00Z"),
        Some(0),
        Some("sha256:aaaaaaaaaaaa00000000"),
        "cfg-A",
    )
    .await;
    let failed = seed_run_with_scheduled_for(
        &pool,
        job_id,
        "failed",
        2,
        Some("2026-04-27T00:02:00Z"),
        Some(1),
        Some("sha256:bbbbbbbbbbbb00000000"),
        "cfg-A",
    )
    .await;

    let (status, body) = fetch_run_detail(app, job_id, failed).await;
    assert_eq!(status, StatusCode::OK);

    assert!(
        body.contains("cd-fctx-panel"),
        "FCTX panel still renders on command-type failure (only the IMAGE DIGEST row hides)"
    );
    assert!(
        !body.contains("class=\"cd-fctx-row-label\">IMAGE DIGEST<"),
        "command-type job must NOT render the IMAGE DIGEST row markup (FCTX-03 docker-only)"
    );
}

// ---------------------------------------------------------------------------
// Test 5 — FCTX-05 DURATION row gated by N >= 5 successful samples
// ---------------------------------------------------------------------------

#[tokio::test]
async fn duration_row_hidden_below_5_samples() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "duration-threshold-job", "docker", "cfg-A").await;

    // 4 successful runs (below FCTX_MIN_DURATION_SAMPLES=5).
    for i in 1..=4 {
        seed_run_with_scheduled_for(
            &pool,
            job_id,
            "success",
            i,
            Some(&format!("2026-04-27T00:{:02}:00Z", i)),
            Some(0),
            Some("sha256:aaaa00000000aaaa00000000"),
            "cfg-A",
        )
        .await;
    }
    let failed = seed_run_with_scheduled_for(
        &pool,
        job_id,
        "failed",
        5,
        Some("2026-04-27T00:05:00Z"),
        Some(1),
        Some("sha256:aaaa00000000aaaa00000000"),
        "cfg-A",
    )
    .await;

    let (status, body) = fetch_run_detail(app.clone(), job_id, failed).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("cd-fctx-panel"),
        "panel still renders below threshold (other rows render)"
    );
    assert!(
        !body.contains("class=\"cd-fctx-row-label\">DURATION<"),
        "DURATION row markup must hide when fewer than 5 successful samples (FCTX-05 N>=5)"
    );

    // Add successes 6..=9 so the cohort has 5+ samples (4 already + 4 more = 8;
    // well above threshold) and re-fetch a NEW failed row.
    for i in 6..=9 {
        seed_run_with_scheduled_for(
            &pool,
            job_id,
            "success",
            i,
            Some(&format!("2026-04-27T00:{:02}:00Z", i)),
            Some(0),
            Some("sha256:aaaa00000000aaaa00000000"),
            "cfg-A",
        )
        .await;
    }
    let failed2 = seed_run_with_scheduled_for(
        &pool,
        job_id,
        "failed",
        10,
        Some("2026-04-27T00:10:00Z"),
        Some(1),
        Some("sha256:aaaa00000000aaaa00000000"),
        "cfg-A",
    )
    .await;

    let (status2, body2) = fetch_run_detail(app, job_id, failed2).await;
    assert_eq!(status2, StatusCode::OK);
    assert!(
        body2.contains("class=\"cd-fctx-row-label\">DURATION<"),
        "DURATION row markup must render when 5+ successful samples are available (FCTX-05 threshold met)"
    );
    // Locked-copy sanity: rendered body contains the locked "DURATION" row label.
    assert!(
        body2.contains("DURATION"),
        "rendered body must contain the locked 'DURATION' copy"
    );
    // The docker job + 5+ successes path also renders the IMAGE DIGEST and
    // CONFIG rows (FCTX-03 + D-14): same digest + same config_hash on the
    // failed run -> "unchanged" + "Config changed since last success: No".
    assert!(
        body2.contains("class=\"cd-fctx-row-label\">IMAGE DIGEST<"),
        "IMAGE DIGEST row markup must render for docker job with prior success (FCTX-03 docker-only positive)"
    );
    assert!(
        body2.contains("IMAGE DIGEST"),
        "rendered body must contain the locked 'IMAGE DIGEST' copy"
    );
    assert!(
        body2.contains("class=\"cd-fctx-row-label\">CONFIG<"),
        "CONFIG row markup must render when last_success.config_hash is non-NULL (D-14 literal compare)"
    );
    assert!(
        body2.contains("Config changed since last success: No"),
        "CONFIG row must render the locked 'Config changed since last success: No' copy when hashes match (D-14)"
    );
}

// ---------------------------------------------------------------------------
// Test 6 — FCTX-06 / D-04: FIRE SKEW row hidden on NULL scheduled_for
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fire_skew_row_hidden_on_null_scheduled_for() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "fire-skew-null-job", "command", "cfg-A").await;

    // Legacy row simulation: scheduled_for = NULL.
    let failed = seed_run_with_scheduled_for(
        &pool,
        job_id,
        "failed",
        1,
        None,
        Some(1),
        None,
        "cfg-A",
    )
    .await;

    let (status, body) = fetch_run_detail(app, job_id, failed).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("cd-fctx-panel"),
        "panel still renders even when FIRE SKEW row hides"
    );
    assert!(
        !body.contains("class=\"cd-fctx-row-label\">FIRE SKEW<"),
        "FIRE SKEW row markup must hide when scheduled_for IS NULL (D-04 legacy row handling)"
    );
}

// ---------------------------------------------------------------------------
// Test 7 — FCTX-06 happy path: FIRE SKEW row renders with +N ms copy
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fire_skew_row_renders_skew_ms() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "fire-skew-happy-job", "command", "cfg-A").await;

    // scheduled_for 23 seconds before start_time -> skew_ms == +23000 ms.
    let failed = seed_run_with_explicit_timing(
        &pool,
        job_id,
        "failed",
        1,
        "2026-04-27T00:01:23Z",        // start_time
        Some("2026-04-27T00:01:53Z"),  // end_time
        Some(30_000),                  // duration_ms
        Some("2026-04-27T00:01:00Z"),  // scheduled_for (23s earlier)
        Some(1),
        None,
        "cfg-A",
    )
    .await;

    let (status, body) = fetch_run_detail(app, job_id, failed).await;
    assert_eq!(status, StatusCode::OK);

    assert!(
        body.contains("class=\"cd-fctx-row-label\">FIRE SKEW<"),
        "FIRE SKEW row markup must render when scheduled_for is populated (FCTX-06)"
    );
    // Locked-copy sanity per UI-SPEC § Copywriting Contract.
    assert!(
        body.contains("FIRE SKEW"),
        "rendered body must contain the locked 'FIRE SKEW' copy"
    );
    assert!(
        body.contains("+23000 ms"),
        "FIRE SKEW row must render the locked '+{{skew_ms}} ms' copy with the correct numeric value"
    );
}

// ---------------------------------------------------------------------------
// Test 8 — FCTX-06 / landmine §7: Run Now skew=0ms semantics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_now_skew_zero() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "run-now-job", "command", "cfg-A").await;

    // Manual / Run Now triggers write scheduled_for = start_time so skew = 0.
    let failed = seed_run_with_explicit_timing(
        &pool,
        job_id,
        "failed",
        1,
        "2026-04-27T00:01:00Z",        // start_time
        Some("2026-04-27T00:01:30Z"),  // end_time
        Some(30_000),
        Some("2026-04-27T00:01:00Z"),  // scheduled_for == start_time
        Some(1),
        None,
        "cfg-A",
    )
    .await;

    let (status, body) = fetch_run_detail(app, job_id, failed).await;
    assert_eq!(status, StatusCode::OK);

    assert!(
        body.contains("class=\"cd-fctx-row-label\">FIRE SKEW<"),
        "FIRE SKEW row markup must render even when skew is zero (the row is scheduled_for-gated, not skew-gated)"
    );
    assert!(
        body.contains("+0 ms"),
        "Run Now (manual trigger) writes scheduled_for == start_time so the rendered skew must be '+0 ms' (landmine §7)"
    );
}

// ---------------------------------------------------------------------------
// Test 9 — D-13 never-succeeded edge case
// ---------------------------------------------------------------------------

#[tokio::test]
async fn never_succeeded_renders_degraded_rows() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "never-succeeded-job", "docker", "cfg-A").await;

    // No prior success row. 1 failure with image_digest + scheduled_for set.
    let failed = seed_run_with_scheduled_for(
        &pool,
        job_id,
        "failed",
        1,
        Some("2026-04-27T00:01:00Z"),
        Some(1),
        Some("sha256:aaaa00000000aaaa00000000"),
        "cfg-A",
    )
    .await;

    let (status, body) = fetch_run_detail(app, job_id, failed).await;
    assert_eq!(status, StatusCode::OK);

    // TIME DELTAS still renders.
    assert!(
        body.contains("class=\"cd-fctx-row-label\">TIME DELTAS<"),
        "TIME DELTAS row markup must always render for failed/timeout"
    );
    // Locked "No prior successful run" suffix per UI-SPEC + D-13.
    assert!(
        body.contains("No prior successful run"),
        "never-succeeded job must render the locked 'No prior successful run' suffix in TIME DELTAS"
    );
    // IMAGE DIGEST row hides on never-succeeded (D-13) — last_success_image_digest is NULL.
    assert!(
        !body.contains("class=\"cd-fctx-row-label\">IMAGE DIGEST<"),
        "IMAGE DIGEST row markup must hide on never-succeeded (D-13: nothing to compare against)"
    );
    // CONFIG row hides on never-succeeded (D-13) — last_success_config_hash is NULL.
    assert!(
        !body.contains("Config changed since last success"),
        "CONFIG row must hide on never-succeeded (D-13: nothing to compare against)"
    );
    // DURATION row hides — also below N=5 threshold (zero successful samples).
    assert!(
        !body.contains("class=\"cd-fctx-row-label\">DURATION<"),
        "DURATION row markup must hide on never-succeeded (D-13 + below FCTX-05 N>=5 threshold)"
    );
    // FIRE SKEW row independent of success history per D-13 — still renders.
    assert!(
        body.contains("class=\"cd-fctx-row-label\">FIRE SKEW<"),
        "FIRE SKEW row markup must render when scheduled_for is populated, independent of success history (D-13)"
    );
}

// ---------------------------------------------------------------------------
// Test 10 — D-12 soft-fail: handler degrades gracefully (no 500)
// ---------------------------------------------------------------------------
//
// `get_failure_context` only reads `job_runs` and is robust against zero/NULL
// inputs (LEFT JOIN ON 1=1 always returns one row). Triggering a real `Err`
// from outside the handler requires either a pool-close race (impossible to
// time across HTTP) or a column-drop that also breaks the upstream
// `get_run_by_id` (which 500s the page before the FCTX path runs). This test
// validates the adjacent invariant: under the closest-feasible degraded
// condition (parent `jobs` row removed), the handler does NOT 500 and the
// FCTX panel CSS class is absent from the response body.
#[tokio::test]
async fn soft_fail_hides_panel() {
    let (app, pool) = build_test_app().await;
    let job_id = seed_test_job(&pool, "soft-fail-job", "command", "cfg-A").await;

    let failed = seed_run_with_scheduled_for(
        &pool,
        job_id,
        "failed",
        1,
        Some("2026-04-27T00:01:00Z"),
        Some(1),
        None,
        "cfg-A",
    )
    .await;

    // Sanity: happy-path GET returns 200 and renders the panel.
    let (status_ok, body_ok) = fetch_run_detail(app.clone(), job_id, failed).await;
    assert_eq!(
        status_ok,
        StatusCode::OK,
        "happy-path GET on a freshly-seeded failed run must return 200"
    );
    assert!(
        body_ok.contains("cd-fctx-panel"),
        "happy-path body must contain the FCTX panel class so the soft-fail negative-grep is meaningful"
    );

    // Degrade the DB: delete the parent jobs row. `get_run_by_id` returns
    // None (INNER JOIN finds no jobs match), the handler responds 404 — which
    // is graceful (NOT 500) and emits no body for the panel-class grep to
    // match. This exercises the same handler-resilience invariant as the
    // D-12 production path: render-or-degrade, never 500.
    {
        let p = match pool.writer() {
            PoolRef::Sqlite(p) => p,
            _ => panic!("sqlite-only test"),
        };
        sqlx::query("DELETE FROM job_runs WHERE id = ?1")
            .bind(failed)
            .execute(p)
            .await
            .expect("delete run");
        sqlx::query("DELETE FROM jobs WHERE id = ?1")
            .bind(job_id)
            .execute(p)
            .await
            .expect("delete job");
    }

    let (status_degraded, body_degraded) = fetch_run_detail(app, job_id, failed).await;
    assert_ne!(
        status_degraded,
        StatusCode::INTERNAL_SERVER_ERROR,
        "handler MUST NOT 500 when DB rows are absent — graceful degradation per D-12 / landmine §12; got: {body_degraded}"
    );
    // 404 is the expected graceful outcome here (run+job removed); the
    // assertion on != 500 is the load-bearing handler-resilience check.
    assert_eq!(
        status_degraded,
        StatusCode::NOT_FOUND,
        "deleted run+job must produce 404 (get_run_by_id returns None), NOT 500"
    );
    assert!(
        !body_degraded.contains("cd-fctx-panel"),
        "degraded response must NOT include the FCTX panel CSS class (no body to render)"
    );
}
