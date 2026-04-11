---
phase: 04-docker-executor-container-network-differentiator
fixed_at: 2026-04-11T12:30:00Z
review_path: .planning/phases/04-docker-executor-container-network-differentiator/04-REVIEW.md
iteration: 1
findings_in_scope: 4
fixed: 3
skipped: 1
status: partial
---

# Phase 4: Code Review Fix Report

**Fixed at:** 2026-04-11T12:30:00Z
**Source review:** .planning/phases/04-docker-executor-container-network-differentiator/04-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 4
- Fixed: 3
- Skipped: 1

## Fixed Issues

### CR-01: `container_id` field stores image digest instead of container ID

**Files modified:** `src/scheduler/docker.rs`, `src/scheduler/run.rs`
**Commit:** 1a14513
**Applied fix:** Renamed the `container_id` field in `DockerExecResult` to `image_digest` to match what the field actually stores (the image digest from `inspect_container`, not the container ID). Updated all struct literal usages across `docker.rs` (6 return sites) and the consumer in `run.rs` line 149 (`docker_result.image_digest`). The start-container error path previously returned `Some(container_id)` which was changed to `None` since no image digest is available at that point (container was created but never inspected).

### WR-02 / WR-03: `_expected_wake_dt` unused and `last_expected_wake` set to actual wake time

**Files modified:** `src/scheduler/mod.rs`
**Commit:** 4ec3568
**Applied fix:** Removed the underscore prefix from `_expected_wake_dt` making it `expected_wake_dt`, and updated `last_expected_wake` assignment from `now_tz` (the actual wake time) to `expected_wake_dt` (the computed expected wake time). This ensures `check_clock_jump` receives the expected wake time from the previous iteration rather than the actual time, allowing it to correctly detect clock jumps even when the scheduler wakes slightly early or late. This is a logic fix that requires human verification since the clock-jump detection semantics are subtle.

## Skipped Issues

### WR-01: Timeout cast may overflow for large `timeout_secs` values

**File:** `src/scheduler/run.rs:78-82`
**Reason:** Already fixed by prior commit `a2a78b9` ("fix(02): WR-03 add safety comment for timeout_secs i64-to-u64 cast guard"). The `tracing::debug!` log for the `<= 0` fallback path and the safety comments for the `as u64` cast are already present in the committed code.
**Original issue:** Undocumented behavior when timeout_secs is <= 0, falling back to effectively-infinite timeout.

---

_Fixed: 2026-04-11T12:30:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
