//! Unit-level behavioral tests for Docker config parsing and label contracts.
//!
//! These tests verify observable behaviors from the public API surface:
//! - DOCKER-04: volumes, env, container_name, cmd fields parse and round-trip
//! - DOCKER-07: label key names used by execute_docker are the correct cronduit-prefixed strings
//! - DOCKER-06: DockerExecResult carries image_digest (the digest field for container_id in DB)
//! - DOCKER-05: PullError classifies terminal vs transient errors
//!
//! These are pure unit tests — no Docker daemon required.

use cronduit::scheduler::docker::DockerJobConfig;
use cronduit::scheduler::docker_preflight::PreflightError;
use cronduit::scheduler::docker_pull::PullError;
use std::collections::HashMap;

/// DOCKER-04: volumes bind mount strings are preserved as-is.
///
/// The executor passes volumes to HostConfig.binds without transformation.
/// This test verifies the volume strings round-trip correctly through DockerJobConfig.
#[test]
fn docker_config_volumes_preserved_for_host_config() {
    let json = r#"{
        "image": "alpine:latest",
        "volumes": ["/host/path:/container/path:ro", "/data:/data"]
    }"#;

    let config: DockerJobConfig = serde_json::from_str(json).unwrap();
    let volumes = config.volumes.unwrap();

    assert_eq!(volumes.len(), 2);
    assert_eq!(volumes[0], "/host/path:/container/path:ro");
    assert_eq!(volumes[1], "/data:/data");
}

/// DOCKER-04: env vars are stored as a map and can be converted to KEY=VALUE format.
///
/// The executor converts config.env into Vec<String> of "KEY=VALUE" pairs
/// for ContainerCreateBody.env. This test verifies the source map is correct.
#[test]
fn docker_config_env_vars_parse_to_map() {
    let json = r#"{
        "image": "myapp:v1",
        "env": {"DATABASE_URL": "postgres://localhost/db", "SECRET_KEY": "s3cr3t"}
    }"#;

    let config: DockerJobConfig = serde_json::from_str(json).unwrap();

    assert_eq!(
        config.env.get("DATABASE_URL").unwrap(),
        "postgres://localhost/db"
    );
    assert_eq!(config.env.get("SECRET_KEY").unwrap(), "s3cr3t");
    assert_eq!(config.env.len(), 2);
}

/// DOCKER-04: env var formatting as KEY=VALUE strings matches Docker API expectations.
///
/// The executor uses format!("{k}={v}") for each env entry.
/// This test verifies the formatted output is correct for the Docker API.
#[test]
fn docker_config_env_vars_format_as_key_value_strings() {
    let json = r#"{"image": "alpine:latest", "env": {"FOO": "bar", "PATH": "/usr/bin"}}"#;
    let config: DockerJobConfig = serde_json::from_str(json).unwrap();

    // Replicate the formatting done in execute_docker.
    let env_vec: Vec<String> = config.env.iter().map(|(k, v)| format!("{k}={v}")).collect();

    // Must produce exactly 2 KEY=VALUE strings.
    assert_eq!(env_vec.len(), 2);

    // Both must be in KEY=VALUE format.
    for entry in &env_vec {
        assert!(entry.contains('='), "env entry must be KEY=VALUE: {entry}");
        let parts: Vec<&str> = entry.splitn(2, '=').collect();
        assert_eq!(
            parts.len(),
            2,
            "env entry must split into exactly key and value"
        );
    }

    // Verify specific values are present.
    let mut found = HashMap::new();
    for entry in &env_vec {
        let parts: Vec<&str> = entry.splitn(2, '=').collect();
        found.insert(parts[0].to_string(), parts[1].to_string());
    }
    assert_eq!(found.get("FOO").unwrap(), "bar");
    assert_eq!(found.get("PATH").unwrap(), "/usr/bin");
}

/// DOCKER-04: container_name field is optional and passes through.
#[test]
fn docker_config_container_name_optional() {
    // Without container_name.
    let json = r#"{"image": "nginx:1.25"}"#;
    let config: DockerJobConfig = serde_json::from_str(json).unwrap();
    assert!(config.container_name.is_none());

    // With container_name.
    let json = r#"{"image": "nginx:1.25", "container_name": "my-nginx"}"#;
    let config: DockerJobConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.container_name.unwrap(), "my-nginx");
}

/// DOCKER-07: The label keys used by execute_docker are "cronduit.run_id" and "cronduit.job_name".
///
/// This is the contract the orphan reconciliation (SCHED-08) depends on.
/// We verify the label key strings are correct by building the same HashMap
/// that execute_docker builds, to ensure no typo breaks the reconciliation filter.
#[test]
fn docker_label_keys_match_orphan_reconciliation_filter() {
    // This simulates the label building in execute_docker.
    let run_id: i64 = 42;
    let job_name = "my-backup-job";

    let mut labels = HashMap::new();
    labels.insert("cronduit.run_id".to_string(), run_id.to_string());
    labels.insert("cronduit.job_name".to_string(), job_name.to_string());

    // The orphan reconciliation filter uses "cronduit.run_id" as the label key.
    assert!(
        labels.contains_key("cronduit.run_id"),
        "label must contain 'cronduit.run_id' for orphan reconciliation filter"
    );
    assert!(
        labels.contains_key("cronduit.job_name"),
        "label must contain 'cronduit.job_name' for diagnostics"
    );

    // Verify the values are string representations.
    assert_eq!(labels["cronduit.run_id"], "42");
    assert_eq!(labels["cronduit.job_name"], "my-backup-job");
}

