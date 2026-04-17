//! Phase 11 UI-20: LogLine id plumbing via Option A (RETURNING id).
//! Covers T-V11-LOG-01 + VALIDATION rows 11-07-01 / 11-07-02.
//!
//! insert_log_batch_returns_ids  — 11-07-01 (DB contract)
//! insert_log_batch_single_tx_per_batch — 11-07-02 (D-03 throughput gate)
//! broadcast_id_populated         — T-V11-LOG-01 (DB -> broadcast contract)

mod common;

use common::v11_fixtures::*;
use cronduit::db::queries::insert_log_batch;
use cronduit::scheduler::log_pipeline::LogLine;
use sqlx::Row;
use tokio::sync::broadcast;

/// VALIDATION 11-07-01: insert_log_batch returns a Vec<i64> with one id per
/// input line, in insert order, strictly monotonic, matching the actual
/// `job_logs.id` values on disk.
#[tokio::test]
async fn insert_log_batch_returns_ids() {
    let pool = setup_sqlite_with_phase11_migrations().await;
    let job_id = seed_test_job(&pool, "id-plumbing-job").await;
    let run_id = seed_running_run(&pool, job_id).await;

    let batch = make_test_batch(10);
    let ids = insert_log_batch(&pool, run_id, &batch)
        .await
        .expect("insert batch");
    assert_eq!(ids.len(), 10, "one id per input line");
    for w in ids.windows(2) {
        assert!(
            w[0] < w[1],
            "ids must be strictly monotonic in insert order: {:?}",
            ids
        );
    }

    // Confirm rows in the DB match input order at those exact ids.
    let p = match pool.reader() {
        cronduit::db::queries::PoolRef::Sqlite(pp) => pp,
        _ => panic!("sqlite-only fixture"),
    };
    let rows = sqlx::query("SELECT id, line FROM job_logs WHERE run_id = ?1 ORDER BY id ASC")
        .bind(run_id)
        .fetch_all(p)
        .await
        .unwrap();
    assert_eq!(rows.len(), 10, "10 rows persisted on disk");
    for (i, row) in rows.iter().enumerate() {
        let row_id: i64 = row.get("id");
        let row_line: String = row.get("line");
        assert_eq!(
            row_id, ids[i],
            "row #{i}: DB id must equal returned Vec<i64> at same index"
        );
        assert_eq!(
            row_line, batch[i].2,
            "row #{i}: DB line must equal input line at same index"
        );
    }
}

/// VALIDATION 11-07-02 / D-03 throughput gate: single-tx contract preserved.
///
/// A 1000-line batch must complete in < 500ms on in-memory SQLite. If it
/// crosses that threshold it's almost certainly because someone introduced a
/// per-line tx / fsync (the single `tx.begin()` + `tx.commit()` contract was
/// broken). This is a proxy for D-03 — the real benchmark
/// (tests/v11_log_dedupe_benchmark.rs) targets p95 < 50ms for 64-line
/// batches, which this 1000-line gate corroborates at a different scale.
#[tokio::test]
async fn insert_log_batch_single_tx_per_batch() {
    let pool = setup_sqlite_with_phase11_migrations().await;
    let job_id = seed_test_job(&pool, "perf-proxy-job").await;
    let run_id = seed_running_run(&pool, job_id).await;

    let batch = make_test_batch(1000);
    let t0 = std::time::Instant::now();
    let ids = insert_log_batch(&pool, run_id, &batch)
        .await
        .expect("insert batch");
    let elapsed = t0.elapsed();

    assert_eq!(ids.len(), 1000, "one id per input line");
    assert!(
        elapsed.as_millis() < 500,
        "1000-line batch took {}ms -- suggests per-line fsync (D-03 violated: \
         insert_log_batch no longer uses a single tx per call)",
        elapsed.as_millis()
    );
}

/// T-V11-LOG-01: the DB -> broadcast contract implemented in
/// `log_writer_task` (src/scheduler/run.rs). Rather than spinning up the full
/// scheduler (which owns `broadcast_tx` internally), we reproduce the
/// zip-with-ids step here against a test-local broadcast channel. If this
/// test fails it means either (a) insert_log_batch stopped returning ids 1:1
/// with the input batch, or (b) the zip + broadcast contract drifted.
///
/// The scheduler's own tests (scheduler::run::tests in src/scheduler/run.rs)
/// exercise the full end-to-end lifecycle including the actual
/// `log_writer_task`; this test locks the isolated contract that task
/// depends on.
#[tokio::test]
async fn broadcast_id_populated() {
    let pool = setup_sqlite_with_phase11_migrations().await;
    let job_id = seed_test_job(&pool, "broadcast-id-job").await;
    let run_id = seed_running_run(&pool, job_id).await;

    // Stand up a broadcast channel + subscribe a receiver BEFORE sending so
    // the sender's capacity accounting matches the scheduler's real path.
    let (broadcast_tx, mut broadcast_rx) = broadcast::channel::<LogLine>(64);

    // Small batch of 3 lines.
    let batch = make_test_batch(3);

    // Persist via insert_log_batch + capture returned Vec<i64>.
    let ids = insert_log_batch(&pool, run_id, &batch)
        .await
        .expect("insert batch");
    assert_eq!(ids.len(), 3, "setup invariant: one id per input line");

    // Mirror log_writer_task's zip step (Task 3 of this plan):
    // build LogLine { id: None, .. } from input, then zip with ids and
    // send each LogLine { id: Some(id), .. } to the broadcast channel.
    let lines: Vec<LogLine> = batch
        .iter()
        .map(|(stream, ts, line)| LogLine {
            id: None, // pre-broadcast value; replaced below via zip
            stream: stream.clone(),
            ts: ts.clone(),
            line: line.clone(),
        })
        .collect();
    for (line, id) in lines.into_iter().zip(ids.clone().into_iter()) {
        broadcast_tx
            .send(LogLine {
                id: Some(id),
                ..line
            })
            .expect("send to subscribed receiver");
    }
    drop(broadcast_tx); // close so recv eventually returns Err(Closed).

    // Drain the receiver -- must get exactly 3 LogLines, each with
    // id.is_some() AND those ids MUST equal the Vec<i64> returned by
    // insert_log_batch, in order.
    let mut received_ids: Vec<i64> = Vec::new();
    while let Ok(line) = broadcast_rx.recv().await {
        assert!(
            line.id.is_some(),
            "every broadcast LogLine MUST carry id = Some(_) per D-01: {:?}",
            line
        );
        received_ids.push(line.id.unwrap());
    }
    assert_eq!(received_ids.len(), 3, "received all 3 lines before close");
    assert_eq!(
        received_ids, ids,
        "broadcast ids must match insert_log_batch's Vec<i64> in order"
    );
}
