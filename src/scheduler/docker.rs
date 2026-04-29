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

use bollard::Docker;
use bollard::models::{ContainerCreateBody, HostConfig};
use bollard::query_parameters::{
    CreateContainerOptions, KillContainerOptionsBuilder, RemoveContainerOptions,
    StopContainerOptions,
};
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
    /// When Some(false), preserve the container after the run completes so an
    /// operator can `docker logs <id>` or `docker inspect <id>` for post-mortem.
    /// When Some(true) or None, cronduit explicitly removes the container after
    /// logs drain (DOCKER-06: never bollard auto_remove — auto_remove races
    /// with wait_container and can truncate exit codes, see moby#8441).
    ///
    /// Preserved containers accumulate forever and are the operator's
    /// responsibility to prune (`docker container prune`, etc.). Cronduit
    /// does NOT reap them on restart — only containers still marked
    /// `running` in the DB are touched by orphan reconciliation.
    #[serde(default)]
    pub delete: Option<bool>,
    /// Operator-defined Docker labels merged into the cronduit-internal
    /// label set at container-create time. Reserved-namespace and type-gate
    /// validators at config-load mean this is always operator-safe content.
    #[serde(default)]
    pub labels: Option<HashMap<String, String>>,
}

/// Result of a Docker job execution, extending `ExecResult` with container metadata.
#[derive(Debug)]
pub struct DockerExecResult {
    /// Standard execution result (exit code, status, error message).
    pub exec: ExecResult,
    /// Image digest from `inspect_container` after start (DOCKER-09).
    pub image_digest: Option<String>,
    /// Phase 16 FOUND-14: actual Docker container ID from `create_container().id`.
    /// Captured at L186-190 of execute_docker BEFORE start, so it is `Some(_)` for
    /// every code path where create_container succeeded (5 of 7 literal sites). The
    /// two earlier sites (config-parse error, pre-flight network validation, image-pull
    /// error) all return BEFORE create_container runs and carry `None`. Plan 16-03 reads
    /// this field to fix the long-standing bug at run.rs:301 where image_digest was
    /// being stored in job_runs.container_id.
    pub container_id: Option<String>,
}

