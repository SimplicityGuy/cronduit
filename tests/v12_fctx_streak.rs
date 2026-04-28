//! Phase 16 / FCTX-07: assert get_failure_context returns the correct
//! consecutive_failures count and last-success metadata across the
//! 5 streak scenarios from CONTEXT.md D-07.
//!
//! Plus FCTX-04 write-site assertions:
//! - T-V12-FCTX-03 (write_site_captures_config_hash): insert_running_run
//!   captures config_hash at fire time.
//! - T-V12-FCTX-04 (reload_changes_config_hash): reload-mid-fire produces
//!   distinct config_hash values across consecutive runs.
//!
//! Test catalog (7 functions):
//!   1. no_successes_returns_none           — T-V12-FCTX-13 / Plan 16-05 D-07 scenario (a)
//!   2. recent_success_returns_zero_streak  — D-07 scenario (b)
//!   3. one_consecutive_failure             — D-07 scenario (c)
//!   4. n_consecutive_failures              — D-07 scenario (d)
//!   5. streak_resets_on_intervening_success — D-07 scenario (e)
//!   6. write_site_captures_config_hash     — T-V12-FCTX-03 (FCTX-04 write site)
//!   7. reload_changes_config_hash          — T-V12-FCTX-04 (FCTX-04 reload-mid-fire)

mod common;

use cronduit::db::DbPool;
use cronduit::db::queries::{self, PoolRef};
use sqlx::Row;

/// In-memory SQLite + all migrations applied (mirrors v11_fixtures helper).
async fn setup_pool() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    pool.migrate().await.expect("apply migrations");
    pool
}

/// Seed a minimally-valid `jobs` row with an explicit `config_hash` and
/// return its id. Distinct from `common::v11_fixtures::seed_test_job`
/// which hard-codes `config_hash = '0'`; tests T-V12-FCTX-03/-04 assert
/// FCTX-04 write-site behavior and need control over the value.
async fn seed_job(pool: &DbPool, name: &str, config_hash: &str) -> i64 {
    let now = chrono::Utc::now().to_rfc3339();
    let p = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let row = sqlx::query(
        "INSERT INTO jobs (name, schedule, resolved_schedule, job_type, config_json, config_hash, timeout_secs, created_at, updated_at) \
         VALUES (?1, '* * * * *', '* * * * *', 'command', '{}', ?2, 60, ?3, ?3) RETURNING id",
    )
    .bind(name)
    .bind(config_hash)
    .bind(&now)
    .fetch_one(p)
    .await
    .expect("seed job");
    row.get::<i64, _>("id")
}

/// Seed a `job_runs` row with the given status at a deterministic
/// lexicographic time. `time_index` is converted to an RFC3339 string of
/// the form `2026-04-27T00:MM:00Z` so monotonically increasing indices
/// produce monotonically increasing start_time values (lexicographic ==
/// chronological for fixed-width RFC3339 — D-05's portability guarantee).
///
/// Each row uses a distinct `job_run_number` (== `time_index`) to satisfy
/// the Phase 11 NOT NULL constraint without sequencing through
/// `insert_running_run`'s counter.
async fn seed_run(pool: &DbPool, job_id: i64, status: &str, time_index: i64) {
    let p = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let start_time = format!("2026-04-27T00:{:02}:00Z", time_index);
    sqlx::query(
        "INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number, image_digest, config_hash) \
         VALUES (?1, ?2, 'manual', ?3, ?4, NULL, 'seed-hash')",
    )
    .bind(job_id)
    .bind(status)
    .bind(&start_time)
    .bind(time_index)
    .execute(p)
    .await
    .expect("seed run");
}

// -- Streak scenarios (D-07) --------------------------------------------

/// D-07 scenario (a) / T-V12-FCTX-13: job has never succeeded — 3 failures
/// + 0 successes -> consecutive_failures == 3, last_success_* == None.
#[tokio::test]
async fn no_successes_returns_none() {
    let pool = setup_pool().await;
    let job_id = seed_job(&pool, "no-success-job", "abc").await;
    seed_run(&pool, job_id, "failed", 1).await;
    seed_run(&pool, job_id, "failed", 2).await;
    seed_run(&pool, job_id, "timeout", 3).await;

    let ctx = queries::get_failure_context(&pool, job_id).await.unwrap();
    assert_eq!(
        ctx.consecutive_failures, 3,
        "all 3 failure-status rows count when no success exists"
    );
    assert_eq!(ctx.last_success_run_id, None);
    assert_eq!(ctx.last_success_image_digest, None);
    assert_eq!(ctx.last_success_config_hash, None);
}

