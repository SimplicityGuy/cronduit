---
phase: 04-docker-executor-container-network-differentiator
plan: 03
subsystem: scheduler/docker
tags: [docker, orphan-reconciliation, executor-wiring, container-lifecycle]
dependency_graph:
  requires: [04-01, 04-02]
  provides: [docker-dispatch, orphan-reconciliation, finalize-run-container-id]
  affects: [scheduler-loop, startup-flow, run-lifecycle]
tech_stack:
  added: []
  patterns: [builder-pattern-bollard-api, option-docker-graceful-fallback]
key_files:
  created:
    - src/scheduler/docker_orphan.rs
  modified:
    - src/scheduler/run.rs
    - src/scheduler/mod.rs
    - src/scheduler/docker.rs
    - src/cli/run.rs
    - src/db/queries.rs
    - Cargo.toml
decisions:
  - "Docker client is Option<Docker> -- non-fatal if Docker unavailable at startup"
  - "Orphan reconciliation runs before scheduler spawn, not inside the loop"
  - "container_id parameter added to finalize_run for image digest storage"
metrics:
  duration_seconds: 1374
  completed: 2026-04-11T18:59:01Z
  tasks_completed: 2
  tasks_total: 2
  files_changed: 7
---

# Phase 04 Plan 03: Docker Executor Wiring and Orphan Reconciliation Summary

Integration plan connecting Docker executor (Plan 01) and image pull/preflight (Plan 02) into the running system with orphan reconciliation at startup.

## One-liner

Orphan container reconciliation at startup with Docker executor dispatch wired into scheduler loop and finalize_run storing image digests.

## Task Results

| Task | Name | Commit | Status |
|------|------|--------|--------|
| 1 | Orphan reconciliation module and finalize_run container_id update | 2c26fbd | Done |
| 2 | Wire Docker executor into run.rs dispatch and orphan reconciliation into startup | 1ddcfb8 | Done |

## Implementation Details

### Task 1: Orphan Reconciliation and finalize_run Update

- Created `src/scheduler/docker_orphan.rs` with `reconcile_orphans()` function
- Lists all containers with `cronduit.run_id` label (including stopped via `all: true`)
- Running orphans stopped with 10s SIGTERM grace (D-07)
- All orphans removed with `force: true` (D-08)
- Each orphan logged at WARN level with container_id, job_name, run_id (D-09)
- DB rows marked as `status=error, error_message="orphaned at restart"` with `AND status = 'running'` guard (T-04-12)
- Updated `finalize_run` signature to accept `container_id: Option<&str>` parameter
- Updated SQLite/Postgres UPDATE queries to include `container_id` column
- Fixed duplicate `bollard` and `futures-util` entries in Cargo.toml from Wave 1

### Task 2: Docker Executor Wiring

- Replaced Phase 4 placeholder in `run.rs` with actual `execute_docker` call
- Added `docker: Option<Docker>` parameter to `run_job`, `SchedulerLoop`, and `spawn()`
- Docker client created at startup via `connect_with_local_defaults()` with graceful fallback
- Orphan reconciliation runs before `scheduler::spawn` (SCHED-08)
- Integrated `preflight_network` and `ensure_image` into `execute_docker` before container creation
- `container_id_for_finalize` captures image digest from Docker executor for DB storage
- All existing tests updated for new `docker` parameter (passing `None` for command/script tests)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed duplicate Cargo.toml entries**
- **Found during:** Task 1
- **Issue:** Wave 1 agents left duplicate `bollard` and `futures-util` entries in Cargo.toml
- **Fix:** Removed duplicate entries, keeping the original ones
- **Files modified:** Cargo.toml
- **Commit:** 2c26fbd

**2. [Rule 1 - Bug] Adapted to bollard 0.20 API types**
- **Found during:** Task 1
- **Issue:** `ListContainersOptions` uses builder pattern in bollard 0.20, `ContainerSummaryStateEnum` is an enum not a string
- **Fix:** Used `ListContainersOptionsBuilder` and compared against `ContainerSummaryStateEnum::RUNNING` variant
- **Files modified:** src/scheduler/docker_orphan.rs
- **Commit:** 2c26fbd

## Decisions Made

1. **Docker client as `Option<Docker>`**: If Docker socket is unavailable at startup, Cronduit continues running but docker-type jobs will fail with a clear error. This is the correct behavior for a tool that also handles command/script jobs.

2. **Orphan reconciliation before scheduler spawn**: Ensures no leftover containers from a previous crash interfere with new runs. Runs once at startup, not periodically.

3. **`container_id` stores image digest**: The `container_id` column in `job_runs` receives the image digest (sha256) from `inspect_container`, not the ephemeral container ID. This matches DOCKER-09 requirements.

## Verification

- `cargo test --lib` passes with 109 tests (0 failures)
- All existing command/script tests pass with updated `finalize_run` and `run_job` signatures
- Docker executor dispatched for `job_type="docker"` in run.rs
- Orphan reconciliation called before `scheduler::spawn` in cli/run.rs

## Self-Check: PASSED

- [x] src/scheduler/docker_orphan.rs exists
- [x] Commit 2c26fbd exists
- [x] Commit 1ddcfb8 exists
- [x] All 7 files changed
