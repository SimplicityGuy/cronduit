//! T-V11-STOP-07..08: process-group kill regression lock (SCHED-12, D-17).
//!
//! These tests prove that `.process_group(0)` + `libc::kill(-pid, SIGKILL)`
//! reaps shell-pipeline grandchildren and background processes. A refactor
//! that adopts `kill_on_drop(true)` instead would pass basic command
//! termination but leak grandchildren and background subshells — these
//! tests catch that regression.
//!
//! Strategy: sentinel-file probes. Each test spawns a shell that arranges
//! for a sentinel file to be written AFTER a sleep that exceeds the test's
//! tolerance window. After firing Stop, we wait past the tolerance window
//! and assert the sentinel file does NOT exist. If a refactor dropped the
//! pgid kill, the grandchildren or backgrounded subshells would survive,
//! the sleep would complete, and the sentinel file would appear — failing
//! the assertion.
//!
//! Platform gate: process-group semantics are Linux-specific (the v1
//! daemon ships Linux-only inside Docker). On other platforms this file
//! compiles to an empty module.

#![cfg(target_os = "linux")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use cronduit::db::DbPool;
use cronduit::db::queries::{self, DbJob};
use cronduit::scheduler::RunEntry;
use cronduit::scheduler::cmd::{SchedulerCmd, StopResult};
use cronduit::scheduler::run::run_job;
use tokio::sync::{RwLock, mpsc, oneshot};
use tokio_util::sync::CancellationToken;

/// Phase 15 / WH-02 — per-test webhook channel. The Receiver is dropped
/// immediately; these tests do not assert on webhook behavior.
fn test_webhook_tx() -> mpsc::Sender<cronduit::webhooks::RunFinalized> {
    let (tx, _rx) = cronduit::webhooks::channel_with_capacity(8);
    tx
}

/// Unique sentinel path per test invocation. Uses nanosecond timestamp + pid
/// so parallel test runs cannot collide. Path is shell-safe (alphanumerics +
/// dashes) — `ThreadId(n)` debug formatting was avoided because its parens
/// collide with `sh` subshell syntax when the path is substituted into
/// `sh -c '... touch <path>'`.
fn sentinel_path(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    PathBuf::from(format!("/tmp/cronduit-pg-kill-{tag}-{pid}-{nanos}"))
}

