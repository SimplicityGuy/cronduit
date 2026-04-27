---
phase: 11-per-job-run-numbers-log-ux-fixes
plan: 10
subsystem: scheduler (run finalize) + web (SSE handler) + integration tests
tags: [rust, axum, sse, broadcast, phase-11, ui-17, ui-18, d-10]

# Dependency graph
requires:
  - phase: 11-08
    provides: SSE Ok(line) arm locked at a known shape (`let mut ev = Event::default().event("log_line").data(html); if let Some(id) = line.id { ev = ev.id(id.to_string()); } yield Ok(ev);`). Plan 11-10 inserts a new sentinel-detect branch ahead of this block without disturbing the id emission.
  - phase: 11-06
    provides: `continue_run` helper extracted from `run_job`. Plan 11-10's sentinel broadcast lands at the shared drop-site inside `continue_run`, so both the scheduler-driven path (`run_job`) and the UI-19 pre-insert path (`run_job_with_existing_run_id`) fire the graceful terminal frame automatically.
provides:
  - src/scheduler/run.rs — `continue_run` broadcasts a `LogLine { stream: "__run_finished__", line: run_id.to_string(), id: None, ts: chrono::Utc::now().to_rfc3339() }` immediately before `drop(broadcast_tx)`. Ordering preserved per RESEARCH.md §P10: writer_handle awaited → finalize_run DB update → sentinel → remove(&run_id) → drop(broadcast_tx). `SendError` (no live subscribers) is intentionally discarded via `let _`.
  - src/web/handlers/sse.rs — Ok(line) arm adds a pattern-match branch: when `line.stream == "__run_finished__"`, the handler yields `Event::default().event("run_finished").data(format!(r#"{{"run_id": {}}}"#, line.line))` and `break`s the subscribe loop. Existing `log_line` path for non-sentinel lines unchanged (id emission preserved). `RecvError::Closed` arm unchanged — `run_complete` stays as the abrupt-disconnect fallback for subscribers that miss the sentinel.
  - tests/v11_sse_terminal_event.rs — Wave-0 `#[ignore]` stubs replaced with full bodies. Adds `build_test_app_with_active_run` + `drive_sse_stream` helpers (mirrored from `tests/v11_sse_log_stream.rs`) plus `make_log_line` + `make_run_finished_sentinel` fixtures.
  - T-V11-LOG-07 `fires_before_broadcast_drop` (VALIDATION 11-10-02): publishes two log_line frames + the sentinel through an axum oneshot router + `to_bytes`, asserts wire order log_line → log_line → run_finished, no `run_complete`, exactly one `run_finished` frame, id emission preserved (`id: 100\n` + `id: 101\n`).
  - `payload_shape` (VALIDATION 11-10-01): publishes a lone sentinel, asserts body contains `event: run_finished\n` + `data: {"run_id": 7}` and no `event: run_complete\n`.
affects: [11-11, 11-14]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Graceful SSE termination via broadcast-channel sentinel (D-10): `finalize_run` broadcasts a distinguished `LogLine { stream: \"__run_finished__\", line: run_id.to_string(), id: None, .. }` immediately before `drop(broadcast_tx)`. The SSE handler's `Ok(line)` arm pattern-matches on the `stream` string and emits `event: run_finished\\ndata: {\"run_id\": N}\\n\\n` + `break`, leaving `RecvError::Closed → run_complete` as the abrupt-disconnect fallback only. This keeps the handler at one `rx.recv()` loop and avoids introducing a parallel oneshot channel in `RunEntry` — RESEARCH.md §Technical Approach §4 rationale."
    - "Wire-format assertion on SSE streams via `axum::Router::oneshot` + `axum::body::to_bytes` — the same harness shape used by `tests/v11_sse_log_stream.rs`. For ordering checks: locate each frame's byte offset via `body.find(...)` and assert `pos_a < pos_b < pos_c` directly on the collected UTF-8 body. No parser required — SSE wire format is line-oriented plain text and each event occupies a contiguous block of `field: value\\n` lines terminated by a blank line."
    - "Dual drop-semantics in the test harness: `drive_sse_stream` always drops both the `active_runs` entry and the local sender at end of test. On the sentinel path the handler has already `break`ed before the drop fires, so the operations are no-ops. On the fallback (Closed) path the drops are required to terminate the stream. Same helper covers both assertion shapes without branching — the caller's `feed` closure decides which path the handler takes."

