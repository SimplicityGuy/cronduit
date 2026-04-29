//! Phase 17 / LBL-02 SC-2: use_defaults=false replaces defaults entirely.
//! End-to-end: parse_and_validate -> apply_defaults short-circuit -> serialize ->
//! execute_docker -> bollard -> daemon -> inspect_container.
//!
//! Run: `cargo test --test v12_labels_use_defaults_false -- --ignored --nocapture --test-threads=1`
//!
//! IMPORTANT: --test-threads=1 (project-wide convention for docker tests).
//! Drives end-to-end through `parse_and_validate` so the apply_defaults
//! short-circuit (use_defaults=false) is exercised on every run.
//!
//! Plan 17-08 (gap closure for CR-02) extends this file with a
//! defaults-only-on-command-job regression test pinning the new Branch B
//! error message ("set `use_defaults = false` ..."). That test is
//! NOT `#[ignore]` — it exercises only the parse pipeline (no Docker
//! daemon required).

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

/// CR-02 regression test (gap closure plan 17-08).
///
/// Pins the binary's behavior for the defaults-only-on-command-job case:
/// when `[defaults].labels` is set and a command job has no
/// `use_defaults = false` and no per-job labels block, `apply_defaults`
/// merges defaults.labels into the command job, and the LBL-04 validator
/// fires Branch B with the new remediation text. The legacy "Remove the
/// `labels` block" phrase MUST NOT appear (the operator never wrote a
/// labels block).
///
/// This test does NOT require a Docker daemon — it only exercises the
/// parse pipeline (interpolate -> toml -> apply_defaults -> validate).
#[tokio::test]
async fn lbl_04_defaults_only_command_job_emits_use_defaults_false_remediation() {
    let toml_text = r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[defaults]
image = "alpine:latest"
labels = { "watchtower.enable" = "false" }

[[jobs]]
name = "lbl-04-bare-command"
schedule = "*/5 * * * *"
command = "echo hi"
timeout = "5m"
"#;

    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile created");
    tmp.write_all(toml_text.as_bytes()).expect("toml written");

    let result = parse_and_validate(tmp.path());
    let errors = result.expect_err(
        "defaults-only-on-command-job MUST fail at config-LOAD with the LBL-04 \
         type-gate validator firing on the merged labels",
    );

    let combined = errors
        .iter()
        .map(|e| e.message.clone())
        .collect::<Vec<_>>()
        .join(" || ");

    // Branch B remediation phrase is present.
    assert!(
        combined.contains("set `use_defaults = false`"),
        "expected new remediation phrase 'set `use_defaults = false`'; got: {combined}"
    );

    // Branch A legacy phrase is ABSENT (operator never wrote a labels block).
    assert!(
        !combined.contains("Remove the `labels` block"),
        "Branch A legacy phrase must not appear in the defaults-only case; got: {combined}"
    );

    // Job name is present so operator knows which job to fix.
    assert!(
        combined.contains("lbl-04-bare-command"),
        "error must name the offending job; got: {combined}"
    );

    // Defaults key MUST NOT leak into the error message (set-diff
    // hides it; this is the contract pinned by
    // `lbl_04_error_does_not_leak_defaults_keys_for_non_docker_jobs`
    // in src/config/defaults.rs:447-509).
    assert!(
        !combined.contains("watchtower.enable"),
        "defaults key must not leak into the error message; got: {combined}"
    );
}
