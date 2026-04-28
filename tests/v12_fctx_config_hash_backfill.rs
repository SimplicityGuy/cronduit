//! Phase 16 / FCTX-04: assert that the bulk backfill migration
//! `20260427_000007_config_hash_backfill.up.sql` populates pre-existing
//! `job_runs` rows with their job's `config_hash`, leaves orphaned rows NULL,
//! and is idempotent across re-runs (T-V12-FCTX-01, T-V12-FCTX-02).
//!
//! Strategy: `setup_sqlite_with_phase11_migrations` applies every migration
//! including `_000007`. To exercise the backfill we seed a `job_runs` row,
//! UPDATE its `config_hash` back to NULL (simulating a pre-v1.2 row that
//! escaped the WHERE-IS-NULL guard), then re-run the same bulk UPDATE the
//! migration runs. This validates the SQL logic without needing a custom
//! "stop-before-007" fixture.

mod common;

use common::v11_fixtures::setup_sqlite_with_phase11_migrations;
use cronduit::db::DbPool;
use cronduit::db::queries::PoolRef;
use sqlx::Row;

/// SQL identical to migrations/sqlite/20260427_000007_config_hash_backfill.up.sql.
/// Re-runnable for orphan + idempotency scenarios.
const BACKFILL_SQL: &str = "UPDATE job_runs \
    SET config_hash = (SELECT j.config_hash FROM jobs j WHERE j.id = job_runs.job_id) \
    WHERE config_hash IS NULL";

/// Seed a `jobs` row with the given `config_hash` and return its id.
async fn seed_job_with_hash(pool: &DbPool, name: &str, config_hash: &str) -> i64 {
    let now = chrono::Utc::now().to_rfc3339();
    let writer = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let row = sqlx::query(
        "INSERT INTO jobs (name, schedule, resolved_schedule, job_type, config_json, config_hash, timeout_secs, created_at, updated_at) \
         VALUES (?1, '* * * * *', '* * * * *', 'command', '{}', ?2, 60, ?3, ?3) \
         RETURNING id",
    )
    .bind(name)
    .bind(config_hash)
    .bind(&now)
    .fetch_one(writer)
    .await
    .expect("seed jobs row");
    row.get::<i64, _>("id")
}

/// Insert a `job_runs` row with the given (optional) `config_hash`.
/// `job_run_number` is bound to 1 — Phase 11 made it NOT NULL; the
/// test only ever seeds one run per job so a constant is sufficient.
async fn seed_job_run(pool: &DbPool, job_id: i64, config_hash: Option<&str>) -> i64 {
    let now = chrono::Utc::now().to_rfc3339();
    let writer = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let row = sqlx::query(
        "INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number, config_hash) \
         VALUES (?1, 'success', 'manual', ?2, 1, ?3) RETURNING id",
    )
    .bind(job_id)
    .bind(&now)
    .bind(config_hash)
    .fetch_one(writer)
    .await
    .expect("seed job_runs row");
    row.get::<i64, _>("id")
}

async fn fetch_run_config_hash(pool: &DbPool, run_id: i64) -> Option<String> {
    let reader = match pool.reader() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    sqlx::query_scalar("SELECT config_hash FROM job_runs WHERE id = ?1")
        .bind(run_id)
        .fetch_one(reader)
        .await
        .expect("read config_hash")
}

#[tokio::test]
async fn backfill_populates_config_hash_for_pre_v12_rows() {
    let pool = setup_sqlite_with_phase11_migrations().await;
    let job_id = seed_job_with_hash(&pool, "backfill-target", "abc123").await;

    // Insert a run with config_hash = NULL (pre-v1.2 shape). Migration
    // _000007 has already run (no-op, table was empty), so we manually
    // re-run the same SQL to populate this row.
    let run_id = seed_job_run(&pool, job_id, None).await;
    assert_eq!(
        fetch_run_config_hash(&pool, run_id).await,
        None,
        "pre-condition: config_hash starts NULL"
    );

    let writer = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    sqlx::query(BACKFILL_SQL)
        .execute(writer)
        .await
        .expect("re-run backfill UPDATE");

    assert_eq!(
        fetch_run_config_hash(&pool, run_id).await.as_deref(),
        Some("abc123"),
        "backfill must populate config_hash from jobs.config_hash"
    );
}

