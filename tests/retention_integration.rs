//! Retention pruner integration tests (DB-08).
//!
//! Tests validate:
//! - Retention pruner emits a boot-time tracing log so operators can confirm
//!   retention is wired up without waiting for the first 24h interval tick
//!   (GAP-2 closure in 06-06-PLAN.md).
//! - Future: batched deletes, FK safety, WAL checkpoint, cancellation.

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::instrument::WithSubscriber;
use tracing_subscriber::fmt::MakeWriter;

use cronduit::db::DbPool;
use cronduit::scheduler::retention::retention_pruner;

#[derive(Clone, Default)]
struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

impl io::Write for CapturedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for CapturedWriter {
    type Writer = Self;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// GAP-2 closure test: after `retention_pruner()` is spawned, the first thing it
/// MUST do — before the 24-hour interval loop starts skipping its initial tick —
/// is emit exactly one `tracing::info!` line on target `cronduit.retention` with
/// message `"retention pruner started"`. Prior to 06-06-PLAN.md the function
/// silently started its interval loop and operators only saw a cronduit.retention
/// log line ~24h later when `run_prune_cycle` first ran.
#[tokio::test]
async fn retention_pruner_emits_startup_log_on_spawn() {
    let captured = CapturedWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(captured.clone())
        .with_max_level(tracing::Level::INFO)
        .with_target(true)
        .finish();

    // In-memory SQLite pool is sufficient — retention_pruner does not touch any
    // tables until its first interval tick, which we cancel long before it fires.
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("open in-memory sqlite pool");

    let cancel = CancellationToken::new();
    let cancel_for_task = cancel.clone();

    // Attach the capturing subscriber to the pruner future via `WithSubscriber`
    // so every tracing event emitted inside the future (including from a worker
    // thread) is routed to our capture buffer. This is the canonical
    // tracing+tokio test pattern: `with_default` only sets the subscriber for
    // the current thread synchronously and does not survive across `.await`
    // points, so we use the future-attached dispatch instead.
    let pruner_future = async move {
        retention_pruner(pool, Duration::from_secs(90 * 24 * 3600), cancel_for_task).await;
    }
    .with_subscriber(subscriber);

    let handle = tokio::spawn(pruner_future);

    // Give the task ~50ms to emit its startup log, then cancel so it exits cleanly.
    tokio::time::sleep(Duration::from_millis(50)).await;
    cancel.cancel();

    // Bound the join so a regression can't hang the suite.
    tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .expect("retention_pruner did not exit within 5s of cancel")
        .expect("retention_pruner task panicked");

    let output = String::from_utf8(captured.0.lock().unwrap().clone())
        .expect("captured tracing output is not valid utf8");

    assert!(
        output.contains("cronduit.retention"),
        "expected target `cronduit.retention` in captured tracing output, got: {output}"
    );
    assert!(
        output.contains("retention pruner started"),
        "expected message `retention pruner started` in captured tracing output, got: {output}"
    );
}

#[cfg(test)]
mod retention_tests {
    // TODO: Import test helpers, DbPool, in-memory SQLite setup

    #[tokio::test]
    #[ignore = "not yet implemented"]
    async fn retention_deletes_old_logs_in_batches() {
        // Setup: create in-memory SQLite, insert 2500 old log rows
        // Act: call delete_old_logs_batch in a loop with batch_size=1000
        // Assert: first batch returns 1000, second returns 1000, third returns 500
        todo!("Implement batched log deletion test")
    }

    #[tokio::test]
    #[ignore = "not yet implemented"]
    async fn retention_deletes_runs_after_logs_removed() {
        // Setup: create in-memory SQLite, insert old runs with logs
        // Act: delete logs first, then delete runs
        // Assert: runs with no remaining logs are deleted, runs with logs are kept
        todo!("Implement FK-safe run deletion test")
    }

    #[tokio::test]
    #[ignore = "not yet implemented"]
    async fn retention_respects_cutoff_date() {
        // Setup: insert runs at various dates (30d, 60d, 100d ago)
        // Act: prune with 90d retention
        // Assert: only 100d run is deleted, 30d and 60d are kept
        todo!("Implement cutoff date test")
    }

    #[tokio::test]
    #[ignore = "not yet implemented"]
    async fn retention_wal_checkpoint_fires_after_threshold() {
        // Setup: create in-memory SQLite, insert >10000 old rows
        // Act: run full prune cycle
        // Assert: WAL checkpoint is called (verify via tracing or side effect)
        todo!("Implement WAL checkpoint threshold test")
    }

    #[tokio::test]
    #[ignore = "not yet implemented"]
    async fn retention_cancellation_stops_prune() {
        // Setup: create in-memory SQLite with many old rows
        // Act: cancel the CancellationToken after first batch
        // Assert: prune cycle stops, not all rows deleted
        todo!("Implement cancellation test")
    }
}
