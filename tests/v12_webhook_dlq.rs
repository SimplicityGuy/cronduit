//! Phase 20 / WH-05: Wave 0 stub for DLQ row write-path integration tests.
//! Plan 02 (RetryingDispatcher classification) appends `#[tokio::test]` functions here.
//! Tests assert: dlq_reason populated correctly per scenario (http_4xx, http_5xx,
//! network, timeout, shutdown_drain); no payload/signature columns; FK integrity.

#[allow(dead_code)]
const PHASE_MARKER: &str = "Phase 20 / WH-05 / DLQ rows D-10";
