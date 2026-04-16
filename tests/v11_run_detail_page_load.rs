//! Phase 11 page-load backfill + URL compat (UI-17, DB-13).
//! Covers T-V11-BACK-01/02, T-V11-RUNNUM-12/13, VALIDATION rows 11-09-01/02 + 11-12-02.
//! Bodies land in Plan 11-09 and Plan 11-12.

#![allow(clippy::assertions_on_constants)]

mod common;

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-09"]
async fn renders_static_backfill() {
    assert!(true, "stub — see Plan 11-09");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-09"]
async fn permalink_by_global_id() {
    assert!(true, "stub — see Plan 11-09");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-09"]
async fn get_recent_job_logs_chronological() {
    assert!(true, "stub — see Plan 11-09");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-12"]
async fn header_renders_runnum_with_id_suffix() {
    assert!(true, "stub — see Plan 11-12");
}
