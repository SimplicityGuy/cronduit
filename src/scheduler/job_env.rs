//! Resolved per-job `[jobs.env]` values, carried from the in-memory config to
//! the Docker executor.
//!
//! `[jobs.env]` values are `SecretString`s resolved at config load (`${VAR}`
//! interpolation runs over the whole file before TOML parsing). They are
//! deliberately NOT written to `config_json` (T-02-03: only `env_keys` is
//! stored), so the executor cannot read them back from the DB. Instead the
//! bin layer (`src/cli/run.rs`) builds a [`JobEnvMap`] from the parsed config
//! at startup, the scheduler loop owns it, and `do_reload` rebuilds it from
//! the freshly parsed config on every successful reload. Each run receives
//! the [`JobEnv`] for its job at spawn time (an `Arc` clone, so an in-flight
//! run keeps the env it started with, like its cloned `DbJob`).
//!
//! The values are exposed only when the Docker executor builds the container's
//! `Env` vector (`docker::container_env`). They never reach `config_json`, the
//! jobs table, logs, the web UI or metrics.
//!
//! ```mermaid
//! flowchart LR
//!     TOML["config file<br/>[jobs.env]"] -->|parse_and_validate| CFG["Config.jobs[].env<br/>BTreeMap&lt;String, SecretString&gt;"]
//!     CFG -->|build_job_env_map| MAP["JobEnvMap<br/>(keyed by job name)"]
//!     CFG -->|serialize_config_json| DB[("jobs.config_json<br/>env_keys only")]
//!     MAP -->|SchedulerLoop::env_for| RUN["run_job"]
//!     RUN --> EXEC["execute_docker<br/>container_env: expose_secret"]
//!     RELOAD["do_reload"] -->|rebuild| MAP
//! ```
//!
//! `[defaults]` has no `env` field: `[jobs.env]` is per-job only and
//! `apply_defaults` never touches it, so the map holds exactly what the
//! operator wrote in each job's `[jobs.env]` block, regardless of
//! `use_defaults`.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use secrecy::SecretString;

use crate::config::Config;

/// One job's resolved `[jobs.env]` (key -> secret value).
pub type JobEnv = Arc<BTreeMap<String, SecretString>>;

/// Resolved `[jobs.env]` for every configured job with a non-empty env block,
/// keyed by job name (names are unique after validation).
pub type JobEnvMap = HashMap<String, JobEnv>;

/// Build the per-job env map from a parsed, validated, defaults-merged config.
pub fn build_job_env_map(config: &Config) -> JobEnvMap {
    config
        .jobs
        .iter()
        .filter(|job| !job.env.is_empty())
        .map(|job| (job.name.clone(), Arc::new(job.env.clone())))
        .collect()
}

/// Look up a job's env by name. Jobs without `[jobs.env]` get an empty map.
pub fn env_for(map: &JobEnvMap, job_name: &str) -> JobEnv {
    map.get(job_name).cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;
    use std::io::Write;

    fn parse(toml: &str) -> crate::config::ParsedConfig {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(toml.as_bytes()).unwrap();
        crate::config::parse_and_validate(f.path()).expect("config parses")
    }

    #[test]
    fn map_holds_resolved_values_keyed_by_job_name() {
        let parsed = parse(
            r#"
[server]
timezone = "UTC"

[[jobs]]
name = "with-env"
schedule = "*/5 * * * *"
image = "alpine:latest"

[jobs.env]
A = "1"
B = "two words"

[[jobs]]
name = "no-env"
schedule = "*/5 * * * *"
image = "alpine:latest"
"#,
        );
        let map = build_job_env_map(&parsed.config);
        assert_eq!(map.len(), 1, "jobs without [jobs.env] are not stored");
        let env = env_for(&map, "with-env");
        let pairs: Vec<(&str, &str)> = env
            .iter()
            .map(|(k, v)| (k.as_str(), v.expose_secret()))
            .collect();
        assert_eq!(pairs, vec![("A", "1"), ("B", "two words")]);
        assert!(env_for(&map, "no-env").is_empty());
        assert!(env_for(&map, "unknown").is_empty());
    }
}
