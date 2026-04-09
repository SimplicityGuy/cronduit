# Pitfalls Research

**Domain:** Self-hosted Docker-native cron scheduler (Rust + bollard + sqlx + web UI)
**Researched:** 2026-04-09
**Confidence:** HIGH (for core bollard/Docker/SQLite/cron DST pitfalls — multiple independent sources and known ecosystem bugs); MEDIUM (for @random correctness — largely original design territory, cross-checked against cron library behavior)

This document catalogs pitfalls specifically relevant to Cronduit's architecture. Generic "write good Rust" advice is excluded. Each pitfall maps to a roadmap phase and a concrete prevention strategy.

---

## Critical Pitfalls

### Pitfall 1: Docker socket mount is quietly root-equivalent, and the README doesn't say so

**What goes wrong:**
Cronduit mounts `/var/run/docker.sock` read-write. Any code path that reaches the Docker API — including a future bug in the web UI that lets a user influence `image`, `volumes`, or `command` — is effectively arbitrary code execution as root on the host. A user reading the README as "it's a cron runner" will not intuit that exposing the web UI to a LAN they don't fully trust, or letting a compromised job container influence config, is equivalent to handing out host root.

**Why it happens:**
The docker.sock owner is root, and granting access to it is equivalent to unrestricted root on the host. An attacker can trivially `docker run -v /:/host ...` and write to the real filesystem. Read-only mounting the socket does NOT mitigate this — the API accepts GETs that still allow container creation via workarounds, and most Cronduit operations need write access anyway. With the unauthenticated v1 web UI, any network-adjacent actor who can reach `:8080` AND trigger a "Run Now" on a job whose config they can influence has host root. Even without config influence, a "Run Now" plus the ability to submit an arbitrary image is full compromise.

**How to avoid:**
1. **README SECURITY section is non-negotiable and must be above the fold.** State plainly: "Cronduit requires read-write access to the Docker socket. This is equivalent to root on the host. Do not expose the Cronduit web UI to any network you do not fully trust. v1 ships without authentication."
2. **Ship a THREAT_MODEL.md** enumerating: (a) what mounting the socket grants, (b) what happens if the web UI is reachable by an untrusted client, (c) what happens if an attacker can edit the config file, (d) what happens if a malicious image is scheduled.
3. **Default bind to `127.0.0.1:8080`, not `0.0.0.0:8080`.** Force the operator to consciously opt into LAN exposure. This single decision prevents most accidental internet exposure.
4. **Refuse to start if bound to `0.0.0.0` AND config file contains no warning acknowledgment** (e.g., require `[server] i_understand_no_auth = true`). Annoying but correct for v1.
5. **Log a loud warning every 5 minutes** while bound to non-loopback without auth: `"WARN: web UI bound to <addr> without authentication — equivalent to root-level access if reachable"`.
6. **docker-compose.example.yml must use `expose:` not `ports:`** for the web UI, forcing users to consciously add a reverse proxy.

**Warning signs:**
- README reviewer says "wait, what does mounting the socket actually allow?"
- Default config has `bind = "0.0.0.0:8080"` with no auth nag.
- First issue from an adopter is "my Cronduit was compromised."

**Phase to address:** Phase 1 (binding default + startup nag) AND Phase N-1 release-prep (README/THREAT_MODEL). This pitfall cannot be deferred — it's table stakes for responsible OSS release.

**Severity:** CRITICAL

---

### Pitfall 2: `container:<name>` network mode breaks silently when the target container restarts

**What goes wrong:**
A job configured with `network = "container:vpn"` runs fine for weeks. The VPN container restarts (update, crash, host reboot). The next Cronduit run attempting to spawn into `container:vpn` fails with `cannot join network namespace of a non running container`, or the job starts into a *stale* network namespace that no longer has external connectivity. Worst case: the operator sees "job failed" in the dashboard but has no signal that the root cause is upstream container health, not their job.

