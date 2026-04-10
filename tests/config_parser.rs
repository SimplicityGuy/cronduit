//! Integration tests for cronduit::config::parse_and_validate.

use cronduit::config::parse_and_validate;
use std::path::PathBuf;
use std::sync::Mutex;

/// Serialize tests that mutate the process environment.
/// See `src/config/interpolate.rs` tests for rationale.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn valid_minimal_parses() {
    let r = parse_and_validate(&fixture("valid-minimal.toml"));
    assert!(r.is_ok(), "unexpected errors: {:?}", r.err());
    let p = r.unwrap();
    assert_eq!(p.config.server.timezone, "UTC");
    assert_eq!(p.config.jobs.len(), 1);
    assert_eq!(p.config.jobs[0].name, "hello");
}

#[test]
fn valid_everything_parses() {
    let r = parse_and_validate(&fixture("valid-everything.toml"));
    assert!(r.is_ok(), "unexpected errors: {:?}", r.err());
    let p = r.unwrap();
    assert_eq!(p.config.server.timezone, "America/Los_Angeles");
    assert_eq!(p.config.jobs.len(), 3);
    assert!(p.config.defaults.is_some());
}

#[test]
fn valid_with_secrets_parses_when_env_is_set() {
    let _guard = ENV_MUTEX.lock().unwrap();
    // SAFETY: ENV_MUTEX guarantees no concurrent env access from tests.
    unsafe {
        std::env::set_var("CRONDUIT_TEST_API_KEY", "hunter2");
    }
    let r = parse_and_validate(&fixture("valid-with-secrets.toml"));
    assert!(r.is_ok(), "unexpected errors: {:?}", r.err());
    // Verify Debug does not leak the secret value.
    let p = r.unwrap();
    let dbg = format!("{:?}", p.config);
    assert!(!dbg.contains("hunter2"), "Debug leaked secret: {dbg}");
}

#[test]
fn missing_timezone_rejected() {
    let r = parse_and_validate(&fixture("invalid-missing-timezone.toml"));
    let errs = r.unwrap_err();
    assert!(
        errs.iter().any(|e| e.message.contains("timezone") || e.message.contains("missing field")),
        "expected timezone error, got: {errs:?}"
    );
}

#[test]
fn duplicate_job_names_both_lines_reported() {
    let r = parse_and_validate(&fixture("invalid-duplicate-job.toml"));
    let errs = r.unwrap_err();
    let dup_errs: Vec<_> = errs.iter().filter(|e| e.message.contains("duplicate job name")).collect();
    assert!(!dup_errs.is_empty(), "expected duplicate error, got: {errs:?}");
    // The reported line should be non-zero (real source line from raw scan).
    assert!(dup_errs[0].line > 0, "duplicate error missing line number");
}

#[test]
fn missing_env_var_reports_name_and_is_not_fatal() {
    let _guard = ENV_MUTEX.lock().unwrap();
    // SAFETY: ENV_MUTEX guarantees no concurrent env access from tests.
    unsafe {
        std::env::remove_var("CRONDUIT_ABSOLUTELY_UNSET_VARIABLE_XYZ");
    }
    let r = parse_and_validate(&fixture("invalid-missing-env-var.toml"));
    let errs = r.unwrap_err();
    assert!(
        errs.iter().any(|e| e.message.contains("CRONDUIT_ABSOLUTELY_UNSET_VARIABLE_XYZ")),
        "expected missing-var error, got: {errs:?}"
    );
}

#[test]
fn multiple_job_types_rejected() {
    let r = parse_and_validate(&fixture("invalid-multiple-job-types.toml"));
    let errs = r.unwrap_err();
    assert!(
        errs.iter().any(|e| e.message.contains("exactly one of")),
        "expected one-of-type error, got: {errs:?}"
    );
}

#[test]
fn bad_network_mode_rejected() {
    let r = parse_and_validate(&fixture("invalid-bad-network.toml"));
    let errs = r.unwrap_err();
    assert!(
        errs.iter().any(|e| e.message.contains("invalid network mode")),
        "expected network-mode error, got: {errs:?}"
    );
}

#[test]
fn collects_all_errors_not_fail_fast() {
    let r = parse_and_validate(&fixture("invalid-multiple.toml"));
    let errs = r.unwrap_err();
    // Expect at least 2 errors: bad tz + duplicate name
    assert!(
        errs.len() >= 2,
        "expected >= 2 errors (collect-all), got {}: {errs:?}",
        errs.len()
    );
}
