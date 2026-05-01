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
    /// Operator-defined Docker labels attached to spawned containers.
    /// Per LBL-01..06 / SEED-001. Merged with cronduit-internal labels
    /// at container-create time. `cronduit.*` namespace reserved (LBL-03).
    /// Type-gated to docker jobs only (LBL-04). Per-value 4 KB / per-set
    /// 32 KB byte-length limits (LBL-06).
    #[serde(default)]
    pub labels: Option<std::collections::HashMap<String, String>>,
    pub delete: Option<bool>,
    #[serde(default, with = "humantime_serde::option")]
    pub timeout: Option<Duration>,
    #[serde(default, with = "humantime_serde::option")]
    pub random_min_gap: Option<Duration>,
    /// Operator-defined webhook delivery configuration applied to all jobs.
    /// Per WH-01 / D-01..D-05. Per-job `webhook` always wins; this is taken
    /// only when `job.webhook` is None and `job.use_defaults != Some(false)`.
    /// See `src/config/defaults.rs::apply_defaults` and `WebhookConfig` below.
    /// Single inline block — replace-on-collision merge (NOT a HashMap union
    /// like `labels`). Validators in `validate.rs` reject malformed shapes.
    #[serde(default)]
    pub webhook: Option<WebhookConfig>,
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
    /// Operator-defined Docker labels attached to spawned containers.
    /// Per LBL-01..06 / SEED-001. Merged with cronduit-internal labels
    /// at container-create time. `cronduit.*` namespace reserved (LBL-03).
    /// Type-gated to docker jobs only (LBL-04). Per-value 4 KB / per-set
    /// 32 KB byte-length limits (LBL-06).
    #[serde(default)]
    pub labels: Option<std::collections::HashMap<String, String>>,
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
    /// Operator-defined webhook delivery configuration for this job.
    /// Per WH-01 / D-01..D-05. When None, `apply_defaults` may fill it from
    /// `[defaults].webhook` (replace-on-collision; see `defaults.rs`).
    /// `use_defaults = Some(false)` disables the inheritance.
    ///
    /// NOTE: `webhook` does NOT enter `DockerJobConfig` / `serialize_config_json` /
    /// `compute_config_hash` — the 5-layer parity invariant from Phase 17 LBL
    /// applies to docker-execution surface only. The dispatcher reads webhook
    /// config from a per-job `Arc<HashMap<i64, WebhookConfig>>` built at the
    /// bin layer (Plan 05) — not from `config_json`. See RESEARCH Open Q 1.
    #[serde(default)]
    pub webhook: Option<WebhookConfig>,
}

/// Per-job and `[defaults]` webhook configuration.
/// Phase 18 — WH-01 / D-02. Schema:
///   webhook = {
///     url        = "https://hook.example.com/...",   # required
///     states     = ["failed", "timeout"],            # default
///     secret     = "${WEBHOOK_SECRET}",              # required UNLESS unsigned=true
///     unsigned   = false,                            # default
///     fire_every = 1                                 # 1=first-of-stream (default); 0=every; N=every Nth match
///   }
///
/// `secret` is wrapped in `SecretString` to scrub Debug/Display per V8 Data
/// Protection (D-03). `${WEBHOOK_SECRET}` interpolation happens BEFORE TOML
/// parsing via `interpolate.rs::interpolate` (whole-file textual pass,
/// Phase 17 CR-01 truth).
#[derive(Debug, Deserialize, Clone)]
pub struct WebhookConfig {
    /// Required. Validated by `check_webhook_url` (`http`/`https` schemes only in Phase 18;
    /// HTTPS-for-non-loopback enforcement is WH-07 / Phase 20).
    pub url: String,
    /// Default `["failed", "timeout"]` if omitted. Each entry must be one of:
    /// `"success" | "failed" | "timeout" | "stopped" | "cancelled" | "error"`.
    /// Empty `[]` is rejected (use absence of `webhook` block to disable).
    #[serde(default = "default_webhook_states")]
    pub states: Vec<String>,
    /// Required when `unsigned = false` (default). When `unsigned = true`, MUST be None.
    /// Wrapped in SecretString — scrubbed Debug/Display.
    #[serde(default)]
    pub secret: Option<SecretString>,
    /// When true, dispatcher omits the `webhook-signature` header entirely (D-05).
    /// Cronduit extension to Standard Webhooks v1 for receivers like Slack/Discord
    /// that don't HMAC-verify. Mutually exclusive with `secret`.
    #[serde(default)]
    pub unsigned: bool,
    /// Coalescing knob (D-16):
    ///   0   → always fire (legacy per-failure)
    ///   1   → first-of-stream (default; fires when filter_position == 1)
    ///   N>1 → every Nth match (fires when filter_position % N == 1: 1, N+1, 2N+1, ...)
    /// Negative values are rejected at validate.
    #[serde(default = "default_fire_every")]
    pub fire_every: i64,
}

