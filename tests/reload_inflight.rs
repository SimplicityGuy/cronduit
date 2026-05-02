//! Integration test: reload does not cancel in-flight runs.
//!
//! Tests RELOAD-06.
//! Verifies that a running job_runs row survives a config reload.

use std::collections::HashMap;
use std::io::Write;
use std::time::Duration;
use tempfile::NamedTempFile;

use cronduit::config::parse_and_validate;
use cronduit::db::DbPool;
use cronduit::db::queries::{DbJob, get_enabled_jobs, get_run_by_id, insert_running_run};
use cronduit::scheduler::cmd::ReloadStatus;
use cronduit::scheduler::reload::do_reload;
use cronduit::scheduler::sync::sync_config_to_db;

async fn setup_pool() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    pool.migrate().await.unwrap();
    pool
}

fn write_config(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("create tempfile");
    f.write_all(content.as_bytes()).expect("write config");
    f.flush().expect("flush config");
    f
}

const CONFIG: &str = r#"
[server]
timezone = "UTC"

[[jobs]]
name = "job-a"
schedule = "*/5 * * * *"
command = "sleep 60"
"#;

#[tokio::test]
async fn inflight_run_survives_reload() {
    let pool = setup_pool().await;

    // 1. Initial sync
    let config_file = write_config(CONFIG);
    let parsed = parse_and_validate(config_file.path()).expect("parse");
    sync_config_to_db(&pool, &parsed.config, Duration::from_secs(0))
        .await
        .expect("initial sync");

    let enabled = get_enabled_jobs(&pool).await.unwrap();
    assert_eq!(enabled.len(), 1);
    let job_a = &enabled[0];

    // 2. Insert a running run for job-a
    let run_id = insert_running_run(&pool, job_a.id, "scheduled", "testhash", None)
        .await
        .expect("insert running run");

    // Verify it's running
    let run_before = get_run_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(run_before.status, "running");

    // 3. Call do_reload with the same config
    let mut jobs: HashMap<i64, DbJob> = enabled.into_iter().map(|j| (j.id, j)).collect();
    let (result, _) = do_reload(&pool, config_file.path(), &mut jobs, chrono_tz::UTC).await;

    assert_eq!(result.status, ReloadStatus::Ok);

    // 4. Verify the running run is still running (not cancelled/errored)
    let run_after = get_run_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(
        run_after.status, "running",
        "in-flight run should still be running after reload"
    );
    assert_eq!(run_after.id, run_id, "run ID should be unchanged");

    pool.close().await;
}
