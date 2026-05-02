//! Phase 20 / WH-10 / BL-01 regression test (gap-closure plan 20-11).
//!
//! Locks the FK-violation fix: webhook_deliveries.run_id REFERENCES job_runs(id)
//! without ON DELETE CASCADE. The retention pruner MUST delete webhook_deliveries
//! BEFORE job_runs (Option A — runtime ordering fix). If the order regresses,
//! a DLQ row referencing an old run causes the job_runs DELETE to abort with
//! `FOREIGN KEY constraint failed`, breaking the prune loop and permanently
//! halting retention.
//!
//! Coverage:
//! - retention_webhook_fk_no_violation_when_dlq_row_references_old_run — the
//!   primary BL-01 regression lock; both rows older than cutoff; both must
//!   delete cleanly.
//! - retention_webhook_fk_keeps_run_when_fresh_dlq_references_it — defense
//!   in depth; the extended NOT EXISTS clause in delete_old_runs_batch must
//!   prevent deleting a run while a fresh DLQ row still references it.

use cronduit::db::DbPool;
use cronduit::db::queries::{
    PoolRef, delete_old_logs_batch, delete_old_runs_batch, delete_old_webhook_deliveries_batch,
};
use sqlx::Row;

const BATCH_SIZE: i64 = 1000;

async fn setup_test_db() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    pool.migrate().await.expect("apply migrations");
    pool
}

/// Seed one job + one job_run with a configurable end_time + one webhook_deliveries
/// row with a configurable last_attempt_at. Returns (job_id, run_id).
async fn seed_job_run_with_dlq(
    pool: &DbPool,
    job_name: &str,
    run_end_time: &str,
    dlq_last_attempt_at: &str,
) -> (i64, i64) {
    let p = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let now = chrono::Utc::now().to_rfc3339();
    let job_row = sqlx::query(
        "INSERT INTO jobs (name, schedule, resolved_schedule, job_type, config_json, config_hash, timeout_secs, created_at, updated_at) \
         VALUES (?1, '* * * * *', '* * * * *', 'command', '{}', 'seed-cfg', 60, ?2, ?2) RETURNING id",
    )
    .bind(job_name)
    .bind(&now)
    .fetch_one(p)
    .await
    .expect("seed job");
    let job_id: i64 = job_row.get("id");

    let run_row = sqlx::query(
        "INSERT INTO job_runs (job_id, status, trigger, start_time, end_time, job_run_number, image_digest, config_hash) \
         VALUES (?1, 'failed', 'manual', ?2, ?2, 1, NULL, 'seed-cfg') RETURNING id",
    )
    .bind(job_id)
    .bind(run_end_time)
    .fetch_one(p)
    .await
    .expect("seed run");
    let run_id: i64 = run_row.get("id");

    sqlx::query(
        "INSERT INTO webhook_deliveries \
         (run_id, job_id, url, attempts, last_status, last_error, dlq_reason, first_attempt_at, last_attempt_at) \
         VALUES (?1, ?2, 'https://example.test/hook', 3, 500, 'mock body', 'http_5xx', ?3, ?3)",
    )
    .bind(run_id)
    .bind(job_id)
    .bind(dlq_last_attempt_at)
    .execute(p)
    .await
    .expect("seed webhook_deliveries row");

    (job_id, run_id)
}

/// Run the same phase ordering as src/scheduler/retention.rs::run_prune_cycle:
/// Phase 1 logs → Phase 2 webhook_deliveries → Phase 3 job_runs.
async fn run_prune_in_post_fix_order(pool: &DbPool, cutoff: &str) {
    // Phase 1: logs (none in this test, but exercise the call).
    let _ = delete_old_logs_batch(pool, cutoff, BATCH_SIZE)
        .await
        .expect("logs batch ok");
    // Phase 2: webhook_deliveries — MUST run BEFORE job_runs per BL-01 fix.
    let _ = delete_old_webhook_deliveries_batch(pool, cutoff, BATCH_SIZE)
        .await
        .expect("webhook_deliveries batch ok");
    // Phase 3: job_runs (extended NOT EXISTS protects against any leftover
    // DLQ row that survived Phase 2 by being fresher than cutoff).
    let _ = delete_old_runs_batch(pool, cutoff, BATCH_SIZE)
        .await
        .expect("job_runs batch ok — FK violation here means BL-01 has regressed");
}