**Why it happens:**
Docker's `container:<name>` mode attaches the new container to another container's network namespace at spawn time. If the target is not running, the create call fails. If the target has been restarted but has a different PID/ns, the attachment may still succeed but the namespace lifecycle is fragile — Moby has known races here ([moby#50326](https://github.com/moby/moby/issues/50326)). Cronduit is explicitly designed around this mode (it's the ofelia-killer feature), so this is a first-class failure mode, not an edge case.

**How to avoid:**
1. **Pre-flight check before spawn:** If the job uses `container:<name>`, inspect the target container first via bollard. If it's not `running`, record a structured failure reason (`network_target_not_running`) — do NOT just surface Docker's raw error.
2. **Distinct failure category in the UI and metrics:** `failures_total{reason="network_target_unavailable"}` — this is the single most useful operational signal for the homelab VPN use case.
3. **Document clearly** in README and job config docs: "If your target container restarts, dependent jobs will fail. This is a Docker limitation, not a Cronduit bug. Consider using named networks instead if your workload tolerates it."
4. **Optionally (v1.1):** Add `network_wait_for_target = "30s"` — poll the target container until running, or fail with the structured reason after timeout.
5. **Never silently retry into a different network.** That would violate operator intent (the whole point is "this job MUST go through the VPN").

**Warning signs:**
- Dashboard shows "failed" with only Docker's raw error message.
- Operator can't distinguish "my script had a bug" from "the VPN was down."
- No metric dimension for network-target failures.

**Phase to address:** Phase 3 (Docker execution) must implement the pre-flight check and structured failure. This is the single feature that justifies Cronduit existing over ofelia — it must be best-in-class.

**Severity:** CRITICAL

---

### Pitfall 3: `wait_container` vs `auto_remove` race loses exit codes

**What goes wrong:**
Job container exits with code 1. Because `auto_remove = true` (the v1 default), the Docker daemon removes the container. Meanwhile Cronduit's `wait_container` stream is racing to read the exit code. If the daemon wins, `wait_container` returns an error like `No such container` instead of the exit code — and Cronduit records "failed with unknown exit code" instead of "failed with exit code 1." Worse, the run log stream (`logs` with `follow=true`) may be torn down mid-read, truncating stdout/stderr.

**Why it happens:**
This is a well-documented Docker architecture race ([docker-py#2655](https://github.com/docker/docker-py/issues/2655), also referenced in [moby#8441](https://github.com/moby/moby/issues/8441)). The Docker daemon doesn't guarantee that `wait` completes before `auto_remove` fires. Airflow and other schedulers hit this intermittently. A naive bollard implementation that calls `create_container(auto_remove=true)` → `start` → `wait` → `logs` will lose data on a non-trivial fraction of runs.

**How to avoid:**
1. **Do NOT use Docker's `auto_remove=true` flag for job execution.** Instead, create the container WITHOUT auto-remove, then:
   - Start the container
   - Attach/stream logs concurrently (see Pitfall 4)
   - Call `wait_container` and collect exit code
   - Explicitly call `remove_container(force=false)` AFTER wait completes and logs drain
2. **Structure execution as a state machine:** `Creating → Starting → Running → Exited → LogsDrained → Removed`. Each transition is observable and recordable. If Cronduit crashes between `Exited` and `Removed`, startup reconciliation (Pitfall 9) cleans up the orphan.
3. **Record exit code BEFORE remove.** Persist to SQLite as soon as wait returns, not after remove.
4. **Integration test that explicitly triggers this race:** spawn a container that exits in <50ms, assert exit code and full stdout are captured reliably across 1000 runs.

**Warning signs:**
- Run records with status="success" but empty logs.
- Run records with exit_code=NULL and status="failed".
- Intermittent test flakes in container execution tests.
- "No such container" errors in tracing output.

**Phase to address:** Phase 3 (Docker execution). This must be correct from day one; retrofitting the state machine later is painful because it changes the persistence schema.

**Severity:** CRITICAL

---

### Pitfall 4: Log streaming back-pressure causes OOM or dropped lines

**What goes wrong:**
A job container produces 500 MB of stdout in 30 seconds (not unusual for a backup or rsync job). Cronduit calls `Docker::logs(follow=true)` and pipes the stream into a channel that feeds both (a) the SQLite writer and (b) any live web UI viewers via SSE. Two failure modes:
- **Unbounded buffering:** The SQLite writer can't keep up, the channel is unbounded, memory grows to 500 MB+, OOM-kill in a memory-constrained homelab.
- **Silent drops:** The channel is bounded with `try_send`, failures are dropped on the floor, the stored log is missing chunks with no indication.

Additionally: very long lines (e.g., a single 100 MB JSON blob, or a binary dump) break naive "split on newline → one DB row per line" schemas — either one row is enormous, or the "line" concept is meaningless.

**Why it happens:**
Bollard exposes logs as an async Stream, which follows Rust's zero-cost abstractions by design: the stream doesn't apply back-pressure to the producer (Docker daemon) — it applies back-pressure to the consumer. If the consumer doesn't handle the buffering strategy explicitly, both failure modes are the default. The SSE path compounds this because the UI viewer is a second consumer that can stall (slow browser, closed tab, network hiccup) without the server noticing promptly.

**How to avoid:**
1. **Bounded channels with an explicit drop policy.** Use `tokio::sync::mpsc::channel(N)` with N small (e.g., 256 lines or 1 MB). On full, apply *tail-sampling*: drop oldest non-critical chunks and record a `[cronduit: N lines dropped due to back-pressure]` marker in the DB so operators see the gap.
2. **Chunk-based, not line-based, storage.** Store log blobs as `(run_id, stream, seq, bytes, captured_at)` where `bytes` is capped (e.g., 64 KB per row). A single giant line spans multiple rows. A line-oriented view can reconstruct lines at query time; a "raw bytes" view is always correct.
3. **Decouple DB writer from SSE broadcast.** SSE viewers subscribe to an in-memory broadcast channel (`tokio::sync::broadcast`) that is ALLOWED to drop for slow consumers. The DB writer is a separate fire-and-forget task with its own bounded buffer. A slow browser MUST NOT cause log loss to SQLite.
4. **Enforce per-run output cap** (configurable, default e.g. 10 MB). On exceed: truncate, mark run metadata with `log_truncated=true`, display prominently in UI.
5. **Sanitize for UI rendering:** binary bytes, NUL bytes, and very long lines must be safe to render (see Pitfall 13).

**Warning signs:**
- Cronduit RSS grows linearly with job output size.
- Reports of "some log lines missing" but no error in metrics.
- OOM-kills in `docker stats` for the Cronduit container during long jobs.
- Browser tab freezing when opening a run detail page for a chatty job.

**Phase to address:** Phase 3 (execution) — the bounded channel and chunk storage must exist from the first execution implementation. Phase 5/6 (web UI) — SSE decoupling.

**Severity:** CRITICAL

---

### Pitfall 5: DST and timezone handling is hand-rolled, not delegated to a library that explicitly tests DST

**What goes wrong:**
- **Spring forward:** A job scheduled `30 2 * * *` (daily at 02:30) on the day of DST spring-forward either (a) silently never runs (gap doesn't exist), (b) runs 30 min late at 03:00, or (c) runs twice.
- **Fall back:** A job scheduled `30 1 * * *` runs twice on the fall-back day because 01:30 occurs twice in wall-clock time.
- **Container `TZ` drift:** The Cronduit container has no `TZ` set, uses UTC. The operator interprets schedules in America/Los_Angeles. Everything is 7-8 hours off and intermittently wrong across DST.
- **Timezone not persisted:** The operator changes the system timezone, Cronduit now computes different "next run" times for the same schedule. Previously-scheduled runs get weird semantics.

**Why it happens:**
Cron libraries vary wildly in DST handling. `cron` (zslayton) has known weekday numbering quirks inherited from Quartz. Hand-rolling cron arithmetic over `chrono::NaiveDateTime` works 99.5% of the time and subtly breaks twice a year. DST is the single most common "we ran in production for a year, then March came" pitfall for any scheduler.

**How to avoid:**
1. **Use `croner-rust`** — it explicitly documents DST behavior aligned with Vixie-cron / OCPS spec: spring-forward gap → fixed-time jobs run at first valid second after gap; fall-back overlap → fixed-time jobs run once at first occurrence. This matches operator intuition and is testable.
2. **Mandatory `[server] timezone = "..."` in config,** defaulting to `UTC`. Never implicitly use host timezone. Document that changing this retroactively changes "next run" semantics.
3. **Compute all schedule arithmetic in the configured timezone using `chrono_tz::Tz`.** Store UTC in the database (`next_run_at`, `started_at`, `ended_at`) and convert at render/compute time.
4. **DST regression test suite:** Unit tests with frozen clocks on March 9 2025 02:00, Nov 2 2025 01:00 (and equivalent for 2026/2027) asserting spring-forward and fall-back behavior for fixed-time and wildcard schedules.
5. **Web UI shows both UTC and local for next_run / last_run.** An operator eyeballing "last run was 02:30" on a DST day needs to see `02:30 PDT (09:30 UTC)` to debug correctly.
6. **Dockerfile sets `TZ=UTC`** by default in the image, and docs explicitly recommend operators set `TZ` in compose to match their config timezone.

**Warning signs:**
- `cron` crate in Cargo.toml (not `croner`) — warrants a second look.
- Any scheduling arithmetic using `Local::now()` or `chrono::NaiveDateTime`.
- No DST tests in the test suite.
- Dashboard shows only local time with no UTC reference.

**Phase to address:** Phase 2 (scheduler core). Choosing the cron library and the timezone model is a Phase 2 decision that cannot be deferred; rewriting it later breaks persistence.

**Severity:** CRITICAL

---

### Pitfall 6: `@random` min-gap is "best effort" instead of an actual guarantee, and its state is invisible

**What goes wrong:**
- Operator configures 5 jobs with `schedule = "@random"` and `random_min_gap = "90m"` expecting jobs to be spread out. Implementation randomizes each job independently, then checks gaps pairwise — if there's a conflict, it shuffles one. But pathological inputs (e.g., 20 jobs with 90m gap in a 24h day) are mathematically infeasible and the implementation either infinite-loops, silently drops the gap constraint, or generates a non-random distribution (first jobs keep their random slots, later ones get crammed).
- Operator reloads config. All `@random` schedules re-randomize silently. Yesterday's "daily random" ran at 03:14; today it runs at 21:47. The operator has no idea when "today's random" is, can't plan around it, can't correlate with upstream events.
- Operator asks "when will job X next run?" The UI shows a computed next-run based on the resolved schedule, but the resolution process is opaque — they can't tell it's random vs. fixed without reading the config file.
- Cronduit restarts. `@random` re-rolls. A job that was scheduled for 04:00 is now scheduled for 19:00. An operator who was waiting for it to run "any minute now" is now waiting 15 hours.

**Why it happens:**
`@random` is Cronduit's second marquee feature (after `container:<name>` networking). It's also entirely original — there's no established library or spec. It's tempting to implement it as "pick a random time and call it a day" without thinking about: persistence, feasibility, observability, reloads, restarts, min-gap as a hard constraint, or the operator mental model.

**How to avoid:**
1. **Treat `@random` resolution as a first-class, persisted event.** On resolution, write a `schedule_resolutions` row: `(job_id, resolved_cron, resolved_at, expires_at, reason)`. This is what `next_run_at` reads from. The DB is the source of truth for "what is today's random."
2. **Re-roll on a predictable cadence, not on every reload.** Default: once per calendar day in the configured timezone, at a stable time (e.g., 00:00 local). Config reloads do NOT re-roll unless the job's other fields changed. Restarts do NOT re-roll unless the resolution has expired.
3. **Feasibility check at config-load time.** Given N `@random` jobs sharing the same day and a `min_gap` of G, verify `N * G ≤ 24h`. If infeasible, FAIL LOUDLY at startup (not silently drop the constraint). Error message: `"@random min_gap of 90m cannot be satisfied for 20 jobs in 24h (max 16). Reduce job count or lower min_gap."`
4. **Constraint satisfaction, not retry-until-fits.** Use a deterministic feasibility algorithm: divide the day into N slots each of size (24h / N), place each job's random offset inside its slot with jitter bounded to preserve min-gap. This is guaranteed to satisfy min-gap if feasibility passes. Seed with a deterministic RNG keyed by `(date, job_name)` so the schedule is reproducible for debugging.
5. **Surface resolutions in the UI.** Job detail shows: "Schedule: `@random` (today resolved to `14 17 * * *`, re-rolls at 00:00 tomorrow)." Dashboard badge distinguishes `@random` from fixed schedules.
6. **Manual "re-roll now" and "pin to today's resolution" actions** for operators debugging.
7. **Structured log event on every re-roll:** `{"event": "random_resolved", "job": "...", "previous": "...", "next": "...", "reason": "daily_rollover"}`. Makes log-based audit trivial.
8. **Metric:** `cronduit_random_resolutions_total{job, reason}` and `cronduit_random_feasibility_failures_total`.

**Warning signs:**
- `@random` resolution lives in an in-memory `HashMap` rather than a DB table.
- No feasibility check — or the check logs a warning and continues.
- UI shows `@random` without revealing the resolved value.
- Re-roll cadence is "on config reload" or "on restart" rather than time-based.
- No tests for N jobs with infeasible min-gap.

**Phase to address:** Phase 2 (scheduler core) — data model and resolution algorithm. Phase 5 (UI) — surface resolutions. This is the single feature most likely to embarrass Cronduit publicly if shipped sloppily — it's novel, visible, and "how does `@random` work?" will be the top README question.

**Severity:** CRITICAL

---

### Pitfall 7: SQLite write contention kills throughput the first time the log writer and the scheduler step on each other

**What goes wrong:**
Cronduit ships with SQLite as the default. A long-running job produces chatty logs; the log-writer task hammers `INSERT INTO job_logs`. Meanwhile the scheduler is trying to `UPDATE jobs SET next_run_at = ?` on its tick, and the web UI is trying to `INSERT INTO job_runs` for a "Run Now" click. SQLite's WAL mode serializes writers — if `busy_timeout` is unset (default: 0), the second writer gets `SQLITE_BUSY` immediately and sqlx returns an error. Cronduit either crashes, drops the write, or livelocks on retry.

Even WITH WAL mode and a long busy timeout, using a default sqlx connection pool of multiple connections for writes creates ping-pong lock contention: each connection thinks it's the writer, they take turns waiting for each other, and throughput collapses.

**Why it happens:**
- SQLite's concurrency model: many concurrent readers, exactly one writer. WAL allows readers to proceed during a write, but writes serialize.
- Default sqlx SqlitePool has multiple connections and will round-robin writes across them, causing them to wait on each other.
- `busy_timeout` is not set by default in sqlx — you must enable it explicitly via `PoolOptions::after_connect` or connection URL params.
- The "happy path" works in development with one user and few logs. It falls over the first time a real homelab workload with 20 jobs and chatty output hits it.

**How to avoid:**
1. **Separate read and write pools.**
   - `write_pool`: `SqlitePool` with `max_connections = 1` — a single dedicated writer. All writes go through an in-process queue backed by this pool. Writes never conflict with each other.
   - `read_pool`: `SqlitePool` with `max_connections = N` (e.g., 4-8) — concurrent readers for UI queries.
2. **Enable WAL mode and pragmas on every connection.** sqlx `after_connect`:
   ```
   PRAGMA journal_mode = WAL;
   PRAGMA synchronous = NORMAL;
   PRAGMA busy_timeout = 5000;
   PRAGMA foreign_keys = ON;
   PRAGMA temp_store = MEMORY;
   ```
3. **Batch log inserts.** Group log lines from a single run into a transaction (e.g., every 500 lines or 250 ms, whichever first). One transaction = one write lock acquisition = 10-100x throughput vs. per-line inserts.
4. **WAL checkpointing strategy.** Default is `PASSIVE` on commit; if a reader is perpetually open (e.g., a stuck SSE viewer), the WAL grows unbounded. Run `PRAGMA wal_checkpoint(TRUNCATE)` on a timer (e.g., every 5 min) from the writer task. Log if WAL file exceeds threshold (e.g., 100 MB).
5. **Load test explicitly:** simulate 10 concurrent jobs each emitting 1000 lines/sec for 60 seconds. Assert: no errors, WAL size stable, DB size grows linearly with data.
6. **Document** in README that SQLite is appropriate for typical homelab (10-100 jobs, modest output); PostgreSQL recommended for heavier workloads.

**Warning signs:**
- Intermittent `SQLITE_BUSY` or "database is locked" errors in tracing.
- WAL file (`cronduit.db-wal`) growing to hundreds of MB.
- SqlitePool configured with `max_connections > 1` and no write-queue wrapper.
- No `busy_timeout` in connection pragmas.
- Log writes happening outside a transaction batch.

**Phase to address:** Phase 4 (persistence) — must be correct from first implementation. Retrofitting a write queue after the codebase assumes "sqlx pool handles it" is a painful refactor.

**Severity:** CRITICAL

---

### Pitfall 8: Schema parity between SQLite and PostgreSQL silently diverges

**What goes wrong:**
Cronduit ships with one migration set that "works on both" SQLite and Postgres. Over time, a developer adds a column with `DEFAULT now()` (works in Postgres, SQLite wants `CURRENT_TIMESTAMP`), or uses `JSONB` (Postgres-only), or an enum type (Postgres-only), or relies on Postgres trigger behavior. CI runs tests against SQLite only. A Postgres user runs migrations and gets a cryptic error, or — worse — migrations succeed but queries return different results across the two backends.

**Why it happens:**
SQLx does NOT abstract SQL dialect; you write raw SQL and it validates against whatever database `DATABASE_URL` points to. The overlap between SQLite and Postgres SQL is real but narrow. Type mappings differ (booleans, timestamps, JSON). It's easy to add a "small tweak" that works on your dev SQLite and breaks on a user's Postgres.

**How to avoid:**
1. **Two migration directories:** `migrations/sqlite/` and `migrations/postgres/`. Parallel files with the same version numbers and conceptual schema but dialect-appropriate syntax. Forces every schema change to be consciously applied to both.
2. **CI matrix:** every PR runs the full test suite against BOTH SQLite and Postgres. Non-optional. Use testcontainers or the `postgres` service in GitHub Actions.
3. **Query abstraction policy:** either (a) all queries written as raw SQL with per-dialect variants for anything non-portable, or (b) strictly limit to a "lowest common denominator" SQL subset (no `JSONB`, no enums, no array columns, no `ON CONFLICT DO UPDATE SET col = EXCLUDED.col` if behavior differs, etc.). Document the policy in `CONTRIBUTING.md`.
4. **Type policy:** timestamps as `TEXT ISO-8601` (SQLite) and `TIMESTAMPTZ` (Postgres) — sqlx handles both if the Rust type is `DateTime<Utc>`. Booleans as `INTEGER 0/1` (SQLite) and `BOOLEAN` (Postgres) — sqlx handles if type is `bool`. JSON fields as `TEXT` (SQLite) and `JSONB` (Postgres) — store serialized JSON and deserialize in Rust. No clever per-backend features.
5. **Integration test for log retention pruning:** tests against both backends to catch dialect-specific `DELETE ... LIMIT` vs. `DELETE WHERE ctid IN (SELECT ctid ...)` differences.
6. **Schema doc (`docs/SCHEMA.md`):** single source of truth describing the conceptual schema. Migrations reference it.

**Warning signs:**
- `migrations/` has only one set of files.
- CI runs tests against only one backend.
- A query uses `JSONB`, `ARRAY`, enum types, `ILIKE`, `RETURNING *` without testing the SQLite path.
- `sqlx::query!` macro compile-time checks against one DATABASE_URL without offline-mode JSON for both.

**Phase to address:** Phase 4 (persistence). Schema + migration layout + CI matrix must be set up at the same time as the first schema.

**Severity:** HIGH

---

### Pitfall 9: Config reload is non-atomic and can orphan running jobs

**What goes wrong:**
Operator edits `cronduit.toml`, saves. The file watcher (or SIGHUP, or API `/reload`) fires. Cronduit parses the new config. The new config disables job `weekly-backup` that is CURRENTLY executing. Cronduit has to decide:
- Kill the running container? (data loss)
- Let it finish, then mark it disabled? (orphaned: not in config, but in DB)
- Refuse to reload because a target job is running? (operator frustrated)

Compounding: the file watcher fires multiple events for a single save (some editors write-truncate-write, triggering several `Modify` events). If reload isn't debounced, Cronduit parses and applies the config 5 times in 500ms, one of which might catch a half-written file and fail to parse — and if the failure handling is "clear the job set and start over," jobs vanish mid-edit.

**Why it happens:**
- `notify` crate fires raw OS events; many editors do atomic writes via rename, or truncate-then-write, producing multiple events.
- Reload logic is often written as "parse → diff → apply" without treating it as a transaction.
- The "job running, config removed" case is an edge case developers forget to spec.

**How to avoid:**
1. **Debounce file-watch events.** Use `notify-debouncer-mini` or a manual 500ms debounce. Apply reload only after the file has been stable for the debounce window.
2. **Parse to a staging structure first.** If parsing fails, log the error, keep the OLD config. NEVER partially apply a new config.
3. **Diff-based apply, not replace.** Compute a diff: `{added, updated, removed}`. Apply in order: updates (that don't change schedule) → additions → removals.
4. **Define "removal of a running job" semantics explicitly:**
   - Running job's `enabled` flag goes to `false`.
   - `next_run_at` set to `NULL`.
   - Currently running execution is allowed to complete (not killed).
   - On completion, run is recorded normally. Job row is marked `removed_at = now()`.
   - History is preserved (per spec's "preserve history for removed jobs").
   - Document this in the "Config reload" section of README.
5. **Atomic apply under a lock.** Use `tokio::sync::Mutex` around the job-set state. Reload takes the lock, applies the diff, releases. A scheduler tick that wants to spawn a new run also takes the lock briefly to read `next_run_at`. Never reload mid-spawn.
6. **Fail reload loudly if parsing partial.** Expose `cronduit_config_reload_errors_total` metric AND surface the last reload error in the UI "Settings" page so operators see it without grepping logs.
7. **SIGHUP AND file watch AND `POST /api/reload` must all use the same code path.** Three entry points, one reload function.
8. **Test with an editor that does truncate-write** (vim with default swap, VSCode with atomic save off) to confirm debouncing actually works.

**Warning signs:**
- File watcher without debounce.
- Config reload implemented as "clear state, re-parse, re-apply."
- Running jobs stopping mid-execution on reload.
- UI "Settings" page has no reload status or last-error field.

**Phase to address:** Phase 2 (scheduler core) for reload semantics; Phase 5 (UI) to surface status.

**Severity:** HIGH

---

### Pitfall 10: Restart-during-execution semantics are undefined and leak orphaned containers

**What goes wrong:**
Cronduit is executing job `long-backup` (a 3-hour job). Operator `docker restart cronduit`. What happens?
- The Cronduit process receives SIGTERM. Running job container is still alive (it was spawned independently via Docker API).
- Without a graceful shutdown handler, Cronduit exits. The running backup container keeps going, but no one is watching its logs, recording its exit code, or cleaning up.
- Cronduit restarts. The DB still says `status=running` for the old execution. The scheduler sees "next run in 1 hour" and may spawn ANOTHER backup concurrent with the orphan.
- The orphan container finishes. Nothing records its outcome. It either auto-removes (losing exit code per Pitfall 3) or sticks around forever as `ghost-backup-abc123`.

**Why it happens:**
Cronduit and the jobs it spawns are separate processes (different containers, even). The scheduler's lifecycle is NOT the jobs' lifecycle. Developers instinctively think "when I exit, my children exit" — not true with bollard.

**How to avoid:**
1. **Graceful shutdown with bounded wait.** On SIGTERM: stop scheduler ticks, wait up to `shutdown_timeout` (configurable, default e.g. 60s) for running jobs to finish, then either (a) kill remaining containers via `docker kill`, or (b) exit and leave them orphaned, recording their state as `status=abandoned` with a note. Policy should be configurable; default to "leave them running" (safer for data integrity) and reconcile on restart.
2. **Label every spawned container.** At create time, set label `cronduit.run_id=<uuid>` and `cronduit.instance=<cronduit_instance_id>`. This is the reconciliation key.
3. **Startup reconciliation.** Before the scheduler starts, query Docker for all containers with `label=cronduit.run_id`. For each:
   - If container is `running` and matches a DB row with `status=running`, re-attach log streaming and wait.
   - If container is `exited` and DB row is `running`, collect exit code and finalize the run.
   - If container doesn't match any DB row, log warning and remove (after configurable grace period, to avoid nuking a valid concurrent run from a manually started Cronduit).
   - If DB row says `running` but no container exists, mark `status=lost` with a note.
4. **Persist a "cronduit run started" record BEFORE the container is created,** not after. If the create call succeeds and Cronduit crashes before persisting, reconciliation finds an orphan container that doesn't match any DB row → cleanup path.
5. **Test restart-during-execution explicitly.** Integration test: start a 10s sleep job, kill Cronduit mid-run, restart, assert the run is either resumed or cleanly finalized.
6. **Document the semantics** in README: "Cronduit will wait up to N seconds for running jobs on shutdown. Longer-running jobs will continue to run and be reconciled on next startup."

**Warning signs:**
- No `cronduit.run_id` label on spawned containers.
- No reconciliation step at startup.
- DB rows stuck in `status=running` forever after a crash.
- Orphan containers with job names visible in `docker ps`.

**Phase to address:** Phase 3 (execution) for the label + shutdown handler; Phase 4 (persistence) for reconciliation logic.

**Severity:** HIGH

---

## High Pitfalls

### Pitfall 11: Log retention pruning is correct in isolation but breaks under load

**What goes wrong:**
`DELETE FROM job_logs WHERE captured_at < now() - interval '90 days'` runs on a schedule. On SQLite with 10M log rows, this single `DELETE` holds the write lock for 30 seconds, blocking all other writes and causing scheduler ticks to fail with `SQLITE_BUSY`. On Postgres, it works but bloats the table (no `VACUUM`). Across both: the foreign key cascade from `job_logs → job_runs → jobs` may or may not delete run metadata the operator expected to keep.

**Why it happens:**
Retention pruning is easy to write ("just a DELETE") and hard to operationalize. A naive DELETE on a hot table interacts badly with the write-serialization from Pitfall 7.

**How to avoid:**
1. **Batch deletes:** `DELETE FROM job_logs WHERE id IN (SELECT id FROM job_logs WHERE captured_at < ? LIMIT 1000)` in a loop with small sleeps between batches. Releases the lock between batches.
2. **Prune logs separately from runs.** Retention of raw log blobs can be shorter (e.g., 30 days) than retention of run metadata (e.g., 365 days). Config exposes both: `log_retention` and `run_retention`.
3. **Don't cascade delete runs when logs are deleted.** Logs have an FK to runs, not the other way around. Runs persist; logs are garbage.
4. **Schedule pruning at low-activity time** (e.g., 04:30 local). Skippable if a scheduler tick is due within the next minute.
5. **Periodic `VACUUM`/`PRAGMA optimize` on SQLite** after large prunes.
6. **Metric for prune duration and rows affected.**

**Warning signs:**
- Single unbounded `DELETE` in pruning code.
- Prune runs on scheduler tick (not on a separate task).
- DB file grows unbounded despite retention config.

**Phase to address:** Phase 4 (persistence).

**Severity:** HIGH

---

### Pitfall 12: Image pull failures have no retry, no caching guidance, and fail the whole run

**What goes wrong:**
Job specifies `image = "myapp:latest"`. On first run, image not present → Cronduit calls `image_create` via bollard. Docker Hub is flaky for 5 seconds. Pull fails. Run marked failed. Operator sees "failed" with no indication of whether it was their code or a transient network issue. Next tick, pull is retried from scratch, succeeds, run succeeds. Now they have a false-failure in history.

Worse: `latest` tag means the pulled image silently changes; yesterday's successful run used `myapp@sha256:abc`, today's used `myapp@sha256:def`. Root-causing a regression becomes archaeology.

**Why it happens:**
- "Pull on not-present" is easy; "pull with retry and structured error classification" is not.
- `:latest` is a Docker anti-pattern but users use it everywhere. A scheduler that doesn't surface the resolved digest makes it worse.

**How to avoid:**
1. **Retry pulls with exponential backoff** (e.g., 3 attempts: 1s, 5s, 25s). Classify failures: network/timeout → retry; `manifest unknown` / `unauthorized` → no retry, fail fast.
2. **Distinct failure category:** `failures_total{reason="image_pull_failed"}` — separate from script-level failures.
3. **Resolve and record image digest** after pull. Store both the requested reference (`myapp:latest`) and the resolved digest (`sha256:...`) on the run row. Surface in UI.
4. **Pull-ahead option:** `[defaults] prepull = true` pulls images on config load so first-run lag doesn't surprise the operator. Failures on prepull are warnings, not hard errors.
5. **Document** that `:latest` is discouraged and Cronduit will surface the digest to mitigate.
6. **Structured error in UI** that distinguishes "image pull failed" from "container ran and failed" — these have completely different operator responses.

**Warning signs:**
- Single `image_create` call with no retry.
- Image pull error and script exit code 1 look identical in the UI.
- No `image_digest` column on `job_runs`.

**Phase to address:** Phase 3 (execution).

**Severity:** HIGH

---

### Pitfall 13: ANSI color codes, binary output, and huge lines render as garbage (or XSS) in the web UI

**What goes wrong:**
A job uses `curl -v` or any modern CLI with colorized output. Cronduit stores raw bytes including `\x1b[` escape sequences. The web UI renders them as literal text: `^[[31mfailed^[[0m` instead of red "failed." Or worse: the UI naively inserts log text into HTML, and a log line containing `<script>alert(1)</script>` executes in the operator's browser.

Additionally: a job dumps a 500 KB binary blob to stdout. The run detail page loads 500 KB of garbage into the DOM and the browser hangs.

**Why it happens:**
- Logs are byte streams, not text. UI developers reach for "text = innerHTML" and get both rendering bugs and XSS.
- ANSI escape handling requires a real parser; naive regex stripping loses formatting info entirely.

**How to avoid:**
1. **Always HTML-escape log content.** Use Askama/Maud's default escaping (do NOT use `{{ raw }}` / `PreEscaped`). This prevents XSS regardless of what a job writes.
2. **Parse ANSI escapes server-side** (crate: `ansi-to-html` or `anser-rs`) and emit safe `<span class="ansi-red">` with a fixed palette defined in Tailwind. Do NOT allow arbitrary CSS or background colors that could mimic UI chrome.
3. **Replace non-printable/binary bytes** with `·` or `\x{NN}` hex notation. Detect binary chunks (high ratio of non-printable bytes) and collapse them: `[binary: 102400 bytes]` with an optional download link.
4. **Line-length cap in UI:** long lines truncated to (e.g.) 2000 chars per line in the DOM, with "show more" to fetch the full line on demand. Backend serves full bytes via an explicit `/api/runs/:id/logs/raw` endpoint (clearly labeled "raw, may be dangerous to render").
5. **Pagination for long runs.** Never load 100K log lines into the DOM at once. SSE streams the tail; "load older" fetches pages.
6. **XSS test case** in CI: a job writes `<script>alert(1)</script>` to stdout, UI must render it as visible text, not execute it. Also test `<img src=x onerror=...>`, `javascript:` URLs.

**Warning signs:**
- Template uses `|safe` or `PreEscaped` on log content.
- No ANSI parsing → escape sequences visible in UI.
- Run detail page loads slowly for chatty jobs.
- No XSS test in CI.

**Phase to address:** Phase 5 (web UI). This is a table-stakes correctness issue for any log viewer.

**Severity:** HIGH

---

### Pitfall 14: Single-binary cross-compile works on dev, breaks on arm64/musl because of sqlx + OpenSSL

**What goes wrong:**
Dev loop is `cargo build` on x86_64 macOS/Linux with glibc + system OpenSSL. First multi-arch Docker build fails on linux/arm64 with `openssl-sys failed`, or on musl with `undefined reference to __gmon_start__`, or produces a binary that dynamically links glibc and segfaults on Alpine. sqlx-macros complicates this because it runs at build time and links differently from the final binary.

**Why it happens:**
- `openssl-sys` is the single largest source of cross-compilation pain in the Rust ecosystem.
- `sqlx-macros` links against `openssl-sys` at compile time (procedural macros are shared libs using the host's libc), while the final sqlx-using binary links against `openssl-sys` statically — you end up needing two OpenSSL builds.
- bollard historically pulled in `hyper-tls` which pulls OpenSSL. `bollard` now supports `ssl_providerless` and rustls features.
- multi-arch Docker builds amplify all of this.

**How to avoid:**
1. **Use rustls everywhere.** Enable `bollard` with `ssl_providerless` or `rustls` feature, configure sqlx with `runtime-tokio-rustls`, and make sure no transitive dep pulls `openssl-sys`. Run `cargo tree -i openssl-sys` to verify — it should return nothing.
2. **Prefer `aws-lc-rs`** as a `rustls` crypto provider, or `ring` — both pure-Rust-ish and cross-compile cleanly.
3. **Build in a container, not on the dev host.** `cross` or a purpose-built multi-stage Dockerfile (builder image = `rust:slim`, runtime image = `gcr.io/distroless/cc` or `alpine` depending on musl vs glibc choice).
4. **Commit to ONE libc story.** Either:
   - musl + fully static (smallest, Alpine-friendly, but DNS edge cases with musl's resolver); OR
   - glibc + dynamically linked against a small base (debian:slim, distroless). Pick one; don't ship both without tests.
5. **CI matrix:** build linux/amd64 AND linux/arm64 on every PR using `docker buildx`. Integration-test the arm64 image under QEMU at least on release.
6. **`cargo deny`** to forbid `openssl-sys` transitively if you chose rustls. Prevents regression from a new dep.
7. **Strip and UPX-avoid:** `strip` the binary; do NOT use UPX (breaks in containers and antivirus).

**Warning signs:**
- `cargo tree -i openssl-sys` returns any results.
- Docker build runs `apt-get install libssl-dev` in the runtime stage (it shouldn't need to).
- No arm64 in CI matrix.
- Binary fails to run on a vanilla Alpine/Debian container.

**Phase to address:** Phase 1 (project setup) — the dep choices (rustls vs OpenSSL, musl vs glibc) must be made at `Cargo.toml` creation time. Phase N (release engineering) — multi-arch CI + image publishing.

**Severity:** HIGH

---

### Pitfall 15: "Zero config" defaults work for the author but surprise adopters

**What goes wrong:**
- Default DATABASE_URL points to `/data/cronduit.db` — fine in the example compose, broken if the user doesn't mount `/data`.
- Default log retention is 90 days — fine for a light homelab, fills the disk on a chatty workload.
- Default bind address is `0.0.0.0:8080` — fine for author who's behind NAT, catastrophic for a user with port-forwarding.
- Default timezone is host timezone (not UTC) — "works on my machine."
- Default config file path is `/etc/cronduit/config.toml` — fine in container, weird for bare-metal.
- Default scheduler grace period is 0 — first SIGTERM kills everything.
- Default SQLite pragmas are sqlx defaults — see Pitfall 7.

**Why it happens:**
Every default encodes an assumption about the operator's environment. Authors test in one environment and ship those assumptions. OSS users have wildly different environments.

**How to avoid:**
1. **`cronduit --check` subcommand.** Validates config, reports all effective settings (with source: default / env / config file), and warns on risky combos. Runs on first startup and can be invoked manually.
2. **Startup summary log:**
   ```
   cronduit 0.1.0 starting
     bind: 127.0.0.1:8080 (default)
     database: sqlite:///data/cronduit.db
     timezone: UTC (default)
     config: /etc/cronduit/config.toml
     log_retention: 90d
     jobs: 12 loaded, 0 disabled, 0 errors
   ```
3. **Fail loudly on suspicious defaults** (see Pitfall 1 for `0.0.0.0` + no auth).
4. **"Known good" sample configs** in `examples/` for the three canonical deployments: (a) single-machine homelab via compose; (b) multi-service homelab with reverse proxy; (c) bare-metal systemd.
5. **No silent fallback.** If a user specifies `database_url = "postgres://..."` but the host isn't reachable, fail startup with a clear error, do not fall back to SQLite.
6. **Panic-on-startup is OK; panic-mid-operation is not.** Config problems at startup should fail fast with human-readable errors. After startup, panics are bugs — use `?` and structured errors everywhere.

**Warning signs:**
- Config file reference doc has no "Defaults" column.
- First-time user issues are "where does the DB go?" or "why isn't it listening?"
- No `--check` or equivalent validation command.

**Phase to address:** Phase 1 (CLI + config loading); Phase N (release prep) for sample configs.

**Severity:** HIGH

---

### Pitfall 16: Manual "Run Now" bypasses scheduling semantics and corrupts @random / overlap state

**What goes wrong:**
Operator clicks "Run Now" on a job that uses `@random` and has `min_gap`. The manual run executes at wall-clock time T, but T is not in the resolved schedule. Did it consume today's slot? Does the next scheduled run still happen at the resolved time? If the job has concurrency policy "skip if running," does Run Now queue or reject?

Or: Operator clicks "Run Now" twice in quick succession. Cronduit spawns two parallel container instances of the same job — neither the UI nor the scheduler has any opinion on this, and the job isn't idempotent.

**Why it happens:**
"Run Now" is tempting to implement as "call the execution function directly, bypassing the scheduler." But the scheduler is the thing that enforces invariants.

**How to avoid:**
1. **Run Now goes through the same execution path as scheduled runs.** A `manual_trigger` event is submitted to the scheduler, which evaluates: is the job currently running? Is concurrency policy "skip"? If so, return 409 with a clear message ("job already running, triggered at X, ETA Y").
2. **Explicit tag on runs:** `trigger = "scheduled" | "manual" | "reload"`. Surface in UI. Manual runs are visually distinguished so operators don't confuse them with scheduled ones.
3. **Manual runs do NOT consume `@random` slots.** They're out-of-band by definition. The next scheduled random run still happens at its resolved time.
4. **Button debounce + idempotency token.** The UI generates a UUID per click and submits it as an idempotency key. Repeat submissions within a window return the same run ID instead of spawning a new container.
5. **Log "manual trigger" as an audit event** with wall-clock time and (in v2+) the user identity.

**Warning signs:**
- "Run Now" code path is a separate function from scheduled execution.
- No `trigger` column on runs.
- Double-clicking "Run Now" spawns two containers.

**Phase to address:** Phase 5 (UI) + Phase 2 (scheduler interface that both paths share).

**Severity:** MEDIUM

---

### Pitfall 17: Prometheus metrics cardinality explodes with per-job labels

**What goes wrong:**
Cronduit exposes `cronduit_runs_total{job="..."}`, `cronduit_run_duration_seconds{job="..."}`, `cronduit_failures_total{job="...", reason="..."}`. A homelab user has 50 jobs × 8 failure reasons = 400 timeseries just for failures — fine. An adopter with an auto-generated config of 5000 jobs × 8 reasons = 40k timeseries, plus histograms multiply further. Prometheus scrapes become slow; a shared Prometheus instance suffers.

Worse: if `reason` is a free-form error string, cardinality is unbounded.

**Why it happens:**
Metrics are easy to add and hard to remove. Labels feel free.

**How to avoid:**
1. **Cap `reason` to a closed enum:** `image_pull_failed`, `network_target_unavailable`, `timeout`, `exit_nonzero`, `abandoned`, `unknown`. Never pass raw error strings as label values.
2. **Document recommended scrape interval** (15-60s) and warn that high cardinality is the operator's responsibility if they have 1000+ jobs.
3. **Consider summary/histogram budgets.** `cronduit_run_duration_seconds` as a histogram has N_buckets × N_jobs series; if this is a problem, use a summary or global histogram without job label.
4. **Don't emit per-run labels** (no `run_id` in a label). Per-run data belongs in logs/DB, not metrics.
5. **Test with 1000 dummy jobs:** assert `/metrics` responds in <500ms and response size is reasonable.

**Warning signs:**
- `reason` label accepts `e.to_string()` from an error.
- `run_id` appears anywhere in a metric label.
- `/metrics` response is many megabytes.

**Phase to address:** Phase 6 (operational / metrics).

**Severity:** MEDIUM

---

## Medium Pitfalls

### Pitfall 18: Environment variable interpolation is naive and leaks secrets into error messages

**What goes wrong:**
Config uses `env = { API_KEY = "${PROD_API_KEY}" }`. Cronduit interpolates via shell-style expansion. At some point, a parse error or validation error logs the resolved config structure — including `API_KEY="sk-actual-secret"` — to stdout, which ends up in Docker logs, which ends up in the operator's log aggregator, which ends up shared with a colleague. Secret leaked.

**Why it happens:**
- "Print the config we loaded" is helpful during debugging and becomes a landmine when it includes interpolated secrets.
- Error messages that include "context" often stringify surrounding state.

**How to avoid:**
1. **Secret-bearing fields are a distinct type** (`SecretString` or similar) whose `Debug` / `Display` impl prints `[redacted]`. Only the job-execution path may call `.expose_secret()`.
2. **Audit all logging.** `#[derive(Debug)]` on a `JobConfig` struct must use the secret type, or customize `Debug` to skip `env`.
3. **Structured errors never include the resolved env map** — they include job name and field name, nothing more.
4. **Interpolation missing-var policy:** if `${FOO}` is unset, fail loudly at config-load time. Do not default to empty string (silent leak of an unset var into a command line).
5. **Document:** "Cronduit does not log secret values. Do not include secrets in job `name` or `command` fields directly; use `env` and interpolate from the host environment."

**Warning signs:**
- `tracing::info!("loaded config: {:?}", config)` anywhere in the code.
- `env` map prints in full on any error path.
- Missing env vars silently become empty strings.

**Phase to address:** Phase 2 (config model) and Phase 3 (execution path).

**Severity:** MEDIUM

---

### Pitfall 19: Graceful shutdown timeout is a guess; operator has no control

**What goes wrong:**
`shutdown_timeout = "60s"` hardcoded. A backup job takes 3 hours. SIGTERM → 60s wait → kill. Corrupted backup.

Or the reverse: `shutdown_timeout = "1h"` hardcoded. Operator wants to bounce Cronduit for a quick config swap, has to wait an hour because there's a long job running they didn't notice.

**Why it happens:**
Shutdown timeout is a global default that must work for workloads you can't predict.

**How to avoid:**
1. **Configurable globally AND per-job.** `[defaults] shutdown_grace = "30s"`, `[[jobs]] shutdown_grace = "2h"` for known long runners.
2. **Respect SIGTERM from Docker honestly.** Docker sends SIGTERM and waits `stop_grace_period` (default 10s) before SIGKILL. If Cronduit needs more, operator sets `stop_grace_period: 2h` in compose.
3. **Log what shutdown is waiting for** every 5s during shutdown: `"shutdown: waiting for 2 running jobs (longest: backup-daily, 14m)"` — operator can see what's holding things up.
4. **Second SIGTERM = immediate kill** (a la docker compose behavior). Operators know this pattern.
5. **State machine for shutdown:** `Running → Draining (no new jobs) → Waiting (running jobs finishing) → Killing (timeout hit) → Stopped`. Each transition logged.

**Warning signs:**
- Hardcoded shutdown timeout.
- Silent shutdown with no progress logs.
- No way to override per-job.

**Phase to address:** Phase 2 (scheduler shutdown) and Phase 6 (operational).

**Severity:** MEDIUM

---

### Pitfall 20: TOML vs YAML decision pushed to users as "support all the things"

**What goes wrong:**
Spec says "pick one as primary, consider supporting multiple." Cronduit tries to support TOML AND YAML AND JSON. Three parsers, three test suites, three error message styles, three config schema docs. Users hit edge cases where the same semantic config is valid in one and invalid in another. Maintenance burden triples.

**Why it happens:**
"Why not support both?" sounds polite but is a maintenance trap.

**How to avoid:**
1. **Ship v1 with TOML only.** Document the decision: Rust-native parsing, strong typing, clean for hand-written files, matches `Cargo.toml` aesthetic.
2. **If YAML is demanded by users post-v1,** add it as an explicit second format with its own parser but same internal representation. Do NOT try to auto-detect format from file extension silently; require `config_format = "toml"` or `"yaml"` explicitly, or gate on file extension with an error if mismatched.
3. **No INI, no JSON.** JSON is hostile to hand-editing (no comments in standard JSON). INI is too limited for nested jobs.
4. **Document the decision in `docs/decisions/001-config-format.md`** so future contributors don't re-litigate.

**Warning signs:**
- Multi-parser code in Phase 1.
- Issue tracker full of "why doesn't my YAML work the same as TOML" questions.

**Phase to address:** Phase 1 (config format decision) — this is a research-phase finding that should be LOCKED before Phase 2 starts.

**Severity:** MEDIUM

---

### Pitfall 21: Running Cronduit in Docker means the "host" commands from `type = "command"` don't mean what users think

**What goes wrong:**
Spec supports `type = "command"` to run local shell commands. But Cronduit itself runs inside a container. "Local" means "inside the Cronduit container," which has its own filesystem, no host binaries, no host cron, no host mounts unless explicitly shared. An operator writes `command = "/usr/local/bin/backup.sh"` expecting it to run on the host; it fails with "not found" because `/usr/local/bin/backup.sh` is on the host, not in the Cronduit image.

**Why it happens:**
The mental model of "a cron daemon that runs commands" collides with the reality of containerization. Users who've used cron or ofelia expect host context.

**How to avoid:**
1. **Document prominently** that `type = "command"` runs inside the Cronduit container, not the host. Headline in README.
2. **Recommend Docker execution (`type = "docker"`) for anything that needs host resources.** Cronduit's strength is Docker execution; push users toward it for host-adjacent work.
3. **`type = "host-command"` is NOT a feature.** Running arbitrary host commands from inside a container without SSH or a side-channel is a whole mess of pseudo-security ("mount the host filesystem and chroot?") that should be explicitly out of scope.
4. **Ship a "baseline tools" image** as the Cronduit base: `curl`, `wget`, `sh`, `bash`, `jq`, `coreutils`. Users can run `curl -sf https://...` inline without spawning a container. Document what's present.
5. **Allow users to extend the base image** via a Dockerfile: `FROM cronduit:latest\nRUN apt-get install -y ...`. Document this pattern.
6. **If a user needs host commands, they should shell them out via a Docker container with the host rootfs mounted** — their call, their responsibility, documented example.

**Warning signs:**
- README doesn't distinguish "local command" from "host command."
- First adopter issues are "why can't my script find `rsync`?"

**Phase to address:** Phase 3 (execution) + Phase N (release prep README).

**Severity:** MEDIUM

---

### Pitfall 22: Scheduler clock drift inside a container that wasn't time-synced

**What goes wrong:**
Cronduit runs inside a Docker container on a NAS that doesn't run NTP. The container inherits the host clock, which drifts by 10 minutes over a week. Scheduled jobs fire 10 minutes late. After a host reboot, the clock jumps. Jobs that were due during the skipped time either all fire at once (catch-up) or disappear (skipped).

**Why it happens:**
Containers inherit the host clock. Hosts without NTP drift. `tokio::time::sleep(next_run_at - now)` doesn't notice.

**How to avoid:**
1. **Use wall-clock scheduling, not monotonic sleep.** On each tick, compute "the next due job at the current wall clock." Don't pre-compute sleeps against monotonic time.
2. **Clock-jump detection.** If `now` moves backwards or forwards by more than 2 minutes between ticks, log a warning and re-evaluate all schedules.
3. **Don't catch up missed runs by default.** If Cronduit wakes up and sees 14 missed runs for a job, firing all 14 is usually wrong. Default: fire once with `catch_up=false` and log the skip count. Configurable per-job: `catch_up = "all" | "one" | "none"` (default `one`).
4. **Document** that Cronduit assumes a reasonably stable clock and users should run NTP. Recommend the host's timesyncd or similar.
5. **Health endpoint includes clock info:** `{"now": "...", "drift_detected": false}`.

**Warning signs:**
- Jobs firing 14 times in quick succession after a resume.
- Jobs missing over long periods after host reboot.
- No clock-jump handling in scheduler tick.

**Phase to address:** Phase 2 (scheduler core).

**Severity:** MEDIUM

---

### Pitfall 23: Embedded static assets (`rust-embed`) break hot reload and bloat binary

**What goes wrong:**
- `rust-embed` in production bakes assets into the binary. In dev, it reads from disk for hot reload. Bug: dev and prod render differently, and a release build with assets missing from the src tree silently ships an empty UI.
- Tailwind CSS compiled at build time bloats the binary by 200-500 KB of unused classes if purge isn't configured.
- Favicons, logos, banners add another few hundred KB.

**Why it happens:**
- `rust-embed` is great but has gotchas with release-vs-debug behavior.
- Tailwind's default CSS is 3+ MB; the JIT/purge step is mandatory for reasonable bundle size.

**How to avoid:**
1. **rust-embed with `#[folder = "..."]` and `debug-embed` feature enabled** so dev and prod behavior match — both read from the embedded tree, not from disk. Trades hot reload for correctness.
2. **Tailwind JIT/purge configured against Askama/Maud templates.** Verify the compiled CSS is <100 KB after purge.
3. **Optimize images.** SVG for logos and favicons (small, scalable). Compress PNG banners.
4. **Check binary size in CI.** Fail if binary grows by >5 MB between PRs without justification.
5. **Serve assets with `Cache-Control: max-age=31536000, immutable`** and a hash in the filename (cronduit.abcdef.css) for cache busting across releases.

**Warning signs:**
- Binary size >50 MB.
- Compiled CSS >500 KB.
- Dev UI renders differently from release UI.

**Phase to address:** Phase 5 (UI build pipeline).

**Severity:** LOW-MEDIUM

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Use `auto_remove=true` instead of explicit remove state machine | Simpler execution code | Lost exit codes and truncated logs on a non-trivial fraction of runs (Pitfall 3) | Never — it breaks the product's core promise |
| Single `DELETE` for log pruning instead of batched | One line of SQL | Scheduler stalls during prune on SQLite; user-visible failures | Only if log table has <10k rows and staying that way |
| Skip DST regression tests | Ship faster | 100% chance of a public embarrassment every March and November | Never |
| `bind = "0.0.0.0:8080"` default | "Works out of the box" | First-day adopter accidentally exposes root to internet | Never |
| Hand-rolled `@random` slot allocation without feasibility check | Simpler math | Infinite loop or silent constraint violation on edge cases (Pitfall 6) | Never — feasibility is a one-time O(n) check |
| Use `cron` crate instead of `croner` | Already in Cargo.toml | Subtle DST + weekday numbering bugs | Only if the test suite explicitly verifies DST and Sunday=0 |
| Single sqlx connection pool for reads and writes | Less code | Write contention collapses throughput (Pitfall 7) | Never with SQLite; acceptable with Postgres |
| Log line as "one DB row per newline" | Simple schema | Breaks on huge lines; 10× write overhead for chatty jobs | Only for very low-volume use cases with hard line-length cap |
| `tracing::info!("config: {:?}", config)` | Great debugging | Secret leak into log aggregators (Pitfall 18) | Never for anything containing `env` |
| Separate code paths for "Run Now" and scheduled runs | UI button is a one-liner | Divergent semantics, doubled bugs (Pitfall 16) | Never |
| "Support TOML, YAML, and INI" in v1 | Flexibility | 3× maintenance, user confusion | Never in v1 |
| Synchronous config reload (no debounce) | Fewer moving parts | Editor save events cause 5 reloads, partial files parsed (Pitfall 9) | Never |

---

## Integration Gotchas

Common mistakes when connecting to external services.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Docker socket (bollard) | Using `auto_remove=true` and expecting `wait` to still give exit code | Create without auto_remove; wait; collect logs; explicit remove |
| Docker socket (bollard) | `container:<name>` without pre-flight checking target is running | Inspect target first; record distinct failure reason |
| Docker socket (bollard) | Pull image once, never retry transient failures | Exponential backoff, classify retryable vs. fatal errors |
| Docker socket (bollard) | Not setting labels on spawned containers | Set `cronduit.run_id` label for reconciliation |
| SQLite (sqlx) | Default connection pool for writes | Single-connection write pool + multi-connection read pool |
| SQLite (sqlx) | No `busy_timeout` | `PRAGMA busy_timeout = 5000` on every connection |
| SQLite (sqlx) | No WAL mode | `PRAGMA journal_mode = WAL` on every connection |
| Postgres (sqlx) | Using JSONB/enum/array types that don't port to SQLite | Lowest-common-denominator schema OR per-dialect migrations |
| Prometheus metrics | Free-form strings as label values | Closed enum of reason codes; no run_id in labels |
| File watching (notify) | Raw events without debounce | Debounce 500ms; parse to staging; atomic apply |
| TLS (rustls vs openssl) | Mixing both → cross-compile breakage | Commit to rustls; `cargo tree -i openssl-sys` = empty |
| HTMX / SSE | Slow viewer blocks log stream | Broadcast channel to viewers; DB writer on separate task |
| HTMX templates (Askama/Maud) | `|safe` on user content | Always escape; parse ANSI to sanitized spans |

---

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Unbounded log channel | RSS grows with job output; OOM-kill | Bounded channel with drop policy + marker (Pitfall 4) | First job that emits >200 MB of stdout |
| Per-line DB inserts | High CPU on SQLite; contention | Batch 500 lines or 250ms in a transaction | ~100 lines/sec sustained |
| Single DELETE for retention | Scheduler ticks fail with SQLITE_BUSY | Batched delete with sleeps | ~1M log rows |
| No WAL checkpoint strategy | `*.db-wal` file grows unbounded | Periodic `PRAGMA wal_checkpoint(TRUNCATE)` | Any long-lived reader |
| Sync reload on every file event | 5× parse per save | Debounce 500ms | Any editor with atomic-save |
| Histogram metric per job label | `/metrics` becomes multi-MB | Consider global histogram or cap jobs per instance | ~1000 jobs |
| Loading all log lines into DOM | Browser freeze on run detail | Paginate; SSE tail only | Any run with >10k lines |
| No connection pool for reads | Dashboard sluggish under load | Multi-connection read pool | First user who opens multiple tabs |
| Polling Docker API every second | CPU pegged; daemon pressure | Event-driven via `events` API | Always — don't poll in v1 |
| Serialized log writers per run | Per-run throughput cap | Single writer task, multiplexed | ~5 concurrent chatty jobs |

---

## Security Mistakes

Domain-specific security issues beyond general web security.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Mounting docker.sock read-only and thinking it's safe | False sense of security; still exploitable | Document honestly: RW + no socket mitigations; treat as root-equivalent |
| Default bind `0.0.0.0:8080` with no auth | Accidental internet exposure = host compromise | Default bind `127.0.0.1`; require opt-in for external bind |
| Interpolating `${VAR}` into command strings | Shell injection if VAR contains shell metacharacters | Never build command strings; pass args as arrays; document `env` uses exec not shell |
| Logging full resolved config | Secret leak into log aggregators | `SecretString` type; custom Debug that redacts `env` |
| Running jobs as root inside their containers | Breakout via kernel vuln | Document `user:` field; encourage non-root job containers (doesn't mitigate socket; still defense-in-depth) |
| Web UI accepts unauthenticated "Run Now" | Any network-adjacent actor can run arbitrary containers = host root | Default LAN-only bind; CSRF token on POST; document reverse proxy + auth |
| CSRF on state-changing endpoints | XSS in a log line → forced Run Now from victim's browser | CSRF token on all POST/DELETE; SameSite=strict cookies |
| Log content rendered as HTML | XSS via job stdout | Escape by default; parse ANSI server-side to sanitized spans |
| Config file writable by non-root | Privilege escalation to full host root via container definition | Document `read_only: true` mount; `:ro` on compose |
| `command = "/host/..."` illusion | Users think they're running host commands from the sandbox | Explicit documentation of container-scoped execution; no `host-command` feature |
| Auth deferred without loud warnings | Users exposing v1 to the internet | Mandatory startup warning; bind default 127.0.0.1; README SECURITY at top |
| Image digest not recorded | Supply chain drift invisible | Record resolved digest per run; surface in UI |

---

## UX Pitfalls

Common user experience mistakes in this domain.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| `@random` shows as "random" in UI without resolved value | User can't plan around it, thinks it's random at run time | Show: "Today: 14:17 (resolves 00:00 daily)" with manual re-roll action |
| Generic "failed" status for all failures | User can't distinguish script bug from network/Docker issue | Distinct categories: `exit_nonzero`, `image_pull`, `network_target`, `timeout`, `abandoned` |
| Timezone shown only in local time on DST day | Operator can't debug what actually ran | Always show local + UTC together on run detail |
| Run detail page loads all logs at once | Browser hangs on chatty jobs | Paginate; tail via SSE; "load older" button |
| No indication a config reload failed | Silent config desync; operator believes their edit is live | Surface last reload status on Settings page; metric; log |
| Job history has no search/filter | Debugging is scrolling through a huge table | Filter by status, date range, job; default to last 100 |
| Manual "Run Now" is indistinguishable from scheduled runs in history | Audit trail is confusing | `trigger` column/badge: scheduled/manual/reload |
| No "next run" countdown | User stares at the page wondering if scheduler is alive | Live-updating "next run in 4m 12s" with last-tick timestamp |
| Truncated logs with no indication | User thinks the log is complete | Explicit `log_truncated=true` banner; link to raw download |
| Error banners auto-dismiss | User misses errors in HTMX updates | Errors persist until dismissed; show error count in header |
| No "healthy"/"degraded" top-level indicator | User doesn't know if scheduler itself is OK | Health badge in header; green/yellow/red; link to Status page |
| Design system not applied to error states | Errors look unstyled, feel broken | Terminal-red treatment consistent with success/pending states |

---

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **Docker execution:** Looks done — spawns containers successfully. Often missing: explicit remove state machine (Pitfall 3), `cronduit.run_id` labels (Pitfall 10), pre-flight for `container:<name>` (Pitfall 2), image pull retry (Pitfall 12), digest recording.
- [ ] **Cron parsing:** Looks done — parses "0 2 * * *" correctly. Often missing: DST tests (Pitfall 5), timezone configuration, clock-jump handling (Pitfall 22), catch-up policy.
- [ ] **`@random`:** Looks done — picks a random time. Often missing: persistence of resolution (Pitfall 6), feasibility check, deterministic re-roll cadence, UI surfacing, manual re-roll action.
- [ ] **Log capture:** Looks done — logs appear in DB. Often missing: bounded channel (Pitfall 4), batch inserts, drop markers, truncation cap, ANSI parsing (Pitfall 13), XSS escaping.
- [ ] **Config reload:** Looks done — SIGHUP reloads. Often missing: file-watch debounce (Pitfall 9), atomic apply, running-job semantics, surface-in-UI of last reload status, partial-parse rollback.
- [ ] **SQLite persistence:** Looks done — queries work. Often missing: WAL mode, `busy_timeout`, separate read/write pools (Pitfall 7), batched retention pruning (Pitfall 11), WAL checkpointing.
- [ ] **Graceful shutdown:** Looks done — SIGTERM stops the process. Often missing: wait for running jobs (Pitfall 10), label-based reconciliation on restart, progress logs during shutdown (Pitfall 19), configurable per-job grace.
- [ ] **Metrics:** Looks done — `/metrics` returns Prometheus text. Often missing: closed-enum label values (Pitfall 17), cardinality budget, scrape-time test, clock info in health.
- [ ] **Web UI:** Looks done — dashboard renders. Often missing: CSRF tokens, XSS escaping (Pitfall 13), log pagination, error persistence, timezone display, "no auth" banner.
- [ ] **Security docs:** Looks done — README mentions Docker socket. Often missing: THREAT_MODEL.md, "not for internet exposure" banner, default-bind-to-localhost (Pitfall 1), docker-compose example with `expose` not `ports`.
- [ ] **Multi-backend DB:** Looks done — SQLite and Postgres both work in dev. Often missing: CI matrix across both (Pitfall 8), per-dialect migrations, schema parity docs.
- [ ] **Cross-compile:** Looks done — `cargo build` succeeds on dev machine. Often missing: arm64 CI (Pitfall 14), `cargo tree -i openssl-sys` check, musl vs glibc decision, multi-arch image publishing.
- [ ] **Manual Run Now:** Looks done — button triggers run. Often missing: shared path with scheduler (Pitfall 16), idempotency token, distinct `trigger=manual` tag, concurrency policy enforcement.
- [ ] **Tests:** Looks done — `cargo test` passes. Often missing: DST regression cases, restart-during-execution, @random feasibility edge cases, XSS in logs, 1000-job metrics scrape, Postgres matrix.
- [ ] **README:** Looks done — installation and example. Often missing: security section at top, threat model, defaults table, troubleshooting for common Docker networking failures, "@random explained" section.

---

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| docker.sock exposed to internet | CRITICAL | Rotate all host secrets; assume full host compromise; rebuild from known-good image; audit all running containers |
| Orphaned containers after crash | LOW | Startup reconciliation with `cronduit.run_id` label finds and finalizes them |
| DB rows stuck in `status=running` | LOW | Reconciliation marks as `lost` with note; operator retriggers if needed |
| Lost exit code from auto_remove race | MEDIUM | Can't recover historical data; fix code to use explicit remove; document known-lost runs |
| Corrupted SQLite after crash | MEDIUM | WAL mode makes corruption rare; `PRAGMA integrity_check`; restore from backup |
| DB schema diverged between SQLite/Postgres | HIGH | Write migration to reconcile; extensive testing; document forced downtime for ops |
| `@random` resolution drift across restarts | LOW | Reconciliation against persisted `schedule_resolutions`; log the discrepancy |
| DST bug fired jobs at wrong time | MEDIUM | Rerun affected jobs manually; fix library; backfill via "Run Now" if idempotent |
| Log retention accidentally deleted data | HIGH | Restore from backup; add unit test for retention boundary |
| Secret leaked via log | CRITICAL | Rotate credential; audit log exporters; purge aggregator history if possible |
| Config reload partially applied | MEDIUM | Startup revalidates from file; fix reload code to be atomic |
| XSS in UI via job log | HIGH | Fix escaping; audit log history for exploit attempts; force session logout if auth exists |
| `container:<name>` silently into stale ns | MEDIUM | Pre-flight check fixes going forward; past runs can't be re-verified |

---

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| 1. Docker socket / no-auth messaging | Phase 1 (binding defaults) + Phase N (README SECURITY) | Default bind is 127.0.0.1; README has SECURITY section above the fold; startup warns when bound externally |
| 2. `container:<name>` silent failures | Phase 3 (Docker execution) | Pre-flight check exists; distinct failure reason in metrics; integration test with target-down scenario |
| 3. `wait_container` / `auto_remove` race | Phase 3 (Docker execution) | State machine in code; no `auto_remove=true` for jobs; 1000-iteration flake test passes |
| 4. Log streaming back-pressure | Phase 3 (execution) + Phase 5 (UI SSE) | Bounded channel; chunk-based storage; load test with 500 MB output; RSS stable |
| 5. DST / timezone correctness | Phase 2 (scheduler core) | `croner` in deps; DST regression test suite; timezone required in config |
| 6. `@random` correctness | Phase 2 (resolution + persistence) + Phase 5 (UI) | `schedule_resolutions` table; feasibility check; UI shows resolved value; deterministic re-roll tests |
| 7. SQLite write contention | Phase 4 (persistence) | Separate read/write pools; WAL + busy_timeout pragmas; batch writes; load test passes |
| 8. Schema parity SQLite/Postgres | Phase 4 (persistence) | Dual migration dirs; CI matrix; schema doc; both pass same integration suite |
| 9. Config reload races | Phase 2 (scheduler) | Debounced file watch; atomic apply; partial-parse rollback test; reload status in UI |
| 10. Restart-during-execution / orphans | Phase 3 (execution) + Phase 4 (persistence) | Labels + reconciliation; integration test for mid-run restart |
| 11. Log retention pruning | Phase 4 (persistence) | Batched delete; separate log vs run retention; prune metrics |
| 12. Image pull retry + digest | Phase 3 (execution) | Exponential backoff; classified errors; digest recorded per run |
| 13. ANSI / XSS / huge lines in UI | Phase 5 (UI) | XSS test case in CI; ANSI parser; line cap; paginated display |
| 14. Cross-compile / musl / OpenSSL | Phase 1 (deps) + Phase N (release CI) | rustls everywhere; `cargo tree -i openssl-sys` empty; arm64 CI builds |
| 15. Default surprises | Phase 1 (CLI/config) + Phase N (docs) | `--check` command; startup summary log; sample configs |
| 16. Run Now bypassing semantics | Phase 2 (scheduler interface) + Phase 5 (UI) | Shared code path; idempotency token; `trigger` column |
| 17. Metrics cardinality | Phase 6 (operational) | Closed-enum reasons; 1000-job scrape test; no run_id labels |
| 18. Secret leak in logs | Phase 2 (config model) + code review | `SecretString` type; no `{:?}` on config; missing-var fails fast |
| 19. Shutdown timeout | Phase 2 (scheduler) + Phase 6 | Per-job configurable; progress logs; second-SIGTERM kill |
| 20. Config format proliferation | Phase 1 (decision LOCKED) | TOML only in v1; documented decision |
| 21. "Local command" ambiguity | Phase 3 (execution) + Phase N (docs) | README headline; baseline tools image; docker-compose example |
| 22. Clock drift / jump | Phase 2 (scheduler core) | Wall-clock scheduling; jump detection; health endpoint |
| 23. Asset embedding / binary bloat | Phase 5 (UI build) | Tailwind purge; `debug-embed`; CI size check |

---

## Sources

**Docker socket security & threat model (Pitfall 1):**
- [OWASP Docker Security Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Docker_Security_Cheat_Sheet.html)
- [Docker Socket Security: A Critical Vulnerability Guide](https://medium.com/@instatunnel/docker-socket-security-a-critical-vulnerability-guide-76f4137a68c5)
- [The Dangers of Docker.sock](https://raesene.github.io/blog/2016/03/06/The-Dangers-Of-Docker.sock/)
- [Dockge Issue #849: Docker socket exposure enables complete host compromise](https://github.com/louislam/dockge/issues/849)

**Unauthenticated homelab exposure (Pitfall 1):**
- [How to Secure a Homelab Network: 7-Step Guide](https://readthemanual.co.uk/secure-your-homelab-2025/)
- [Self-Hosted WAF for Homelab](https://dev.to/arina_cholee/self-hosted-web-application-firewall-for-my-homelab-58oa)

**Docker `container:<name>` network mode races (Pitfall 2):**
- [moby#50326: Containers with `restart: always` and shared network namespace may fail to start](https://github.com/moby/moby/issues/50326)
- [docker/compose#6626: Restart/Reconnect containers connected via `network_mode: service`](https://github.com/docker/compose/issues/6626)
- [docker/compose#10263: Restart behavior with `network_mode: service`](https://github.com/docker/compose/issues/10263)
- [Docker Forum: network_mode container cannot start linked container after VPN restart](https://forums.docker.com/t/network-mode-container-vpn-cannot-start-linked-container-if-vpn-has-been-restart/144798)

**bollard wait/auto-remove race (Pitfall 3):**
- [docker-py#2655: Possible race condition in Remove/wait](https://github.com/docker/docker-py/issues/2655)
- [moby#8441: docker 1.2: Wait, exit code, and deleting the container fails](https://github.com/moby/moby/issues/8441)
- [runc#2185: Fix race checking for process exit and waiting for exec fifo](https://github.com/opencontainers/runc/pull/2185)
- [bollard docs (latest)](https://docs.rs/bollard/latest/bollard/struct.Docker.html)

**Log streaming back-pressure (Pitfall 4):**
- [bollard GitHub](https://github.com/fussybeaver/bollard)
- [tokio::sync::broadcast docs](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html) (general pattern)

**Cron DST & Rust cron libraries (Pitfall 5):**
- [croner-rust GitHub](https://github.com/Hexagon/croner-rust)
- [Croner DST handling — 56k.guru](https://hexagon.56k.guru/posts/croner-for-rust/)
- [Cloudflare blog: Using One Cron Parser Everywhere With Rust and Saffron](https://blog.cloudflare.com/using-one-cron-parser-everywhere-with-rust-and-saffron/)
- [How Debian Cron Handles DST Transitions — Healthchecks.io](https://blog.healthchecks.io/2021/10/how-debian-cron-handles-dst-transitions/)
- [node-cron#56: Cron jobs kicked off at wrong time for DST](https://github.com/kelektiv/node-cron/issues/56)
- [Sentry#66763: Cron monitor timezones not using DST adjustment](https://github.com/getsentry/sentry/issues/66763)
- [Red Hat: How cron jobs are affected by DST](https://access.redhat.com/solutions/477963)
- [chrono-tz GitHub](https://github.com/chronotope/chrono-tz)

**SQLite concurrency (Pitfalls 7, 11):**
- [The Write Stuff: Concurrent Write Transactions in SQLite (oldmoe)](https://oldmoe.blog/2024/07/08/the-write-stuff-concurrent-write-transactions-in-sqlite/)
- [PSA: Your SQLite Connection Pool Might Be Ruining Your Write Performance (Evan Schwartz)](https://emschwartz.me/psa-your-sqlite-connection-pool-might-be-ruining-your-write-performance/)
- [SQLite concurrent writes and "database is locked" errors](https://tenthousandmeters.com/blog/sqlite-concurrent-writes-and-database-is-locked-errors/)
- [Abusing SQLite to Handle Concurrency (SkyPilot)](https://blog.skypilot.co/abusing-sqlite-to-handle-concurrency/)
- [Four different ways to handle SQLite concurrency (Gwendal Roué)](https://medium.com/@gwendal.roue/four-different-ways-to-handle-sqlite-concurrency-db3bcc74d00e)

**sqlx + cross-compile + OpenSSL (Pitfalls 8, 14):**
- [sqlx FAQ](https://github.com/launchbadge/sqlx/blob/main/FAQ.md)
- [sqlx#670: Cross-compiling or statically linking sqlx fails because of OpenSSL](https://github.com/launchbadge/sqlx/issues/670)
- [rust-openssl#1337: Static linking of OpenSSL with sqlx](https://github.com/sfackler/rust-openssl/issues/1337)
- [rust-openssl#1627: Unable to cross compile for musl](https://github.com/sfackler/rust-openssl/issues/1627)
- [Build statically linked Rust binary with musl](https://dev.to/abhishekpareek/build-statically-linked-rust-binary-with-musl-and-avoid-a-common-pitfall-ahc)
- [rust-musl-builder](https://github.com/emk/rust-musl-builder)

**File watcher / config reload (Pitfall 9):**
- [notify crate](https://github.com/notify-rs/notify)
- [cfg_watcher](https://lib.rs/crates/cfg_watcher)
- [async-watcher](https://github.com/justinrubek/async-watcher)
- [tokio sync primitives](https://docs.rs/tokio/latest/tokio/sync/)

**Overlapping job execution (Pitfall 16):**
- [Cronitor: How to prevent duplicate cron executions](https://cronitor.io/guides/how-to-prevent-duplicate-cron-executions)
- [Prevent cronjobs from overlapping in Linux (ma.ttias.be)](https://ma.ttias.be/prevent-cronjobs-from-overlapping-in-linux/)

**ofelia context (Cronduit's differentiator):**
- [ofelia GitHub](https://github.com/mcuadros/ofelia)
- [ofelia#126: label configuration not being picked up](https://github.com/mcuadros/ofelia/issues/126)

**HTMX / SSE (Pitfall 13):**
- [HTMX SSE Extension](https://htmx.org/extensions/sse/)
- [Live website updates with Go, SSE, and htmx (Three Dots Labs)](https://threedots.tech/post/live-website-updates-go-sse-htmx/)

**Docker image pull handling (Pitfall 12):**
- [How to troubleshoot network timed out error when pulling images (LabEx)](https://labex.io/tutorials/docker-how-to-troubleshoot-network-timed-out-error-when-pulling-images-417523)
- [Kubernetes ImagePullBackOff backoff semantics (Spacelift)](https://spacelift.io/blog/kubernetes-imagepullbackoff)

**Internal references:**
- `.planning/PROJECT.md` — Cronduit project context and locked decisions
- `docs/SPEC.md` — v1 spec (authoritative for this milestone)

---
*Pitfalls research for: Rust Docker-native cron scheduler with web UI (Cronduit v1)*
*Researched: 2026-04-09*
