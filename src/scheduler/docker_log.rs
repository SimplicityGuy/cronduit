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
    let options = LogsOptions {
        follow: true,
        stdout: true,
        stderr: true,
        timestamps: false,
        ..Default::default()
    };

    let mut stream = docker.logs(&container_id, Some(options));

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
                    }
                }
            }
            Err(e) => {
                sender.send(make_log_line(
                    "system",
                    format!("[docker log error: {e}]"),
                ));
                break;
            }
        }
    }
}
