//! Integration test: config edit -> do_reload -> verify DB state.
//!
//! Tests RELOAD-01, RELOAD-05, RELOAD-07.
//! Does NOT send an actual SIGHUP (that's platform-dependent and flaky in CI).
//! Instead, calls do_reload() directly -- the same code path SIGHUP uses.

use std::collections::HashMap;
use std::io::Write;
use std::time::Duration;
use tempfile::NamedTempFile;

use cronduit::db::DbPool;
use cronduit::db::queries::{get_enabled_jobs, get_job_by_name, DbJob};
use cronduit::scheduler::cmd::ReloadStatus;
use cronduit::scheduler::reload::do_reload;
use cronduit::scheduler::sync::sync_config_to_db;
use cronduit::config::parse_and_validate;

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

const INITIAL_CONFIG: &str = r#"
[server]
timezone = "UTC"

[[jobs]]
name = "job-a"
schedule = "*/5 * * * *"
command = "echo a"

[[jobs]]
name = "job-b"
schedule = "0 * * * *"
command = "echo b"
"#;

const MODIFIED_CONFIG: &str = r#"
[server]
timezone = "UTC"

[[jobs]]
name = "job-a"
schedule = "*/10 * * * *"
command = "echo a-changed"

[[jobs]]
name = "job-c"
schedule = "30 * * * *"
command = "echo c"
"#;

#[tokio::test]
async fn reload_creates_updates_disables_jobs() {
    let pool = setup_pool().await;

    // 1. Write initial config and do initial sync via parse_and_validate + sync_config_to_db
    let config_file = write_config(INITIAL_CONFIG);
    let parsed = parse_and_validate(config_file.path()).expect("initial parse");
    let sync_result = sync_config_to_db(&pool, &parsed.config, Duration::from_secs(0))
        .await
        .expect("initial sync");
    assert_eq!(sync_result.inserted, 2);

    // Verify initial state
    let enabled = get_enabled_jobs(&pool).await.unwrap();
    assert_eq!(enabled.len(), 2);
    let job_a_before = get_job_by_name(&pool, "job-a").await.unwrap().unwrap();
    let job_a_hash_before = job_a_before.config_hash.clone();

    // 2. Overwrite config file with modified version (removes job-b, adds job-c, changes job-a)
    std::fs::write(config_file.path(), MODIFIED_CONFIG).expect("overwrite config");

    // 3. Build the in-memory jobs map (as the scheduler would have it)
    let mut jobs: HashMap<i64, DbJob> = enabled.into_iter().map(|j| (j.id, j)).collect();

    // 4. Call do_reload
    let (result, new_heap) = do_reload(
        &pool,
        config_file.path(),
        &mut jobs,
        chrono_tz::UTC,
    )
    .await;

    // 5. Assert ReloadResult counts
    assert_eq!(result.status, ReloadStatus::Ok);
    assert_eq!(result.added, 1, "job-c should be added");
    assert_eq!(result.updated, 1, "job-a should be updated (schedule changed)");
    assert_eq!(result.disabled, 1, "job-b should be disabled");
    assert!(result.error_message.is_none());
    assert!(new_heap.is_some(), "heap should be rebuilt on success");

    // 6. Verify DB state
    let job_b = get_job_by_name(&pool, "job-b").await.unwrap().unwrap();
    assert!(!job_b.enabled, "job-b should be disabled");

    let job_c = get_job_by_name(&pool, "job-c").await.unwrap().unwrap();
    assert!(job_c.enabled, "job-c should be enabled");
    assert_eq!(job_c.schedule, "30 * * * *");

    let job_a_after = get_job_by_name(&pool, "job-a").await.unwrap().unwrap();
    assert!(job_a_after.enabled);
    assert_ne!(
        job_a_after.config_hash, job_a_hash_before,
        "job-a config_hash should change after schedule update"
    );
    assert_eq!(job_a_after.schedule, "*/10 * * * *");

    pool.close().await;
}

#[tokio::test]
async fn reload_with_parse_error_leaves_config_untouched() {
    let pool = setup_pool().await;

    // Initial sync
    let config_file = write_config(INITIAL_CONFIG);
    let parsed = parse_and_validate(config_file.path()).expect("initial parse");
    sync_config_to_db(&pool, &parsed.config, Duration::from_secs(0))
        .await
        .expect("initial sync");

    let enabled_before = get_enabled_jobs(&pool).await.unwrap();
    let mut jobs: HashMap<i64, DbJob> = enabled_before
        .iter()
        .map(|j| (j.id, j.clone()))
        .collect();

    // Write invalid TOML
    std::fs::write(config_file.path(), "this is [[[invalid toml").expect("write bad config");

    let (result, new_heap) = do_reload(
        &pool,
        config_file.path(),
        &mut jobs,
        chrono_tz::UTC,
    )
    .await;

    // RELOAD-04: Failed reload leaves running config untouched
    assert_eq!(result.status, ReloadStatus::Error);
    assert!(result.error_message.is_some());
    assert!(new_heap.is_none(), "no heap on error");

    // DB state should be unchanged
    let enabled_after = get_enabled_jobs(&pool).await.unwrap();
    assert_eq!(enabled_before.len(), enabled_after.len());

    pool.close().await;
}
