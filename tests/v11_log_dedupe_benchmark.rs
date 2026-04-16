//! T-V11-LOG-02 benchmark — gates Option A per CONTEXT.md D-02.
//! Body lands in Plan 11-01 (the Wave-1 benchmark-gate plan).
//!
//! Invariant: p95 insert latency for a 64-line batch against in-memory SQLite
//! must be < 50ms. If this fails on the CI runner, Phase 11 flips to Option B.

#![allow(clippy::assertions_on_constants)]

mod common;

#[tokio::test]
#[ignore = "Wave-0 stub — real body lands in Plan 11-01"]
async fn p95_under_50ms() {
    assert!(true, "stub — see Plan 11-01");
}
