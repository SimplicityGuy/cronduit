//! # Config plumbing parity invariant
//!
//! This module is the single point of truth for how `[defaults]` merges into
//! per-job `JobConfig`s, but it is ALSO load-bearing as documentation for the
//! broader "config-to-executor plumbing" invariant. Five layers must stay in
//! lock-step for any field that ends up on an executor deserialize struct:
//!
//! 1. `JobConfig` in `src/config/mod.rs` -- the TOML-side struct.
//! 2. `serialize_config_json` in `src/scheduler/sync.rs` -- writes to the DB
//!    `config_json` column that the executor reads back.
//! 3. `compute_config_hash` in `src/config/hash.rs` -- change-detection for
//!    `sync_config_to_db` so an operator's edit triggers an `updated` upsert.
//! 4. `apply_defaults` in THIS file -- decides whether `[defaults]` merges
//!    into the field or the field is per-job-only.
//! 5. `DockerJobConfig` in `src/scheduler/docker.rs` -- the executor-side
//!    deserialize struct that reads the serialized JSON.
//!
//! When one of the five drifts without the others, silent behavior regressions
//! slip through unit tests that construct hand-rolled fixtures. The class of
//! bug that produced both the `[defaults]` merge bug (issue #20) AND the
//! missing `cmd` field was the same root cause: the executor-side struct was
//! never cross-referenced with the TOML-side struct or the DB path.
//!
//! ```mermaid
//! classDiagram
//!     class JobConfig {
//!         +name: String
//!         +schedule: String
//!         +command: Option~String~
//!         +script: Option~String~
//!         +image: Option~String~
//!         +volumes: Option~Vec~String~~
//!         +network: Option~String~
//!         +container_name: Option~String~
//!         +cmd: Option~Vec~String~~
//!         +delete: Option~bool~
//!         +timeout: Option~Duration~
//!         +env: BTreeMap~String,SecretString~
//!         +use_defaults: Option~bool~
//!     }
//!     class DefaultsConfig {
//!         +image: Option~String~
//!         +network: Option~String~
//!         +volumes: Option~Vec~String~~
//!         +delete: Option~bool~
//!         +timeout: Option~Duration~
//!         +random_min_gap: Option~Duration~
//!     }
//!     class DockerJobConfig {
//!         +image: String
//!         +env: HashMap~String,String~
//!         +volumes: Option~Vec~String~~
//!         +cmd: Option~Vec~String~~
//!         +network: Option~String~
//!         +container_name: Option~String~
//!     }
//!     JobConfig --> DefaultsConfig : apply_defaults merges image/network/volumes/delete/timeout
//!     JobConfig --> DockerJobConfig : serialize_config_json -> config_json -> deserialize
//! ```
//!
//! ## Parity table
//!
//! | DockerJobConfig field | JobConfig field  | serialize_config_json  | compute_config_hash | apply_defaults decision | Notes |
//! |---|---|---|---|---|---|
//! | `image`           | `image`          | yes                    | yes                 | mergeable               | Falls back to `[defaults].image` |
//! | `env`             | `env`            | keys only (`env_keys`) | excluded            | per-job only (secret)   | T-02-03: values are `SecretString`, never hashed/logged |
//! | `volumes`         | `volumes`        | yes                    | yes                 | mergeable               | Per-job REPLACES defaults (no concatenation) |
//! | `cmd`             | `cmd`            | yes                    | yes                 | per-job only            | NOT in `DefaultsConfig`. `Some(vec![])` is distinct from `None` |
//! | `network`         | `network`        | yes                    | yes                 | mergeable               | Includes `container:<name>` VPN mode -- marquee feature |
//! | `container_name`  | `container_name` | yes                    | yes                 | per-job only            | NOT in `DefaultsConfig` -- container names must be unique |
//!
//! Fields in `JobConfig` that are NOT read by `DockerJobConfig` but still
//! flow through the plumbing: `name`, `schedule`, `command`, `script`,
//! `timeout` (becomes `DbJob.timeout_secs`, used by every executor),
//! `delete` (serialized + hashed but not yet honored by the docker executor
//! -- see Known Gap in the plan's objective), `use_defaults` (consumed by
//! `apply_defaults` itself and then dropped -- not serialized, not hashed).
//!
//! ## Adding a new field
//!
//! Any future PR that adds a field to any ONE of these five layers MUST
//! update the other four in the same commit. The
//! `parity_with_docker_job_config_is_maintained` unit test below is a
//! regression guard for the JSON surface -- it will fail loudly if
//! `serialize_config_json` drops a field that `DockerJobConfig` reads. It
//! does NOT catch `compute_config_hash` or `apply_defaults` drift; those
//! still rely on PR review discipline and the parity table above.
//!
//! ## Merge semantics
//!
//! After `apply_defaults` runs, every downstream consumer (validator, sync,
//! hash, executor) reads the already-merged `JobConfig` directly and MUST NOT
//! consult `Config.defaults` for per-job fields. The only remaining consumer
//! of `Config.defaults` is `random_min_gap`, which is a global `@random`
//! scheduler knob and NOT a per-job field -- see `src/cli/run.rs` and
//! `src/scheduler/reload.rs`.

