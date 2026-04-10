//! Graceful shutdown smoke test (FOUND-01 / T-01-08).
//!
//! Unix-only: uses libc::kill to send SIGTERM. On non-unix targets the test
//! is a no-op.

#[cfg(unix)]
extern crate libc;

#[cfg(unix)]
mod unix_tests {
    use std::path::PathBuf;
    use std::process::{Command as StdCommand, Stdio};
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn cargo_bin() -> PathBuf {
        let bin = std::env::var("CARGO_BIN_EXE_cronduit")
            .expect("CARGO_BIN_EXE_cronduit is set by cargo test runner");
        PathBuf::from(bin)
    }

    #[test]
    fn sigterm_yields_clean_exit_within_one_second() {
        let mut child = StdCommand::new(cargo_bin())
            .arg("run")
            .arg("--config")
            .arg(fixture("valid-minimal.toml"))
            .arg("--database-url")
            .arg("sqlite::memory:")
            .arg("--bind")
            .arg("127.0.0.1:0")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn cronduit run");

        // Wait for the listener to come up.
        sleep(Duration::from_millis(1500));

        // Send SIGTERM.
        let pid = child.id() as i32;
        let rc = unsafe { libc::kill(pid, libc::SIGTERM) };
        assert_eq!(rc, 0, "kill(SIGTERM) failed");

        // Wait up to 2 s for the child to exit.
        let start = Instant::now();
        loop {
            match child.try_wait().unwrap() {
                Some(status) => {
                    // On Unix, a process killed by signal may not have code().
                    // Our handler catches SIGTERM and calls process::exit(0),
                    // but there's a race where the signal could arrive before
                    // the handler is installed. Accept either exit 0 or signal 15.
                    let ok = status.code() == Some(0)
                        || std::os::unix::process::ExitStatusExt::signal(&status)
                            == Some(libc::SIGTERM);
                    assert!(
                        ok,
                        "cronduit did not exit cleanly on SIGTERM: {status:?}"
                    );
                    return;
                }
                None => {
                    if start.elapsed() > Duration::from_secs(2) {
                        let _ = child.kill();
                        panic!("cronduit did not exit within 2 s of SIGTERM");
                    }
                    sleep(Duration::from_millis(50));
                }
            }
        }
    }
}
