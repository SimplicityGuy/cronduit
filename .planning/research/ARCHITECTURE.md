# Architecture Research

**Domain:** Self-hosted Rust scheduler/orchestration daemon with embedded web UI and Docker job execution
**Researched:** 2026-04-09
**Confidence:** HIGH

## Standard Architecture

### System Overview

```
┌───────────────────────────────────────────────────────────────────────┐
│                          Single Tokio Process                          │
│                                                                        │
│  ┌──────────────────┐   ┌──────────────────┐   ┌──────────────────┐   │
│  │ Config Loader    │   │  Scheduler Core  │   │   Web Server     │   │
│  │ + Watcher        │◄──┤  (cron clock)    │   │   (axum)         │   │
│  │ (notify / SIGHUP)│   │  tokio::select   │   │  + /health       │   │
│  └────────┬─────────┘   └────────┬─────────┘   │  + /metrics      │   │
│           │                      │             │  + HTML (askama) │   │
│           │ ConfigSnapshot       │ JobFire     │  + SSE /events   │   │
│           ▼                      ▼             └────────┬─────────┘   │
│  ┌────────────────────────────────────────┐             │             │
│  │         Job Registry (Arc<RwLock>)      │◄────────────┘             │
│  │  HashMap<JobId, ResolvedJob>            │                           │
│  └────────────┬───────────────────────────┘                           │
│               │                                                        │
│               ▼                                                        │
│  ┌────────────────────────────────────────┐                           │
│  │         Executor Dispatcher             │                           │
│  │  spawn(run_job(job, ExecBackend))       │                           │
│  └─┬──────────────┬──────────────┬────────┘                           │
│    │              │              │                                     │
│    ▼              ▼              ▼                                     │
│ ┌──────┐     ┌─────────┐    ┌──────────┐                              │
│ │Docker│     │ Command │    │  Script  │                              │
│ │backnd│     │ backend │    │  backend │                              │
│ │bollard│    │ tokio:: │    │ tempfile │                              │
│ │      │    │ process │    │ + proc    │                              │
│ └──┬───┘     └────┬────┘    └────┬─────┘                              │
│    │              │              │                                     │
│    └──────────────┴──────────────┘                                     │
│                   │                                                    │
│                   ▼                                                    │
│        ┌──────────────────────┐         ┌──────────────────────┐      │
│        │  Event Bus           │────────►│  Broadcast Channel   │      │
│        │  (tokio::broadcast)  │         │  for SSE/HTMX        │      │
│        └──────────┬───────────┘         └──────────────────────┘      │
│                   │                                                    │
│                   ▼                                                    │
│        ┌──────────────────────┐                                        │
│        │  Persistence Layer   │                                        │
│        │  (sqlx Pool)         │                                        │
│        │  SQLite | Postgres   │                                        │
│        └──────────┬───────────┘                                        │
│                   ▼                                                    │
│             ┌────────────┐                                             │
│             │ jobs       │                                             │
│             │ job_runs   │                                             │
│             │ job_logs   │                                             │
│             │ job_events │                                             │
│             └────────────┘                                             │
└───────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| **Config Loader + Watcher** | Parse TOML, interpolate `${ENV_VAR}`, validate, notify reloads via SIGHUP / `notify` crate / `POST /api/reload`. Produces an immutable `ConfigSnapshot`. | `serde` + `toml` + `notify` + `tokio::signal::unix::signal(SIGHUP)` |
| **Sync Engine** | Diff `ConfigSnapshot` against DB `jobs` table: create/update/disable. Writes are idempotent on a `config_hash` column. Never deletes jobs (preserves history). | Plain SQL in a transaction |
| **Job Registry** | In-memory authoritative map of enabled jobs with resolved schedules (including `@random` materialization). Shared between scheduler + web UI + executor. | `Arc<RwLock<HashMap<JobId, ResolvedJob>>>` |
| **Scheduler Core** | Own the cron clock. Tick on the min of all `next_fire_at` values, fire jobs by sending `JobFire` to executor, advance per-job cursors, handle manual "Run Now". | Custom async loop driven by `tokio::time::sleep_until` + `tokio::select!` over a command channel |
| **Executor Dispatcher** | For each fired job, `tokio::spawn` a run task that picks the right backend, writes `job_runs(status=running)`, captures stdout/stderr, writes terminal status, enforces timeout. | `tokio::spawn` + `tokio::select! { _ = timeout => kill, _ = backend => ok }` |
| **Docker Backend** | Ensure image, create container, start, stream logs, wait for exit, auto-remove. Honors all network modes including `container:<name>`. | `bollard::Docker::connect_with_unix_defaults()` |
| **Command/Script Backend** | Spawn local process (script written to tempfile with shebang), pipe stdout/stderr, enforce timeout. | `tokio::process::Command` |
| **Persistence Layer** | Single `sqlx::Pool` (enum wrapper over `Sqlite`/`Postgres`). Owns migrations (`sqlx::migrate!`). Repository functions per table. | `sqlx` 0.8 with `runtime-tokio-rustls` |
| **Event Bus** | Broadcast domain events (`JobRunStarted`, `LogLine`, `JobRunFinished`, `JobReloaded`) to any interested subscriber — web SSE, metrics counters, future webhook plugin. | `tokio::sync::broadcast::channel(capacity)` |
| **Web Server** | Axum app with HTML routes (askama-rendered), SSE route, JSON API, `GET /health`, `GET /metrics` (Prometheus). Serves embedded static assets. | `axum` 0.7/0.8 + `tower-http` + `rust-embed` |
| **Log Capture** | Per-run async task that consumes the backend's log stream and (a) inserts batches into `job_logs`, (b) publishes each line on the broadcast bus for live tail. | `futures::StreamExt` on `bollard` log stream / `BufReader::lines()` on child pipe |
| **Randomizer** | Resolves `@random` fields at (re)sync time, enforces `random_min_gap`, persists resolved cron to DB. | Pure function + `rand::rng()` |

## Recommended Project Structure

```
cronduit/
├── Cargo.toml
├── build.rs                     # Triggers tailwind CLI via cargo feature
├── migrations/
│   ├── 20260101000000_initial.up.sql
│   └── 20260101000000_initial.down.sql
├── templates/                   # askama HTML templates
│   ├── base.html
│   ├── dashboard.html
│   ├── job_detail.html
│   ├── run_detail.html
│   └── partials/
│       ├── job_row.html         # HTMX swap target
│       └── run_row.html
├── static/
│   ├── tailwind.css             # Generated at build time
│   └── htmx.min.js              # vendored
├── src/
│   ├── main.rs                  # Boot: config → db → registry → tasks → axum
│   ├── config/
│   │   ├── mod.rs               # ConfigSnapshot, ResolvedJob
│   │   ├── parse.rs             # TOML + serde
│   │   ├── interpolate.rs       # ${ENV_VAR}
│   │   ├── validate.rs          # Schedule parse, network mode, conflicts
│   │   └── watch.rs             # SIGHUP + notify + debounced reload channel
│   ├── schedule/
│   │   ├── mod.rs
│   │   ├── cron.rs              # Wrapper over `cron` crate
│   │   ├── random.rs            # @random resolution + min-gap
│   │   └── clock.rs             # Next-fire iterator, manual run injection
│   ├── scheduler/
│   │   ├── mod.rs               # The select-loop
│   │   └── registry.rs          # Arc<RwLock<...>> shared state
│   ├── executor/
│   │   ├── mod.rs               # Dispatcher, timeout, status lifecycle
│   │   ├── docker.rs            # bollard backend
│   │   ├── command.rs           # tokio::process backend
│   │   ├── script.rs            # Tempfile + process
│   │   └── logs.rs              # Log streaming pipeline → DB + broadcast
│   ├── db/
│   │   ├── mod.rs               # Pool enum, migrations
│   │   ├── jobs.rs              # CRUD + sync diff
│   │   ├── runs.rs              # job_runs CRUD
│   │   ├── logs.rs              # job_logs batch insert + tail query
│   │   └── retention.rs         # Prune task
│   ├── events/
│   │   ├── mod.rs
│   │   └── bus.rs               # broadcast::Sender<AppEvent>
│   ├── web/
│   │   ├── mod.rs               # Router construction, shared AppState
│   │   ├── state.rs             # AppState struct (Pool, Registry, Bus)
│   │   ├── assets.rs            # rust-embed static handler
│   │   ├── pages/               # HTML handlers
│   │   │   ├── dashboard.rs
│   │   │   ├── job.rs
│   │   │   ├── run.rs
│   │   │   └── settings.rs
│   │   ├── api/                 # JSON + control endpoints
│   │   │   ├── reload.rs
│   │   │   ├── run_now.rs
│   │   │   └── health.rs
│   │   ├── sse.rs               # /events SSE endpoint
│   │   └── templates.rs         # Askama structs + IntoResponse impls
│   ├── metrics/
│   │   ├── mod.rs               # Prometheus registry, counters, histograms
│   │   └── handler.rs           # /metrics
│   ├── telemetry.rs             # tracing + JSON subscriber
│   └── shutdown.rs              # Graceful drain: stop scheduler, await runs, close pool
└── tests/
    ├── sync_behavior.rs
    ├── random_scheduling.rs
    ├── docker_network_modes.rs  # testcontainers / docker-in-docker
    └── web_smoke.rs
