//! Phase 11 SSE id-line emission (UI-18, UI-20). Covers T-V11-LOG-05/06.
//!
//! Asserts that the `sse_logs` handler (at `src/web/handlers/sse.rs`) emits
//! the persisted `job_logs.id` as the SSE frame's `id:` field whenever the
//! broadcast `LogLine` carries `id: Some(n)` (populated by `log_writer_task`
//! via Plan 11-07's RETURNING-id zip). This is the server half of the D-09
//! dedupe contract — the browser stores the value and sends it back on
//! `Last-Event-ID` reconnect; the HTMX SSE extension surfaces it as
//! `event.lastEventId` for Plan 11-11's client-side dedupe against
//! `data-max-id`.
//!
//! Test shape:
//! 1. Build an axum test Router holding the `sse_logs` handler + an
//!    `AppState` whose `active_runs` map has an entry with a broadcast
//!    sender we also keep a local clone of.
//! 2. Spawn the `oneshot(GET /events/runs/{id}/logs)` + `to_bytes` future.
//! 3. Yield so the handler subscribes to our broadcast channel.
//! 4. Send one or more `LogLine` values through the broadcast tx.
//! 5. Remove the run from `active_runs` and drop our local sender — zero
//!    sender refcount causes the next `rx.recv()` to yield
//!    `RecvError::Closed`, which the handler translates to a `run_complete`
//!    event before exiting the loop. `to_bytes` then completes.
//! 6. Inspect the body bytes for the expected SSE wire-format strings.

#![allow(clippy::assertions_on_constants)]

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use axum::routing::get;
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt; // .oneshot()

use cronduit::db::DbPool;
use cronduit::scheduler::RunEntry;
use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::scheduler::control::RunControl;
use cronduit::scheduler::log_pipeline::LogLine;
use cronduit::telemetry::setup_metrics;
use cronduit::web::handlers;
use cronduit::web::{AppState, ReloadState};

/// Broadcast channel capacity for the per-run log stream in tests. Mirrors
/// the production default (256) at a smaller scale so tests stay fast while
/// still exercising the subscribe-then-send shape.
const BROADCAST_CAPACITY: usize = 64;

/// Build a minimal test Router wired for the SSE log endpoint plus a
/// pre-registered `active_runs` entry for `run_id`. Returns the router,
/// the broadcast sender (so the caller can push `LogLine`s through), and a
/// handle to the `active_runs` map (so the caller can remove the entry to
/// trigger `RecvError::Closed` on the handler's subscriber).
async fn build_test_app_with_active_run(
    run_id: i64,
) -> (
    Router,
    broadcast::Sender<LogLine>,
    Arc<RwLock<HashMap<i64, RunEntry>>>,
) {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");

    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);

    let metrics_handle = setup_metrics();

    let active_runs: Arc<RwLock<HashMap<i64, RunEntry>>> = Arc::new(RwLock::new(HashMap::new()));

    // Create the broadcast channel first so we can seed active_runs with a
    // clone of the sender and keep our own local clone for sending test
    // messages.
    let (broadcast_tx, _) = broadcast::channel::<LogLine>(BROADCAST_CAPACITY);

    let cancel = CancellationToken::new();
    let control = RunControl::new(cancel);

    active_runs.write().await.insert(
        run_id,
        RunEntry {
            broadcast_tx: broadcast_tx.clone(),
            control,
            job_name: "v11-sse-log-stream-test".to_string(),
        },
    );

    let state = AppState {
        started_at: chrono::Utc::now(),
        version: "test",
        pool,
        cmd_tx,
        config_path: std::path::PathBuf::from("/tmp/cronduit-test.toml"),
        tz: chrono_tz::UTC,
        last_reload: Arc::new(Mutex::new(None::<ReloadState>)),
        watch_config: false,
        metrics_handle,
        active_runs: active_runs.clone(),
    };

    let router = Router::new()
        .route("/events/runs/{run_id}/logs", get(handlers::sse::sse_logs))
        .with_state(state);

    (router, broadcast_tx, active_runs)
}

/// Build a single synthetic LogLine with the given id. The stream/ts/line
/// fields are irrelevant to the id-emission assertion — we just need the
/// SSE frame to flow through `format_log_line_html`.
fn make_line(id: i64) -> LogLine {
    LogLine {
        stream: "stdout".to_string(),
        ts: "2026-04-17T00:00:00Z".to_string(),
        line: format!("test-line-{}", id),
        id: Some(id),
    }
}