use super::{DefaultsConfig, JobConfig};

/// Apply [defaults] to a single job. Per-job fields always win.
///
/// - Returns `job` unchanged if `defaults` is `None` or `job.use_defaults == Some(false)` (CONF-04).
/// - Otherwise fills `image`, `network`, `volumes`, `timeout`, `delete` from defaults
///   when the job field is `None`. Per-job values ALWAYS override (CONF-06).
/// - Never merges `random_min_gap` -- that field does not exist on JobConfig;
///   it is a global scheduler knob consumed directly from Config.defaults.
/// - Never touches `cmd` or `container_name` -- both are per-job only by spec.
pub fn apply_defaults(mut job: JobConfig, defaults: Option<&DefaultsConfig>) -> JobConfig {
    let Some(defaults) = defaults else {
        return job;
    };
    if job.use_defaults == Some(false) {
        return job;
    }

    // Whether this job is non-docker (has command or script). For non-docker
    // jobs we MUST NOT merge docker-only fields (image/network/volumes/delete)
    // because doing so would violate the "exactly one of command/script/image"
    // invariant in `check_one_of_job_type` AND silently attach docker-only
    // settings to a job that has no container lifecycle. `timeout` is the one
    // defaults field that applies to every job type and is therefore always
    // mergeable.
    let is_non_docker = job.command.is_some() || job.script.is_some();

    if !is_non_docker
        && job.image.is_none()
        && let Some(v) = &defaults.image
    {
        job.image = Some(v.clone());
    }
    if !is_non_docker
        && job.network.is_none()
        && let Some(v) = &defaults.network
    {
        job.network = Some(v.clone());
    }
    if !is_non_docker
        && job.volumes.is_none()
        && let Some(v) = &defaults.volumes
    {
        job.volumes = Some(v.clone());
    }
    if job.timeout.is_none()
        && let Some(v) = defaults.timeout
    {
        job.timeout = Some(v);
    }
    if !is_non_docker
        && job.delete.is_none()
        && let Some(v) = defaults.delete
    {
        job.delete = Some(v);
    }
    // NOTE: random_min_gap is intentionally NOT merged -- see module doc.
    // NOTE: cmd is per-job ONLY -- DefaultsConfig has no `cmd` field.
    // NOTE: container_name is per-job ONLY -- two containers cannot share a name.

    job
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::SecretString;
    use std::collections::BTreeMap;
    use std::time::Duration;

    fn empty_job() -> JobConfig {
        JobConfig {
            name: "t".into(),
            schedule: "*/5 * * * *".into(),
            command: None,
            script: None,
            image: None,
            use_defaults: None,
            env: BTreeMap::new(),
            volumes: None,
            labels: None,
            network: None,
            container_name: None,
            timeout: None,
            delete: None,
            cmd: None,
        }
    }

    fn full_defaults() -> DefaultsConfig {
        DefaultsConfig {
            image: Some("alpine:latest".into()),
            network: Some("container:vpn".into()),
            volumes: Some(vec!["/host/a:/a".into(), "/host/b:/b".into()]),
            labels: None,
            delete: Some(true),
            timeout: Some(Duration::from_secs(300)),
            random_min_gap: Some(Duration::from_secs(90 * 60)),
        }
    }

    #[test]
    fn apply_defaults_fills_image_from_defaults() {
        let job = empty_job();
        let defaults = DefaultsConfig {
            image: Some("alpine:latest".into()),
            network: None,
            volumes: None,
            labels: None,
            delete: None,
            timeout: None,
            random_min_gap: None,
        };
        let merged = apply_defaults(job, Some(&defaults));
        assert_eq!(merged.image.as_deref(), Some("alpine:latest"));
    }

    #[test]
    fn apply_defaults_fills_network_from_defaults() {
        let job = empty_job();
        let defaults = DefaultsConfig {
            image: None,
            network: Some("container:vpn".into()),
            volumes: None,
            labels: None,
            delete: None,
            timeout: None,
            random_min_gap: None,
        };
        let merged = apply_defaults(job, Some(&defaults));
        assert_eq!(merged.network.as_deref(), Some("container:vpn"));
    }

    #[test]
    fn apply_defaults_fills_volumes_from_defaults() {
        let job = empty_job();
        let defaults = DefaultsConfig {
            image: None,
            network: None,
            volumes: Some(vec!["/host/a:/a".into(), "/host/b:/b".into()]),
            labels: None,
            delete: None,
            timeout: None,
            random_min_gap: None,
        };
        let merged = apply_defaults(job, Some(&defaults));
        assert_eq!(
            merged.volumes,
            Some(vec!["/host/a:/a".to_string(), "/host/b:/b".to_string()])
        );
    }

    #[test]
    fn apply_defaults_fills_timeout_from_defaults() {
        let job = empty_job();
        let defaults = DefaultsConfig {
            image: None,
            network: None,
            volumes: None,
            labels: None,
            delete: None,
            timeout: Some(Duration::from_secs(300)),
            random_min_gap: None,
        };
        let merged = apply_defaults(job, Some(&defaults));
        assert_eq!(merged.timeout, Some(Duration::from_secs(300)));
    }

    #[test]
    fn apply_defaults_fills_delete_from_defaults() {
        let job = empty_job();
        let defaults = DefaultsConfig {
            image: None,
            network: None,
            volumes: None,
            labels: None,
            delete: Some(true),
            timeout: None,
            random_min_gap: None,
        };
        let merged = apply_defaults(job, Some(&defaults));
        assert_eq!(merged.delete, Some(true));
    }

    #[test]
    fn apply_defaults_job_override_wins_image() {
        let mut job = empty_job();
        job.image = Some("nginx:1.25".into());
        let merged = apply_defaults(job, Some(&full_defaults()));
        assert_eq!(merged.image.as_deref(), Some("nginx:1.25"));
    }

    #[test]
    fn apply_defaults_job_override_wins_network() {
        let mut job = empty_job();
        job.network = Some("host".into());
        let merged = apply_defaults(job, Some(&full_defaults()));
        assert_eq!(merged.network.as_deref(), Some("host"));
    }

    #[test]
    fn apply_defaults_job_override_wins_volumes() {
        let mut job = empty_job();
        job.volumes = Some(vec!["/job".into()]);
        let merged = apply_defaults(job, Some(&full_defaults()));
        assert_eq!(merged.volumes, Some(vec!["/job".to_string()]));
    }

    #[test]
    fn apply_defaults_job_override_wins_timeout() {
        let mut job = empty_job();
        job.timeout = Some(Duration::from_secs(60));
        let merged = apply_defaults(job, Some(&full_defaults()));
        assert_eq!(merged.timeout, Some(Duration::from_secs(60)));
    }

    #[test]
    fn apply_defaults_job_override_wins_delete() {
        let mut job = empty_job();
        job.delete = Some(false);
        let merged = apply_defaults(job, Some(&full_defaults()));
        assert_eq!(merged.delete, Some(false));
    }

    #[test]
    fn apply_defaults_use_defaults_false_disables_merge() {
        let mut job = empty_job();
        job.use_defaults = Some(false);
        let merged = apply_defaults(job, Some(&full_defaults()));
        assert_eq!(merged.image, None);
        assert_eq!(merged.network, None);
        assert_eq!(merged.volumes, None);
        assert_eq!(merged.timeout, None);
        assert_eq!(merged.delete, None);
    }

    #[test]
    fn apply_defaults_none_returns_job_unchanged() {
        let job = JobConfig {
            name: "j".into(),
            schedule: "*/5 * * * *".into(),
            command: Some("echo hi".into()),
            script: None,
            image: None,
            use_defaults: None,
            env: BTreeMap::new(),
            volumes: None,
            labels: None,
            network: None,
            container_name: None,
            timeout: None,
            delete: None,
            cmd: None,
        };
        let merged = apply_defaults(job, None);
        assert_eq!(merged.image, None);
        assert_eq!(merged.network, None);
        assert_eq!(merged.volumes, None);
        assert_eq!(merged.timeout, None);
        assert_eq!(merged.delete, None);
        assert_eq!(merged.cmd, None);
        assert_eq!(merged.command.as_deref(), Some("echo hi"));
    }

    #[test]
    fn apply_defaults_does_not_touch_random_min_gap() {
        // random_min_gap is a global scheduler knob; it has no JobConfig field
        // and must never leak into per-job state.
        let job_with_gap = empty_job();
        let job_without_gap = empty_job();

        let defaults_with_gap = DefaultsConfig {
            image: Some("alpine:latest".into()),
            network: None,
            volumes: None,
            labels: None,
            delete: None,
            timeout: None,
            random_min_gap: Some(Duration::from_secs(90 * 60)),
        };
        let defaults_without_gap = DefaultsConfig {
            image: Some("alpine:latest".into()),
            network: None,
            volumes: None,
            labels: None,
            delete: None,
            timeout: None,
            random_min_gap: None,
        };

        let merged_with = apply_defaults(job_with_gap, Some(&defaults_with_gap));
        let merged_without = apply_defaults(job_without_gap, Some(&defaults_without_gap));

        // Both merged jobs must be identical w.r.t. every field, since
        // random_min_gap has no representation in JobConfig at all.
        assert_eq!(merged_with.name, merged_without.name);
        assert_eq!(merged_with.image, merged_without.image);
        assert_eq!(merged_with.network, merged_without.network);
        assert_eq!(merged_with.volumes, merged_without.volumes);
        assert_eq!(merged_with.timeout, merged_without.timeout);
        assert_eq!(merged_with.delete, merged_without.delete);
        assert_eq!(merged_with.cmd, merged_without.cmd);
    }

    #[test]
    fn apply_defaults_skips_docker_fields_on_command_jobs() {
        // Regression: a [defaults] section with image/network/volumes/delete
        // must NOT auto-attach those fields to a command/script job.
        // Otherwise check_one_of_job_type would fire ("found 2") because the
        // job would end up with both `command` AND `image`. `timeout` is the
        // one defaults field that should still merge into command jobs.
        let mut job = empty_job();
        job.command = Some("echo hi".into());
        let merged = apply_defaults(job, Some(&full_defaults()));
        assert_eq!(merged.image, None, "image must NOT merge into command jobs");
        assert_eq!(
            merged.network, None,
            "network must NOT merge into command jobs"
        );
        assert_eq!(
            merged.volumes, None,
            "volumes must NOT merge into command jobs"
        );
        assert_eq!(
            merged.delete, None,
            "delete must NOT merge into command jobs"
        );
        // timeout SHOULD merge -- it applies to every job type.
        assert_eq!(
            merged.timeout,
            Some(Duration::from_secs(300)),
            "timeout must merge into command jobs"
        );
    }

    #[test]
    fn apply_defaults_skips_docker_fields_on_script_jobs() {
        let mut job = empty_job();
        job.command = None;
        job.script = Some("#!/bin/sh\necho hi".into());
        let merged = apply_defaults(job, Some(&full_defaults()));
        assert_eq!(merged.image, None);
        assert_eq!(merged.network, None);
        assert_eq!(merged.volumes, None);
        assert_eq!(merged.delete, None);
        assert_eq!(merged.timeout, Some(Duration::from_secs(300)));
    }

    #[test]
    fn apply_defaults_does_not_touch_cmd() {
        // cmd is per-job ONLY -- there is no DefaultsConfig.cmd field, and
        // apply_defaults must pass `job.cmd` through untouched whether it
        // started as Some(vec) or None. Mirrors the random_min_gap invariant
        // for the new per-job-only `cmd` field.
        let mut job_with_cmd = empty_job();
        job_with_cmd.cmd = Some(vec!["a".to_string(), "b".to_string()]);
        let merged = apply_defaults(job_with_cmd, Some(&full_defaults()));
        assert_eq!(
            merged.cmd,
            Some(vec!["a".to_string(), "b".to_string()]),
            "apply_defaults must not modify a job's cmd field"
        );

        let job_without_cmd = empty_job();
        let merged = apply_defaults(job_without_cmd, Some(&full_defaults()));
        assert_eq!(
            merged.cmd, None,
            "apply_defaults must not invent a cmd from defaults (no DefaultsConfig.cmd field)"
        );

        // SecretString unused-import guard: keep the import live for cross-test consistency.
        let _ = SecretString::from("unused");
    }

    #[test]
    fn apply_defaults_does_not_touch_container_name() {
        // container_name is per-job ONLY -- two containers cannot share a
        // name, so there is no DefaultsConfig.container_name field and
        // apply_defaults must pass `job.container_name` through untouched
        // whether it started as Some(name) or None. Mirrors the random_min_gap
        // and cmd invariants.
        let mut job_with_name = empty_job();
        job_with_name.container_name = Some("fixed-name".to_string());
        let merged = apply_defaults(job_with_name, Some(&full_defaults()));
        assert_eq!(
            merged.container_name.as_deref(),
            Some("fixed-name"),
            "apply_defaults must not modify a job's container_name"
        );

        let job_without_name = empty_job();
        let merged = apply_defaults(job_without_name, Some(&full_defaults()));
        assert_eq!(
            merged.container_name, None,
            "apply_defaults must not invent a container_name from defaults \
             (no DefaultsConfig.container_name field)"
        );
    }

    #[test]
    fn parity_with_docker_job_config_is_maintained() {
        // Structural regression guard: construct a fully-populated JobConfig
        // with every non-secret field DockerJobConfig reads, serialize it
        // via sync::serialize_config_json, and assert every expected key is
        // present in the JSON output. Also confirms the output is a valid
        // DockerJobConfig so a future rename like `image` -> `image_ref` on
        // one side (but not the other) fails loudly here.
        use crate::scheduler::docker::DockerJobConfig;
        use crate::scheduler::sync;

        let mut env = BTreeMap::new();
        env.insert("SECRET_KEY".to_string(), SecretString::from("super-secret"));
        let job = JobConfig {
            name: "parity-test".to_string(),
            schedule: "*/5 * * * *".to_string(),
            command: None,
            script: None,
            image: Some("alpine:latest".to_string()),
            use_defaults: None,
            env,
            volumes: Some(vec!["/host:/container".to_string()]),
            labels: None,
            network: Some("container:vpn".to_string()),
            container_name: Some("parity-test-container".to_string()),
            timeout: Some(Duration::from_secs(300)),
            delete: Some(true),
            cmd: Some(vec!["echo".to_string(), "parity".to_string()]),
        };

        let json_str = sync::serialize_config_json(&job);
        let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
        let obj = v.as_object().expect("top-level object");

        // Every non-secret field DockerJobConfig reads MUST be in the output.
        assert!(
            obj.contains_key("image"),
            "image missing from config_json -- DockerJobConfig would fail to deserialize"
        );
        assert!(
            obj.contains_key("volumes"),
            "volumes missing from config_json"
        );
        assert!(
            obj.contains_key("network"),
            "network missing from config_json"
        );
        assert!(
            obj.contains_key("container_name"),
            "container_name missing from config_json"
        );
        assert!(obj.contains_key("cmd"), "cmd missing from config_json");
        // env is the secret allowlist: env_keys present, raw env values ABSENT.
        assert!(
            obj.contains_key("env_keys"),
            "env_keys missing -- key-name allowlist broken"
        );
        let json_body = serde_json::to_string(obj).unwrap();
        assert!(
            !json_body.contains("super-secret"),
            "T-02-03 breach: raw SecretString value leaked into config_json"
        );

        // DockerJobConfig compile-time smoke: confirm the emitted JSON is at
        // least structurally deserializable as a DockerJobConfig. This is a
        // one-way assertion -- DockerJobConfig only consumes a subset -- but
        // it fails loudly if a typed field name drifts (e.g. someone renames
        // `image` -> `image_ref` on JobConfig without updating both sides).
        let _check: DockerJobConfig = serde_json::from_str(&json_str)
            .expect("serialize_config_json output must be a valid DockerJobConfig");
    }
}
