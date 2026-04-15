//! SSE log streaming handler for in-progress runs (UI-14).
//!
//! GET /events/runs/:id/logs returns a Server-Sent Events stream that
//! delivers log lines in real time from the broadcast channel associated
//! with an active run. Completed or unknown runs receive an immediate
//! `run_complete` event and the stream closes.
//!
//! D-01: Slow subscribers get a `[skipped N lines]` marker.
//! D-02: Run completion sends `run_complete` event then closes.
//! D-03: Uses tokio::sync::broadcast per active run.
//! T-6-02: Broadcast capacity 256; slow subscribers get Lagged, not backpressure.
//! T-6-03: All log content HTML-escaped before insertion into SSE data.

use std::convert::Infallible;

use async_stream::stream;
use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::Stream;
use tokio::sync::broadcast::error::RecvError;

use crate::scheduler::log_pipeline::LogLine;
use crate::web::AppState;

/// SSE endpoint for streaming log lines of an in-progress run.
///
/// If the run is active, subscribes to its broadcast channel and yields
/// `log_line` events. If the run is already completed (not in active_runs),
/// yields an immediate `run_complete` event and closes.
pub async fn sse_logs(
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let maybe_rx = {
        let active = state.active_runs.read().await;
        active
            .get(&run_id)
            .map(|entry| entry.broadcast_tx.subscribe())
    };

    let s = stream! {
        match maybe_rx {
            Some(mut rx) => {
                loop {
                    match rx.recv().await {
                        Ok(line) => {
                            let html = format_log_line_html(&line);
                            yield Ok(Event::default().event("log_line").data(html));
                        }
                        Err(RecvError::Lagged(n)) => {
                            let marker = format!(
                                "<div style=\"font-size:var(--cd-text-xs);color:var(--cd-status-disabled);text-align:center;font-style:italic\">[skipped {} lines -- reload page for full log]</div>",
                                n
                            );
                            yield Ok(Event::default().event("log_line").data(marker));
                        }
                        Err(RecvError::Closed) => {
                            yield Ok(Event::default().event("run_complete").data("done"));
                            break;
                        }
                    }
                }
            }
            None => {
                // Run already completed or never existed -- close immediately.
                yield Ok(Event::default().event("run_complete").data("done"));
            }
        }
    };

    Sse::new(s).keep_alive(KeepAlive::default())
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// HTML-escape a string for safe insertion into SSE data (T-6-03).
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Format a LogLine as an HTML div matching the existing log_viewer.html line format.
///
/// The output mirrors the structure in `templates/partials/log_viewer.html`:
/// stream label span + escaped content, with stderr border styling.
fn format_log_line_html(line: &LogLine) -> String {
    let stderr_class = if line.stream == "stderr" {
        " border-l-4 border-[var(--cd-status-error)] bg-[var(--cd-status-error-bg)]"
    } else {
        ""
    };
    let escaped = html_escape(&line.line);
    format!(
        "<div class=\"{stderr_class} py-[2px] px-[var(--cd-space-2)]\" style=\"font-size:var(--cd-text-base);line-height:1.6\">\
         <span class=\"text-[var(--cd-text-secondary)] select-none\" style=\"font-size:var(--cd-text-xs)\">{stream}</span> \
         {content}\
         </div>",
        stderr_class = stderr_class.trim(),
        stream = html_escape(&line.stream),
        content = escaped,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escape_special_chars() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape("\"hello\""), "&quot;hello&quot;");
        assert_eq!(html_escape("plain text"), "plain text");
    }

    #[test]
    fn format_log_line_html_stdout() {
        let line = LogLine {
            stream: "stdout".to_string(),
            ts: "2026-01-01T00:00:00Z".to_string(),
            line: "hello world".to_string(),
        };
        let html = format_log_line_html(&line);
        assert!(html.contains("hello world"));
        assert!(html.contains("stdout"));
        assert!(!html.contains("border-l-4"));
    }

    #[test]
    fn format_log_line_html_stderr() {
        let line = LogLine {
            stream: "stderr".to_string(),
            ts: "2026-01-01T00:00:00Z".to_string(),
            line: "error <msg>".to_string(),
        };
        let html = format_log_line_html(&line);
        assert!(html.contains("border-l-4"));
        assert!(html.contains("&lt;msg&gt;"));
    }
}
