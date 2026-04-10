# Phase 2: Scheduler Core & Command/Script Executor - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-10
**Phase:** 02-scheduler-core-command-script-executor
**Areas discussed:** Scheduler loop design, Log pipeline architecture, Script execution & tempfile handling, Graceful shutdown semantics

---

## Scheduler Loop Design

### Missed Fire Handling

| Option | Description | Selected |
|--------|-------------|----------|
| Enqueue all missed | For each missed fire in skipped interval, log WARN and enqueue one catch-up run. Matches SCHED-03. | ✓ |
| Log-only, no catch-up | Log each missed fire at WARN but do NOT enqueue catch-up runs. | |
| You decide | Claude picks approach that best satisfies SCHED-03. | |

**User's choice:** Enqueue all missed
**Notes:** Matches SCHED-03 literally. Simple, predictable, matches cron tradition.

### Tick Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Sleep-to-next-fire | Compute next fire time, tokio::time::sleep_until. Minimal CPU, natural for croner's next_after() API. | ✓ |
| Fixed 1-second poll | Wake every second, check if any job should fire. Simple but wasteful. | |
| You decide | Claude picks based on croner API and tokio ergonomics. | |

**User's choice:** Sleep-to-next-fire
**Notes:** None.

### Job Sync on Startup

| Option | Description | Selected |
|--------|-------------|----------|
| Upsert on startup | After migrations, iterate config jobs and upsert into jobs table using config_hash. | ✓ |
| Defer sync to Phase 5 | Phase 2 writes job_runs referencing config directly. Sync engine lands with reload. | |
| You decide | Claude decides based on requirements. | |

**User's choice:** Upsert on startup
**Notes:** Foundation for Phase 5 reload.

### Timezone Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Server timezone only | All jobs fire in [server].timezone. Matches CONF-08 and SCHED-02. | ✓ |
| Per-job timezone override | Add optional timezone field to [[jobs]]. More flexible but complex. | |
| You decide | Claude picks. | |

**User's choice:** Server timezone only
**Notes:** Per-job timezone is v2 if needed.

### Multi-Fire Behavior

| Option | Description | Selected |
|--------|-------------|----------|
| All concurrent | tokio::spawn all due jobs immediately. No artificial stagger. | ✓ |
| Stagger by 100ms | Small delay between spawns to avoid thundering herd on DB writes. | |
| You decide | Claude picks. | |

**User's choice:** All concurrent
**Notes:** Each run is independent per SCHED-06.

### Run Tracking

| Option | Description | Selected |
|--------|-------------|----------|
| JoinSet | tokio::task::JoinSet for spawning + join_next() for reaping. Idiomatic tokio. | ✓ |
| HashMap<RunId, JoinHandle> | Explicit tracking by run ID. More control but more bookkeeping. | |
| You decide | Claude picks. | |

**User's choice:** JoinSet
**Notes:** Pairs well with tokio::select! in the main loop.

### Fire Queue Data Structure

| Option | Description | Selected |
|--------|-------------|----------|
| Global min scan | Call croner next_after() for every enabled job, pick earliest. O(n) per wake. | |
| BinaryHeap priority queue | Maintain min-heap of (next_fire, job_id). O(log n) per wake. | ✓ |
| You decide | Claude picks. | |

**User's choice:** BinaryHeap priority queue
**Notes:** User chose this over the recommended global min scan. Deliberate architectural choice for cleanliness.

### Module Home

| Option | Description | Selected |
|--------|-------------|----------|
| src/scheduler/ | New top-level module with loop, fire logic, run tracking. Testable in isolation. | ✓ |
| Inline in cli/run.rs | Keep everything in existing run command. Simpler but grows large. | |
| You decide | Claude decides module layout. | |

**User's choice:** src/scheduler/
**Notes:** Clean separation from CLI layer.

---

## Log Pipeline Architecture

### Backpressure Policy

| Option | Description | Selected |
|--------|-------------|----------|
| Tail-drop + marker | Drop newest incoming lines when full. Preserves early output. | |
| Head-drop + marker | Drop oldest buffered lines, keep newest. Preserves recent output. | ✓ |
| You decide | Claude picks. | |

**User's choice:** Head-drop + marker
**Notes:** Deviates from EXEC-04 "tail-sampling" language. User prefers preserving recent output for failure diagnosis.

### Insert Mode

