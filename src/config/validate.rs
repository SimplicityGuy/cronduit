use super::{Config, ConfigError, JobConfig};
use croner::Cron;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;

static NETWORK_RE: Lazy<Regex> = Lazy::new(|| {
    // bridge | host | none | container:<name> | <named>
    Regex::new(r"^(bridge|host|none|container:[a-zA-Z0-9_.-]+|[a-zA-Z0-9_.-]+)$").unwrap()
});

/// Run every post-parse check; push errors into `errors`. Never fail-fast.
pub fn run_all_checks(cfg: &Config, path: &Path, raw: &str, errors: &mut Vec<ConfigError>) {
    check_timezone(&cfg.server.timezone, path, errors);
    check_bind(&cfg.server.bind, path, errors);
    check_duplicate_job_names(&cfg.jobs, path, raw, errors);
    for job in &cfg.jobs {
        check_one_of_job_type(job, path, errors);
        check_cmd_only_on_docker_jobs(job, path, errors);
        check_network_mode(job, path, errors);
        check_schedule(job, path, errors);
    }
}

fn check_timezone(tz: &str, path: &Path, errors: &mut Vec<ConfigError>) {
    if tz.parse::<chrono_tz::Tz>().is_err() {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!("not a valid IANA timezone: `{tz}` (see [server].timezone)"),
        });
    }
}

fn check_bind(bind: &str, path: &Path, errors: &mut Vec<ConfigError>) {
    if SocketAddr::from_str(bind).is_err() {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!("[server].bind is not a valid socket address: `{bind}`"),
        });
    }
}

fn check_one_of_job_type(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let count =
        job.command.is_some() as u8 + job.script.is_some() as u8 + job.image.is_some() as u8;
    if count != 1 {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}` must declare exactly one of `command`, `script`, or `image` (found {count}). Note: `image` may also come from `[defaults].image` unless the job sets `use_defaults = false`.",
                job.name
            ),
        });
    }
}

fn check_network_mode(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    if let Some(net) = &job.network
        && !NETWORK_RE.is_match(net)
    {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: invalid network mode `{net}` (expected bridge|host|none|container:<name>|<named-network>)",
                job.name
            ),
        });
    }
}

/// Reject `cmd` on command/script jobs. `cmd` is a Docker container CMD override
/// with no meaningful analog for command or script jobs (no container to receive
/// it). Runs AFTER `apply_defaults`, so the merged view of the job is what we
/// inspect — for command/script jobs the docker-only fields (`image`/`network`/
/// `volumes`/`delete`) are intentionally not merged by `apply_defaults`, so an
/// `image.is_none()` test reliably distinguishes "this is a non-docker job" from
/// "this is a docker job inheriting its image from `[defaults]`".
fn check_cmd_only_on_docker_jobs(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    if job.cmd.is_some() && job.image.is_none() {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: `cmd` is only valid on docker jobs (job with `image = \"...\"` set either directly or via `[defaults].image`); command and script jobs cannot set `cmd` because there is no container to receive it. Remove the `cmd` line, or switch the job to a docker job by setting `image`.",
                job.name
            ),
        });
    }
}

fn check_schedule(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    use crate::scheduler::random::is_random_schedule;

    // Schedules containing @random tokens are resolved at sync time, not here.
    // Validate only the non-@random fields by substituting @random with valid
    // stand-in values per field position (minute=0, hour=0, dom=1, month=1, dow=0).
    const RANDOM_FALLBACKS: [&str; 5] = ["0", "0", "1", "1", "0"];
    let schedule_to_validate = if is_random_schedule(&job.schedule) {
        job.schedule
            .split_whitespace()
            .enumerate()
            .map(|(i, f)| {
                if f == "@random" {
                    RANDOM_FALLBACKS.get(i).copied().unwrap_or("0")
                } else {
                    f
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        job.schedule.clone()
    };

    if let Err(e) = schedule_to_validate.parse::<Cron>() {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: invalid cron expression `{}`: {}",
                job.name, job.schedule, e
            ),
        });
    }
}