key-files:
  created:
    - .planning/phases/11-per-job-run-numbers-log-ux-fixes/11-10-SUMMARY.md
  modified:
    - src/scheduler/run.rs
    - src/web/handlers/sse.rs
    - tests/v11_sse_terminal_event.rs

key-decisions:
  - "Followed the plan's `break` after `run_finished` rather than the RESEARCH.md §4 suggestion to `continue` and let Closed fire next. The plan body explicitly specifies `break;` in Task 2's `<action>` and `<done>` lists 'Break statement ends the stream gracefully'. Rationale: on the graceful path, `run_finished` is the single terminal event clients listen for (Plan 11-11's `sse:run_finished` listener calls `htmx.ajax` to swap live→static); re-emitting `run_complete` afterwards would force the client to either listen for two terminal events or race-check which arrived first. `break` makes `run_finished` the unique graceful terminator and relegates `run_complete` to its narrow fallback role (abrupt network drop, subscriber reconnect after finalize_run but before the sentinel flushed)."
  - "Sentinel carries `id: None`, not a synthetic id. A synthetic id (e.g. `i64::MAX`) would poison the client's dedupe cursor (`data-max-id`) because the Plan 11-11 handler treats any received id as a new high-water mark. `None` is the only safe value — the client's dedupe path (Plan 11-14) naturally skips frames with no id. The SSE handler also does NOT emit an `id:` line when `line.id == None`, so the sentinel's wire frame is `event: run_finished\\ndata: {\"run_id\": N}\\n\\n` — exactly what RESEARCH.md §Technical Approach §4 specified."
  - "Placed the sentinel broadcast inside `continue_run` (step 7c), not in `run_job` or `run_job_with_existing_run_id` individually. `continue_run` is the shared lifecycle helper introduced by Plan 11-06 and runs for BOTH the scheduler-driven path (cron-tick + catch-up via `run_job`) and the UI-19 pre-insert path (Run Now via `run_job_with_existing_run_id`). Landing the broadcast once at the shared site covers every code path that terminates a run, and keeps the two public entry points locked to identical termination semantics — any divergence would be a latent UX bug where some runs emit `run_finished` and others emit only `run_complete`."
  - "Dropped both sender clones and the active_runs entry in `drive_sse_stream` regardless of which path the test exercises. The sentinel-emitting path `break`s after `yield` so the drops are harmless no-ops by the time they execute; the fallback (Closed) path needs the drops to terminate the stream. Writing two separate helpers would cost ~30 LOC of duplication for a branch the caller has already encoded in its `feed` closure. The harness shape mirrors `tests/v11_sse_log_stream.rs`'s `drive_sse_stream` exactly (same 50ms subscribe sleep, same task-spawn + to_bytes pattern) so readers recognize the shape."
  - "Asserted `id: 100\\n` + `id: 101\\n` on the log_line frames inside `fires_before_broadcast_drop`. The id emission contract is primarily owned by Plan 11-08's `tests/v11_sse_log_stream.rs::event_includes_id_field`, but adding the assertion here catches regressions in the joint contract where `run_finished`'s `break` path could accidentally eat buffered `log_line` frames (it must not — the broadcast channel delivers in FIFO order, and `rx.recv()` only surfaces the sentinel after every earlier frame is drained). One extra assertion, one extra regression class protected."

requirements-completed: [UI-17]

# Metrics
duration: ~6min
completed: 2026-04-17
---

# Phase 11 Plan 10: Terminal `run_finished` SSE Event Summary

