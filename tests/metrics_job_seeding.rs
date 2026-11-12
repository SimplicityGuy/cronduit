//! cronduit-dkt: per-job zero-seeding of `cronduit_runs_total{job,status}` and
//! `cronduit_run_failures_total{job,reason}` at boot and on reload, so a job's
//! first failure is visible to `increase()`.
//!
//! One test binary == one process-global recorder, so every test uses unique
//! job names and reads only its own label sets.

use std::collections::HashMap;
use std::io::Write;
use std::time::Duration;

use tempfile::NamedTempFile;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use cronduit::config::parse_and_validate;
use cronduit::db::DbPool;
use cronduit::db::queries::{self, DbJob};
use cronduit::scheduler::reload::do_reload;
use cronduit::scheduler::run::run_job;
use cronduit::scheduler::sync::sync_config_to_db;
use cronduit::telemetry::{RUN_STATUSES, seed_job_metrics, setup_metrics};

const REASONS: [&str; 7] = [
    "image_pull_failed",
    "network_target_unavailable",
    "timeout",
    "exit_nonzero",
    "oom",
    "abandoned",
    "unknown",
];

/// Value of the exact series line `series` (e.g. `name{a="b"}`), if rendered.
fn sample(body: &str, series: &str) -> Option<f64> {
    body.lines()
        .find_map(|l| l.strip_prefix(series)?.strip_prefix(' '))
        .map(|v| v.trim().parse().expect("numeric sample"))
}

fn run_series(job: &str, status: &str) -> String {
    format!(r#"cronduit_runs_total{{job="{job}",status="{status}"}}"#)
}

fn failure_series(job: &str, reason: &str) -> String {
    format!(r#"cronduit_run_failures_total{{job="{job}",reason="{reason}"}}"#)
}

fn assert_seeded(body: &str, job: &str) {
    for s in RUN_STATUSES {
        assert_eq!(
            sample(body, &run_series(job, s)),
            Some(0.0),
            "{job}/{s}:\n{body}"
        );
    }
    for r in REASONS {
        assert_eq!(
            sample(body, &failure_series(job, r)),
            Some(0.0),
            "{job}/{r}:\n{body}"
        );
    }
}

fn write_config(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("tempfile");
    f.write_all(content.as_bytes()).expect("write");
    f.flush().expect("flush");
    f
}

async fn pool() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    pool.migrate().await.unwrap();
    pool
}

#[tokio::test]
async fn boot_seeds_every_job_status_and_reason() {
    let handle = setup_metrics();
    let pool = pool().await;
    let cfg = write_config(
        r#"
[server]
timezone = "UTC"

[[jobs]]
name = "seed-boot-a"
schedule = "0 0 31 2 *"
command = "echo a"

[[jobs]]
name = "seed-boot-b"
schedule = "0 0 31 2 *"
command = "echo b"
"#,
    );
    let parsed = parse_and_validate(cfg.path()).expect("parse");
    // Same two calls cli::run makes at boot.
    let sync = sync_config_to_db(&pool, &parsed.config, Duration::from_secs(0))
        .await
        .unwrap();
    seed_job_metrics(sync.jobs.iter().map(|j| j.name.as_str()));

    let body = handle.render();
    assert_seeded(&body, "seed-boot-a");
    assert_seeded(&body, "seed-boot-b");
    // The never-moving status-only series are gone.
    assert!(!body.contains(r#"cronduit_runs_total{status="#), "{body}");
}

#[tokio::test]
async fn reload_adding_a_job_seeds_it() {
    let handle = setup_metrics();
    let pool = pool().await;
    let one = r#"
[server]
timezone = "UTC"

[[jobs]]
name = "seed-reload-a"
schedule = "0 0 31 2 *"
command = "echo a"
"#;
    let cfg = write_config(one);
    let mut jobs = HashMap::new();
    let (r, _) = do_reload(&pool, cfg.path(), &mut jobs, chrono_tz::UTC).await;
    assert_eq!(r.added, 1, "{:?}", r.error_message);
    let body = handle.render();
    assert_seeded(&body, "seed-reload-a");
    assert!(sample(&body, &run_series("seed-reload-b", "failed")).is_none());

    std::fs::write(
        cfg.path(),
        format!(
            "{one}\n[[jobs]]\nname = \"seed-reload-b\"\nschedule = \"0 0 31 2 *\"\ncommand = \"echo b\"\n"
        ),
    )
    .unwrap();
    let (r, _) = do_reload(&pool, cfg.path(), &mut jobs, chrono_tz::UTC).await;
    assert_eq!(r.added, 1, "{:?}", r.error_message);
    let body = handle.render();
    assert_seeded(&body, "seed-reload-a");
    assert_seeded(&body, "seed-reload-b");
}

#[tokio::test]
async fn first_failure_moves_the_seeded_series_from_0_to_1() {
    let handle = setup_metrics();
    let pool = pool().await;
    let name = "seed-fail-first";
    let id = queries::upsert_job(
        &pool,
        name,
        "0 0 31 2 *",
        "0 0 31 2 *",
        "command",
        r#"{"command":"false"}"#,
        "seed-hash",
        3600,
        "[]",
    )
    .await
    .unwrap();
    let job = DbJob {
        id,
        name: name.to_string(),
        schedule: "0 0 31 2 *".into(),
        resolved_schedule: "0 0 31 2 *".into(),
        job_type: "command".into(),
        config_json: r#"{"command":"false"}"#.into(),
        config_hash: "seed-hash".into(),
        enabled: true,
        enabled_override: None,
        timeout_secs: 3600,
        created_at: String::new(),
        updated_at: String::new(),
    };
    seed_job_metrics([name]);

    let failed = run_series(name, "failed");
    let reason = failure_series(name, "exit_nonzero");
    let before = handle.render();
    assert_eq!(sample(&before, &failed), Some(0.0));
    assert_eq!(sample(&before, &reason), Some(0.0));

    let (wtx, _wrx) = cronduit::webhooks::channel_with_capacity(8);
    let active = std::sync::Arc::new(RwLock::new(HashMap::new()));
    let res = run_job(
        pool.clone(),
        None,
        job,
        "manual".to_string(),
        CancellationToken::new(),
        active,
        wtx,
        None,
    )
    .await;
    assert_eq!(res.status, "failed");

    // Same label sets: 0 sample before, 1 after -- what increase() needs.
    let after = handle.render();
    assert_eq!(sample(&after, &failed), Some(1.0));
    assert_eq!(sample(&after, &reason), Some(1.0));
}
