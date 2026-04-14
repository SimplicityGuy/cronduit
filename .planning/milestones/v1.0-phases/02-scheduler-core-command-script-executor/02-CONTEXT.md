# Phase 2: Scheduler Core & Command/Script Executor - Context

**Gathered:** 2026-04-10
**Status:** Ready for planning

<domain>
## Phase Boundary

A hand-rolled tokio scheduler loop that:

1. Syncs job definitions from the parsed TOML config into the `jobs` table on startup (upsert via `config_hash`)
2. Fires each enabled job at every match of its `resolved_schedule` in `[server].timezone`, using a BinaryHeap priority queue for efficient next-fire computation
3. Executes `command`-type jobs via `tokio::process::Command` and `script`-type jobs via shebang'd tempfiles
4. Captures stdout/stderr through a bounded per-run log channel into `job_logs` with micro-batch DB inserts
5. Enforces per-job timeouts via `tokio::select!`
6. Drains cleanly on SIGINT/SIGTERM with a configurable grace period, force-exits on a second signal

**Explicitly NOT in Phase 2:** Docker container execution (Phase 4), web UI (Phase 3), `@random` resolution (Phase 5), config reload / SIGHUP / file watch (Phase 5), SSE log streaming (Phase 6), `/health` and `/metrics` endpoints (Phase 6), retention pruner (Phase 6), orphan container reconciliation (Phase 4, SCHED-08).

New capabilities belong in other phases (see ROADMAP.md Phases 3-6).

</domain>

<decisions>
## Implementation Decisions

### Scheduler Loop Design

- **D-01:** The scheduler loop uses **`tokio::select!`** over three futures: (a) `tokio::time::sleep_until(next_fire_time)` for the nearest job fire, (b) `cancel_token.cancelled()` for shutdown, and (c) `join_set.join_next()` for reaping completed runs. **Sleep-to-next-fire** strategy — no fixed polling interval.
- **D-02:** A **`BinaryHeap`** (min-heap by next fire time) tracks per-job next-fire instants. After each wake, fired jobs are popped and re-inserted with their next-after time. O(log n) per fire vs O(n) scan. Chosen for clean architecture even at homelab scale.
- **D-03:** When the wall clock jumps forward (DST spring-forward or NTP correction), **all missed fires in the skipped interval are enqueued** as catch-up runs. Each missed fire is logged at WARN with the job name and the missed timestamp. At most one catch-up run per skipped fire per job. Matches SCHED-03 literally.
- **D-04:** When multiple jobs fire at the same instant, **all spawn concurrently** via `tokio::spawn`. No artificial stagger. Each run is independent per SCHED-06.
- **D-05:** In-flight runs are tracked via a **`tokio::task::JoinSet`**. `join_next()` in the main select loop reaps completed tasks. Clean, idiomatic tokio.
- **D-06:** On startup, after migrations, the scheduler **upserts config jobs into the `jobs` table** using `config_hash` for change detection. New jobs INSERT, changed jobs UPDATE (with new config_hash + config_json), removed jobs set `enabled=0` (DB-07). This is the sync engine foundation — Phase 5 reload will reuse this logic.
- **D-07:** All job fire evaluation uses **`[server].timezone` only**. No per-job timezone override in v1. Matches CONF-08 and SCHED-02.
- **D-08:** The scheduler lives in a new **`src/scheduler/`** module (with `mod.rs` for the loop, sub-modules for fire logic, run tracking, sync). `cli/run.rs` wires it up. Clean separation, testable in isolation.

### Log Pipeline Architecture

- **D-09:** Each spawned run gets its own **bounded channel (256 lines)** for log capture. Per-run isolation means one chatty job cannot starve another's log buffer. Truncation markers are per-run.
- **D-10:** Backpressure uses a **head-drop policy**: when the channel is full, the **oldest buffered lines are dropped** and a `[truncated N lines]` marker is inserted. This preserves the most recent output (typically more diagnostic for failures).
- **D-11:** Lines longer than **16 KB are truncated** with a `[line truncated at 16384 bytes]` marker appended. Matches EXEC-05.
- **D-12:** Each run spawns a **per-run writer task** that drains its channel and **micro-batch inserts** (up to 64 lines per transaction) into `job_logs`. Writer completes when the run's producers close the channel. Natural lifecycle, easy cleanup.
- **D-13:** Both `command` and `script` jobs pipe stdout/stderr through the **same log pipeline**. Stdout lines tagged `stream='stdout'`, stderr tagged `stream='stderr'`. DRY, consistent truncation rules.