/// Drive the SSE handler to completion: spawns `oneshot(GET /events/...)`,
/// gives the handler time to subscribe, invokes the caller's `feed` closure
/// to publish log lines, then drops every sender clone so the stream
/// terminates via `RecvError::Closed` and `to_bytes` completes. Returns the
/// collected body bytes as a `String` (SSE wire format is UTF-8 text).
async fn drive_sse_stream<F>(
    router: Router,
    broadcast_tx: broadcast::Sender<LogLine>,
    active_runs: Arc<RwLock<HashMap<i64, RunEntry>>>,
    run_id: i64,
    feed: F,
) -> String
where
    F: FnOnce(&broadcast::Sender<LogLine>),
{
    // Spawn the GET on a task so we can interleave broadcast sends after
    // the handler has subscribed. `oneshot` consumes the router.
    let req = Request::builder()
        .method("GET")
        .uri(format!("/events/runs/{}/logs", run_id))
        .body(Body::empty())
        .expect("build request");

    let request_handle = tokio::spawn(async move {
        let response = router.oneshot(req).await.expect("oneshot");
        // SSE responses are text/event-stream; collect the full body.
        // `to_bytes` drives the stream until the handler's `stream!` block
        // completes (which happens when all broadcast senders drop and the
        // handler yields `run_complete`).
        let bytes = axum::body::to_bytes(response.into_body(), 10 * 1024 * 1024)
            .await
            .expect("to_bytes");
        String::from_utf8(bytes.to_vec()).expect("utf-8 body")
    });

    // Give the spawned task a chance to run — the handler must perform
    // `active_runs.read().await.get(&run_id).map(|e| e.broadcast_tx.subscribe())`
    // before we send, otherwise broadcast drops messages that have no
    // subscribers. A short sleep is more robust than yield_now because
    // oneshot() has to progress through the tower service stack + the
    // handler's async fn prologue before reaching subscribe().
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Publish the log lines the caller wants.
    feed(&broadcast_tx);

    // Drop every sender clone so the handler's `rx.recv()` yields
    // `RecvError::Closed` → `run_complete` → break → stream ends.
    active_runs.write().await.remove(&run_id);
    drop(broadcast_tx);

    request_handle.await.expect("request task")
}

#[tokio::test]
async fn event_includes_id_field() {
    // T-V11-LOG-05: when the broadcast delivers a LogLine with
    // id = Some(42), the SSE frame must include `id: 42` on its own line,
    // alongside the existing `event: log_line` + `data:` lines.
    const RUN_ID: i64 = 1;
    let (router, broadcast_tx, active_runs) = build_test_app_with_active_run(RUN_ID).await;

    let body = drive_sse_stream(router, broadcast_tx, active_runs, RUN_ID, |tx| {
        tx.send(make_line(42)).expect("send log line");
    })
    .await;

    // SSE wire format: each frame is a sequence of `field: value\n` lines
    // terminated by a blank line. axum's Sse serializer emits the id field
    // on its own line when `Event::id()` is set.
    assert!(
        body.contains("id: 42\n"),
        "SSE body must contain `id: 42\\n` line for LogLine with id = Some(42); body was:\n{}",
        body
    );
    assert!(
        body.contains("event: log_line\n"),
        "SSE body must preserve the existing `event: log_line` field; body was:\n{}",
        body
    );
    assert!(
        body.contains("data:"),
        "SSE body must include a `data:` field carrying the HTML; body was:\n{}",
        body
    );
    assert!(
        body.contains("test-line-42"),
        "SSE body's data payload must contain the LogLine content (escaped); body was:\n{}",
        body
    );
    // After the single log_line frame, the Closed arm yields run_complete
    // before exiting. Assert it — keeps the lifecycle explicit.
    assert!(
        body.contains("event: run_complete\n"),
        "SSE body must end with `event: run_complete` once all senders drop; body was:\n{}",
        body
    );
}

#[tokio::test]
async fn ids_monotonic_per_run() {
    // T-V11-LOG-06: five LogLines with ids 10..=14 produce five `id: N`
    // lines in the body, in strictly monotonic insertion order. This locks
    // the contract that the broadcast channel preserves insertion order
    // (FIFO per subscriber) and that the handler emits the id of each
    // frame rather than a single one at end-of-stream.
    const RUN_ID: i64 = 2;
    const IDS: [i64; 5] = [10, 11, 12, 13, 14];

    let (router, broadcast_tx, active_runs) = build_test_app_with_active_run(RUN_ID).await;

    let body = drive_sse_stream(router, broadcast_tx, active_runs, RUN_ID, |tx| {
        for id in IDS {
            tx.send(make_line(id)).expect("send log line");
        }
    })
    .await;

    // Collect the positions of each `id: N\n` occurrence and assert they
    // appear in strictly ascending order matching the send order. Using
    // `find` + slice offsets from the previous match guarantees each next
    // id is later in the body, which proves monotonicity on the wire.
    let mut cursor = 0usize;
    let mut observed: Vec<i64> = Vec::with_capacity(IDS.len());
    for id in IDS {
        let needle = format!("id: {}\n", id);
        let found = body[cursor..].find(&needle).unwrap_or_else(|| {
            panic!(
                "expected `id: {}\\n` at or after byte {} of SSE body; body was:\n{}",
                id, cursor, body
            )
        });
        let abs = cursor + found;
        observed.push(id);
        cursor = abs + needle.len();
    }

    assert_eq!(
        observed,
        IDS.to_vec(),
        "ids must appear on the wire in exact send order (FIFO broadcast + handler preserves order)"
    );

    // Sanity check: every id line is paired with an `event: log_line` —
    // the handler does not drop either field when emitting the combined
    // frame. We simply count occurrences and require at least IDS.len()
    // `event: log_line` lines (the skipped-lines marker arm is never hit
    // because the channel capacity is 64 >> 5).
    let log_line_count = body.matches("event: log_line\n").count();
    assert!(
        log_line_count >= IDS.len(),
        "expected at least {} `event: log_line` frames, found {}; body was:\n{}",
        IDS.len(),
        log_line_count,
        body
    );
}
