//! Command execution backend.
//!
//! Executes command-type jobs via `tokio::process::Command` with
//! `shell-words` for argv splitting (no shell invocation).

use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::log_pipeline::{LogSender, make_log_line};

/// Status of a completed job run.
#[derive(Debug, Clone, PartialEq)]
pub enum RunStatus {
    /// Exited with code 0.
    Success,
    /// Exited with non-zero code.
    Failed,
    /// Killed due to timeout.
    Timeout,
    /// Cancelled due to graceful shutdown.
    Shutdown,
    /// Could not start or other error.
    Error,
    /// Killed by operator via UI Stop button (SCHED-09, Phase 10).
    Stopped,
}

/// Result of executing a command or script.
#[derive(Debug)]
pub struct ExecResult {
    /// The process exit code, if available.
    pub exit_code: Option<i32>,
    /// High-level status.
    pub status: RunStatus,
    /// Error message for Error/Timeout/Shutdown statuses.
    pub error_message: Option<String>,
}

/// Read lines from an async reader and send them to the log channel.
async fn read_lines_to_channel<R: AsyncRead + Unpin>(
    reader: R,
    stream_name: &str,
    sender: LogSender,
) {
    let buf_reader = BufReader::new(reader);
    let mut lines = buf_reader.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        sender.send(make_log_line(stream_name, line));
    }
}

/// Execute a child process with timeout and cancellation support.
///
/// Shared implementation used by both `execute_command` and `execute_script`.
/// Takes ownership of a spawned child, captures stdout/stderr to the log
/// channel, and handles timeout/cancellation via process group kill.
///
/// The cancel-arm distinguishes operator Stop from shutdown by reading
/// `control.reason()` AFTER `cancel.cancelled()` yields. `RunControl::stop`
/// orders the SeqCst store before the cancel fire, so the load below always
/// observes the correct reason. See `control.rs` module docs.
pub(crate) async fn execute_child(
    mut child: tokio::process::Child,
    timeout: Duration,
    cancel: CancellationToken,
    sender: LogSender,
    control: &crate::scheduler::control::RunControl,
) -> ExecResult {
    // Take stdout/stderr handles for line-by-line capture
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdout_sender = sender.clone();
    let stderr_sender = sender.clone();

    let stdout_task = tokio::spawn(async move {
        if let Some(out) = stdout {
            read_lines_to_channel(out, "stdout", stdout_sender).await;
        }
    });

    let stderr_task = tokio::spawn(async move {
        if let Some(err) = stderr {
            read_lines_to_channel(err, "stderr", stderr_sender).await;
        }
    });

    let result = tokio::select! {
        exit_result = child.wait() => {
            // Natural exit — wait for readers to finish
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            sender.close();

            match exit_result {
                Ok(status) => {
                    let code = status.code();
                    let run_status = if code == Some(0) {
                        RunStatus::Success
                    } else {
                        RunStatus::Failed
                    };
                    ExecResult {
                        exit_code: code,
                        status: run_status,
                        error_message: None,
                    }
                }
                Err(e) => {
                    ExecResult {
                        exit_code: None,
                        status: RunStatus::Error,
                        error_message: Some(format!("failed to wait on child: {e}")),
                    }
                }
            }
        }
        _ = tokio::time::sleep(timeout) => {
            // Timeout — kill process group
            kill_process_group(&child);
            let _ = child.wait().await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            sender.close();

            ExecResult {
                exit_code: None,
                status: RunStatus::Timeout,
                error_message: Some(format!("timed out after {timeout:?}")),
            }
        }
        _ = cancel.cancelled() => {
            // D-17 PRESERVATION: kill the child process group FIRST via pgid kill.
            // This is the `.process_group(0)` + `libc::kill(-pid, SIGKILL)` pattern
            // locked by SCHED-12 (see `kill_process_group` below). The tokio
            // drop-kill convenience was deliberately NOT adopted because it would
            // orphan shell-pipeline grandchildren (PITFALLS §1.3, Correction #1).
            kill_process_group(&child);
            let _ = child.wait().await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            sender.close();

            // Distinguish the cancel cause — operator vs shutdown. The SeqCst
            // ordering in `RunControl::stop` guarantees this load observes the
            // operator reason if the cancel came via the UI Stop button.
            let (status, msg) = match control.reason() {
                crate::scheduler::control::StopReason::Operator => (
                    RunStatus::Stopped,
                    "stopped by operator".to_string(),
                ),
                crate::scheduler::control::StopReason::Shutdown => (
                    RunStatus::Shutdown,
                    "cancelled due to shutdown".to_string(),
                ),
            };
            ExecResult {
                exit_code: None,
                status,
                error_message: Some(msg),
            }
        }
    };

    result
}

