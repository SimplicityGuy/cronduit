# Phase 4: Docker Executor & container-network Differentiator - Research

**Researched:** 2026-04-10
**Domain:** Docker container lifecycle management via bollard, network mode differentiation, orphan reconciliation
**Confidence:** HIGH

## Summary

Phase 4 implements the headline feature of Cronduit: ephemeral Docker container execution via the `bollard` crate. The existing codebase has a clean executor pattern (`ExecResult`/`RunStatus` in `command.rs`, `LogSender` channel in `log_pipeline.rs`) and a dispatch point in `run.rs` (lines 133-139) with a placeholder that returns an error. The `container_id` column already exists in the `job_runs` schema but has no query writing to it yet.

The core challenge is the container lifecycle state machine: `Creating -> Starting -> Running -> Exited -> LogsDrained -> Removed`. The critical ordering constraint is that `wait_container` must resolve and exit code must be persisted to the DB BEFORE `remove_container` is called. The `auto_remove=false` setting on bollard's `HostConfig` prevents Docker from racing against our log drain. All five network modes are supported by bollard's `HostConfig.network_mode` field as a plain `Option<String>`.

**Primary recommendation:** Create a `src/scheduler/docker.rs` module that follows the exact same `ExecResult` return pattern as `command.rs`, with a state-machine approach to container lifecycle. Wire it into the existing `run.rs` dispatch. Add orphan reconciliation as a startup step in `src/scheduler/mod.rs` before entering the main loop.

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Verbose pull retry logging -- each retry attempt logged with reason + backoff duration
- **D-02:** Fail fast on terminal errors -- classify pull failures: network/timeout retry with exponential backoff (3 attempts), unauthorized/manifest-unknown fail immediately
- **D-03:** Silent on successful pull -- only log resolved digest on success
- **D-04:** Graceful stop on timeout -- send SIGTERM via bollard `stop_container` with 10s grace period, then force-kill
- **D-05:** Drain logs until EOF after exit -- after `wait_container` returns, continue reading log stream until bollard closes it naturally
- **D-06:** Stop containers on cronduit shutdown -- during graceful shutdown, send `stop_container` to all in-flight Docker jobs
- **D-07:** Kill and mark error for live orphans -- on startup, stop + remove still-running cronduit-labeled containers, mark DB rows as error
- **D-08:** Remove stopped orphans too -- also remove stopped cronduit-labeled containers and update DB rows
- **D-09:** Log each orphan individually -- WARN-level log per reconciled orphan
- **D-10:** Strict container:\<name\> validation -- inspect target container and verify running state
- **D-11:** Pre-flight named networks too -- call bollard `inspect_network` before creating container for named networks
- **D-12:** Distinct error categories -- separate `docker_unavailable` from `network_target_unavailable` from `network_not_found`

### Claude's Discretion

No areas deferred -- all gray areas were discussed and decided.

### Deferred Ideas (OUT OF SCOPE)

None.

</user_constraints>

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DOCKER-01 | Docker jobs via `image = "..."` spawn ephemeral containers via bollard | bollard 0.20.2 `create_container` + `start_container` API verified; `ContainerCreateBody` has `image` field |
| DOCKER-02 | All five network modes supported (bridge, host, none, container:\<name\>, named) | `HostConfig.network_mode: Option<String>` accepts all Docker standard values directly |
| DOCKER-03 | Pre-flight check for container:\<name\> -- structured error on failure | `inspect_container` API returns `ContainerInspectResponse` with `State.status`; `inspect_network` for named networks |
| DOCKER-04 | Volume mounts, env vars, container_name, per-job timeout honored | `HostConfig.binds`, `ContainerCreateBody.env`, `CreateContainerOptions.name`, existing timeout pattern in `run.rs` |
| DOCKER-05 | Image auto-pull with retry + error classification | `create_image` returns `Stream<Item=Result<CreateImageInfo, Error>>` -- classify errors from stream |
| DOCKER-06 | auto_remove=false, explicit remove after wait + log drain | `HostConfig.auto_remove: Some(false)`, `wait_container` stream, `remove_container` after finalize |
| DOCKER-07 | Labels: `cronduit.run_id` and `cronduit.job_name` on every container | `ContainerCreateBody.labels: Option<HashMap<String, String>>` |
| DOCKER-08 | Stream logs via `bollard.logs(follow=true)` into existing log pipeline | `LogOutput` enum has `StdOut { message: Bytes }` and `StdErr { message: Bytes }` variants |
| DOCKER-09 | Record image digest in `job_runs.container_id` | `inspect_container` response includes `Image` (digest) field; `container_id` column pre-allocated in schema |
| DOCKER-10 | Integration test for container:\<name\> via testcontainers | testcontainers 0.27 `GenericImage` + `AsyncRunner` -- start target container, get container name, test executor |
| SCHED-08 | Orphan reconciliation on startup via cronduit labels | `list_containers` with label filter `cronduit.run_id`, cross-reference with `job_runs` table |

