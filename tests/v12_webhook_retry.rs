//! Phase 20 / WH-05: Wave 0 stub for the 3-attempt retry chain integration tests.
//! Plan 02 (RetryingDispatcher) appends `#[tokio::test]` functions here.
//! Tests assert: 3 attempts at t=0, t=30s, t=300s with full-jitter; 4xx-permanent
//! short-circuit at attempt 1; 5xx-exhausted writes one DLQ row at attempt 3.

#[allow(dead_code)]
const PHASE_MARKER: &str = "Phase 20 / WH-05 / RetryingDispatcher";
