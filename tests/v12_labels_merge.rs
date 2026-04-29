//! Phase 17 / LBL-01 / LBL-02: defaults+per-job labels merge round-trip
//! end-to-end through parse_and_validate -> apply_defaults -> serialize ->
//! execute_docker -> bollard -> docker daemon -> inspect_container.
//!
//! Run: `cargo test --test v12_labels_merge -- --ignored --nocapture --test-threads=1`
//!
//! IMPORTANT: --test-threads=1 (project-wide convention for docker tests).
//! This file intentionally goes end-to-end through `parse_and_validate` so
//! the parse / interpolate / apply_defaults / validate / serialize layers
//! are exercised on EVERY run — not bypassed by hand-built `config_json`.

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
async fn defaults_plus_per_job_labels_merge_appears_on_container() {
    let docker = docker_client().await;
    let (sender, _receiver) = log_pipeline::channel(64);
    let cancel = CancellationToken::new();
    let control = RunControl::new(cancel.clone());

    let container_name = format!("cronduit-test-labels-merge-{}", std::process::id());

    // Step 1 — Construct TOML with [defaults].labels + per-job labels.
    let toml_text = format!(
        r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[defaults]
image = "alpine:latest"
labels = {{ "watchtower.enable" = "false" }}

[[jobs]]
name = "labels-merge-job"
schedule = "*/5 * * * *"
image = "alpine:latest"
cmd = ["sh", "-c", "exit 0"]
delete = false
container_name = "{container_name}"
labels = {{ "traefik.enable" = "true", "traefik.http.routers.x.rule" = "Host(`x.local`)" }}
"#
    );

    // Step 2 — Write to tempfile.
    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile created");
    tmp.write_all(toml_text.as_bytes()).expect("toml written");

    // Step 3 — parse_and_validate runs interpolate -> toml -> apply_defaults -> validators.
    let parsed = parse_and_validate(tmp.path()).expect("config parses + validates");

    // Step 4 — Access the MERGED JobConfig (apply_defaults already ran inside parse_and_validate).
    let job = parsed
        .config
        .jobs
        .iter()
        .find(|j| j.name == "labels-merge-job")
        .expect("job present after parse");
    let job_labels = job.labels.as_ref().expect("merged labels present");

    // PIN: merge ran (defaults.watchtower.enable + per-job traefik.* present).
    assert_eq!(
        job_labels.get("watchtower.enable").map(String::as_str),
        Some("false"),
        "defaults label inherited via apply_defaults"
    );
    assert_eq!(
        job_labels.get("traefik.enable").map(String::as_str),
        Some("true"),
        "per-job label retained after merge"
    );
    assert_eq!(
        job_labels
            .get("traefik.http.routers.x.rule")
            .map(String::as_str),
        Some("Host(`x.local`)"),
        "per-job label with backticks survives merge"
    );

    // Step 5 — Serialize merged JobConfig via the canonical serializer (BLOCKER #4 preferred path).
    let config_json = serialize_config_json_for_tests(job);

    // Step 6 — execute_docker -> bollard -> daemon.
    let result = execute_docker(
        &docker,
        &config_json,
        "labels-merge-job",
        42,
        Duration::from_secs(30),
        cancel,
        sender,
        &control,
    )
    .await;

    let container_id = result
        .container_id
        .clone()
        .expect("container_id populated by execute_docker");

    // Container exited 0 (so the test isn't asserting against a failed run).
    assert_eq!(
        result.exec.status,
        RunStatus::Success,
        "container should exit 0 to make assertion meaningful: {:?}",
        result.exec
    );

    // Step 7 — Inspect the container — bollard 0.20.2 API.
    let inspect = docker
        .inspect_container(
            &container_id,
            None::<bollard::query_parameters::InspectContainerOptions>,
        )
        .await
        .expect("inspect_container succeeds");
    let labels = inspect
        .config
        .as_ref()
        .and_then(|c| c.labels.as_ref())
        .expect("container has labels");

    // Operator labels present (end-to-end through parse_and_validate -> merge -> bollard):
    assert_eq!(
        labels.get("watchtower.enable").map(String::as_str),
        Some("false"),
        "defaults label `watchtower.enable=false` must reach container (Layer 4 -> Layer 6)"
    );
    assert_eq!(
        labels.get("traefik.enable").map(String::as_str),
        Some("true"),
        "per-job label `traefik.enable=true` must reach container"
    );
    assert_eq!(
        labels
            .get("traefik.http.routers.x.rule")
            .map(String::as_str),
        Some("Host(`x.local`)"),
        "per-job label with backticks must round-trip unmodified through TOML -> JSON -> bollard"
    );

    // Cronduit-internal labels still present (defense-in-depth ordering from Task 1):
    assert_eq!(
        labels.get("cronduit.run_id").map(String::as_str),
        Some("42"),
        "cronduit-internal run_id must remain intact (LBL-01 / SC-1)"
    );
    assert_eq!(
        labels.get("cronduit.job_name").map(String::as_str),
        Some("labels-merge-job"),
        "cronduit-internal job_name must remain intact"
    );

    // Cleanup — best-effort.
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
