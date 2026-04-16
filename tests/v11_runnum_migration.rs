//! Phase 11 migration integration tests (DB-09, DB-10, DB-12).
//!
//! Wave-0 stubs. Each `#[ignore]` test is filled in by its owning plan:
//!   - migration_01_*   — Plan 11-02
//!   - migration_02_*   — Plan 11-03
//!   - migration_03_*   — Plan 11-04

// Wave-0 stubs: each #[ignore] test has an `assert!(true, "stub — see Plan ...")`
// body so the owning plan is recorded inline and the files compile. Owners
// replace the assertion with real logic when they land.
#![allow(clippy::assertions_on_constants)]

mod common;

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-02"]
async fn migration_01_add_nullable_columns() {
    assert!(true, "stub — see Plan 11-02");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-02"]
async fn migration_01_idempotent() {
    assert!(true, "stub — see Plan 11-02");
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
