//! Daily retention pruner (DB-08).
//!
//! Deletes job_logs and job_runs older than `[server].log_retention` in batches
//! to avoid SQLite write contention (Pitfall 11).

use crate::db::{DbPool, queries};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const BATCH_SIZE: i64 = 1000;
const BATCH_SLEEP: Duration = Duration::from_millis(100);
const WAL_CHECKPOINT_THRESHOLD: i64 = 10_000;

/// Spawn the retention pruner as a background task.
/// Runs on a 24-hour interval from startup (not wall-clock aligned).
/// Checks `CancellationToken` between batches for graceful shutdown.
pub async fn retention_pruner(pool: DbPool, retention: Duration, cancel: CancellationToken) {
    // GAP-2 fix (06-06-PLAN.md): emit a boot-time log on target cronduit.retention
    // so operators can confirm from startup logs that retention is wired up, without
    // waiting 24h for the first prune cycle to fire its own log line.
    tracing::info!(
        target: "cronduit.retention",
        retention_secs = retention.as_secs(),
        "retention pruner started"
    );

    let mut interval = tokio::time::interval(Duration::from_secs(86400));
    // Skip the initial immediate tick -- first prune after 24 hours.
    interval.tick().await;

    loop {
        tokio::select! {
            _ = interval.tick() => {
                run_prune_cycle(&pool, retention, &cancel).await;
            }
            _ = cancel.cancelled() => {
                tracing::info!(target: "cronduit.retention", "retention_pruner shutting down");
                break;
            }
        }
    }
}

async fn run_prune_cycle(pool: &DbPool, retention: Duration, cancel: &CancellationToken) {
    let cutoff = chrono::Utc::now()
        - chrono::Duration::from_std(retention).unwrap_or(chrono::Duration::days(90));
    let cutoff_str = cutoff.to_rfc3339();

    tracing::info!(
        target: "cronduit.retention",
        cutoff = %cutoff_str,
        "retention prune cycle started"
    );

    // Phase 1: Delete old log lines (children first for FK safety).
    let mut total_logs_deleted: i64 = 0;
    loop {
        if cancel.is_cancelled() {
            tracing::warn!(
                target: "cronduit.retention",
                logs_deleted = total_logs_deleted,
                "prune interrupted by shutdown"
            );
            return;
        }
        match queries::delete_old_logs_batch(pool, &cutoff_str, BATCH_SIZE).await {
            Ok(deleted) => {
                total_logs_deleted += deleted;
                if deleted > 0 {
                    tracing::debug!(
                        target: "cronduit.retention",
                        deleted,
                        total = total_logs_deleted,
                        "prune_batch: logs"
                    );
                }
                if deleted < BATCH_SIZE {
                    break;
                }
                tokio::time::sleep(BATCH_SLEEP).await;
            }
            Err(e) => {
                tracing::error!(
                    target: "cronduit.retention",
                    error = %e,
                    "retention prune: failed to delete log batch"
                );
                break;
            }
        }
    }

    // Phase 2: Delete orphaned job_runs (no remaining logs).
    let mut total_runs_deleted: i64 = 0;
    loop {
        if cancel.is_cancelled() {
            tracing::warn!(
                target: "cronduit.retention",
                logs_deleted = total_logs_deleted,
                runs_deleted = total_runs_deleted,
                "prune interrupted by shutdown"
            );
            return;
        }
        match queries::delete_old_runs_batch(pool, &cutoff_str, BATCH_SIZE).await {
            Ok(deleted) => {
                total_runs_deleted += deleted;
                if deleted > 0 {
                    tracing::debug!(
                        target: "cronduit.retention",
                        deleted,
                        total = total_runs_deleted,
                        "prune_batch: runs"
                    );
                }
                if deleted < BATCH_SIZE {
                    break;
                }
                tokio::time::sleep(BATCH_SLEEP).await;
            }
            Err(e) => {
                tracing::error!(
                    target: "cronduit.retention",
                    error = %e,
                    "retention prune: failed to delete run batch"
                );
                break;
            }
        }
    }

    // Phase 3: WAL checkpoint if large prune (SQLite only).
    let total_deleted = total_logs_deleted + total_runs_deleted;
    if total_deleted > WAL_CHECKPOINT_THRESHOLD {
        tracing::info!(
            target: "cronduit.retention",
            total_deleted,
            "issuing WAL checkpoint after large prune"
        );
        if let Err(e) = queries::wal_checkpoint(pool).await {
            tracing::error!(
                target: "cronduit.retention",
                error = %e,
                "WAL checkpoint failed"
            );
        }
    }

    tracing::info!(
        target: "cronduit.retention",
        logs_deleted = total_logs_deleted,
        runs_deleted = total_runs_deleted,
        "retention prune cycle completed"
    );
}
