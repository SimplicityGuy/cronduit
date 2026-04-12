//! Retention pruner integration tests (DB-08).
//!
//! Tests validate:
//! - Batched deletes remove old logs and runs
//! - Batch size is respected (1000 per batch)
//! - FK safety: logs deleted before runs
//! - WAL checkpoint fires after threshold
//! - CancellationToken interrupts prune cycle

#[cfg(test)]
mod retention_tests {
    // TODO: Import test helpers, DbPool, in-memory SQLite setup

    #[tokio::test]
    async fn retention_deletes_old_logs_in_batches() {
        // Setup: create in-memory SQLite, insert 2500 old log rows
        // Act: call delete_old_logs_batch in a loop with batch_size=1000
        // Assert: first batch returns 1000, second returns 1000, third returns 500
        todo!("Implement batched log deletion test")
    }

    #[tokio::test]
    async fn retention_deletes_runs_after_logs_removed() {
        // Setup: create in-memory SQLite, insert old runs with logs
        // Act: delete logs first, then delete runs
        // Assert: runs with no remaining logs are deleted, runs with logs are kept
        todo!("Implement FK-safe run deletion test")
    }

    #[tokio::test]
    async fn retention_respects_cutoff_date() {
        // Setup: insert runs at various dates (30d, 60d, 100d ago)
        // Act: prune with 90d retention
        // Assert: only 100d run is deleted, 30d and 60d are kept
        todo!("Implement cutoff date test")
    }

    #[tokio::test]
    async fn retention_wal_checkpoint_fires_after_threshold() {
        // Setup: create in-memory SQLite, insert >10000 old rows
        // Act: run full prune cycle
        // Assert: WAL checkpoint is called (verify via tracing or side effect)
        todo!("Implement WAL checkpoint threshold test")
    }

    #[tokio::test]
    async fn retention_cancellation_stops_prune() {
        // Setup: create in-memory SQLite with many old rows
        // Act: cancel the CancellationToken after first batch
        // Assert: prune cycle stops, not all rows deleted
        todo!("Implement cancellation test")
    }
}
