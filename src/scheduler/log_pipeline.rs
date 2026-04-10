//! Head-drop bounded log channel with line truncation.
//!
//! Each spawned job run gets its own channel. When the channel is full,
//! the oldest lines are dropped (head-drop policy per D-10) to preserve
//! the most recent output for failure diagnosis.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

/// Maximum bytes per log line before truncation (D-11 / EXEC-05).
pub const MAX_LINE_BYTES: usize = 16384;

/// Default channel capacity in lines (D-09).
pub const DEFAULT_CHANNEL_CAPACITY: usize = 256;

/// Default batch size for drain operations (D-12).
pub const DEFAULT_BATCH_SIZE: usize = 64;

/// A single captured log line from a job's stdout or stderr.
#[derive(Debug, Clone)]
pub struct LogLine {
    /// Stream identifier: "stdout", "stderr", or "system".
    pub stream: String,
    /// RFC 3339 timestamp when the line was captured.
    pub ts: String,
    /// Line content (already truncated if it exceeded `MAX_LINE_BYTES`).
    pub line: String,
}

/// Internal shared state between sender and receiver.
struct SharedState {
    buf: VecDeque<LogLine>,
    capacity: usize,
    dropped_count: usize,
    closed: bool,
}

/// Sending half of the head-drop log channel.
///
/// Cloneable so both stdout and stderr reader tasks can share one channel.
#[derive(Clone)]
pub struct LogSender {
    state: Arc<Mutex<SharedState>>,
    notify: Arc<Notify>,
}

impl LogSender {
    /// Send a log line into the channel. If the channel is at capacity,
    /// the oldest line is dropped (head-drop policy).
    ///
    /// If the receiver has been dropped, this is a no-op.
    pub fn send(&self, line: LogLine) {
        let mut state = self.state.lock().unwrap();
        if state.closed {
            return;
        }
        if state.buf.len() >= state.capacity {
            state.buf.pop_front(); // HEAD-DROP: remove oldest (D-10)
            state.dropped_count += 1;
        }
        state.buf.push_back(line);
        drop(state); // release lock before notify
        self.notify.notify_one();
    }

    /// Return the total number of lines dropped since channel creation.
    pub fn dropped_count(&self) -> usize {
        self.state.lock().unwrap().dropped_count
    }

    /// Mark the channel as closed (no more lines will be sent).
    pub fn close(&self) {
        let mut state = self.state.lock().unwrap();
        state.closed = true;
        drop(state);
        self.notify.notify_one();
    }
}

/// Receiving half of the head-drop log channel.
pub struct LogReceiver {
    state: Arc<Mutex<SharedState>>,
    notify: Arc<Notify>,
}

impl LogReceiver {
    /// Drain up to `max` lines from the buffer. If lines were dropped
    /// since the last drain, a `[truncated N lines]` marker is prepended
    /// as the first line of the batch.
    pub fn drain_batch(&self, max: usize) -> Vec<LogLine> {
        let mut state = self.state.lock().unwrap();
        let mut batch = Vec::with_capacity(max.min(state.buf.len() + 1));

        // If lines were dropped, insert truncation marker FIRST
        if state.dropped_count > 0 {
            batch.push(LogLine {
                stream: "system".to_string(),
                ts: chrono::Utc::now().to_rfc3339(),
                line: format!("[truncated {} lines]", state.dropped_count),
            });
            state.dropped_count = 0;
        }

        let drain_count = max.saturating_sub(batch.len()).min(state.buf.len());
        for _ in 0..drain_count {
            if let Some(line) = state.buf.pop_front() {
                batch.push(line);
            }
        }
        batch
    }

    /// Wait for new lines to be available, then drain up to `max`.
    /// Returns an empty vec only when the channel is closed and fully drained.
    pub async fn drain_batch_async(&self, max: usize) -> Vec<LogLine> {
        loop {
            let batch = self.drain_batch(max);
            if !batch.is_empty() {
                return batch;
            }
            let is_closed = self.state.lock().unwrap().closed;
            if is_closed {
                return vec![];
            }
            self.notify.notified().await;
        }
    }

    /// Returns true when the channel is closed and all lines have been drained.
    pub fn is_empty_and_closed(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.closed && state.buf.is_empty() && state.dropped_count == 0
    }
}

impl Drop for LogReceiver {
    fn drop(&mut self) {
        self.state.lock().unwrap().closed = true;
    }
}

