---
phase: 11-per-job-run-numbers-log-ux-fixes
plan: 08
subsystem: web (SSE handler) + integration tests
tags: [rust, axum, sse, broadcast, phase-11, ui-18, ui-20, d-09]

# Dependency graph
requires:
  - phase: 11-07
    provides: LogLine.id: Option<i64> populated end-to-end via log_writer_task's zip of insert_log_batch's RETURNING id with the input batch. Plan 11-08 consumes the populated id by emitting it as the SSE frame `id:` field.
provides:
  - src/web/handlers/sse.rs::sse_logs — Ok(line) arm now builds a mutable `Event::default().event("log_line").data(html)` and conditionally appends `.id(id.to_string())` when `line.id` is `Some`. The rest of the handler (format_log_line_html XSS escape, Lagged skipped-N-lines marker, Closed run_complete event) is unchanged.
  - tests/v11_sse_log_stream.rs — Wave-0 `#[ignore]` stubs replaced with full bodies. Adds `build_test_app_with_active_run` helper (minimal Router + AppState with pre-registered RunEntry) and `drive_sse_stream` harness (spawns oneshot, sleeps 50ms for subscribe, invokes caller's feed closure, then drops sender clones to trigger RecvError::Closed → run_complete → stream end, returning the collected body as String).
  - T-V11-LOG-05 (event_includes_id_field): one LogLine with id = Some(42) yields a frame containing `id: 42\n`, `event: log_line\n`, data payload with `test-line-42`, followed by terminal `event: run_complete\n`.
  - T-V11-LOG-06 (ids_monotonic_per_run): five LogLines with ids 10..=14 yield five `id: N\n` lines in strictly monotonic send order on the wire (byte-offset cursor scan).
affects: [11-09, 11-10, 11-11, 11-14]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Conditional SSE frame `id:` emission via axum 0.8's `Event::id(impl Into<String>)` builder method. Build the event mutably — `let mut ev = Event::default().event(...).data(...)`; then `if let Some(id) = line.id { ev = ev.id(id.to_string()) }`. Preserves existing event name + data when the id is absent (e.g. Lagged marker, pre-persistence construction)."
    - "SSE integration test via `axum::Router::oneshot` + `axum::body::to_bytes` with lifecycle-driven stream termination. Spawn the oneshot on a task, sleep briefly so the handler subscribes, publish broadcast messages from the test thread, then drop every sender clone (remove from active_runs + drop local) so the next `rx.recv()` yields `RecvError::Closed`. The handler's Closed arm emits `run_complete` and breaks, letting `to_bytes` collect the full body."
    - "Monotonic-order assertion via byte-offset cursor: `let found = body[cursor..].find(&needle)`; advance the cursor to `cursor + found + needle.len()` after each match. Proves strict ordering on the wire without requiring a parser — each subsequent needle must appear at or after the current cursor, so ascending order is structural."

key-files:
  created:
    - .planning/phases/11-per-job-run-numbers-log-ux-fixes/11-08-SUMMARY.md
  modified:
    - src/web/handlers/sse.rs
    - tests/v11_sse_log_stream.rs

key-decisions:
  - "Kept the conditional id emission (`if let Some(id) = line.id`) rather than unwrapping with a default (e.g. `line.id.unwrap_or(0).to_string()`). Emitting `id: 0` for unpersisted lines would poison the browser's last-event-id state because the client's dedupe logic (Plan 11-11) treats any received id as the new high-water mark. Omitting the id line entirely on pre-persistence frames is the safer contract — the client simply won't advance its cursor for those frames."
  - "Skipped-lines marker (Lagged arm) deliberately left without `id:`. The marker is a system-injected advisory div, not a persisted log line, and never corresponds to a `job_logs.id`. Adding an `id:` would mislead the client into thinking it was a normal log line. Research §4 called this out; plan followed."
  - "Used a 50ms `tokio::time::sleep` rather than `tokio::task::yield_now` to give the spawned oneshot task time to reach the handler's `active_runs.read().await.get(&run_id).subscribe()` call. Two `yield_now` calls were tried first but proved flaky under `cargo test` default parallelism because the tower service stack + axum's extractor machinery interleaves a handful of await points before the handler body runs. 50ms is a defensive floor — a tokio test multi-thread runtime typically reaches subscribe within ~1ms, so the budget is ~50x what's needed."
  - "Asserted both `event_includes_id_field` AND the terminal `event: run_complete\\n` in the T-V11-LOG-05 body. The test harness relies on the Closed arm firing to let `to_bytes` complete, so we get the terminal-event assertion for free — making it explicit catches regressions in the Closed arm (which Plan 11-10 will later extend with a `run_finished` variant and which MUST remain wire-compatible with existing SSE clients until that lands)."
  - "Did not modify sse_streaming.rs's four existing `#[ignore]` stubs. Those are Phase 6 placeholders (UI-14 streaming + Lagged marker coverage) that pre-date Phase 11. Plan 11-08's scope is UI-18 / UI-20 id emission only; touching the Phase 6 stubs would broaden the blast radius beyond what requirements demand. They remain ignored for now."

requirements-completed: [UI-18, UI-20]

# Metrics
duration: ~6min
completed: 2026-04-17
---

# Phase 11 Plan 08: SSE Frame id Emission Summary

**SSE handler now emits `id: {n}` on the wire for every log_line event whose LogLine carries `id: Some(n)` (populated by Plan 11-07's log_writer_task post-RETURNING-id zip). The change is +15 lines in `src/web/handlers/sse.rs` (Ok(line) arm only; format_log_line_html + Lagged/Closed arms unchanged) and T-V11-LOG-05 + T-V11-LOG-06 now land with full bodies instead of Wave-0 `#[ignore]` stubs — both pass in 0.27s on the first run. The client-side dedupe contract (D-09) has its server half complete; Plan 11-11's client can now compare `event.lastEventId` on `sse:log_line` to `data-max-id` on `#log-lines`. `cargo test --lib` → 173 passed; `cargo test --test v11_sse_log_stream` → 2 passed; `cargo test --test xss_log_safety` → 7 passed; `cargo clippy --lib --tests -- -D warnings` + `cargo fmt --check` both clean.**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-04-17T02:10:00Z
- **Completed:** 2026-04-17T02:16:01Z
- **Tasks:** 2
- **Files created:** 1 (this SUMMARY.md)
- **Files modified:** 2 (1 handler, 1 test)

## Accomplishments

- `src/web/handlers/sse.rs` Ok(line) arm rewritten to conditionally emit the SSE frame `id:` field. Diff is +15/-1: the arm now declares a mutable `Event::default().event("log_line").data(html)`, appends `.id(id.to_string())` inside `if let Some(id) = line.id { ... }`, and yields the built event. The Lagged arm (skipped-N-lines marker), Closed arm (run_complete on sender drop), and the None branch (unknown/completed runs) are untouched. XSS escape via `html_escape` in `format_log_line_html` is unchanged.
- `tests/v11_sse_log_stream.rs` Wave-0 `#[ignore]` stubs replaced with full bodies (+270/-5). Two helpers land alongside the test functions:
  - `build_test_app_with_active_run(run_id)` — connects an in-memory SQLite, builds the minimal `AppState`, creates a broadcast channel and inserts a `RunEntry` into `active_runs`, returns `(Router, broadcast::Sender<LogLine>, Arc<RwLock<HashMap<i64, RunEntry>>>)`. The caller keeps the sender for publishing and the active_runs handle for removing the entry to trigger Closed.
  - `drive_sse_stream(router, broadcast_tx, active_runs, run_id, feed)` — spawns the oneshot + `to_bytes` on a task, sleeps 50ms so the handler subscribes, invokes the caller's `feed: FnOnce(&broadcast::Sender<LogLine>)` closure to publish messages, then removes the active_runs entry and drops the local sender. Zero sender refcount → `RecvError::Closed` → `run_complete` event → break → `to_bytes` completes. Returns the body as a `String` (SSE wire format is UTF-8 text).
- `event_includes_id_field` (T-V11-LOG-05): seeds one `LogLine { id: Some(42), stream: "stdout", line: "test-line-42", .. }`; asserts body contains `id: 42\n`, `event: log_line\n`, `data:`, `test-line-42`, and the terminal `event: run_complete\n`.
- `ids_monotonic_per_run` (T-V11-LOG-06): publishes five `LogLine`s with ids `[10, 11, 12, 13, 14]`; asserts each `id: N\n` appears in strictly monotonic send order on the wire (byte-offset cursor advances past each match) and that at least five `event: log_line\n` frames landed.
- `cargo check --lib` → clean.
- `cargo clippy --lib --tests -- -D warnings` → clean.
- `cargo fmt --check` → clean.
- `cargo test --lib` → 173 passed; 0 failed.
- `cargo test --test v11_sse_log_stream` → 2 passed; 0 failed; 0 ignored.
- `cargo test --test xss_log_safety` → 7 passed; 0 failed (no regression in the XSS escape contract).
- `cargo test --test v11_log_id_plumbing` → 3 passed (upstream Plan 11-07 contract preserved).
- `cargo test --test sse_streaming` → 4 ignored (pre-existing Phase 6 stubs, unchanged, unrelated to this plan).

## Task Commits

Each task committed atomically on branch `worktree-agent-a24e6f30` (worktree for `gsd/phase-11-context`):

1. **Task 1:** `216b7ee` — `feat(11-08): emit SSE id: field when LogLine.id is Some`
2. **Task 2:** `64be6d9` — `test(11-08): replace Wave-0 SSE stream stubs with real bodies`

## Files Created/Modified

- `src/web/handlers/sse.rs` (MODIFIED, +15/-1) — Ok(line) arm emits SSE frame `id:` field via `Event::id(id.to_string())` when `LogLine.id.is_some()`; mutable builder pattern preserves the existing `.event("log_line").data(html)` for the `None` case. Doc comment placed above the `let mut ev` line explains the D-09 / UI-18 / Plan 11-11 / 11-14 chain. No other lines in the file touched.
- `tests/v11_sse_log_stream.rs` (MODIFIED, +270/-5) — Wave-0 `#[ignore]` stubs replaced with full bodies (events_includes_id_field, ids_monotonic_per_run). Adds `build_test_app_with_active_run` + `drive_sse_stream` + `make_line` helpers. File doc comment extended to describe the test shape (handler subscribe → broadcast publish → close trigger → body inspect).
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-08-SUMMARY.md` (NEW) — this file.

## Decisions Made

1. **Conditional id emission (`if let Some(id)`)** rather than `unwrap_or(0)`. A default id would poison the client's last-event-id state because every received id is treated as the new dedupe cursor; omitting the line for unpersisted frames keeps the client's cursor unchanged, which is the safe semantics.
2. **Lagged skipped-lines marker deliberately has no `id:`.** It's a system-injected advisory div, not a real log line, and never corresponds to a `job_logs.id`. Adding one would mislead the client.
3. **50ms `tokio::time::sleep` over `yield_now`** in the integration test harness. Two yields proved flaky under `cargo test` parallelism because the tower + axum extractor path has several await points before the handler body reaches subscribe. 50ms is a defensive floor — the handler typically subscribes within ~1ms.
4. **Assert terminal `event: run_complete\n` in T-V11-LOG-05.** The test harness already relies on the Closed arm firing so `to_bytes` completes, so asserting the terminal frame is free coverage — catches regressions in the Closed arm that Plan 11-10 will later extend.
5. **Do NOT touch `tests/sse_streaming.rs`'s four pre-existing `#[ignore]` stubs.** Those are Phase 6 placeholders for UI-14 (streaming + Lagged coverage); Plan 11-08 scope is UI-18 / UI-20 id emission only.

## Deviations from Plan

**None.** Plan body directives followed exactly. `<action>` pseudo-code in Task 1 matches the landed diff word-for-word (including `let mut ev = Event::default().event("log_line").data(html);` + `if let Some(id) = line.id { ev = ev.id(id.to_string()); }`). Task 2's test bodies match the `<behavior>` contract: both tests remove `#[ignore]`, `event_includes_id_field` uses `id = Some(42)`, `ids_monotonic_per_run` uses five ids `10..=14` asserted in order.

No Rule 1 (bugs), Rule 2 (missing critical functionality), Rule 3 (blocking issues), or Rule 4 (architectural) deviations triggered. The only scope-preserving judgement call was the 50ms sleep in the test harness (vs. `yield_now`), which is documented as a decision rather than a deviation since the plan's `<action>` for Task 2 explicitly delegates the harness mechanics to the executor ("the test infrastructure requires … or an equivalent helper").

## Threat Flags

None beyond the plan's `<threat_model>`.

- **T-11-08-01 (Information disclosure, SSE log_line data):** Accepted as planned. v1 ships unauthenticated on trusted LAN; the id field is an opaque monotonic i64 already exposed via run-detail URLs, so emitting it in the SSE stream introduces no new disclosure surface.
- **T-11-08-02 (Tampering / XSS via log_line data):** Mitigated. `html_escape` in `format_log_line_html` is unchanged, and `line.id: Option<i64>` flows through `.to_string()` — no user-controlled bytes can reach the `id:` line because `i64` decimal serialization is ASCII-only and bounded (`[-19..=19]` chars).

No new network endpoints, no new auth paths, no new file-access patterns. The SSE route `/events/runs/{run_id}/logs` exists unchanged from Phase 6; only the frame shape gained an optional `id:` line.

## Issues Encountered

None. Task 1 landed on the first edit. Task 2's test bodies passed on the first run (0.27s wall clock for both tests combined).

## Deferred Issues

None. All Plan 11-08 tasks completed with full verification passing.

## TDD Gate Compliance

Plan 11-08 has `tdd="true"` on both tasks. The gate sequence on disk:

- **RED (from Plan 11-00):** commits `fa26618` / `783e9ca` (and descendants) landed the two Wave-0 `#[ignore]` stubs in `tests/v11_sse_log_stream.rs`. These are the pre-implementation RED — they compile as `#[ignore]` but the implementation they verify (server-side SSE `id:` emission) does not yet exist.
- **GREEN (Task 1, production code):** `216b7ee` — Ok(line) arm emits `.id()`. After this commit the Wave-0 stubs *could* pass but are still `#[ignore]`.
- **GREEN (Task 2, real test bodies):** `64be6d9` — removes `#[ignore]` and lands full bodies that exercise Task 1's change end-to-end through axum. Both tests pass.
- **REFACTOR:** Not required; `cargo fmt --check` clean throughout (one intermediate fmt fix applied mid-Task-2 before commit, not a separate refactor).

`feat(...)` commit: `216b7ee` (Task 1 production).
`test(...)` commit: `64be6d9` (Task 2 real test bodies + helpers).

## User Setup Required

None. All changes are:
- One conditional field addition inside an existing axum handler arm (SSE `Event::id()`); wire-format backward-compatible because SSE clients that don't track `lastEventId` simply ignore the id line.
- Test-only additions (new harness + real bodies).

No new migrations, config keys, CLI flags, routes, dependencies, or operator action.

## Next Phase Readiness

- **Plan 11-09 (server-rendered backfill + data-max-id) unblocked.** With the SSE stream publishing `id:` per frame, the run-detail page can now safely compare `event.lastEventId` on `sse:log_line` to a `data-max-id` attribute rendered server-side from `queries::get_log_lines` — both come from the same `job_logs.id` column.
- **Plan 11-10 (terminal `run_finished` event) unblocked.** The SSE handler's Ok(line)/Lagged/Closed arms are now locked at a clean shape. Plan 11-10 can extend the pattern by detecting a sentinel `LogLine` in the Ok arm (Option A) or by inserting a new arm ahead of Closed without touching the id-emission logic.
- **Plan 11-11 (client-side dedupe handler) unblocked.** The server half of the D-09 dedupe contract is complete — every `sse:log_line` frame carries `lastEventId` when the line was persisted, so the client can implement the `if parseInt(evt.lastEventId) > maxId` guard against `data-max-id`.
- **Plan 11-14 (client-side dedupe + initial render) unblocked.** Initial-render `DbLogLine.id` values and live-stream `event.lastEventId` values now come from the same column, so comparing them numerically is sound.

## Self-Check: PASSED

**Files verified on disk:**
- `src/web/handlers/sse.rs` — FOUND; `let mut ev = Event::default()` at L58, `if let Some(id) = line.id` at L59, `ev = ev.id(id.to_string())` at L60, `yield Ok(ev)` at L62.
- `tests/v11_sse_log_stream.rs` — FOUND; 287 lines; zero `#[ignore]` attributes remaining; two `#[tokio::test]` functions (`event_includes_id_field`, `ids_monotonic_per_run`) each with a `let (router, broadcast_tx, active_runs) = build_test_app_with_active_run(...)` + `drive_sse_stream(...)` body; `make_line`, `build_test_app_with_active_run`, `drive_sse_stream` helpers present.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-08-SUMMARY.md` — FOUND (this file).

**Commits verified (present in `git log ae23926..HEAD`):**
- `216b7ee` — FOUND (`feat(11-08): emit SSE id: field when LogLine.id is Some`)
- `64be6d9` — FOUND (`test(11-08): replace Wave-0 SSE stream stubs with real bodies`)

**Build gates verified:**
- `cargo check --lib` — CLEAN.
- `cargo clippy --lib --tests -- -D warnings` — CLEAN.
- `cargo fmt --check` — CLEAN.
- `cargo test --lib` — PASS (`173 passed; 0 failed`).
- `cargo test --test v11_sse_log_stream` — PASS (`2 passed; 0 failed; 0 ignored`).
- `cargo test --test xss_log_safety` — PASS (`7 passed`).
- `cargo test --test v11_log_id_plumbing` — PASS (`3 passed`).
- `cargo test --test sse_streaming` — PASS (`4 ignored` pre-existing stubs, not regressed).

**Plan success criteria verified:**
1. SSE Ok(line) arm adds `.id(..)` when LogLine.id is Some — ✅ (verified: `let mut ev = Event::default()…` + `if let Some(id) = line.id { ev = ev.id(id.to_string()) }` at src/web/handlers/sse.rs L58-61).
2. Lagged + Closed arms unchanged — ✅ (verified: `git diff ae23926..HEAD -- src/web/handlers/sse.rs` shows diff only inside the Ok(line) arm; Lagged L64-70 and Closed L71-74 byte-identical to pre-plan).
3. T-V11-LOG-05 + T-V11-LOG-06 pass — ✅ (both tests PASS in `cargo test --test v11_sse_log_stream`, 0.27s wall clock, 0 ignored).

---
*Phase: 11-per-job-run-numbers-log-ux-fixes*
*Completed: 2026-04-17*