| Option | Description | Selected |
|--------|-------------|----------|
| Micro-batch | Drain up to 64 lines per DB write in a single transaction. | ✓ |
| Single-row inserts | Each line gets its own INSERT. Simplest but may bottleneck. | |
| You decide | Claude picks. | |

**User's choice:** Micro-batch
**Notes:** Balances latency with SQLite write throughput.

### Channel Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Per-run channel | Each run gets its own bounded channel (256 lines). Isolation. | ✓ |
| Shared global channel | One channel for all runs. Simpler but noisy job can starve quiet ones. | |
| You decide | Claude picks. | |

**User's choice:** Per-run channel
**Notes:** None.

### Writer Task

| Option | Description | Selected |
|--------|-------------|----------|
| Per-run writer | Each run spawns a writer task that drains its channel. Natural lifecycle. | ✓ |
| Global writer task | One long-lived task reads from all channels. Centralizes DB access. | |
| You decide | Claude picks. | |

**User's choice:** Per-run writer
**Notes:** None.

---

## Script Execution & Tempfile Handling

### Cleanup Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Delete on completion | Remove tempfile immediately after script exits. Drop guard or explicit cleanup. | ✓ |
| Delete on next startup | Leave tempfiles during run for debugging. Clean on startup. | |
| You decide | Claude picks. | |

**User's choice:** Delete on completion
**Notes:** No disk accumulation.

### Tempfile Naming

| Option | Description | Selected |
|--------|-------------|----------|
| Random | Use tempfile crate's NamedTempFile for random names. | ✓ |
| Predictable (job name + run ID) | Name like cronduit-{name}-{id}.sh. Easier to debug but collision risk. | |
| You decide | Claude picks. | |

**User's choice:** Random
**Notes:** Standard practice, no collision risk.

### Execution Method

| Option | Description | Selected |
|--------|-------------|----------|
| Direct exec with shebang | Write tempfile with shebang, chmod +x, exec directly. Matches EXEC-02. | ✓ |
| Shell -c wrapper | Pass to /bin/sh -c. No tempfile but loses shebang customization. | |
| You decide | Claude picks. | |

**User's choice:** Direct exec with shebang
**Notes:** None.

### Log Pipeline Sharing

| Option | Description | Selected |
|--------|-------------|----------|
| Same pipeline | Both command and script use same bounded channel + per-run writer. DRY. | ✓ |
| Separate pipeline | Scripts get their own log path. No practical reason. | |
| You decide | Claude picks. | |

**User's choice:** Same pipeline
**Notes:** Consistent truncation rules across both job types.

---

## Graceful Shutdown Semantics

### Double Signal Behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Force-exit immediately | Second signal aborts grace wait. In-flight runs get status='error'. | ✓ |
| Ignore second signal | Honor only first. Grace period always completes. | |
| You decide | Claude picks. | |

**User's choice:** Force-exit immediately
**Notes:** Standard Unix double-Ctrl+C convention.

### Grace Period Expiry

| Option | Description | Selected |
|--------|-------------|----------|
| Cancel + record timeout | Cancel remaining via CancellationToken, mark status='timeout'. Drain logs. | ✓ |
| Hard-drop, status='error' | Drop JoinSet. Runs may not get final status. Faster. | |
| You decide | Claude picks. | |

**User's choice:** Cancel + record timeout
**Notes:** Preserves data integrity.

### Web Server on Shutdown

| Option | Description | Selected |
|--------|-------------|----------|
| Close web immediately | Stop accepting HTTP on first signal. Scheduler drains separately. | ✓ |
| Keep web open during grace | Web stays up for observation. More complex. | |
| You decide | Claude picks. | |

**User's choice:** Close web immediately
**Notes:** Clean separation. Sets pattern for Phase 3.

### Shutdown Summary Log

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, summary line | One tracing::info! with in_flight, drained, force_killed, grace_elapsed. | ✓ |
| No extra logging | Existing per-run logs sufficient. | |
| You decide | Claude picks. | |

**User's choice:** Yes, summary line
**Notes:** One line, greppable, useful for post-mortem.

---

## Claude's Discretion

- Channel implementation (tokio::sync::mpsc vs alternatives)
- BinaryHeap wrapper details
- Micro-batch size tuning
- shell-words usage timing
- src/scheduler/ sub-module naming
- resolved_schedule placeholder for Phase 2
- DST test approach
- tempfile persistence strategy

## Deferred Ideas

None — discussion stayed within phase scope.
