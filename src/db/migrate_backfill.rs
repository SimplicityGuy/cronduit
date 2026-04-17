//! Phase 11 per-job run-number backfill (DB-09, DB-10, DB-11, DB-12).
//!
//! Called from DbPool::migrate() AFTER sqlx::migrate!'s first pass applies
//! files 1 + 2 marker, and BEFORE the second pass (added in Plan 11-04)
//! applies file 3 to flip job_run_number to NOT NULL.
//!
//! Ordering strategy (post-revision): `DbPool::migrate` calls sqlx::migrate!
//! twice against the SAME directory, with this orchestrator between. On a
//! fresh install the first pass applies all three files (file 3 is vacuous
//! because there are no NULLs); the orchestrator's sentinel check then
//! short-circuits. On upgrade-in-place, the first pass applies files 1+2
//! (file 3 would fail, but sqlx applies in order — file 3 only lands in the
//! SECOND pass which runs AFTER the orchestrator has filled every NULL).
//! See src/db/mod.rs for the two-pass wiring (Plan 11-04 adds the second pass).
//!
//! The orchestrator loops 10k-row batches of `UPDATE job_runs SET job_run_number
//! = ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id ASC)`, emitting one
//! INFO log per batch (D-13), and finally resyncs jobs.next_run_number to
//! MAX(job_run_number) + 1 per job so post-backfill inserts continue without
//! gaps. A sentinel table `_v11_backfill_done` locks the O(1) fast-path on
//! re-runs.
//!
//! Idempotency: every step guards on `WHERE job_run_number IS NULL`. A partial
//! crash during batching restarts cleanly on the next process launch (D-14
//! fail-fast + container orchestrator restart).
//!
//! Concurrency (T-11-03-02): D-12 mandates backfill runs BEFORE the HTTP
//! listener binds, so no concurrent writers exist.

use crate::db::{DbPool, queries};

const BATCH_SIZE: i64 = 10_000;

pub async fn backfill_job_run_number(pool: &DbPool) -> anyhow::Result<()> {
    // Step 1 — O(1) sentinel fast-path.
    if queries::v11_backfill_sentinel_exists(pool).await? {
        tracing::info!(
            target: "cronduit.migrate",
            "job_run_number backfill: sentinel _v11_backfill_done present — skipping"
        );
        return Ok(());
    }

    // Step 2 — NULL count.
    let total = queries::count_job_runs_with_null_run_number(pool).await?;
    if total == 0 {
        tracing::info!(
            target: "cronduit.migrate",
            "job_run_number backfill: no rows need backfilling"
        );
        // Still run the resync so jobs without runs get next_run_number = 1 guaranteed.
        queries::resync_next_run_number(pool).await?;
        queries::v11_backfill_sentinel_mark_done(pool).await?;
        return Ok(());
    }

    tracing::info!(
        target: "cronduit.migrate",
        rows_total = total,
        batch_size = BATCH_SIZE,
        "job_run_number backfill: starting"
    );

    let overall_start = std::time::Instant::now();
    let mut done: i64 = 0;
    let mut batch_num: u64 = 0;
    // Manual ceiling divide — `i64::div_ceil` is still unstable on stable Rust 1.94.
    let batches_est = (total + BATCH_SIZE - 1) / BATCH_SIZE;

    // Step 3 — chunked loop.
    loop {
        let batch_start = std::time::Instant::now();
        let rows = queries::backfill_job_run_number_batch(pool, BATCH_SIZE)
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "job_run_number backfill failed at batch={} done={}: {}",
                    batch_num,
                    done,
                    e
                )
            })?;
        if rows == 0 {
            break;
        }
        done += rows;
        batch_num += 1;
        let pct = 100.0 * (done as f64) / (total.max(1) as f64);
        tracing::info!(
            target: "cronduit.migrate",
            batch = batch_num,
            batches_total = batches_est,
            rows_done = done,
            rows_total = total,
            pct = format!("{:.1}", pct),
            elapsed_ms = batch_start.elapsed().as_millis() as u64,
            "job_run_number backfill: batch"
        );
    }

    // Step 4 — resync counter so post-backfill inserts continue without gaps.
    queries::resync_next_run_number(pool).await?;

    // Step 5 — mark sentinel BEFORE logging complete, so if logging crashes the
    // sentinel is already durable.
    queries::v11_backfill_sentinel_mark_done(pool).await?;

    tracing::info!(
        target: "cronduit.migrate",
        total_rows = total,
        total_batches = batch_num,
        total_elapsed_ms = overall_start.elapsed().as_millis() as u64,
        "job_run_number backfill: complete"
    );
    Ok(())
}