fn default_webhook_states() -> Vec<String> {
    vec!["failed".to_string(), "timeout".to_string()]
}

fn default_fire_every() -> i64 {
    1
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

#[cfg(test)]
mod tests {
    #[test]
    fn webhook_config_parses_per_job() {
        let toml_text = r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[[jobs]]
name = "j1"
schedule = "* * * * *"
command = "true"
webhook = { url = "https://hook.example.com/x", states = ["failed"], secret = "shh", fire_every = 3 }
"#;
        let cfg: super::Config = toml::from_str(toml_text).expect("parse");
        let wh = cfg.jobs[0].webhook.as_ref().expect("webhook present");
        assert_eq!(wh.url, "https://hook.example.com/x");
        assert_eq!(wh.states, vec!["failed".to_string()]);
        assert!(wh.secret.is_some());
        assert!(!wh.unsigned);
        assert_eq!(wh.fire_every, 3);
    }

    #[test]
    fn webhook_config_parses_defaults_block() {
        let toml_text = r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[defaults]
webhook = { url = "https://hook.example.com/y", unsigned = true }

[[jobs]]
name = "j1"
schedule = "* * * * *"
command = "true"
"#;
        let cfg: super::Config = toml::from_str(toml_text).expect("parse");
        let wh = cfg
            .defaults
            .as_ref()
            .unwrap()
            .webhook
            .as_ref()
            .expect("defaults webhook");
        assert_eq!(wh.url, "https://hook.example.com/y");
        // Default-filled:
        assert_eq!(wh.states, vec!["failed".to_string(), "timeout".to_string()]);
        assert!(wh.secret.is_none());
        assert!(wh.unsigned);
        assert_eq!(wh.fire_every, 1);
    }

    #[test]
    fn webhook_config_states_default_when_omitted() {
        assert_eq!(
            super::default_webhook_states(),
            vec!["failed".to_string(), "timeout".to_string()]
        );
    }

    #[test]
    fn webhook_config_fire_every_default_when_omitted() {
        assert_eq!(super::default_fire_every(), 1);
    }

    // Phase 20 / WH-10 (Plan 06 Task 1): webhook_drain_grace tests
    //
    // These tests lock the new [server].webhook_drain_grace field's
    // humantime parsing + 30s default. The field is required for the
    // worker drain budget on SIGTERM; default 30s matches the spec
    // value used by Plan 04's bin-layer hardcode (D-15..D-18).
    #[test]
    fn webhook_drain_grace_default_is_30s() {
        let toml_text = r#"
[server]
bind = "127.0.0.1:8080"
timezone = "UTC"
database_url = "sqlite::memory:"
"#;
        let cfg: super::Config = toml::from_str(toml_text).expect("parse");
        assert_eq!(
            cfg.server.webhook_drain_grace,
            std::time::Duration::from_secs(30)
        );
    }

    #[test]
    fn webhook_drain_grace_humantime_parses_45s() {
        let toml_text = r#"
[server]
bind = "127.0.0.1:8080"
timezone = "UTC"
database_url = "sqlite::memory:"
webhook_drain_grace = "45s"
"#;
        let cfg: super::Config = toml::from_str(toml_text).expect("parse");
        assert_eq!(
            cfg.server.webhook_drain_grace,
            std::time::Duration::from_secs(45)
        );
    }

    #[test]
    fn default_webhook_drain_grace_returns_30s() {
        assert_eq!(
            super::default_webhook_drain_grace(),
            std::time::Duration::from_secs(30)
        );
    }
}
