//! cronduit TOML config parsing, env interpolation, and validation.
//!
//! This is the shared core used by BOTH `cronduit check` (Plan 03) and
//! `cronduit run` (Plan 04). It never touches the database.

pub mod defaults;
pub mod errors;
pub mod hash;
pub mod interpolate;
pub mod validate;

use secrecy::SecretString;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use errors::{ConfigError, byte_offset_to_line_col};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    /// `[defaults]` section. After `parse_and_validate` returns, per-job
    /// merging has already happened (see `crate::config::defaults::apply_defaults`)
    /// so downstream code MUST NOT re-consult `Config.defaults` for
    /// `image`/`network`/`volumes`/`timeout`/`delete`. The ONLY legitimate
    /// post-parse consumer is `random_min_gap` in `src/cli/run.rs` and
    /// `src/scheduler/reload.rs` -- that field is a global scheduler knob,
    /// not a per-job field.
    #[serde(default)]
    pub defaults: Option<DefaultsConfig>,
    #[serde(default, rename = "jobs")]
    pub jobs: Vec<JobConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_db_url")]
    pub database_url: SecretString,
    /// MANDATORY (D-19). No implicit host-timezone fallback.
    pub timezone: String,
    #[serde(default = "default_shutdown_grace", with = "humantime_serde")]
    pub shutdown_grace: Duration,
    #[serde(default = "default_log_retention", with = "humantime_serde")]
    pub log_retention: Duration,
    /// Enable file watcher for automatic config reload (D-10, RELOAD-03).
    /// Default: true. Disable with `watch_config = false` in `[server]`.
    #[serde(default = "default_watch_config")]
    pub watch_config: bool,
}

fn default_bind() -> String {
    "127.0.0.1:8080".into()
}
fn default_db_url() -> SecretString {
    // Docker quickstart sets DATABASE_URL=sqlite:///data/cronduit.db via compose.
    // Local dev falls back to a writable path relative to CWD so `cargo run`
    // and `cronduit check` work without extra setup.
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://./cronduit.db?mode=rwc".to_string());
    SecretString::from(url)
}
fn default_shutdown_grace() -> Duration {
    Duration::from_secs(30)
}
fn default_log_retention() -> Duration {
    Duration::from_secs(60 * 60 * 24 * 90)
}
fn default_watch_config() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct DefaultsConfig {
    pub image: Option<String>,
    pub network: Option<String>,
    pub volumes: Option<Vec<String>>,
    pub delete: Option<bool>,
    #[serde(default, with = "humantime_serde::option")]
    pub timeout: Option<Duration>,
    #[serde(default, with = "humantime_serde::option")]
    pub random_min_gap: Option<Duration>,
}

#[derive(Debug, Deserialize)]
pub struct JobConfig {
    pub name: String,
    pub schedule: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    /// None = defaults apply; Some(false) = disable defaults (CONF-04).
    #[serde(default)]
    pub use_defaults: Option<bool>,
    #[serde(default)]
    pub env: BTreeMap<String, SecretString>,
    pub volumes: Option<Vec<String>>,
    pub network: Option<String>,
    pub container_name: Option<String>,
    #[serde(default, with = "humantime_serde::option")]
    pub timeout: Option<Duration>,
    /// Cronduit-side container removal toggle. Merged from `[defaults].delete`
    /// when the job omits it. NOTE: honoring `delete = false` (keep failed
    /// containers for inspection) is a Known Gap -- the executor currently
    /// always removes. See plan 260414-gbf objective for follow-up issue.
    #[serde(default)]
    pub delete: Option<bool>,
    /// Override the Docker image's baked-in CMD. Per-job ONLY -- NOT
    /// defaults-eligible. When None, the container runs with the image's
    /// default CMD; when Some(vec), the vec is passed verbatim to bollard's
    /// ContainerCreateBody.cmd (note: `Some(vec![])` is a valid override
    /// meaning "run with NO command", semantically distinct from None).
    #[serde(default)]
    pub cmd: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct ParsedConfig {
    pub config: Config,
    pub source_path: PathBuf,
}

/// Shared by `cronduit check` and `cronduit run`. Never touches the DB.
///
/// Collects ALL errors into a Vec<ConfigError> (D-21 -- not fail-fast).
pub fn parse_and_validate(path: &Path) -> Result<ParsedConfig, Vec<ConfigError>> {
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            return Err(vec![ConfigError {
                file: path.into(),
                line: 0,
                col: 0,
                message: format!("cannot read file: {e}"),
            }]);
        }
    };

    let (interpolated, interp_errors) = interpolate::interpolate(&raw);

    let mut errors: Vec<ConfigError> = interp_errors
        .into_iter()
        .map(|e| {
            let (line, col) = byte_offset_to_line_col(&raw, e.byte_range.start);
            ConfigError {
                file: path.into(),
                line,
                col,
                message: match e.kind {
                    interpolate::ErrorKind::MissingVar(ref v) => {
                        format!("missing environment variable `${{{v}}}`")
                    }
                    interpolate::ErrorKind::DefaultSyntaxForbidden => {
                        "`${VAR:-default}` syntax is not supported in v1 \
                         -- use `${VAR}` and ensure the variable is set"
                            .to_string()
                    }
                },
            }
        })
        .collect();

    let mut parsed = match toml::from_str::<Config>(&interpolated) {
        Ok(c) => Some(c),
        Err(e) => {
            let (line, col) = e
                .span()
                .map(|r| byte_offset_to_line_col(&interpolated, r.start))
                .unwrap_or((0, 0));
            errors.push(ConfigError {
                file: path.into(),
                line,
                col,
                message: e.message().to_string(),
            });
            None
        }
    };

    if let Some(cfg) = &mut parsed {
        // Apply [defaults] to every job before validation so downstream consumers
        // (validator, sync, hash, executor) see already-merged jobs and never need
        // to re-read Config.defaults for per-job fields. The only exception is
        // `random_min_gap`, which is a global @random scheduler knob and stays
        // on Config.defaults -- see src/cli/run.rs and src/scheduler/reload.rs.
        let defaults = cfg.defaults.as_ref();
        cfg.jobs = std::mem::take(&mut cfg.jobs)
            .into_iter()
            .map(|j| crate::config::defaults::apply_defaults(j, defaults))
            .collect();
    }

    if let Some(cfg) = &parsed {
        validate::run_all_checks(cfg, path, &raw, &mut errors);
    }

    if errors.is_empty() {
        Ok(ParsedConfig {
            config: parsed.unwrap(),
            source_path: path.into(),
        })
    } else {
        Err(errors)
    }
}
