//! Phase 11 D-10: terminal `run_finished` SSE event.
//!
//! Asserts that `sse_logs` at `src/web/handlers/sse.rs` translates the
//! graceful terminal sentinel — a `LogLine { stream: "__run_finished__",
//! line: "{run_id}", id: None, ts: .. }` broadcast by
//! `scheduler::run::continue_run` immediately before `drop(broadcast_tx)` —
//! into `event: run_finished\ndata: {"run_id": N}\n\n` and breaks the
//! subscribe loop. Covers VALIDATION rows 11-10-01 (`payload_shape`) and
//! 11-10-02 (T-V11-LOG-07 `fires_before_broadcast_drop`).
//!
//! Test shape mirrors `tests/v11_sse_log_stream.rs`:
//! 1. Build an axum test Router with `sse_logs` + an `AppState` whose
//!    `active_runs` has an entry with a broadcast sender we also retain a
//!    local clone of.
//! 2. Spawn `oneshot(GET /events/runs/{id}/logs)` + `to_bytes` on a task.
//! 3. Sleep 50ms so the handler subscribes.
//! 4. Publish log_lines + the sentinel through the broadcast tx.
//! 5. Drop the active_runs entry + the local sender. On the `break` path,
//!    the handler has already terminated (the sentinel emitted run_finished
//!    and the loop broke) — `to_bytes` completes. On the fallback path
//!    (no sentinel sent), the handler's `RecvError::Closed` arm emits
//!    `run_complete` and exits. Both land at `to_bytes` completion.
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

/// Broadcast capacity for the per-run log stream in tests. Mirrors the
/// production default at a smaller scale.
const BROADCAST_CAPACITY: usize = 64;