/// D-07 scenario (b): most recent run is a success -> consecutive_failures
/// == 0, last_success_* populated from that row.
#[tokio::test]
async fn recent_success_returns_zero_streak() {
    let pool = setup_pool().await;
    let job_id = seed_job(&pool, "recent-success", "abc").await;
    seed_run(&pool, job_id, "failed", 1).await;
    seed_run(&pool, job_id, "success", 2).await;

    let ctx = queries::get_failure_context(&pool, job_id).await.unwrap();
    assert_eq!(
        ctx.consecutive_failures, 0,
        "no failures since most recent success"
    );
    assert!(
        ctx.last_success_run_id.is_some(),
        "success row should populate last_success_run_id"
    );
    // image_digest seeded as NULL (command-style row); config_hash seeded as 'seed-hash'.
    assert_eq!(ctx.last_success_image_digest, None);
    assert_eq!(ctx.last_success_config_hash, Some("seed-hash".to_string()));
}

/// D-07 scenario (c): success then 1 failure -> consecutive_failures == 1.
#[tokio::test]
async fn one_consecutive_failure() {
    let pool = setup_pool().await;
    let job_id = seed_job(&pool, "one-fail", "abc").await;
    seed_run(&pool, job_id, "success", 1).await;
    seed_run(&pool, job_id, "failed", 2).await;

    let ctx = queries::get_failure_context(&pool, job_id).await.unwrap();
    assert_eq!(ctx.consecutive_failures, 1);
    assert!(ctx.last_success_run_id.is_some());
}

/// D-07 scenario (d): success then 5 failures -> consecutive_failures == 5.
#[tokio::test]
async fn n_consecutive_failures() {
    let pool = setup_pool().await;
    let job_id = seed_job(&pool, "five-fails", "abc").await;
    seed_run(&pool, job_id, "success", 1).await;
    for i in 2..=6 {
        seed_run(&pool, job_id, "failed", i).await;
    }

    let ctx = queries::get_failure_context(&pool, job_id).await.unwrap();
    assert_eq!(
        ctx.consecutive_failures, 5,
        "5 failures since success at time_index=1"
    );
    assert!(ctx.last_success_run_id.is_some());
}

/// D-07 scenario (e): success -> fail -> success -> fail. Streak resets
/// to 1 on the second success and counts only the post-second-success
/// failure.
#[tokio::test]
async fn streak_resets_on_intervening_success() {
    let pool = setup_pool().await;
    let job_id = seed_job(&pool, "reset-streak", "abc").await;
    seed_run(&pool, job_id, "success", 1).await;
    seed_run(&pool, job_id, "failed", 2).await;
    seed_run(&pool, job_id, "success", 3).await;
    seed_run(&pool, job_id, "failed", 4).await;

    let ctx = queries::get_failure_context(&pool, job_id).await.unwrap();
    assert_eq!(
        ctx.consecutive_failures, 1,
        "streak should reset to 1 after the second success"
    );
}

// -- FCTX-04 write-site assertions --------------------------------------

/// T-V12-FCTX-03: insert_running_run captures `config_hash` at fire time
/// and writes it to `job_runs.config_hash`. Validates the FCTX-04 write
/// site landed by Plan 16-04a/04b.
#[tokio::test]
async fn write_site_captures_config_hash() {
    let pool = setup_pool().await;
    let job_id = seed_job(&pool, "config-write", "test-config-A").await;
    let run_id = queries::insert_running_run(&pool, job_id, "manual", "test-config-A")
        .await
        .unwrap();

    let p = match pool.reader() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let row = sqlx::query("SELECT config_hash FROM job_runs WHERE id = ?1")
        .bind(run_id)
        .fetch_one(p)
        .await
        .unwrap();
    let stored: Option<String> = row.get("config_hash");
    assert_eq!(
        stored,
        Some("test-config-A".to_string()),
        "insert_running_run must write the config_hash arg into job_runs.config_hash"
    );
}

/// T-V12-FCTX-04: simulate reload-mid-fire by passing distinct config_hash
/// values to two consecutive insert_running_run calls and assert each row
/// reflects the value passed at fire time (not the latest).
#[tokio::test]
async fn reload_changes_config_hash() {
    let pool = setup_pool().await;
    let job_id = seed_job(&pool, "config-reload", "v1").await;
    let r1 = queries::insert_running_run(&pool, job_id, "manual", "v1")
        .await
        .unwrap();
    let r2 = queries::insert_running_run(&pool, job_id, "manual", "v2")
        .await
        .unwrap();

    let p = match pool.reader() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let h1: Option<String> = sqlx::query("SELECT config_hash FROM job_runs WHERE id = ?1")
        .bind(r1)
        .fetch_one(p)
        .await
        .unwrap()
        .get("config_hash");
    let h2: Option<String> = sqlx::query("SELECT config_hash FROM job_runs WHERE id = ?1")
        .bind(r2)
        .fetch_one(p)
        .await
        .unwrap()
        .get("config_hash");
    assert_eq!(h1, Some("v1".to_string()));
    assert_eq!(h2, Some("v2".to_string()));
    assert_ne!(h1, h2, "reload-mid-fire must produce distinct hashes");
}