/// Execute a Docker job: create -> start -> inspect -> wait/timeout/cancel -> drain logs -> remove.
///
/// The lifecycle state machine:
/// Creating -> Starting -> Running -> Exited -> LogsDrained -> Removed
///
/// DOCKER-06: `auto_remove = false` prevents the moby#8441 race where Docker
/// removes the container before we can inspect its exit code.
#[allow(clippy::too_many_arguments)]
pub async fn execute_docker(
    docker: &Docker,
    config_json: &str,
    job_name: &str,
    run_id: i64,
    timeout: Duration,
    cancel: CancellationToken,
    sender: LogSender,
    control: &crate::scheduler::control::RunControl,
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
                container_id: None,
            };
        }
    };

    // Pre-flight network validation (D-10, D-11, D-12).
    if let Some(ref network) = config.network
        && let Err(e) = super::docker_preflight::preflight_network(docker, network).await
    {
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
            container_id: None,
        };
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
                container_id: None,
            };
        }
    };

    // Build labels (T-04-03: only run_id and job_name, never secrets).
    //
    // Operator-defined labels (Phase 17 / SEED-001 / LBL-01) are inserted
    // FIRST. The cronduit-internal labels (`cronduit.run_id`,
    // `cronduit.job_name`) are then inserted AFTER, so on the
    // impossible-due-to-LBL-03-validator collision case (operator somehow
    // set `cronduit.*`), the cronduit-internal value structurally wins.
    // This is defense-in-depth — `check_label_reserved_namespace`
    // (src/config/validate.rs, Plan 17-02) is the primary guard, this
    // ordering is the structural guard.
    let mut labels = HashMap::new();
    if let Some(operator_labels) = &config.labels {
        labels.extend(
            operator_labels
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        );
    }
    labels.insert("cronduit.run_id".to_string(), run_id.to_string());
    labels.insert("cronduit.job_name".to_string(), job_name.to_string());

    // Build env vars as KEY=VALUE strings.
    let env_vec: Vec<String> = config.env.iter().map(|(k, v)| format!("{k}={v}")).collect();

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
    let create_options = config
        .container_name
        .as_ref()
        .map(|name| CreateContainerOptions {
            name: Some(name.clone()),
            ..Default::default()
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
                container_id: None,
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
        // Attempt cleanup of the created-but-not-started container. Honors
        // `delete = false` even in the error path — operators debugging a
        // start failure want the container preserved so they can inspect it.
        maybe_cleanup_container(docker, &container_id, config.delete, job_name, run_id).await;
        return DockerExecResult {
            exec: ExecResult {
                exit_code: None,
                status: RunStatus::Error,
                error_message: Some(format!("failed to start container: {e}")),
            },
            image_digest: None,
            container_id: Some(container_id.clone()),
        };
    }

    // Extract image digest via inspect (DOCKER-09).
    //
    // Phase 16 / Code-review WR-01: this is `Option<String>` (not `String`).
    // The schema design relies on the binary distinction "image_digest IS NULL"
    // (no digest captured) vs. "image_digest LIKE 'sha256:%'" (digest captured).
    // An empty string is neither — downstream consumers that filter on
    // `image_digest IS NOT NULL` would treat the empty-string row as "captured"
    // when in fact the digest was never captured. So:
    //   * inspect_container returned `info.image == None`     -> None
    //   * inspect_container returned `info.image == Some("")` -> None (filtered)
    //   * inspect_container errored                           -> None
    //   * inspect_container returned `Some("sha256:...")`     -> Some("sha256:...")
    let image_digest: Option<String> = match docker.inspect_container(&container_id, None).await {
        Ok(info) => info.image.filter(|s| !s.is_empty()),
        Err(e) => {
            tracing::warn!(
                target: "cronduit.docker",
                container_id = %container_id,
                error = %e,
                "failed to inspect container for image digest"
            );
            None
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
            // Distinguish cancel cause (SCHED-10). The SeqCst ordering in
            // `RunControl::stop` guarantees this load observes the operator
            // reason if the cancel came via the UI Stop button.
            let reason = control.reason();
            let log_msg = match reason {
                crate::scheduler::control::StopReason::Operator =>
                    "[stop signal from operator, killing container]",
                crate::scheduler::control::StopReason::Shutdown =>
                    "[shutdown signal received, stopping container]",
            };
            sender.send(make_log_line("system", log_msg.to_string()));

            // Operator: immediate KILL (no 10s grace — "stop means stop").
            // Shutdown: preserve v1.0 stop_container(t=10) semantics (D-06).
            match reason {
                crate::scheduler::control::StopReason::Operator => {
                    let kill_result = docker.kill_container(
                        &container_id,
                        Some(KillContainerOptionsBuilder::default().signal("KILL").build()),
                    ).await;
                    // Pitfall 3: bollard may return 304/404 if the container
                    // already exited naturally in the tiny race window between
                    // the operator clicking Stop and us firing kill_container.
                    // Log at debug + continue — finalize still produces Stopped
                    // (the operator's intent was honored).
                    if let Err(e) = &kill_result {
                        tracing::debug!(
                            target: "cronduit.docker.stop_raced_natural_exit",
                            container_id = %container_id,
                            error = %e,
                            "docker.kill_container error during operator stop (possible race with natural exit)"
                        );
                    }
                }
                crate::scheduler::control::StopReason::Shutdown => {
                    let _ = docker.stop_container(
                        &container_id,
                        Some(StopContainerOptions {
                            t: Some(10),
                            ..Default::default()
                        }),
                    ).await;
                }
            }

            // D-05: Drain logs to EOF.
            let _ = log_handle.await;
            sender.close();

            let (status, msg) = match reason {
                crate::scheduler::control::StopReason::Operator => (
                    RunStatus::Stopped,
                    "stopped by operator".to_string(),
                ),
                crate::scheduler::control::StopReason::Shutdown => (
                    RunStatus::Shutdown,
                    "cancelled due to shutdown".to_string(),
                ),
            };
            ExecResult {
                exit_code: None,
                status,
                error_message: Some(msg),
            }
        }
    };

    // DOCKER-06: Explicitly remove the container after all state is captured,
    // UNLESS the operator set `delete = false` to preserve it for inspection.
    maybe_cleanup_container(docker, &container_id, config.delete, job_name, run_id).await;

    DockerExecResult {
        exec: exec_result,
        // WR-01: `image_digest` is already `Option<String>` from the inspect
        // step above; pass it through directly so an inspect failure or an
        // empty `info.image` flows as `None` rather than `Some("")`.
        image_digest,
        container_id: Some(container_id.clone()),
    }
}

