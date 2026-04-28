//! Phase 16 / FOUND-14: assert that v1.2 docker runs persist the REAL Docker
//! container ID into `job_runs.container_id` (NOT the `sha256:` image digest
//! that v1.0 / v1.1 silently stored due to the run.rs:301 bug). Also asserts
//! the parallel `image_digest` column is populated correctly for docker jobs
//! and left NULL for command/script jobs.
//!
//! Phase 16 FOUND-14 — covers T-V12-FCTX-07 and T-V12-FCTX-08:
//! * T-V12-FCTX-07: docker run captures `image_digest` as a non-NULL `sha256:`
//!   value AND `container_id` as a non-`sha256:` value (the operator-observable
//!   bug fix).
//! * T-V12-FCTX-08: command run leaves `image_digest = NULL` (non-docker jobs
//!   have no image).
//!
//! Wave-2 sequencing note: this file lands in Plan 16-03 alongside the
//! `run.rs` bug-fix edits, but the `insert_running_run` / `finalize_run`
//! signature changes that make it compile end-to-end land in Plans 16-04a and
//! 16-04b. The wave-2 orchestrator gates the cargo-test exercise after
//! 16-04b's gate task; the file is present here so the bug-fix test text
//! lives in the same commit as the bug-fix code.
//!
//! Docker-gated tests are `#[ignore]` and require a running Docker daemon:
//!   `cargo test --test v12_run_rs_277_bug_fix -- --ignored --nocapture --test-threads=1`

#![allow(clippy::assertions_on_constants)]

mod common;

use std::time::Duration;

use bollard::Docker;
use common::v11_fixtures::{seed_test_job, setup_sqlite_with_phase11_migrations};
use cronduit::db::DbPool;
use cronduit::db::queries::{self, PoolRef};
use cronduit::scheduler::command::RunStatus;
use cronduit::scheduler::control::RunControl;
use cronduit::scheduler::docker::execute_docker;
use cronduit::scheduler::log_pipeline;
use sqlx::Row;
use tokio_util::sync::CancellationToken;

/// Connect to the local Docker daemon. Panics if unavailable.
async fn docker_client() -> Docker {
    Docker::connect_with_local_defaults()
        .expect("Docker daemon must be running for integration tests")
}

/// Set up an in-memory SQLite DB with all migrations applied (including
/// Phase 16's image_digest + config_hash columns from Plan 16-01).
async fn setup_test_db() -> DbPool {
    setup_sqlite_with_phase11_migrations().await
}

/// Drain log_pipeline lines until the executor closes the sender, then return
/// them. Mirrors the `tests/docker_executor.rs` collector pattern.
async fn drain_logs(receiver: log_pipeline::LogReceiver) -> Vec<log_pipeline::LogLine> {
    let mut all = Vec::new();
    loop {
        let batch = receiver.drain_batch_async(256).await;
        if batch.is_empty() {
            break;
        }
        all.extend(batch);
    }
    all
}

