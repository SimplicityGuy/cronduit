//! Phase 11 per-job counter tests (DB-11). Covers T-V11-RUNNUM-02/-10/-11.
//! Bodies land in Plan 11-05.

#![allow(clippy::assertions_on_constants)]

mod common;

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-05"]
async fn runnum_starts_at_1() {
    assert!(true, "stub — see Plan 11-05");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-05"]
async fn insert_running_run_uses_counter_transaction() {
    assert!(true, "stub — see Plan 11-05");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-05"]
async fn concurrent_inserts_distinct_numbers() {
    assert!(true, "stub — see Plan 11-05");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-05"]
async fn next_run_number_invariant() {
    assert!(true, "stub — see Plan 11-05");
}
