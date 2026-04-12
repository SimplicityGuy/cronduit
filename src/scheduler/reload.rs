//! Config reload infrastructure: do_reload(), do_reroll(), and file watcher.
//!
//! D-09: All reload trigger sources (SIGHUP, file-watch, API) funnel through
//! do_reload(), which parses config, syncs to DB, and rebuilds the fire heap.
//! RELOAD-04: Failed reloads leave the running config untouched.

use std::collections::BinaryHeap;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;

use crate::config;
use crate::db::DbPool;
use crate::db::queries::DbJob;
use crate::scheduler::cmd::{ReloadResult, ReloadStatus};
use crate::scheduler::fire;
use crate::scheduler::sync;
use chrono_tz::Tz;

/// Execute a config reload: parse -> validate -> sync to DB -> rebuild heap.
///
/// On parse/validate failure, returns ReloadResult with status=Error and the running
/// config is not touched (RELOAD-04). On success, updates `jobs` HashMap and returns
/// a new fire heap (RELOAD-05, RELOAD-06, RELOAD-07).
///
/// In-flight runs are not affected because they hold cloned DbJob values (RELOAD-06).
pub async fn do_reload(
    pool: &DbPool,
    config_path: &Path,
    jobs: &mut HashMap<i64, DbJob>,
    tz: Tz,
) -> (ReloadResult, Option<BinaryHeap<Reverse<fire::FireEntry>>>) {
    // 1. Parse and validate (RELOAD-04: failure leaves running config untouched)
    let parsed = match config::parse_and_validate(config_path) {
        Ok(p) => p,
        Err(errors) => {
            let msg = errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            tracing::error!(target: "cronduit.reload", error = %msg, "config reload failed: parse error");
            return (
                ReloadResult {
                    status: ReloadStatus::Error,
                    added: 0,
                    updated: 0,
                    disabled: 0,
                    unchanged: 0,
                    error_message: Some(msg),
                },
                None,
            );
        }
    };

    // 2. Sync to DB (creates/updates/disables) (RELOAD-05, RELOAD-07)
    match sync::sync_config_to_db(pool, &parsed.config).await {
        Ok(sync_result) => {
            tracing::info!(
                target: "cronduit.reload",
                added = sync_result.inserted,
                updated = sync_result.updated,
                disabled = sync_result.disabled,
                "config reload successful"
            );

            // 3. Rebuild in-memory jobs map (RELOAD-06: in-flight runs hold clones)
            *jobs = sync_result
                .jobs
                .iter()
                .map(|j| (j.id, j.clone()))
                .collect();

            // 4. Rebuild fire heap with new job set
            let new_heap = fire::build_initial_heap(&sync_result.jobs, tz);

            (
                ReloadResult {
                    status: ReloadStatus::Ok,
                    added: sync_result.inserted,
                    updated: sync_result.updated,
                    disabled: sync_result.disabled,
                    unchanged: 0, // TODO: track unchanged count in SyncResult
                    error_message: None,
                },
                Some(new_heap),
            )
        }
        Err(e) => {
            let msg = format!("DB sync failed: {e}");
            tracing::error!(target: "cronduit.reload", error = %msg, "config reload failed");
            (
                ReloadResult {
                    status: ReloadStatus::Error,
                    added: 0,
                    updated: 0,
                    disabled: 0,
                    unchanged: 0,
                    error_message: Some(msg),
                },
                None,
            )
        }
    }
}

/// Re-roll the @random schedule for a specific job (D-06).
///
/// Returns error if the job doesn't exist or doesn't have @random in its schedule.
/// Placeholder: @random resolution will be implemented by Plan 01.
pub async fn do_reroll(
    pool: &DbPool,
    job_id: i64,
    jobs: &mut HashMap<i64, DbJob>,
    tz: Tz,
) -> (ReloadResult, Option<BinaryHeap<Reverse<fire::FireEntry>>>) {
    use crate::db::queries;

    let job = match queries::get_job_by_id(pool, job_id).await {
        Ok(Some(j)) => j,
        Ok(None) => {
            return (
                ReloadResult {
                    status: ReloadStatus::Error,
                    added: 0,
                    updated: 0,
                    disabled: 0,
                    unchanged: 0,
                    error_message: Some("Job not found".to_string()),
                },
                None,
            )
        }
        Err(e) => {
            return (
                ReloadResult {
                    status: ReloadStatus::Error,
                    added: 0,
                    updated: 0,
                    disabled: 0,
                    unchanged: 0,
                    error_message: Some(format!("DB error: {e}")),
                },
                None,
            )
        }
    };

    // Check if job has @random fields
    if !job.schedule.contains("@random") {
        return (
            ReloadResult {
                status: ReloadStatus::Error,
                added: 0,
                updated: 0,
                disabled: 0,
                unchanged: 0,
                error_message: Some("This job has no @random schedule".to_string()),
            },
            None,
        );
    }

    // For now, just use the existing resolved schedule as-is.
    // Plan 01 will add the random module with proper @random resolution.
    // When that lands, this will call random::resolve_schedule().
    let new_resolved = job.resolved_schedule.clone();

    // Update DB
    match queries::update_resolved_schedule(pool, job_id, &new_resolved).await {
        Ok(()) => {
            tracing::info!(
                target: "cronduit.reload",
                job_id,
                job_name = %job.name,
                resolved = %new_resolved,
                "schedule re-rolled"
            );

            // Update in-memory map
            if let Some(mem_job) = jobs.get_mut(&job_id) {
                mem_job.resolved_schedule = new_resolved;
            }

            // Rebuild fire heap
            let all_jobs: Vec<DbJob> = jobs.values().cloned().collect();
            let new_heap = fire::build_initial_heap(&all_jobs, tz);

            (
                ReloadResult {
                    status: ReloadStatus::Ok,
                    added: 0,
                    updated: 1,
                    disabled: 0,
                    unchanged: 0,
                    error_message: None,
                },
                Some(new_heap),
            )
        }
        Err(e) => (
            ReloadResult {
                status: ReloadStatus::Error,
                added: 0,
                updated: 0,
                disabled: 0,
                unchanged: 0,
                error_message: Some(format!("DB error: {e}")),
            },
            None,
        ),
    }
}
