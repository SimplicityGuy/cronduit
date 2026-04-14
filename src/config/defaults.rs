//! Merge [defaults] into each JobConfig exactly once, during parse_and_validate.
//!
//! After this runs, every downstream consumer (validator, sync, hash, executor)
//! reads the already-merged JobConfig directly and MUST NOT consult
//! Config.defaults for per-job fields. The only remaining consumer of
//! Config.defaults is `random_min_gap`, which is a global @random scheduler
//! knob and NOT a per-job field -- see src/cli/run.rs and src/scheduler/reload.rs.

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
            delete: None,
            timeout: None,
            random_min_gap: Some(Duration::from_secs(90 * 60)),
        };
        let defaults_without_gap = DefaultsConfig {
            image: Some("alpine:latest".into()),
            network: None,
            volumes: None,
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
}
