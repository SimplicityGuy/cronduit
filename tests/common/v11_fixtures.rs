//! Phase 11 shared test fixtures.
//!
//! Used by every tests/v11_*.rs harness. Provides:
//! - setup_sqlite_with_phase11_migrations: in-memory SQLite + all migrations applied.
//! - seed_test_job / seed_running_run: fast DB seeders.
//! - make_test_batch: synthetic log-line batches sized for DEFAULT_BATCH_SIZE testing.
//! - seed_null_runs: for migration_02_* tests that backfill pre-existing NULL rows.
//!
//! Schema note: `seed_test_job` binds only the NOT-NULL columns in the initial
//! migration at `migrations/sqlite/20260410_000000_initial.up.sql`
//! (name, schedule, resolved_schedule, job_type, config_json, config_hash,
//! timeout_secs, created_at, updated_at). `enabled` has DEFAULT 1 and is
//! omitted.

#![allow(dead_code)]

use cronduit::db::DbPool;
use cronduit::db::queries::{self, PoolRef};
use sqlx::Row;

/// Connect to an in-memory SQLite DB and apply all migrations. Returns a ready-to-use pool.
pub async fn setup_sqlite_with_phase11_migrations() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    pool.migrate().await.expect("apply migrations");
    pool
}

/// Seed a minimally-valid `jobs` row and return its id.
///
/// Matches the columns in the initial migration exactly. Downstream plans that
/// add columns (e.g. `next_run_number` via Plan 11-02) will pick up DEFAULTs so
/// this fixture keeps compiling without edits.
pub async fn seed_test_job(pool: &DbPool, name: &str) -> i64 {
    let now = chrono::Utc::now().to_rfc3339();
    let writer = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        PoolRef::Postgres(_) => panic!("fixture is sqlite-only"),
    };
    let row = sqlx::query(
        "INSERT INTO jobs (name, schedule, resolved_schedule, job_type, config_json, config_hash, timeout_secs, created_at, updated_at) \
         VALUES (?1, '* * * * *', '* * * * *', 'command', '{}', '0', 60, ?2, ?2) \
         RETURNING id",
    )
    .bind(name)
    .bind(&now)
    .fetch_one(writer)
    .await
    .expect("seed job");
    row.get::<i64, _>("id")
}

/// Insert a running `job_runs` row for `job_id` and return its id. Uses the
/// canonical `queries::insert_running_run` so the fixture stays aligned with
/// production code paths (including Plan 11-05's two-statement counter
/// transaction once it lands).
pub async fn seed_running_run(pool: &DbPool, job_id: i64) -> i64 {
    queries::insert_running_run(pool, job_id, "manual")
        .await
        .expect("insert running run")
}

/// Construct `n` synthetic `(stream, ts, line)` triples suitable for feeding
/// `insert_log_batch`. Alternates stdout/stderr; lines are ~80 chars each.
pub fn make_test_batch(n: usize) -> Vec<(String, String, String)> {
    let ts = chrono::Utc::now().to_rfc3339();
    (0..n)
        .map(|i| {
            let stream = if i % 2 == 0 { "stdout" } else { "stderr" };
            let line = format!(
                "2026-04-16T12:00:00Z INFO test-line #{:05} payload with ~80 chars of body xxxxxxxx",
                i
            );
            (stream.to_string(), ts.clone(), line)
        })
        .collect()
}

/// Seed N `job_runs` rows for `job_id`, returning ids in insert order.
///
/// Wave-0 version: inserts rows as status='success' with the initial-migration
/// columns only. Once Plan 11-02 adds `job_run_number` as nullable, downstream
/// plans (11-03 backfill tests) re-seed the column via targeted updates rather
/// than bind-on-insert, keeping this fixture stable across schema evolution.
pub async fn seed_null_runs(pool: &DbPool, job_id: i64, n: usize) -> Vec<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    let writer = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        PoolRef::Postgres(_) => panic!("sqlite-only fixture"),
    };
    let mut ids = Vec::with_capacity(n);
    for _ in 0..n {
        let row = sqlx::query(
            "INSERT INTO job_runs (job_id, status, trigger, start_time) \
             VALUES (?1, 'success', 'schedule', ?2) RETURNING id",
        )
        .bind(job_id)
        .bind(&now)
        .fetch_one(writer)
        .await
        .expect("seed null run");
        ids.push(row.get::<i64, _>("id"));
    }
    ids
}