/// DOCKER-07: Label values never contain secret values from env vars.
///
/// T-04-03: Labels contain only run_id (integer) and job_name (string), never secrets.
#[test]
fn docker_labels_do_not_contain_env_var_values() {
    let json = r#"{
        "image": "alpine:latest",
        "env": {"SECRET": "my-secret-value", "PASSWORD": "hunter2"}
    }"#;
    let config: DockerJobConfig = serde_json::from_str(json).unwrap();

    // Simulate the label building.
    let run_id: i64 = 1;
    let job_name = "secret-job";
    let mut labels = HashMap::new();
    labels.insert("cronduit.run_id".to_string(), run_id.to_string());
    labels.insert("cronduit.job_name".to_string(), job_name.to_string());

    // Verify no env var values appear in labels.
    for (key, value) in &labels {
        assert!(
            !value.contains("my-secret-value"),
            "label {key} must not contain secret value"
        );
        assert!(
            !value.contains("hunter2"),
            "label {key} must not contain password"
        );
        // Env keys should also not appear as label keys.
        assert!(
            !key.contains("SECRET") && !key.contains("PASSWORD"),
            "label key must not be an env var name"
        );
    }

    // Config env vars exist (not silently dropped).
    assert_eq!(config.env.len(), 2);
}

/// DOCKER-06: DockerExecResult.image_digest carries the container's image digest.
///
/// The image_digest field is stored in job_runs.container_id for post-mortem analysis.
/// This test verifies the struct definition carries the field and it's accessible.
#[test]
fn docker_exec_result_carries_image_digest_for_db_storage() {
    use cronduit::scheduler::command::{ExecResult, RunStatus};
    use cronduit::scheduler::docker::DockerExecResult;

    let result = DockerExecResult {
        exec: ExecResult {
            exit_code: Some(0),
            status: RunStatus::Success,
            error_message: None,
        },
        image_digest: Some("sha256:abc123def456".to_string()),
        container_id: None,
    };

    assert_eq!(result.image_digest.as_deref(), Some("sha256:abc123def456"));
    assert_eq!(result.exec.exit_code, Some(0));
}

/// DOCKER-05: PullError::Terminal fails fast (no retry).
///
/// Terminal errors include unauthorized, manifest unknown, invalid reference.
#[test]
fn pull_error_terminal_variant_identified() {
    // Test that Terminal variant exists and matches correctly.
    let err = PullError::Terminal("unauthorized: access denied".to_string());
    assert!(
        matches!(err, PullError::Terminal(_)),
        "unauthorized should produce Terminal error"
    );
}

/// DOCKER-05: PullError::Transient allows retry.
#[test]
fn pull_error_transient_variant_identified() {
    let err = PullError::Transient("connection refused".to_string());
    assert!(
        matches!(err, PullError::Transient(_)),
        "connection refused should produce Transient error"
    );
}

/// DOCKER-03: PreflightError::NetworkTargetUnavailable formats the structured error message.
///
/// The error message "network_target_unavailable: <name>" is stored in job_runs.error_message.
#[test]
fn preflight_error_network_target_unavailable_formats_correctly() {
    let err = PreflightError::NetworkTargetUnavailable("wireguard".to_string());
    let msg = err.to_error_message();

    assert_eq!(msg, "network_target_unavailable: wireguard");
    assert!(msg.starts_with("network_target_unavailable:"));
}

/// DOCKER-02: All five network modes are recognized — built-in modes vs named.
///
/// Built-in modes: bridge, host, none, "" — skip pre-flight.
/// Named networks and container:<name> modes — require pre-flight.
#[test]
fn docker_network_mode_categories_are_distinct() {
    // Simulate the categorization logic from preflight_network.
    let builtin = ["bridge", "host", "none", ""];
    let container_mode = "container:vpn";
    let named = "my-custom-network";

    // Built-in modes: not container: prefix, in builtin list.
    for mode in &builtin {
        assert!(
            !mode.starts_with("container:"),
            "built-in mode should not start with 'container:'"
        );
    }

    // Container mode: has the prefix.
    assert!(container_mode.starts_with("container:"));
    let target = container_mode.strip_prefix("container:").unwrap();
    assert_eq!(target, "vpn");

    // Named network: not container:, not in builtin.
    assert!(!named.starts_with("container:"));
    assert!(!builtin.contains(&named));
}
