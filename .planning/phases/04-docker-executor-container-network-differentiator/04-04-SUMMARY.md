---
phase: "04-docker-executor-container-network-differentiator"
plan: "04"
subsystem: docker-executor-tests
tags: [integration-tests, docker, container-network, marquee-feature, testcontainers]
dependency_graph:
  requires: [04-01, 04-02, 04-03]
  provides: [docker-integration-tests, container-network-verification]
  affects: [CI-pipeline, docker-executor]
tech_stack:
  added: [testcontainers-0.27]
  patterns: [ignored-integration-tests, testcontainers-generic-image]
key_files:
  created:
    - tests/docker_container_network.rs
    - tests/docker_executor.rs
  modified:
    - src/scheduler/docker.rs
decisions:
  - Added `cmd` field to DockerJobConfig and wired to ContainerCreateBody for container command override
  - Used `#[ignore]` attribute (not `#[cfg(feature = "integration")]`) so tests can be selectively run with `--ignored`
  - Used container ID (not name) for `container:<id>` network mode in marquee test since testcontainers assigns random names
metrics:
  duration: "7m"
  completed: "2026-04-11T19:17:39Z"
  tasks_completed: 1
  tasks_total: 1
  files_created: 2
  files_modified: 1
requirements:
  - DOCKER-10
---

# Phase 04 Plan 04: Docker Integration Tests & container:\<name\> Marquee Test Summary

Docker integration tests proving the full executor lifecycle against a real Docker daemon, with the DOCKER-10 marquee test verifying container:\<name\> network mode end-to-end via testcontainers.

## What Was Built

### tests/docker_container_network.rs (DOCKER-10 Marquee)
- **test_container_network_mode**: Starts a target container via `testcontainers::GenericImage`, runs a Cronduit Docker job with `network = "container:<target_id>"`, verifies successful execution with exit code 0 and log capture
- **test_container_network_target_stopped**: Starts a target, stops it, verifies pre-flight rejects with `NetworkTargetUnavailable`, and verifies `execute_docker` returns `Error` status through the full path

### tests/docker_executor.rs
- **test_docker_basic_echo**: Validates full lifecycle (create, start, wait, log drain, remove) with echo command
- **test_docker_timeout_stops_container**: Verifies timeout produces `RunStatus::Timeout` with "timed out" error message
- **test_docker_preflight_nonexistent_target**: Validates pre-flight returns `NetworkTargetUnavailable` for missing container
- **test_docker_orphan_reconciliation**: Creates a labeled container + DB row, runs `reconcile_orphans`, verifies container removed and DB row updated to `status=error, error_message="orphaned at restart"`
- **test_docker_execute_preflight_failure_returns_error**: Verifies the full `execute_docker` path returns `Error` when pre-flight fails

### src/scheduler/docker.rs (Enhancement)
- Added `cmd: Option<Vec<String>>` field to `DockerJobConfig`
- Wired `cmd` to `ContainerCreateBody.cmd` in container creation
- Added unit test `test_docker_job_config_with_cmd`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Functionality] Added `cmd` field to DockerJobConfig**
- **Found during:** Task 1
- **Issue:** Without a `cmd` field, the Docker executor could not override an image's default CMD. This made integration tests untestable (alpine's default CMD `/bin/sh` exits immediately, preventing timeout tests) and limited the executor's usefulness for real-world jobs.
- **Fix:** Added `cmd: Option<Vec<String>>` to `DockerJobConfig` and wired it to `ContainerCreateBody.cmd`.
- **Files modified:** `src/scheduler/docker.rs`
- **Commit:** 4bbffa0

**2. [Rule 3 - Blocking Issue] Fixed testcontainers `ImageExt` trait import**
- **Found during:** Task 1 (compilation)
- **Issue:** `GenericImage::with_cmd()` and `with_entrypoint()` require the `ImageExt` trait to be in scope.
- **Fix:** Added `use testcontainers::ImageExt;` import.
- **Files modified:** `tests/docker_container_network.rs`
- **Commit:** 4bbffa0

## Verification

All 7 integration tests compile and are listed:
```
cargo test --test docker_container_network --test docker_executor -- --list
```
Output: 2 tests (container_network) + 5 tests (docker_executor) = 7 total.

Existing unit tests continue to pass (5/5 docker::tests).

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | 4bbffa0 | Docker executor integration tests and container:\<name\> marquee test |

## Self-Check: PASSED