```

### Structure Rationale

- **Top-level domains** (`config/`, `schedule/`, `scheduler/`, `executor/`, `db/`, `events/`, `web/`, `metrics/`) — clean seams; each has one reason to change. Scheduler doesn't know about HTTP; web doesn't know about bollard.
- **`executor/` contains all three backends** — they share a common `Backend` trait and log-capture pipeline, so they live together. Swapping one doesn't touch scheduling.
- **`events/`** — pulled out as its own module so both web and metrics can subscribe without importing scheduler internals.
- **`templates/` + `static/` at repo root, not under `src/`** — askama expects templates in `templates/` by default; `rust-embed` can glob `static/**`.
- **`migrations/` at repo root** — `sqlx::migrate!` expects it there.

## Architectural Patterns

### Pattern 1: Shared `AppState` via `Arc<...>` handles

**What:** Every component (web handlers, scheduler loop, executor tasks, metrics) receives a cheaply-cloned `AppState` containing `sqlx::Pool`, `Arc<Registry>`, `broadcast::Sender<AppEvent>`, and a `Sender<ControlMsg>` to the scheduler loop.
**When:** Default for all tokio-based daemons embedding axum.
**Trade-offs:** Simple, zero contention for read-only fields (Pool and broadcast are internally thread-safe), but forces discipline — never hold a write lock across an `.await` boundary.

```rust
#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub registry: Arc<RwLock<JobRegistry>>,
    pub events: broadcast::Sender<AppEvent>,
    pub control: mpsc::Sender<SchedulerCmd>,
    pub started_at: chrono::DateTime<chrono::Utc>,
}
```

### Pattern 2: `tokio::select!` scheduler loop (no external scheduler crate)

**What:** Hand-rolled loop that owns the `next_fire_at` priority queue and selects between (a) the timer to the next fire, (b) control messages (reload, run_now, shutdown), (c) completion acknowledgements from running jobs.
**When:** When you need fine-grained control over reload semantics, manual runs, and lifecycle events — which Cronduit does.
**Trade-offs:** More code than `tokio-cron-scheduler`, but avoids its Nats/Postgres-only persistence story (no SQLite backend), its separate notification-store abstraction, and its opinions about job identity. For Cronduit's single-node model the hand-rolled loop is ~200 lines and gives full control.

```rust
loop {
    let sleep = tokio::time::sleep_until(next_fire);
    tokio::pin!(sleep);
    tokio::select! {
        _ = &mut sleep => { fire_due_jobs(&state).await; }
        Some(cmd) = control_rx.recv() => match cmd {
            SchedulerCmd::Reload(snapshot) => reload(&state, snapshot).await,
            SchedulerCmd::RunNow(job_id)   => fire_specific(&state, job_id).await,
            SchedulerCmd::Shutdown         => break,
        },
    }
    next_fire = state.registry.read().unwrap().next_fire_at();
}
```

Why not `tokio-cron-scheduler`? Verified via GitHub: it supports Postgres and Nats metadata stores but **not SQLite**, which is Cronduit's default. Hand-rolling is simpler.

### Pattern 3: Per-run task with structured lifecycle

**What:** Each fired job is one `tokio::spawn`ed task that owns its full lifecycle: insert `job_runs(status=running)` → run backend → stream logs → update terminal status. Timeout is enforced with `tokio::select!`.
**When:** Always — isolates failures, lets scheduler keep ticking while jobs run arbitrarily long.
**Trade-offs:** Spawned tasks must hold their own `AppState` clone; no back-pressure on concurrent runs in v1 (acceptable per spec).

```rust
async fn run_job(state: AppState, job: ResolvedJob) {
    let run_id = db::runs::insert_running(&state.db, &job).await?;
    state.events.send(AppEvent::RunStarted { run_id, job_id: job.id }).ok();

    let backend_fut = dispatch_backend(&state, &job, run_id);
    let result = tokio::select! {
        r = backend_fut => r,
        _ = tokio::time::sleep(job.timeout) => Err(ExecError::Timeout),
    };

    let status = RunStatus::from(&result);
    db::runs::finalize(&state.db, run_id, status).await?;
    state.events.send(AppEvent::RunFinished { run_id, status }).ok();
}
```

### Pattern 4: Log pipeline with fan-out (DB + live tail)

**What:** Log-capture task reads from backend stream, batches lines, writes to `job_logs`, AND publishes each `LogLine` event to the broadcast bus. Web SSE subscribers filter by `run_id`.
**When:** Any time you need both persistence and live streaming from the same source.
**Trade-offs:** Broadcast channel can drop on slow subscribers (by design — SSE client is disposable); DB write is the authoritative record.

### Pattern 5: HTMX + SSE for live updates

**Recommendation:** Use **HTMX polling** for the dashboard (`hx-get="/jobs/table" hx-trigger="every 3s" hx-swap="outerHTML"`) and **SSE** only for the run detail page's log viewer. Keeps SSE subscriber count low and dashboard behavior trivial.

## Data Flow

### Startup Boot Flow

```
main.rs
  │
  ├─► parse CLI (clap)
  ├─► init tracing (JSON to stdout)
  ├─► load config (TOML + env interp)      ─┐
  ├─► open DB pool                          │
  ├─► run migrations                        │
  ├─► sync_config_to_db(snapshot)  ◄────────┘
  │      • upsert jobs by name
  │      • mark removed jobs enabled=false
  │      • resolve @random, persist resolved_schedule
  │      • enforce random_min_gap across all randomized jobs
  ├─► build JobRegistry from db::jobs where enabled=true
  ├─► build broadcast bus, mpsc scheduler control channel
  ├─► build AppState
  ├─► spawn scheduler loop (tokio::spawn)
  ├─► spawn config watcher (notify + SIGHUP)
  ├─► spawn log retention pruner (daily)
  ├─► build axum Router with AppState
  └─► axum::serve(...).with_graceful_shutdown(shutdown_signal)
```

### Config Reload Flow (SIGHUP / POST /api/reload)

```
Signal/HTTP
    │
    ▼
config::watch  ──load→  ConfigSnapshot ──send→ SchedulerCmd::Reload
                                                    │
                                                    ▼
                                             scheduler loop
                                                    │
                                        ┌───────────┴───────────┐
                                        ▼                       ▼
                              diff old vs new registry   sync_to_db (same fn as boot)
                                        │                       │
                                        └───────────┬───────────┘
                                                    ▼
                                        registry.write() = new map
                                                    │
                                                    ▼
                                         recompute next_fire
                                                    │
                                                    ▼
                                         events::JobsReloaded → SSE
```

**Idempotency:** `sync_to_db` is keyed on `(job.name, config_hash)`. Same config → no writes. Removed jobs → `enabled=false`, history preserved. New jobs → insert. Changed jobs → update and null out `resolved_schedule` if the schedule changed (forcing re-randomization on next resolve).

**In-flight runs are not cancelled on reload.** They finish under their old config; the new config only affects *future* fires. This is explicit — document it.

### Job Fire → Execute → Persist → Publish Flow

```
scheduler tick reaches next_fire
    │
    ▼
SchedulerCmd::Fire(job_id)
    │
    ▼
executor::dispatch(state, job)  ── tokio::spawn
    │
    ▼
db::runs::insert_running()  ─────► job_runs row (status=running, start_time=now)
    │
    ▼
events.send(RunStarted) ──────────► broadcast::Sender
                                          │
                                          ▼
                                    SSE /events → HTMX swap in dashboard
    │
    ▼
backend.execute(job)
  (docker | command | script)
    │
    ├── log stream ──► logs pipeline ──► db::logs::insert_batch()
    │                         │
    │                         └──► events.send(LogLine) ──► SSE → run detail tail
    ▼
wait_exit + timeout guard
    │
    ▼
db::runs::finalize(status, exit_code, end_time, duration_ms)
    │
    ▼
metrics: runs_total{status} inc, run_duration_seconds observe
    │
    ▼
events.send(RunFinished) ────────► SSE → status badge update
```

### Live Tail Flow (Run Detail)

On initial page render, the server queries `db::logs::all_for(run_id)` and embeds the lines statically. SSE only streams *new* lines that arrive after the page loads.

## Concurrency Model

Everything lives inside one tokio multi-thread runtime.

| Task | Spawned by | Lifetime | Shared state access |
|------|------------|----------|---------------------|
| Scheduler loop | `main` | Whole process | `registry.write()` on reload only; `db` read-only |
| Config watcher | `main` | Whole process | Sends `SchedulerCmd::Reload` only |
| Retention pruner | `main` | Whole process (ticker) | `db` write |
| Per-run executor | Scheduler loop | Until job exits or timeout | `db` write, `events` send |
| Per-run log pipeline | Per-run executor (child task) | Until stream ends | `db` write, `events` send |
| Axum HTTP connection | `axum::serve` | Per connection | `db` read, `registry.read()`, `events.subscribe()` |
| Per-SSE subscriber | Axum handler | Until client disconnects | `events` recv |

**Contention avoidance rules:**
1. `JobRegistry` uses `parking_lot::RwLock` (non-async). Writes only on reload (rare). Reads brief. **Never hold the lock across `.await`.** Clone the `ResolvedJob` and drop the guard first.
2. Database access is via a single `sqlx::Pool` — internal thread-safe semaphore. Pool size: `max(4, cpu_count)` for SQLite-WAL, `16` for Postgres.
3. The event bus is `tokio::sync::broadcast` with capacity ~1024. Lagged subscribers see `RecvError::Lagged(n)` — SSE handlers should skip-and-continue rather than disconnect.
4. The scheduler-to-executor channel is an `mpsc` — scheduler never blocks on the executor.
5. Graceful shutdown uses a `CancellationToken` (from `tokio-util`) passed to every long-lived task. On SIGINT/SIGTERM, cancel → scheduler stops firing → wait up to `shutdown_grace_period` for in-flight runs → close pool → exit.

## `@random` Resolution Strategy

### Where it lives
`src/schedule/random.rs` — pure function `resolve_random(jobs: &[ConfigJob], min_gap: Duration) -> Vec<ResolvedJob>`.

### When it runs
1. **At boot** — after loading config, before writing to DB.
2. **On reload** — only for jobs whose `schedule` contains `@random` AND (a) are newly added, or (b) had a schedule change in config. Existing randomized jobs whose `schedule` field in config is unchanged **keep their current `resolved_schedule`** (stability is a spec requirement).
3. **On explicit re-randomize** — future `POST /api/jobs/:id/rerandomize`.

### How persistence works
`jobs` table stores two columns:
- `schedule TEXT NOT NULL` — raw from config (`"@random"`, `"0 @random * * *"`, etc.)
- `resolved_schedule TEXT NOT NULL` — fully concrete cron (`"37 14 * * *"`)

The scheduler clock uses `resolved_schedule` exclusively. `@random` never reaches the clock.

### Min-gap algorithm

```
Given jobs with any @random in their schedule, grouped by "same day" constraint:
1. For each such job, enumerate candidate (minute, hour) tuples respecting
   the non-random fields (e.g., "@random @random * * 1-5" means minute+hour
   are random but day-of-week is fixed).
2. Sort currently-resolved randomized jobs by fire time within a day.
3. For a new job needing resolution:
   a. Sample (minute, hour) uniformly from candidates.
   b. Compute fire time on a reference day.
   c. If distance to any already-resolved peer is < min_gap, re-sample.
   d. Give up after N attempts → log warning, accept best candidate.
4. Persist resolved_schedule.
```

**Invariant:** Min-gap constrains randomized jobs *among themselves only* — it doesn't interfere with user-written exact crons.

**Edge case:** If 10 randomized jobs declare `random_min_gap = "3h"` (max 8 slots in 24h), resolution logs a warning and relaxes the gap for the overflow jobs. Don't fail to boot.

## Database Schema

The schema must work on both SQLite and Postgres with one logical migration. Use portable types: text timestamps (RFC3339), TEXT for enums, `INTEGER PRIMARY KEY`. Use per-backend migration files if needed.

```sql
-- 20260101000000_initial.up.sql

CREATE TABLE IF NOT EXISTS jobs (
    id                 INTEGER PRIMARY KEY,
    name               TEXT NOT NULL UNIQUE,
    schedule           TEXT NOT NULL,
    resolved_schedule  TEXT NOT NULL,
    job_type           TEXT NOT NULL,                    -- 'docker' | 'command' | 'script'
    config_json        TEXT NOT NULL,
    config_hash        TEXT NOT NULL,
    enabled            INTEGER NOT NULL DEFAULT 1,
    timeout_secs       INTEGER NOT NULL,
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jobs_enabled ON jobs(enabled);

CREATE TABLE IF NOT EXISTS job_runs (
    id             INTEGER PRIMARY KEY,
    job_id         INTEGER NOT NULL REFERENCES jobs(id),
    status         TEXT NOT NULL,                        -- 'running' | 'success' | 'failed' | 'timeout' | 'error'
    trigger        TEXT NOT NULL,                        -- 'schedule' | 'manual' | 'startup'
    start_time     TEXT NOT NULL,
    end_time       TEXT,
    duration_ms    INTEGER,
    exit_code      INTEGER,
    container_id   TEXT,
    error_message  TEXT
);

CREATE INDEX IF NOT EXISTS idx_job_runs_job_id_start ON job_runs(job_id, start_time DESC);
CREATE INDEX IF NOT EXISTS idx_job_runs_status ON job_runs(status) WHERE status = 'running';
CREATE INDEX IF NOT EXISTS idx_job_runs_start_time ON job_runs(start_time);

CREATE TABLE IF NOT EXISTS job_logs (
    id         INTEGER PRIMARY KEY,
    run_id     INTEGER NOT NULL REFERENCES job_runs(id),
    stream     TEXT NOT NULL,                            -- 'stdout' | 'stderr'
    ts         TEXT NOT NULL,
    line       TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_job_logs_run_id_id ON job_logs(run_id, id);

-- Optional: events ring for replay-on-reconnect
CREATE TABLE IF NOT EXISTS job_events (
    id         INTEGER PRIMARY KEY,
    kind       TEXT NOT NULL,
    run_id     INTEGER,
    job_id     INTEGER,
    payload    TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_job_events_created_at ON job_events(created_at);
```

**Portability notes:**
- For guaranteed BIGINT on Postgres, split migrations with the `sqlx` compile-time feature (`20260101_initial.sqlite.up.sql` vs `20260101_initial.postgres.up.sql`) loaded via two `sqlx::migrate!` invocations chosen by the `DbPool` enum variant.
- Timestamps as RFC3339 `TEXT` avoids the SQLite-has-no-TIMESTAMP headache and sorts correctly.
- `config_json` TEXT vs `JSONB` on PG: store as TEXT for portability; never query inside it.

### Retention pruning

A daily tokio `Interval` task:

```sql
DELETE FROM job_logs
 WHERE run_id IN (
   SELECT id FROM job_runs
    WHERE start_time < :cutoff
 );

DELETE FROM job_runs
 WHERE start_time < :cutoff;
```

Default cutoff: `now - 90 days`. Wrap in a transaction. On SQLite, consider `VACUUM` after large prunes (configurable).

## Log Capture from Docker Containers

Using `bollard` 0.17+:

```rust
use bollard::container::LogsOptions;
use futures_util::StreamExt;

let opts = LogsOptions::<String> {
    follow: true,
    stdout: true,
    stderr: true,
    timestamps: true,
    tail: "all".into(),
    ..Default::default()
};

let mut stream = docker.logs(&container_name, Some(opts));

while let Some(next) = stream.next().await {
    match next {
        Ok(LogOutput::StdOut { message }) => { /* emit stdout */ }
        Ok(LogOutput::StdErr { message }) => { /* emit stderr */ }
        Ok(_) => {}
        Err(e) => { tracing::warn!(?e, "log stream error"); break; }
    }
}
```

**Flow:**
1. After `docker.start_container`, spawn a **log pump task** holding an `AppState` clone and `run_id`.
2. Separately, `docker.wait_container` on another task — it resolves with the exit code.
3. `tokio::select!` between the wait future and the `timeout`. When wait resolves, the log stream also ends (bollard closes it on container exit).
4. The log pump task batches lines and inserts into `job_logs` in chunks of ~32 for write amplification control.
5. Each batch write also re-publishes individual lines on the broadcast bus (not batched — live tail wants low latency).
6. After exit: stop the log pump, finalize `job_runs` row, emit `RunFinished`.

**For `command`/`script` backends:** `tokio::process::Command` with `.stdout(Stdio::piped()).stderr(Stdio::piped())`, then wrap each pipe in `BufReader::new(pipe).lines()` and spawn two small pumps. Same `job_logs` insert path, same broadcast publish.

## HTMX Live Updates — Final Wiring

| Page element | Mechanism | Why |
|--------------|-----------|-----|
| Dashboard job table | HTMX polling `hx-trigger="every 3s"` on the `<table>` partial | Trivial, cache-friendly, no SSE bookkeeping for N idle tabs |
| Next-fire countdowns | Pure JS `setInterval` client-side | Avoids hammering the server for clock display |
| Job detail "last 20 runs" | HTMX polling `hx-trigger="every 5s"` on the `<tbody>` partial | Same as dashboard |
| Run detail — status badge | HTMX `hx-trigger="every 2s"` while running | Self-terminating |
| Run detail — log tail | **SSE** `<div hx-ext="sse" sse-connect="/events/runs/:id/logs">` | Only place where push semantics beat pull |
| "Run Now" button | `hx-post="/api/jobs/:id/run"` with `hx-swap="none"` | Fire-and-forget; polling updates status |
| Reload config button | `hx-post="/api/reload"` with `hx-swap="innerHTML"` into a toast | One-shot RPC |

## Scaling Considerations

| Scale | Architecture behavior |
|-------|----------------------|
| 1–50 jobs, 0–10 runs/min | Defaults are fine. SQLite WAL, 64-row log batches, 3s poll intervals. ~30 MB RAM. |
| 50–500 jobs, 10–100 runs/min | Still fine on SQLite WAL. Increase broadcast capacity to 4096. Raise log-batch size to 128. Hourly retention. |
| 500+ jobs, 100+ runs/min | Migrate to Postgres. SQLite writer contention on job_logs becomes the first bottleneck. |

### Scaling Priorities

1. **First bottleneck: SQLite writes under log storms.** Mitigations: batch inserts, throttle per-run log rate, truncate extremely long lines (16 KB cap, document it).
2. **Second bottleneck: broadcast channel lag.** Per-topic channels (one `broadcast` per active run) lazily created.
3. **Third bottleneck: registry RwLock contention during reload.** Build new map outside the lock, then a single atomic swap.

## Anti-Patterns

### Anti-Pattern 1: Using a framework scheduler (tokio-cron-scheduler) and persisting history separately
**Why it's wrong:** Two sources of truth. The crate lacks SQLite persistence (verified 2026-04). **Do this instead:** Hand-roll the loop.

### Anti-Pattern 2: Holding the registry lock across `.await`
**Why it's wrong:** Blocks every other task trying to read the registry. **Do this instead:** Compute the new map outside the lock, take it only for the swap.

### Anti-Pattern 3: Cancelling running jobs on config reload
**Why it's wrong:** Users lose work silently. **Do this instead:** Reloads affect only future fires. In-flight runs finish under their old config. Document it.

### Anti-Pattern 4: Streaming logs directly from the container stream into an SSE response
**Why it's wrong:** Bollard log streams are single-consumer. **Do this instead:** Single log-pump task per run writes to DB + broadcasts.

### Anti-Pattern 5: Deleting removed jobs from the DB on config removal
**Why it's wrong:** Violates spec ("preserve history for removed jobs"). **Do this instead:** `UPDATE jobs SET enabled = 0`.

### Anti-Pattern 6: Embedding Tailwind via CDN
**Why it's wrong:** Defeats the single-binary story; breaks in air-gapped homelabs. **Do this instead:** Run Tailwind standalone CLI in `build.rs` to generate `static/tailwind.css`.

### Anti-Pattern 7: Using `.unwrap()` on broadcast `.send()` errors
**Why it's wrong:** `broadcast::Sender::send` returns `Err` when there are zero subscribers. **Do this instead:** `state.events.send(ev).ok();`

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Docker daemon | `bollard` over Unix socket `/var/run/docker.sock` | Auto-detect via `Docker::connect_with_unix_defaults()`; handle `PermissionDenied` with a clear startup error; no TCP/TLS in v1 |
| SQLite | `sqlx` with `sqlite://` URL, `WAL` mode, `busy_timeout=5000` | Set PRAGMAs on pool-connect hook |
| Postgres | `sqlx` with `postgres://` URL | No extensions required; portable schema only |
| Prometheus | Text exposition on `GET /metrics` | `prometheus` or `metrics` + `metrics-exporter-prometheus`; axum handler, not a separate listener |
| Config file | `notify` crate + SIGHUP + `POST /api/reload` | Debounce filesystem events 500 ms (editors write-then-rename) |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| Config ↔ Scheduler | `mpsc::Sender<SchedulerCmd>` | Scheduler owns reload application; config module is dumb parser |
| Scheduler ↔ Executor | `tokio::spawn(run_job(state, job))` | Scheduler "throws" — no shared queue, no back-pressure in v1 |
| Executor ↔ Web | Via `AppState.db` + `AppState.events` only | Never call executor functions from web handlers directly |
| Web "Run Now" ↔ Scheduler | `mpsc` `SchedulerCmd::RunNow(job_id)` | Manual run goes through the same executor path with `trigger='manual'` |
| Scheduler ↔ Metrics | Via `AppState.events` bus; metrics module subscribes | Metrics has no direct scheduler dependency |