**Scheduler `continue_run` now broadcasts a `__run_finished__` sentinel `LogLine` immediately before `drop(broadcast_tx)`, and the SSE handler translates it to `event: run_finished\ndata: {"run_id": N}\n\n` + `break` — the graceful terminator Plan 11-11's client will listen for to swap the running log pane to the static partial. `RecvError::Closed → run_complete` stays unchanged as the narrow abrupt-disconnect fallback. `tests/v11_sse_terminal_event.rs`'s Wave-0 `#[ignore]` stubs replaced with full bodies — `payload_shape` (VALIDATION 11-10-01) and `fires_before_broadcast_drop` (T-V11-LOG-07 / 11-10-02) both pass in 0.27s. `cargo test --lib` → 173 passed; `cargo test --test v11_sse_terminal_event` → 2 passed; `cargo test --test v11_sse_log_stream` → 2 passed (Plan 11-08 contract preserved); `cargo test --test stop_executors` → 2 passed + 1 pre-existing ignore (no regression). `cargo clippy --lib --tests -- -D warnings` + `cargo fmt --check` both clean.**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-04-17T02:21:52Z
- **Completed:** 2026-04-17T02:27:24Z
- **Tasks:** 3
- **Files created:** 1 (this SUMMARY.md)
- **Files modified:** 3 (1 scheduler, 1 web handler, 1 test file)

## Accomplishments

