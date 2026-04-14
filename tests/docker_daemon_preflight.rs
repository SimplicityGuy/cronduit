//! Phase 8 D-11..D-14 integration test for the cronduit_docker_reachable gauge.
//!
//! Asserts three behaviors through the real Prometheus exporter render output
//! (the same render path /metrics serves):
//!   1. Gauge is registered and described from boot with initial value 0.
//!   2. update_reachable_gauge(true/false) flips the rendered value.
//!   3. preflight_ping(None) forces the gauge to 0 even if it was previously 1.
//!
//! All three assertions run in a single test function because
//! telemetry::setup_metrics() uses a process-global OnceLock — multiple tests
//! racing on it would observe each other's side effects.

use cronduit::scheduler::docker_daemon;
use cronduit::telemetry;

#[tokio::test]
async fn docker_daemon_preflight_gauge_lifecycle() {
    let handle = telemetry::setup_metrics();

    // --- Phase 1: registered from boot at zero -------------------------------
    let body = handle.render();
    assert!(
        body.contains("# HELP cronduit_docker_reachable"),
        "cronduit_docker_reachable HELP line missing from render output; got:\n{body}"
    );
    assert!(
        body.contains("# TYPE cronduit_docker_reachable gauge"),
        "cronduit_docker_reachable TYPE line missing from render output; got:\n{body}"
    );
    assert!(
        body.contains("cronduit_docker_reachable 0"),
        "cronduit_docker_reachable initial value should be 0 at boot; got:\n{body}"
    );

    // --- Phase 2: update_reachable_gauge(true) flips to 1 --------------------
    docker_daemon::update_reachable_gauge(true);
    let body = handle.render();
    assert!(
        body.contains("cronduit_docker_reachable 1"),
        "cronduit_docker_reachable should be 1 after update_reachable_gauge(true); got:\n{body}"
    );

    // --- Phase 3: update_reachable_gauge(false) flips back to 0 --------------
    docker_daemon::update_reachable_gauge(false);
    let body = handle.render();
    assert!(
        body.contains("cronduit_docker_reachable 0"),
        "cronduit_docker_reachable should be 0 after update_reachable_gauge(false); got:\n{body}"
    );

    // --- Phase 4: preflight_ping(None) forces zero even after a prior 1 ------
    docker_daemon::update_reachable_gauge(true);
    let body_before = handle.render();
    assert!(
        body_before.contains("cronduit_docker_reachable 1"),
        "preflight_ping test setup failed: expected 1 before ping; got:\n{body_before}"
    );

    docker_daemon::preflight_ping(None).await;
    let body_after = handle.render();
    assert!(
        body_after.contains("cronduit_docker_reachable 0"),
        "preflight_ping(None) should force cronduit_docker_reachable to 0; got:\n{body_after}"
    );
}
