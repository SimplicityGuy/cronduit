//! Docker executor: full container lifecycle management via bollard.
//!
//! Creates an ephemeral container with labels, volumes, env vars, and network_mode,
//! starts it, streams logs concurrently with wait, handles timeout/cancellation
//! via stop_container with 10s SIGTERM grace, drains logs to EOF, extracts image
//! digest, and explicitly removes the container after all state is captured.
//!
//! DOCKER-06: auto_remove=false to avoid the moby#8441 race.
//! DOCKER-07: Labels carry run_id and job_name (never secrets).

use std::collections::HashMap;
use std::time::Duration;

use bollard::models::{ContainerCreateBody, HostConfig};
use bollard::query_parameters::{
    CreateContainerOptions, RemoveContainerOptions, StopContainerOptions,
};
use bollard::Docker;
use futures_util::StreamExt;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use super::command::{ExecResult, RunStatus};
use super::log_pipeline::{LogSender, make_log_line};

/// Docker-specific fields deserialized from the job's `config_json`.
#[derive(Debug, Deserialize)]
pub struct DockerJobConfig {
    /// Docker image (required for docker-type jobs).
    pub image: String,
    /// Environment variables to pass to the container.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Volume bind mounts (e.g. `["/host:/container:ro"]`).
    #[serde(default)]
    pub volumes: Option<Vec<String>>,
    /// Command to run inside the container (overrides image CMD).
    #[serde(default)]
    pub cmd: Option<Vec<String>>,
    /// Network mode (e.g. `"host"`, `"container:<name>"`, `"bridge"`).
    #[serde(default)]
    pub network: Option<String>,
    /// Explicit container name (optional).
    #[serde(default)]
    pub container_name: Option<String>,
}

/// Result of a Docker job execution, extending `ExecResult` with container metadata.
#[derive(Debug)]
pub struct DockerExecResult {
    /// Standard execution result (exit code, status, error message).
    pub exec: ExecResult,
    /// Image digest from `inspect_container` after start (DOCKER-09).
    pub image_digest: Option<String>,
}