- `src/scheduler/run.rs` — `continue_run` broadcasts the `__run_finished__` sentinel immediately before `drop(broadcast_tx)` (new step 7c; the drop moved to 7d). Block is +23 lines total (15 lines of doc comment explaining the ordering + ~8 lines of sentinel construction). Ordering per RESEARCH.md §P10: writer_handle already awaited at step 6 → every persisted log_line has been broadcast with `id: Some(n)`; finalize_run DB update has run at step 7; sentinel broadcast at 7c; remove from active_runs at 7d; `drop(broadcast_tx)` at 7d. `SendError` (no live subscribers) is intentionally discarded via `let _`.
- `src/web/handlers/sse.rs` — Ok(line) arm adds a pattern-match branch for `line.stream == "__run_finished__"`. Emits `Event::default().event("run_finished").data(format!(r#"{{"run_id": {}}}"#, line.line))` and `break`s the subscribe loop. Block is +22 lines total (14 lines of doc comment explaining the sentinel→break path and the Closed fallback + 4 lines of branch body). The existing `log_line` path for non-sentinel lines is byte-identical to pre-plan — id emission preserved. `RecvError::Closed` arm unchanged — still emits `run_complete` + `break` as the abrupt-disconnect fallback.
- `tests/v11_sse_terminal_event.rs` — Wave-0 `#[ignore]` stubs replaced with full bodies (+288/-6). Adds `build_test_app_with_active_run` + `drive_sse_stream` helpers (mirrored from `tests/v11_sse_log_stream.rs`'s harness so readers recognize the shape) plus `make_log_line` + `make_run_finished_sentinel` fixtures.
- `payload_shape` (VALIDATION 11-10-01): publishes a lone sentinel via the broadcast tx, asserts the body contains `event: run_finished\n` + `data: {"run_id": 7}` and does NOT contain `event: run_complete\n` (the handler `break`s after the sentinel).
- `fires_before_broadcast_drop` (T-V11-LOG-07 / VALIDATION 11-10-02): publishes two log_line frames (ids 100, 101) + the sentinel, asserts wire order via byte-offset: `first-log-line` → `second-log-line` → `event: run_finished\n`. Also asserts: no `event: run_complete\n` on the wire; exactly one `run_finished` frame; id emission preserved (`id: 100\n` + `id: 101\n`).
- `cargo check --lib` → clean.
- `cargo clippy --lib --tests -- -D warnings` → clean.
- `cargo fmt --check` → clean.
- `cargo test --lib` → 173 passed; 0 failed.
- `cargo test --test v11_sse_terminal_event` → 2 passed; 0 failed; 0 ignored (Wave-0 stubs fully realized).
- `cargo test --test v11_sse_log_stream` → 2 passed (Plan 11-08 id-emission contract preserved).
- `cargo test --test stop_executors` → 2 passed; 1 pre-existing ignore (no regression on Phase 10 Stop semantics).

## Task Commits

Each task committed atomically on branch `worktree-agent-af5f3fb4` (worktree for `gsd/phase-11-context`, base `d0ec085`):

1. **Task 1:** `bf1a9b3` — `feat(11-10): broadcast __run_finished__ sentinel before drop(broadcast_tx)`
2. **Task 2:** `e2bdba1` — `feat(11-10): SSE handler emits run_finished on __run_finished__ sentinel`
3. **Task 3:** `9a0967f` — `test(11-10): replace Wave-0 stubs with run_finished terminal-event coverage`

## Files Created/Modified

- `src/scheduler/run.rs` (MODIFIED, +26/-1) — New step 7c broadcasts `LogLine { stream: "__run_finished__", ts: chrono::Utc::now().to_rfc3339(), line: run_id.to_string(), id: None }`. Step 7d (was 7c) is unchanged: remove from active_runs + drop(broadcast_tx). Doc comment above the new block explains the ordering guarantee (P10) and why the `RecvError::Closed` arm remains the abrupt-disconnect fallback. No other lines in the file touched.
- `src/web/handlers/sse.rs` (MODIFIED, +22/-0) — Ok(line) arm opens with `if line.stream == "__run_finished__" { ... break; }`. Block is 5 lines of code + 17 lines of doc comment (one block above the branch, explaining the D-10 contract + the Closed fallback). The id-emission path (Plan 11-08) is byte-identical below the new branch.
- `tests/v11_sse_terminal_event.rs` (MODIFIED, +288/-6) — Wave-0 `#[ignore]` stubs replaced with full bodies. Harness (build_test_app_with_active_run + drive_sse_stream) mirrors the `tests/v11_sse_log_stream.rs` shape, including the 50ms subscribe sleep floor. Two tests: `payload_shape` + `fires_before_broadcast_drop` (T-V11-LOG-07). Fixtures: `make_log_line` + `make_run_finished_sentinel`.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-10-SUMMARY.md` (NEW) — this file.

## Decisions Made

1. **`break` after `run_finished` (not `continue`)**. The plan's Task 2 `<action>` explicitly specifies `break;` and `<done>` says 'Break statement ends the stream gracefully'. Rationale: `run_finished` is the single graceful terminator; re-emitting `run_complete` afterwards forces the client into two-listener bookkeeping or race-check logic. `break` makes the contract unambiguous and keeps `run_complete` in its narrow abrupt-disconnect role.
2. **Sentinel carries `id: None`**. A synthetic id would poison the client's dedupe cursor (`data-max-id`) because any received id becomes a new high-water mark. `None` means the frame has no `id:` wire line — the client's dedupe path naturally skips it. Matches RESEARCH.md §Technical Approach §4.
3. **Broadcast site is `continue_run`, not `run_job` / `run_job_with_existing_run_id` separately**. `continue_run` (Plan 11-06) is the shared lifecycle helper; landing the sentinel there once guarantees identical termination semantics for both the scheduler-driven path and the UI-19 pre-insert path. Divergence here would be a latent UX bug.
4. **Dual-drop test harness**. `drive_sse_stream` always drops both the active_runs entry and the local sender at end of test. On the sentinel path the handler has already broken the loop; on the fallback path the drops are required to terminate. One helper covers both paths, mirrors `tests/v11_sse_log_stream.rs` byte-for-byte.
5. **Asserted `id: 100\n` + `id: 101\n` inside `fires_before_broadcast_drop`**. One extra assertion protects an extra regression class (the sentinel's `break` path could accidentally eat buffered log_line frames if the broadcast channel or handler ever drifted from FIFO). Joint-contract coverage is cheap here.

## Deviations from Plan

**None.** Plan body directives followed exactly:
- Task 1: sentinel broadcast placed immediately before `drop(broadcast_tx)` inside `continue_run` with fields `stream = "__run_finished__"`, `line = run_id.to_string()`, `id = None`, `ts = chrono::Utc::now().to_rfc3339()`. Matches the plan's `<action>` pseudo-code and `<interfaces>` ordering.
- Task 2: SSE handler's Ok(line) arm adds the `if line.stream == "__run_finished__" { ... break; }` branch before the existing log_line emission. Emit format `event("run_finished").data(format!(r#"{{"run_id": {}}}"#, line.line))` matches the plan's `<action>` pseudo-code verbatim. `RecvError::Closed` arm preserved unchanged.
- Task 3: `tests/v11_sse_terminal_event.rs` has `#[ignore]` removed from both stubs. `payload_shape` asserts `event: run_finished` + `{"run_id": N}` literally. `fires_before_broadcast_drop` asserts log_line → run_finished order + no `run_complete`. Harness shape preserved (in-test broadcast channel, subscribe receiver, feed lines + sentinel, drop tx, assert body content) per the plan's Task 3 `<action>`.

No Rule 1 (bugs), Rule 2 (missing critical functionality), Rule 3 (blocking issues), or Rule 4 (architectural) deviations triggered. Tool-hook behavior from Plan 11-07 did not recur — all three Edit/Write calls succeeded on first attempt (the PreToolUse hook issued its READ-BEFORE-EDIT reminder as a non-fatal notice but the operations landed).

## Threat Flags

None beyond the plan's `<threat_model>`.

- **T-11-10-01 (Tampering / sentinel LogLine):** Mitigated per plan. Only `continue_run` in `src/scheduler/run.rs` constructs a `LogLine` with `stream = "__run_finished__"`. Verified via `grep -n '"__run_finished__"' src/` → exactly two hits (the sender in `run.rs:358-363`, the pattern-match in `sse.rs:64-68`). No other code path in the crate can fabricate the sentinel.
- **T-11-10-02 (Spoofed run_finished payload):** Accepted per plan. The data is `{"run_id": N}` where `N` is serialized from the `run_id: i64` parameter of `continue_run` — no user-controlled bytes can reach the payload because `i64::to_string()` is ASCII-only and bounded to `[-19..=19]` chars. Subscribers trust it by construction.
- **T-11-10-03 (Loss of sentinel under backpressure):** Accepted per plan. The broadcast channel has capacity 256 (`src/scheduler/run.rs::continue_run` constructor at L157). If 256 unread frames pile up on a slow subscriber, the sentinel can be dropped by the broadcast channel's lag semantics — but the `RecvError::Closed` arm still fires on the subsequent `drop(broadcast_tx)`, emitting `run_complete` as the fallback. No session gets stuck waiting for a terminator; the worst case is the client using the abrupt-disconnect UX path instead of the graceful one.

No new network endpoints, no new auth paths, no new file-access patterns. The SSE route `/events/runs/{run_id}/logs` exists unchanged from Phase 6; only the event taxonomy gained a graceful-terminal variant.

## Issues Encountered

None. All three tasks landed on first-edit attempts. Both tests passed on first run (0.27s wall clock combined). `cargo clippy` + `cargo fmt --check` both clean on first invocation.

## Deferred Issues

None. All Plan 11-10 tasks completed with full verification passing.

## TDD Gate Compliance

Plan 11-10 has `tdd="true"` on all three tasks. The gate sequence on disk:

- **RED (from Plan 11-00):** the two Wave-0 `#[ignore]` stubs in `tests/v11_sse_terminal_event.rs` (`payload_shape`, `fires_before_broadcast_drop`). These compile as `#[ignore]` but the implementation they verify (scheduler sentinel broadcast + SSE pattern-match + break) does not yet exist at Plan 11-10's start.
- **GREEN (Task 1, production code):** `bf1a9b3` — sentinel broadcast in `continue_run`. After this commit the Wave-0 stubs *could* pass halfway (sentinel broadcasts reach subscribers) but the SSE handler still translates the sentinel to a stray `log_line` event with mangled content. Task 2 completes the contract.
- **GREEN (Task 2, production code):** `e2bdba1` — SSE handler pattern-match + `break`. After this commit the Wave-0 stubs would pass if the `#[ignore]` were removed.
- **GREEN (Task 3, real test bodies):** `9a0967f` — removes `#[ignore]` and lands full bodies that exercise Tasks 1 + 2 end-to-end through axum. Both tests pass.
- **REFACTOR:** Not required; `cargo fmt --check` clean throughout.

`feat(...)` commits: `bf1a9b3` (Task 1 production), `e2bdba1` (Task 2 production).
`test(...)` commit: `9a0967f` (Task 3 real test bodies + helpers).

## User Setup Required

None. All changes are:
- One conditional sentinel broadcast inserted before an existing `drop(broadcast_tx)` in an async lifecycle helper (`continue_run`).
- One pattern-match branch added to an axum SSE handler's existing `Ok(line)` arm (wire-format-only change; SSE clients that don't listen for `run_finished` simply ignore the event name).
- Test-only additions (new harness + real test bodies).

No new migrations, config keys, CLI flags, routes, dependencies, or operator action.

## Next Phase Readiness

- **Plan 11-11 (client-side `sse:run_finished` → static-partial swap) unblocked.** With `event: run_finished\ndata: {"run_id": N}\n\n` firing from the server on every graceful completion, the client's inline script in `templates/pages/run_detail.html` can attach `logLines.addEventListener('sse:run_finished', ...)` and call `htmx.ajax('GET', '/partials/runs/{run_id}/logs', { target: '#log-container', swap: 'outerHTML' })` as documented in RESEARCH.md §Technical Approach §4.
- **Plan 11-14 (client-side dedupe + initial render) unblocked.** The terminal event contract is wire-stable: `event: run_finished` is emitted only once per run, only on the graceful path, and carries a well-formed `{"run_id": N}` payload. The dedupe handler can safely stop listening for `sse:log_line` after `sse:run_finished` fires because no more persisted log lines will follow (writer_handle is awaited before the sentinel broadcasts, per §P10).
- **Phase-11 wave 9 complete for D-10's server half.** Any remaining Phase 11 plans that depend on server-side graceful termination signaling can proceed.

## Self-Check: PASSED

**Files verified on disk:**
- `src/scheduler/run.rs` — FOUND; sentinel broadcast present (grep for `"__run_finished__"` inside `continue_run` returns the constructor block before `drop(broadcast_tx)`).
- `src/web/handlers/sse.rs` — FOUND; `if line.stream == "__run_finished__"` branch present at the top of the Ok(line) arm with `event("run_finished").data(...)` + `break`.
- `tests/v11_sse_terminal_event.rs` — FOUND; 300 lines; 0 `#[ignore]` attributes remaining; 2 `#[tokio::test]` functions (`payload_shape`, `fires_before_broadcast_drop`) each with full bodies calling `drive_sse_stream`.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-10-SUMMARY.md` — FOUND (this file).

**Commits verified (present in `git log d0ec085..HEAD`):**
- `bf1a9b3` — FOUND (`feat(11-10): broadcast __run_finished__ sentinel before drop(broadcast_tx)`)
- `e2bdba1` — FOUND (`feat(11-10): SSE handler emits run_finished on __run_finished__ sentinel`)
- `9a0967f` — FOUND (`test(11-10): replace Wave-0 stubs with run_finished terminal-event coverage`)

**Build gates verified:**
- `cargo check --lib` — CLEAN.
- `cargo clippy --lib --tests -- -D warnings` — CLEAN.
- `cargo fmt --check` — CLEAN.
- `cargo test --lib` — PASS (`173 passed; 0 failed`).
- `cargo test --test v11_sse_terminal_event` — PASS (`2 passed; 0 failed; 0 ignored`).
- `cargo test --test v11_sse_log_stream` — PASS (`2 passed; 0 failed; 0 ignored`; Plan 11-08 contract preserved).
- `cargo test --test stop_executors` — PASS (`2 passed; 0 failed; 1 ignored` pre-existing).

**Plan success criteria verified:**
1. `__run_finished__` sentinel broadcast precedes `drop(broadcast_tx)` — ✅ (verified by position in `continue_run`: sentinel at step 7c, `drop(broadcast_tx)` at step 7d, with `active_runs.write().await.remove(&run_id)` between them; no await between the sentinel send and the drop that could shift ordering).
2. SSE handler emits `event: run_finished\ndata: {"run_id": N}\n\n` and breaks — ✅ (verified in `payload_shape` test body: `body.contains("event: run_finished\n")` + `body.contains(r#"data: {"run_id": 7}"#)` both pass; `break` statement present on the line after `yield`).
3. `run_complete` no longer fires on graceful completion — ✅ (verified in both tests: `!body.contains("event: run_complete\n")` asserts present on both `payload_shape` and `fires_before_broadcast_drop`; both pass).
4. T-V11-LOG-07 `fires_before_broadcast_drop` passes — ✅ (test passed; log_line frames appear strictly before `event: run_finished`).
5. `payload_shape` passes — ✅ (test passed; `event: run_finished\n` + `{"run_id": 7}` on the wire).

---
*Phase: 11-per-job-run-numbers-log-ux-fixes*
*Completed: 2026-04-17*