/// Build a minimal test Router wired for the SSE log endpoint plus a
/// pre-registered `active_runs` entry for `run_id`. Returns the router,
/// the broadcast sender, and a handle to the `active_runs` map (so tests
/// can remove the entry to trigger the Closed fallback path if needed).
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
    // clone of the sender while keeping a local clone for the test thread.
    let (broadcast_tx, _) = broadcast::channel::<LogLine>(BROADCAST_CAPACITY);

    let cancel = CancellationToken::new();
    let control = RunControl::new(cancel);

    active_runs.write().await.insert(
        run_id,
        RunEntry {
            broadcast_tx: broadcast_tx.clone(),
            control,
            job_name: "v11-sse-terminal-event-test".to_string(),
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

/// Construct a normal log_line. The id-emission contract is exercised by
/// `tests/v11_sse_log_stream.rs`; this harness only needs lines to interleave
/// before the sentinel for the ordering assertion in
/// `fires_before_broadcast_drop`.
fn make_log_line(id: i64, content: &str) -> LogLine {
    LogLine {
        stream: "stdout".to_string(),
        ts: "2026-04-17T00:00:00Z".to_string(),
        line: content.to_string(),
        id: Some(id),
    }
}

/// Construct the terminal sentinel LogLine that `scheduler::run::continue_run`
/// broadcasts immediately before `drop(broadcast_tx)`.
fn make_run_finished_sentinel(run_id: i64) -> LogLine {
    LogLine {
        stream: "__run_finished__".to_string(),
        ts: "2026-04-17T00:00:00Z".to_string(),
        line: run_id.to_string(),
        id: None,
    }
}

/// Drive the SSE handler to completion: spawn `oneshot(GET ...)`, let the
/// handler subscribe, invoke `feed`, then drop sender clones. Returns the
/// body as a `String` (SSE wire format is UTF-8 text).
///
/// The sentinel-emitting path ends via `break` after `yield` — the handler
/// doesn't need the senders to drop to terminate. The fallback (no sentinel)
/// path relies on `RecvError::Closed`, which requires every sender clone to
/// drop. We always drop both in this helper so either path lands cleanly.
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
    let req = Request::builder()
        .method("GET")
        .uri(format!("/events/runs/{}/logs", run_id))
        .body(Body::empty())
        .expect("build request");

    let request_handle = tokio::spawn(async move {
        let response = router.oneshot(req).await.expect("oneshot");
        let bytes = axum::body::to_bytes(response.into_body(), 10 * 1024 * 1024)
            .await
            .expect("to_bytes");
        String::from_utf8(bytes.to_vec()).expect("utf-8 body")
    });

    // Let the handler reach its `subscribe()` call. 50ms is defensive — the
    // handler typically reaches subscribe within ~1ms; same floor used in
    // `tests/v11_sse_log_stream.rs`.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Publish what the caller wants.
    feed(&broadcast_tx);

    // Drop every sender clone so the fallback `RecvError::Closed` path can
    // fire if the sentinel wasn't published. On the sentinel path, the
    // handler has already broken the loop by the time we get here, so these
    // operations are harmless.
    active_runs.write().await.remove(&run_id);
    drop(broadcast_tx);

    request_handle.await.expect("request task")
}

/// VALIDATION row 11-10-01 `payload_shape`: publish a single sentinel
/// LogLine and assert the SSE wire emits
/// `event: run_finished\ndata: {"run_id": N}`.
#[tokio::test]
async fn payload_shape() {
    const RUN_ID: i64 = 7;
    let (router, broadcast_tx, active_runs) = build_test_app_with_active_run(RUN_ID).await;

    let body = drive_sse_stream(router, broadcast_tx, active_runs, RUN_ID, |tx| {
        tx.send(make_run_finished_sentinel(RUN_ID))
            .expect("send sentinel");
    })
    .await;

    assert!(
        body.contains("event: run_finished\n"),
        "SSE body must contain `event: run_finished\\n` when sentinel fires; body was:\n{}",
        body
    );
    assert!(
        body.contains(&format!(r#"data: {{"run_id": {}}}"#, RUN_ID)),
        "SSE body must contain the JSON payload `data: {{\"run_id\": {}}}`; body was:\n{}",
        RUN_ID,
        body
    );
    // The handler `break`s after emitting run_finished, so `run_complete`
    // (the RecvError::Closed arm's event) must NOT appear on the wire.
    assert!(
        !body.contains("event: run_complete\n"),
        "SSE body must NOT contain `event: run_complete` when the sentinel already fired \
         (the handler breaks after run_finished; run_complete is only the abrupt-disconnect \
         fallback); body was:\n{}",
        body
    );
}

/// T-V11-LOG-07 / VALIDATION row 11-10-02 `fires_before_broadcast_drop`:
/// publish log_lines, then the sentinel, then let the senders drop. Assert
/// the wire order is (1) log_line frames → (2) run_finished → (3) NO
/// run_complete. This locks the ordering contract from
/// `scheduler::run::continue_run`: sentinel is broadcast AFTER the log
/// writer task has flushed every persisted line (RESEARCH.md §P10) and
/// BEFORE `drop(broadcast_tx)`, so subscribers see log_lines first and the
/// terminal frame as the final event.
#[tokio::test]
async fn fires_before_broadcast_drop() {
    const RUN_ID: i64 = 13;
    let (router, broadcast_tx, active_runs) = build_test_app_with_active_run(RUN_ID).await;

    let body = drive_sse_stream(router, broadcast_tx, active_runs, RUN_ID, |tx| {
        // Two normal log_line frames first — mimics flushed log batches.
        tx.send(make_log_line(100, "first-log-line"))
            .expect("send log line 100");
        tx.send(make_log_line(101, "second-log-line"))
            .expect("send log line 101");
        // Then the sentinel — mimics finalize_run's pre-drop broadcast.
        tx.send(make_run_finished_sentinel(RUN_ID))
            .expect("send sentinel");
    })
    .await;

    // Locate each frame's start offset so we can assert strict ordering on
    // the wire (log_line frames → run_finished → end).
    let pos_log_100 = body
        .find("first-log-line")
        .unwrap_or_else(|| panic!("expected `first-log-line` in body; got:\n{}", body));
    let pos_log_101 = body
        .find("second-log-line")
        .unwrap_or_else(|| panic!("expected `second-log-line` in body; got:\n{}", body));
    let pos_run_finished = body
        .find("event: run_finished\n")
        .unwrap_or_else(|| panic!("expected `event: run_finished\\n` in body; got:\n{}", body));

    assert!(
        pos_log_100 < pos_log_101,
        "log_line frames must appear in broadcast order; got pos_log_100={} >= pos_log_101={}",
        pos_log_100,
        pos_log_101
    );
    assert!(
        pos_log_101 < pos_run_finished,
        "run_finished must come AFTER every log_line; got pos_log_101={} >= pos_run_finished={}",
        pos_log_101,
        pos_run_finished
    );
    // The handler breaks after run_finished → no run_complete on the wire.
    assert!(
        !body.contains("event: run_complete\n"),
        "run_complete must NOT appear when the sentinel already fired \
         (RecvError::Closed arm stays the abrupt-disconnect fallback); \
         body was:\n{}",
        body
    );

    // Assert both log_line frames landed with their `id: N` lines (id
    // emission is owned by Plan 11-08 but we sanity-check the joint contract).
    assert!(
        body.contains("id: 100\n"),
        "first log_line must carry `id: 100\\n`; body was:\n{}",
        body
    );
    assert!(
        body.contains("id: 101\n"),
        "second log_line must carry `id: 101\\n`; body was:\n{}",
        body
    );

    // Sanity: exactly one run_finished frame (no duplicates / leaks).
    assert_eq!(
        body.matches("event: run_finished\n").count(),
        1,
        "exactly one run_finished frame expected on the wire; body was:\n{}",
        body
    );
}