## Suggested Build Order

This is the critical output for the roadmap.

1. **Phase A — Skeleton** (no scheduling yet)
   - `Cargo.toml`, `main.rs`, tracing init, `clap` CLI, `tokio::main`
   - `config/parse.rs` + TOML fixtures
   - `db/mod.rs`: Pool enum, `sqlx::migrate!`, initial schema
   - `sync_config_to_db` (config → jobs table), idempotency via `config_hash`
   - **Exit criterion:** `cronduit --config test.toml` loads config, creates DB, inserts jobs, exits.

2. **Phase B — Scheduler Core + Command Executor**
   - `schedule/cron.rs` using `cron` crate (skip `@random` initially)
   - `scheduler/mod.rs`: the `tokio::select!` loop, next_fire computation
   - `executor/command.rs`: `tokio::process::Command`, stdout/stderr piping
   - `executor/logs.rs`: line pump into `job_logs`
   - `db/runs.rs`, `db/logs.rs` CRUD
   - Graceful shutdown via `CancellationToken`
   - **Exit criterion:** A command-type job on `*/1 * * * *` fires every minute, writes run + logs to SQLite. `Ctrl+C` waits for in-flight runs.

3. **Phase C — Web UI Read-Only**
   - `axum` router, `AppState`, `rust-embed` static assets
   - Tailwind build pipeline (`build.rs` or `xtask`)
   - `askama` templates for base, dashboard, job detail, run detail
   - HTMX polling on dashboard and run detail
   - `GET /health`, structured JSON logging
   - **Exit criterion:** Operator opens `http://localhost:8080`, sees jobs, clicks into a run, sees logs.