### Script Execution & Tempfile Handling

- **D-14:** Script bodies are written to a tempfile via the **`tempfile` crate** (`NamedTempFile`) with **random names** in the system temp dir. No collision risk.
- **D-15:** The tempfile gets the configured **shebang** (default `#!/bin/sh`), is `chmod +x`'d, and executed directly via `tokio::process::Command` on the file path. **No shell `-c` wrapper.** Matches EXEC-02.
- **D-16:** Tempfiles are **deleted immediately on run completion** (success, failure, or timeout). A Drop guard or explicit cleanup in the run task ensures no disk accumulation.

### Graceful Shutdown Semantics

- **D-17:** On first SIGINT/SIGTERM, the **web server (axum listener) closes immediately** — no new HTTP requests accepted. The scheduler stops accepting new fires and begins draining in-flight runs for up to `shutdown_grace` (default 30s).
- **D-18:** A **second SIGINT/SIGTERM force-exits immediately**. In-flight runs get `status='error'` with `error_message='shutdown forced'`. Partial logs are whatever was already flushed to DB. Standard Unix double-Ctrl+C convention.
- **D-19:** When `shutdown_grace` expires with runs still in-flight, remaining tasks are **cancelled via CancellationToken** and marked `status='timeout'` with `error_message='shutdown grace expired'`. Any buffered logs are drained before closing DB pools.
- **D-20:** At shutdown completion, a **structured `tracing::info!` summary** is emitted with fields: `in_flight_count`, `drained_count`, `force_killed_count`, `grace_elapsed_ms`. One line, greppable, useful for post-mortem.

### Claude's Discretion

The planner / researcher may decide the following without re-asking:

- Exact channel implementation (tokio::sync::mpsc vs crossbeam vs custom ring buffer) as long as it supports the 256-line bound and head-drop semantics
- Whether the BinaryHeap uses `std::collections::BinaryHeap` with `Reverse` or a third-party min-heap crate
- Exact micro-batch size tuning (64 is a starting point; planner may adjust based on benchmarks or research)
- Whether `shell-words` is used for command tokenization in Phase 2 or deferred (CLAUDE.md lists it as recommended)
- Internal structure of `src/scheduler/` sub-modules (e.g., `loop.rs`, `fire.rs`, `sync.rs`, `run.rs` — or different names)
- Whether the sync engine writes `resolved_schedule` = raw `schedule` as a placeholder in Phase 2 (since `@random` resolution is Phase 5)
- DST regression test approach (frozen clock via `tokio::time::pause()` or injected clock trait)
- Whether `tempfile::NamedTempFile` is kept open or persisted before exec (implementation detail)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-Level Contracts

- `CLAUDE.md` — Locked tech stack, `croner 3.0` for cron parsing, `shell-words` for command tokenization, `tokio-util` for CancellationToken. Full version table.
- `.planning/PROJECT.md` — Vision, locked decisions, out-of-scope for v1.
- `.planning/REQUIREMENTS.md` — Phase 2 requirements: SCHED-01 through SCHED-07, EXEC-01 through EXEC-06. Success criteria in ROADMAP.md.
- `.planning/ROADMAP.md` §"Phase 2: Scheduler Core & Command/Script Executor" — Phase goal, success criteria, dependencies, requirement mapping.

### Specification & Research

- `docs/SPEC.md` §"Core Scheduler", §"Job Types" (command/script sections), §"Configuration" — Authoritative v1 spec for scheduler behavior, job lifecycle, and config shape.
- `.planning/research/ARCHITECTURE.md` §"Scheduler Loop", §"Job Execution Pipeline", §"Startup Boot Flow" — Component responsibilities, recommended patterns, data flow diagrams.
- `.planning/research/PITFALLS.md` §7 (SQLite write contention — already mitigated by split pools), §10 (cron library timezone handling), §11 (DST edge cases), §12 (graceful shutdown ordering).
- `.planning/research/STACK.md` — Version pins for `croner 3.0.1`, `tokio-util 0.7.18`, `shell-words 1.x`, `tempfile` (implicit via standard practice).

