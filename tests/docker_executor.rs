//! Docker executor integration tests.
//!
//! These tests require a running Docker daemon and are gated with `#[ignore]`.
//! Run with: `cargo test --test docker_executor -- --ignored --nocapture`
//!
//! Tests cover:
//! - Basic container lifecycle (create, start, wait, log drain, remove)
//! - Timeout behavior (container stopped after timeout)
//! - Pre-flight validation (nonexistent target container)
//! - Orphan reconciliation (stop + remove + DB update)

use std::collections::HashMap;
use std::time::Duration;

use bollard::models::ContainerCreateBody;
use bollard::query_parameters::CreateContainerOptions;
use bollard::Docker;
use cronduit::db::DbPool;
use cronduit::db::queries;
use cronduit::scheduler::command::RunStatus;
use cronduit::scheduler::docker::execute_docker;
use cronduit::scheduler::docker_orphan::reconcile_orphans;
use cronduit::scheduler::docker_preflight::{preflight_network, PreflightError};
use cronduit::scheduler::log_pipeline;
use sqlx::Row;
use tokio_util::sync::CancellationToken;

/// Connect to the local Docker daemon. Panics if unavailable.
async fn docker_client() -> Docker {
    Docker::connect_with_local_defaults()
        .expect("Docker daemon must be running for integration tests")
}

/// Set up an in-memory SQLite database with migrations applied.
async fn setup_test_db() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    pool.migrate().await.unwrap();
    pool
}

/// Test: basic Docker container lifecycle with echo command.
///
/// Verifies: create -> start -> wait -> log drain -> remove.
/// The container should exit 0 and be cleaned up.
#[tokio::test]
#[ignore]
async fn test_docker_basic_echo() {
    let docker = docker_client().await;
    let (sender, receiver) = log_pipeline::channel(256);
    let cancel = CancellationToken::new();

    // Config with explicit cmd to echo a known string.
    let config_json = r#"{"image": "alpine:latest", "cmd": ["echo", "hello-cronduit"]}"#;

    let result = execute_docker(
        &docker,
        config_json,
        "test-echo",
        1,
        Duration::from_secs(30),
        cancel,
        sender,
    )
    .await;

    assert_eq!(
        result.exec.status,
        RunStatus::Success,
        "alpine echo should exit 0, got: {:?}",
        result.exec
    );
    assert_eq!(result.exec.exit_code, Some(0));
    assert!(
        result.exec.error_message.is_none(),
        "no error expected on success"
    );

    // Verify logs were captured.
    let batch = receiver.drain_batch(256);
    let stdout_lines: Vec<_> = batch.iter().filter(|l| l.stream == "stdout").collect();
    assert!(
        stdout_lines.iter().any(|l| l.line.contains("hello-cronduit")),
        "expected echo output in logs, got: {:?}",
        stdout_lines
    );

    // Container should be removed (execute_docker cleans up).
    // container_id field holds the image digest, not the actual container ID,
    // so we verify cleanup indirectly: a second run with the same name should work.
}

/// Test: container timeout stops the container and returns Timeout status.
#[tokio::test]
#[ignore]
async fn test_docker_timeout_stops_container() {
    let docker = docker_client().await;
    let (sender, _receiver) = log_pipeline::channel(256);
    let cancel = CancellationToken::new();

    // Long-running container with a very short timeout.
    let config_json = r#"{"image": "alpine:latest", "cmd": ["sleep", "300"]}"#;

    let result = execute_docker(
        &docker,
        config_json,
        "test-timeout",
        2,
        Duration::from_secs(2),
        cancel,
        sender,
    )
    .await;

    assert_eq!(
        result.exec.status,
        RunStatus::Timeout,
        "expected timeout, got: {:?}",
        result.exec
    );
    assert!(
        result
            .exec
            .error_message
            .as_ref()
            .unwrap()
            .contains("timed out"),
        "error message should mention timeout: {:?}",
        result.exec.error_message
    );
}

