//! Phase 17 / LBL-02 SC-2: use_defaults=false replaces defaults entirely.
//! End-to-end: parse_and_validate -> apply_defaults short-circuit -> serialize ->
//! execute_docker -> bollard -> daemon -> inspect_container.
//!
//! Run: `cargo test --test v12_labels_use_defaults_false -- --ignored --nocapture --test-threads=1`
//!
//! IMPORTANT: --test-threads=1 (project-wide convention for docker tests).
//! Drives end-to-end through `parse_and_validate` so the apply_defaults
//! short-circuit (use_defaults=false) is exercised on every run.

use bollard::Docker;
use cronduit::config::parse_and_validate;
use cronduit::scheduler::command::RunStatus;
use cronduit::scheduler::control::RunControl;
use cronduit::scheduler::docker::execute_docker;
use cronduit::scheduler::log_pipeline;
use cronduit::scheduler::sync::serialize_config_json_for_tests;
use std::io::Write;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

async fn docker_client() -> Docker {
    Docker::connect_with_local_defaults()
        .expect("Docker daemon must be running for integration tests")
}

#[tokio::test]
#[ignore]
async fn use_defaults_false_replaces_defaults_labels_on_container() {
    let docker = docker_client().await;
    let (sender, _receiver) = log_pipeline::channel(64);
    let cancel = CancellationToken::new();
    let control = RunControl::new(cancel.clone());

    let container_name = format!("cronduit-test-labels-replace-{}", std::process::id());

    // Step 1 — TOML with [defaults].labels AND per-job use_defaults=false + per-job labels.
    // After apply_defaults's short-circuit, the merged labels are ONLY
    // {"backup.exclude":"true"} — defaults' watchtower.enable is dropped.
    let toml_text = format!(
        r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[defaults]
image = "alpine:latest"
labels = {{ "watchtower.enable" = "false" }}

[[jobs]]
name = "labels-replace-job"
schedule = "*/5 * * * *"
image = "alpine:latest"
cmd = ["sh", "-c", "exit 0"]
delete = false
container_name = "{container_name}"
use_defaults = false
labels = {{ "backup.exclude" = "true" }}
"#
    );

    // Step 2 — tempfile + parse_and_validate.
    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile created");
    tmp.write_all(toml_text.as_bytes()).expect("toml written");
    let parsed = parse_and_validate(tmp.path()).expect("config parses + validates");

    let job = parsed
        .config
        .jobs
        .iter()
        .find(|j| j.name == "labels-replace-job")
        .expect("job present");
    let job_labels = job.labels.as_ref().expect("per-job labels present");

    // PIN: short-circuit ran (defaults dropped, only per-job present).
    assert_eq!(
        job_labels.get("backup.exclude").map(String::as_str),
        Some("true"),
        "per-job-only label preserved"
    );
    assert!(
        !job_labels.contains_key("watchtower.enable"),
        "use_defaults=false MUST drop defaults labels at the apply_defaults layer"
    );

    // Step 3 — Serialize merged JobConfig and run through bollard.
    let config_json = serialize_config_json_for_tests(job);
    let result = execute_docker(
        &docker,
        &config_json,
        "labels-replace-job",
        43,
        Duration::from_secs(30),
        cancel,
        sender,
        &control,
    )
    .await;

    let container_id = result.container_id.clone().expect("container_id populated");
    assert_eq!(result.exec.status, RunStatus::Success);

    let inspect = docker
        .inspect_container(
            &container_id,
            None::<bollard::query_parameters::InspectContainerOptions>,
        )
        .await
        .expect("inspect succeeds");
    let labels = inspect
        .config
        .as_ref()
        .and_then(|c| c.labels.as_ref())
        .expect("labels present");

    assert_eq!(
        labels.get("backup.exclude").map(String::as_str),
        Some("true"),
        "per-job-only label must reach container"
    );
    assert!(
        !labels.contains_key("watchtower.enable"),
        "use_defaults=false must DROP defaults labels — `watchtower.enable` must NOT be on the container (SC-2)"
    );
    // Cronduit-internal labels still there:
    assert_eq!(
        labels.get("cronduit.run_id").map(String::as_str),
        Some("43")
    );

    let _ = docker
        .remove_container(
            &container_id,
            Some(bollard::query_parameters::RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
}
