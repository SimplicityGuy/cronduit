//! T-V11-STOP-15/16: metrics stopped label declaration + increment tests
//! (SCHED-09 telemetry, D-10).
//!
//! Two invariants locked here:
//!
//! 1. `metrics_pre_declares_stopped_label` (T-V11-STOP-16) — the Prometheus
//!    exporter renders `cronduit_runs_total{status="stopped"}` from boot,
//!    BEFORE any run has finalized as stopped. PITFALLS §1.6: label values
//!    that never fire are absent from /metrics text output without an
//!    explicit pre-declaration.
//!
//! 2. `stop_increments_runs_total_stopped` (T-V11-STOP-15) — after a real
//!    command executor is stopped via the scheduler's Stop arm, the
//!    `cronduit_runs_total{job="...",status="stopped"}` counter has a
//!    positive sample AND `cronduit_run_failures_total` is NOT incremented
//!    for the same run (Pitfall 1 / D-10 — operator stops are NOT failures).
//!
//! The test binary is isolated (each `tests/*.rs` file compiles to its own
//! binary under `cargo test`), so the OnceLock-backed metrics handle in
//! `cronduit::telemetry::setup_metrics` is fresh for this file's process.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use cronduit::db::DbPool;
use cronduit::db::queries::{self, DbJob};
use cronduit::scheduler::RunEntry;
use cronduit::scheduler::cmd::{SchedulerCmd, StopResult};
use cronduit::scheduler::run::run_job;
use cronduit::telemetry::setup_metrics;
use tokio::sync::{RwLock, mpsc, oneshot};
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// T-V11-STOP-16: metrics pre-declares stopped label at boot
// ---------------------------------------------------------------------------

#[tokio::test]
async fn metrics_pre_declares_stopped_label() {
    let handle = setup_metrics();
    let body = handle.render();

    assert!(
        body.contains(r#"cronduit_runs_total{status="stopped"}"#),
        "T-V11-STOP-16 (PITFALLS §1.6): setup_metrics() must pre-declare the \
         cronduit_runs_total{{status=\"stopped\"}} label so Prometheus alerts \
         that reference the stopped status do not go silent before the first \
         operator stop. /metrics body was:\n{body}"
    );

    // Good hygiene: also pre-declare the other terminal statuses so the
    // /metrics shape is stable from boot. These are locked for the metric
    // name pattern, not for exact counts — they may later be incremented by
    // real runs in the same process.
    for status in ["success", "failed", "timeout", "cancelled", "error"] {
        let needle = format!("cronduit_runs_total{{status=\"{status}\"}}");
        assert!(
            body.contains(&needle),
            "setup_metrics() must pre-declare cronduit_runs_total{{status=\"{status}\"}}; \
             /metrics body was:\n{body}"
        );
    }
}

// ---------------------------------------------------------------------------
// T-V11-STOP-15: stop increments runs_total{status=stopped} and does NOT
// increment cronduit_run_failures_total (D-10 / Pitfall 1 lock).
// ---------------------------------------------------------------------------

async fn seed_command_job(pool: &DbPool, name: &str, command_str: &str) -> DbJob {
    let id = queries::upsert_job(
        pool,
        name,
        "0 0 31 2 *",
        "0 0 31 2 *",
        "command",
        &format!(r#"{{"command":{}}}"#, serde_json::json!(command_str)),
        "metrics-stopped-hash",
        3600,
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
        config_hash: "metrics-stopped-hash".to_string(),
        enabled: true,
        enabled_override: None,
        timeout_secs: 3600,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

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

#[tokio::test]
async fn stop_increments_runs_total_stopped() {
    // Install the Prometheus recorder FIRST so run.rs's counter! macros
    // feed into a real registry this test can scrape.
    let handle = setup_metrics();

    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("sqlite pool");
    pool.migrate().await.expect("migrate");

    // Unique job name per test invocation so the counter sample for this
    // specific job is 1 even if other tests ran in the same binary.
    let job_name = "metrics-stopped-cmd-T15";
    let job = seed_command_job(&pool, job_name, "sleep 30").await;

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
        )
        .await
    });

    let run_id = wait_for_active_run(&active_runs, Duration::from_secs(5)).await;

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
        .expect("executor did not panic");
    assert_eq!(result.status, "stopped");

    // Scrape /metrics.
    let body = handle.render();

    // Expect a counter line for THIS job + stopped status. The metrics-
    // exporter-prometheus text format renders labels in key-sorted order,
    // so the canonical form is `{job="...",status="stopped"}`. We check
    // for the job-scoped sample with a positive count.
    let expected_prefix = format!(r#"cronduit_runs_total{{job="{job_name}",status="stopped"}}"#);
    let line = body
        .lines()
        .find(|l| l.starts_with(&expected_prefix))
        .unwrap_or_else(|| {
            panic!(
                "T-V11-STOP-15: expected /metrics to contain a line starting with {expected_prefix} \
                 after stopping a run; body was:\n{body}"
            )
        });
    // The line is `cronduit_runs_total{...} <number>`. Parse the trailing
    // number and assert it is at least 1.
    let value: f64 = line
        .rsplit_once(' ')
        .map(|(_, n)| n.trim().parse().unwrap_or(0.0))
        .unwrap_or(0.0);
    assert!(
        value >= 1.0,
        "T-V11-STOP-15: cronduit_runs_total{{job,status=stopped}} must be >= 1 after stopping \
         a run; got {value} (line: {line})"
    );

    // D-10 / Pitfall 1 regression lock: stopping a run must NOT increment
    // cronduit_run_failures_total for the same job. Find any line for this
    // job on the failures counter and assert it is absent OR zero.
    let failures_prefix = format!(r#"cronduit_run_failures_total{{job="{job_name}""#);
    for line in body.lines() {
        if line.starts_with(&failures_prefix) {
            let v: f64 = line
                .rsplit_once(' ')
                .map(|(_, n)| n.trim().parse().unwrap_or(0.0))
                .unwrap_or(0.0);
            assert!(
                v == 0.0,
                "T-V11-STOP-15 / Pitfall 1 regression: cronduit_run_failures_total \
                 must NOT be incremented for stopped runs; got {v} (line: {line})"
            );
        }
    }

    drop(cmd_tx);
    let _ = driver.await;
    pool.close().await;
}