fn check_duplicate_job_names(
    jobs: &[JobConfig],
    path: &Path,
    raw: &str,
    errors: &mut Vec<ConfigError>,
) {
    // Find line numbers by scanning raw source for `name = "..."` matches in order.
    let mut first_seen: HashMap<&str, usize> = HashMap::new();
    let lines: Vec<&str> = raw.lines().collect();

    // Pre-compute (job_name, line_number) pairs from the raw text.
    let name_re = Regex::new(r#"^\s*name\s*=\s*"([^"]+)""#).unwrap();
    let mut occurrences: Vec<(String, usize)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(caps) = name_re.captures(line) {
            occurrences.push((caps[1].to_string(), i + 1));
        }
    }

    for job in jobs {
        let hits: Vec<usize> = occurrences
            .iter()
            .filter(|(n, _)| n == &job.name)
            .map(|(_, ln)| *ln)
            .collect();
        if hits.len() > 1 && !first_seen.contains_key(job.name.as_str()) {
            first_seen.insert(&job.name, hits[0]);
            for &dup_line in hits.iter().skip(1) {
                errors.push(ConfigError {
                    file: path.into(),
                    line: dup_line,
                    col: 1,
                    message: format!(
                        "duplicate job name `{}` (first declared at {}:{})",
                        job.name,
                        path.display(),
                        hits[0]
                    ),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iana_tz_accepted() {
        let mut e = Vec::new();
        check_timezone("America/Los_Angeles", Path::new("x"), &mut e);
        assert!(e.is_empty());
    }

    #[test]
    fn iana_tz_rejected() {
        let mut e = Vec::new();
        check_timezone("America/Los_Angles", Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(e[0].message.contains("Los_Angles"));
    }

    #[test]
    fn network_mode_container_accepted() {
        assert!(NETWORK_RE.is_match("container:vpn"));
        assert!(NETWORK_RE.is_match("bridge"));
        assert!(NETWORK_RE.is_match("host"));
        assert!(NETWORK_RE.is_match("none"));
        assert!(NETWORK_RE.is_match("my_net"));
    }

    #[test]
    fn network_mode_whitespace_rejected() {
        assert!(!NETWORK_RE.is_match("container: vpn"));
        assert!(!NETWORK_RE.is_match(""));
    }

    fn stub_job(schedule: &str) -> JobConfig {
        JobConfig {
            name: "test-job".into(),
            schedule: schedule.into(),
            command: Some("echo hi".into()),
            script: None,
            image: None,
            use_defaults: None,
            env: Default::default(),
            volumes: None,
            network: None,
            container_name: None,
            timeout: None,
            delete: None,
            cmd: None,
        }
    }

    #[test]
    fn schedule_valid_5field_accepted() {
        let mut e = Vec::new();
        check_schedule(&stub_job("*/5 * * * *"), Path::new("x"), &mut e);
        assert!(e.is_empty());
    }

    #[test]
    fn schedule_invalid_rejected() {
        let mut e = Vec::new();
        check_schedule(&stub_job("foo bar baz"), Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(e[0].message.contains("invalid cron"));
        assert!(e[0].message.contains("test-job"));
    }

    #[test]
    fn schedule_l_modifier_accepted() {
        let mut e = Vec::new();
        check_schedule(&stub_job("0 3 L * *"), Path::new("x"), &mut e);
        assert!(e.is_empty());
    }

    #[test]
    fn schedule_empty_rejected() {
        let mut e = Vec::new();
        check_schedule(&stub_job(""), Path::new("x"), &mut e);
        assert!(!e.is_empty());
    }

    #[test]
    fn check_one_of_job_type_error_mentions_defaults() {
        // Issue #20: when a user relies on [defaults].image but typos the
        // defaults section away, the error must tell them `image` can come
        // from [defaults] so they know where else to look.
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.script = None;
        job.image = None;
        let mut e = Vec::new();
        check_one_of_job_type(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(
            e[0].message.contains("[defaults]"),
            "error must point at [defaults] as a valid source of `image`: {}",
            e[0].message
        );
        assert!(
            e[0].message.contains("use_defaults"),
            "error must mention `use_defaults` as the opt-out knob: {}",
            e[0].message
        );
    }

    #[test]
    fn check_cmd_only_on_docker_jobs_rejects_on_command_job() {
        // A command job with `cmd = [...]` is nonsense — there's no container
        // to pass the args to. Pre-fix this was silently accepted and the
        // `cmd` field was dropped from serialization because command jobs
        // never read `config_json` back through DockerJobConfig. Reject
        // loudly so the operator fixes the config intent.
        let mut job = stub_job("*/5 * * * *");
        // stub_job defaults: command = Some, image = None — i.e. a command job.
        job.cmd = Some(vec!["echo".to_string(), "hi".to_string()]);
        let mut e = Vec::new();
        check_cmd_only_on_docker_jobs(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(
            e[0].message.contains("test-job"),
            "error must name the job: {}",
            e[0].message
        );
        assert!(
            e[0].message.contains("cmd"),
            "error must name the offending field: {}",
            e[0].message
        );
        assert!(
            e[0].message.contains("docker jobs"),
            "error must explain cmd is docker-only: {}",
            e[0].message
        );
    }

    #[test]
    fn check_cmd_only_on_docker_jobs_rejects_on_script_job() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.script = Some("#!/bin/sh\necho hi\n".to_string());
        job.cmd = Some(vec!["ignored".to_string()]);
        let mut e = Vec::new();
        check_cmd_only_on_docker_jobs(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(e[0].message.contains("test-job"));
        assert!(e[0].message.contains("docker jobs"));
    }

    #[test]
    fn check_cmd_only_on_docker_jobs_accepts_docker_job_with_cmd() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".to_string());
        job.cmd = Some(vec!["echo".to_string(), "hi".to_string()]);
        let mut e = Vec::new();
        check_cmd_only_on_docker_jobs(&job, Path::new("x"), &mut e);
        assert!(e.is_empty(), "docker job with cmd must pass: got {e:?}");
    }

    #[test]
    fn check_cmd_only_on_docker_jobs_accepts_when_cmd_is_none() {
        // Default case — no cmd set on any job type, validator is a no-op.
        let job = stub_job("*/5 * * * *"); // command job, cmd = None
        let mut e = Vec::new();
        check_cmd_only_on_docker_jobs(&job, Path::new("x"), &mut e);
        assert!(e.is_empty());
    }
}