/// Kill the process group of a child process via SIGKILL.
///
/// Uses `process_group(0)` convention: the child's PID is its process
/// group leader, so `kill(-pid, SIGKILL)` kills the entire group.
fn kill_process_group(child: &tokio::process::Child) {
    if let Some(pid) = child.id() {
        let pid_i32: i32 = match pid.try_into() {
            Ok(p) => p,
            Err(_) => {
                tracing::error!(
                    target: "cronduit.executor",
                    pid,
                    "PID exceeds i32::MAX, cannot kill process group"
                );
                return;
            }
        };
        unsafe {
            libc::kill(-pid_i32, libc::SIGKILL);
        }
    }
}

/// Execute a command string by tokenizing it via `shell_words::split`.
///
/// The command is NOT passed to a shell — it is split into argv tokens
/// and executed directly via `tokio::process::Command`.
///
/// `control` carries the cancel token's stop reason so the cancel arm in
/// `execute_child` can distinguish operator-Stop from shutdown (SCHED-10).
pub async fn execute_command(
    command_str: &str,
    timeout: Duration,
    cancel: CancellationToken,
    sender: LogSender,
    control: &crate::scheduler::control::RunControl,
) -> ExecResult {
    let argv = match shell_words::split(command_str) {
        Ok(args) if args.is_empty() => {
            sender.close();
            return ExecResult {
                exit_code: None,
                status: RunStatus::Error,
                error_message: Some("empty command".to_string()),
            };
        }
        Ok(args) => args,
        Err(e) => {
            sender.close();
            return ExecResult {
                exit_code: None,
                status: RunStatus::Error,
                error_message: Some(format!("failed to parse command: {e}")),
            };
        }
    };

    let child = match Command::new(&argv[0])
        .args(&argv[1..])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .process_group(0)
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            sender.close();
            return ExecResult {
                exit_code: None,
                status: RunStatus::Error,
                error_message: Some(format!("failed to spawn command: {e}")),
            };
        }
    };

    execute_child(child, timeout, cancel, sender, control).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::control::{RunControl, StopReason};
    use crate::scheduler::log_pipeline;

    #[tokio::test]
    async fn execute_echo_captures_stdout() {
        let (tx, rx) = log_pipeline::channel(256);
        let cancel = CancellationToken::new();
        let control = RunControl::new(cancel.clone());
        let result =
            execute_command("echo hello", Duration::from_secs(5), cancel, tx, &control).await;
        assert_eq!(result.status, RunStatus::Success);
        assert_eq!(result.exit_code, Some(0));
        let batch = rx.drain_batch(256);
        let stdout_lines: Vec<_> = batch.iter().filter(|l| l.stream == "stdout").collect();
        assert!(!stdout_lines.is_empty());
        assert_eq!(stdout_lines[0].line, "hello");
    }

    #[tokio::test]
    async fn execute_stderr_capture() {
        let (tx, rx) = log_pipeline::channel(256);
        let cancel = CancellationToken::new();
        let control = RunControl::new(cancel.clone());
        let result = execute_command(
            "sh -c 'echo err >&2'",
            Duration::from_secs(5),
            cancel,
            tx,
            &control,
        )
        .await;
        assert_eq!(result.status, RunStatus::Success);
        let batch = rx.drain_batch(256);
        let stderr_lines: Vec<_> = batch.iter().filter(|l| l.stream == "stderr").collect();
        assert!(!stderr_lines.is_empty());
        assert_eq!(stderr_lines[0].line, "err");
    }

    #[tokio::test]
    async fn execute_nonzero_exit() {
        let (tx, _rx) = log_pipeline::channel(256);
        let cancel = CancellationToken::new();
        let control = RunControl::new(cancel.clone());
        let result = execute_command(
            "sh -c 'exit 42'",
            Duration::from_secs(5),
            cancel,
            tx,
            &control,
        )
        .await;
        assert_eq!(result.status, RunStatus::Failed);
        assert_eq!(result.exit_code, Some(42));
    }

    #[tokio::test]
    async fn execute_timeout() {
        let (tx, rx) = log_pipeline::channel(256);
        let cancel = CancellationToken::new();
        let control = RunControl::new(cancel.clone());
        let result = execute_command(
            "sh -c 'echo before-timeout; sleep 30'",
            Duration::from_millis(200),
            cancel,
            tx,
            &control,
        )
        .await;
        assert_eq!(result.status, RunStatus::Timeout);
        // Partial logs should be preserved
        let batch = rx.drain_batch(256);
        let stdout_lines: Vec<_> = batch.iter().filter(|l| l.stream == "stdout").collect();
        assert!(
            stdout_lines.iter().any(|l| l.line == "before-timeout"),
            "partial logs should be preserved on timeout"
        );
    }

    #[tokio::test]
    async fn execute_shutdown_cancellation() {
        let (tx, _rx) = log_pipeline::channel(256);
        let cancel = CancellationToken::new();
        let control = RunControl::new(cancel.clone());
        cancel.cancel(); // cancel immediately
        let result =
            execute_command("sleep 30", Duration::from_secs(30), cancel, tx, &control).await;
        assert_eq!(result.status, RunStatus::Shutdown);
    }

    /// T-V11-STOP-09 (command variant): RunControl::stop(Operator) must
    /// produce RunStatus::Stopped with an "operator" error_message.
    #[tokio::test]
    async fn stop_operator_yields_stopped() {
        let (tx, _rx) = log_pipeline::channel(256);
        let cancel = CancellationToken::new();
        let control = RunControl::new(cancel.clone());
        let control_for_stop = control.clone();

        let handle = tokio::spawn(async move {
            execute_command(
                "sleep 30",
                Duration::from_secs(600),
                cancel,
                tx,
                &control,
            )
            .await
        });

        tokio::time::sleep(Duration::from_millis(100)).await;
        control_for_stop.stop(StopReason::Operator);

        let result = handle.await.unwrap();
        assert_eq!(result.status, RunStatus::Stopped);
        assert!(
            result
                .error_message
                .as_deref()
                .unwrap_or("")
                .contains("operator"),
            "expected 'operator' in error_message, got: {:?}",
            result.error_message
        );
    }

    /// Regression lock: cancelling the underlying token directly (NOT via
    /// RunControl::stop) must still classify as Shutdown. This protects the
    /// existing graceful shutdown semantics.
    #[tokio::test]
    async fn shutdown_cancel_yields_shutdown() {
        let (tx, _rx) = log_pipeline::channel(256);
        let cancel = CancellationToken::new();
        let control = RunControl::new(cancel.clone());
        let cancel_for_shutdown = cancel.clone();

        let handle = tokio::spawn(async move {
            execute_command(
                "sleep 30",
                Duration::from_secs(600),
                cancel,
                tx,
                &control,
            )
            .await
        });

        tokio::time::sleep(Duration::from_millis(100)).await;
        cancel_for_shutdown.cancel();

        let result = handle.await.unwrap();
        assert_eq!(result.status, RunStatus::Shutdown);
        assert!(
            result
                .error_message
                .as_deref()
                .unwrap_or("")
                .contains("shutdown"),
            "expected 'shutdown' in error_message, got: {:?}",
            result.error_message
        );
    }

    #[test]
    fn shell_words_parsing() {
        let args = shell_words::split("curl -sf 'https://example.com'").unwrap();
        assert_eq!(args, vec!["curl", "-sf", "https://example.com"]);
    }
}
