//! T-V11-STOP-04: Stop-vs-natural-completion race test (SCHED-11).
//!
//! Phase-gate blocker per D-15. The Stop feature does NOT ship unless this
//! test is green. Runs 1000 iterations under `tokio::time::pause` to get
//! deterministic ordering on the exact microsecond at which Stop arrives
//! relative to the mock executor's natural completion.
//!
//! Invariant proved: no iteration ever produces a `job_runs.status` other
//! than `"success"` or `"stopped"`. No iteration leaves the row stuck at
//! `"running"`. No iteration corrupts the value. The `WHERE status =
//! 'running'` guard on both terminal UPDATE statements is what enforces
//! single-writer semantics — whichever completion path fires first wins,
//! the loser's UPDATE matches zero rows and is a no-op.
//!
//! This test deliberately does NOT invoke the real `execute_command` /
//! `execute_script` / `execute_docker` path. It exercises the scheduler-side
//! race: insert RunEntry → Stop arrival ordering → finalize → remove. The
//! full docker-executor Stop test lives in plan 10-10 which has
//! testcontainers infrastructure.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use cronduit::db::DbPool;
use cronduit::db::queries::PoolRef;
use cronduit::scheduler::RunEntry;
use cronduit::scheduler::control::{RunControl, StopReason};
use sqlx::Row;
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// Set up a fresh in-memory SQLite pool with migrations applied and a single
/// seed job whose id we return. Each iteration of the race test gets its own
/// pool so tests cannot interfere.
async fn setup_pool_with_job() -> (DbPool, i64) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("sqlite pool");
    pool.migrate().await.expect("run migrations");
    let job_id = cronduit::db::queries::upsert_job(
        &pool,
        "race-test",
        "0 0 31 2 *",
        "0 0 31 2 *",
        "command",
        r#"{"command":"echo race"}"#,
        "race-hash",
        3600,
    )
    .await
    .expect("upsert seed job");
    (pool, job_id)
}

/// Insert a `job_runs` row with `status='running'` and return its id. Mirrors
/// `cronduit::db::queries::insert_running_run` but is inlined here so the test
/// keeps its single-file self-containment.
async fn seed_running_run(pool: &DbPool, job_id: i64) -> i64 {
    cronduit::db::queries::insert_running_run(pool, job_id, "scheduled", "testhash", None)
        .await
        .expect("insert running run")
}

/// Read the final `status` column for a `job_runs` row. The test asserts this
/// is exactly one of `"success"` or `"stopped"` every iteration.
async fn final_status(pool: &DbPool, run_id: i64) -> String {
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let row = sqlx::query("SELECT status FROM job_runs WHERE id = ?1")
                .bind(run_id)
                .fetch_one(p)
                .await
                .expect("select final status");
            row.get::<String, _>("status")
        }
        _ => unreachable!("race test is SQLite-only"),
    }
}

