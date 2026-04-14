---
phase: 02-scheduler-core-command-script-executor
plan: 02
subsystem: scheduler
tags: [tokio, process, shell-words, tempfile, log-pipeline, head-drop-channel]

# Dependency graph
requires:
  - phase: 01-foundation-security-posture-persistence-base
    provides: config types (JobConfig), DB schema (job_logs table), Cargo.toml dependencies
provides:
  - HeadDropChannel (LogSender/LogReceiver) with 256-line bounded buffer and head-drop backpressure
  - Line truncation at 16 KB with marker
  - execute_command() with shell-words argv splitting and process group management
  - execute_script() with tempfile + shebang + chmod + auto-cleanup
  - ExecResult/RunStatus types for all execution outcomes
  - read_lines_to_channel() for async stdout/stderr capture
affects: [02-03-scheduler-loop, 02-04-log-writer, 03-web-ui]

# Tech tracking
tech-stack:
  added: [shell-words 1.1, tempfile 3, libc 0.2]
  patterns: [head-drop-channel, process-group-kill, shared-execute-child]

key-files:
  created:
    - src/scheduler/log_pipeline.rs
    - src/scheduler/command.rs
    - src/scheduler/script.rs
    - src/scheduler/mod.rs
  modified:
    - src/lib.rs
    - Cargo.toml

key-decisions:
  - "Used std::sync::Mutex + Arc + Notify for channel (no tokio::sync::mpsc) to support head-drop semantics"
  - "Factored execute_child() as shared logic between command and script executors"
  - "Process group kill via libc::kill(-pid, SIGKILL) for clean timeout/shutdown of child process trees"

patterns-established:
  - "Head-drop channel: VecDeque with pop_front on overflow, truncation marker prepended on drain"
  - "Process execution: shell-words split -> Command::new -> process_group(0) -> select!(wait, timeout, cancel)"
  - "Log capture: BufReader::lines on piped stdout/stderr -> make_log_line with truncation -> LogSender"

requirements-completed: [EXEC-01, EXEC-02, EXEC-03, EXEC-04, EXEC-05, EXEC-06]

# Metrics
duration: 9min
completed: 2026-04-10
---

# Phase 2 Plan 02: Command & Script Executor Summary

**Head-drop bounded log channel with 16 KB line truncation, command executor via shell-words tokenization, and script executor via tempfile with shebang and auto-cleanup**

## Performance

- **Duration:** 9 min
- **Started:** 2026-04-10T19:57:02Z
- **Completed:** 2026-04-10T20:05:49Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Head-drop bounded channel (LogSender/LogReceiver) correctly drops oldest lines when at capacity, preserving recent output for failure diagnosis
- Command execution backend tokenizes via shell-words (no shell invocation), captures stdout/stderr with stream tags, handles timeout via process group SIGKILL
- Script execution backend writes tempfile with configurable shebang, chmod 0o755, executes directly, auto-deletes on completion
- 20 comprehensive unit tests covering all channel behaviors, execution paths, exit codes, timeout, shutdown, and tempfile cleanup

## Task Commits

Each task was committed atomically:

1. **Task 1: Head-drop bounded log channel + line truncation** - `57f4988` (feat)
2. **Task 2: Command executor + script executor** - `7f305b3` (feat)

## Files Created/Modified
- `src/scheduler/log_pipeline.rs` - Head-drop bounded channel with LogSender/LogReceiver, line truncation, make_log_line helper
- `src/scheduler/command.rs` - ExecResult/RunStatus types, execute_command with shell-words, execute_child shared logic, process group kill
- `src/scheduler/script.rs` - execute_script with tempfile + shebang + chmod + auto-cleanup
- `src/scheduler/mod.rs` - Module declarations for log_pipeline, command, script
- `src/lib.rs` - Added scheduler module
- `Cargo.toml` - Added shell-words, tempfile, libc to runtime deps; removed from dev-deps

## Decisions Made
- Used `std::sync::Mutex` (not tokio Mutex) for the channel state since operations are fast lock-unlock with no async work inside the critical section -- simpler and avoids Send/Sync issues
- Factored `execute_child()` as a `pub(crate)` function in `command.rs` that both `execute_command` and `execute_script` call, keeping the timeout/cancel/kill logic DRY
- Process group kill via `libc::kill(-(pid as i32), libc::SIGKILL)` ensures child processes and their descendants are killed on timeout/shutdown (T-02-06 mitigation)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Log pipeline (LogSender/LogReceiver) ready for Plan 03's per-run writer task and Plan 04's log writer micro-batch inserts
- execute_command/execute_script ready for Plan 03's scheduler loop to wire into run tasks
- ExecResult/RunStatus types provide the contract for mapping execution outcomes to job_runs status

## Self-Check: PASSED

- All 4 created files exist on disk
- Both task commits (57f4988, 7f305b3) found in git log
- 20/20 tests passing
- cargo build clean
- cargo clippy clean

---
*Phase: 02-scheduler-core-command-script-executor*
*Completed: 2026-04-10*
