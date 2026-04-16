//! Phase 11 UI-19 race fix: Run Now handler must insert the job_runs row
//! synchronously before returning HX-Refresh, so immediate click-through
//! never 404s. Bodies land in Plan 11-06.

#![allow(clippy::assertions_on_constants)]

mod common;

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-06"]
async fn handler_inserts_before_response() {
    assert!(true, "stub — see Plan 11-06");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-06"]
async fn no_race_after_run_now() {
    assert!(true, "stub — see Plan 11-06");
}

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-06"]
async fn scheduler_cmd_run_now_with_run_id_variant() {
    assert!(true, "stub — see Plan 11-06");
}
