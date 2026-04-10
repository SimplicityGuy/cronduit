use super::{Config, ConfigError, JobConfig};
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
pub fn run_all_checks(
    cfg: &Config,
    path: &Path,
    raw: &str,
    raw_doc: Option<&toml::Value>,
    errors: &mut Vec<ConfigError>,
) {
    check_timezone(&cfg.server.timezone, path, errors);
    check_bind(&cfg.server.bind, path, errors);
    check_duplicate_job_names(&cfg.jobs, path, raw, raw_doc, errors);
    for job in &cfg.jobs {
        check_one_of_job_type(job, path, errors);
        check_network_mode(job, path, errors);
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
                "[[jobs]] `{}` must declare exactly one of `command`, `script`, or `image` (found {count})",
                job.name
            ),
        });
    }
}

fn check_network_mode(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    if let Some(net) = &job.network {
        if !NETWORK_RE.is_match(net) {
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
}

fn check_duplicate_job_names(
    jobs: &[JobConfig],
    path: &Path,
    raw: &str,
    _raw_doc: Option<&toml::Value>,
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
}
