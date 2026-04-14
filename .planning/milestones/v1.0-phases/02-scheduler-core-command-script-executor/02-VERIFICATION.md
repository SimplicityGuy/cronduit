---
phase: 02-scheduler-core-command-script-executor
verified: 2026-04-10T21:00:00Z
status: passed
score: 6/6 must-haves verified
overrides_applied: 0
re_verification: false
---

# Phase 02: Scheduler Core & Command/Script Executor — Verification Report

**Phase Goal:** A hand-rolled tokio scheduler loop that fires jobs on their cron schedule in the configured timezone, executes local command and script backends, captures stdout/stderr through a bounded log pipeline into the DB, and drains cleanly on SIGINT/SIGTERM — all without Docker or a UI.
**Verified:** 2026-04-10T21:00:00Z
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (Roadmap Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A command-type job fires via tokio::process::Command; stdout/stderr land in job_logs with correct stream tags | VERIFIED | `src/scheduler/command.rs` uses `shell_words::split` + `tokio::process::Command` with piped stdout/stderr tagged "stdout"/"stderr". Integration test `test_command_job_fires_and_captures_logs` verifies DB records. All 6 integration tests pass. |
| 2 | Script job writes body to tempfile with shebang, executes; exit 0 = success, non-zero = failed, timeout = timeout with partial logs | VERIFIED | `src/scheduler/script.rs` uses `NamedTempFile`, `0o755` permissions, shebang prepended. `run.rs` maps `RunStatus::Success/Failed/Timeout`. Integration tests `test_script_job_fires_and_captures_logs`, `test_failed_command_records_exit_code`, `test_timeout_preserves_partial_logs` all pass. |
| 3 | DST regression tests (spring-forward + fall-back) pass; clock jumps >2min emit WARN and do not drop missed fires | VERIFIED | `src/scheduler/fire.rs` has `check_clock_jump()` with 2-min threshold + 24h cap. Unit tests `dst_spring_forward_skips_nonexistent_time`, `dst_fall_back_fires_once`, `clock_jump_detects_missed_fires`, `clock_jump_no_false_positive` all pass (20/20 lib tests for Plan 01). |
| 4 | SIGINT drains in-flight runs up to shutdown_grace; second SIGTERM force-exits immediately | VERIFIED | `src/shutdown.rs` calls `wait_for_signal()` twice; second triggers `std::process::exit(1)`. `src/scheduler/mod.rs` has full drain state machine with `grace_deadline`, `abort_all()` on expiry, structured summary log with all 4 fields. Shutdown unit tests pass. |
| 5 | Log pipeline uses bounded 256-line channel with head-drop + [truncated N lines] marker; 16KB line truncation with marker | VERIFIED | `src/scheduler/log_pipeline.rs`: `DEFAULT_CHANNEL_CAPACITY = 256`, `MAX_LINE_BYTES = 16384`, `pop_front()` on overflow (head-drop D-10), `"[truncated {} lines]"` marker prepended on drain. 8+ unit tests pass. |
| 6 | Concurrent runs of same job create separate job_runs rows; trigger='scheduled' for cron fires | VERIFIED | `src/scheduler/run.rs` inserts via `insert_running_run()` per-run. `run_job()` called with `"scheduled"` trigger in `mod.rs`. `test_concurrent_runs_same_job` integration test verifies two distinct rows. |

**Score: 6/6 truths verified**

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/scheduler/mod.rs` | Module root, SchedulerLoop struct, spawn(), tokio::select! loop | VERIFIED | 401 lines; contains `SchedulerLoop`, `pub fn spawn()`, `tokio::select!`, `join_set.spawn`, `HashMap`, `BinaryHeap` wiring |
| `src/scheduler/sync.rs` | sync_config_to_db() function with SyncResult | VERIFIED | 388 lines; `pub async fn sync_config_to_db`, `pub struct SyncResult`, calls `compute_config_hash`, `upsert_job`, `disable_missing_jobs` |
| `src/scheduler/fire.rs` | BinaryHeap fire logic, clock-jump detection, DST handling | VERIFIED | 422 lines; `pub struct FireEntry`, `pub fn build_initial_heap`, `pub fn check_clock_jump`, `pub fn fire_due_jobs`, `pub fn requeue_job`, `find_next_occurrence` |
| `src/scheduler/log_pipeline.rs` | HeadDropChannel, LogLine, LogWriter, line truncation | VERIFIED | 285 lines; `pub struct LogSender`, `pub struct LogReceiver`, `pub fn channel`, `MAX_LINE_BYTES = 16384`, `pop_front` head-drop, truncation marker |
| `src/scheduler/command.rs` | execute_command() using shell-words + tokio::process::Command | VERIFIED | 286 lines; `pub async fn execute_command`, `pub enum RunStatus`, `pub struct ExecResult`, `shell_words::split`, `process_group(0)`, `libc::kill` |
| `src/scheduler/script.rs` | execute_script() using tempfile + shebang + chmod | VERIFIED | 214 lines; `pub async fn execute_script`, `NamedTempFile`, `set_permissions`, `0o755` |
| `src/scheduler/run.rs` | run_job() task, log_writer_task, full lifecycle | VERIFIED | 459 lines; `pub async fn run_job`, `log_writer_task`, `log_pipeline::channel`, dispatches to `command::execute_command` and `script::execute_script` |
| `src/db/queries.rs` | DB query helpers: upsert_job, disable_missing_jobs, get_enabled_jobs, insert_running_run, finalize_run, insert_log_batch | VERIFIED | 692 lines; all 6 query functions present, `pub fn writer()`, `pub fn reader()`, `PoolRef` enum |
| `src/shutdown.rs` | Double-signal handler | VERIFIED | 38 lines; `wait_for_signal()` called twice, `std::process::exit(1)` on second signal |
| `tests/scheduler_integration.rs` | 6 end-to-end integration tests | VERIFIED | 376 lines; all 6 test functions present and passing |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/scheduler/sync.rs` | `src/db/queries.rs` | `upsert_job + disable_missing_jobs` | WIRED | Line 113,128 calls `upsert_job`; line 146 calls `disable_missing_jobs` |
| `src/scheduler/fire.rs` | `croner::Cron` | `find_next_occurrence` for next-fire computation | WIRED | `cron.find_next_occurrence()` at lines 65, 132, 278, 286, 300, 309 |
| `src/scheduler/sync.rs` | `src/config/hash.rs` | `compute_config_hash` for change detection | WIRED | `use crate::config::hash::compute_config_hash` at line 10; called at line 93 |
| `src/scheduler/command.rs` | `src/scheduler/log_pipeline.rs` | `LogSender` for capturing stdout/stderr | WIRED | `use super::log_pipeline::{make_log_line, LogSender}` at line 12; sender passed through `execute_child` |
| `src/scheduler/script.rs` | `src/scheduler/log_pipeline.rs` | `LogSender` for capturing stdout/stderr | WIRED | `use super::log_pipeline::LogSender` at line 15; `log_pipeline::channel(256)` in tests |
| `src/scheduler/command.rs` | `shell_words::split` | Command tokenization | WIRED | `shell_words::split(command_str)` at line 168; also in test at line 283 |
| `src/scheduler/run.rs` | `src/scheduler/command.rs` | `execute_command` for command-type jobs | WIRED | `command::execute_command(...)` at line 91 |
| `src/scheduler/run.rs` | `src/scheduler/script.rs` | `execute_script` for script-type jobs | WIRED | `super::script::execute_script(...)` at line 112 |
| `src/scheduler/run.rs` | `src/scheduler/log_pipeline.rs` | `channel() + drain_batch_async` for log capture | WIRED | `log_pipeline::channel(DEFAULT_CHANNEL_CAPACITY)` at line 69; `log_writer_task` drains via `drain_batch_async` |
| `src/scheduler/run.rs` | `src/db/queries.rs` | `insert_running_run + finalize_run + insert_log_batch` | WIRED | All three called in `run_job()` and `log_writer_task()` |
| `src/cli/run.rs` | `src/scheduler/mod.rs` | `scheduler::spawn()` in boot sequence | WIRED | `crate::scheduler::spawn(...)` at line 114 with pool, jobs, tz, cancel, shutdown_grace |
| `src/shutdown.rs` | `src/scheduler/mod.rs` | `CancellationToken` cancellation triggers drain | WIRED | `cancel.clone()` passed to `spawn()`; `cancelled()` arm in scheduler select loop triggers drain state machine |
| `src/scheduler/mod.rs` | `src/db/queries.rs` | `finalize_run` for force-killed runs | WIRED | `finalize_run` imported and called in run task; scheduler calls `run::run_job` which handles finalization |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `src/scheduler/run.rs` | `run_id` from `insert_running_run` | `job_runs` DB INSERT returning id | Yes — real SQLite INSERT | FLOWING |
| `src/scheduler/run.rs` | `exec_result` from `execute_command`/`execute_script` | Real process execution via `tokio::process::Command` | Yes — real process | FLOWING |
| `src/scheduler/run.rs` | `log batch` in `log_writer_task` | `LogReceiver::drain_batch_async()` from process stdout/stderr | Yes — real pipe output | FLOWING |
| `src/scheduler/mod.rs` | `jobs` HashMap | `sync_config_to_db()` returning `SyncResult.jobs` from DB | Yes — real DB SELECT | FLOWING |
| `tests/scheduler_integration.rs` | `job_runs` rows after test | In-memory SQLite with real queries | Yes — verified by SELECT after run | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| 70 unit lib tests pass | `cargo test --lib` | 70 passed; 0 failed (1.44s) | PASS |
| 6 integration tests pass | `cargo test --test scheduler_integration` | 6 passed; 0 failed (1.06s) | PASS |
| Build is clean | `cargo build` | Finished dev profile cleanly | PASS |
| Clippy is clean | `cargo clippy --all-targets -- -D warnings` | No warnings | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| SCHED-01 | 02-01-PLAN | Hand-rolled tokio::select! scheduler loop | SATISFIED | `src/scheduler/mod.rs` has `tokio::select!` over fire/join/cancel arms; no external scheduler crate |
| SCHED-02 | 02-01-PLAN | Fires at every resolved_schedule match in configured timezone, DST correct | SATISFIED | croner `find_next_occurrence` with `Tz` from `[server].timezone`; DST tests pass |
| SCHED-03 | 02-01-PLAN | Clock jumps logged at WARN, catch-up runs enqueued | SATISFIED | `check_clock_jump()` with >2min threshold; WARN log per missed fire; 24h cap |
| SCHED-04 | 02-03-PLAN | Each job runs as tokio::spawn task with insert_running -> exec -> log -> finalize | SATISFIED | `run_job()` in `run.rs`; `join_set.spawn(run::run_job(..., "scheduled", ...))` in mod.rs |
| SCHED-05 | 02-03-PLAN | Per-job timeout via tokio::select!; status=timeout with partial logs | SATISFIED | `execute_child()` in command.rs has `tokio::select!` with `sleep(timeout)` arm; `test_timeout_preserves_partial_logs` passes |
| SCHED-06 | 02-03-PLAN | Concurrent runs of same job allowed; each = separate job_runs row | SATISFIED | No concurrency limit; `insert_running_run` per run; `test_concurrent_runs_same_job` passes with 2 rows |
| SCHED-07 | 02-04-PLAN | SIGINT/SIGTERM stops new fires, drains up to shutdown_grace, exits 0; second signal force-exits | SATISFIED | Drain state machine with `grace_deadline`, `abort_all()`, structured summary; `wait_for_signal()` x2 in shutdown.rs; `std::process::exit(1)` |
| EXEC-01 | 02-02-PLAN | command-type jobs via tokio::process::Command | SATISFIED | `execute_command()` in command.rs; no shell wrapper |
| EXEC-02 | 02-02-PLAN | script-type jobs write to tempfile with shebang, chmod, execute | SATISFIED | `execute_script()` in script.rs; `NamedTempFile`, `0o755`, shebang prepended |
| EXEC-03 | 02-02-PLAN | stdout/stderr captured line-by-line with correct stream tags | SATISFIED | `read_lines_to_channel()` on piped stdout ("stdout") and stderr ("stderr"); ordering preserved via BufReader |
| EXEC-04 | 02-02-PLAN | Bounded channel with head-drop (oldest dropped) + [truncated N lines] marker | SATISFIED | `log_pipeline.rs`: `pop_front()` on overflow, marker prepended on drain; Note: CONTEXT doc specifies head-drop (D-10) which deviates from EXEC-04 wording "tail-sampling" but SC-5 roadmap says "tail-sampling" — actual implementation is head-drop per D-10 decision; both the spec and the plan explicitly chose head-drop |
| EXEC-05 | 02-02-PLAN | Lines >16KB truncated with marker | SATISFIED | `MAX_LINE_BYTES = 16384`; `truncate_line()` appends `" [line truncated at 16384 bytes]"` |
| EXEC-06 | 02-02-PLAN | exit_code=0 -> status=success, non-zero -> status=failed | SATISFIED | `RunStatus::Success/Failed` mapped in `execute_child()` and `run_job()` |

**Note on EXEC-04 / SC-5 wording discrepancy:** The ROADMAP success criteria says "tail-sampling drop policy" but the CONTEXT document (D-10) and both plans explicitly specify **head-drop** (drop oldest, keep newest), citing that head-drop preserves the most recent/diagnostic output. This is a deliberate design decision documented in the CONTEXT — the implementation correctly follows the CONTEXT over the ROADMAP wording. This is not a gap; it is an intentional deviation from ambiguous spec language.

**Note on SCHED-07 second signal:** The ROADMAP SC-4 says "second SIGTERM kills immediately" and plan says "second SIGINT/SIGTERM force-exits." Implementation force-exits with `std::process::exit(1)` (not code 0) on second signal, which is correct for a forced kill scenario. The REQUIREMENTS.md SCHED-07 says "exits with code 0" only for graceful shutdown (first signal drain completing), not second-signal force-exit.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | — | No stubs, TODOs, or placeholder returns found in Phase 2 files | — | — |

The only comment-style TODOs found in mod.rs referred to Phase 3/4 Docker executor extension points which are explicitly deferred per the CONTEXT doc.

### Human Verification Required

No items require human verification. All truths were verifiable programmatically via code inspection and automated tests.

### Gaps Summary

No gaps found. All 6 roadmap success criteria are satisfied, all 13 requirements (SCHED-01 through SCHED-07, EXEC-01 through EXEC-06) are implemented and tested, all artifacts exist and are substantive, all key links are wired, and data flows through the full pipeline to real DB records.

---

_Verified: 2026-04-10T21:00:00Z_
_Verifier: Claude (gsd-verifier)_
