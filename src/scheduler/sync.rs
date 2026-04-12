//! Config-to-DB sync engine.
//!
//! D-06: On startup, upserts config jobs into the `jobs` table using
//! `config_hash` for change detection. New jobs INSERT, changed jobs UPDATE,
//! removed jobs set `enabled=0`.
//!
//! T-02-03: `config_json` excludes SecretString env values -- only env key
//! names appear, never secret values.

use crate::config::hash::compute_config_hash;
use crate::config::{Config, JobConfig};
use crate::db::DbPool;
use crate::db::queries::{
    DbJob, disable_missing_jobs, get_enabled_jobs, get_job_by_name, upsert_job,
};
use super::random;
use std::time::Duration;

/// Result of a config sync operation.
#[derive(Debug)]
pub struct SyncResult {
    pub inserted: u64,
    pub updated: u64,
    pub disabled: u64,
    pub unchanged: u64,
    pub jobs: Vec<DbJob>,
}

/// Determine the job type string from a `JobConfig`.
fn job_type(job: &JobConfig) -> &'static str {
    if job.command.is_some() {
        "command"
    } else if job.script.is_some() {
        "script"
    } else if job.image.is_some() {
        "docker"
    } else {
        // Validation should have caught this, but fallback gracefully.
        "unknown"
    }
}

/// Serialize job config to JSON for storage, excluding secret env values.
///
/// T-02-03: Only env key names are stored, never secret values.
fn serialize_config_json(job: &JobConfig) -> String {
    let mut map = serde_json::Map::new();
    map.insert("name".into(), serde_json::json!(job.name));
    map.insert("schedule".into(), serde_json::json!(job.schedule));
    if let Some(c) = &job.command {
        map.insert("command".into(), serde_json::json!(c));
    }
    if let Some(s) = &job.script {
        map.insert("script".into(), serde_json::json!(s));
    }
    if let Some(i) = &job.image {
        map.insert("image".into(), serde_json::json!(i));
    }
    if let Some(v) = &job.volumes {
        map.insert("volumes".into(), serde_json::json!(v));
    }
    if let Some(n) = &job.network {
        map.insert("network".into(), serde_json::json!(n));
    }
    if let Some(cn) = &job.container_name {
        map.insert("container_name".into(), serde_json::json!(cn));
    }
    if let Some(t) = &job.timeout {
        map.insert("timeout_secs".into(), serde_json::json!(t.as_secs()));
    }
    // T-02-03: Only store env key names, never values.
    if !job.env.is_empty() {
        let keys: Vec<&str> = job.env.keys().map(|k| k.as_str()).collect();
        map.insert("env_keys".into(), serde_json::json!(keys));
    }
    serde_json::to_string(&serde_json::Value::Object(map)).unwrap_or_default()
}

