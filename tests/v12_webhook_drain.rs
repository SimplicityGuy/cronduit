//! Phase 20 / WH-10: Wave 0 stub for drain-on-shutdown integration tests.
//! Plan 04 (worker drain budget) appends `#[tokio::test]` functions here.
//! Tests assert: SIGTERM → drain budget enforced; in-flight HTTP NOT cancelled;
//! remaining queued events drained-and-dropped at expiry; counter increments per drop.

#[allow(dead_code)]
const PHASE_MARKER: &str = "Phase 20 / WH-10 / drain budget D-15";
