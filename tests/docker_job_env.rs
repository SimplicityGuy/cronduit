//! cronduit-1b1: `[jobs.env]` values reach Docker job containers.
//!
//! Before the fix, `config_json` stored only `env_keys` (T-02-03), the docker
//! executor read env from `config_json`, and every docker container was
//! created with no `[jobs.env]` at all. These tests inspect the created
//! container's `Config.Env` through bollard:
//!
//! 1. `jobs_env_reaches_container_env`: literal and `${VAR}`-interpolated
//!    values, end to end through `parse_and_validate` -> `build_job_env_map`
//!    -> `execute_docker`.
//! 2. `jobs_env_reload_reaches_next_run_without_restart`: the real scheduler
//!    loop runs the job, the config changes (a value edited, a key added, an
//!    interpolated secret rotated: none of which changes `config_hash`), a
//!    reload command is processed, and the next run's container has the new
//!    env.
//!
//! Each job container uses `delete = false` (so it survives the run for
//! inspection) and a unique `container_name` owned by an `ExecutorContainer`
//! guard, which removes it on success and on failure.
//!
//! Run: `cargo test --test docker_job_env -- --ignored --nocapture --test-threads=1`
//! (`--test-threads=1` because the tests set process env vars for `${VAR}`.)

#[path = "support/containers.rs"]
mod containers;

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bollard::Docker;
use bollard::query_parameters::InspectContainerOptions;
use containers::ExecutorContainer;
use cronduit::config::parse_and_validate;
use cronduit::db::DbPool;
use cronduit::db::queries::{get_run_by_id, insert_running_run};
use cronduit::scheduler::cmd::{ReloadStatus, SchedulerCmd};
use cronduit::scheduler::command::RunStatus;
use cronduit::scheduler::control::RunControl;
use cronduit::scheduler::docker::execute_docker;
use cronduit::scheduler::job_env::{build_job_env_map, env_for};
use cronduit::scheduler::log_pipeline;
use cronduit::scheduler::sync::{serialize_config_json_for_tests, sync_config_to_db};
use tokio_util::sync::CancellationToken;

const IMAGE: &str = "alpine:latest";

fn write_config(file: &Path, body: &str) {
    let mut f = std::fs::File::create(file).expect("create config");
    f.write_all(body.as_bytes()).expect("write config");
    f.flush().expect("flush config");
}

/// The env Docker should report for a container created from `IMAGE` with
/// `configured` as its `Env`: the image's own env, overridden by the
/// configured pairs. Sorted for comparison.
async fn expected_env(docker: &Docker, configured: &[(&str, &str)]) -> Vec<String> {
    let image = docker.inspect_image(IMAGE).await.expect("inspect image");
    let mut env: BTreeMap<String, String> = image
        .config
        .and_then(|c| c.env)
        .unwrap_or_default()
        .into_iter()
        .map(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or((&kv, ""));
            (k.to_string(), v.to_string())
        })
        .collect();
    for (k, v) in configured {
        env.insert(k.to_string(), v.to_string());
    }
    env.into_iter().map(|(k, v)| format!("{k}={v}")).collect()
}

async fn container_env(docker: &Docker, name: &str) -> Vec<String> {
    let inspect = docker
        .inspect_container(name, None::<InspectContainerOptions>)
        .await
        .expect("job container exists after the run (delete = false)");
    let mut env = inspect.config.and_then(|c| c.env).unwrap_or_default();
    env.sort();
    env
}

#[tokio::test]
#[ignore]
async fn jobs_env_reaches_container_env() {
    let docker = containers::docker().expect("Docker client");
    let job_name = "env-e2e";
    let guard = ExecutorContainer::new(job_name);

    // SAFETY: this binary runs with --test-threads=1 and the variable name is
    // unique to this test.
    unsafe { std::env::set_var("CRONDUIT_TEST_1B1_E2E_PASSWORD", "s3cr3t p@ss=word") };

    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("cronduit.toml");
    write_config(
        &config_path,
        &format!(
            r#"
[server]
timezone = "UTC"

[[jobs]]
name = "{job_name}"
schedule = "0 0 1 1 *"
image = "{IMAGE}"
cmd = ["sh", "-c", "exit 0"]
delete = false
container_name = "{name}"

[jobs.env]
POSTGRES_USERNAME = "groovemap"
POSTGRES_DATABASE = "embeddings"
POSTGRES_PASSWORD = "${{CRONDUIT_TEST_1B1_E2E_PASSWORD}}"
"#,
            name = guard.name()
        ),
    );

    let parsed = parse_and_validate(&config_path).expect("config parses");
    let job = &parsed.config.jobs[0];
    let config_json = serialize_config_json_for_tests(job);
    // Acceptance 3: config_json carries the key names only.
    assert!(config_json.contains("\"env_keys\""), "{config_json}");
    for secret in ["groovemap", "embeddings", "s3cr3t p@ss=word"] {
        assert!(
            !config_json.contains(secret),
            "env value leaked into config_json: {config_json}"
        );
    }

    let env_map = build_job_env_map(&parsed.config);
    let (sender, _receiver) = log_pipeline::channel(64);
    let cancel = CancellationToken::new();
    let control = RunControl::new(cancel.clone());
    let result = execute_docker(
        &docker,
        &config_json,
        &env_for(&env_map, job_name),
        job_name,
        1,
        Duration::from_secs(60),
        cancel,
        sender,
        &control,
    )
    .await;
    assert_eq!(result.exec.status, RunStatus::Success, "{:?}", result.exec);

    assert_eq!(
        container_env(&docker, guard.name()).await,
        expected_env(
            &docker,
            &[
                ("POSTGRES_USERNAME", "groovemap"),
                ("POSTGRES_DATABASE", "embeddings"),
                ("POSTGRES_PASSWORD", "s3cr3t p@ss=word"),
            ],
        )
        .await,
        "container Env must be the image env plus exactly the [jobs.env] pairs"
    );
}

