//! SSE log streaming integration tests (UI-14).
//!
//! Tests validate:
//! - SSE endpoint returns event stream for active runs
//! - Completed/unknown run_id returns immediate run_complete
//! - Slow subscriber receives skipped-lines marker
//! - Stream closes when run completes

#[cfg(test)]
mod sse_tests {
    // TODO: Import test helpers, AppState builder, axum test utilities

    #[tokio::test]
    #[ignore = "not yet implemented"]
    async fn sse_active_run_streams_log_lines() {
        // Setup: create AppState with active_runs containing a broadcast sender
        // Act: send GET /events/runs/:id/logs via axum test client
        // Assert: response is SSE stream, first event is log_line type
        todo!("Implement SSE active run streaming test")
    }

    #[tokio::test]
    #[ignore = "not yet implemented"]
    async fn sse_completed_run_returns_immediate_close() {
        // Setup: create AppState with empty active_runs
        // Act: send GET /events/runs/999/logs
        // Assert: response contains run_complete event and stream closes
        todo!("Implement SSE completed run test")
    }

    #[tokio::test]
    #[ignore = "not yet implemented"]
    async fn sse_slow_subscriber_gets_skip_marker() {
        // Setup: create broadcast channel with capacity 4
        // Send 10 messages rapidly, then subscribe
        // Assert: subscriber receives Lagged and skip marker appears in stream
        todo!("Implement SSE slow subscriber test")
    }

    #[tokio::test]
    #[ignore = "not yet implemented"]
    async fn sse_stream_closes_on_run_finalize() {
        // Setup: create active broadcast channel, subscribe
        // Act: drop the broadcast sender (simulating run finalization)
        // Assert: subscriber receives run_complete event, stream ends
        todo!("Implement SSE stream close test")
    }
}