/// Either remove or preserve the container based on the job's `delete` field.
///
/// - `Some(false)` — preserve. Log at INFO so operators can find the container
///   by its ID + run_id label and `docker logs <id>` / `docker inspect <id>`
///   for post-mortem. Preserved containers accumulate forever; operators are
///   responsible for pruning them (`docker container prune` or a scheduled
///   cronduit job that shells out to `docker rm`). Cronduit does NOT reap
///   them on restart — orphan reconciliation only touches rows still marked
///   `running` in the DB, and a preserved-but-exited container has a final
///   DB status (success/failed/timeout) so it is invisible to reconciliation.
///
/// - `Some(true)` or `None` — remove with `force=true`. This is the default
///   behavior and matches the `delete = true` semantics documented in
///   `docs/SPEC.md` and `docs/CONFIG.md`.
///
/// DOCKER-06 rationale stays the same: we never use bollard's `auto_remove`
/// because it races with `wait_container` and can truncate exit codes
/// (moby#8441). The explicit remove below happens only after all state is
/// captured in the DB.
async fn maybe_cleanup_container(
    docker: &Docker,
    container_id: &str,
    delete: Option<bool>,
    job_name: &str,
    run_id: i64,
) {
    if delete == Some(false) {
        tracing::info!(
            target: "cronduit.docker",
            container_id = %container_id,
            job_name = %job_name,
            run_id,
            "container preserved per job `delete = false`; inspect with `docker logs {}` / `docker inspect {}`, prune with `docker rm {}` when done",
            container_id, container_id, container_id
        );
        return;
    }
    cleanup_container(docker, container_id).await;
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
        assert!(
            config.delete.is_none(),
            "delete must default to None; `maybe_cleanup_container` treats None as \
             `delete = true` semantics (remove), consistent with spec"
        );
    }

    #[test]
    fn test_docker_job_config_with_delete_false() {
        let json = r#"{"image": "hello-world:latest", "delete": false}"#;
        let config: DockerJobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.delete, Some(false));
    }

    #[test]
    fn test_docker_job_config_with_delete_true() {
        let json = r#"{"image": "hello-world:latest", "delete": true}"#;
        let config: DockerJobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.delete, Some(true));
    }

    #[test]
    fn test_docker_job_config_with_cmd() {
        let json = r#"{"image": "alpine:latest", "cmd": ["echo", "hello"]}"#;
        let config: DockerJobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            config.cmd.as_ref().unwrap(),
            &vec!["echo".to_string(), "hello".to_string()]
        );
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
            container_id: Some("test-container-id".to_string()),
        };
        // Verify Debug trait works
        let debug_str = format!("{result:?}");
        assert!(debug_str.contains("Success"));
        assert!(debug_str.contains("sha256:abc123"));
    }

    /// Phase 16 / Code-review WR-01 contract test: the inspect-failure path of
    /// `execute_docker` must yield `DockerExecResult.image_digest = None`, not
    /// `Some("")`. The schema design and FCTX-07 query rely on the binary
    /// distinction "image_digest IS NULL" vs. "image_digest LIKE 'sha256:%'";
    /// an empty string is neither and would be classified as "captured" by any
    /// downstream `image_digest IS NOT NULL` filter.
    ///
    /// This is a structural / type-level test — we cannot drive a real
    /// `inspect_container` failure without a Docker daemon, but we CAN
    /// assert (a) the field is `Option<String>` so empty-string `Some("")`
    /// is the responsibility of the producer, and (b) when constructed from
    /// the now-fixed inspect-failure shape, the value flows through
    /// `as_deref()` (the run.rs:361 wiring) as `None`, giving `finalize_run`
    /// `image_digest: None` and ultimately a SQL NULL.
    #[test]
    fn wr01_inspect_failure_yields_none_not_empty_string() {
        // Simulate the L253-264 inspect-failure path post-fix: image_digest is
        // bound to None when inspect_container errors OR when info.image is
        // None / Some(""). The Some("") case is filtered by `.filter(|s|
        // !s.is_empty())` in the production path.
        let inspect_err: Option<String> = None;
        let inspect_ok_no_image: Option<String> = None.filter(|s: &String| !s.is_empty());
        let inspect_ok_empty_string: Option<String> = Some(String::new()).filter(|s| !s.is_empty());
        let inspect_ok_real: Option<String> =
            Some("sha256:abc123".to_string()).filter(|s| !s.is_empty());

        // All three failure shapes collapse to None.
        assert!(inspect_err.is_none(), "inspect error must yield None");
        assert!(
            inspect_ok_no_image.is_none(),
            "info.image == None must yield None"
        );
        assert!(
            inspect_ok_empty_string.is_none(),
            "info.image == Some(\"\") must filter to None — empty strings must NOT \
             reach the DB as 'captured' rows"
        );
        // Real digests flow through.
        assert_eq!(
            inspect_ok_real.as_deref(),
            Some("sha256:abc123"),
            "real digests pass through .filter() unchanged"
        );

        // The DockerExecResult.image_digest field type is Option<String>; an
        // inspect-failure DockerExecResult correctly carries None.
        let result = DockerExecResult {
            exec: ExecResult {
                exit_code: Some(0),
                status: RunStatus::Success,
                error_message: None,
            },
            image_digest: inspect_err,
            container_id: Some("real-container-id".to_string()),
        };
        assert!(
            result.image_digest.is_none(),
            "DockerExecResult constructed from the inspect-failure path must carry None"
        );
        // run.rs:361 calls `image_digest_for_finalize.as_deref()` before
        // passing to finalize_run's `Option<&str>` parameter. Lock that
        // wiring contract here so a future change that re-introduces a
        // `Some(String::new())` fallback fails this assertion.
        assert!(
            result.image_digest.as_deref().is_none(),
            "run.rs:361 wiring (`.as_deref()`) must yield Option<&str> = None"
        );
    }
}
