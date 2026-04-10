//! Black-box integration tests for `cronduit check`.
//!
//! These tests compile and invoke the real binary via assert_cmd.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn check_valid_minimal_exits_zero() {
    Command::cargo_bin("cronduit")
        .unwrap()
        .arg("check")
        .arg(fixture("valid-minimal.toml"))
        .assert()
        .success()
        .stderr(predicate::str::contains("ok:"));
}

#[test]
fn check_missing_timezone_reports_error() {
    Command::cargo_bin("cronduit")
        .unwrap()
        .arg("check")
        .arg(fixture("invalid-missing-timezone.toml"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("error:"))
        .stderr(
            predicate::str::contains("timezone")
                .or(predicate::str::contains("missing field")),
        );
}

#[test]
fn check_collects_all_errors() {
    // invalid-multiple.toml has a bad timezone AND a duplicate job name.
    let output = Command::cargo_bin("cronduit")
        .unwrap()
        .arg("check")
        .arg(fixture("invalid-multiple.toml"))
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8(output).unwrap();
    let error_lines = stderr.matches("error:").count();
    assert!(
        error_lines >= 2,
        "expected >= 2 `error:` lines (collect-all), got {error_lines} in:\n{stderr}"
    );
    assert!(
        stderr.contains(" error(s)"),
        "expected trailing `N error(s)` summary, got:\n{stderr}"
    );
}

#[test]
fn check_nonexistent_file_reports_cannot_read() {
    Command::cargo_bin("cronduit")
        .unwrap()
        .arg("check")
        .arg("/nonexistent/path/that/does/not/exist.toml")
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot read file"));
}

#[test]
fn check_does_not_open_db() {
    // Run `check` in a temp directory and confirm no .db / .db-wal / .db-shm
    // files are created as a side effect. This is a proxy for "no DB I/O".
    let tmp = tempfile::tempdir().unwrap();
    Command::cargo_bin("cronduit")
        .unwrap()
        .current_dir(tmp.path())
        .arg("check")
        .arg(fixture("valid-minimal.toml"))
        .assert()
        .success();

    // Walk tmp dir; assert no *.db* file exists.
    for entry in std::fs::read_dir(tmp.path()).unwrap() {
        let e = entry.unwrap();
        let name = e.file_name().to_string_lossy().to_string();
        assert!(
            !name.contains(".db"),
            "check created a database file: {name}"
        );
    }
}

#[test]
fn check_invalid_schedule_exits_nonzero() {
    Command::cargo_bin("cronduit")
        .unwrap()
        .arg("check")
        .arg(fixture("invalid-schedule.toml"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid cron expression"));
}

#[test]
fn check_does_not_leak_secret_value() {
    // Set a distinctive secret in env and run check on the secrets fixture.
    // Assert the VALUE never appears in stdout or stderr -- only the VAR NAME
    // is allowed.
    let secret_value = "SUPER_SECRET_PASSWORD_MARKER_99XYZ";
    let out = Command::cargo_bin("cronduit")
        .unwrap()
        .env("CRONDUIT_TEST_API_KEY", secret_value)
        .arg("check")
        .arg(fixture("valid-with-secrets.toml"))
        .assert()
        .get_output()
        .clone();

    let stdout = String::from_utf8(out.stdout).unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        !stdout.contains(secret_value),
        "stdout leaked secret value: {stdout}"
    );
    assert!(
        !stderr.contains(secret_value),
        "stderr leaked secret value: {stderr}"
    );
}
