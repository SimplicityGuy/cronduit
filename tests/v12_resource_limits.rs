//! Per-job `memory` / `cpus` limits: parse_and_validate -> apply_defaults ->
//! serialize -> execute_docker -> bollard -> daemon -> inspect_container.
//!
//! Run: `cargo test --test v12_resource_limits -- --ignored --nocapture --test-threads=1`
//!
//! Each test owns a uniquely named container (pid + test tag) and removes it
//! in all paths, including assertion failure, via `OwnedContainer`'s guard.

use bollard::Docker;
use bollard::query_parameters::{InspectContainerOptions, RemoveContainerOptions};
use cronduit::config::parse_and_validate;
use cronduit::scheduler::command::RunStatus;
use cronduit::scheduler::control::RunControl;
use cronduit::scheduler::docker::{OOM_ERROR_PREFIX, execute_docker};
use cronduit::scheduler::log_pipeline;
use cronduit::scheduler::sync::serialize_config_json_for_tests;
use std::io::Write;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

async fn docker_client() -> Docker {
    Docker::connect_with_local_defaults()
        .expect("Docker daemon must be running for integration tests")
}

/// Removes the named (explicitly owned) container on drop, even on panic.
struct OwnedContainer {
    name: String,
}

impl Drop for OwnedContainer {
    fn drop(&mut self) {
        let name = self.name.clone();
        let _ = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                if let Ok(d) = Docker::connect_with_local_defaults() {
                    let _ = d
                        .remove_container(
                            &name,
                            Some(RemoveContainerOptions {
                                force: true,
                                ..Default::default()
                            }),
                        )
                        .await;
                }
            });
        })
        .join();
    }
}

fn load_job(toml_text: &str) -> cronduit::config::JobConfig {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(toml_text.as_bytes()).unwrap();
    let parsed = parse_and_validate(tmp.path()).expect("config parses + validates");
    parsed.config.jobs.into_iter().next().expect("one job")
}

#[tokio::test]
#[ignore]
async fn memory_and_cpus_applied_to_container() {
    let docker = docker_client().await;
    let (sender, _rx) = log_pipeline::channel(64);
    let cancel = CancellationToken::new();
    let control = RunControl::new(cancel.clone());
    let name = format!("cronduit-test-limits-apply-{}", std::process::id());
    let _guard = OwnedContainer { name: name.clone() };

    let job = load_job(&format!(
        r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[defaults]
image = "alpine:latest"
cpus = 2

[[jobs]]
name = "limits-job"
schedule = "*/5 * * * *"
cmd = ["sh", "-c", "exit 0"]
delete = false
container_name = "{name}"
memory = "64m"
"#
    ));
    assert_eq!(job.memory.as_deref(), Some("64m"));
    assert_eq!(job.cpus, Some(2.0), "cpus inherited from [defaults]");

    let result = execute_docker(
        &docker,
        &serialize_config_json_for_tests(&job),
        "limits-job",
        1,
        Duration::from_secs(60),
        cancel,
        sender,
        &control,
    )
    .await;
    assert_eq!(result.exec.status, RunStatus::Success, "{:?}", result.exec);
    let id = result.container_id.expect("container id");

    let info = docker
        .inspect_container(&id, None::<InspectContainerOptions>)
        .await
        .expect("inspect");
    let hc = info.host_config.expect("host_config");
    let limit = 64 * 1024 * 1024;
    assert_eq!(hc.memory, Some(limit));
    assert_eq!(hc.memory_swap, Some(limit), "swap must equal memory");
    assert_eq!(hc.nano_cpus, Some(2_000_000_000));
}

#[tokio::test]
#[ignore]
async fn oom_kill_is_reported_distinctly() {
    let docker = docker_client().await;
    let (sender, _rx) = log_pipeline::channel(64);
    let cancel = CancellationToken::new();
    let control = RunControl::new(cancel.clone());
    let name = format!("cronduit-test-limits-oom-{}", std::process::id());
    let _guard = OwnedContainer { name: name.clone() };

    // Allocate well past the 16m limit: the shell holds the whole 200 MB output in a variable.
    let job = load_job(&format!(
        r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[[jobs]]
name = "oom-job"
schedule = "*/5 * * * *"
image = "alpine:latest"
cmd = ["sh", "-c", "x=$(yes | head -c 200000000); echo ${{#x}}"]
delete = false
container_name = "{name}"
memory = "16m"
"#
    ));

    let result = execute_docker(
        &docker,
        &serialize_config_json_for_tests(&job),
        "oom-job",
        2,
        Duration::from_secs(60),
        cancel,
        sender,
        &control,
    )
    .await;
    assert_eq!(result.exec.status, RunStatus::Failed, "{:?}", result.exec);
    let msg = result.exec.error_message.expect("OOM error message");
    assert!(msg.starts_with(OOM_ERROR_PREFIX), "got: {msg}");
}

#[tokio::test]
#[ignore]
async fn plain_nonzero_exit_is_not_oom() {
    let docker = docker_client().await;
    let (sender, _rx) = log_pipeline::channel(64);
    let cancel = CancellationToken::new();
    let control = RunControl::new(cancel.clone());
    let name = format!("cronduit-test-limits-exit-{}", std::process::id());
    let _guard = OwnedContainer { name: name.clone() };

    let job = load_job(&format!(
        r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[[jobs]]
name = "exit-job"
schedule = "*/5 * * * *"
image = "alpine:latest"
cmd = ["sh", "-c", "exit 3"]
delete = false
container_name = "{name}"
memory = "64m"
"#
    ));
    let result = execute_docker(
        &docker,
        &serialize_config_json_for_tests(&job),
        "exit-job",
        3,
        Duration::from_secs(60),
        cancel,
        sender,
        &control,
    )
    .await;
    assert_eq!(result.exec.status, RunStatus::Failed);
    assert_eq!(result.exec.exit_code, Some(3));
    assert!(result.exec.error_message.is_none());
}

#[test]
fn limits_rejected_on_command_job_at_load() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(
        br#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[[jobs]]
name = "cmd-job"
schedule = "*/5 * * * *"
command = "echo hi"
memory = "1g"
"#,
    )
    .unwrap();
    let errs = parse_and_validate(tmp.path()).expect_err("must reject");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("only valid on docker jobs"))
    );
}