/// Phase 16 FOUND-14 / T-V12-FCTX-07.
///
/// Drives a real docker job (alpine echo) end-to-end:
///   1. seed jobs row + insert_running_run (job_runs.id reserved)
///   2. execute_docker (real Docker daemon — ignore-gated)
///   3. finalize_run(container_id_for_finalize, image_digest_for_finalize)
///   4. SELECT container_id FROM job_runs WHERE id = ? AND assert
///      `!container_id.starts_with("sha256:")` — i.e. v1.0/v1.1's misnamed
///      assignment (image digest mistakenly stored as container_id) is GONE.
#[tokio::test]
#[ignore]
async fn docker_run_writes_real_container_id_not_digest() {
    let docker = docker_client().await;
    let pool = setup_test_db().await;
    let job_id = seed_test_job(&pool, "v12-fctx-bug-fix-cid").await;

    // Reserve the running row up-front (mirrors the production fire path).
    // After 16-04b, insert_running_run takes a config_hash &str.
    let run_id = queries::insert_running_run(&pool, job_id, "manual", "testhash")
        .await
        .expect("insert running run");

    let (sender, receiver) = log_pipeline::channel(256);
    let cancel = CancellationToken::new();
    let control = RunControl::new(cancel.clone());

    let collector = tokio::spawn(drain_logs(receiver));

    let config_json = r#"{"image": "alpine:latest", "cmd": ["echo", "hello-cronduit"]}"#;
    let docker_result = execute_docker(
        &docker,
        config_json,
        "v12-fctx-bug-fix-cid",
        run_id,
        Duration::from_secs(30),
        cancel,
        sender,
        &control,
    )
    .await;

    let _ = collector.await;

    assert_eq!(
        docker_result.exec.status,
        RunStatus::Success,
        "alpine echo should exit 0; got: {:?}",
        docker_result.exec
    );

    // Mirror src/scheduler/run.rs L301 + L348-356 wiring exactly so this test
    // exercises the same observable as production.
    let container_id_for_finalize = docker_result.container_id.clone();
    let image_digest_for_finalize = docker_result.image_digest.clone();

    queries::finalize_run(
        &pool,
        run_id,
        "success",
        Some(0),
        tokio::time::Instant::now(),
        None,
        container_id_for_finalize.as_deref(),
        image_digest_for_finalize.as_deref(),
    )
    .await
    .expect("finalize run");

    // SELECT and assert.
    let p = match pool.reader() {
        PoolRef::Sqlite(pp) => pp,
        _ => panic!("sqlite-only test"),
    };
    let row = sqlx::query("SELECT container_id, image_digest FROM job_runs WHERE id = ?1")
        .bind(run_id)
        .fetch_one(p)
        .await
        .expect("select job_run row");

    let container_id: Option<String> = row.get("container_id");
    let cid = container_id.expect(
        "container_id must be Some(_) after a successful docker run — Plan 16-02 populates \
         DockerExecResult.container_id from create_container",
    );
    assert!(
        !cid.starts_with("sha256:"),
        "Phase 16 FOUND-14: job_runs.container_id must hold the real Docker container ID, \
         NOT the sha256 image digest. Got: {cid}"
    );
    assert!(
        !cid.is_empty(),
        "Phase 16 FOUND-14: container_id must be non-empty for a successful docker run; got empty string"
    );
}

/// Phase 16 FOUND-14 / T-V12-FCTX-07.
///
/// Same setup as the previous test, but asserts the parallel observable:
/// `job_runs.image_digest` IS Some(_) AND starts with `sha256:` — i.e. the
/// digest is captured separately into the new column added by Plan 16-01 and
/// flowed through Plans 16-03/16-04 from `DockerExecResult.image_digest`.
#[tokio::test]
#[ignore]
async fn docker_run_writes_image_digest_as_sha256() {
    let docker = docker_client().await;
    let pool = setup_test_db().await;
    let job_id = seed_test_job(&pool, "v12-fctx-bug-fix-digest").await;

    let run_id = queries::insert_running_run(&pool, job_id, "manual", "testhash")
        .await
        .expect("insert running run");

    let (sender, receiver) = log_pipeline::channel(256);
    let cancel = CancellationToken::new();
    let control = RunControl::new(cancel.clone());

    let collector = tokio::spawn(drain_logs(receiver));

    let config_json = r#"{"image": "alpine:latest", "cmd": ["echo", "digest-check"]}"#;
    let docker_result = execute_docker(
        &docker,
        config_json,
        "v12-fctx-bug-fix-digest",
        run_id,
        Duration::from_secs(30),
        cancel,
        sender,
        &control,
    )
    .await;

    let _ = collector.await;

    assert_eq!(docker_result.exec.status, RunStatus::Success);

    let container_id_for_finalize = docker_result.container_id.clone();
    let image_digest_for_finalize = docker_result.image_digest.clone();

    queries::finalize_run(
        &pool,
        run_id,
        "success",
        Some(0),
        tokio::time::Instant::now(),
        None,
        container_id_for_finalize.as_deref(),
        image_digest_for_finalize.as_deref(),
    )
    .await
    .expect("finalize run");

    let p = match pool.reader() {
        PoolRef::Sqlite(pp) => pp,
        _ => panic!("sqlite-only test"),
    };
    let row = sqlx::query("SELECT image_digest FROM job_runs WHERE id = ?1")
        .bind(run_id)
        .fetch_one(p)
        .await
        .expect("select job_run row");

    let image_digest: Option<String> = row.get("image_digest");
    let dig = image_digest.expect(
        "Phase 16 FOUND-14: image_digest must be Some(_) for a successful docker run — \
         DockerExecResult.image_digest is populated by inspect_container post-start",
    );
    assert!(
        dig.starts_with("sha256:"),
        "Phase 16 FOUND-14: image_digest must be a sha256:... digest; got: {dig}"
    );
}

