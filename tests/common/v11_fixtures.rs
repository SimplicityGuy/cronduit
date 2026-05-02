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

/// Connect to an in-memory SQLite DB and apply only the initial schema + file 1
/// (nullable `job_run_number`) + file 2 (backfill marker). File 3 (NOT NULL
/// tightening, landed by Plan 11-04) is **not** applied, so `job_run_number`
/// remains nullable and the backfill orchestrator can exercise its seeding +
/// chunk loop on NULL-filled rows.
///
/// This fixture is the "pre-file-3" state used by `migration_01_*` (which
/// asserts the nullable column shape) and `migration_02_*` (which seeds rows
/// with NULL job_run_number, then invokes the backfill orchestrator).
/// `migration_03_*` uses the full `setup_sqlite_with_phase11_migrations`
/// because its assertions depend on file 3 having run.
///
/// Implementation: applies each migration file's SQL directly against the
/// writer pool. Does **not** populate `_sqlx_migrations` — the sqlx bookkeeping
/// is irrelevant for these tests because they never call `DbPool::migrate`
/// afterwards. (Tests that do call `DbPool::migrate` post-setup — like
/// `migration_01_idempotent` — continue to use `setup_sqlite_with_phase11_migrations`
/// and accept the full all-three-files state.)
pub async fn setup_sqlite_before_file3_migrations() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    let writer = match pool.writer() {
        PoolRef::Sqlite(p) => p.clone(),
        _ => panic!("sqlite-only fixture"),
    };
    // Apply files 0, 1, 2 SQL directly. File names tracked manually here so
    // the fixture is explicit about which migrations it stops before.
    let files = [
        include_str!("../../migrations/sqlite/20260410_000000_initial.up.sql"),
        include_str!("../../migrations/sqlite/20260416_000001_job_run_number_add.up.sql"),
        include_str!("../../migrations/sqlite/20260417_000002_job_run_number_backfill.up.sql"),
    ];
    for sql in files {
        sqlx::query(sql)
            .execute(&writer)
            .await
            .expect("apply migration sql");
    }
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
    queries::insert_running_run(pool, job_id, "manual", "testhash", None)
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
