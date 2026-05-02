//! Prometheus metrics integration tests (OPS-02).
//!
//! Validates that every cronduit metric family is render-visible in `/metrics`
//! output from boot, not just after first observation. GAP-1 closure in 06-06-PLAN.md.

use cronduit::telemetry;

/// GAP-1 closure test: after `setup_metrics()` installs the recorder, ALL cronduit
/// metric families must render HELP/TYPE lines even with zero runs and zero syncs.
///
/// Before 06-06-PLAN.md this would have failed because the metrics facade registers
/// metrics lazily and the Prometheus exporter does not render a metric until it has
/// been explicitly described OR observed. The fix in `src/telemetry.rs::setup_metrics`
/// adds `describe_gauge!` / `describe_counter!` / `describe_histogram!` calls after
/// `install_recorder()` so every cronduit_* family appears from boot.
#[test]
fn metrics_families_described_from_boot() {
    // Install the recorder. This is global per-process; cargo builds each integration
    // test file as a separate binary, so the global recorder is installed exactly once
    // inside this binary.
    let handle = telemetry::setup_metrics();

    let body = handle.render();

    // Every cronduit metric family must have HELP and TYPE lines from boot.
    assert!(
        body.contains("# HELP cronduit_scheduler_up"),
        "missing HELP for cronduit_scheduler_up; body: {body}"
    );
    assert!(
        body.contains("# TYPE cronduit_scheduler_up gauge"),
        "missing TYPE for cronduit_scheduler_up; body: {body}"
    );

    assert!(
        body.contains("# HELP cronduit_jobs_total"),
        "missing HELP for cronduit_jobs_total; body: {body}"
    );
    assert!(
        body.contains("# TYPE cronduit_jobs_total gauge"),
        "missing TYPE for cronduit_jobs_total; body: {body}"
    );

    assert!(
        body.contains("# HELP cronduit_runs_total"),
        "missing HELP for cronduit_runs_total; body: {body}"
    );
    assert!(
        body.contains("# TYPE cronduit_runs_total counter"),
        "missing TYPE for cronduit_runs_total; body: {body}"
    );

    assert!(
        body.contains("# HELP cronduit_run_duration_seconds"),
        "missing HELP for cronduit_run_duration_seconds; body: {body}"
    );
    assert!(
        body.contains("# TYPE cronduit_run_duration_seconds histogram"),
        "missing TYPE for cronduit_run_duration_seconds; body: {body}"
    );

    assert!(
        body.contains("# HELP cronduit_run_failures_total"),
        "missing HELP for cronduit_run_failures_total; body: {body}"
    );
    assert!(
        body.contains("# TYPE cronduit_run_failures_total counter"),
        "missing TYPE for cronduit_run_failures_total; body: {body}"
    );

    // Phase 15 / WH-02 / D-11 — drop counter must render HELP/TYPE from boot.
    assert!(
        body.contains("# HELP cronduit_webhook_delivery_dropped_total"),
        "missing HELP for cronduit_webhook_delivery_dropped_total; body: {body}"
    );
    assert!(
        body.contains("# TYPE cronduit_webhook_delivery_dropped_total counter"),
        "missing TYPE for cronduit_webhook_delivery_dropped_total; body: {body}"
    );

    // Phase 20 / WH-11 / D-22: P18 flat counters (`_sent_total`, `_failed_total`)
    // REMOVED. The labeled per-DELIVERY family `cronduit_webhook_deliveries_total`
    // is the boot-described replacement (`status` is the closed enum
    // {success, failed, dropped}). The full Phase 20 metric family contract is
    // exercised in tests/v12_webhook_metrics_family.rs — this test asserts only
    // the boot-described HELP/TYPE lines that other tests depend on.
    assert!(
        body.contains("# HELP cronduit_webhook_deliveries_total"),
        "expected deliveries_total HELP line (P20 D-22 replacement); rendered:\n{body}"
    );
    assert!(
        body.contains("# TYPE cronduit_webhook_deliveries_total counter"),
        "expected deliveries_total TYPE counter; rendered:\n{body}"
    );
    assert!(
        body.contains("# HELP cronduit_webhook_delivery_duration_seconds"),
        "expected delivery_duration_seconds HELP line (P20 D-24); rendered:\n{body}"
    );
    assert!(
        body.contains("# TYPE cronduit_webhook_delivery_duration_seconds histogram"),
        "expected delivery_duration_seconds TYPE histogram; rendered:\n{body}"
    );
    assert!(
        body.contains("# HELP cronduit_webhook_queue_depth"),
        "expected queue_depth HELP line (P20 D-25); rendered:\n{body}"
    );
    assert!(
        body.contains("# TYPE cronduit_webhook_queue_depth gauge"),
        "expected queue_depth TYPE gauge; rendered:\n{body}"
    );
    // P18 flat counters MUST be GONE per D-22 BREAKING CHANGE.
    assert!(
        !body.contains("cronduit_webhook_delivery_sent_total"),
        "P18 _sent_total counter must be removed (P20 D-22 BREAKING); rendered:\n{body}"
    );
    assert!(
        !body.contains("cronduit_webhook_delivery_failed_total"),
        "P18 _failed_total counter must be removed (P20 D-22 BREAKING); rendered:\n{body}"
    );
}

// The stubs below remain intentional Nyquist compliance placeholders. They are
// `#[ignore]`d and panic if run; they define the contract for future implementation
// of behavioral metrics tests that require a live AppState + axum test harness.
#[cfg(test)]
mod metrics_tests {
    #[tokio::test]
    #[ignore = "not yet implemented"]
    async fn metrics_endpoint_returns_prometheus_format() {
        // Setup: create AppState with metrics_handle
        // Act: GET /metrics
        // Assert: response is 200, Content-Type is text/plain; version=0.0.4
        // Assert: body contains "cronduit_scheduler_up 1"
        todo!("Implement metrics endpoint format test")
    }

    #[test]
    #[ignore = "not yet implemented"]
    fn failure_reason_labels_are_bounded_enum() {
        // Assert: FailureReason enum has exactly 6 variants
        // Assert: each variant produces a valid label string
        // Assert: no variant produces an empty or whitespace label
        todo!("Implement failure reason label validation test")
    }

    #[test]
    #[ignore = "not yet implemented"]
    fn failure_reason_classification_covers_known_errors() {
        // Assert: classify_failure_reason("timeout", None) => Timeout
        // Assert: classify_failure_reason("failed", None) => ExitNonzero
        // Assert: classify_failure_reason("error", Some("image pull failed:...")) => ImagePullFailed
        // Assert: classify_failure_reason("error", Some("network_target_unavailable: vpn")) => NetworkTargetUnavailable
        // Assert: classify_failure_reason("error", Some("orphaned at restart")) => Abandoned
        // Assert: classify_failure_reason("error", Some("unknown error")) => Unknown
        todo!("Implement failure classification test")
    }
}