</phase_requirements>

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| bollard | 0.20.2 | Docker API client | Locked by project decision. Async, tokio-native, covers all Docker API endpoints. Latest release 2026-03-15. [VERIFIED: crates.io API] |
| tokio | 1.51 | Async runtime | Already in Cargo.toml; bollard requires it [VERIFIED: existing Cargo.toml] |
| futures-util | (via bollard) | Stream combinators | `StreamExt` for consuming `wait_container` and `logs` streams. Transitive dep of bollard. [VERIFIED: cargo tree] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| testcontainers | 0.27.2 | Integration test containers | Already in dev-dependencies; used for Docker executor integration tests [VERIFIED: Cargo.toml] |
| bytes | (via bollard) | Zero-copy byte buffers | `LogOutput` variants contain `Bytes`; use for log extraction [VERIFIED: bollard docs] |

### No New Dependencies Required

Bollard needs to be added to `[dependencies]` in Cargo.toml. It is currently only a transitive dev-dependency via testcontainers. All other needed crates are already present.

**Installation:**
```bash
# Add to Cargo.toml [dependencies]:
bollard = { version = "0.20", default-features = false, features = ["ssl", "json"] }
```

Note: Use `default-features = false` and select features explicitly to avoid pulling openssl. Verify with `cargo tree -i openssl-sys` after adding. The `ssl` feature in bollard 0.20 uses rustls when `openssl` is not explicitly enabled. [ASSUMED -- verify during implementation that bollard's `ssl` feature does not pull openssl-sys]

## Architecture Patterns

### Recommended Module Structure

```
src/scheduler/
  mod.rs              # Add orphan reconciliation call before main loop
  docker.rs           # NEW: Docker executor (core of Phase 4)
  docker_pull.rs      # NEW: Image pull with retry + classification
  docker_preflight.rs # NEW: Network pre-flight checks
  docker_orphan.rs    # NEW: Orphan reconciliation logic
  run.rs              # Wire "docker" arm to docker::execute_docker()
  command.rs          # Unchanged (ExecResult/RunStatus shared)
  log_pipeline.rs     # Unchanged (LogSender/LogReceiver reused)
```

### Pattern 1: Container Lifecycle State Machine

**What:** A disciplined ordering of Docker API calls to avoid the `auto_remove` race (moby/moby#8441).

**When to use:** Every Docker job execution.

```rust
// Source: Derived from bollard docs + moby#8441 analysis
async fn execute_docker(
    docker: &Docker,
    config: DockerJobConfig,
    run_id: i64,
    cancel: CancellationToken,
    sender: LogSender,
) -> ExecResult {
    // 1. Pre-flight: validate network mode
    //    - container:<name> -> inspect target, verify running
    //    - named network -> inspect_network, verify exists
    //    - bridge/host/none -> no pre-flight needed
    
    // 2. Pull image (if not local)
    //    - create_image stream with retry + classification
    //    - Record resolved digest
    
    // 3. Create container
    //    - auto_remove: false (CRITICAL)
    //    - labels: cronduit.run_id, cronduit.job_name
    //    - network_mode, binds, env, container_name
    
    // 4. Start container
    
    // 5. Concurrently:
    //    a. Stream logs (follow=true) -> LogSender
    //    b. wait_container -> get exit code
    //    c. Listen for cancel token (timeout/shutdown)
    
    // 6. After wait resolves:
    //    - Continue reading log stream until EOF (D-05)
    //    - Persist exit code + container_id to DB
    
    // 7. Remove container (explicit, after all state captured)
}
```

### Pattern 2: Image Pull with Retry and Error Classification

**What:** Exponential backoff for transient failures, immediate fail for terminal errors.

**When to use:** Before creating every Docker container.

```rust
// Source: D-01, D-02, D-03 from CONTEXT.md
enum PullError {
    /// Retryable: network timeout, connection refused, temporary server error
    Transient(String),
    /// Terminal: manifest unknown, unauthorized, invalid reference
    Terminal(String),
}

async fn pull_image_with_retry(
    docker: &Docker,
    image: &str,
    max_attempts: u32,  // 3
) -> Result<String, PullError> {
    // Parse image reference (name:tag or name@digest)
    // Attempt create_image stream
    // Consume stream to completion
    // On error: classify as Transient or Terminal
    // Transient: retry with 1s, 2s, 4s backoff
    // Terminal: return immediately
    // On success: extract digest from stream, return it
}
```

### Pattern 3: Pre-flight Network Validation

**What:** Inspect target containers/networks before creating the job container.

**When to use:** For `container:<name>` and named network modes (D-10, D-11).

```rust
// Source: D-10, D-11, D-12 from CONTEXT.md
enum PreflightError {
    DockerUnavailable(String),
    NetworkTargetUnavailable(String),
    NetworkNotFound(String),
}

async fn preflight_network(
    docker: &Docker,
    network_mode: &str,
) -> Result<(), PreflightError> {
    if network_mode.starts_with("container:") {
        let target = &network_mode["container:".len()..];
        let info = docker.inspect_container(target, None).await
            .map_err(classify_docker_error)?;
        let status = info.state
            .and_then(|s| s.status)
            .unwrap_or_default();
        if status != "running" {
            return Err(PreflightError::NetworkTargetUnavailable(
                target.to_string()
            ));
        }
    } else if !matches!(network_mode, "bridge" | "host" | "none" | "") {
        // Named network
        docker.inspect_network::<String>(network_mode, None).await
            .map_err(|_| PreflightError::NetworkNotFound(
                network_mode.to_string()
            ))?;
    }
    Ok(())
}
```

### Pattern 4: Orphan Reconciliation at Startup

**What:** Find and clean up containers from previous Cronduit runs.

**When to use:** Once, before entering the scheduler main loop (SCHED-08).

```rust
// Source: D-07, D-08, D-09 from CONTEXT.md
async fn reconcile_orphans(
    docker: &Docker,
    pool: &DbPool,
) -> anyhow::Result<u32> {
    // 1. list_containers with filter label=cronduit.run_id
    //    (include stopped containers)
    // 2. For each container:
    //    a. Extract run_id from label
    //    b. Check DB: is this run_id in status='running'?
    //    c. If container is running: stop_container + remove_container
    //    d. If container is stopped: remove_container
    //    e. Update DB row: status='error', error_message='orphaned at restart'
    //    f. WARN log per orphan (D-09)
    // 3. Return count of reconciled orphans
}
```

### Pattern 5: Log Streaming from Docker

**What:** Convert bollard's `LogOutput` stream into `LogSender` messages.

**When to use:** During container execution, concurrent with `wait_container`.

```rust
// Source: bollard docs + existing log_pipeline pattern
use bollard::container::LogOutput;
use futures_util::StreamExt;

async fn stream_docker_logs(
    docker: &Docker,
    container_id: &str,
    sender: LogSender,
) {
    let options = LogsOptions::<String> {
        follow: Some(true),
        stdout: Some(true),
        stderr: Some(true),
        timestamps: Some(true),
        ..Default::default()
    };
    
    let mut stream = docker.logs(container_id, Some(options));
    while let Some(result) = stream.next().await {
        match result {
            Ok(LogOutput::StdOut { message }) => {
                let text = String::from_utf8_lossy(&message);
                for line in text.lines() {
                    sender.send(make_log_line("stdout", line.to_string()));
                }
            }
            Ok(LogOutput::StdErr { message }) => {
                let text = String::from_utf8_lossy(&message);
                for line in text.lines() {
                    sender.send(make_log_line("stderr", line.to_string()));
                }
            }
            Ok(_) => {} // StdIn, Console -- ignore
            Err(e) => {
                sender.send(make_log_line("system",
                    format!("[docker log error: {e}]")));
                break;
            }
        }
    }
}
```

### Anti-Patterns to Avoid

- **Setting `auto_remove: true`:** Docker will remove the container before you can read logs or exit code. This is the moby#8441 race. Always use `auto_remove: false` and remove explicitly.
- **Using `docker.logs()` without `follow: true`:** You'll miss log lines emitted after the initial fetch. Always follow, then let EOF signal completion.
- **Calling `remove_container` before `finalize_run`:** If remove fails or crashes, you lose the exit code. Always persist state to DB first, then remove.
- **Blocking on `wait_container` without concurrent log streaming:** Logs should stream in real-time, not be fetched after exit. Use `tokio::select!` or `tokio::join!` to run both concurrently.
- **Parsing `container:<name>` without pre-flight:** Docker's raw error when the target isn't running is cryptic. Always inspect first and return a structured error.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Docker API communication | HTTP client for Docker socket | bollard 0.20.2 | Handles Unix socket, TLS, version negotiation, all API endpoints |
| Container log stream parsing | Manual multiplexed stream parser | bollard `LogOutput` enum | Docker's stream multiplexing protocol is non-trivial |
| Image reference parsing | Regex for `name:tag@digest` | bollard `CreateImageOptions` | Handles registry defaults, tag defaults, digest references |
| Exponential backoff | Manual sleep loop | Simple helper with `tokio::time::sleep` | Only 3 attempts; a 10-line helper is fine. Don't pull a backoff crate for this. |

**Key insight:** bollard abstracts the Docker Engine API completely. The executor code should be pure bollard API calls + business logic (pre-flight, retry, error classification). No raw HTTP or socket communication needed.

## Common Pitfalls

### Pitfall 1: auto_remove Race (Pitfall 3 from PITFALLS.md)

**What goes wrong:** Setting `auto_remove=true` causes Docker to remove the container immediately on exit. If `wait_container` hasn't returned yet, or if you need to read remaining logs, the container is gone and you get a 404.
**Why it happens:** Docker's `auto_remove` triggers removal from the daemon side, racing with the client-side `wait` and `logs` calls.
**How to avoid:** Always set `HostConfig.auto_remove = Some(false)`. Call `remove_container` explicitly after: (1) `wait_container` resolves, (2) logs are drained to EOF, (3) exit code + container_id persisted to DB.
**Warning signs:** Intermittent "container not found" errors on fast-exiting containers; missing exit codes recorded as `None`.

### Pitfall 2: container:\<name\> Silent Break (Pitfall 2 from PITFALLS.md)

**What goes wrong:** A `container:<name>` job silently fails with a cryptic Docker error when the target container isn't running.
**Why it happens:** Docker returns a generic "container not found" or "network not available" error rather than a clear message about the target container's state.
**How to avoid:** Pre-flight `inspect_container` on the target, verify `state.status == "running"`. Return structured `network_target_unavailable: <name>` error.
**Warning signs:** Docker jobs failing with raw bollard errors in the `error_message` column.

### Pitfall 3: Log Stream Incomplete on Fast Exit

**What goes wrong:** A container that exits in <50ms may have its logs truncated because the log stream hasn't fully flushed.
**Why it happens:** Docker's log multiplexer may have buffered output that hasn't been sent to the client stream yet when the container exits.
**How to avoid:** After `wait_container` resolves, do NOT close the log stream. Continue reading until the stream returns `None` (EOF). This is decision D-05: drain logs until EOF.
**Warning signs:** Missing last lines of output from short-lived containers.

### Pitfall 4: bollard TLS Feature Pulling OpenSSL

**What goes wrong:** Adding bollard with default features may pull `openssl-sys`, violating FOUND-06.
**Why it happens:** bollard's `ssl` feature can resolve to either rustls or native-tls depending on feature flags.
**How to avoid:** Use `default-features = false` and explicitly select features. After adding, run `cargo tree -i openssl-sys` to verify empty output.
**Warning signs:** CI failure on the openssl-sys check.

### Pitfall 5: Orphan Containers Leaking Across Restarts (Pitfall 10 from PITFALLS.md)

**What goes wrong:** If Cronduit crashes or is killed (SIGKILL), containers it spawned keep running. On restart, those containers are invisible to the new process.
**Why it happens:** No tracking mechanism survives process death.
**How to avoid:** Label every container with `cronduit.run_id` and `cronduit.job_name`. On startup, `list_containers` with label filter, reconcile against DB, stop+remove any orphans, mark DB rows as error.
**Warning signs:** Resource leaks (containers accumulating), DB rows stuck in `status='running'` forever.

### Pitfall 6: wait_container Returns a Stream, Not a Future

**What goes wrong:** Treating `wait_container` as a simple async function that returns one result.
**Why it happens:** bollard's `wait_container` returns `impl Stream<Item=Result<ContainerWaitResponse, Error>>` because Docker's wait API can emit multiple status updates.
**How to avoid:** Use `StreamExt::next()` to get the first (and typically only) result from the stream. The stream will yield one `ContainerWaitResponse` with `status_code` when the container exits.
**Warning signs:** Compilation errors about `Stream` not implementing `Future`.

## Code Examples

### Creating a Container with bollard

```rust
// Source: bollard docs.rs + HostConfig docs
use bollard::container::{
    Config as ContainerConfig, CreateContainerOptions,
};
use bollard::models::HostConfig;
use std::collections::HashMap;

let mut labels = HashMap::new();
labels.insert("cronduit.run_id".to_string(), run_id.to_string());
labels.insert("cronduit.job_name".to_string(), job_name.to_string());

let host_config = HostConfig {
    network_mode: Some(network_mode.to_string()),
    binds: Some(volumes.clone()),
    auto_remove: Some(false), // CRITICAL: never auto_remove
    ..Default::default()
};

let config = ContainerConfig {
    image: Some(image.to_string()),
    env: Some(env_vars),
    labels: Some(labels),
    host_config: Some(host_config),
    ..Default::default()
};

let options = container_name.as_ref().map(|name| {
    CreateContainerOptions {
        name: name.as_str(),
        platform: None,
    }
});

let response = docker.create_container(options, config).await?;
let container_id = response.id;
```

### Concurrent Wait + Log Stream

```rust
// Source: bollard docs + D-05 decision
use futures_util::StreamExt;
use tokio::select;

// Start log streaming task
let log_handle = tokio::spawn(stream_docker_logs(
    docker.clone(), container_id.clone(), sender.clone()
));

// Wait for container exit with timeout/cancel
let wait_result = select! {
    result = docker.wait_container::<String>(&container_id, None)
        .next() => {
        match result {
            Some(Ok(response)) => {
                let code = response.status_code;
                Ok(code)
            }
            Some(Err(e)) => Err(e),
            None => Err(/* unexpected stream end */),
        }
    }
    _ = tokio::time::sleep(timeout) => {
        // Timeout: stop container gracefully (D-04)
        docker.stop_container(&container_id, Some(StopContainerOptions {
            t: 10, // 10s grace period
        })).await.ok();
        Err(/* timeout */)
    }
    _ = cancel.cancelled() => {
        // Shutdown: stop container (D-06)
        docker.stop_container(&container_id, Some(StopContainerOptions {
            t: 10,
        })).await.ok();
        Err(/* shutdown */)
    }
};

// D-05: Wait for log stream to complete (EOF)
let _ = log_handle.await;
sender.close();
```

### Orphan Reconciliation

```rust
// Source: D-07, D-08, D-09, SCHED-08
use bollard::container::ListContainersOptions;

let mut filters = HashMap::new();
filters.insert("label", vec!["cronduit.run_id"]);

let options = ListContainersOptions {
    all: Some(true), // Include stopped containers (D-08)
    filters,
    ..Default::default()
};

let containers = docker.list_containers(Some(options)).await?;
for container in containers {
    let labels = container.labels.unwrap_or_default();
    let run_id = labels.get("cronduit.run_id");
    let job_name = labels.get("cronduit.job_name");
    let container_id = container.id.unwrap_or_default();
    
    // Stop if running
    if container.state.as_deref() == Some("running") {
        docker.stop_container(&container_id, Some(StopContainerOptions {
            t: 10,
        })).await.ok();
    }
    
    // Remove container
    docker.remove_container(&container_id, None).await.ok();
    
    // Update DB row
    if let Some(rid) = run_id {
        if let Ok(rid) = rid.parse::<i64>() {
            mark_run_orphaned(pool, rid).await.ok();
        }
    }
    
    tracing::warn!(
        target: "cronduit.reconcile",
        container_id = %container_id,
        job_name = ?job_name,
        run_id = ?run_id,
        "Reconciled orphan container"
    );
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| bollard 0.17-0.18 (hyper 0.14) | bollard 0.20 (hyper 1.x) | March 2026 | Don't mix with old hyper middleware [VERIFIED: crates.io] |
| `shiplift` Docker client | bollard | 2023+ | shiplift is unmaintained; bollard is the standard [VERIFIED: project decision] |
| `auto_remove=true` pattern | Explicit remove after state capture | Always | moby#8441 race; project explicitly bans auto_remove |

**Deprecated/outdated:**
- **shiplift:** Unmaintained Docker client for Rust. Do not use.
- **bollard < 0.18:** Used hyper 0.14; incompatible with the project's hyper 1.x stack.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | bollard's `ssl` feature with `default-features = false` does not pull openssl-sys | Standard Stack | CI will fail on FOUND-06 check; fix by adjusting feature flags |
| A2 | `ContainerWaitResponse.status_code` is i64 matching Docker's exit code | Code Examples | May need type conversion; low risk |
| A3 | bollard's `LogOutput` chunks may split mid-line for long output | Pitfall 3 | Log lines may be split across chunks; need line reassembly or chunk-based storage per DOCKER-08 |
| A4 | `inspect_container` returns `State.status` as a string like "running", "exited", etc. | Pattern 3 | Field might be an enum; adjust match accordingly |

## Open Questions

1. **bollard feature flags for rustls-only**
   - What we know: bollard 0.20 supports both rustls and native-tls. The project requires rustls-only (FOUND-06).
   - What's unclear: Exact feature flag combination to avoid openssl-sys.
   - Recommendation: Test `default-features = false` + verify `cargo tree -i openssl-sys` during first implementation task.

2. **Docker log chunk boundaries**
   - What we know: Docker's multiplexed stream sends chunks, not lines. bollard's `LogOutput.message` is `Bytes`.
   - What's unclear: Whether chunks always end at line boundaries for short output.
   - Recommendation: Per DOCKER-08, use chunk-based storage. Split on newlines for the log pipeline but handle incomplete lines at chunk boundaries.

3. **finalize_run needs container_id parameter**
   - What we know: Current `finalize_run` does not accept `container_id`. The column exists in schema.
   - What's unclear: Whether to modify `finalize_run` or create a separate `set_container_id` query.
   - Recommendation: Add `container_id: Option<&str>` parameter to `finalize_run` and include it in the UPDATE statement. This is backward-compatible since command/script callers pass `None`.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Docker daemon | Container execution + integration tests | Yes | 29.1.4-rd (Rancher Desktop) | -- |
| Rust toolchain | Compilation | Yes | 1.94.1 | -- |
| Docker socket | bollard connection | Yes (Rancher Desktop) | -- | -- |

**Missing dependencies with no fallback:** None.

**Missing dependencies with fallback:** None.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test + cargo-nextest |
| Config file | Cargo.toml (existing test setup) |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo nextest run --all-targets` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DOCKER-01 | Docker job spawns ephemeral container via bollard | integration | `cargo test --test docker_executor -- docker_job_runs_container` | No -- Wave 0 |
| DOCKER-02 | All 5 network modes exercised | integration | `cargo test --test docker_executor -- network_modes` | No -- Wave 0 |
| DOCKER-03 | container:\<name\> pre-flight structured error | unit + integration | `cargo test --lib docker_preflight` | No -- Wave 0 |
| DOCKER-04 | Volumes, env, container_name, timeout honored | integration | `cargo test --test docker_executor -- config_fields` | No -- Wave 0 |
| DOCKER-05 | Image pull retry + error classification | unit | `cargo test --lib docker_pull` | No -- Wave 0 |
| DOCKER-06 | auto_remove=false, explicit remove lifecycle | integration | `cargo test --test docker_executor -- explicit_remove` | No -- Wave 0 |
| DOCKER-07 | Container labels set correctly | integration | `cargo test --test docker_executor -- labels` | No -- Wave 0 |
| DOCKER-08 | Log streaming into pipeline | integration | `cargo test --test docker_executor -- log_streaming` | No -- Wave 0 |
| DOCKER-09 | Image digest recorded in container_id column | integration | `cargo test --test docker_executor -- digest_recorded` | No -- Wave 0 |
| DOCKER-10 | container:\<name\> end-to-end with testcontainers | integration | `cargo test --test docker_container_network` | No -- Wave 0 |
| SCHED-08 | Orphan reconciliation on startup | integration | `cargo test --test docker_executor -- orphan_reconciliation` | No -- Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test --lib`
- **Per wave merge:** `cargo nextest run --all-targets`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `tests/docker_executor.rs` -- integration tests for DOCKER-01 through DOCKER-09, SCHED-08
- [ ] `tests/docker_container_network.rs` -- marquee integration test for DOCKER-10
- [ ] Unit tests in `src/scheduler/docker_pull.rs` -- pull retry logic
- [ ] Unit tests in `src/scheduler/docker_preflight.rs` -- network validation logic

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | N/A (Docker socket access is host-level) |
| V3 Session Management | No | N/A |
| V4 Access Control | Yes (Docker socket) | Docker socket mounted read-only is not possible; document threat in THREAT_MODEL.md |
| V5 Input Validation | Yes | Validate network mode strings, image references, volume mount paths before passing to bollard |
| V6 Cryptography | No | N/A (no crypto in this phase) |

### Known Threat Patterns for Docker Socket Access

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Docker socket access = root equivalent | Elevation of Privilege | Document in THREAT_MODEL.md; bind 127.0.0.1 default |
| Volume mount path traversal | Tampering | Config file is operator-authored and mounted read-only; no user input for volumes |
| Env var secrets in container labels | Information Disclosure | Labels contain only run_id and job_name, never secrets |
| Image pull from untrusted registry | Tampering | Operator responsibility; document in README |

## Sources

### Primary (HIGH confidence)
- [bollard 0.20.2 on crates.io](https://crates.io/crates/bollard) -- version 0.20.2, published 2026-03-15 [VERIFIED: crates.io API]
- [bollard docs.rs](https://docs.rs/bollard/latest/bollard/struct.Docker.html) -- API function signatures for create_container, wait_container, logs, etc. [VERIFIED: docs.rs]
- [bollard HostConfig docs](https://docs.rs/bollard/latest/bollard/models/struct.HostConfig.html) -- network_mode, binds, auto_remove field types [VERIFIED: docs.rs]
- [bollard container.rs source](https://github.com/fussybeaver/bollard/blob/master/src/container.rs) -- LogOutput enum definition [VERIFIED: GitHub]
- [moby/moby#8441](https://github.com/moby/moby/issues/8441) -- auto_remove race condition with wait [VERIFIED: GitHub issue]
- Existing codebase: `src/scheduler/command.rs`, `run.rs`, `log_pipeline.rs`, `mod.rs`, `config/mod.rs`, `db/queries.rs` -- executor patterns, DB schema [VERIFIED: codebase read]

### Secondary (MEDIUM confidence)
- [testcontainers-rs docs](https://docs.rs/testcontainers/latest/testcontainers/) -- GenericImage, AsyncRunner patterns [VERIFIED: docs.rs]

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- bollard version verified on crates.io, API verified on docs.rs
- Architecture: HIGH -- patterns derived from existing codebase patterns + bollard API + locked decisions
- Pitfalls: HIGH -- moby#8441 verified, all pitfalls referenced from project's PITFALLS.md
- Integration points: HIGH -- exact file locations and line numbers verified in codebase

**Research date:** 2026-04-10
**Valid until:** 2026-05-10 (bollard 0.20.x is stable; no breaking changes expected)
