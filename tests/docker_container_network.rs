//! DOCKER-10: Integration test for network = "container:<name>" mode.
//!
//! This is the marquee differentiator feature for Cronduit. The test proves
//! the complete path works: target container start -> pre-flight validation ->
//! container creation with correct network_mode -> execution -> log capture -> cleanup.
//!
//! Run with: `cargo test --test docker_container_network -- --ignored --nocapture`

use std::time::Duration;

use bollard::Docker;
use cronduit::scheduler::command::RunStatus;
use cronduit::scheduler::docker::execute_docker;
use cronduit::scheduler::docker_preflight::{PreflightError, preflight_network};
use cronduit::scheduler::log_pipeline;
use testcontainers::runners::AsyncRunner;
use testcontainers::{GenericImage, ImageExt};
use tokio_util::sync::CancellationToken;

/// Connect to the local Docker daemon. Panics if unavailable.
async fn docker_client() -> Docker {
    Docker::connect_with_local_defaults()
        .expect("Docker daemon must be running for integration tests")
}

/// DOCKER-10 Marquee Test: container:<name> network mode end-to-end.
///
/// 1. Start a "target" container via testcontainers (alpine sleep).
/// 2. Run a Cronduit Docker job with network = "container:<target_id>".
/// 3. Verify execute_docker succeeds (pre-flight passes, container starts
///    in the target's network namespace, exits 0).
/// 4. testcontainers handles target cleanup automatically.
#[tokio::test]
#[ignore]
async fn test_container_network_mode() {
    // Start a long-running target container that we'll attach to.
    let target = GenericImage::new("alpine", "latest")
        .with_entrypoint("sleep")
        .with_cmd(vec!["300"])
        .start()
        .await
        .expect("failed to start target container via testcontainers");

    let target_id = target.id().to_string();
    eprintln!("[marquee test] target container ID: {target_id}");

    // Verify the target is running (pre-flight should pass).
    let docker = docker_client().await;
    let preflight_result = preflight_network(&docker, &format!("container:{target_id}")).await;
    assert!(
        preflight_result.is_ok(),
        "pre-flight should pass for running target: {:?}",
        preflight_result.err()
    );

    // Build config_json with container:<target_id> network mode.
    // The job container runs `echo ok` to prove it can start in the target's
    // network namespace and execute successfully.
    let config_json = format!(
        r#"{{"image": "alpine:latest", "cmd": ["echo", "network-ok"], "network": "container:{target_id}"}}"#
    );

    let (sender, receiver) = log_pipeline::channel(256);
    let cancel = CancellationToken::new();

    let result = execute_docker(
        &docker,
        &config_json,
        "marquee-container-network",
        100,
        Duration::from_secs(30),
        cancel,
        sender,
    )
    .await;

    eprintln!("[marquee test] result: {:?}", result.exec);

    assert_eq!(
        result.exec.status,
        RunStatus::Success,
        "container:<name> job should succeed (exit 0), got: {:?}",
        result.exec
    );
    assert_eq!(result.exec.exit_code, Some(0));

    // Verify log capture worked.
    let batch = receiver.drain_batch(256);
    let stdout_lines: Vec<_> = batch.iter().filter(|l| l.stream == "stdout").collect();
    assert!(
        stdout_lines.iter().any(|l| l.line.contains("network-ok")),
        "expected 'network-ok' in logs, got: {:?}",
        stdout_lines
    );

    // Target container cleanup is handled by testcontainers on drop.
    eprintln!("[marquee test] PASSED: container:<name> network mode works end-to-end");
}

/// Test: pre-flight fails when the target container is stopped.
///
/// Starts a target, stops it, then verifies that pre-flight rejects it
/// with NetworkTargetUnavailable.
#[tokio::test]
#[ignore]
async fn test_container_network_target_stopped() {
    let docker = docker_client().await;

    // Start a target container.
    let target = GenericImage::new("alpine", "latest")
        .with_entrypoint("sleep")
        .with_cmd(vec!["300"])
        .start()
        .await
        .expect("failed to start target container");

    let target_id = target.id().to_string();
    eprintln!("[stopped-target test] target container ID: {target_id}");

    // Stop the target container.
    docker
        .stop_container(
            &target_id,
            Some(bollard::query_parameters::StopContainerOptions {
                t: Some(1),
                ..Default::default()
            }),
        )
        .await
        .expect("failed to stop target container");

    // Pre-flight should now fail because the target is stopped.
    let result = preflight_network(&docker, &format!("container:{target_id}")).await;
    assert!(result.is_err(), "pre-flight should fail for stopped target");

    match result.unwrap_err() {
        PreflightError::NetworkTargetUnavailable(name) => {
            assert_eq!(name, target_id, "error should reference the target ID");
        }
        other => panic!("expected NetworkTargetUnavailable, got: {:?}", other),
    }

    // Also verify that execute_docker returns Error status through the full path.
    let config_json =
        format!(r#"{{"image": "alpine:latest", "network": "container:{target_id}"}}"#);

    let (sender, _receiver) = log_pipeline::channel(256);
    let cancel = CancellationToken::new();

    let result = execute_docker(
        &docker,
        &config_json,
        "test-stopped-target",
        101,
        Duration::from_secs(30),
        cancel,
        sender,
    )
    .await;

    assert_eq!(
        result.exec.status,
        RunStatus::Error,
        "execute_docker should return Error for stopped target: {:?}",
        result.exec
    );
    assert!(
        result
            .exec
            .error_message
            .as_ref()
            .unwrap()
            .contains("network_target_unavailable"),
        "error should mention network_target_unavailable: {:?}",
        result.exec.error_message
    );

    eprintln!("[stopped-target test] PASSED: pre-flight correctly rejects stopped target");
}
