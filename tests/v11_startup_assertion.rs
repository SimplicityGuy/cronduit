//! Phase 11 D-15 startup assertion. Covers T-V11-RUNNUM-03 + VALIDATION 11-13-01/02.
//! Bodies land in Plan 11-13.

#![allow(clippy::assertions_on_constants)]

mod common;

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-13"]
async fn panics_when_null_rows_present() {
    assert!(true, "stub — see Plan 11-13");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-13"]
async fn listener_after_backfill() {
    assert!(true, "stub — see Plan 11-13");
}