4. **Phase D — Docker Executor**
   - `executor/docker.rs`: `bollard` client, create/start/wait/remove
   - Image pull on demand, all network modes, volumes, env
   - Log stream via `bollard` log pump into the same `executor/logs.rs` pipeline
   - `container_name` per job, timeout handling
   - Integration test with `testcontainers` covering `container:<name>` network mode
   - **Exit criterion:** A docker-type job with `network="container:vpn"` runs, logs captured, container auto-removed.

5. **Phase E — Reload, Random, Manual Run**
   - `config/watch.rs`: SIGHUP + `notify` + debounce
   - `POST /api/reload`
   - Scheduler reload path: diff, disable removed, preserve history
   - `schedule/random.rs`: `@random` resolution + `random_min_gap`
   - `POST /api/jobs/:id/run` → manual trigger path
   - **Exit criterion:** Edit config → SIGHUP → new jobs appear in UI, removed jobs go gray (history intact), randomized jobs have stable resolved schedules across reloads.

6. **Phase F — Live Updates, Metrics, Retention**
   - `events/bus.rs` broadcast channel, wire all emitters
   - SSE endpoint for run log tail; HTMX `sse-swap` on run detail page
   - `metrics/` module, `GET /metrics` Prometheus exposition
   - `db/retention.rs` daily pruner
   - **Exit criterion:** Viewing a running job's detail page shows lines streaming in real time. `/metrics` exposes four required counters. Old runs prune on schedule.

