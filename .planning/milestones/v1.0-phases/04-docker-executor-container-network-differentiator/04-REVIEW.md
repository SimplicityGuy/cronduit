---
phase: 04-docker-executor-container-network-differentiator
reviewed: 2026-04-11T12:00:00Z
depth: standard
files_reviewed: 12
files_reviewed_list:
  - Cargo.toml
  - src/cli/run.rs
  - src/db/queries.rs
  - src/scheduler/docker.rs
  - src/scheduler/docker_log.rs
  - src/scheduler/docker_orphan.rs
  - src/scheduler/docker_preflight.rs
  - src/scheduler/docker_pull.rs
  - src/scheduler/mod.rs
  - src/scheduler/run.rs
  - tests/docker_container_network.rs
  - tests/docker_executor.rs
findings:
  critical: 1
  warning: 3
  info: 3
  total: 7
status: issues_found
---

# Phase 4: Code Review Report

**Reviewed:** 2026-04-11T12:00:00Z
**Depth:** standard
**Files Reviewed:** 12
**Status:** issues_found

## Summary

Reviewed the Docker executor implementation including container lifecycle management, network pre-flight validation (`container:<name>` mode), image pull with retry, orphan reconciliation, log streaming, and integration tests. The implementation is well-structured with clear separation of concerns across modules. However, there is one critical semantic bug where `container_id` is populated with the image digest instead of the actual container ID, plus several warnings around edge cases and a potential integer overflow.

## Critical Issues

### CR-01: `container_id` field stores image digest instead of container ID

**File:** `src/scheduler/docker.rs:330-331`
**Issue:** At the end of `execute_docker`, the `DockerExecResult` is returned with `container_id: Some(image_digest)` instead of `container_id: Some(container_id)`. The `image_digest` variable holds the image hash (from `inspect_container`), not the container ID. This means:
1. The `container_id` field stored in `job_runs` via `finalize_run` will contain the image digest, not the actual container ID.
2. The `DockerExecResult.container_id` field is semantically wrong -- its doc comment says "Image digest from `inspect_container`" but the struct field is named `container_id`.
3. Downstream consumers (orphan reconciliation, UI) that read `container_id` from the DB will get an image hash, which is confusing and potentially incorrect for diagnostics.

**Fix:** Either rename the field to `image_digest` to match what it actually stores, or store the actual container ID and add a separate `image_digest` field. The minimal fix for the naming confusion:
```rust
// In DockerExecResult struct:
pub struct DockerExecResult {
    pub exec: ExecResult,
    /// Image digest from inspect_container (DOCKER-09).
    pub image_digest: Option<String>,
}

// At the return site (line 328-331):
DockerExecResult {
    exec: exec_result,
    image_digest: Some(image_digest),
}
```
And update `src/scheduler/run.rs:149` to use `docker_result.image_digest` instead of `docker_result.container_id`. Also consider whether `finalize_run`'s `container_id` parameter should be renamed to `image_digest` in the DB schema, or if both values should be stored.

## Warnings

### WR-01: Timeout cast may overflow for large `timeout_secs` values

**File:** `src/scheduler/run.rs:78-82`
**Issue:** `job.timeout_secs` is `i64` and is cast to `u64` via `as u64` on line 82. If `timeout_secs` is negative (not caught by the `<= 0` check which handles it) this is safe, but a value of 0 falls through to the else branch where `Duration::from_secs(0)` would cause an immediate timeout on every run. The `<= 0` guard correctly catches this, but the fallback of `86400 * 365` seconds (effectively ~31.5 million seconds) for a zero/negative timeout is undocumented behavior that operators might not expect.

**Fix:** Document the behavior explicitly and consider logging a warning when timeout_secs is <= 0:
```rust
let timeout = if job.timeout_secs <= 0 {
    tracing::debug!(
        target: "cronduit.run",
        job = %job.name,
        timeout_secs = job.timeout_secs,
        "timeout_secs <= 0, using effectively-infinite timeout (1 year)"
    );
    Duration::from_secs(86400 * 365)
} else {
    Duration::from_secs(job.timeout_secs as u64)
};
```

### WR-02: `_expected_wake_dt` computed but never used

**File:** `src/scheduler/mod.rs:68-69`
**Issue:** The variable `_expected_wake_dt` is computed every loop iteration but prefixed with `_` indicating it is intentionally unused. Meanwhile, `last_expected_wake` (line 56) is used for clock-jump detection but is set to `Utc::now()` rather than the calculated expected wake time. This means the clock-jump detection in `check_clock_jump` is comparing against the actual wake time rather than the expected wake time, which could miss small clock jumps.

**Fix:** Either use `_expected_wake_dt` as the expected wake value for clock-jump detection, or remove the dead computation:
```rust
// Option A: Use it properly
let expected_wake_dt = Utc::now().with_timezone(&self.tz)
    + chrono::Duration::from_std(sleep_duration).unwrap_or(chrono::Duration::zero());
// ... then after waking:
let missed = fire::check_clock_jump(expected_wake_dt, now_tz, self.tz, &jobs_vec);

// Option B: Remove the dead code
// Delete lines 68-69 entirely
```

### WR-03: `last_expected_wake` initialized to current time, not the first expected wake

**File:** `src/scheduler/mod.rs:56`
**Issue:** `last_expected_wake` is initialized to `Utc::now().with_timezone(&self.tz)` and then updated to `now_tz` on line 103 after each wake. This means on the first iteration, the clock-jump check compares the actual wake time against the time the scheduler started, not against the expected first-fire time. If the first fire is far in the future, the gap between scheduler start and the first wake will look like a clock jump. This is related to WR-02 -- the two issues together suggest the clock-jump detection may not work as intended.

**Fix:** Initialize `last_expected_wake` properly and update it to the expected wake time (not actual) after each iteration, similar to the fix in WR-02.

## Info

### IN-01: Empty lines silently dropped in Docker log streaming

**File:** `src/scheduler/docker_log.rs:44`
**Issue:** Lines that are empty after splitting on newlines are silently skipped. This means if a container outputs intentional blank lines (common in formatted output), they will be lost. This may or may not be desired behavior.

**Fix:** Consider whether blank lines should be preserved. If so, remove the `!line.is_empty()` check, or make it configurable.

### IN-02: Hardcoded retry count in `ensure_image`

**File:** `src/scheduler/docker_pull.rs:197`
**Issue:** The retry count is hardcoded to `3` in `ensure_image`. This is reasonable for v1 but could be made configurable via job config or global config in a future iteration.

**Fix:** No change needed for v1; note for future configurability.

### IN-03: `backoffs` array size and `max_attempts` are not coupled

**File:** `src/scheduler/docker_pull.rs:67`
**Issue:** The `backoffs` array has 3 elements `[1, 2, 4]` and `max_attempts` is passed as a parameter. If a caller passes `max_attempts > 3`, the `unwrap_or(4)` on line 110 handles it, but this implicit coupling between the array size and max attempts could be surprising.

**Fix:** Consider deriving the backoff dynamically (e.g., `2^(attempt-1)` capped at some max) rather than using a fixed array, or document the relationship.

---

_Reviewed: 2026-04-11T12:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
