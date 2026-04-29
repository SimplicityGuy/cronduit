//! Phase 17 / LBL-05 / SC-5a: ${VAR} env-var interpolation in label VALUES
//! resolves at config-LOAD; resolved value reaches the container via bollard.
//! Keys are NEVER interpolated (rejected by D-02 strict char regex if any
//! ${ leftover, see check_label_key_chars in src/config/validate.rs).
//!
//! Run: `cargo test --test v12_labels_interpolation -- --ignored --nocapture --test-threads=1`
//!
//! IMPORTANT: --test-threads=1 is critical for THIS test file specifically:
//! it mutates a process-global env var (`DEPLOYMENT_ID`). Parallel test
//! threads in the same process would race on the env-var read inside
//! `parse_and_validate`'s pre-parse interpolation pass. Project-wide
//! convention already runs docker tests with --test-threads=1.
//!
//! Plan 17-07 (gap closure for CR-01) extends this file with two
//! KEY-position tests pinning the post-CR-01 documentation contract:
//!   * lbl_05_key_position_interpolation_env_set_resolves_to_literal_when_pattern_matches
//!   * lbl_05_key_position_interpolation_env_unset_caught_by_strict_chars
//!
//! These tests do NOT require a Docker daemon — they exercise only the
//! parse pipeline (interpolate -> toml -> apply_defaults -> validate).

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
async fn label_value_var_interpolated_at_load_reaches_container() {
    let docker = docker_client().await;
    let (sender, _receiver) = log_pipeline::channel(64);
    let cancel = CancellationToken::new();
    let control = RunControl::new(cancel.clone());

    // Set the env var BEFORE parse_and_validate so the pre-parse
    // interpolation pass (src/config/interpolate.rs:22) resolves it.
    // SAFETY: set_var is `unsafe` in Rust 1.85+ (Edition 2024). Documented
    // safe in single-threaded test setup contexts; this file runs under
    // --test-threads=1 (see file-level doc comment).
    unsafe {
        std::env::set_var("DEPLOYMENT_ID", "12345");
    }

    let container_name = format!("cronduit-test-labels-interp-{}", std::process::id());

    let toml_text = format!(
        r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[defaults]
image = "alpine:latest"

[[jobs]]
name = "labels-interp-job"
schedule = "*/5 * * * *"
image = "alpine:latest"
cmd = ["sh", "-c", "exit 0"]
delete = false
container_name = "{container_name}"
labels = {{ "deployment.id" = "${{DEPLOYMENT_ID}}" }}
"#
    );

    // Step 1a — Write TOML to tempfile (parse_and_validate takes &Path).
    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile created");
    tmp.write_all(toml_text.as_bytes()).expect("toml written");

    // Step 1b — parse via the FULL pipeline (interpolate -> toml -> apply_defaults -> validate).
    let parsed = parse_and_validate(tmp.path()).expect("config parses + validates");
    let job = parsed
        .config
        .jobs
        .iter()
        .find(|j| j.name == "labels-interp-job")
        .expect("job present");
    // PIN: the JobConfig labels map already has the RESOLVED value
    // (not the literal "${DEPLOYMENT_ID}"). This is the LBL-05 contract.
    assert_eq!(
        job.labels
            .as_ref()
            .and_then(|m| m.get("deployment.id"))
            .map(String::as_str),
        Some("12345"),
        "LBL-05: ${{DEPLOYMENT_ID}} in label VALUE must be interpolated at config-LOAD"
    );

    // Step 2 — serialize merged JobConfig and run through bollard.
    let config_json = serialize_config_json_for_tests(job);
    let result = execute_docker(
        &docker,
        &config_json,
        "labels-interp-job",
        44,
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

    // Resolved value reaches the container:
    assert_eq!(
        labels.get("deployment.id").map(String::as_str),
        Some("12345"),
        "LBL-05: resolved env-var value must reach the container"
    );
    // Negative case: literal `${DEPLOYMENT_ID}` MUST NOT be on the container.
    assert!(
        !labels.values().any(|v| v.contains("${DEPLOYMENT_ID}")),
        "literal ${{...}} placeholder must not survive the load pipeline"
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

    // SAFETY: same single-threaded justification as the set_var above.
    unsafe {
        std::env::remove_var("DEPLOYMENT_ID");
    }
}

/// CR-01 regression test (gap closure plan 17-07).
///
/// Pins the documented behavior: when an env var named in a LABEL KEY
/// position is SET in the environment, the whole-file textual interpolation
/// pass at `src/config/interpolate.rs::interpolate` resolves it BEFORE TOML
/// parsing. The resolved key is then validated by `check_label_key_chars`
/// (D-02). If the resolved value matches the strict pattern, the load
/// succeeds — by design, per the post-CR-01 README contract.
///
/// This test does NOT require a Docker daemon — it only exercises the
/// parse pipeline (interpolate -> toml -> apply_defaults -> validate).
#[tokio::test]
async fn lbl_05_key_position_interpolation_env_set_resolves_to_literal_when_pattern_matches() {
    // SAFETY: same single-threaded justification as the value-side test;
    // file runs under --test-threads=1.
    unsafe {
        std::env::set_var("TEAM", "ops");
    }

    let toml_text = r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[defaults]
image = "alpine:latest"

[[jobs]]
name = "lbl-05-key-set"
schedule = "*/5 * * * *"
image = "alpine:latest"
cmd = ["sh", "-c", "exit 0"]
labels = { "${TEAM}" = "v" }
"#;

    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile created");
    tmp.write_all(toml_text.as_bytes()).expect("toml written");

    let parsed = parse_and_validate(tmp.path())
        .expect("env-set key-position interpolation must succeed (resolved key matches D-02)");

    let job = parsed
        .config
        .jobs
        .iter()
        .find(|j| j.name == "lbl-05-key-set")
        .expect("job present");

    // The resolved key is the LITERAL string "ops" — NOT "${TEAM}".
    let labels = job.labels.as_ref().expect("labels present");
    assert!(
        labels.contains_key("ops"),
        "interpolation must resolve ${{TEAM}} -> 'ops' in key position; \
         got keys: {:?}",
        labels.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        labels.get("ops").map(String::as_str),
        Some("v"),
        "value v must survive interpolation pass intact"
    );
    // Negative — the literal placeholder MUST NOT be a key.
    assert!(
        !labels.contains_key("${TEAM}"),
        "literal ${{TEAM}} must NOT survive interpolation in env-set case"
    );

    // SAFETY: same single-threaded justification.
    unsafe {
        std::env::remove_var("TEAM");
    }
}

/// CR-01 regression test (gap closure plan 17-07).
///
/// Pins the documented behavior: when an env var named in a LABEL KEY
/// position is UNSET, the interpolation pass at
/// `src/config/interpolate.rs::interpolate` emits a `MissingVar` error and
/// the load FAILS at the interpolation stage (BEFORE the validator runs).
/// The error path produces exit 1 with a "missing environment variable"
/// message — distinct from the D-02 strict-char message that would fire
/// only if the missing-var error were somehow suppressed.
///
/// This test does NOT require a Docker daemon.
#[tokio::test]
async fn lbl_05_key_position_interpolation_env_unset_caught_by_strict_chars() {
    // SAFETY: same single-threaded justification; ensure TEAM is not set
    // (a previous test in this file may have set it).
    unsafe {
        std::env::remove_var("TEAM");
    }

    let toml_text = r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[defaults]
image = "alpine:latest"

[[jobs]]
name = "lbl-05-key-unset"
schedule = "*/5 * * * *"
image = "alpine:latest"
cmd = ["sh", "-c", "exit 0"]
labels = { "${TEAM}" = "v" }
"#;

    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile created");
    tmp.write_all(toml_text.as_bytes()).expect("toml written");

    let result = parse_and_validate(tmp.path());
    let errors = result.expect_err("env-unset key-position interpolation MUST fail at config-LOAD");

    // Either path produces a load-time failure (exit 1 in the binary):
    //   * MissingVar error from interpolate.rs (the actual binary path)
    //   * Strict-char validator error (would fire only if MissingVar were
    //     suppressed; documented as the fallback in README's
    //     "Recommended pattern" section).
    // Assert that AT LEAST ONE of the two messages is present.
    let combined = errors
        .iter()
        .map(|e| e.message.clone())
        .collect::<Vec<_>>()
        .join(" || ");
    assert!(
        combined.contains("missing environment variable")
            || combined.contains("invalid label keys"),
        "expected MissingVar or D-02 invalid-char error; got: {combined}"
    );
    // The variable name MUST appear in the error — operator must know
    // which env var to set or which key to literalize.
    assert!(
        combined.contains("TEAM"),
        "error must name the offending var/key 'TEAM'; got: {combined}"
    );
}
