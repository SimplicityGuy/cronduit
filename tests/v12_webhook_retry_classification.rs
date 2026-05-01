//! Phase 20 / WH-05: Wave 0 stub for the D-06 classification table coverage.
//! Plan 02 appends `#[tokio::test]` functions here.
//! Tests assert: 200 → success; 408/429 → transient; 4xx (other) → permanent;
//! 5xx → transient; reqwest network error → transient; reqwest timeout → transient.

#[allow(dead_code)]
const PHASE_MARKER: &str = "Phase 20 / WH-05 / classification table D-06";
