//! Phase 11 migration integration tests (DB-09, DB-10, DB-12).
//!
//! Wave-0 stubs. Each `#[ignore]` test is filled in by its owning plan:
//!   - migration_01_*   — Plan 11-02  (ACTIVE: bodies landed)
//!   - migration_02_*   — Plan 11-03
//!   - migration_03_*   — Plan 11-04

// Wave-0 stubs: each #[ignore] test has an `assert!(true, "stub — see Plan ...")`
// body so the owning plan is recorded inline and the files compile. Owners
// replace the assertion with real logic when they land.
#![allow(clippy::assertions_on_constants)]

mod common;

use common::v11_fixtures::setup_sqlite_with_phase11_migrations;
use cronduit::db::queries::PoolRef;

#[tokio::test]
async fn migration_01_add_nullable_columns() {
    let pool = setup_sqlite_with_phase11_migrations().await;
    let sqlite_pool = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };

    // PRAGMA table_info returns (cid, name, type, notnull, dflt_value, pk)
    let jobs_info: Vec<(String, String, i64, Option<String>)> =
        sqlx::query_as("SELECT name, type, \"notnull\", dflt_value FROM pragma_table_info('jobs')")
            .fetch_all(sqlite_pool)
            .await
            .expect("pragma jobs");

    let nrn = jobs_info
        .iter()
        .find(|r| r.0 == "next_run_number")
        .expect("jobs.next_run_number exists");
    assert_eq!(nrn.1.to_uppercase(), "INTEGER", "next_run_number type");
    assert_eq!(nrn.2, 1, "next_run_number must be NOT NULL");
    assert_eq!(nrn.3.as_deref(), Some("1"), "next_run_number DEFAULT 1");

    let job_runs_info: Vec<(String, String, i64, Option<String>)> = sqlx::query_as(
        "SELECT name, type, \"notnull\", dflt_value FROM pragma_table_info('job_runs')",
    )
    .fetch_all(sqlite_pool)
    .await
    .expect("pragma job_runs");

    let jrn = job_runs_info
        .iter()
        .find(|r| r.0 == "job_run_number")
        .expect("job_runs.job_run_number exists");
    assert_eq!(jrn.1.to_uppercase(), "INTEGER", "job_run_number type");
    assert_eq!(
        jrn.2, 0,
        "job_run_number must be nullable in file 1 (file 3 tightens)"
    );
}

#[tokio::test]
async fn migration_01_idempotent() {
    let pool = setup_sqlite_with_phase11_migrations().await;
    // Second migrate() is a no-op because sqlx records applied migrations.
    pool.migrate().await.expect("re-migrate is a no-op");
    // Verify columns still exist after the second run.
    let sqlite_pool = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('jobs') WHERE name = 'next_run_number'",
    )
    .fetch_one(sqlite_pool)
    .await
    .expect("count next_run_number column");
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-03"]
async fn migration_02_backfill_completes() {
    assert!(true, "stub — see Plan 11-03");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-03"]
async fn migration_02_logs_progress() {
    assert!(true, "stub — see Plan 11-03");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-03"]
async fn migration_02_resume_after_crash() {
    assert!(true, "stub — see Plan 11-03");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-03"]
async fn migration_02_counter_reseed() {
    assert!(true, "stub — see Plan 11-03");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-03"]
async fn migration_02_row_number_order_by_id() {
    assert!(true, "stub — see Plan 11-03");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-04"]
async fn migration_03_sqlite_table_rewrite() {
    assert!(true, "stub — see Plan 11-04");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-04"]
async fn migration_03_sqlite_indexes_preserved() {
    assert!(true, "stub — see Plan 11-04");
}

#[cfg(feature = "integration")]
#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-04 (integration-gated)"]
async fn migration_03_postgres_not_null() {
    assert!(true, "stub — see Plan 11-04");
}