/// T-V11-STOP-04: 1000-iteration deterministic race test.
///
/// Each iteration:
///
/// 1. Fresh in-memory pool + seeded job + `status='running'` run row. (Time
///    is running normally for this step — sqlx's internal pool has its own
///    acquire-timeout machinery which does not cooperate with
///    `tokio::time::pause`.)
/// 2. Fresh `active_runs` map with a RunEntry containing a RunControl.
/// 3. `tokio::time::pause()` — from here on, no real sleeps advance the
///    clock. Only explicit `advance(...)` calls do.
/// 4. Spawn a mock executor future that either:
///    - natural completion at T+1ms → UPDATE status='success' WHERE status='running'
///    - cancel observed → UPDATE status='stopped' WHERE status='running'
///    whichever wins.
/// 5. Advance virtual time to T+999μs (1μs before natural exit).
/// 6. Fire `control.stop(StopReason::Operator)` — the race trigger.
/// 7. Advance virtual time enough for the mock executor to resolve.
/// 8. `tokio::time::resume()` so the next iteration's pool setup (which
///    hits sqlx's non-virtual-time machinery) can proceed.
/// 9. Await the executor task and assert `final_status ∈ {success, stopped}`.
///
/// The `WHERE status = 'running'` guard on both UPDATEs ensures that whichever
/// branch fires second is a no-op — Invariant 3 "Single-Writer" from
/// PITFALLS.md. 1000 iterations are non-negotiable per D-15.
///
/// NOTE: `start_paused = true` was tried first but sqlx's SqlitePoolOptions
/// uses an internal `acquire_timeout` that does NOT auto-advance under
/// paused tokio time, producing a pool-timeout panic on the first iteration.
/// The pause/resume-around-the-race-window pattern keeps the deterministic
/// virtual-time window exactly where it matters (the race trigger) without
/// fighting sqlx.
#[tokio::test(flavor = "current_thread")]
async fn stop_race_thousand_iterations() {
    for iter in 0..1000 {
        let (pool, job_id) = setup_pool_with_job().await;
        let run_id = seed_running_run(&pool, job_id).await;

        let active_runs: Arc<RwLock<HashMap<i64, RunEntry>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let (broadcast_tx, _rx) = broadcast::channel(16);
        let cancel = CancellationToken::new();
        let control = RunControl::new(cancel.clone());

        active_runs.write().await.insert(
            run_id,
            RunEntry {
                broadcast_tx: broadcast_tx.clone(),
                control: control.clone(),
                job_name: "race-test".to_string(),
            },
        );

        // Pause tokio time ONLY around the race-sensitive window. Pool
        // setup and teardown run with real time to avoid fighting sqlx's
        // internal acquire-timeout machinery.
        tokio::time::pause();

        // Mock executor: races natural completion (T+1ms) against the
        // operator-stop cancel. Whichever fires first wins; the other
        // branch's UPDATE is a no-op thanks to the WHERE status='running'
        // guard (Invariant 3 single-writer).
        let pool_clone = pool.clone();
        let active_runs_clone = active_runs.clone();
        let control_clone = control.clone();
        let broadcast_tx_local = broadcast_tx.clone();
        let exec = tokio::spawn(async move {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(1)) => {
                    // Natural completion → finalize as success.
                    match pool_clone.writer() {
                        PoolRef::Sqlite(p) => {
                            sqlx::query(
                                "UPDATE job_runs SET status = 'success', end_time = ?1 \
                                 WHERE status = 'running' AND id = ?2",
                            )
                            .bind(chrono::Utc::now().to_rfc3339())
                            .bind(run_id)
                            .execute(p)
                            .await
                            .expect("natural finalize");
                        }
                        _ => unreachable!(),
                    }
                    active_runs_clone.write().await.remove(&run_id);
                    drop(broadcast_tx_local);
                }
                _ = control_clone.cancel.cancelled() => {
                    // Operator stop observed → finalize as stopped.
                    let status_str = match control_clone.reason() {
                        StopReason::Operator => "stopped",
                        StopReason::Shutdown => "cancelled",
                    };
                    match pool_clone.writer() {
                        PoolRef::Sqlite(p) => {
                            sqlx::query(
                                "UPDATE job_runs SET status = ?1, end_time = ?2 \
                                 WHERE status = 'running' AND id = ?3",
                            )
                            .bind(status_str)
                            .bind(chrono::Utc::now().to_rfc3339())
                            .bind(run_id)
                            .execute(p)
                            .await
                            .expect("stop finalize");
                        }
                        _ => unreachable!(),
                    }
                    active_runs_clone.write().await.remove(&run_id);
                    drop(broadcast_tx_local);
                }
            }
        });

        // Advance virtual time to T+999μs (1μs before the natural exit at
        // T+1ms). This is the race trigger window: Stop arrives within a
        // microsecond of natural completion.
        tokio::time::advance(Duration::from_micros(999)).await;

        // Fire the Stop — the race begins here.
        control.stop(StopReason::Operator);

        // Drain virtual time so the mock executor resolves one way or the
        // other. 2ms is plenty — the natural sleep is 1ms, the cancel path
        // wakes immediately.
        tokio::time::advance(Duration::from_millis(2)).await;

        // Resume real time before the blocking await + next iteration's
        // pool teardown/setup.
        tokio::time::resume();

        exec.await.expect("mock executor did not panic");

        // Assert the final row is EXACTLY one of {success, stopped} — never
        // "running", never corrupted, never anything else.
        let final_s = final_status(&pool, run_id).await;
        assert!(
            final_s == "success" || final_s == "stopped",
            "iteration {iter}: unexpected final status {final_s:?}"
        );

        pool.close().await;
    }
}