/// Phase 16 FOUND-14 / T-V12-FCTX-08.
///
/// Command/script jobs have no Docker image, so the per-run image_digest
/// column must remain NULL after finalize_run. This test does NOT require a
/// Docker daemon — it exercises the no-docker path through finalize_run with
/// `None` for both container_id and image_digest, and asserts the column
/// state.
#[tokio::test]
async fn command_run_leaves_image_digest_null() {
    let pool = setup_test_db().await;
    let job_id = seed_test_job(&pool, "v12-fctx-cmd-null-digest").await;

    let run_id = queries::insert_running_run(&pool, job_id, "manual", "testhash")
        .await
        .expect("insert running run");

    // Mirror the command/script arm of run_job: container_id_for_finalize and
    // image_digest_for_finalize both start as None and never get assigned for
    // non-docker job types.
    queries::finalize_run(
        &pool,
        run_id,
        "success",
        Some(0),
        tokio::time::Instant::now(),
        None,
        None,
        None,
    )
    .await
    .expect("finalize run");

    let p = match pool.reader() {
        PoolRef::Sqlite(pp) => pp,
        _ => panic!("sqlite-only test"),
    };
    let row = sqlx::query("SELECT image_digest, container_id FROM job_runs WHERE id = ?1")
        .bind(run_id)
        .fetch_one(p)
        .await
        .expect("select job_run row");

    let image_digest: Option<String> = row.get("image_digest");
    let container_id: Option<String> = row.get("container_id");
    assert!(
        image_digest.is_none(),
        "Phase 16 FOUND-14: command-job runs must leave image_digest NULL; got: {image_digest:?}"
    );
    assert!(
        container_id.is_none(),
        "Phase 16 FOUND-14: command-job runs must leave container_id NULL; got: {container_id:?}"
    );
}

/// Phase 16 FOUND-14 / T-V12-FCTX-09 (partial).
///
/// If `inspect_container` post-start were to fail, `DockerExecResult.image_digest`
/// may be `None` (the existing fallback at docker.rs L240-251). This test
/// asserts that finalize_run can be called with `Some(container_id)` +
/// `None` digest without panicking, and that the row is still queryable
/// post-finalize. No Docker daemon required — exercises the wiring contract,
/// not the Docker side. `#[ignore]` is NOT applied: this is a fast in-memory
/// SQLite test that must pass in standard CI.
#[tokio::test]
async fn digest_persists_across_inspect_failure() {
    let pool = setup_test_db().await;
    let job_id = seed_test_job(&pool, "v12-fctx-inspect-failure").await;

    let run_id = queries::insert_running_run(&pool, job_id, "manual", "testhash")
        .await
        .expect("insert running run");

    // Simulate the post-start inspect_container failure path: container_id
    // is known (start succeeded), but image_digest is None (inspect failed).
    queries::finalize_run(
        &pool,
        run_id,
        "success",
        Some(0),
        tokio::time::Instant::now(),
        None,
        Some("simulated-real-container-id-abc123"),
        None,
    )
    .await
    .expect("finalize run");

    let p = match pool.reader() {
        PoolRef::Sqlite(pp) => pp,
        _ => panic!("sqlite-only test"),
    };
    let row = sqlx::query("SELECT container_id, image_digest, status FROM job_runs WHERE id = ?1")
        .bind(run_id)
        .fetch_one(p)
        .await
        .expect("select job_run row");

    let container_id: Option<String> = row.get("container_id");
    let image_digest: Option<String> = row.get("image_digest");
    let status: String = row.get("status");

    assert_eq!(
        container_id.as_deref(),
        Some("simulated-real-container-id-abc123"),
        "container_id from a successful start should be persisted even when image_digest is None"
    );
    assert!(
        image_digest.is_none(),
        "image_digest should remain None when inspect_container failed; got: {image_digest:?}"
    );
    assert_eq!(status, "success", "row must reach a terminal status, not panic");
}
