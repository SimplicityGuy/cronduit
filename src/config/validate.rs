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

static LABEL_KEY_RE: Lazy<Regex> = Lazy::new(|| {
    // Strict ASCII: leading char alphanumeric or underscore; subsequent chars
    // alphanumeric, dot, hyphen, or underscore. Per CONTEXT D-02; mirrors the
    // once_cell idiom at validate.rs:10-13 and interpolate.rs:23-24.
    Regex::new(r"^[a-zA-Z0-9_][a-zA-Z0-9._-]*$").unwrap()
});

/// Maximum byte length for an individual label value (LBL-06).
/// Aligns with Docker's documented label-value convention.
const MAX_LABEL_VALUE_BYTES: usize = 4 * 1024; // 4 KB

/// Maximum total byte length of all keys + values for a single job's labels (LBL-06).
/// Cronduit-side; well below dockerd's informal ~250 KB limit so operators see a
/// clear cronduit error at config-load instead of a confusing dockerd 400 at create.
const MAX_LABEL_SET_BYTES: usize = 32 * 1024; // 32 KB

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
        // Phase 17 / SEED-001 — operator labels (LBL-03, LBL-04, LBL-06, D-02)
        check_label_reserved_namespace(job, path, errors);
        check_labels_only_on_docker_jobs(job, path, errors);
        check_label_size_limits(job, path, errors);
        check_label_key_chars(job, path, errors);
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

/// LBL-03: reject operator labels under the reserved `cronduit.*` namespace.
/// The cronduit.* prefix is reserved for cronduit-internal labels (currently
/// cronduit.run_id, cronduit.job_name; consumed by docker_orphan reconciliation
/// at src/scheduler/docker_orphan.rs:31). Sorting the offending-key list is
/// CRITICAL — HashMap iteration is non-deterministic (see RESEARCH Pitfall 2).
fn check_label_reserved_namespace(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(labels) = &job.labels else { return };
    let mut offending: Vec<&str> = labels
        .keys()
        .filter(|k| k.starts_with("cronduit."))
        .map(String::as_str)
        .collect();
    if offending.is_empty() {
        return;
    }
    offending.sort(); // determinism — HashMap iter order is random
    errors.push(ConfigError {
        file: path.into(),
        line: 0,
        col: 0,
        message: format!(
            "[[jobs]] `{}`: labels under reserved namespace `cronduit.*` are not allowed: {}. Remove these keys; the cronduit.* prefix is reserved for cronduit-internal labels.",
            job.name,
            offending.join(", ")
        ),
    });
}

/// LBL-04: reject `labels = ...` on non-docker (command/script) jobs.
/// Mirrors check_cmd_only_on_docker_jobs (validate.rs:89). Runs AFTER
/// apply_defaults so `image.is_none()` reliably distinguishes non-docker.
fn check_labels_only_on_docker_jobs(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    if job.labels.is_some() && job.image.is_none() {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: `labels` is only valid on docker jobs (job with `image = \"...\"` set either directly or via `[defaults].image`); command and script jobs cannot set `labels` because there is no container to attach them to. Remove the `labels` block, or switch the job to a docker job by setting `image`.",
                job.name
            ),
        });
    }
}

/// LBL-06: enforce per-value (4 KB) and per-set (32 KB) byte-length limits.
/// Two independent checks may both fire for one job (per D-01 aggregation).
fn check_label_size_limits(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(labels) = &job.labels else { return };

    // Per-value check
    let mut oversized_keys: Vec<&str> = labels
        .iter()
        .filter(|(_, v)| v.len() > MAX_LABEL_VALUE_BYTES)
        .map(|(k, _)| k.as_str())
        .collect();
    if !oversized_keys.is_empty() {
        oversized_keys.sort();
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: label values exceed 4 KB limit: {}. Each label value must be ≤ {} bytes.",
                job.name,
                oversized_keys.join(", "),
                MAX_LABEL_VALUE_BYTES
            ),
        });
    }

    // Per-job total check
    let total_bytes: usize = labels.iter().map(|(k, v)| k.len() + v.len()).sum();
    if total_bytes > MAX_LABEL_SET_BYTES {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: total label-set size {} bytes exceeds 32 KB limit. Sum of all key+value byte lengths must be ≤ {} bytes.",
                job.name, total_bytes, MAX_LABEL_SET_BYTES
            ),
        });
    }
}

