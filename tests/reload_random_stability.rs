//! Integration test: @random resolved_schedule stability across reloads.
//!
//! Tests RAND-02, RAND-03.
//! Verifies that unchanged @random jobs retain their resolved_schedule,
//! and that changing the schedule field triggers re-randomization.

use std::collections::HashMap;
use std::io::Write;
use std::time::Duration;
use tempfile::NamedTempFile;

use cronduit::config::parse_and_validate;
use cronduit::db::DbPool;
use cronduit::db::queries::{DbJob, get_enabled_jobs, get_job_by_name};
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

const RANDOM_CONFIG: &str = r#"
[server]
timezone = "UTC"

[[jobs]]
name = "job-a"
schedule = "@random 14 * * *"
command = "echo randomized"
"#;

const CHANGED_RANDOM_CONFIG: &str = r#"
[server]
timezone = "UTC"

[[jobs]]
name = "job-a"
schedule = "@random 15 * * *"
command = "echo randomized"
"#;

#[tokio::test]
async fn random_schedule_stable_across_unchanged_reload() {
    let pool = setup_pool().await;

    // 1. Initial sync with @random schedule
    let config_file = write_config(RANDOM_CONFIG);
    let parsed = parse_and_validate(config_file.path()).expect("parse");
    sync_config_to_db(&pool, &parsed.config, Duration::from_secs(0))
        .await
        .expect("initial sync");

    // 2. Note the resolved_schedule
    let job_a_v1 = get_job_by_name(&pool, "job-a").await.unwrap().unwrap();
    let resolved_v1 = job_a_v1.resolved_schedule.clone();

    // The resolved schedule should have a concrete minute, not @random
    assert!(
        !resolved_v1.contains("@random"),
        "resolved_schedule should not contain @random, got: {resolved_v1}"
    );
    // Hour should be 14
    let fields: Vec<&str> = resolved_v1.split_whitespace().collect();
    assert_eq!(fields.len(), 5);
    assert_eq!(fields[1], "14", "hour field should be 14");

    // 3. Reload with SAME config (no changes)
    let enabled = get_enabled_jobs(&pool).await.unwrap();
    let mut jobs: HashMap<i64, DbJob> = enabled.into_iter().map(|j| (j.id, j)).collect();
    let (result, _) = do_reload(&pool, config_file.path(), &mut jobs, chrono_tz::UTC).await;

    assert_eq!(result.status, ReloadStatus::Ok);

    // 4. Verify resolved_schedule is IDENTICAL (stability)
    let job_a_v2 = get_job_by_name(&pool, "job-a").await.unwrap().unwrap();
    assert_eq!(
        job_a_v2.resolved_schedule, resolved_v1,
        "resolved_schedule should be stable across unchanged reload"
    );

    pool.close().await;
}

#[tokio::test]
async fn random_schedule_rerandomized_on_change() {
    let pool = setup_pool().await;

    // 1. Initial sync with hour=14
    let config_file = write_config(RANDOM_CONFIG);
    let parsed = parse_and_validate(config_file.path()).expect("parse");
    sync_config_to_db(&pool, &parsed.config, Duration::from_secs(0))
        .await
        .expect("initial sync");

    let job_a_v1 = get_job_by_name(&pool, "job-a").await.unwrap().unwrap();
    let resolved_v1 = job_a_v1.resolved_schedule.clone();
    let fields_v1: Vec<&str> = resolved_v1.split_whitespace().collect();
    assert_eq!(fields_v1[1], "14");

    // 2. Change schedule to hour=15
    std::fs::write(config_file.path(), CHANGED_RANDOM_CONFIG).expect("write changed config");

    let enabled = get_enabled_jobs(&pool).await.unwrap();
    let mut jobs: HashMap<i64, DbJob> = enabled.into_iter().map(|j| (j.id, j)).collect();
    let (result, _) = do_reload(&pool, config_file.path(), &mut jobs, chrono_tz::UTC).await;

    assert_eq!(result.status, ReloadStatus::Ok);
    assert_eq!(result.updated, 1, "job-a should be updated");

    // 3. Verify re-randomization: hour should now be 15
    let job_a_v2 = get_job_by_name(&pool, "job-a").await.unwrap().unwrap();
    let resolved_v2 = job_a_v2.resolved_schedule.clone();
    let fields_v2: Vec<&str> = resolved_v2.split_whitespace().collect();

    assert_eq!(
        fields_v2[1], "15",
        "hour should be 15 after schedule change, got: {resolved_v2}"
    );
    assert!(
        !resolved_v2.contains("@random"),
        "resolved_schedule should not contain @random after re-randomization"
    );

    pool.close().await;
}
