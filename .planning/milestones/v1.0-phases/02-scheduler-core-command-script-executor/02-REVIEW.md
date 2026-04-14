---
phase: 02-scheduler-core-command-script-executor
reviewed: 2026-04-10T12:00:00Z
depth: standard
files_reviewed: 13
files_reviewed_list:
  - src/scheduler/command.rs
  - src/scheduler/fire.rs
  - src/scheduler/log_pipeline.rs
  - src/scheduler/mod.rs
  - src/scheduler/run.rs
  - src/scheduler/script.rs
  - src/scheduler/sync.rs
  - src/db/queries.rs
  - src/cli/run.rs
  - src/shutdown.rs
  - src/db/mod.rs
  - src/lib.rs
  - tests/scheduler_integration.rs
findings:
  critical: 1
  warning: 5
  info: 3
  total: 9
status: issues_found
---

# Phase 2: Code Review Report

**Reviewed:** 2026-04-10T12:00:00Z
**Depth:** standard
**Files Reviewed:** 13
**Status:** issues_found

## Summary

The Phase 2 scheduler core is well-structured with good separation of concerns across modules (fire queue, log pipeline, command/script executors, config sync, run lifecycle). Error handling is generally thorough, and the test coverage is strong with both unit and integration tests. The code follows Rust idioms well and adheres to project constraints (no shell-out for Docker, `shell-words` for argv splitting, `croner` for cron, split SQLite pools).

Key concerns: an integer overflow in `kill_process_group` on platforms where PID can exceed `i32::MAX`, a potential data race in the log pipeline's `drain_batch_async`, and the scheduler loop using a snapshot of jobs that never updates after startup.

## Critical Issues

### CR-01: Integer overflow in `kill_process_group` PID cast

**File:** `src/scheduler/command.rs:153`
**Issue:** `child.id()` returns `Option<u32>`. The cast `pid as i32` will silently wrap on PIDs above `i32::MAX` (2,147,483,647). On Linux, PID max defaults to 32768 but can be configured up to 4,194,304 (`/proc/sys/kernel/pid_max`), and on 64-bit systems the theoretical max is `2^22`. While unlikely in practice for most homelab setups, the wrapping produces an incorrect negative PID, which would send SIGKILL to the wrong process group -- a correctness and safety issue.
**Fix:**
```rust
fn kill_process_group(child: &tokio::process::Child) {
    if let Some(pid) = child.id() {
        let pid_i32: i32 = match pid.try_into() {
            Ok(p) => p,
            Err(_) => {
                tracing::error!(
                    target: "cronduit.executor",
                    pid,
                    "PID exceeds i32::MAX, cannot kill process group"
                );
                return;
            }
        };
        unsafe {
            libc::kill(-pid_i32, libc::SIGKILL);
        }
    }
}
```

## Warnings

### WR-01: Scheduler loop uses stale job snapshot -- never re-syncs

**File:** `src/scheduler/mod.rs:44-46`
**Issue:** `SchedulerLoop::run` clones the jobs map into `jobs_vec` at line 44 and the `self.jobs` HashMap is immutable for the loop's lifetime. If jobs are added/removed/changed in the config, the scheduler has no mechanism to pick up those changes until a full restart. The fire queue and job dispatch both reference this stale snapshot. This is likely intentional for Phase 2, but the code has no comment or TODO indicating that config reload is deferred, making it easy for a future contributor to miss.
**Fix:** Add a clarifying comment at lines 44-46:
```rust
// TODO(Phase 5): Config reload. Currently the job set is immutable for the
// scheduler's lifetime. Hot-reload will require rebuilding the heap and
// updating self.jobs via a channel or watch.
let jobs_vec: Vec<DbJob> = self.jobs.values().cloned().collect();
```

### WR-02: `drain_batch_async` has a TOCTOU race between empty-check and `notified()`

**File:** `src/scheduler/log_pipeline.rs:116-128`
**Issue:** Between calling `drain_batch` (which returns empty) at line 118 and checking `is_closed` at line 122, a sender could both send a line and close the channel. The `notified()` at line 126 would then wait forever because the notification was already consumed by a prior wakeup that found the batch empty. In practice, this race is unlikely because the sender closes after the executor completes, but it could cause the log writer task to hang indefinitely in edge cases (e.g., a very fast-completing process where send + close happen between the two lock acquisitions).
**Fix:** Check the buffer again after checking `is_closed`, or restructure to hold the lock across both checks:
```rust
pub async fn drain_batch_async(&self, max: usize) -> Vec<LogLine> {
    loop {
        {
            let state = self.state.lock().unwrap();
            if !state.buf.is_empty() || state.dropped_count > 0 {
                drop(state);
                return self.drain_batch(max);
            }
            if state.closed {
                return vec![];
            }
        }
        self.notify.notified().await;
    }
}
```

### WR-03: `timeout_secs` cast from `i64` to `u64` without bounds check

