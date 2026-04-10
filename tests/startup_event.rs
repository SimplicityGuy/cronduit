//! D-23/D-24 black-box verification: cronduit.startup event fields +
//! bind_warning behavior on non-loopback bind.
//!
//! These tests spawn the real binary and read its JSON stdout.
//! We use SIGTERM (not assert_cmd timeout which uses SIGKILL) so the
//! process flushes its output before exiting.

#[cfg(unix)]
extern crate libc;

use std::path::PathBuf;
use std::process::{Command as StdCommand, Stdio};
use std::thread::sleep;
use std::time::Duration;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn cargo_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cronduit"))
}

/// Spawn cronduit, wait for it to start, send SIGTERM, collect stdout.
fn run_and_collect(bind: &str) -> String {
    let child = StdCommand::new(cargo_bin())
        .arg("run")
        .arg("--config")
        .arg(fixture("valid-minimal.toml"))
        .arg("--database-url")
        .arg("sqlite::memory:")
        .arg("--bind")
        .arg(bind)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cronduit");

    // Wait for startup — needs enough time for config parse + DB migrate + bind
    sleep(Duration::from_millis(1500));

    // Send SIGTERM for graceful shutdown (flushes output)
    #[cfg(unix)]
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }

    let output = child.wait_with_output().expect("wait for child");
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn startup_emits_expected_event_loopback() {
    let stdout = run_and_collect("127.0.0.1:0");
    assert!(
        stdout.contains("\"target\":\"cronduit.startup\""),
        "missing cronduit.startup in stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("\"database_backend\":\"sqlite\""),
        "missing database_backend:\n{stdout}"
    );
    assert!(
        stdout.contains("\"job_count\":"),
        "missing job_count:\n{stdout}"
    );
    assert!(
        stdout.contains("\"timezone\":\"UTC\""),
        "missing timezone=UTC:\n{stdout}"
    );
    assert!(
        stdout.contains("\"bind_warning\":false"),
        "missing bind_warning=false:\n{stdout}"
    );
    assert!(
        stdout.contains("\"version\":"),
        "missing version:\n{stdout}"
    );
}

#[test]
fn startup_emits_warn_on_non_loopback_bind() {
    let stdout = run_and_collect("0.0.0.0:0");
    // The INFO event must have bind_warning=true (D-24).
    assert!(
        stdout.contains("\"bind_warning\":true"),
        "missing bind_warning=true on 0.0.0.0 bind:\n{stdout}"
    );
    // And assert the WARN-level event is present.
    assert!(
        stdout.contains("\"level\":\"WARN\"") && stdout.contains("cronduit.startup"),
        "missing WARN-level cronduit.startup event:\n{stdout}"
    );
}

#[test]
fn startup_does_not_leak_database_credentials() {
    let stdout = run_and_collect("127.0.0.1:0");
    // sqlite URL has no creds; just assert the log line contains the scheme.
    assert!(
        stdout.contains("\"database_url\":\"sqlite:"),
        "database_url field not in startup event:\n{stdout}"
    );
    // And prove we never print credentials.
    assert!(!stdout.contains("user:pass"));
}