/// Insert a run row and dispatch it through the scheduler loop (the UI
/// "Run Now" path), then wait for it to finalize.
async fn run_now(
    pool: &DbPool,
    cmd_tx: &tokio::sync::mpsc::Sender<SchedulerCmd>,
    job_id: i64,
) -> String {
    let run_id = insert_running_run(pool, job_id, "manual", "test", None)
        .await
        .expect("insert run");
    cmd_tx
        .send(SchedulerCmd::RunNowWithRunId { job_id, run_id })
        .await
        .expect("scheduler accepts RunNowWithRunId");
    tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            let run = get_run_by_id(pool, run_id).await.unwrap().unwrap();
            if run.status != "running" {
                return run.status;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("run finalizes")
}

#[tokio::test]
#[ignore]
async fn jobs_env_reload_reaches_next_run_without_restart() {
    let docker = containers::docker().expect("Docker client");
    let job_name = "env-reload";
    let guard = ExecutorContainer::new(job_name);
    let var = "CRONDUIT_TEST_1B1_RELOAD_TOKEN";

    let config = |literal: &str, extra: &str| {
        format!(
            r#"
[server]
timezone = "UTC"

[[jobs]]
name = "{job_name}"
schedule = "0 0 1 1 *"
image = "{IMAGE}"
cmd = ["sh", "-c", "exit 0"]
delete = false
container_name = "{name}"

[jobs.env]
LITERAL = "{literal}"
TOKEN = "${{{var}}}"
{extra}
"#,
            name = guard.name()
        )
    };

    // SAFETY: --test-threads=1; variable name unique to this test.
    unsafe { std::env::set_var(var, "token-v1") };
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("cronduit.toml");
    write_config(&config_path, &config("one", ""));

    // Startup, as `cronduit run` does it: parse, sync, build the env map,
    // spawn the scheduler loop.
    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    pool.migrate().await.unwrap();
    let parsed = parse_and_validate(&config_path).expect("config parses");
    let sync = sync_config_to_db(&pool, &parsed.config, Duration::ZERO)
        .await
        .expect("sync");
    let job_id = sync.jobs[0].id;
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(8);
    let (webhook_tx, _webhook_rx) = cronduit::webhooks::channel();
    let cancel = CancellationToken::new();
    let scheduler = cronduit::scheduler::spawn(
        pool.clone(),
        Some(docker.clone()),
        sync.jobs,
        chrono_tz::UTC,
        cancel.clone(),
        Duration::from_secs(5),
        cmd_rx,
        config_path.clone(),
        Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        webhook_tx,
        build_job_env_map(&parsed.config),
    );

    // Run 1: the startup env.
    assert_eq!(run_now(&pool, &cmd_tx, job_id).await, "success");
    assert_eq!(
        container_env(&docker, guard.name()).await,
        expected_env(&docker, &[("LITERAL", "one"), ("TOKEN", "token-v1")]).await,
    );
    guard.remove().await.expect("remove run 1 container");

    // Edit a value, add a key, and rotate the interpolated secret. None of
    // these change config_hash, so sync reports the job as unchanged: the
    // env map must be rebuilt anyway.
    unsafe { std::env::set_var(var, "token-v2") };
    write_config(&config_path, &config("two", "ADDED = \"new-key\""));
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    cmd_tx
        .send(SchedulerCmd::Reload { response_tx })
        .await
        .expect("scheduler accepts Reload");
    let reload = response_rx.await.expect("reload result");
    assert_eq!(
        reload.status,
        ReloadStatus::Ok,
        "{:?}",
        reload.error_message
    );

    // Run 2: the reloaded env, same process, no restart.
    assert_eq!(run_now(&pool, &cmd_tx, job_id).await, "success");
    assert_eq!(
        container_env(&docker, guard.name()).await,
        expected_env(
            &docker,
            &[
                ("ADDED", "new-key"),
                ("LITERAL", "two"),
                ("TOKEN", "token-v2"),
            ],
        )
        .await,
    );

    // config_json in the DB still holds key names only.
    let row = cronduit::db::queries::get_job_by_id(&pool, job_id)
        .await
        .unwrap()
        .unwrap();
    for secret in ["one", "two", "token-v1", "token-v2", "new-key"] {
        assert!(
            !row.config_json.contains(&format!("\"{secret}\"")),
            "env value leaked into jobs.config_json: {}",
            row.config_json
        );
    }

    cancel.cancel();
    let _ = scheduler.await;
}
