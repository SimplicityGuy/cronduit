---
phase: 04-docker-executor-container-network-differentiator
plan: 01
subsystem: scheduler
tags: [bollard, docker, container-lifecycle, futures-util]

# Dependency graph
requires:
  - phase: 02-scheduler-core-command-script-executor
    provides: ExecResult, RunStatus, LogSender, make_log_line, log_pipeline
provides:
  - Docker executor entry point: execute_docker()
  - Docker log streaming: stream_docker_logs()
  - DockerExecResult with image digest
  - DockerJobConfig deserialization from JSON
affects: [04-02, 04-03, 04-04]

# Tech tracking
tech-stack:
  added: [bollard 0.20, futures-util 0.3]
  patterns: [bollard 0.20 query_parameters API, ContainerCreateBody, explicit container removal]

key-files:
  created:
    - src/scheduler/docker.rs
    - src/scheduler/docker_log.rs
  modified:
    - Cargo.toml
    - src/scheduler/mod.rs

key-decisions:
  - "Used bollard default features (no json/ssl feature needed) - connects via Unix socket"
  - "Implemented full lifecycle in single pass rather than placeholder-then-complete approach"

patterns-established:
  - "Docker executor pattern: create -> start -> inspect -> select(wait|timeout|cancel) -> drain logs -> remove"
  - "cleanup_container helper for consistent force-removal with warning logging"

requirements-completed: [DOCKER-01, DOCKER-04, DOCKER-06, DOCKER-07, DOCKER-08, DOCKER-09]

# Metrics
duration: 10min
completed: 2026-04-11
---

# Phase 4 Plan 01: Docker Executor Core Summary

**Docker container lifecycle executor via bollard 0.20 with auto_remove=false, 10s SIGTERM grace timeout, concurrent log streaming, and explicit post-drain container removal**

## Performance

- **Duration:** 10 min
- **Started:** 2026-04-11T18:24:53Z
- **Completed:** 2026-04-11T18:35:27Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Full Docker container lifecycle: create with labels/env/volumes/network -> start -> inspect for digest -> concurrent wait+log stream -> timeout/cancel with 10s SIGTERM -> drain logs to EOF -> explicit remove
- bollard 0.20 integrated without pulling openssl-sys (rustls constraint maintained)
- DockerJobConfig deserialization from JSON with required image field and optional env/volumes/network/container_name
- DockerExecResult extends ExecResult with image digest for DB storage

## Task Commits

Each task was committed atomically:

1. **Task 1: Add bollard dependency and create docker.rs executor skeleton with container creation** - `3ada394` (feat)
2. **Task 2: Implement concurrent wait + log stream + timeout/cancel + explicit remove lifecycle** - included in `3ada394` (implemented full lifecycle in Task 1 rather than placeholder approach)

## Files Created/Modified
- `Cargo.toml` - Added bollard 0.20 and futures-util 0.3 dependencies
- `src/scheduler/docker.rs` - Docker executor with execute_docker(), DockerJobConfig, DockerExecResult, cleanup_container
- `src/scheduler/docker_log.rs` - Log streaming via stream_docker_logs() with stdout/stderr/console handling
- `src/scheduler/mod.rs` - Registered docker and docker_log modules

## Decisions Made
- Used bollard default features instead of `features = ["json"]` as bollard 0.20 removed that feature flag; default features connect via Unix socket which is what we need
- Implemented the complete lifecycle (Tasks 1 and 2) in a single commit since the code was straightforward and splitting would have required a non-compiling placeholder
- Used `bollard::models::ContainerCreateBody` (bollard 0.20 API) instead of the old `bollard::container::Config` which no longer exists
- Used `bollard::query_parameters::*` for options types (bollard 0.20 moved these from `bollard::container`)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] bollard 0.20 API changes from plan assumptions**
- **Found during:** Task 1 (compilation)
- **Issue:** Plan assumed bollard features `["ssl", "json"]` and old API types (Config, generic parameters on methods). Bollard 0.20 removed these features, renamed `Config` to `ContainerCreateBody`, moved options to `query_parameters`, and removed generic parameters from `wait_container`/`start_container`.
- **Fix:** Used default features, `bollard::models::ContainerCreateBody`, `bollard::query_parameters::*`, and non-generic method calls
- **Files modified:** Cargo.toml, src/scheduler/docker.rs, src/scheduler/docker_log.rs
- **Verification:** `cargo check` passes, `cargo test --lib scheduler::docker::tests` passes (4/4)
- **Committed in:** 3ada394

---

**Total deviations:** 1 auto-fixed (1 blocking - API mismatch)
**Impact on plan:** API surface change required import/type adjustments. No scope creep. All acceptance criteria met.

## Issues Encountered
None beyond the bollard API changes documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Docker executor core is ready for integration with the scheduler run loop (Plan 04-02)
- `execute_docker()` returns `DockerExecResult` compatible with the existing `ExecResult`/`RunStatus` pattern
- `stream_docker_logs()` uses the same `LogSender`/`make_log_line` pipeline as command/script executors

---
*Phase: 04-docker-executor-container-network-differentiator*
*Completed: 2026-04-11*