/// Sync the parsed config into the database.
///
/// For each job in config:
/// - Compute `config_hash` via `compute_config_hash`
/// - If the job doesn't exist in DB, insert it (counted as `inserted`)
/// - If the job exists but hash differs, update it (counted as `updated`)
/// - If the job exists with the same hash, skip (no-op)
///
/// After processing all config jobs, disable any DB jobs whose names
/// are not in the config (counted as `disabled`).
///
/// Returns the final list of enabled jobs.
pub async fn sync_config_to_db(
    pool: &DbPool,
    config: &Config,
    random_min_gap: Duration,
) -> anyhow::Result<SyncResult> {
    let mut inserted: u64 = 0;
    let mut updated: u64 = 0;
    let mut unchanged: u64 = 0;

    // Build a name->DbJob cache from a single pass of DB queries so each job
    // is fetched only once (not twice per sync cycle).
    let mut existing_cache: std::collections::HashMap<String, Option<DbJob>> =
        std::collections::HashMap::new();

    // Build batch resolver input: (name, raw_schedule, existing_resolved_from_db).
    // NOTE: rng must be created AFTER the async loop to avoid holding !Send
    // ThreadRng across await points (tokio::spawn requires Send futures).
    let mut batch_input: Vec<(String, String, Option<String>)> = Vec::new();
    for job in &config.jobs {
        let hash = compute_config_hash(job);
        let existing = get_job_by_name(pool, &job.name).await?;
        let existing_resolved = existing
            .as_ref()
            .filter(|db_job| db_job.config_hash == hash && db_job.enabled)
            .map(|db_job| db_job.resolved_schedule.clone());
        batch_input.push((job.name.clone(), job.schedule.clone(), existing_resolved));
        existing_cache.insert(job.name.clone(), existing);
    }
    let resolved_map: std::collections::HashMap<String, String> = {
        let mut rng = rand::thread_rng();
        random::resolve_random_schedules_batch(&batch_input, random_min_gap, &mut rng)
            .into_iter()
            .collect()
    };

    for job in &config.jobs {
        let hash = compute_config_hash(job);
        let jtype = job_type(job);
        let config_json = serialize_config_json(job);
        let resolved_schedule = resolved_map
            .get(&job.name)
            .cloned()
            .unwrap_or_else(|| job.schedule.clone());
        let timeout_secs = job
            .timeout
            .unwrap_or(Duration::from_secs(3600))
            .as_secs() as i64;

        // Use the cached DB lookup from the first pass instead of fetching again.
        let existing = existing_cache.remove(&job.name).flatten();

        match existing {
            Some(ref db_job) if db_job.config_hash == hash && db_job.enabled => {
                // No change, skip upsert entirely.
                unchanged += 1;
                continue;
            }
            Some(_) => {
                // Exists but changed (or was disabled) -- update.
                upsert_job(
                    pool,
                    &job.name,
                    &job.schedule,
                    &resolved_schedule,
                    jtype,
                    &config_json,
                    &hash,
                    timeout_secs,
                )
                .await?;
                updated += 1;
            }
            None => {
                // New job -- insert.
                upsert_job(
                    pool,
                    &job.name,
                    &job.schedule,
                    &resolved_schedule,
                    jtype,
                    &config_json,
                    &hash,
                    timeout_secs,
                )
                .await?;
                inserted += 1;
            }
        }
    }

    // Disable jobs not in config.
    let active_names: Vec<String> = config.jobs.iter().map(|j| j.name.clone()).collect();
    let disabled = disable_missing_jobs(pool, &active_names).await?;

    // Fetch final enabled job list.
    let jobs = get_enabled_jobs(pool).await?;

    tracing::info!(
        target: "cronduit.sync",
        inserted,
        updated,
        disabled,
        unchanged,
        total = jobs.len(),
        "config sync complete"
    );

    Ok(SyncResult {
        inserted,
        updated,
        disabled,
        unchanged,
        jobs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, JobConfig, ServerConfig};
    use secrecy::SecretString;
    use std::collections::BTreeMap;
    use std::time::Duration;

    fn make_server_config() -> ServerConfig {
        ServerConfig {
            bind: "127.0.0.1:8080".into(),
            database_url: SecretString::from("sqlite::memory:"),
            timezone: "UTC".into(),
            shutdown_grace: Duration::from_secs(30),
            log_retention: Duration::from_secs(86400),
            watch_config: true,
        }
    }

    fn make_job(
        name: &str,
        schedule: &str,
        command: Option<&str>,
        script: Option<&str>,
    ) -> JobConfig {
        JobConfig {
            name: name.into(),
            schedule: schedule.into(),
            command: command.map(|s| s.into()),
            script: script.map(|s| s.into()),
            image: None,
            use_defaults: None,
            env: BTreeMap::new(),
            volumes: None,
            network: None,
            container_name: None,
            timeout: None,
        }
    }

    async fn setup_pool() -> DbPool {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        pool.migrate().await.unwrap();
        pool
    }

    #[tokio::test]
    async fn sync_inserts_new_jobs() {
        let pool = setup_pool().await;
        let config = Config {
            server: make_server_config(),
            defaults: None,
            jobs: vec![
                make_job("job-a", "*/5 * * * *", Some("echo a"), None),
                make_job("job-b", "0 * * * *", None, Some("#!/bin/sh\necho b")),
            ],
        };

        let result = sync_config_to_db(&pool, &config, Duration::from_secs(0)).await.unwrap();
        assert_eq!(result.inserted, 2);
        assert_eq!(result.updated, 0);
        assert_eq!(result.disabled, 0);
        assert_eq!(result.jobs.len(), 2);

        let a = get_job_by_name(&pool, "job-a").await.unwrap().unwrap();
        assert_eq!(a.job_type, "command");
        assert!(a.enabled);
        assert_eq!(a.resolved_schedule, "*/5 * * * *");

        let b = get_job_by_name(&pool, "job-b").await.unwrap().unwrap();
        assert_eq!(b.job_type, "script");
        assert!(b.enabled);

        pool.close().await;
    }

    #[tokio::test]
    async fn sync_updates_changed_job() {
        let pool = setup_pool().await;
        let config1 = Config {
            server: make_server_config(),
            defaults: None,
            jobs: vec![make_job("job-a", "*/5 * * * *", Some("echo a"), None)],
        };
        sync_config_to_db(&pool, &config1, Duration::from_secs(0)).await.unwrap();

        let before = get_job_by_name(&pool, "job-a").await.unwrap().unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        // Change the command (which changes config_hash).
        let config2 = Config {
            server: make_server_config(),
            defaults: None,
            jobs: vec![make_job("job-a", "*/5 * * * *", Some("echo changed"), None)],
        };
        let result = sync_config_to_db(&pool, &config2, Duration::from_secs(0)).await.unwrap();
        assert_eq!(result.inserted, 0);
        assert_eq!(result.updated, 1);

        let after = get_job_by_name(&pool, "job-a").await.unwrap().unwrap();
        assert_ne!(before.config_hash, after.config_hash);
        assert_ne!(before.updated_at, after.updated_at);

        pool.close().await;
    }

    #[tokio::test]
    async fn sync_disables_removed_job() {
        let pool = setup_pool().await;
        let config1 = Config {
            server: make_server_config(),
            defaults: None,
            jobs: vec![
                make_job("keep", "*/5 * * * *", Some("echo keep"), None),
                make_job("remove", "*/5 * * * *", Some("echo remove"), None),
            ],
        };
        sync_config_to_db(&pool, &config1, Duration::from_secs(0)).await.unwrap();

        // Second sync with "remove" absent.
        let config2 = Config {
            server: make_server_config(),
            defaults: None,
            jobs: vec![make_job("keep", "*/5 * * * *", Some("echo keep"), None)],
        };
        let result = sync_config_to_db(&pool, &config2, Duration::from_secs(0)).await.unwrap();
        assert_eq!(result.disabled, 1);
        assert_eq!(result.jobs.len(), 1);

        let removed = get_job_by_name(&pool, "remove").await.unwrap().unwrap();
        assert!(!removed.enabled);

        let kept = get_job_by_name(&pool, "keep").await.unwrap().unwrap();
        assert!(kept.enabled);

        pool.close().await;
    }

    #[tokio::test]
    async fn sync_noop_same_hash() {
        let pool = setup_pool().await;
        let config = Config {
            server: make_server_config(),
            defaults: None,
            jobs: vec![make_job("job-a", "*/5 * * * *", Some("echo a"), None)],
        };
        sync_config_to_db(&pool, &config, Duration::from_secs(0)).await.unwrap();
        let before = get_job_by_name(&pool, "job-a").await.unwrap().unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        // Same config, should be a no-op.
        let result = sync_config_to_db(&pool, &config, Duration::from_secs(0)).await.unwrap();
        assert_eq!(result.inserted, 0);
        assert_eq!(result.updated, 0);
        assert_eq!(result.disabled, 0);

        let after = get_job_by_name(&pool, "job-a").await.unwrap().unwrap();
        assert_eq!(before.updated_at, after.updated_at);

        pool.close().await;
    }

    #[tokio::test]
    async fn sync_config_json_excludes_secret_values() {
        // T-02-03: Verify env values are NOT in config_json.
        let pool = setup_pool().await;
        let mut env = BTreeMap::new();
        env.insert(
            "SECRET_KEY".to_string(),
            SecretString::from("super-secret-value"),
        );

        let config = Config {
            server: make_server_config(),
            defaults: None,
            jobs: vec![JobConfig {
                name: "with-env".into(),
                schedule: "*/5 * * * *".into(),
                command: Some("echo test".into()),
                script: None,
                image: None,
                use_defaults: None,
                env,
                volumes: None,
                network: None,
                container_name: None,
                timeout: None,
            }],
        };

        sync_config_to_db(&pool, &config, Duration::from_secs(0)).await.unwrap();
        let job = get_job_by_name(&pool, "with-env").await.unwrap().unwrap();

        // config_json should contain the key name but NOT the secret value.
        assert!(job.config_json.contains("SECRET_KEY"));
        assert!(!job.config_json.contains("super-secret-value"));

        pool.close().await;
    }

    #[tokio::test]
    async fn sync_get_enabled_jobs_returns_only_enabled() {
        let pool = setup_pool().await;
        let config = Config {
            server: make_server_config(),
            defaults: None,
            jobs: vec![
                make_job("a", "*/5 * * * *", Some("echo a"), None),
                make_job("b", "*/5 * * * *", Some("echo b"), None),
            ],
        };
        sync_config_to_db(&pool, &config, Duration::from_secs(0)).await.unwrap();

        // Remove "b".
        let config2 = Config {
            server: make_server_config(),
            defaults: None,
            jobs: vec![make_job("a", "*/5 * * * *", Some("echo a"), None)],
        };
        sync_config_to_db(&pool, &config2, Duration::from_secs(0)).await.unwrap();

        let enabled = get_enabled_jobs(&pool).await.unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "a");

        pool.close().await;
    }
}