async fn seed_command_job(pool: &DbPool, name: &str, command_str: &str) -> DbJob {
    let id = queries::upsert_job(
        pool,
        name,
        "0 0 31 2 *",
        "0 0 31 2 *",
        "command",
        &format!(r#"{{"command":{}}}"#, serde_json::json!(command_str)),
        "pg-kill-hash",
        3600,
        "[]",
    )
    .await
    .expect("upsert job");

    DbJob {
        id,
        name: name.to_string(),
        schedule: "0 0 31 2 *".to_string(),
        resolved_schedule: "0 0 31 2 *".to_string(),
        job_type: "command".to_string(),
        config_json: format!(r#"{{"command":{}}}"#, serde_json::json!(command_str)),
        config_hash: "pg-kill-hash".to_string(),
        enabled: true,
        enabled_override: None,
        timeout_secs: 3600,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

async fn seed_script_job(pool: &DbPool, name: &str, body: &str) -> DbJob {
    let config_json = format!(r#"{{"script":{}}}"#, serde_json::json!(body));
    let id = queries::upsert_job(
        pool,
        name,
        "0 0 31 2 *",
        "0 0 31 2 *",
        "script",
        &config_json,
        "pg-kill-hash",
        3600,
        "[]",
    )
    .await
    .expect("upsert script job");

    DbJob {
        id,
        name: name.to_string(),
        schedule: "0 0 31 2 *".to_string(),
        resolved_schedule: "0 0 31 2 *".to_string(),
        job_type: "script".to_string(),
        config_json,
        config_hash: "pg-kill-hash".to_string(),
        enabled: true,
        enabled_override: None,
        timeout_secs: 3600,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

/// Minimal Stop arm driver — same verbatim replication of the scheduler
/// Stop arm as in `tests/stop_executors.rs`.
fn spawn_stop_arm_driver(
    active_runs: Arc<RwLock<HashMap<i64, RunEntry>>>,
    mut cmd_rx: mpsc::Receiver<SchedulerCmd>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            if let SchedulerCmd::Stop {
                run_id,
                response_tx,
            } = cmd
            {
                let maybe_control = {
                    let active = active_runs.read().await;
                    active.get(&run_id).map(|entry| entry.control.clone())
                };
                let result = match maybe_control {
                    Some(control) => {
                        control.stop(cronduit::scheduler::control::StopReason::Operator);
                        StopResult::Stopped
                    }
                    None => StopResult::AlreadyFinalized,
                };
                let _ = response_tx.send(result);
            }
        }
    })
}

async fn wait_for_active_run(
    active_runs: &Arc<RwLock<HashMap<i64, RunEntry>>>,
    deadline: Duration,
) -> i64 {
    let start = tokio::time::Instant::now();
    loop {
        {
            let map = active_runs.read().await;
            if let Some(run_id) = map.keys().next().copied() {
                return run_id;
            }
        }
        if start.elapsed() > deadline {
            panic!("no run entered active_runs within {:?}", deadline);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

// ---------------------------------------------------------------------------
// T-V11-STOP-07: shell-pipeline grandchildren are reaped via pgid kill
// ---------------------------------------------------------------------------
//
// Pipeline shape: `sh -c 'sleep 3 | cat | cat; touch <sentinel>'`
//
// The direct child of `tokio::process::Command` is `sh`. The pipeline forks
// three grandchildren (`sleep`, `cat`, `cat`) connected by pipes. The
// `touch` after the pipeline only runs if the `sleep 3` completes — it
// should NEVER be reached after an operator Stop, because the pgid kill
// should reap the `sh` AND all three grandchildren, preventing the `touch`
// from running.
//
// If a refactor adopts `kill_on_drop(true)` instead:
//   - `kill_on_drop` targets the direct child (`sh`) only.
//   - Killing `sh` may or may not propagate to `sleep` depending on pipe
//     close timing. Even if `sleep` dies via SIGPIPE, the `touch` still
//     runs as part of the already-queued `sh` command list.
//   - The sentinel file appears → test fails.
//
// With `process_group(0) + kill(-pgid, SIGKILL)`:
//   - The entire process group (sh + sleep + cat + cat) dies instantly.
//   - `sh` never reaches the `touch` statement.
//   - Sentinel file does NOT appear → test passes.

#[tokio::test]
async fn stop_kills_shell_pipeline_grandchildren() {
    let sentinel = sentinel_path("pipeline");
    // Ensure the sentinel does not already exist from a previous run.
    let _ = std::fs::remove_file(&sentinel);

    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("sqlite pool");
    pool.migrate().await.expect("migrate");

    // sh -c 'sleep 3 | cat | cat; touch <sentinel>'
    //
    // If the pgid kill works, the `; touch` branch never executes.
    let cmd_str = format!(
        "sh -c {}",
        shell_words::quote(&format!(
            "sleep 3 | cat | cat; touch {}",
            sentinel.display()
        ))
    );
    let job = seed_command_job(&pool, "pg-kill-pipeline", &cmd_str).await;

    let active_runs: Arc<RwLock<HashMap<i64, RunEntry>>> = Arc::new(RwLock::new(HashMap::new()));
    let (cmd_tx, cmd_rx) = mpsc::channel::<SchedulerCmd>(16);
    let driver = spawn_stop_arm_driver(active_runs.clone(), cmd_rx);

    let pool_clone = pool.clone();
    let active_clone = active_runs.clone();
    let cancel = CancellationToken::new();
    let exec_handle = tokio::spawn(async move {
        run_job(
            pool_clone,
            None,
            job,
            "manual".to_string(),
            cancel,
            active_clone,
            test_webhook_tx(),
            None, // Phase 21 FCTX-06: test passes None
        )
        .await
    });

    let run_id = wait_for_active_run(&active_runs, Duration::from_secs(5)).await;

    // Give the pipeline ~200ms to fully start (sh forks + pipe setup).
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Dispatch Stop.
    let (resp_tx, resp_rx) = oneshot::channel();
    cmd_tx
        .send(SchedulerCmd::Stop {
            run_id,
            response_tx: resp_tx,
        })
        .await
        .expect("dispatch Stop");
    assert_eq!(resp_rx.await.expect("stop reply"), StopResult::Stopped);

    // Wait for run_job to finalize.
    let result = tokio::time::timeout(Duration::from_secs(10), exec_handle)
        .await
        .expect("executor did not finalize")
        .expect("executor task did not panic");
    assert_eq!(result.status, "stopped");

    // The `sleep 3` would naturally finish 3s after spawn. Wait past that
    // so a surviving grandchild would have time to touch the sentinel.
    tokio::time::sleep(Duration::from_secs(4)).await;

    assert!(
        !sentinel.exists(),
        "T-V11-STOP-07 (D-17 regression lock): sentinel file {} should NOT exist — \
         pgid kill must have reaped the `sh` grandchildren before the `touch` ran. \
         Its presence indicates a refactor broke the process-group kill semantics \
         (SCHED-12 preservation lock).",
        sentinel.display()
    );

    drop(cmd_tx);
    let _ = driver.await;
    pool.close().await;
}

// ---------------------------------------------------------------------------
// T-V11-STOP-08: backgrounded subshells in scripts are reaped via pgid kill
// ---------------------------------------------------------------------------
//
// Script shape:
//   #!/bin/sh
//   (sleep 3 && touch <sentinel-bg>) &
//   sleep 30
//
// The backgrounded subshell inherits the parent's process group because
// `.process_group(0)` established the group at spawn time. The main script
// also sleeps. When Stop fires, `kill(-pgid, SIGKILL)` kills both the main
// `sh` AND the backgrounded subshell — the entire group.
//
// If a refactor drops the pgid kill and uses `kill_on_drop(true)` instead:
//   - The main `sh` dies.
//   - The backgrounded subshell is a separate process (different PID) but
//     it is NOT the direct child of `tokio::process::Command` — it's a
//     grandchild. `kill_on_drop` does not touch it.
//   - The backgrounded `sleep 3` completes and touches the sentinel.
//   - Test fails.
//
// With pgid kill: the entire group dies instantly, sentinel never appears.

#[tokio::test]
async fn stop_kills_backgrounded_processes_in_script() {
    let sentinel = sentinel_path("background");
    let _ = std::fs::remove_file(&sentinel);

    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("sqlite pool");
    pool.migrate().await.expect("migrate");

    // (sleep 3 && touch <sentinel>) &
    // sleep 30
    let script_body = format!("(sleep 3 && touch {}) &\nsleep 30\n", sentinel.display());
    let job = seed_script_job(&pool, "pg-kill-background", &script_body).await;

    let active_runs: Arc<RwLock<HashMap<i64, RunEntry>>> = Arc::new(RwLock::new(HashMap::new()));
    let (cmd_tx, cmd_rx) = mpsc::channel::<SchedulerCmd>(16);
    let driver = spawn_stop_arm_driver(active_runs.clone(), cmd_rx);

    let pool_clone = pool.clone();
    let active_clone = active_runs.clone();
    let cancel = CancellationToken::new();
    let exec_handle = tokio::spawn(async move {
        run_job(
            pool_clone,
            None,
            job,
            "manual".to_string(),
            cancel,
            active_clone,
            test_webhook_tx(),
            None, // Phase 21 FCTX-06: test passes None
        )
        .await
    });

    let run_id = wait_for_active_run(&active_runs, Duration::from_secs(5)).await;

    // Give the script time to fork the backgrounded subshell.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (resp_tx, resp_rx) = oneshot::channel();
    cmd_tx
        .send(SchedulerCmd::Stop {
            run_id,
            response_tx: resp_tx,
        })
        .await
        .expect("dispatch Stop");
    assert_eq!(resp_rx.await.expect("stop reply"), StopResult::Stopped);

    let result = tokio::time::timeout(Duration::from_secs(10), exec_handle)
        .await
        .expect("executor did not finalize")
        .expect("executor task did not panic");
    assert_eq!(result.status, "stopped");

    // Wait past the backgrounded `sleep 3`'s natural completion.
    tokio::time::sleep(Duration::from_secs(4)).await;

    assert!(
        !sentinel.exists(),
        "T-V11-STOP-08 (D-17 regression lock): sentinel file {} should NOT exist — \
         pgid kill must have reaped the backgrounded subshell. Its presence \
         indicates a refactor broke the process-group kill semantics for script \
         executors (SCHED-12 preservation lock).",
        sentinel.display()
    );

    drop(cmd_tx);
    let _ = driver.await;
    pool.close().await;
}
