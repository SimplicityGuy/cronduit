//! Phase 20 / WH-11: labeled webhook metric family integration tests.
//!
//! Locks the operator-visible /metrics contract for the new
//! `cronduit_webhook_*` family at boot:
//!   - HELP + TYPE lines for `_deliveries_total`, `_delivery_duration_seconds`,
//!     `_queue_depth`, and the preserved P15 `_delivery_dropped_total`.
//!   - Closed-enum status zero-baseline rows
//!     `_deliveries_total{status="success|failed|dropped"} 0`.
//!   - Histogram bucket boundaries match RESEARCH §4.4
//!     `[0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]`.
//!   - `_queue_depth` gauge zero-baselined.
//!   - REMOVED: P18 `_sent_total` and `_failed_total` flat counters
//!     (D-22 BREAKING CHANGE).
//!   - PRESERVED: P15 `_delivery_dropped_total` (D-26 channel-saturation
//!     vs drain-on-shutdown semantic split).

use cronduit::telemetry::setup_metrics;

#[test]
fn webhook_family_has_help_and_type_at_boot() {
    let handle = setup_metrics();
    let body = handle.render();

    for metric in [
        "cronduit_webhook_deliveries_total",
        "cronduit_webhook_delivery_duration_seconds",
        "cronduit_webhook_queue_depth",
        "cronduit_webhook_delivery_dropped_total", // P15 — preserved per D-22/D-26.
    ] {
        assert!(
            body.contains(&format!("# HELP {metric}")),
            "missing # HELP for {metric}; body:\n{body}"
        );
    }

    assert!(
        body.contains("# TYPE cronduit_webhook_deliveries_total counter"),
        "missing # TYPE counter for _deliveries_total; body:\n{body}"
    );
    assert!(
        body.contains("# TYPE cronduit_webhook_delivery_duration_seconds histogram"),
        "missing # TYPE histogram for _delivery_duration_seconds; body:\n{body}"
    );
    assert!(
        body.contains("# TYPE cronduit_webhook_queue_depth gauge"),
        "missing # TYPE gauge for _queue_depth; body:\n{body}"
    );
    assert!(
        body.contains("# TYPE cronduit_webhook_delivery_dropped_total counter"),
        "missing # TYPE counter for _delivery_dropped_total (P15 preserved); body:\n{body}"
    );
}

#[test]
fn webhook_family_status_seed_zero_at_boot() {
    let handle = setup_metrics();
    let body = handle.render();

    // The status-only seed (no job label) — per Plan 05 Task 2 / D-22+D-23.
    // The exporter renders the line as
    //   `cronduit_webhook_deliveries_total{status="<status>"} 0`
    // because only the `status` label is populated by the seed loop in
    // src/telemetry.rs. Per-job × per-status rows materialize on first
    // observation (or after Plan 06 wires the per-job seed at boot).
    for status in ["success", "failed", "dropped"] {
        let line = format!("cronduit_webhook_deliveries_total{{status=\"{status}\"}} 0");
        assert!(
            body.contains(&line),
            "missing zero-baseline `{line}`; body:\n{body}"
        );
    }
}

#[test]
fn webhook_family_old_flat_counters_removed() {
    // D-22 BREAKING: P18 flat counters _sent_total and _failed_total are
    // REPLACED by the labeled `cronduit_webhook_deliveries_total{status=...}`
    // family. This test locks the breaking change so a future regression
    // accidentally re-adding the flat counters fails CI.
    let handle = setup_metrics();
    let body = handle.render();
    assert!(
        !body.contains("cronduit_webhook_delivery_sent_total"),
        "P18 _sent_total counter must be removed (D-22 BREAKING); body:\n{body}"
    );
    assert!(
        !body.contains("cronduit_webhook_delivery_failed_total"),
        "P18 _failed_total counter must be removed (D-22 BREAKING); body:\n{body}"
    );
}

#[test]
fn webhook_histogram_buckets_match_research() {
    // RESEARCH §4.4 / D-24: operator-tuned bucket boundaries
    // [0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]. The 10s top matches
    // reqwest's per-attempt timeout cap (P18 D-18).
    let handle = setup_metrics();
    let body = handle.render();
    // Each bucket renders as a `_bucket{le="<n>"} <count>` line. Integer-
    // valued boundaries render without a trailing `.0` in this exporter
    // (verified empirically against tests/metrics_endpoint.rs output:
    // `le="1"`, `le="5"`, `le="10"`). Accept either form to be tolerant.
    for le in ["0.05", "0.1", "0.25", "0.5", "1", "2.5", "5", "10"] {
        let alt = format!("{le}.0");
        let needle_a = format!("cronduit_webhook_delivery_duration_seconds_bucket{{le=\"{le}\"}}");
        let needle_b = format!("cronduit_webhook_delivery_duration_seconds_bucket{{le=\"{alt}\"}}");
        assert!(
            body.contains(&needle_a) || body.contains(&needle_b),
            "missing histogram bucket le=\"{le}\" (or {alt}); body:\n{body}"
        );
    }
    // The +Inf catch-all bucket must always render.
    assert!(
        body.contains("cronduit_webhook_delivery_duration_seconds_bucket{le=\"+Inf\"}"),
        "missing +Inf bucket; body:\n{body}"
    );
}

#[test]
fn webhook_queue_depth_zero_at_boot() {
    // D-25: the `_queue_depth` gauge is zero-baselined in setup_metrics().
    // Live updates fire from src/webhooks/worker.rs (Plan 04 wiring).
    let handle = setup_metrics();
    let body = handle.render();
    assert!(
        body.contains("cronduit_webhook_queue_depth 0"),
        "queue_depth must be zero-baselined at boot; body:\n{body}"
    );
}

#[test]
fn webhook_p15_dropped_counter_preserved() {
    // D-26: the P15 channel-saturation `_delivery_dropped_total` counter STAYS
    // as a SEPARATE family from the new labeled `_deliveries_total{status=...}`.
    // Operators dashboards keyed off `_delivery_dropped_total` for
    // backpressure detection stay green across the v1.1→v1.2 boundary.
    let handle = setup_metrics();
    let body = handle.render();
    assert!(
        body.contains("# HELP cronduit_webhook_delivery_dropped_total"),
        "P15 _delivery_dropped_total HELP must be preserved per D-22/D-26; body:\n{body}"
    );
    assert!(
        body.contains("# TYPE cronduit_webhook_delivery_dropped_total counter"),
        "P15 _delivery_dropped_total TYPE must be preserved per D-22/D-26; body:\n{body}"
    );
    // The zero-baseline row must render too — the unlabeled counter line is
    // `cronduit_webhook_delivery_dropped_total 0` (no labels).
    assert!(
        body.contains("cronduit_webhook_delivery_dropped_total 0"),
        "P15 _delivery_dropped_total zero-baseline must be preserved; body:\n{body}"
    );
}
