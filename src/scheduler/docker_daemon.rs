//! Docker daemon startup pre-flight ping + cronduit_docker_reachable gauge wiring.
//!
//! Phase 8 D-11..D-14. This module is NOT the same as `docker_preflight` — that
//! module validates per-job network configuration (container:<name> targets,
//! named networks) before every container create. This module runs exactly once
//! at startup to check whether the Docker daemon itself is reachable, and exposes
//! the result via a Prometheus gauge so operators can write alerting rules on
//! `cronduit_docker_reachable == 0` without parsing logs.
//!
//! Non-fatal by design (D-14): even if the daemon is unreachable, cronduit keeps
//! booting so command/script-only configs still run. Per-run docker failures
//! surface through the existing `cronduit_run_failures_total{reason=...}` closed
//! enum from Phase 6 D-05; the gauge is a coarse liveness signal, not a
//! replacement for per-run observability.

use bollard::Docker;

/// Set the `cronduit_docker_reachable` gauge to 1.0 (reachable) or 0.0 (unreachable).
///
/// Called from two places:
/// - `preflight_ping` at startup (D-11).
/// - Opportunistically from docker job launch code on first success after a failure
///   (D-12/D-13). The scheduler's docker job hot path calls this with `true` when a
///   container create succeeds, allowing the gauge to recover without a config reload.
pub fn update_reachable_gauge(reachable: bool) {
    let value = if reachable { 1.0 } else { 0.0 };
    metrics::gauge!("cronduit_docker_reachable").set(value);
}

/// Run the startup pre-flight Docker ping.
///
/// D-11: fires once, after the Docker client is created and before the scheduler
/// loop begins. D-14: non-fatal — a failed ping never propagates an error up;
/// the function only logs and updates the gauge.
///
/// Arguments:
/// - `docker`: `Some(&Docker)` when `bollard::Docker::connect_with_local_defaults()`
///   succeeded; `None` when it failed upstream. Both cases flip the gauge to 0 and
///   emit a WARN line so the operator sees something regardless of which side broke.
pub async fn preflight_ping(docker: Option<&Docker>) {
    let Some(docker) = docker else {
        tracing::warn!(
            target: "cronduit.docker",
            error = "docker client not initialized",
            "docker daemon unreachable: client init failed. cronduit will keep scheduling command/script jobs; docker jobs will fail. remediation: set DOCKER_GID to the host docker group (Linux: `stat -c %g /var/run/docker.sock`; Rancher Desktop macOS: 102). See README Troubleshooting.",
        );
        update_reachable_gauge(false);
        return;
    };

    match docker.ping().await {
        Ok(_) => {
            tracing::info!(
                target: "cronduit.docker",
                "docker daemon reachable"
            );
            update_reachable_gauge(true);
        }
        Err(err) => {
            tracing::warn!(
                target: "cronduit.docker",
                error = %err,
                "docker daemon unreachable. cronduit will keep scheduling command/script jobs; docker jobs will fail. remediation: set DOCKER_GID to the host docker group (Linux: `stat -c %g /var/run/docker.sock`; Rancher Desktop macOS: 102). See README Troubleshooting."
            );
            update_reachable_gauge(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn preflight_ping_with_none_sets_gauge_zero_and_does_not_panic() {
        // Safe to call without an installed recorder: metrics::gauge!() is a no-op
        // when no recorder is attached. This proves the function is non-fatal.
        preflight_ping(None).await;
        // No assertion on gauge value here — tests/docker_daemon_preflight.rs
        // installs a real recorder and asserts the render output.
    }

    #[test]
    fn update_reachable_gauge_is_safe_without_recorder() {
        update_reachable_gauge(true);
        update_reachable_gauge(false);
    }
}
