# Phase 2: Scheduler Core & Command/Script Executor - Research

**Researched:** 2026-04-10
**Domain:** Async scheduler loop, process execution, log pipeline, graceful shutdown (Rust/tokio)
**Confidence:** HIGH

## Summary

Phase 2 builds the scheduler core on top of Phase 1's foundation (config parsing, DB pools, shutdown signal handler, schema). The domain is well-understood: a `tokio::select!` loop driven by a `BinaryHeap` priority queue that fires jobs at cron-matched times using `croner` 3.0's timezone-aware `find_next_occurrence`, spawns `tokio::process::Command` for command/script execution, captures logs through bounded per-run channels with head-drop backpressure, and drains cleanly on signals.

All core libraries are already in `Cargo.toml` (`croner`, `tokio`, `chrono`, `chrono-tz`, `sqlx`, `tokio-util`). Two new dependencies are needed: `shell-words` 1.1.1 for command tokenization and `tempfile` 3.27.0 for script execution (already in dev-deps, needs promotion to runtime). The existing `src/shutdown.rs`, `src/config/mod.rs`, `src/config/hash.rs`, and `src/db/mod.rs` provide clean integration points.

**Primary recommendation:** Build the scheduler as a `src/scheduler/` module with sub-modules for the loop, sync engine, run task, and log pipeline. Use `croner::Cron::find_next_occurrence` with `chrono_tz::Tz` for all schedule evaluation. The sync engine reuses `config::hash::compute_config_hash`. DST tests use `tokio::time::pause()` with manually constructed `DateTime<Tz>` values.

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Scheduler loop uses `tokio::select!` over three futures: sleep_until(next_fire), cancel_token.cancelled(), join_set.join_next(). Sleep-to-next-fire strategy.
- **D-02:** BinaryHeap (min-heap by next fire time) tracks per-job next-fire instants. O(log n) per fire.
- **D-03:** Wall clock jumps forward: all missed fires in skipped interval are enqueued as catch-up runs. Each logged at WARN. At most one catch-up per skipped fire per job.
- **D-04:** Multiple jobs firing at same instant all spawn concurrently via tokio::spawn. No stagger.
- **D-05:** In-flight runs tracked via tokio::task::JoinSet. join_next() in main select loop reaps completed tasks.
- **D-06:** On startup, sync engine upserts config jobs into jobs table using config_hash for change detection. New jobs INSERT, changed jobs UPDATE, removed jobs set enabled=0 (DB-07).
- **D-07:** All job fire evaluation uses [server].timezone only. No per-job timezone in v1.
- **D-08:** Scheduler lives in src/scheduler/ module with sub-modules.
- **D-09:** Each spawned run gets its own bounded channel (256 lines). Per-run isolation.
- **D-10:** Head-drop policy: when channel full, oldest buffered lines dropped, [truncated N lines] marker inserted. Preserves recent output.
- **D-11:** Lines longer than 16 KB truncated with [line truncated at 16384 bytes] marker.
- **D-12:** Per-run writer task drains channel with micro-batch inserts (up to 64 lines per transaction) into job_logs.
- **D-13:** Both command and script jobs use same log pipeline. stdout tagged stream='stdout', stderr tagged stream='stderr'.
- **D-14:** Script bodies written to tempfile via tempfile crate (NamedTempFile) with random names.
- **D-15:** Tempfile gets configured shebang (default #!/bin/sh), chmod +x, executed directly. No shell -c wrapper.
- **D-16:** Tempfiles deleted immediately on run completion. Drop guard or explicit cleanup.
- **D-17:** On first SIGINT/SIGTERM, web server closes immediately. Scheduler stops new fires, drains in-flight for shutdown_grace.
- **D-18:** Second SIGINT/SIGTERM force-exits immediately. In-flight runs get status='error'.
- **D-19:** When shutdown_grace expires, remaining tasks cancelled via CancellationToken, marked status='timeout'.
- **D-20:** At shutdown completion, structured tracing::info! summary with in_flight_count, drained_count, force_killed_count, grace_elapsed_ms.

### Claude's Discretion

- Exact channel implementation (tokio::sync::mpsc vs crossbeam vs custom ring buffer) for 256-line bound + head-drop
- Whether BinaryHeap uses std::collections::BinaryHeap with Reverse or third-party min-heap
- Exact micro-batch size tuning (64 starting point)
- Whether shell-words is used for command tokenization in Phase 2 or deferred
- Internal structure of src/scheduler/ sub-modules
- Whether sync engine writes resolved_schedule = raw schedule as placeholder (since @random is Phase 5)
- DST regression test approach (frozen clock via tokio::time::pause() or injected clock trait)
- Whether tempfile::NamedTempFile is kept open or persisted before exec

### Deferred Ideas (OUT OF SCOPE)

None -- discussion stayed within phase scope.

</user_constraints>

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SCHED-01 | Hand-rolled tokio::select! scheduler loop owns the cron clock | Pattern 2 (select loop), croner API for find_next_occurrence, BinaryHeap priority queue |
| SCHED-02 | Fire each enabled job at every match of resolved_schedule in configured timezone, correct DST behavior | croner 3.0 timezone support via chrono_tz::Tz, DST test patterns |
| SCHED-03 | Wall clock jump forward: missed fires not silently dropped, each logged WARN, catch-up runs enqueued | Clock-jump detection pattern, Instant vs wall-clock comparison |
| SCHED-04 | Each fired job runs as tokio::spawn task owning its lifecycle | Per-run task pattern with JoinSet tracking |
| SCHED-05 | Per-job timeout enforced via tokio::select! | Timeout pattern with CancellationToken + child process kill |
| SCHED-06 | Concurrent runs allowed, each a separate job_runs row, trigger='scheduled' | JoinSet allows unlimited concurrent tasks; DB insert per run |
| SCHED-07 | Graceful shutdown: stop new fires, drain in-flight up to shutdown_grace, exit 0 | Shutdown state machine pattern, double-signal handling |
| EXEC-01 | Command-type jobs run via tokio::process::Command | shell-words for argv splitting, Command API |
| EXEC-02 | Script-type jobs: tempfile + shebang + chmod + execute | tempfile crate API, NamedTempFile pattern |
| EXEC-03 | stdout/stderr captured line-by-line with correct stream tags, ordering preserved | BufReader::lines on piped stdout/stderr, select! merge pattern |
| EXEC-04 | Bounded channel decouples log producers from writers; head-drop on overflow with truncation marker | Custom ring buffer or VecDeque-backed bounded channel |
| EXEC-05 | Lines > 16 KB truncated with marker | Pre-check in line reader before channel send |
| EXEC-06 | Success = status='success' exit_code=0; non-zero = status='failed' exit_code=N | ExitStatus API mapping |

</phase_requirements>

## Standard Stack

### Core (already in Cargo.toml)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| croner | 3.0.1 | Cron expression parsing + next-fire computation | DST-aware, L/\#/W modifiers, timezone via chrono_tz, human descriptions [VERIFIED: Cargo.toml + docs.rs] |
| tokio | 1.51 | Async runtime, process spawning, timers, signals | De facto Rust async runtime; `tokio::process::Command` for child processes [VERIFIED: Cargo.toml] |
| tokio-util | 0.7.18 | CancellationToken for shutdown cascading | Already used in `src/shutdown.rs` [VERIFIED: codebase] |
| chrono | 0.4.44 | Timestamps, DateTime arithmetic | Already integrated with croner and sqlx [VERIFIED: Cargo.toml] |
| chrono-tz | 0.10.4 | Named timezone support (e.g., America/New_York) | Required for `[server].timezone` evaluation [VERIFIED: Cargo.toml] |
| sqlx | 0.8.6 | Async DB operations for job_runs/job_logs writes | Split read/write pools already configured [VERIFIED: codebase] |
| tracing | 0.1.44 | Structured logging for scheduler events | Already established in Phase 1 [VERIFIED: codebase] |

### New Dependencies Needed

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| shell-words | 1.1.1 | Split command strings into argv | EXEC-01: tokenize `command = "curl -sf https://..."` into argv without invoking a shell [VERIFIED: crates.io search] |
| tempfile | 3.27.0 | Temporary script files | EXEC-02: write script body to NamedTempFile for execution. Already in dev-deps; promote to runtime dep [VERIFIED: Cargo.toml dev-deps] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| shell-words for argv split | Invoke shell via `sh -c "cmd"` | Shell invocation adds attack surface and unexpected behavior (globbing, variable expansion). shell-words gives clean POSIX tokenization without a shell process. Use shell-words. |
| Custom head-drop channel | tokio::sync::mpsc with try_send | mpsc only supports tail-drop (newest dropped on full). Head-drop requires a custom wrapper around VecDeque or similar. |
| std::collections::BinaryHeap | priority-queue crate | BinaryHeap with Reverse is standard library, zero deps, well-understood. No reason for a third-party crate at homelab scale. |

**Installation:**

```bash
cargo add shell-words@1.1.1
# tempfile already in dev-deps; move to [dependencies]
```

## Architecture Patterns

### Recommended Module Structure

```
src/scheduler/
  mod.rs          -- SchedulerLoop struct, public spawn() fn, main select! loop
  sync.rs         -- sync_config_to_db(): upsert jobs by name using config_hash
  fire.rs         -- BinaryHeap<Reverse<FireEntry>>, next-fire computation via croner
  run.rs          -- run_job() task: insert_running -> dispatch -> capture logs -> finalize
  log_pipeline.rs -- LogChannel (head-drop bounded), LogWriter task, line truncation
  command.rs      -- execute_command(): shell-words split + tokio::process::Command
  script.rs       -- execute_script(): tempfile write + shebang + chmod + execute
```

### Pattern 1: Select Loop with Three Arms

**What:** The scheduler loop selects over (a) sleep until next fire, (b) shutdown cancellation, (c) JoinSet completion.
**When to use:** Always -- this is the locked design (D-01).

```rust
// Source: CONTEXT.md D-01 + ARCHITECTURE.md Pattern 2
use std::collections::BinaryHeap;
use std::cmp::Reverse;
use tokio::task::JoinSet;

pub async fn run_loop(
    pool: DbPool,
    jobs: Vec<ResolvedJob>,
    timezone: chrono_tz::Tz,
    cancel: CancellationToken,
    shutdown_grace: Duration,
) {
    let mut heap: BinaryHeap<Reverse<FireEntry>> = build_initial_heap(&jobs, timezone);
    let mut join_set: JoinSet<RunResult> = JoinSet::new();

    loop {
        let next_fire = heap.peek().map(|r| r.0.instant);
        let sleep = match next_fire {
            Some(t) => tokio::time::sleep_until(t),
            None => tokio::time::sleep(Duration::from_secs(60)), // idle poll
        };
        tokio::pin!(sleep);

        tokio::select! {
            _ = &mut sleep => {
                check_clock_jump(&mut last_tick); // SCHED-03
                fire_due_jobs(&mut heap, &pool, &mut join_set, timezone).await;
            }
            Some(result) = join_set.join_next() => {
                handle_completed_run(result).await;
            }
            _ = cancel.cancelled() => {
                drain_and_shutdown(&mut join_set, &pool, shutdown_grace).await;
                break;
            }
        }
    }
}
```

### Pattern 2: Per-Run Log Channel with Head-Drop

**What:** Each run gets a bounded VecDeque-backed channel. When full, the oldest line is popped before pushing the new one, and a drop counter increments. On close, if drops occurred, a `[truncated N lines]` marker is inserted.
**When to use:** Every run (D-09, D-10).

```rust
// [ASSUMED] -- head-drop requires custom implementation since tokio::sync::mpsc
// only supports tail-drop (blocking sender or dropping newest on try_send).
pub struct HeadDropChannel {
    buf: VecDeque<LogLine>,
    capacity: usize,
    dropped_count: usize,
}

impl HeadDropChannel {
    pub fn new(capacity: usize) -> Self {
        Self { buf: VecDeque::with_capacity(capacity), capacity, dropped_count: 0 }
    }

    pub fn push(&mut self, line: LogLine) {
        if self.buf.len() >= self.capacity {
            self.buf.pop_front(); // drop oldest
            self.dropped_count += 1;
        }
        self.buf.push_back(line);
    }
}
```

**Async variant:** Since stdout/stderr readers and the DB writer are in separate tasks, use a `tokio::sync::mpsc::channel(256)` for the async boundary, but wrap the sender side: when `try_send` fails (full), read one item via `try_recv` on a companion receiver to make room (or use a `Mutex<VecDeque>` as a shared ring buffer). The simpler approach is a `tokio::sync::mpsc::channel(256)` with `send().await` and a separate drain task that keeps up. If the drain task falls behind, the producing side does a bounded wait then drops -- but given D-10 specifies head-drop (drop oldest), the cleanest implementation is:

1. Producer reads lines from stdout/stderr
2. Producer sends via `mpsc::Sender::try_send()`
3. On `Full` error: increment an atomic drop counter, discard the OLDEST line that the writer would have consumed (but since we can't reach into the receiver's buffer, the pragmatic approach is to use a `Mutex<VecDeque<LogLine>>` shared between producer and writer, with the writer draining in micro-batches)

**Recommended implementation:** Use `Arc<Mutex<VecDeque<LogLine>>>` + `tokio::sync::Notify` as the channel primitive. Producer locks, does head-drop if full, pushes, notifies. Writer locks, drains up to 64 lines, writes batch to DB. Simple, correct, testable. [ASSUMED]

### Pattern 3: Sync Engine (Config to DB)

**What:** On startup, diff config jobs against DB `jobs` table by name + config_hash. INSERT new, UPDATE changed, SET enabled=0 for removed.
**When to use:** Boot sequence, before scheduler loop starts (D-06).

```rust
// Source: CONTEXT.md D-06 + ARCHITECTURE.md sync engine description
pub async fn sync_config_to_db(pool: &DbPool, config: &Config, tz: &str) -> anyhow::Result<Vec<DbJob>> {
    let now = Utc::now().to_rfc3339();
    let config_jobs: Vec<_> = config.jobs.iter().map(|j| {
        let hash = compute_config_hash(j);
        let job_type = if j.command.is_some() { "command" }
                       else if j.script.is_some() { "script" }
                       else { "docker" };
        // In Phase 2, resolved_schedule = schedule (placeholder for @random in Phase 5)
        (j, hash, job_type)
    }).collect();

    // In a single transaction:
    // 1. For each config job: UPSERT by name
    // 2. For jobs in DB but not in config: SET enabled=0
    // 3. Return all enabled jobs for the scheduler
    todo!()
}
```

### Pattern 4: Process Execution with Timeout

**What:** Spawn a child process, pipe stdout/stderr to log channel, enforce timeout via `tokio::select!`.
**When to use:** Every command/script run (SCHED-05, EXEC-01, EXEC-02).

```rust
// Source: ARCHITECTURE.md Pattern 3 + CONTEXT.md D-05/D-15
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};

async fn execute_and_capture(
    mut cmd: Command,
    timeout: Duration,
    log_channel: Arc<Mutex<VecDeque<LogLine>>>,
    notify: Arc<Notify>,
    cancel: CancellationToken,
) -> RunResult {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Spawn readers for both streams
    let stdout_task = tokio::spawn(read_lines(stdout, "stdout", log_channel.clone(), notify.clone()));
    let stderr_task = tokio::spawn(read_lines(stderr, "stderr", log_channel.clone(), notify.clone()));

    let result = tokio::select! {
        status = child.wait() => {
            // Process exited naturally
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            status.map(|s| s.code())
        }
        _ = tokio::time::sleep(timeout) => {
            child.kill().await.ok();
            Err(ExecError::Timeout)
        }
        _ = cancel.cancelled() => {
            child.kill().await.ok();
            Err(ExecError::Shutdown)
        }
    };
    result
}
```

### Anti-Patterns to Avoid

- **Using `sh -c` for command execution:** Invokes a shell process, enables glob expansion and variable substitution. Use `shell-words::split()` + direct `Command::new(argv[0]).args(&argv[1..])` instead. [CITED: CLAUDE.md shell-words recommendation]
- **Unbounded log channels:** OOM risk on chatty jobs. Always use bounded channel with drop policy. [CITED: PITFALLS.md Pitfall 4]
- **Monotonic sleep for scheduling:** `tokio::time::sleep(next - now)` doesn't detect clock jumps. Use wall-clock comparison on each wake. [CITED: PITFALLS.md Pitfall 22]
- **Using `Local::now()` or implicit host timezone:** Always use the configured `chrono_tz::Tz` from `[server].timezone`. [CITED: PITFALLS.md Pitfall 5]
- **Holding Mutex across .await:** The `Arc<Mutex<VecDeque>>` log channel must be locked only for the push/pop operation, never across an await point. [ASSUMED]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Shell command tokenization | Custom split on spaces | `shell-words::split()` | Handles quoting, escaping, edge cases per POSIX spec [VERIFIED: docs.rs] |
| Cron expression parsing | Custom cron parser | `croner::Cron::from_str()` | DST-aware, L/\#/W, human descriptions, timezone support [VERIFIED: docs.rs] |
| Next-fire computation | Manual cron arithmetic | `croner::Cron::find_next_occurrence()` | Handles DST gaps/overlaps correctly per Vixie-cron rules [CITED: croner docs.rs] |
| Temp file creation | Manual `/tmp/cronduit-XXXXX` | `tempfile::NamedTempFile` | Race-free creation, auto-cleanup on drop [VERIFIED: crates.io] |
| Cancellation propagation | Manual AtomicBool | `tokio_util::sync::CancellationToken` | Tree-structured cancellation, already in use [VERIFIED: codebase] |
| Config hash | Manual serialization | `config::hash::compute_config_hash()` | Already implemented in Phase 1, stable SHA-256 [VERIFIED: codebase] |

## Common Pitfalls

### Pitfall 1: Head-Drop Channel Complexity

**What goes wrong:** D-10 specifies head-drop (drop oldest, keep newest) but `tokio::sync::mpsc` only offers tail-drop semantics (sender blocks or drops newest). Implementing head-drop across async task boundaries requires careful synchronization.
**Why it happens:** Most async channel implementations optimize for the common case (backpressure on producer), not for ring-buffer semantics.
**How to avoid:** Use a `Arc<Mutex<VecDeque<LogLine>>>` + `tokio::sync::Notify` pattern. The Mutex lock is held only for the brief push/pop, never across await. The Notify wakes the writer task when new lines are available. Test under load with a producer that generates lines faster than the writer can flush.
**Warning signs:** Deadlocks under load, writer starvation, lost truncation markers.

### Pitfall 2: tokio::process::Command Kill on Timeout Does Not Kill Process Group

**What goes wrong:** `child.kill()` sends SIGKILL to the child process only, not its children. A script that spawns subprocesses leaves orphans.
**Why it happens:** `tokio::process::Command` inherits the behavior of `std::process::Command` -- kill targets the PID, not the process group.
**How to avoid:** Use `child.id()` to get the PID, then `libc::kill(-pid, SIGKILL)` to kill the process group (negative PID). Alternatively, use `Command::process_group(0)` (available since Rust 1.64) to put the child in its own process group, then kill the group. [ASSUMED -- verify Command::process_group availability in edition 2024]
**Warning signs:** Zombie processes accumulating after timeouts.

### Pitfall 3: BufReader::lines() Blocks on Incomplete Lines

**What goes wrong:** `BufReader::lines()` waits for a newline. A process that writes "progress: 50%" without a trailing newline blocks the reader until the process exits or writes more. The last partial line is only delivered on EOF.
**Why it happens:** Line-oriented reading requires a delimiter. No delimiter = no line emitted.
**How to avoid:** This is acceptable behavior for Phase 2. Document that log lines are newline-delimited. The final partial line (if any) will be captured when the process exits and the pipe closes (EOF triggers the last line). No action needed unless real-time partial-line display is required (Phase 6 SSE could address this). [ASSUMED]
**Warning signs:** Missing last line of output in job_logs.

### Pitfall 4: Clock Jump Detection False Positives

**What goes wrong:** The scheduler wakes from sleep and checks `Utc::now() - last_tick > 2min`. But the sleep itself could be >2min if the next job is far in the future. False positive clock-jump detection on every wake.
**Why it happens:** Confusing "time elapsed since last tick" with "unexpected clock movement."
**How to avoid:** Track both (a) the expected wake time (from the sleep target) and (b) the actual wake time. Clock jump = `|actual - expected| > 2min`. If the scheduler slept for 3 hours because the next job was 3 hours away, `actual ~= expected` and there is no jump. [ASSUMED]
**Warning signs:** Spurious WARN logs about clock jumps every time the scheduler wakes for a distant job.

### Pitfall 5: SQLite Write Contention from Log Micro-Batches

**What goes wrong:** Multiple concurrent runs each have a log writer task doing micro-batch inserts. With SQLite's single-writer constraint, writers contend on the write pool (max_connections=1). Under load, writers queue up and latency spikes.
**Why it happens:** Phase 1's split read/write pool (Pitfall 7) serializes writes through a single connection. Multiple concurrent micro-batch writers compete for that connection.
**How to avoid:** This is mitigated by: (1) the write pool has `busy_timeout=5000ms` so writers wait rather than fail, (2) micro-batches are small (64 lines = fast transaction), (3) homelab scale means few concurrent runs. If contention becomes measurable, consider a single centralized log-writer task that aggregates from all runs. For Phase 2, the per-run writer pattern is correct and sufficient. [ASSUMED]
**Warning signs:** `SQLITE_BUSY` errors in logs, increasing job_logs insert latency.

### Pitfall 6: Script Tempfile Permission Denied on Some Platforms

**What goes wrong:** `NamedTempFile` creates the file, but `chmod +x` may fail if the temp directory is mounted `noexec` (common in hardened Linux setups).
**Why it happens:** `/tmp` is sometimes mounted with `noexec` flag for security.
**How to avoid:** Allow configurable temp directory (default to system temp). If chmod fails, log a clear error: "temp directory may be mounted noexec; configure a different path or use command-type jobs." Consider using `std::os::unix::fs::PermissionsExt` to set mode 0o755. [ASSUMED]
**Warning signs:** "Permission denied" errors on script-type jobs only, not command-type.

## Code Examples

### Croner: Parse and Find Next Occurrence

```rust
// Source: docs.rs/croner/3.0.1 [VERIFIED]
use croner::Cron;
use chrono::Utc;
use chrono_tz::Tz;
use std::str::FromStr;

let tz: Tz = "America/New_York".parse().unwrap();
let cron = Cron::from_str("30 2 * * *").unwrap();
let now = Utc::now().with_timezone(&tz);
let next = cron.find_next_occurrence(&now, false).unwrap();
// next is DateTime<Tz> -- the next 02:30 in America/New_York
// During DST spring-forward, croner skips the gap per Vixie-cron rules
```

### Croner: Human-Readable Description

```rust
// Source: docs.rs/croner/3.0.1 [VERIFIED]
let cron = Cron::from_str("0 */6 * * *").unwrap();
let desc = cron.describe();
// e.g., "Every 6 hours"
```

### Shell-Words: Split Command String

```rust
// Source: docs.rs/shell-words [VERIFIED]
let argv = shell_words::split("curl -sf 'https://example.com/health'").unwrap();
// argv = ["curl", "-sf", "https://example.com/health"]

let mut cmd = tokio::process::Command::new(&argv[0]);
cmd.args(&argv[1..]);
```

### Tempfile: Script Execution

```rust
// Source: tempfile crate docs + CONTEXT.md D-14/D-15 [VERIFIED + locked decision]
use tempfile::NamedTempFile;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;

let shebang = "#!/bin/sh";
let script_body = "echo hello\ndate";
let mut tmpfile = NamedTempFile::new()?;
writeln!(tmpfile, "{}", shebang)?;
write!(tmpfile, "{}", script_body)?;
tmpfile.flush()?;

// Set executable permission
let mut perms = tmpfile.as_file().metadata()?.permissions();
perms.set_mode(0o755);
tmpfile.as_file().set_permissions(perms)?;

let path = tmpfile.path().to_owned();
let mut cmd = tokio::process::Command::new(&path);
// tmpfile is kept alive until run completes, then dropped (auto-deletes)
```

### DB: Insert Running Run

```rust
// [ASSUMED] -- pattern based on schema + sqlx conventions
pub async fn insert_running(pool: &DbPool, job_id: i64, trigger: &str) -> anyhow::Result<i64> {
    let now = Utc::now().to_rfc3339();
    let writer = pool.writer(); // helper to get write pool

    let id = sqlx::query_scalar!(
        r#"INSERT INTO job_runs (job_id, status, trigger, start_time)
           VALUES (?, 'running', ?, ?)
           RETURNING id"#,
        job_id, trigger, now
    )
    .fetch_one(writer)
    .await?;

    Ok(id)
}
```

### DB: Micro-Batch Log Insert

```rust
// [ASSUMED] -- pattern for micro-batch insert
pub async fn insert_log_batch(pool: &DbPool, run_id: i64, lines: &[LogLine]) -> anyhow::Result<()> {
    let writer = pool.writer();
    let mut tx = writer.begin().await?;

    for line in lines {
        sqlx::query!(
            "INSERT INTO job_logs (run_id, stream, ts, line) VALUES (?, ?, ?, ?)",
            run_id, line.stream, line.ts, line.line
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}
```

### Shutdown: Double-Signal Pattern

```rust
// Source: CONTEXT.md D-17/D-18 + existing src/shutdown.rs [VERIFIED: codebase pattern]
pub fn install(cancel: CancellationToken, force_exit: Arc<AtomicBool>) {
    tokio::spawn(async move {
        wait_for_signal().await;
        tracing::info!("received signal, initiating graceful shutdown");
        cancel.cancel();

        // Wait for second signal
        wait_for_signal().await;
        tracing::warn!("received second signal, forcing immediate exit");
        force_exit.store(true, Ordering::SeqCst);
        std::process::exit(1);
    });
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `tokio-cron-scheduler` for cron loops | Hand-rolled `tokio::select!` loop | Project decision (no SQLite support in TCS) | Full control over @random, shutdown, lifecycle |
| `cron` crate for parsing | `croner` 3.0 | Project decision (L/\#/W + DST + descriptions) | Richer cron expressions, timezone-aware |
| Line-based unbounded channel | Head-drop bounded channel (256 lines) | Phase 2 decision (D-09/D-10) | Prevents OOM, preserves most recent diagnostic output |
| `sh -c "command"` for execution | `shell-words::split()` + direct exec | Project constraint | No shell injection, no globbing surprises |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Arc<Mutex<VecDeque>> + Notify is the best pattern for head-drop async channel | Architecture Patterns, Pattern 2 | Low -- alternative is a custom mpsc wrapper. Either works; the VecDeque approach is simpler to reason about. |
| A2 | Command::process_group(0) is available in edition 2024 / Rust 1.94 | Pitfalls, Pitfall 2 | Medium -- if unavailable, must use libc::setpgid in pre_exec hook. Verify before implementation. |
| A3 | BufReader::lines() delivers the final partial line on EOF | Pitfalls, Pitfall 3 | Low -- standard tokio behavior. If not, we lose the last line of output when it lacks a trailing newline. |
| A4 | Per-run log writers with SQLite single-writer pool are sufficient at homelab scale | Pitfalls, Pitfall 5 | Low -- busy_timeout handles contention. Centralized writer is a Phase 5+ optimization if needed. |
| A5 | tokio::time::pause() can be used for DST regression tests with manually constructed DateTime values | Claude's Discretion | Medium -- pause() controls tokio's time driver but croner uses chrono wall-clock. Tests may need an injected clock trait or direct DateTime construction without relying on tokio time. |

## Open Questions

1. **Head-drop channel: shared VecDeque vs custom mpsc wrapper?**
   - What we know: tokio::sync::mpsc does not support head-drop. A shared VecDeque+Mutex works but is less idiomatic.
   - What's unclear: Whether the lock contention between producer (line reader) and consumer (DB writer) is negligible.
   - Recommendation: Use the VecDeque+Mutex+Notify pattern. Lock hold time is microseconds (push/pop of a String). No contention concern at this scale.

2. **DST test strategy: tokio::time::pause() vs injected clock?**
   - What we know: croner's `find_next_occurrence` takes a `&DateTime<Tz>` -- we control the input. We don't need to mock system time; we just pass constructed DateTimes.
   - What's unclear: Whether the scheduler loop's `Utc::now()` calls need to be mockable for integration tests.
   - Recommendation: Unit-test DST behavior by calling `find_next_occurrence` directly with specific DateTime values (no clock mocking needed). Integration-test the full scheduler loop with `tokio::time::pause()` + `advance()` for timer control, passing pre-constructed "now" values.

3. **DbPool writer() helper method**
   - What we know: The current `DbPool` enum exposes `Sqlite { write, read }` and `Postgres(pool)`.
   - What's unclear: Whether to add `fn writer(&self) -> &SqlitePool` / `&PgPool` helper or use pattern matching at each call site.
   - Recommendation: Add `fn writer(&self) -> PoolRef` and `fn reader(&self) -> PoolRef` helpers to DbPool. Both return the appropriate pool. For Postgres, both return the same pool.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo-nextest 0.9.x (via `just nextest`) |
| Config file | `.config/nextest.toml` |
| Quick run command | `cargo nextest run --all-features -E 'test(scheduler)' --profile ci` |
| Full suite command | `just nextest` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SCHED-01 | Scheduler loop fires jobs on cron schedule | integration | `cargo test --test scheduler_fires_on_schedule` | Wave 0 |
| SCHED-02 | Jobs fire in configured timezone | unit | `cargo test scheduler::fire::tests::timezone_fire` | Wave 0 |
| SCHED-03 | Clock jump detection + catch-up runs | unit | `cargo test scheduler::fire::tests::clock_jump_catchup` | Wave 0 |
| SCHED-03 | DST spring-forward: missed fires logged WARN | unit | `cargo test scheduler::fire::tests::dst_spring_forward` | Wave 0 |
| SCHED-03 | DST fall-back: no double fire | unit | `cargo test scheduler::fire::tests::dst_fall_back` | Wave 0 |
| SCHED-05 | Per-job timeout produces status='timeout' | integration | `cargo test --test job_timeout` | Wave 0 |
| SCHED-06 | Concurrent runs create separate job_runs rows | integration | `cargo test --test concurrent_runs` | Wave 0 |
| SCHED-07 | Graceful shutdown drains in-flight runs | integration | `cargo test --test graceful_shutdown` | Exists (extend) |
| EXEC-01 | Command-type job executes and captures output | integration | `cargo test --test command_execution` | Wave 0 |
| EXEC-02 | Script-type job: tempfile + shebang + execute | integration | `cargo test --test script_execution` | Wave 0 |
| EXEC-03 | stdout/stderr tagged correctly in job_logs | unit | `cargo test scheduler::log_pipeline::tests::stream_tags` | Wave 0 |
| EXEC-04 | Head-drop on channel overflow + truncation marker | unit | `cargo test scheduler::log_pipeline::tests::head_drop` | Wave 0 |
| EXEC-05 | Lines > 16KB truncated | unit | `cargo test scheduler::log_pipeline::tests::line_truncation` | Wave 0 |
| EXEC-06 | Exit code 0 = success, non-zero = failed | integration | `cargo test --test exit_code_mapping` | Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo nextest run --all-features -E 'test(scheduler) | test(exec)' --profile ci`
- **Per wave merge:** `just nextest`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `tests/scheduler_fires_on_schedule.rs` -- covers SCHED-01
- [ ] `tests/job_timeout.rs` -- covers SCHED-05
- [ ] `tests/concurrent_runs.rs` -- covers SCHED-06
- [ ] `tests/command_execution.rs` -- covers EXEC-01, EXEC-03, EXEC-06
- [ ] `tests/script_execution.rs` -- covers EXEC-02
- [ ] Unit tests in `src/scheduler/fire.rs` -- covers SCHED-02, SCHED-03 (DST)
- [ ] Unit tests in `src/scheduler/log_pipeline.rs` -- covers EXEC-04, EXEC-05
- [ ] Extend `tests/graceful_shutdown.rs` -- covers SCHED-07 with scheduler running

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | N/A (v1 no auth) |
| V3 Session Management | No | N/A |
| V4 Access Control | No | N/A |
| V5 Input Validation | Yes | shell-words for command parsing (no shell injection); croner validates cron expressions; config parser validates job type exclusivity |
| V6 Cryptography | No | N/A (config_hash is not security-critical) |

### Known Threat Patterns for Command/Script Execution

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Shell injection via command field | Tampering | shell-words tokenization, no shell invocation (EXEC-01) |
| Path traversal via script tempfile | Information Disclosure | tempfile crate uses random names in system temp dir; no user-controlled path components |
| Resource exhaustion via log flood | Denial of Service | Bounded channel (256 lines) + head-drop + 16KB line limit (EXEC-04, EXEC-05) |
| Orphan processes on timeout | Denial of Service | Process group kill (-PGID) on timeout; cleanup in Drop guard |
| Tempfile accumulation on crash | Denial of Service | NamedTempFile auto-deletes on drop; explicit cleanup in run task |

## Sources

### Primary (HIGH confidence)

- [croner 3.0.1 docs.rs](https://docs.rs/croner/3.0.1/croner/) -- API: Cron::from_str, find_next_occurrence, describe, iter_after; DST behavior
- [shell-words docs.rs](https://docs.rs/shell-words/latest/shell_words/) -- API: split(), join(), quote()
- Codebase: `src/shutdown.rs`, `src/config/mod.rs`, `src/config/hash.rs`, `src/db/mod.rs`, `src/cli/run.rs`, `src/web/mod.rs` -- Phase 1 foundation
- Codebase: `migrations/sqlite/20260410_000000_initial.up.sql` -- Schema with jobs, job_runs, job_logs tables
- Codebase: `Cargo.toml` -- All dependency versions verified present

### Secondary (MEDIUM confidence)

- `.planning/research/ARCHITECTURE.md` -- Scheduler loop pattern, sync engine, per-run task lifecycle
- `.planning/research/PITFALLS.md` -- Pitfalls 4 (log backpressure), 5 (DST), 19 (shutdown), 22 (clock drift)
- [croner-rust GitHub](https://github.com/Hexagon/croner-rust) -- DST handling documentation

### Tertiary (LOW confidence)

- A5: DST test strategy with tokio::time::pause() -- needs validation during implementation

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH -- all libraries already in Cargo.toml or verified on crates.io. Versions confirmed.
- Architecture: HIGH -- patterns are standard tokio idioms, locked by user decisions, and well-documented in ARCHITECTURE.md research.
- Pitfalls: HIGH -- sourced from dedicated PITFALLS.md research, cross-referenced with croner docs for DST behavior.
- Log pipeline (head-drop): MEDIUM -- the head-drop channel pattern is custom and has no standard library solution. Implementation approach is sound but untested.

**Research date:** 2026-04-10
**Valid until:** 2026-05-10 (stable domain; crate versions pinned)