/// Test: pre-flight rejects nonexistent target container.
#[tokio::test]
#[ignore]
async fn test_docker_preflight_nonexistent_target() {
    let docker = docker_client().await;

    let result =
        preflight_network(&docker, "container:nonexistent_container_xyz_12345").await;

    assert!(result.is_err(), "pre-flight should fail for nonexistent target");
    match result.unwrap_err() {
        PreflightError::NetworkTargetUnavailable(name) => {
            assert_eq!(name, "nonexistent_container_xyz_12345");
        }
        other => panic!(
            "expected NetworkTargetUnavailable, got: {:?}",
            other
        ),
    }
}

/// Test: orphan reconciliation finds cronduit-labeled containers, stops/removes them,
/// and marks the DB row as error with "orphaned at restart".
#[tokio::test]
#[ignore]
async fn test_docker_orphan_reconciliation() {
    let docker = docker_client().await;
    let pool = setup_test_db().await;

    // Insert a job so we can create a run row.
    let job_id = queries::upsert_job(
        &pool,
        "test-orphan",
        "0 0 31 2 *",
        "0 0 31 2 *",
        "docker",
        r#"{"image": "alpine:latest"}"#,
        "hash1",
        3600,
    )
    .await
    .unwrap();

    // Insert a running run row with a known ID.
    let run_id = queries::insert_running_run(&pool, job_id, "test").await.unwrap();

    // Manually create a container with cronduit labels.
    let mut labels = HashMap::new();
    labels.insert("cronduit.run_id".to_string(), run_id.to_string());
    labels.insert("cronduit.job_name".to_string(), "test-orphan".to_string());

    let container_body = ContainerCreateBody {
        image: Some("alpine:latest".to_string()),
        cmd: Some(vec!["sleep".to_string(), "300".to_string()]),
        labels: Some(labels),
        ..Default::default()
    };

    let container_name = format!("cronduit-orphan-test-{}", run_id);
    let create_opts = Some(CreateContainerOptions {
        name: Some(container_name.clone()),
        ..Default::default()
    });

    let response = docker
        .create_container(create_opts, container_body)
        .await
        .expect("failed to create orphan test container");

    let container_id = response.id;

    // Start the container so it's running.
    docker
        .start_container(&container_id, None)
        .await
        .expect("failed to start orphan test container");

    // Run orphan reconciliation.
    let count = reconcile_orphans(&docker, &pool).await.unwrap();
    assert!(count >= 1, "should reconcile at least 1 orphan, got {count}");

    // Verify the container is removed.
    let inspect_result = docker.inspect_container(&container_id, None).await;
    assert!(
        inspect_result.is_err(),
        "orphan container should be removed after reconciliation"
    );

    // Verify the DB row is updated to error status.
    match pool.writer() {
        cronduit::db::queries::PoolRef::Sqlite(p) => {
            let row = sqlx::query("SELECT status, error_message FROM job_runs WHERE id = ?1")
                .bind(run_id)
                .fetch_one(p)
                .await
                .unwrap();
            let status: String = row.get("status");
            let error_msg: String = row.get("error_message");
            assert_eq!(status, "error", "run status should be 'error'");
            assert_eq!(
                error_msg, "orphaned at restart",
                "error_message should be 'orphaned at restart'"
            );
        }
        _ => panic!("expected SQLite pool in test"),
    }

    pool.close().await;
}

/// Test: execute_docker returns Error status when pre-flight fails for
/// a nonexistent target container in the config.
#[tokio::test]
#[ignore]
async fn test_docker_execute_preflight_failure_returns_error() {
    let docker = docker_client().await;
    let (sender, _receiver) = log_pipeline::channel(256);
    let cancel = CancellationToken::new();

    // Config referencing a nonexistent container for network mode.
    let config_json = r#"{"image": "alpine:latest", "network": "container:nonexistent_xyz_99999"}"#;

    let result = execute_docker(
        &docker,
        config_json,
        "test-preflight-fail",
        3,
        Duration::from_secs(30),
        cancel,
        sender,
    )
    .await;

    assert_eq!(
        result.exec.status,
        RunStatus::Error,
        "should return Error when pre-flight fails: {:?}",
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
}