### Phase 1 Foundation (already built)

- `.planning/phases/01-foundation-security-posture-persistence-base/01-CONTEXT.md` — D-05 (single crate), D-06 (edition 2024, toolchain 1.94.1), D-07 (sqlx features), D-09/D-10 (justfile + just-only CI), D-13/D-14 (split migrations + schema parity test), D-15 (config_hash column exists).
- `src/config/mod.rs` — Config types (`JobConfig` with `command`, `script`, `schedule`, `timeout` fields), `parse_and_validate()`.
- `src/config/hash.rs` — `config_hash` computation for the sync engine.
- `src/db/mod.rs` — `DbPool` enum with split SQLite read/write pools, `migrate()`, `close()`.
- `src/shutdown.rs` — `CancellationToken`-based signal handler (SIGINT + SIGTERM). Phase 2 must extend this for double-signal force-exit (D-18).
- `src/cli/run.rs` — Current startup flow: parse config -> connect DB -> migrate -> start web. Phase 2 inserts the sync engine and scheduler loop between migrate and web serve.
- `migrations/sqlite/20260410_000000_initial.up.sql` — Schema with `jobs`, `job_runs`, `job_logs` tables ready for Phase 2 data writes.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`src/config/mod.rs` — `JobConfig` struct**: Already has `command`, `script`, `schedule`, `timeout`, `name`, `env` fields. The sync engine reads these directly.
- **`src/config/hash.rs` — `config_hash()`**: Computes SHA-256 of normalized job config. Used by the sync engine for change detection (D-06).
- **`src/db/mod.rs` — `DbPool` enum**: Split read/write SQLite pools are ready. The log writer (D-12) should use the write pool; the scheduler loop can use the read pool for job queries.
- **`src/shutdown.rs` — signal handler**: Already cancels a `CancellationToken` on first SIGINT/SIGTERM. Phase 2 needs to extend this for double-signal force-exit (D-18) — either a second token or a direct `std::process::exit`.

### Established Patterns

- **Just-only CI** (D-09/D-10 from Phase 1): All new recipes (e.g., `just test-scheduler`, integration test recipes) must go through the justfile.
- **Split migration directories**: Any new migration (e.g., partial index on `job_runs(status)` deferred from Phase 1) goes into both `migrations/sqlite/` and `migrations/postgres/`.
- **`tracing` structured logging**: Phase 1 established JSON-to-stdout with `RUST_LOG` env filter. Phase 2 scheduler events should follow the same pattern.

### Integration Points

- **`cli/run.rs` line 62-103**: After `pool.migrate()` and before `web::serve()`, Phase 2 inserts: (1) sync engine upsert, (2) scheduler loop spawn, (3) pass `CancellationToken` + `DbPool` + parsed config to the scheduler.
- **`AppState` in `src/web/mod.rs`**: May need to be extended to hold a scheduler handle or channel for Phase 3's "Run Now" button, but Phase 2 doesn't need web integration.

</code_context>

<specifics>
## Specific Ideas

- The user chose **BinaryHeap priority queue** over the simpler global-min scan. This is a deliberate architectural choice for cleanliness even at homelab scale. The planner should implement a proper min-heap (via `std::collections::BinaryHeap` with `Reverse` wrapper or equivalent).
- The user chose **head-drop** (drop oldest, keep newest) for the log channel backpressure — opposite of the spec's "tail-sampling drop policy" language in EXEC-04. The CONTEXT decision (D-10) takes precedence: head-drop preserves recent output which is more diagnostic for failure analysis. The planner should note this deviation from EXEC-04's wording and implement head-drop.
- The user wants the **web server to close immediately on first signal** (D-17), even though Phase 2 doesn't have meaningful web routes yet. This sets the shutdown ordering pattern that Phase 3 will inherit.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

### Reviewed Todos (not folded)

*(None — no pending todos matched Phase 2 at discussion time.)*

</deferred>

---

*Phase: 02-scheduler-core-command-script-executor*
*Context gathered: 2026-04-10*