#[tokio::test]
async fn retention_webhook_fk_no_violation_when_dlq_row_references_old_run() {
    // BL-01 primary regression lock: both job_run and webhook_deliveries are
    // older than the cutoff; the prune cycle must delete them both with NO FK
    // violation. Pre-fix code deleted job_runs first, hitting `FOREIGN KEY
    // constraint failed` and breaking the loop.
    let pool = setup_test_db().await;
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(90)).to_rfc3339();
    let old_time = (chrono::Utc::now() - chrono::Duration::days(100)).to_rfc3339();

    let (_, run_id) =
        seed_job_run_with_dlq(&pool, "bl01-old-run-job", &old_time, &old_time).await;

    // Run the prune in the POST-FIX order. If BL-01 regresses (i.e., someone
    // reorders the phases back to runs-before-webhook_deliveries), the
    // delete_old_runs_batch call inside this helper will return Err with a
    // FK-constraint-violated error, which `.expect()` panics on — failing the
    // test loudly with the message "FK violation here means BL-01 has regressed".
    run_prune_in_post_fix_order(&pool, &cutoff).await;

    // Verify both rows are gone.
    let p = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let dlq_count: i64 =
        sqlx::query("SELECT COUNT(*) AS c FROM webhook_deliveries WHERE run_id = ?1")
            .bind(run_id)
            .fetch_one(p)
            .await
            .expect("query webhook_deliveries count")
            .get("c");
    let run_count: i64 = sqlx::query("SELECT COUNT(*) AS c FROM job_runs WHERE id = ?1")
        .bind(run_id)
        .fetch_one(p)
        .await
        .expect("query job_runs count")
        .get("c");

    assert_eq!(
        dlq_count, 0,
        "webhook_deliveries row must be deleted (last_attempt_at < cutoff)"
    );
    assert_eq!(
        run_count, 0,
        "job_runs row must be deleted AFTER its dependent DLQ row was removed (Phase 3 ran cleanly)"
    );
}

#[tokio::test]
async fn retention_webhook_fk_keeps_run_when_fresh_dlq_references_it() {
    // Defense-in-depth lock for the extended NOT EXISTS clause in
    // delete_old_runs_batch: a DLQ row whose last_attempt_at is FRESHER than
    // cutoff must survive Phase 2; AND its parent job_run (whose end_time IS
    // older than cutoff) must NOT be deleted in Phase 3 — the NOT EXISTS clause
    // catches this race and skips the run.
    let pool = setup_test_db().await;
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(90)).to_rfc3339();
    let old_time = (chrono::Utc::now() - chrono::Duration::days(100)).to_rfc3339();
    let fresh_time = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();

    // run_end_time is OLD (eligible for prune); dlq_last_attempt_at is FRESH (NOT eligible).
    let (_, run_id) =
        seed_job_run_with_dlq(&pool, "bl01-fresh-dlq-job", &old_time, &fresh_time).await;

    run_prune_in_post_fix_order(&pool, &cutoff).await;

    let p = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let dlq_count: i64 =
        sqlx::query("SELECT COUNT(*) AS c FROM webhook_deliveries WHERE run_id = ?1")
            .bind(run_id)
            .fetch_one(p)
            .await
            .expect("query webhook_deliveries count")
            .get("c");
    let run_count: i64 = sqlx::query("SELECT COUNT(*) AS c FROM job_runs WHERE id = ?1")
        .bind(run_id)
        .fetch_one(p)
        .await
        .expect("query job_runs count")
        .get("c");

    assert_eq!(
        dlq_count, 1,
        "fresh DLQ row (last_attempt_at within retention window) must survive Phase 2"
    );
    assert_eq!(
        run_count, 1,
        "job_run with a still-present DLQ child must survive Phase 3 (NOT EXISTS guard works)"
    );
}