**File:** `src/scheduler/run.rs:76-79`
**Issue:** `job.timeout_secs` is `i64` (from the DB schema). If it is negative (due to a corrupted DB row or bad migration), the check `job.timeout_secs <= 0` falls through to the else branch at line 79, where `job.timeout_secs as u64` would wrap a negative value to a very large `u64`, creating an effectively infinite timeout rather than the intended 1-year fallback. The `<= 0` check should use the fallback for negative values too, but the real issue is that the `as u64` cast on an unexpected negative value produces a silently wrong result.
**Fix:**
```rust
let timeout = if job.timeout_secs > 0 {
    Duration::from_secs(job.timeout_secs as u64)
} else {
    Duration::from_secs(86400 * 365) // effectively no timeout
};
```
This is actually the same logic -- `<= 0` already handles negatives. The current code is correct, but worth a comment explaining that negative values are intentionally treated as "no timeout." Consider using `u64` for `timeout_secs` in `DbJob` if the schema allows it, to make the type system enforce non-negativity.

### WR-04: Postgres `disable_missing_jobs` uses `!= ALL` instead of `NOT IN` / `<> ALL`

**File:** `src/db/queries.rs:164`
**Issue:** The query `name != ALL($1)` is semantically incorrect for the intended behavior. `!= ALL(array)` means "name is not equal to every element" which is equivalent to `name NOT IN (array)` only when the array is non-empty and contains no NULLs. However, the `!=` operator is non-standard SQL. The standard operator is `<>`. More importantly, if `active_names` contains duplicates or if Postgres coerces the bind differently, this could produce unexpected results. The standard idiom is `NOT (name = ANY($1))`.
**Fix:**
```rust
let result = sqlx::query(
    "UPDATE jobs SET enabled = false WHERE enabled = true AND name <> ALL($1)",
)
```
Or more idiomatically:
```rust
let result = sqlx::query(
    "UPDATE jobs SET enabled = false WHERE enabled = true AND NOT (name = ANY($1))",
)
```

### WR-05: `finalize_run` parameter named `duration` is actually a start `Instant`

**File:** `src/db/queries.rs:325`
**Issue:** The parameter `duration: tokio::time::Instant` is misleadingly named. It is not a duration -- it is the start instant from which elapsed time is computed at line 328. This will confuse future contributors and could lead to incorrect usage (e.g., passing an actual `Duration` if the signature ever changes).
**Fix:** Rename the parameter to `start` or `start_instant`:
```rust
pub async fn finalize_run(
    pool: &DbPool,
    run_id: i64,
    status: &str,
    exit_code: Option<i32>,
    start_instant: tokio::time::Instant,
    error_message: Option<&str>,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let duration_ms = start_instant.elapsed().as_millis() as i64;
```
Note: The caller in `run.rs:169` already uses a variable named `start`, so this is consistent.

## Info

### IN-01: Unused parameter `_tz` in `requeue_job` and `check_clock_jump`

**File:** `src/scheduler/fire.rs:118`, `src/scheduler/fire.rs:188`
**Issue:** Both `requeue_job` and `check_clock_jump` accept a `_tz: Tz` parameter that is never used (prefixed with `_` to suppress warnings). This suggests the parameter was planned for future use but currently adds noise to the API surface.
**Fix:** Remove the `_tz` parameter if it is not needed, or add a comment explaining its planned use. If kept for API stability, that is fine but should be documented.

### IN-02: `truncate_line` slices at byte boundary without UTF-8 awareness

**File:** `src/scheduler/log_pipeline.rs:169`
**Issue:** `line[..MAX_LINE_BYTES]` slices at a byte offset. If the input contains multi-byte UTF-8 characters and the boundary falls in the middle of a character, this will panic at runtime. Rust's `String` indexing panics on non-char boundaries.
**Fix:** Use `char_indices` or `floor_char_boundary` (nightly) to find a safe truncation point:
```rust
pub fn truncate_line(line: String) -> String {
    if line.len() <= MAX_LINE_BYTES {
        line
    } else {
        // Find the last valid char boundary at or before MAX_LINE_BYTES.
        let mut end = MAX_LINE_BYTES;
        while !line.is_char_boundary(end) {
            end -= 1;
        }
        let mut truncated = line[..end].to_string();
        truncated.push_str(" [line truncated at 16384 bytes]");
        truncated
    }
}
```

### IN-03: `serialize_config_json` uses `unwrap_or_default` which silently swallows serialization errors

**File:** `src/scheduler/sync.rs:73`
**Issue:** `serde_json::to_string(...).unwrap_or_default()` will produce an empty string if serialization fails, which would then be stored in the DB as config_json. This would make the config_hash and config_json inconsistent, potentially causing the sync engine to skip updates it should apply. While `serde_json::Value::Object` serialization is unlikely to fail, a silent fallback to `""` is fragile.
**Fix:** Propagate the error or use `expect` with a clear message:
```rust
serde_json::to_string(&serde_json::Value::Object(map))
    .expect("serializing JSON object should never fail")
```

---

_Reviewed: 2026-04-10T12:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
