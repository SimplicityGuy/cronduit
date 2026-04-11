---
phase: 04-docker-executor-container-network-differentiator
verified: 2026-04-11T20:00:00Z
status: human_needed
score: 6/6 must-haves verified
overrides_applied: 0
human_verification:
  - test: "Run `cargo test --test docker_container_network -- --ignored --nocapture` against a live Docker daemon"
    expected: "test_container_network_mode PASSES — target container starts, Cronduit job joins its network namespace, exits 0, and 'network-ok' appears in captured logs"
    why_human: "Requires a running Docker daemon; cannot execute in static analysis. The marquee DOCKER-10 test is the primary differentiator claim."
  - test: "Run `cargo test --test docker_executor -- --ignored --nocapture` against a live Docker daemon"
    expected: "All 5 executor tests pass: basic echo (Success, exit 0), timeout (Timeout status + 'timed out' message), preflight nonexistent (NetworkTargetUnavailable), orphan reconciliation (container removed + DB row error='orphaned at restart'), preflight failure through execute_docker (Error + 'network_target_unavailable' in message)"
    why_human: "Requires a running Docker daemon with network access to pull alpine:latest."
---

# Phase 4: Docker Executor & container-network Differentiator Verification Report

**Phase Goal:** The headline feature: ephemeral Docker container jobs via bollard with full support for every network mode including container:<name>, structured pre-flight failures, correct wait_container + explicit-remove sequencing, image auto-pull with retry, per-container labeling, and startup orphan reconciliation — validated by a testcontainers integration test of the container:<name> path.
**Verified:** 2026-04-11T20:00:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (Roadmap Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | docker-type job spawns ephemeral container via bollard with auto_remove=false, concurrent log streaming, exit code persisted before remove, image digest recorded | VERIFIED | `src/scheduler/docker.rs`: `auto_remove: Some(false)`, `tokio::select!` with `wait_container` + `stream_docker_logs` concurrent spawn, `cleanup_container` called AFTER select block, `container_id: Some(image_digest)` returned |
| 2 | All five network modes exercised; container:<name> test uses testcontainers | VERIFIED (human needed at runtime) | `src/scheduler/docker_preflight.rs`: handles `container:`, bridge/host/none/empty, named networks. `tests/docker_container_network.rs` exists with `test_container_network_mode` using `testcontainers::GenericImage`. Tests compile and list correctly. |
| 3 | container:<name> job when target not running records error_message='network_target_unavailable: <name>' | VERIFIED | `src/scheduler/docker_preflight.rs`: `Err(PreflightError::NetworkTargetUnavailable(target))` for non-running/missing containers. `docker.rs` calls `preflight_network` and returns `error_message: Some(err_msg)` where `err_msg = "network_target_unavailable: <name>"`. Test `test_container_network_target_stopped` covers this path. |
| 4 | Image auto-pull retries with exponential backoff (3 attempts), distinguishes terminal from transient failures | VERIFIED | `src/scheduler/docker_pull.rs`: loop for `max_attempts=3`, backoffs `[1,2,4]`, `PullError::Terminal` fails fast (D-02), `PullError::Transient` retries with `tracing::warn!` logging attempt/reason/backoff_secs (D-01). `ensure_image` calls `pull_image_with_retry`. Unit tests for all four classification cases pass. |
| 5 | Every container labeled cronduit.run_id and cronduit.job_name; volumes, env, container_name, timeout honored | VERIFIED | `src/scheduler/docker.rs`: labels HashMap built with `cronduit.run_id` and `cronduit.job_name`. `HostConfig` receives `network_mode`, `binds` (volumes), `auto_remove`. `ContainerCreateBody` receives `image`, `cmd`, `env`, `labels`. `CreateContainerOptions` uses `container_name`. Timeout flows through `tokio::time::sleep(timeout)` select branch. |
| 6 | On startup, containers matching cronduit.run_id=* with running status reconciled to error='orphaned at restart' | VERIFIED | `src/scheduler/docker_orphan.rs`: `list_containers` with label filter `cronduit.run_id`, `all: true`. Running orphans stopped with `t: Some(10)`. All orphans removed with `force: true`. `mark_run_orphaned` updates `status='error', error_message='orphaned at restart'` with `AND status = 'running'` guard. Called in `src/cli/run.rs` before `scheduler::spawn`. |

**Score:** 6/6 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/scheduler/docker.rs` | Docker executor entry point: execute_docker() | VERIFIED | 419 lines, full lifecycle. `pub async fn execute_docker` present, `DockerJobConfig`, `DockerExecResult`, `cleanup_container`. |
| `src/scheduler/docker_log.rs` | Docker log streaming: stream_docker_logs() | VERIFIED | `pub async fn stream_docker_logs` handles `StdOut`, `StdErr`, `Console`, errors. Caller manages sender lifecycle. |
| `src/scheduler/docker_pull.rs` | pull_image_with_retry() function | VERIFIED | `pub async fn pull_image_with_retry`, `pub async fn ensure_image`, `pub enum PullError {Transient, Terminal}`, 4 unit tests. |
| `src/scheduler/docker_preflight.rs` | preflight_network() function and PreflightError enum | VERIFIED | `pub async fn preflight_network`, `pub enum PreflightError {DockerUnavailable, NetworkTargetUnavailable, NetworkNotFound}`, `to_error_message()`, 3 unit tests. |
| `src/scheduler/docker_orphan.rs` | reconcile_orphans() function | VERIFIED | `pub async fn reconcile_orphans`, `list_containers` with label filter, stop/remove/mark logic, `mark_run_orphaned` with SQLite/Postgres branches. |
| `src/scheduler/run.rs` | Docker dispatch arm calling docker::execute_docker | VERIFIED | `"docker" => match &docker { Some(client) => execute_docker(...), None => Error }`. `container_id_for_finalize` captured and passed to `finalize_run`. |
| `src/db/queries.rs` | Updated finalize_run with container_id parameter | VERIFIED | Signature `finalize_run(..., container_id: Option<&str>)`. SQLite UPDATE includes `container_id = ?6`, Postgres `container_id = $6`. |
| `tests/docker_executor.rs` | Docker executor integration tests | VERIFIED | 5 tests: basic_echo, timeout, preflight_nonexistent, orphan_reconciliation (full DB verification), preflight_failure_returns_error. All use `#[ignore]`. |
| `tests/docker_container_network.rs` | Marquee integration test: container:<name> end-to-end | VERIFIED | `test_container_network_mode` uses `testcontainers::GenericImage`, joins target network namespace, verifies exit 0 + log capture. `test_container_network_target_stopped` covers pre-flight rejection path. |
| `Cargo.toml` | bollard and testcontainers dependencies | VERIFIED | `bollard = "0.20"`, `testcontainers = "0.27.2"`. `cargo tree -i openssl-sys` returns empty — rustls constraint maintained. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/scheduler/docker.rs` | `src/scheduler/docker_preflight.rs` | `super::docker_preflight::preflight_network` before container creation | WIRED | Line 95 in docker.rs |
| `src/scheduler/docker.rs` | `src/scheduler/docker_pull.rs` | `super::docker_pull::ensure_image` before container creation | WIRED | Line 114 in docker.rs |
| `src/scheduler/docker.rs` | `src/scheduler/docker_log.rs` | `tokio::spawn(super::docker_log::stream_docker_logs(...))` | WIRED | Lines 239-246 in docker.rs |
| `src/scheduler/run.rs` | `src/scheduler/docker.rs` | `super::docker::execute_docker()` in "docker" match arm | WIRED | Lines 139-150 in run.rs |
| `src/scheduler/docker_orphan.rs` | `src/db/queries.rs` | `mark_run_orphaned` calling `sqlx::query UPDATE job_runs` | WIRED | Lines 84, 115-143 in docker_orphan.rs |
| `src/cli/run.rs` | `src/scheduler/docker_orphan.rs` | `crate::scheduler::docker_orphan::reconcile_orphans` before scheduler::spawn | WIRED | Lines 118, 135 in cli/run.rs |
| `src/scheduler/mod.rs` | `src/scheduler/run.rs` | `run_job(pool, self.docker.clone(), ...)` — docker passed in all 3 fire paths | WIRED | Lines 89, 112, 155 in mod.rs |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|-------------------|--------|
| `docker_orphan.rs` | `containers` | `docker.list_containers` with label filter | Yes — real Docker API call | FLOWING |
| `docker.rs` | `image_digest` | `docker.inspect_container` after start | Yes — real Docker API response | FLOWING |
| `db/queries.rs` | `container_id` column | `finalize_run(..., container_id_for_finalize.as_deref())` | Yes — image digest from executor | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| All 7 integration test files compile and list | `cargo test --test docker_container_network --test docker_executor -- --list` | 7 tests listed (2 + 5); exit 0 | PASS |
| All 110 unit tests pass | `cargo test --lib` | 110 passed, 0 failed, 0 ignored | PASS |
| openssl-sys not in dependency tree | `cargo tree -i openssl-sys` | No matches (package not found) | PASS |
| bollard 0.20 in Cargo.toml | grep bollard Cargo.toml | `bollard = "0.20"` present | PASS |
| Docker executor marked not available without client | Check run.rs None branch | Error returned: "docker executor unavailable (no Docker client)" | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| DOCKER-01 | 04-01 | Jobs of type docker run via bollard connecting to Docker socket | SATISFIED | `execute_docker` in docker.rs using `bollard::Docker`, called from run.rs dispatch |
| DOCKER-02 | 04-02 | All five network modes supported | SATISFIED | docker_preflight.rs handles `container:`, bridge/host/none/empty (no-op), named networks (inspect) |
| DOCKER-03 | 04-02 | container:<name> pre-flight produces error_message='network_target_unavailable: <name>' | SATISFIED | `PreflightError::NetworkTargetUnavailable` + `to_error_message()` = `"network_target_unavailable: <name>"` |
| DOCKER-04 | 04-01 | Volumes, env vars, container_name, timeout honored | SATISFIED | `HostConfig.binds`, `ContainerCreateBody.env`, `CreateContainerOptions.name`, timeout via select branch |
| DOCKER-05 | 04-02 | Image auto-pull with 3-attempt exponential backoff and error classification | SATISFIED | `pull_image_with_retry` with 1s/2s/4s backoff, Terminal vs Transient PullError |
| DOCKER-06 | 04-01 | auto_remove=false + explicit remove after wait + log drain | SATISFIED | `auto_remove: Some(false)` in HostConfig; `cleanup_container` called after select block (after log drain) |
| DOCKER-07 | 04-01 | Containers labeled cronduit.run_id and cronduit.job_name | SATISFIED | Labels HashMap built with both keys in execute_docker |
| DOCKER-08 | 04-01 | stdout/stderr streamed via bollard logs(follow=true) into job_logs pipeline | SATISFIED | `stream_docker_logs` with `follow:true, stdout:true, stderr:true`; uses same `LogSender`/`make_log_line` pipeline |
| DOCKER-09 | 04-01 | Image digest recorded in job_runs.container_id | SATISFIED | `inspect_container` after start, digest returned as `DockerExecResult.container_id`, written by `finalize_run(container_id=...)` |
| DOCKER-10 | 04-04 | testcontainers integration test for container:<name> path | SATISFIED (human at runtime) | `tests/docker_container_network.rs` with `test_container_network_mode` using `testcontainers::GenericImage`. Compiles and lists. Runtime execution requires human. |
| SCHED-08 | 04-03 | Orphan containers reconciled on startup with label filter | SATISFIED | `reconcile_orphans` in docker_orphan.rs called in cli/run.rs before scheduler::spawn |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/scheduler/docker.rs` | 114 | `let _image_digest =` — underscore prefix discards the result from `ensure_image` | Info | The digest from `ensure_image` is intentionally discarded; the actual digest for DB storage comes from `inspect_container` after container start (line 225-236). This is correct per the design: `ensure_image` is called for its pull side-effect, and `inspect_container` provides the running container's actual digest. Not a stub. |

No blockers or warnings found. The `_image_digest` discard is intentional — the image digest for recording is obtained post-start via `inspect_container`, not from the pre-start pull response.

### Human Verification Required

#### 1. Marquee container:<name> Network Mode Test (DOCKER-10)

**Test:** With Docker daemon running: `cargo test --test docker_container_network -- --ignored --nocapture`
**Expected:** Both tests pass. `test_container_network_mode` must show:
- "target container ID: ..." logged
- "result: DockerExecResult { exec: ExecResult { exit_code: Some(0), status: Success, ... } }"
- "network-ok" appears in captured log lines
- "PASSED: container:<name> network mode works end-to-end" printed
**Why human:** Requires a running Docker daemon with internet access to pull `alpine:latest`. This is the marquee differentiating feature — the primary reason Phase 4 exists.

#### 2. Full Docker Executor Test Suite

**Test:** With Docker daemon running: `cargo test --test docker_executor -- --ignored --nocapture`
**Expected:** All 5 tests pass:
- `test_docker_basic_echo`: exit_code=Some(0), status=Success, "hello-cronduit" in logs
- `test_docker_timeout_stops_container`: status=Timeout, error_message contains "timed out"
- `test_docker_preflight_nonexistent_target`: Err(NetworkTargetUnavailable("nonexistent_container_xyz_12345"))
- `test_docker_orphan_reconciliation`: count>=1, container removed (inspect 404), DB status="error", error_message="orphaned at restart"
- `test_docker_execute_preflight_failure_returns_error`: status=Error, error_message contains "network_target_unavailable"
**Why human:** Requires a running Docker daemon. The orphan reconciliation test particularly validates the SCHED-08 lifecycle end-to-end.

### Gaps Summary

No gaps found. All 6 roadmap success criteria are satisfied by substantive, wired code:

1. The executor lifecycle is correctly ordered: create → start → inspect (digest) → concurrent(wait, log-stream) → drain → remove. The `auto_remove=false` + explicit remove sequencing correctly addresses the moby#8441 race.

2. The `container:<name>` pre-flight and marquee integration test are structurally complete. The test compiles and lists; runtime execution requires a Docker daemon (human verification).

3. The three-module design (docker.rs, docker_pull.rs, docker_preflight.rs) cleanly separates concerns. All modules are registered in scheduler/mod.rs and wired into the dispatch path.

4. The `finalize_run` container_id parameter correctly carries the image digest (from `inspect_container`) through to the database, satisfying DOCKER-09.

5. Orphan reconciliation runs before the scheduler loop starts (cli/run.rs lines 118, 135), correctly isolating startup cleanup from ongoing scheduling.

---

_Verified: 2026-04-11T20:00:00Z_
_Verifier: Claude (gsd-verifier)_
