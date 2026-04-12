//! Config reload infrastructure: do_reload(), do_reroll(), and file watcher.
//!
//! D-09: All reload trigger sources (SIGHUP, file-watch, API) funnel through
//! do_reload(), which parses config, syncs to DB, and rebuilds the fire heap.
//! RELOAD-04: Failed reloads leave the running config untouched.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};

use crate::config;
use crate::db::DbPool;
use crate::db::queries::DbJob;
use crate::scheduler::cmd::{ReloadResult, ReloadStatus, SchedulerCmd};
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
    let random_min_gap = parsed
        .config
        .defaults
        .as_ref()
        .and_then(|d| d.random_min_gap)
        .unwrap_or(std::time::Duration::from_secs(0));
    match sync::sync_config_to_db(pool, &parsed.config, random_min_gap).await {
        Ok(sync_result) => {
            tracing::info!(
                target: "cronduit.reload",
                added = sync_result.inserted,
                updated = sync_result.updated,
                disabled = sync_result.disabled,
                "config reload successful"
            );

            // 3. Rebuild in-memory jobs map (RELOAD-06: in-flight runs hold clones)
            *jobs = sync_result.jobs.iter().map(|j| (j.id, j.clone())).collect();

            // 4. Rebuild fire heap with new job set
            let new_heap = fire::build_initial_heap(&sync_result.jobs, tz);

            (
                ReloadResult {
                    status: ReloadStatus::Ok,
                    added: sync_result.inserted,
                    updated: sync_result.updated,
                    disabled: sync_result.disabled,
                    unchanged: sync_result.unchanged,
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
            );
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
            );
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

    // Resolve a fresh @random schedule (RAND-03c: explicit re-randomize)
    let new_resolved = {
        let mut rng = rand::thread_rng();
        crate::scheduler::random::resolve_schedule(&job.schedule, None, &mut rng)
    };

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
            } else {
                tracing::warn!(
                    target: "cronduit.reload",
                    job_id,
                    "reroll: job not in in-memory map; DB updated but scheduler will use stale schedule until next reload"
                );
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

/// Spawn a file watcher task for the config file (D-09, D-10, RELOAD-03).
///
/// Uses manual tokio debounce (500ms) per D-09. Editor atomic saves
/// (write-then-rename) generate multiple events that the debounce absorbs.
///
/// D-10: Logs startup message "watching config file for changes".
/// Pitfall 6: Keeps the `Watcher` alive inside the spawned task.
pub fn spawn_file_watcher(
    config_path: PathBuf,
    cmd_tx: tokio::sync::mpsc::Sender<SchedulerCmd>,
) -> anyhow::Result<()> {
    let (notify_tx, mut notify_rx) =
        tokio::sync::mpsc::channel::<notify::Result<notify::Event>>(16);

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = notify_tx.blocking_send(res);
        },
        NotifyConfig::default(),
    )?;

    // Watch the parent directory to catch rename-based atomic saves.
    let watch_path = config_path.parent().unwrap_or(&config_path).to_path_buf();
    watcher.watch(&watch_path, RecursiveMode::NonRecursive)?;

    let config_filename = config_path
        .file_name()
        .map(|f| f.to_os_string())
        .unwrap_or_default();

    tracing::info!(
        target: "cronduit.reload",
        path = %config_path.display(),
        "watching config file for changes"
    );

    tokio::spawn(async move {
        let _watcher = watcher; // Keep watcher alive (Pitfall 6)
        let mut debounce_active = false;

        loop {
            tokio::select! {
                Some(event_result) = notify_rx.recv() => {
                    if let Ok(event) = event_result {
                        // Only trigger on events that affect our config file.
                        let affects_config = event.paths.iter().any(|p| {
                            p.file_name().map(|f| f == config_filename).unwrap_or(false)
                        });
                        if affects_config {
                            debounce_active = true;
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(500)), if debounce_active => {
                    debounce_active = false;
                    tracing::debug!(
                        target: "cronduit.reload",
                        "file change detected, triggering reload"
                    );
                    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                    if cmd_tx
                        .send(SchedulerCmd::Reload { response_tx: resp_tx })
                        .await
                        .is_err()
                    {
                        tracing::debug!(
                            target: "cronduit.reload",
                            "scheduler channel closed, stopping file watcher"
                        );
                        break;
                    }
                    // Log the result so file-watch reload outcomes are observable.
                    match resp_rx.await {
                        Ok(result) if result.status == ReloadStatus::Error => {
                            tracing::warn!(
                                target: "cronduit.reload",
                                error = ?result.error_message,
                                "file-watch triggered reload failed"
                            );
                        }
                        Err(_) => {
                            tracing::debug!(
                                target: "cronduit.reload",
                                "file-watch reload response channel dropped"
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    Ok(())
}
