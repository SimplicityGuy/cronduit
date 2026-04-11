//! Docker log streaming via bollard.
//!
//! Streams stdout/stderr from a running container into the log pipeline.
//! Does NOT close the sender -- the caller manages channel lifecycle.

use bollard::container::LogOutput;
use bollard::query_parameters::LogsOptions;
use bollard::Docker;
use futures_util::StreamExt;

use super::log_pipeline::{LogSender, make_log_line};

/// Stream logs from a Docker container into the log pipeline.
///
/// Follows the container's stdout and stderr until the stream ends
/// (container exits or Docker closes the stream). Each chunk is split
/// on newlines to produce individual log lines.
///
/// Does NOT call `sender.close()` -- the caller is responsible for
/// closing the channel after all log sources are drained.
pub async fn stream_docker_logs(docker: Docker, container_id: String, sender: LogSender) {
    // Try follow mode first (streams logs in real-time until container exits).
    // If follow fails immediately (some Docker runtimes like Rancher Desktop),
    // retry once without follow to fetch whatever logs exist.
    for follow in [true, false] {
        let options = LogsOptions {
            follow,
            stdout: true,
            stderr: true,
            timestamps: false,
            ..Default::default()
        };

        let mut stream = docker.logs(&container_id, Some(options));
        let mut got_any = false;

        while let Some(result) = stream.next().await {
            match result {
                Ok(output) => {
                    let (stream_name, message) = match output {
                        LogOutput::StdOut { message } => ("stdout", message),
                        LogOutput::StdErr { message } => ("stderr", message),
                        LogOutput::Console { message } => ("stdout", message),
                        LogOutput::StdIn { message: _ } => continue,
                    };

                    let text = String::from_utf8_lossy(&message);
                    for line in text.lines() {
                        if !line.is_empty() {
                            sender.send(make_log_line(stream_name, line.to_string()));
                            got_any = true;
                        }
                    }
                }
                Err(e) => {
                    if follow {
                        // Follow mode failed — will retry without follow.
                        tracing::debug!(
                            target: "cronduit.docker",
                            container_id = %container_id,
                            error = %e,
                            "log follow failed, retrying without follow"
                        );
                    } else {
                        sender.send(make_log_line(
                            "system",
                            format!("[docker log error: {e}]"),
                        ));
                    }
                    break;
                }
            }
        }

        if got_any || !follow {
            // Either we got logs, or we already tried the non-follow fallback.
            break;
        }
    }
}