/// D-02: enforce strict ASCII char regex on label keys.
/// Partially enforces LBL-05's "keys are NOT interpolated" — leftover `${`/`}`
/// chars after a failed/unintended interpolation are rejected here.
/// Sort before format (RESEARCH Pitfall 2).
fn check_label_key_chars(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(labels) = &job.labels else { return };
    let mut invalid: Vec<&str> = labels
        .keys()
        .filter(|k| !LABEL_KEY_RE.is_match(k))
        .map(String::as_str)
        .collect();
    if invalid.is_empty() {
        return;
    }
    invalid.sort();
    errors.push(ConfigError {
        file: path.into(),
        line: 0,
        col: 0,
        message: format!(
            "[[jobs]] `{}`: invalid label keys: {}. Keys must match the pattern `^[a-zA-Z0-9_][a-zA-Z0-9._-]*$` (alphanumeric or underscore start; alphanumeric, dot, hyphen, or underscore body). Note: env-var ${{...}} interpolation runs only on label VALUES, not keys.",
            job.name,
            invalid.join(", ")
        ),
    });
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
            labels: None,
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

    // ---- LBL-03 reserved-namespace ----

    #[test]
    fn check_label_reserved_namespace_rejects_cronduit_prefix() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert("cronduit.foo".to_string(), "v".to_string());
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_reserved_namespace(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(e[0].message.contains("cronduit.foo"));
        assert!(e[0].message.contains("reserved namespace"));
    }

    #[test]
    fn check_label_reserved_namespace_lists_multiple_keys_sorted() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert("cronduit.zeta".to_string(), "v".to_string());
        labels.insert("cronduit.alpha".to_string(), "v".to_string());
        labels.insert("cronduit.mid".to_string(), "v".to_string());
        labels.insert("traefik.enable".to_string(), "true".to_string()); // not reserved
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_reserved_namespace(&job, Path::new("x"), &mut e);
        assert_eq!(
            e.len(),
            1,
            "D-01: one ConfigError per job per violation type"
        );
        // Determinism: alphabetical ordering. Per RESEARCH Pitfall 2.
        let pos_alpha = e[0].message.find("cronduit.alpha").expect("contains alpha");
        let pos_mid = e[0].message.find("cronduit.mid").expect("contains mid");
        let pos_zeta = e[0].message.find("cronduit.zeta").expect("contains zeta");
        assert!(pos_alpha < pos_mid, "alphabetical: alpha before mid");
        assert!(pos_mid < pos_zeta, "alphabetical: mid before zeta");
        assert!(
            !e[0].message.contains("traefik.enable"),
            "non-reserved key not listed"
        );
    }

    #[test]
    fn check_label_reserved_namespace_accepts_cronduit_underscore_keys() {
        // Per RESEARCH Edge Case 8.5 — `cronduit_foo` does NOT start with `cronduit.`
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert("cronduit_foo".to_string(), "v".to_string());
        labels.insert("cronduitfoo".to_string(), "v".to_string());
        labels.insert("cronduit-foo".to_string(), "v".to_string());
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_reserved_namespace(&job, Path::new("x"), &mut e);
        assert!(
            e.is_empty(),
            "underscore/no-separator forms must pass: got {e:?}"
        );
    }

    #[test]
    fn check_label_reserved_namespace_accepts_none() {
        let job = stub_job("*/5 * * * *"); // labels: None
        let mut e = Vec::new();
        check_label_reserved_namespace(&job, Path::new("x"), &mut e);
        assert!(e.is_empty());
    }

    // ---- LBL-04 type-gate ----

    #[test]
    fn check_labels_only_on_docker_jobs_rejects_on_command_job() {
        let mut job = stub_job("*/5 * * * *");
        // stub_job defaults command = Some("echo hi") and image = None — command job.
        let mut labels = std::collections::HashMap::new();
        labels.insert("a".to_string(), "v".to_string());
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_labels_only_on_docker_jobs(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(e[0].message.contains("test-job"));
        assert!(e[0].message.contains("docker jobs"));
        assert!(e[0].message.contains("labels"));
    }

    #[test]
    fn check_labels_only_on_docker_jobs_rejects_on_script_job() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.script = Some("echo hi".into());
        // image still None
        let mut labels = std::collections::HashMap::new();
        labels.insert("a".to_string(), "v".to_string());
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_labels_only_on_docker_jobs(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
    }

    #[test]
    fn check_labels_only_on_docker_jobs_accepts_docker_job() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert("a".to_string(), "v".to_string());
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_labels_only_on_docker_jobs(&job, Path::new("x"), &mut e);
        assert!(e.is_empty(), "docker job with labels must pass: got {e:?}");
    }

    // ---- LBL-06 size limits ----

    #[test]
    fn check_label_size_limits_rejects_per_value_over_4kb() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert("k".to_string(), "x".repeat(4097)); // 4097 bytes — over 4 KB
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_size_limits(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(e[0].message.contains("4 KB") || e[0].message.contains("4096"));
        assert!(e[0].message.contains("k"));
    }

    #[test]
    fn check_label_size_limits_accepts_value_at_4kb_boundary() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert("k".to_string(), "x".repeat(4096)); // exactly 4096 — must pass
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_size_limits(&job, Path::new("x"), &mut e);
        assert!(e.is_empty(), "4096-byte value must pass: got {e:?}");
    }

    #[test]
    fn check_label_size_limits_rejects_per_set_over_32kb() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        // Build a label set whose total bytes (keys + values) exceeds 32 KB but
        // keeps each value ≤ 4 KB so only the per-set check fires.
        for i in 0..10 {
            labels.insert(format!("key{:02}", i), "x".repeat(3500)); // ~3500 bytes each — 10 entries → ~35 KB
        }
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_size_limits(&job, Path::new("x"), &mut e);
        assert!(!e.is_empty(), "expected at least one error");
        assert!(
            e.iter().any(|err| err.message.contains("32 KB")),
            "expected a 32 KB error: got {e:?}"
        );
    }

    // ---- D-02 key chars ----

    #[test]
    fn check_label_key_chars_rejects_space_slash_empty() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert("my key".to_string(), "v".to_string()); // space
        labels.insert("foo/bar".to_string(), "v".to_string()); // slash
        labels.insert("".to_string(), "v".to_string()); // empty
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_key_chars(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1, "D-01: one ConfigError per violation type");
        assert!(e[0].message.contains("my key"));
        assert!(e[0].message.contains("foo/bar"));
    }

    #[test]
    fn check_label_key_chars_rejects_leading_dot() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert(".foo".to_string(), "v".to_string()); // leading char must be alphanumeric/underscore
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_key_chars(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
    }

    #[test]
    fn check_label_key_chars_accepts_dotted_and_underscore_keys() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert(
            "com.centurylinklabs.watchtower.enable".to_string(),
            "false".to_string(),
        );
        labels.insert("traefik.http.routers.x.rule".to_string(), "v".to_string());
        labels.insert("_internal".to_string(), "v".to_string());
        labels.insert("a-b-c".to_string(), "v".to_string());
        labels.insert("0starts_digit".to_string(), "v".to_string());
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_key_chars(&job, Path::new("x"), &mut e);
        assert!(e.is_empty(), "valid keys must all pass: got {e:?}");
    }
}