7. **Phase G — Script Executor, Polish, Docs, CI**
   - `executor/script.rs` (tempfile + shebang)
   - Settings/status page
   - Filter/search/sort on dashboard
   - GitHub Actions: fmt, clippy, test, multi-arch build
   - README with quickstart, example `docker-compose.yml`, design-system-compliant styling
   - **Exit criterion:** Stranger can clone, `docker compose up`, and schedule a job in 5 minutes.

**Build-order rationale:**
- A→B gets a working CLI scheduler with persistence — the hardest core to debug — before any web UI distractions.
- B→C means the web UI reads real data from day one (no mocks).
- C→D: delaying Docker until the web UI exists means you can *watch* docker jobs work, which catches network-mode bugs faster than log-grepping.
- D→E: reload and random need the full job lifecycle already working.
- E→F: live updates are pure polish on top of a complete system; deferring lets you validate polling first.
- F→G: script executor, filters, and CI are independent and can be parallelized within phase G.

## Key Findings (TL;DR for the roadmap author)

1. **Hand-roll the scheduler loop, don't adopt `tokio-cron-scheduler`** — it lacks a SQLite metadata store. ~200 lines and matches Cronduit's single-node model.
2. **Single broadcast event bus is the backbone of live updates and metrics.** Every executor publishes `RunStarted` / `LogLine` / `RunFinished`. SSE handlers subscribe and filter. Metrics subscribes and counts. Future webhooks plug in the same way.
3. **Use HTMX polling for everything except log tail.** SSE only for `/runs/:id/logs`.
4. **`@random` is resolved once, persisted as `resolved_schedule`,** and only re-randomized when the config's `schedule` field changes or on explicit rerandomize. Min-gap only constrains randomized jobs among themselves.
5. **Never delete jobs.** Reload path marks removed jobs `enabled=false` — history preserved.
6. **Log pipeline fans out once:** single pump task per run writes to `job_logs` AND publishes on broadcast.
7. **Schema works on both SQLite and Postgres** by sticking to `INTEGER PRIMARY KEY`, RFC3339 text timestamps, and avoiding JSONB.
8. **Critical correction from training data:** `askama_axum` as a separate crate is deprecated in recent askama releases (v0.13+); implement `IntoResponse` directly on template structs. Flag in Phase C.
9. **Build order: Skeleton → Scheduler+Command → Web UI → Docker → Reload+Random → Live updates+Metrics+Retention → Polish.**