/// Create a new head-drop bounded log channel with the given capacity.
///
/// Returns a `(LogSender, LogReceiver)` pair. The sender is cloneable.
pub fn channel(capacity: usize) -> (LogSender, LogReceiver) {
    let state = Arc::new(Mutex::new(SharedState {
        buf: VecDeque::with_capacity(capacity),
        capacity,
        dropped_count: 0,
        closed: false,
    }));
    let notify = Arc::new(Notify::new());
    (
        LogSender {
            state: state.clone(),
            notify: notify.clone(),
        },
        LogReceiver { state, notify },
    )
}

/// Truncate a line to `MAX_LINE_BYTES` if it exceeds that length.
/// Appends a `[line truncated at 16384 bytes]` marker if truncated.
pub fn truncate_line(line: String) -> String {
    if line.len() <= MAX_LINE_BYTES {
        line
    } else {
        let mut truncated = line[..MAX_LINE_BYTES].to_string();
        truncated.push_str(" [line truncated at 16384 bytes]");
        truncated
    }
}

/// Create a `LogLine` with the current UTC timestamp and automatic truncation.
pub fn make_log_line(stream: &str, content: String) -> LogLine {
    LogLine {
        stream: stream.to_string(),
        ts: chrono::Utc::now().to_rfc3339(),
        line: truncate_line(content),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_no_drop() {
        let (tx, rx) = channel(4);
        for i in 0..4 {
            tx.send(make_log_line("stdout", format!("line{i}")));
        }
        let batch = rx.drain_batch(10);
        assert_eq!(batch.len(), 4);
        assert_eq!(batch[0].line, "line0");
        assert_eq!(batch[3].line, "line3");
        assert_eq!(tx.dropped_count(), 0);
    }

    #[test]
    fn channel_head_drop() {
        let (tx, rx) = channel(4);
        for i in 0..6 {
            tx.send(make_log_line("stdout", format!("line{i}")));
        }
        assert_eq!(tx.dropped_count(), 2);
        let batch = rx.drain_batch(10);
        // First line should be the truncation marker
        assert_eq!(batch[0].stream, "system");
        assert!(batch[0].line.contains("[truncated 2 lines]"));
        // Then the 4 surviving lines (line2..line5)
        assert_eq!(batch[1].line, "line2");
        assert_eq!(batch[4].line, "line5");
        assert_eq!(batch.len(), 5); // 1 marker + 4 data lines
    }

    #[test]
    fn truncation_marker_on_drain() {
        let (tx, rx) = channel(4);
        for i in 0..6 {
            tx.send(make_log_line("stdout", format!("line{i}")));
        }
        let batch = rx.drain_batch(10);
        assert_eq!(batch[0].stream, "system");
        assert!(batch[0].line.contains("[truncated 2 lines]"));
    }

    #[test]
    fn line_truncation_exact_boundary() {
        let line = "x".repeat(MAX_LINE_BYTES);
        let result = truncate_line(line.clone());
        assert_eq!(result.len(), MAX_LINE_BYTES);
        assert_eq!(result, line);
    }

    #[test]
    fn line_truncation_over_boundary() {
        let line = "x".repeat(MAX_LINE_BYTES + 1);
        let result = truncate_line(line);
        assert!(result.ends_with(" [line truncated at 16384 bytes]"));
        // The first MAX_LINE_BYTES chars should be 'x'
        assert!(result.starts_with(&"x".repeat(MAX_LINE_BYTES)));
    }

    #[test]
    fn log_line_stream_tag_and_timestamp() {
        let line = make_log_line("stderr", "error happened".to_string());
        assert_eq!(line.stream, "stderr");
        // Timestamp should be valid RFC3339
        chrono::DateTime::parse_from_rfc3339(&line.ts).expect("valid RFC3339 timestamp");
        assert_eq!(line.line, "error happened");
    }

    #[test]
    fn drain_batch_respects_max() {
        let (tx, rx) = channel(256);
        for i in 0..100 {
            tx.send(make_log_line("stdout", format!("line{i}")));
        }
        let batch = rx.drain_batch(64);
        assert_eq!(batch.len(), 64);
    }

    #[test]
    fn send_after_receiver_dropped_no_panic() {
        let (tx, rx) = channel(4);
        drop(rx);
        // This should not panic
        tx.send(make_log_line("stdout", "orphan line".to_string()));
    }

    #[tokio::test]
    async fn drain_batch_async_returns_on_close() {
        let (tx, rx) = channel(4);
        tx.send(make_log_line("stdout", "hello".to_string()));
        tx.close();
        let batch = rx.drain_batch_async(10).await;
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].line, "hello");
        // After close + drain, next async drain returns empty
        let batch2 = rx.drain_batch_async(10).await;
        assert!(batch2.is_empty());
    }
}
