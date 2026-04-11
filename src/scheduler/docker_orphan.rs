//! Orphan container reconciliation at startup (SCHED-08).
//!
//! On startup, finds all containers labeled with `cronduit.run_id`,
//! stops running ones (10s grace), removes all, and marks DB rows as orphaned.
//!
//! D-07: Running orphans stopped with 10s SIGTERM grace.
//! D-08: Stopped orphans also removed.
//! D-09: Each orphan logged individually at WARN level.

use std::collections::HashMap;

use bollard::Docker;
use bollard::models::ContainerSummaryStateEnum;
use bollard::query_parameters::{
    ListContainersOptionsBuilder, RemoveContainerOptions, StopContainerOptions,
};

use crate::db::DbPool;
use crate::db::queries::PoolRef;

/// Reconcile orphan containers from a previous Cronduit run.
///
/// Finds all containers with the `cronduit.run_id` label, stops running ones
/// (10s SIGTERM grace per D-07), removes all (D-08), and marks their DB rows
/// as `status=error` with `error_message="orphaned at restart"`.
///
/// Returns the number of orphan containers reconciled.
pub async fn reconcile_orphans(docker: &Docker, pool: &DbPool) -> anyhow::Result<u32> {
    // Build filter: all containers (including stopped) with our label.
    let mut filters = HashMap::new();
    filters.insert("label".to_string(), vec!["cronduit.run_id".to_string()]);

    let options = ListContainersOptionsBuilder::default()
        .all(true) // D-08: include stopped containers
        .filters(&filters)
        .build();

    let containers = docker.list_containers(Some(options)).await?;
    let mut reconciled_count: u32 = 0;

    for container in &containers {
        let container_id = container.id.clone().unwrap_or_default();
        if container_id.is_empty() {
            continue;
        }

        let labels = container.labels.clone().unwrap_or_default();
        let run_id_str = labels.get("cronduit.run_id").cloned().unwrap_or_default();
        let job_name = labels
            .get("cronduit.job_name")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        // D-07: Stop running containers with 10s grace.
        let is_running = container.state == Some(ContainerSummaryStateEnum::RUNNING);
        if is_running {
            docker
                .stop_container(
                    &container_id,
                    Some(StopContainerOptions {
                        t: Some(10),
                        ..Default::default()
                    }),
                )
                .await
                .ok();
        }

        // D-08: Remove all orphan containers (both previously-running and stopped).
        docker
            .remove_container(
                &container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
            .ok();

        // Mark DB row as orphaned if run_id parses.
        if let Ok(run_id) = run_id_str.parse::<i64>() {
            mark_run_orphaned(pool, run_id).await.ok();
        }

        // D-09: Log each orphan individually at WARN.
        tracing::warn!(
            target: "cronduit.reconcile",
            container_id = %container_id,
            job_name = %job_name,
            run_id = %run_id_str,
            container_state = ?container.state,
            "Reconciled orphan container"
        );

        reconciled_count += 1;
    }

    if reconciled_count > 0 {
        tracing::info!(
            target: "cronduit.reconcile",
            count = reconciled_count,
            "orphan reconciliation complete"
        );
    }

    Ok(reconciled_count)
}

/// Mark a run as orphaned in the DB.
///
/// T-04-12: WHERE clause includes `AND status = 'running'` to prevent
/// overwriting finalized runs.
async fn mark_run_orphaned(pool: &DbPool, run_id: i64) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    match pool.writer() {
        PoolRef::Sqlite(p) => {
            sqlx::query(
                "UPDATE job_runs SET status = ?1, error_message = ?2, end_time = ?3 WHERE id = ?4 AND status = 'running'",
            )
            .bind("error")
            .bind("orphaned at restart")
            .bind(&now)
            .bind(run_id)
            .execute(p)
            .await?;
        }
        PoolRef::Postgres(p) => {
            sqlx::query(
                "UPDATE job_runs SET status = $1, error_message = $2, end_time = $3 WHERE id = $4 AND status = 'running'",
            )
            .bind("error")
            .bind("orphaned at restart")
            .bind(&now)
            .bind(run_id)
            .execute(p)
            .await?;
        }
    }

    Ok(())
}