#[tokio::test]
async fn backfill_is_idempotent() {
    // pool.migrate() applies _000007 once during setup. Calling it a
    // second time must not error and must not change already-populated
    // values (the WHERE config_hash IS NULL guard makes the UPDATE a
    // no-op for rows already populated).
    let pool = setup_sqlite_with_phase11_migrations().await;
    let job_id = seed_job_with_hash(&pool, "idempotent-target", "stable-hash").await;
    let run_id = seed_job_run(&pool, job_id, Some("stable-hash")).await;

    pool.migrate().await.expect("second migrate (idempotent)");

    assert_eq!(
        fetch_run_config_hash(&pool, run_id).await.as_deref(),
        Some("stable-hash"),
        "idempotent re-run must leave existing config_hash untouched"
    );

    let writer = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    sqlx::query(BACKFILL_SQL)
        .execute(writer)
        .await
        .expect("third bare backfill UPDATE");

    assert_eq!(
        fetch_run_config_hash(&pool, run_id).await.as_deref(),
        Some("stable-hash"),
        "third backfill UPDATE must remain a no-op for already-populated rows"
    );
}

#[tokio::test]
async fn orphaned_rows_stay_null() {
    let pool = setup_sqlite_with_phase11_migrations().await;

    // Seed a real job (so job_id 1 exists) but then point our test row at
    // a non-existent job_id. SQLite enforces FK only when foreign_keys=ON
    // AND a row in the parent exists at insert time. We disable FK
    // checks for this raw insert to simulate the orphan scenario the
    // backfill must tolerate.
    let writer = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(writer)
        .await
        .expect("disable FK");

    let now = chrono::Utc::now().to_rfc3339();
    let row = sqlx::query(
        "INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number, config_hash) \
         VALUES (?1, 'success', 'manual', ?2, 1, NULL) RETURNING id",
    )
    .bind(99_999_i64)
    .bind(&now)
    .fetch_one(writer)
    .await
    .expect("orphan insert");
    let orphan_run_id: i64 = row.get("id");

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(writer)
        .await
        .expect("re-enable FK");

    // Run the same UPDATE the migration runs. The correlated subquery
    // returns NULL for the missing job_id; the SET assigns NULL → NULL
    // (no semantic change), so the row stays NULL.
    sqlx::query(BACKFILL_SQL)
        .execute(writer)
        .await
        .expect("re-run backfill UPDATE");

    assert_eq!(
        fetch_run_config_hash(&pool, orphan_run_id).await,
        None,
        "orphaned row must keep config_hash NULL after backfill"
    );
}

#[tokio::test]
async fn columns_exist_after_full_migrate() {
    let pool = setup_sqlite_with_phase11_migrations().await;
    let reader = match pool.reader() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let cols: Vec<(String, i64)> =
        sqlx::query_as("SELECT name, \"notnull\" FROM pragma_table_info('job_runs')")
            .fetch_all(reader)
            .await
            .expect("pragma table_info job_runs");

    let names: Vec<&String> = cols.iter().map(|(n, _)| n).collect();
    assert!(
        cols.iter().any(|(n, _)| n == "image_digest"),
        "image_digest column missing from job_runs: {names:?}"
    );
    assert!(
        cols.iter().any(|(n, _)| n == "config_hash"),
        "config_hash column missing from job_runs: {names:?}"
    );

    // Both columns must be NULLABLE (notnull == 0).
    let image_digest = cols
        .iter()
        .find(|(n, _)| n == "image_digest")
        .expect("image_digest present");
    assert_eq!(image_digest.1, 0, "image_digest must be NULLABLE");
    let config_hash = cols
        .iter()
        .find(|(n, _)| n == "config_hash")
        .expect("config_hash present");
    assert_eq!(config_hash.1, 0, "config_hash must be NULLABLE");
}
