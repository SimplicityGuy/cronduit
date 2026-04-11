---
phase: 04-docker-executor-container-network-differentiator
plan: 02
subsystem: scheduler
tags: [bollard, docker, image-pull, network-preflight, retry, error-classification]

# Dependency graph
requires:
  - phase: 02-scheduler-core-command-script-executor
    provides: ExecResult/RunStatus pattern, scheduler module structure
provides:
  - pull_image_with_retry() with 3-attempt exponential backoff and terminal/transient classification
  - ensure_image() entry point with local-first check
  - preflight_network() for container:<name> and named network validation
  - PreflightError enum with three distinct error categories
  - PullError enum with Transient/Terminal variants
affects: [04-01-docker-executor, 04-03-orphan-reconciliation]

# Tech tracking
tech-stack:
  added: [bollard 0.20, futures-util 0.3]
  patterns: [error classification for Docker API errors, exponential backoff retry, structured pre-flight validation]

key-files:
  created:
    - src/scheduler/docker_pull.rs
    - src/scheduler/docker_preflight.rs
  modified:
    - src/scheduler/mod.rs
    - Cargo.toml

key-decisions:
  - "Used bollard builder pattern (CreateImageOptionsBuilder) for bollard 0.20 API compatibility"
  - "Error classification via string matching on bollard error messages -- simple and covers all known Docker registry error patterns"
  - "Built-in network modes (bridge, host, none, empty) skip preflight entirely -- no Docker API calls for default modes"

patterns-established:
  - "Docker error classification: classify bollard errors into actionable categories before surfacing to operators"
  - "Structured error messages with prefix pattern (docker_unavailable:, network_target_unavailable:, network_not_found:) for DB storage and UI display"
  - "Pre-flight validation pattern: validate external dependencies before container creation to produce clear errors"

requirements-completed: [DOCKER-02, DOCKER-03, DOCKER-05]

# Metrics
duration: 10min
completed: 2026-04-11
---

# Phase 4 Plan 2: Image Pull and Network Pre-flight Summary

**Image pull with 3-attempt exponential backoff, terminal/transient error classification, and network pre-flight validation for container:<name> and named networks with three distinct error categories**

## Performance

- **Duration:** 10 min
- **Started:** 2026-04-11T18:24:55Z
- **Completed:** 2026-04-11T18:34:36Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Image pull with retry logic: transient errors retry 3 times with 1s/2s/4s backoff, terminal errors (unauthorized, manifest unknown) fail immediately
- Digest extraction from pull response stream and local image inspection
- Network pre-flight validation: container:<name> checks target is running, named networks verified via inspect, built-in modes skip validation
- Three distinct pre-flight error categories (DockerUnavailable, NetworkTargetUnavailable, NetworkNotFound) with structured messages for DB storage
- Added bollard 0.20 and futures-util 0.3 as project dependencies

## Task Commits

Each task was committed atomically:

1. **Task 1: Image pull with retry, error classification, and digest extraction** - `61d8cfa` (feat)
2. **Task 2: Network pre-flight validation with distinct error categories** - `057335a` (feat)

## Files Created/Modified
- `src/scheduler/docker_pull.rs` - Image pull with retry, error classification (PullError), ensure_image entry point, digest extraction
- `src/scheduler/docker_preflight.rs` - Network pre-flight validation (PreflightError), container:<name> and named network checks
- `src/scheduler/mod.rs` - Registered docker_pull and docker_preflight modules
- `Cargo.toml` - Added bollard 0.20 and futures-util 0.3 dependencies

## Decisions Made
- Used bollard 0.20 builder pattern (CreateImageOptionsBuilder) -- the old struct-based CreateImageOptions API is no longer available in bollard 0.20
- Error classification uses string matching on bollard error messages -- pragmatic approach that covers Docker registry error patterns without coupling to specific HTTP status codes
- Built-in network modes bypass pre-flight entirely to avoid unnecessary Docker API calls

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added bollard and futures-util dependencies**
- **Found during:** Task 1 (Image pull implementation)
- **Issue:** bollard was not yet in Cargo.toml despite being locked in project spec; futures-util needed for StreamExt on bollard response streams
- **Fix:** Added `bollard = "0.20"` and `futures-util = "0.3"` to Cargo.toml
- **Files modified:** Cargo.toml
- **Verification:** cargo check passes
- **Committed in:** 61d8cfa (Task 1 commit)

**2. [Rule 3 - Blocking] Adapted to bollard 0.20 API changes**
- **Found during:** Task 1 (Image pull implementation)
- **Issue:** Plan referenced `bollard::image::CreateImageOptions` but bollard 0.20 moved it to `bollard::query_parameters::CreateImageOptionsBuilder` with builder pattern
- **Fix:** Used `CreateImageOptionsBuilder::default().from_image(image).build()` instead of struct literal
- **Files modified:** src/scheduler/docker_pull.rs
- **Verification:** cargo test passes
- **Committed in:** 61d8cfa (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both auto-fixes necessary to compile against actual bollard 0.20 API. No scope creep.

## Issues Encountered
None beyond the dependency/API deviations documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- docker_pull.rs ready for use by Docker executor (Plan 01) via ensure_image()
- docker_preflight.rs ready for use by Docker executor via preflight_network()
- Both modules are standalone with no scheduler loop integration yet (that happens in Plan 01)

---
*Phase: 04-docker-executor-container-network-differentiator*
*Completed: 2026-04-11*
