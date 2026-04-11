//! Network pre-flight validation before Docker container creation.
//!
//! D-10: Strict container:<name> validation -- target must be running.
//! D-11: Pre-flight named networks -- inspect before create.
//! D-12: Distinct error categories for pre-flight failures.

use bollard::Docker;
use bollard::models::ContainerStateStatusEnum;

/// Pre-flight validation error categories (D-12).
///
/// Each variant maps to a distinct operator-actionable error:
/// - Infrastructure problem (Docker unavailable)
/// - Configuration problem (target container down or network missing)
#[derive(Debug)]
pub enum PreflightError {
    /// Docker daemon is unreachable (socket error, permission denied).
    DockerUnavailable(String),
    /// Target container for `container:<name>` mode is not running.
    NetworkTargetUnavailable(String),
    /// Named network does not exist.
    NetworkNotFound(String),
}

impl PreflightError {
    /// Return structured error message for storage in `job_runs.error_message`.
    pub fn to_error_message(&self) -> String {
        match self {
            Self::DockerUnavailable(msg) => format!("docker_unavailable: {msg}"),
            Self::NetworkTargetUnavailable(name) => {
                format!("network_target_unavailable: {name}")
            }
            Self::NetworkNotFound(name) => format!("network_not_found: {name}"),
        }
    }
}

impl std::fmt::Display for PreflightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_error_message())
    }
}

impl std::error::Error for PreflightError {}

/// Built-in Docker network modes that need no pre-flight validation.
const BUILTIN_NETWORKS: &[&str] = &["bridge", "host", "none", ""];

/// Returns true if the network mode is a built-in that needs no pre-flight.
fn is_builtin_network(network_mode: &str) -> bool {
    BUILTIN_NETWORKS.contains(&network_mode)
}

/// Validate network configuration before creating a Docker container.
///
/// - `container:<name>`: inspects target container and verifies running state (D-10).
/// - Built-in modes (`bridge`, `host`, `none`, `""`): no validation needed.
/// - Named network: inspects network existence (D-11).
///
/// Returns three distinct error categories (D-12):
/// - `DockerUnavailable`: Docker daemon unreachable
/// - `NetworkTargetUnavailable`: target container not running
/// - `NetworkNotFound`: named network does not exist
pub async fn preflight_network(
    docker: &Docker,
    network_mode: &str,
) -> Result<(), PreflightError> {
    if let Some(target) = network_mode.strip_prefix("container:") {
        // container:<name> mode -- inspect target and verify running (D-10).
        validate_container_target(docker, target).await
    } else if is_builtin_network(network_mode) {
        // Built-in network modes need no pre-flight.
        Ok(())
    } else {
        // Named network -- verify existence (D-11).
        validate_named_network(docker, network_mode).await
    }
}

/// Validate that a target container exists and is running.
async fn validate_container_target(
    docker: &Docker,
    target: &str,
) -> Result<(), PreflightError> {
    match docker.inspect_container(target, None).await {
        Ok(response) => {
            // Check container state.
            let status = response
                .state
                .as_ref()
                .and_then(|s| s.status);

            match status {
                Some(ContainerStateStatusEnum::RUNNING) => Ok(()),
                Some(other) => {
                    tracing::warn!(
                        target: "cronduit.docker.preflight",
                        container = target,
                        status = ?other,
                        "target container is not running"
                    );
                    Err(PreflightError::NetworkTargetUnavailable(
                        target.to_string(),
                    ))
                }
                None => {
                    tracing::warn!(
                        target: "cronduit.docker.preflight",
                        container = target,
                        "target container has no status"
                    );
                    Err(PreflightError::NetworkTargetUnavailable(
                        target.to_string(),
                    ))
                }
            }
        }
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 404,
            ..
        }) => {
            // Container does not exist.
            Err(PreflightError::NetworkTargetUnavailable(
                target.to_string(),
            ))
        }
        Err(e) => {
            // Docker daemon error -- classify as unavailable.
            Err(PreflightError::DockerUnavailable(e.to_string()))
        }
    }
}

/// Validate that a named network exists.
async fn validate_named_network(
    docker: &Docker,
    network_name: &str,
) -> Result<(), PreflightError> {
    match docker.inspect_network(network_name, None).await {
        Ok(_) => Ok(()),
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 404,
            ..
        }) => {
            // Network does not exist.
            Err(PreflightError::NetworkNotFound(
                network_name.to_string(),
            ))
        }
        Err(e) => {
            // Docker daemon error.
            Err(PreflightError::DockerUnavailable(e.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preflight_error_messages() {
        let err = PreflightError::DockerUnavailable("socket error".to_string());
        assert_eq!(err.to_error_message(), "docker_unavailable: socket error");

        let err = PreflightError::NetworkTargetUnavailable("vpn".to_string());
        assert_eq!(
            err.to_error_message(),
            "network_target_unavailable: vpn"
        );

        let err = PreflightError::NetworkNotFound("mynet".to_string());
        assert_eq!(err.to_error_message(), "network_not_found: mynet");
    }

    #[test]
    fn test_builtin_networks_skip_preflight() {
        // Verify that built-in network modes are recognized.
        assert!(is_builtin_network("bridge"));
        assert!(is_builtin_network("host"));
        assert!(is_builtin_network("none"));
        assert!(is_builtin_network(""));

        // Named and container: modes are NOT built-in.
        assert!(!is_builtin_network("my-custom-net"));
        assert!(!is_builtin_network("container:vpn"));
    }

    #[test]
    fn test_container_mode_parsing() {
        // Verify strip_prefix logic used in preflight_network.
        let mode = "container:wireguard";
        let target = mode.strip_prefix("container:");
        assert_eq!(target, Some("wireguard"));

        let mode = "bridge";
        let target = mode.strip_prefix("container:");
        assert_eq!(target, None);
    }
}
