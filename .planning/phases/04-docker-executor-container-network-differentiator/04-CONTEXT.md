# Phase 4: Docker Executor & container-network Differentiator - Context

**Gathered:** 2026-04-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Deliver ephemeral Docker container execution via bollard: spawn containers from `image = "..."` config, support all five network modes (bridge, host, none, named network, container:\<name\>), stream logs into the existing pipeline, capture exit codes reliably with auto_remove=false, implement image auto-pull with retry and error classification, label every container for orphan tracking, and reconcile orphans at startup. The existing command/script executor paths must remain untouched.

</domain>

<decisions>
## Implementation Decisions

### Image Pull Behavior
- **D-01:** Verbose pull retry logging — each retry attempt logged with reason + backoff duration (e.g., `[pull] attempt 2/3 failed: connection timeout, retrying in 4s`)
- **D-02:** Fail fast on terminal errors — classify pull failures: network/timeout errors trigger retry with exponential backoff (3 attempts), unauthorized/manifest-unknown fail immediately with structured error message
- **D-03:** Silent on successful pull — only log the resolved digest on success (`[pull] alpine:latest resolved to sha256:abc123`). No layer-by-layer progress for successful pulls.

### Container Lifecycle
- **D-04:** Graceful stop on timeout — send SIGTERM via bollard `stop_container` with 10s grace period, then force-kill if still running. Matches Docker CLI behavior and lets containers clean up.
- **D-05:** Drain logs until EOF after exit — after `wait_container` returns, continue reading the log stream until bollard closes it naturally (EOF). No arbitrary timeout on log drain.
- **D-06:** Stop containers on cronduit shutdown — during graceful shutdown (SIGINT/SIGTERM), send `stop_container` to all in-flight Docker jobs. Prevents orphans. Consistent with existing command executor killing child processes.

### Orphan Reconciliation
- **D-07:** Kill and mark error for live orphans — on startup, stop + remove any still-running cronduit-labeled container, mark the DB row as `status='error'`, `error_message='orphaned at restart'`. Clean slate on every boot.
- **D-08:** Remove stopped orphans too — also remove stopped (exited) cronduit-labeled containers and update DB rows if needed. Full cleanup so no stale containers accumulate across restarts.
- **D-09:** Log each orphan individually — WARN-level log per reconciled orphan: `Reconciled orphan container abc123 (job: backup, run: xyz)`. Not just a summary count.

### Network Pre-flight
- **D-10:** Strict container:\<name\> validation — inspect target container and verify it's in `running` state. If stopped/paused/dead, fail with `error_message='network_target_unavailable: <name>'`.
- **D-11:** Pre-flight named networks too — call bollard `inspect_network` before creating the container for named network mode. Fail with `network_not_found: <netname>` rather than a raw Docker error. Consistent with container:\<name\> pre-flight.
- **D-12:** Distinct error categories for pre-flight — separate `docker_unavailable` (socket unreachable) from `network_target_unavailable` (target container down) from `network_not_found` (named network missing). Operators and metrics can distinguish infrastructure problems from configuration problems.

### Claude's Discretion
No areas deferred to Claude's discretion — all gray areas were discussed and decided.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Docker Executor Requirements
- `.planning/REQUIREMENTS.md` — DOCKER-01 through DOCKER-10, SCHED-08 acceptance criteria
- `.planning/ROADMAP.md` §Phase 4 — success criteria, pitfalls addressed, dependency chain

### Existing Executor Pattern
- `src/scheduler/command.rs` — ExecResult/RunStatus pattern that Docker executor must follow
- `src/scheduler/script.rs` — Script execution pattern (tempfile + shebang)
- `src/scheduler/run.rs` — Job dispatch logic (lines 82-149), Phase 4 placeholder at lines 133-139
- `src/scheduler/log_pipeline.rs` — Bounded channel (256 lines), micro-batch writer, head-drop backpressure

### Database Schema
- `migrations/sqlite/20260410_000000_initial.up.sql` — job_runs table with container_id column pre-allocated
- `src/db/queries.rs` — Existing job/run read/write queries

### Configuration
- `src/config/mod.rs` — JobConfig with image, network, volumes, container_name, env, timeout fields

### Scheduler Integration
- `src/scheduler/mod.rs` — Main tokio::select! loop, CancellationToken per-job, JoinSet reaping, graceful shutdown

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ExecResult` struct (exit_code, status, error_message) — Docker executor returns this same type
- `LogSender` / bounded channel — Docker log stream feeds into same pipeline as command/script
- `CancellationToken` per-job — Docker executor listens on child token for timeout/shutdown
- `container_id` column in job_runs — pre-allocated for storing image digest

### Established Patterns
- Job dispatch in `run.rs` matches on `job_type` field — add `"docker"` arm calling new `docker.rs` module
- Error messages are structured strings stored in `error_message` column — Docker pre-flight errors follow same pattern
- Split SQLite pools (single writer, multi reader) — log writer task uses writer pool exclusively

### Integration Points
- `src/scheduler/run.rs` line ~133 — Replace Phase 4 placeholder with Docker executor call
- `src/scheduler/mod.rs` — Add orphan reconciliation before entering main loop
- `src/scheduler/mod.rs` — Extend shutdown path to stop Docker containers via bollard
- `Cargo.toml` — Add bollard dependency (not yet present)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches guided by the decisions above.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 04-docker-executor-container-network-differentiator*
*Context gathered: 2026-04-10*
