//! T-V11-LOG-02 benchmark — gates Option A per CONTEXT.md D-02.
//!
//! Invariant: p95 insert latency for a 64-line batch against in-memory SQLite
//! must be < 50ms. If this fails on the CI runner, Phase 11 flips to Option B
//! (monotonic seq: u64 column on LogLine + nullable seq column on job_logs)
//! and this plan's SUMMARY.md records the flip.

mod common;
use common::v11_fixtures::*;

use cronduit::db::queries::insert_log_batch;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn p95_under_50ms() {
    let pool = setup_sqlite_with_phase11_migrations().await;
    let job_id = seed_test_job(&pool, "bench-job").await;
    let run_id = seed_running_run(&pool, job_id).await;

    const ITERS: usize = 100;
    const BATCH_SIZE: usize = 64;
    let mut durations_us: Vec<u64> = Vec::with_capacity(ITERS);

    // Warmup — discard first 5 iterations to let SQLite's WAL / pragmas settle.
    for _ in 0..5 {
        let batch = make_test_batch(BATCH_SIZE);
        insert_log_batch(&pool, run_id, &batch).await.unwrap();
    }

    for _ in 0..ITERS {
        let batch = make_test_batch(BATCH_SIZE);
        let t0 = std::time::Instant::now();
        insert_log_batch(&pool, run_id, &batch)
            .await
            .expect("insert batch");
        durations_us.push(t0.elapsed().as_micros() as u64);
    }

    durations_us.sort_unstable();
    let p50 = durations_us[ITERS / 2];
    let p95 = durations_us[(ITERS * 95) / 100];
    let p99 = durations_us[(ITERS * 99) / 100];
    let mean: u64 = durations_us.iter().sum::<u64>() / ITERS as u64;

    eprintln!(
        "T-V11-LOG-02 benchmark (64-line batch × {} iters):\n  \
         mean = {}us  p50 = {}us  p95 = {}us  p99 = {}us",
        ITERS, mean, p50, p95, p99
    );

    assert!(
        p95 < 50_000,
        "p95 = {}us exceeds 50ms budget (T-V11-LOG-02). \
         Phase 11 must flip to Option B (monotonic seq column) per CONTEXT.md D-02. \
         Full summary: mean={}us p50={}us p95={}us p99={}us",
        p95,
        mean,
        p50,
        p95,
        p99
    );
}