/// Execute a Docker job: create -> start -> inspect -> wait/timeout/cancel -> drain logs -> remove.
///
/// The lifecycle state machine:
/// Creating -> Starting -> Running -> Exited -> LogsDrained -> Removed
///
/// DOCKER-06: `auto_remove = false` prevents the moby#8441 race where Docker
/// removes the container before we can inspect its exit code.
pub async fn execute_docker(
    docker: &Docker,
    config_json: &str,
    job_name: &str,
    run_id: i64,
    timeout: Duration,
    cancel: CancellationToken,
    sender: LogSender,
) -> DockerExecResult {
    // Parse the docker-specific config from JSON.
    let config: DockerJobConfig = match serde_json::from_str(config_json) {
        Ok(c) => c,
        Err(e) => {
            sender.send(make_log_line(
                "system",
                format!("[docker config error: {e}]"),
            ));
            sender.close();
            return DockerExecResult {
                exec: ExecResult {
                    exit_code: None,
                    status: RunStatus::Error,
                    error_message: Some(format!("failed to parse docker config: {e}")),
                },
                image_digest: None,
            };
        }
    };

    // Pre-flight network validation (D-10, D-11, D-12).
    if let Some(ref network) = config.network {
        if let Err(e) = super::docker_preflight::preflight_network(docker, network).await {
            let err_msg = e.to_error_message();
            sender.send(make_log_line(
                "system",
                format!("[pre-flight failed: {err_msg}]"),
            ));
            sender.close();
            return DockerExecResult {
                exec: ExecResult {
                    exit_code: None,
                    status: RunStatus::Error,
                    error_message: Some(err_msg),
                },
                image_digest: None,
            };
        }
    }

    // Ensure image is available locally, pulling if necessary.
    let _image_digest = match super::docker_pull::ensure_image(docker, &config.image).await {
        Ok(digest) => digest,
        Err(e) => {
            let err_msg = format!("image pull failed: {e}");
            sender.send(make_log_line("system", format!("[{err_msg}]")));
            sender.close();
            return DockerExecResult {
                exec: ExecResult {
                    exit_code: None,
                    status: RunStatus::Error,
                    error_message: Some(err_msg),
                },
                image_digest: None,
            };
        }
    };

    // Build labels (T-04-03: only run_id and job_name, never secrets).
    let mut labels = HashMap::new();
    labels.insert("cronduit.run_id".to_string(), run_id.to_string());
    labels.insert("cronduit.job_name".to_string(), job_name.to_string());

    // Build env vars as KEY=VALUE strings.
    let env_vec: Vec<String> = config
        .env
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();

    // Build HostConfig with network_mode, volumes, and auto_remove=false.
    let host_config = HostConfig {
        network_mode: config.network.clone(),
        binds: config.volumes.clone(),
        auto_remove: Some(false), // CRITICAL: DOCKER-06
        ..Default::default()
    };

    // Build ContainerCreateBody (bollard 0.20 API).
    let container_body = ContainerCreateBody {
        image: Some(config.image.clone()),
        cmd: config.cmd.clone(),
        env: if env_vec.is_empty() {
            None
        } else {
            Some(env_vec)
        },
        labels: Some(labels),
        host_config: Some(host_config),
        ..Default::default()
    };

    // Build CreateContainerOptions with optional name.
    let create_options = config.container_name.as_ref().map(|name| {
        CreateContainerOptions {
            name: Some(name.clone()),
            ..Default::default()
        }
    });

    // Create the container.
    let container_id = match docker
        .create_container(create_options, container_body)
        .await
    {
        Ok(response) => response.id,
        Err(e) => {
            sender.send(make_log_line(
                "system",
                format!("[docker create error: {e}]"),
            ));
            sender.close();
            return DockerExecResult {
                exec: ExecResult {
                    exit_code: None,
                    status: RunStatus::Error,
                    error_message: Some(format!("failed to create container: {e}")),
                },
                image_digest: None,
            };
        }
    };

    tracing::info!(
        target: "cronduit.docker",
        container_id = %container_id,
        image = %config.image,
        job_name = %job_name,
        run_id,
        cmd = ?config.cmd,
        "container created"
    );

    // Start the container.
    if let Err(e) = docker.start_container(&container_id, None).await {
        sender.send(make_log_line(
            "system",
            format!("[docker start error: {e}]"),
        ));
        sender.close();
        // Attempt cleanup of the created-but-not-started container.
        cleanup_container(docker, &container_id).await;
        return DockerExecResult {
            exec: ExecResult {
                exit_code: None,
                status: RunStatus::Error,
                error_message: Some(format!("failed to start container: {e}")),
            },
            image_digest: None,
        };
    }

    // Extract image digest via inspect (DOCKER-09).
    let image_digest = match docker.inspect_container(&container_id, None).await {
        Ok(info) => info.image.unwrap_or_default(),
        Err(e) => {
            tracing::warn!(
                target: "cronduit.docker",
                container_id = %container_id,
                error = %e,
                "failed to inspect container for image digest"
            );
            String::new()
        }
    };

    // Spawn log streaming task (concurrent with wait).
    let log_docker = docker.clone();
    let log_container = container_id.clone();
    let log_sender = sender.clone();
    let log_handle = tokio::spawn(super::docker_log::stream_docker_logs(
        log_docker,
        log_container,
        log_sender,
    ));

    // Wait for container exit with timeout and cancel support.
    //
    // Strategy: try `wait_container` first (efficient, long-poll). If it errors
    // immediately (some Docker runtimes like Rancher Desktop don't support wait),
    // fall back to polling `inspect_container` every 250ms.
    let exec_result = tokio::select! {
        exit_code = async {
            // Try the wait API first (efficient long-poll).
            let mut stream = docker.wait_container(&container_id, None);
            match stream.next().await {
                Some(Ok(response)) => {
                    // Wait succeeded — container exited.
                    response.status_code as i32
                }
                Some(Err(_)) | None => {
                    // Wait failed or stream closed — fall back to inspect polling.
                    // This handles Docker runtimes where wait returns an error immediately
                    // (e.g. Rancher Desktop).
                    //
                    // IMPORTANT: Check for status == "exited" specifically, not just
                    // !running. During container creation, running=false but the container
                    // hasn't started yet — we must wait until it exits, not catch it
                    // in the "created" state.
                    loop {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                        match docker.inspect_container(&container_id, None).await {
                            Ok(info) => {
                                if let Some(state) = &info.state {
                                    use bollard::models::ContainerStateStatusEnum;
                                    match state.status {
                                        Some(ContainerStateStatusEnum::EXITED)
                                        | Some(ContainerStateStatusEnum::DEAD) => {
                                            return state.exit_code.unwrap_or(-1) as i32;
                                        }
                                        _ => {} // Still running or starting — keep polling.
                                    }
                                }
                            }
                            Err(_) => {
                                // Container gone (already removed?) — treat as exit 0.
                                return 0;
                            }
                        }
                    }
                }
            }
        } => {
            let code = exit_code;
            let status = if code == 0 { RunStatus::Success } else { RunStatus::Failed };

            let _ = log_handle.await;
            sender.close();

            ExecResult { exit_code: Some(code), status, error_message: None }
        }

        _ = tokio::time::sleep(timeout) => {
            // Timeout: send SIGTERM via stop_container with 10s grace (D-04).
            sender.send(make_log_line("system", format!("[timeout after {timeout:?}, stopping container]")));
            let _ = docker.stop_container(
                &container_id,
                Some(StopContainerOptions {
                    t: Some(10),
                    ..Default::default()
                }),
            ).await;

            // D-05: Drain logs to EOF.
            let _ = log_handle.await;
            sender.close();

            ExecResult {
                exit_code: None,
                status: RunStatus::Timeout,
                error_message: Some(format!("timed out after {timeout:?}")),
            }
        }

        _ = cancel.cancelled() => {
            // Shutdown cancellation: stop with 10s grace (D-06).
            sender.send(make_log_line("system", "[shutdown signal received, stopping container]".to_string()));
            let _ = docker.stop_container(
                &container_id,
                Some(StopContainerOptions {
                    t: Some(10),
                    ..Default::default()
                }),
            ).await;

            // D-05: Drain logs to EOF.
            let _ = log_handle.await;
            sender.close();

            ExecResult {
                exit_code: None,
                status: RunStatus::Shutdown,
                error_message: Some("cancelled due to shutdown".to_string()),
            }
        }
    };

    // DOCKER-06: Explicitly remove the container after all state is captured.
    cleanup_container(docker, &container_id).await;

    DockerExecResult {
        exec: exec_result,
        image_digest: Some(image_digest),
    }
}