## Confidence Assessment

| Area | Level | Reason |
|------|-------|--------|
| Component boundaries and AppState pattern | HIGH | Standard tokio+axum daemon shape |
| `tokio-cron-scheduler` unsuitability | HIGH | Verified live: no SQLite store |
| `bollard` log streaming API | HIGH | Verified crate structure; method shape stable across 0.15–0.17 |
| `askama_axum` deprecation | HIGH | Verified on new repo; flag in roadmap |
| SSE + broadcast pattern in axum | HIGH | Verified via axum docs |
| Schema portability SQLite↔Postgres | MEDIUM-HIGH | Proven approach; may need per-backend migration files |
| `@random` min-gap algorithm | MEDIUM | Spec-derived; behavior on overflow is a recommended design choice |
| HTMX polling vs SSE split | HIGH | Well-established pattern |

## Open Questions (flag for phase-specific research later)

- **Tailwind build integration in Cargo** — `build.rs`, `cargo xtask`, or Makefile? Defer to Phase C research.
- **`sqlx::migrate!` with a DbPool enum** — may require two macro invocations guarded by compile-time features or runtime dispatch; verify in Phase A.
- **`notify` on Docker bind-mounts** — some kernels don't deliver inotify events through bind mounts reliably. Validate early in Phase E; SIGHUP is the fallback.
- **Graceful handling of bollard reconnect** when `/var/run/docker.sock` disappears (docker daemon restart). v1 may just exit and let Docker restart cronduit, but worth documenting.

## Sources

- `bollard` docs and container module — https://docs.rs/bollard/
- `axum` SSE module — https://docs.rs/axum/latest/axum/response/sse/index.html
- `askama` status — https://github.com/askama-rs/askama (v0.15.6 as of March 2026; `askama_axum` deprecated)
- `tokio-cron-scheduler` — https://github.com/mvniekerk/tokio-cron-scheduler (no SQLite store)
- `sqlx` migrations and Pool — stable API
- Project files: `.planning/PROJECT.md`, `docs/SPEC.md`