/// Remove a container with force=true. Logs a warning on failure but does not propagate.
async fn cleanup_container(docker: &Docker, container_id: &str) {
    if let Err(e) = docker
        .remove_container(
            container_id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await
    {
        tracing::warn!(
            target: "cronduit.docker",
            container_id = %container_id,
            error = %e,
            "failed to remove container"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_job_config_deserialize() {
        let json = r#"{
            "image": "alpine:latest",
            "env": {"FOO": "bar", "BAZ": "qux"},
            "volumes": ["/host:/container:ro"],
            "network": "container:vpn",
            "container_name": "my-job"
        }"#;
        let config: DockerJobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.image, "alpine:latest");
        assert_eq!(config.env.get("FOO").unwrap(), "bar");
        assert_eq!(config.env.get("BAZ").unwrap(), "qux");
        assert_eq!(config.volumes.as_ref().unwrap().len(), 1);
        assert_eq!(config.volumes.as_ref().unwrap()[0], "/host:/container:ro");
        assert_eq!(config.network.as_ref().unwrap(), "container:vpn");
        assert_eq!(config.container_name.as_ref().unwrap(), "my-job");
    }

    #[test]
    fn test_docker_job_config_defaults() {
        let json = r#"{"image": "nginx:1.25"}"#;
        let config: DockerJobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.image, "nginx:1.25");
        assert!(config.env.is_empty());
        assert!(config.cmd.is_none());
        assert!(config.volumes.is_none());
        assert!(config.network.is_none());
        assert!(config.container_name.is_none());
    }

    #[test]
    fn test_docker_job_config_with_cmd() {
        let json = r#"{"image": "alpine:latest", "cmd": ["echo", "hello"]}"#;
        let config: DockerJobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.cmd.as_ref().unwrap(), &vec!["echo".to_string(), "hello".to_string()]);
    }

    #[test]
    fn test_docker_job_config_missing_image_fails() {
        let json = r#"{"network": "host"}"#;
        let result = serde_json::from_str::<DockerJobConfig>(json);
        assert!(result.is_err(), "image is required");
    }

    #[test]
    fn test_docker_exec_result_debug() {
        let result = DockerExecResult {
            exec: ExecResult {
                exit_code: Some(0),
                status: RunStatus::Success,
                error_message: None,
            },
            image_digest: Some("sha256:abc123".to_string()),
        };
        // Verify Debug trait works
        let debug_str = format!("{result:?}");
        assert!(debug_str.contains("Success"));
        assert!(debug_str.contains("sha256:abc123"));
    }
}
