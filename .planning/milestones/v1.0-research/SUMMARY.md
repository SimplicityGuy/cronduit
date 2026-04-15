# Project Research Summary

**Project:** Cronduit
**Domain:** Self-hosted Docker-native cron scheduler with embedded server-rendered web UI (Rust)
**Researched:** 2026-04-09
**Confidence:** HIGH

## Executive Summary

Cronduit occupies a specific gap in the self-hosted scheduler space: a single-binary, Docker-native, config-file-driven scheduler with a real observability dashboard. Every mature competitor covers part of this space but misses somewhere critical — ofelia lacks `container:<name>` network support, Cronicle is Node.js-heavy and UI-first, dkron is distributed-first (overkill for single homelab), and docker-crontab has no UI or history. The recommended implementation is a hand-rolled tokio scheduler loop driving a bollard-based executor, with an axum+askama server-rendered web UI and SQLite as the default store — all compiled into one static Rust binary. The research strongly confirms the PROJECT.md spec and stack choices, with several important clarifications documented below.

The highest-risk areas are (1) the `@random` + `random_min_gap` feature, which has no prior art and requires a carefully designed persistence model to avoid operator surprise, and (2) the Docker socket security posture, which demands explicit, prominent documentation even in v1 despite auth being deferred. Both risks are manageable with the specific patterns the research identified. The implementation path is well-understood: six sequential phases from skeleton to release, each with a concrete exit criterion, progressing from core scheduler to Docker execution to UI to live events and observability.

The research produced several decisions that are now locked: `croner 3.0` for cron parsing (not the `cron` crate), `askama_web 0.15` with the `axum-0.8` feature (not the deprecated `askama_axum`), TOML as the config format (the PROJECT.md "soft-locked" caveat should be removed — `serde-yaml` is archived and TOML is definitively correct), a hand-rolled scheduler loop (not `tokio-cron-scheduler`, which lacks SQLite persistence and would create a dual source of truth), and cross-compilation via rustls (zero OpenSSL). These are not open questions; they are research conclusions.

---

## Key Findings

### Recommended Stack

The stack is well-defined and high-confidence. Every core technology choice has been verified against current crate versions and official sources.

**Core technologies:**

- **tokio 1.51** — async runtime; required by bollard, sqlx, and axum; use `features = ["full"]` for v1
- **axum 0.8 + tower-http 0.6** — HTTP server; tokio-native, lean, first-class middleware (TraceLayer, CompressionLayer)
- **askama 0.15 + askama_web 0.15 (feature: `axum-0.8`)** — server-rendered templates; designer-friendly HTML files, compile-time type checking, `{% extends %}` for layout inheritance. `askama_axum` is deprecated (crate description literally reads "+deprecated"); `askama_web` with the `axum-0.8` feature is the officially blessed replacement. Any tutorial predating late 2025 that mentions `askama_axum` is wrong.
- **croner 3.0** — cron parsing; the only actively maintained crate with `L`/`#`/`W` modifiers, timezone-aware `next_after(DateTime)`, and built-in human-readable descriptions ("Every hour at minute 0" displayable in UI). `saffron` is dead (last release 2021). The generic `cron` crate lacks extended modifiers and has no description support. Note: `croner` does NOT handle `@random` — that is a Cronduit config-layer feature where the field is resolved to a concrete expression before being handed to croner.
- **bollard 0.20** — Docker API client; handles all network modes including `container:<name>`, volumes, image pull, log streaming; no CLI shell-out. Use with `ssl_providerless` or `rustls` feature.
- **sqlx 0.8 with `runtime-tokio-rustls`** — async DB with offline query checking; supports SQLite and Postgres from one query surface. Use `runtime-tokio-rustls` (NOT the OpenSSL variant) for clean cross-compilation.
- **toml 1.1 + serde** — config parsing; TOML is locked. `serde-yaml` is archived on GitHub (dtolnay set the archive flag, last release 2024-03). The ecosystem has fragmented into `serde_yml`, `serde_norway`, and others — none with trustworthy governance. Additionally, cron expressions must be quoted in YAML to avoid parse-as-sequence footguns, and `@random` must be quoted because leading `@` is reserved YAML syntax. TOML is definitively correct; the PROJECT.md "soft-locked" caveat must be updated to "locked."
- **rust-embed 8.11** — static asset embedding; disk-read in debug mode enables fast Tailwind iteration without rebuilds
- **tokio-util (CancellationToken)** — graceful shutdown propagation to all long-lived tasks
- **rand 0.8/0.9** — `@random` randomized field selection
- **metrics 0.24 + metrics-exporter-prometheus 0.18** — Prometheus exposition via metrics facade (not the `prometheus` crate directly; facade decouples instrumentation from export format)
- **chrono 0.4 + chrono-tz** — timestamps and timezone arithmetic; integrates with croner and sqlx
- **humantime + humantime-serde** — parse `"90m"`, `"2h"` in `random_min_gap`, `timeout`, config fields

**Key stack decisions settled:**
- Cross-compile via `cargo-zigbuild` (not QEMU-emulated builds, which are 10x slower on GHA runners); `cargo-chef` for dep-layer caching in multi-stage Dockerfile
- No `openssl-sys` anywhere; run `cargo tree -i openssl-sys` to verify zero results; `cargo deny` to forbid it transitively
- Tailwind via standalone CLI binary in `build.rs` (no Node.js dependency); vendor htmx 2.x into `static/vendor/`
- `cargo-nextest` for CI test runner; `testcontainers-rs 0.27` for Docker-dependent integration tests
- `axum-htmx 0.8` for HTMX request/response helpers (`HxRequest` extractor for partial rendering, `HxResponseTrigger` for "Run Now" toast)

### Expected Features

The feature landscape is clear and competitive analysis confirms the scope is correct. The PROJECT.md out-of-scope list is defensible — do not expand it.

**Must have (table stakes — absence means users close the tab):**
- Standard 5-field cron + `@hourly`/`@daily`/`@weekly`/`@monthly` shorthands
- Dashboard with job list, status badges, next-run time, last-run outcome
- Per-run stdout/stderr capture, persistence, and log viewer
- Manual "Run Now" button (every serious competitor has this; must go through the scheduler, not bypass it)
- Full Docker network mode support including `container:<name>` — the primary reason this product exists; ofelia's #1 failure point
- Volume mounts, env vars, image auto-pull per Docker job
- Run history: start/end/duration/exit_code/status per run
- Auto-refresh for running jobs (HTMX polling)
- Graceful shutdown with configurable drain timeout
- `/health` and `/metrics` endpoints
- Structured JSON logs to stdout
- TOML config as source of truth with sync-on-startup semantics
- Single binary + multi-arch Docker image

**Should have (differentiators — the "why Cronduit over ofelia" story):**
- `@random` schedule field — no competitor has this; resolves at boot to a concrete schedule, persisted to DB, re-rolls on a daily cadence (not on every restart or reload)
- `random_min_gap` constraint solver — companion to `@random`; slot-based algorithm guarantees N random jobs are spread across the day by at least G minutes
- Full `--network container:<name>` support — ofelia's single biggest wart; Cronduit's headline differentiator; requires structured failure handling when the target container is not running
- Single static binary (<30 MB) + distroless/Alpine image
- Terminal-green Tailwind aesthetic matching the design system
- Config-first with GitOps story (commit `cronduit.toml` to git)
- Prometheus `/metrics` out of the box
- PostgreSQL as documented alternative (same schema, flip `DATABASE_URL`)

**Defer to v1.x (post-launch, user-feedback driven):**
- Live log tail via SSE (v1 uses HTMX polling on run detail; upgrade to SSE in v1.x / Phase F)
- Log retention cleanup worker (spec: default 90 days)
- Per-job "skip if previous still running" flag
- `cronduit check config.toml` validation subcommand
- Example Grafana dashboard JSON

**Never (explicit non-goals, confirmed by research):**
- Web UI authentication — deferred to v2; but this requires a clear security posture statement (see Critical Pitfalls), not absence of security thinking
- RBAC / multi-tenancy / multi-node
- Workflow DAGs / job dependencies
- SPA frontend (React/Vue/Svelte)
- In-UI job CRUD (UI is read-mostly observability; all job changes happen via config file)
- Email/webhook notifications (use Prometheus + Alertmanager)

### Architecture Approach

The architecture is a single tokio process with five durable tasks (scheduler loop, config watcher, web server, log retention pruner, graceful shutdown handler) communicating via shared `AppState` (Arc-cloned), a `tokio::sync::broadcast` event bus, and mpsc channels for scheduler control. The scheduler loop is hand-rolled (`tokio::select!` over next-fire timer, control messages, and shutdown signal); there is no external scheduler crate.

**Major components and responsibilities:**
1. **Config Loader + Watcher** — parse TOML, expand `${ENV_VAR}`, produce immutable `ConfigSnapshot`, notify reloads via SIGHUP / `notify` crate (500ms debounce) / `POST /api/reload`; all three entry points share one reload function
2. **Sync Engine** — diff `ConfigSnapshot` against DB `jobs` table; create/update/`enabled=false` for removed (never delete); idempotent on `config_hash`; triggers `@random` resolution for new or changed jobs
3. **Job Registry** — `Arc<RwLock<HashMap<JobId, ResolvedJob>>>` shared between scheduler, web, and executor; only written on reload; never hold write lock across `.await`; clone the `ResolvedJob` and drop guard before awaiting
4. **Scheduler Core** — hand-rolled `tokio::select!` loop; ticks on min of all `next_fire_at` values; accepts `SchedulerCmd::{Reload, RunNow, Shutdown}`; does NOT use `tokio-cron-scheduler` (verified: it has no SQLite persistence store, would create a dual source of truth with our `job_runs` table)
5. **Executor Dispatcher** — `tokio::spawn` per job run; owns full lifecycle: insert `job_runs(running)` → run backend → stream logs → finalize status → emit events; enforces timeout via nested `tokio::select!`
6. **Docker Backend** — bollard client; create container (NO `auto_remove=true` — Docker races `wait_container`); start; concurrent log pump task + `wait_container`; persist exit code before remove; explicit `remove_container` after wait; label every container with `cronduit.run_id`; pre-flight inspect for `container:<name>` jobs
7. **Command / Script Backends** — `tokio::process::Command` with piped stdout/stderr; same log pump pipeline as Docker backend
8. **Persistence Layer** — sqlx pool enum (Sqlite | Postgres); separate write pool (max_connections=1) and read pool (max_connections=N) for SQLite; WAL + pragmas on every connection; dual migration directories (`migrations/sqlite/`, `migrations/postgres/`)
9. **Event Bus** — `tokio::sync::broadcast::channel(1024)`; emits `RunStarted`, `LogLine`, `RunFinished`, `JobsReloaded`; SSE handlers subscribe and filter by `run_id`; slow SSE consumers drop (by design); DB writer is the authoritative record and must not be coupled to SSE consumer backpressure
10. **Web Server** — axum router with askama-rendered HTML, HTMX partials, SSE endpoint, `/health`, `/metrics`; static assets via rust-embed; `AppState` injected into every handler
11. **Randomizer** — pure function `resolve_random(jobs, min_gap) -> Vec<ResolvedJob>`; slot-based algorithm (not retry-until-fits); persists `resolved_schedule` to DB; daily re-roll cadence (not on restart or reload unless schedule field changed); feasibility check at config-load time (fail loudly if N * gap > 24h)

### Critical Pitfalls

The research identified 6 CRITICAL pitfalls and 8+ HIGH pitfalls. The roadmap must address the critical ones in the correct phase — retrofitting is expensive.

**Critical pitfalls:**

1. **Docker socket security requires active framing, not silence.** Auth is deferred to v2, but the docker socket is root-equivalent. Default bind must be `127.0.0.1:8080` (not `0.0.0.0`). Log a warning every 5 minutes when bound to non-loopback without auth. Require an explicit `[server] i_understand_no_auth = true` opt-in for non-loopback binding. Ship `THREAT_MODEL.md`. `docker-compose.example.yml` must use `expose:` not `ports:`. This is a Phase A + Phase F responsibility, not a "v2 problem."

2. **`container:<name>` network failure must produce a named, structured failure.** Pre-flight inspect the target container before spawning. If not running, record `network_target_not_running` (not Docker's raw error string). Emit `failures_total{reason="network_target_unavailable"}` metric. This is the feature that justifies Cronduit's existence — it must have better error surfaces than ofelia.

3. **Do NOT use `auto_remove=true` on job containers.** Docker's auto-remove races with `wait_container`, causing exit code loss and log truncation. The correct sequence: create without auto_remove → start → concurrent log pump + `wait_container` → persist exit code to DB → explicit `remove_container` after wait completes. Structure as an observable state machine. Must be correct in Phase D from day one.

4. **SQLite write contention needs a dedicated write pool from Phase A.** Single-connection write pool + multi-connection read pool + WAL mode + `busy_timeout=5000` pragma. Batch log inserts (500 lines or 250ms). The "happy path works in dev" pattern masks this until a real homelab workload hits it. The separation must exist before the log writer is built on top of it.

5. **`@random` correctness is the highest-risk new feature — no prior art.** Four failure modes to prevent: re-rolling on every restart (use daily cadence), infinite-loop on infeasible min-gap (fail loudly at startup with a clear count message), invisible resolved schedule (surface in UI and structured logs), state in a transient HashMap (persist to DB `schedule_resolutions` table). The algorithm must be slot-based (divide 24h into N slots, place each job with jitter inside its slot) rather than retry-until-fits. Address data model in Phase B, algorithm in Phase E.

6. **DST handling must use `croner 3.0`, explicit configured timezone, and UTC storage.** `croner 3.0` explicitly documents its DST behavior. All schedule arithmetic happens in the configured TZ (mandatory `[server] timezone = "..."`, defaulting to `"UTC"`); all stored timestamps are RFC3339 UTC. Address in Phase B. DST regression tests are required.

**Additional high-severity pitfalls the roadmap must plan for:**

- **Log streaming back-pressure** — bounded channels (256 lines), tail-sampling drop policy with DB marker, decouple DB writer from SSE broadcast, per-run output cap (configurable, default 10 MB)
- **Schema parity SQLite/Postgres** — two migration directories, CI matrix runs tests against both on every PR; no `JSONB`, no arrays, no Postgres-only syntax
- **Cross-compile via rustls** — `bollard` with `ssl_providerless`/`rustls`, `sqlx` with `runtime-tokio-rustls`; `cargo tree -i openssl-sys` must return empty; `cargo deny` enforces this
- **Config reload non-atomicity** — 500ms debounce on file watch; parse to staging structure first, keep old config on failure; in-flight runs are NOT cancelled on reload
- **Startup reconciliation for orphan containers** — label every spawned container with `cronduit.run_id`; at boot, query Docker for all labeled containers and reconcile against DB; DB rows stuck in `running` state must be resolved
- **Log XSS / ANSI rendering** — always HTML-escape log content (never `| safe` or `PreEscaped` on log content); ANSI parsed server-side via `ansi-to-html`; binary bytes replaced with placeholder; XSS test in CI is mandatory

---

## Implications for Roadmap

Both ARCHITECTURE.md and FEATURES.md independently proposed nearly identical 6-phase build orders. The following is the reconciled, unified phase structure. Each phase has a concrete exit criterion — nothing advances until the criterion passes.

### Phase A — Project Skeleton + Persistence Foundation

**Rationale:** Config parser and DB layer are shared dependencies for everything that follows. Security posture (default bind, startup nag) must be correct from the first deployable binary. Schema parity decisions (dual migration dirs, write pool separation) are cheapest when made before any code depends on a different assumption.

**Delivers:**
- `Cargo.toml` with locked deps (rustls everywhere, no openssl-sys), `main.rs`, tokio runtime, clap CLI with `run` and `check` subcommands
- `config/parse.rs`: TOML deserialization + `${ENV_VAR}` expansion + defaults merge; `SecretString` type for env values so they print `[redacted]` in Debug
- `db/mod.rs`: sqlx pool enum (Sqlite/Postgres), separate write pool (max_connections=1) and read pool, WAL pragmas on every connection, `sqlx::migrate!`, initial schema
- Dual migration directories (`migrations/sqlite/`, `migrations/postgres/`) with initial schema in both (jobs, job_runs, job_logs tables)
- `sync_config_to_db`: upsert jobs by name, `config_hash` idempotency, `enabled=false` for removed jobs, `jobs.resolved_schedule` column present (populated in Phase E)
- Default bind `127.0.0.1:8080`; startup summary log (bind, db, timezone, config, job count); startup nag on non-loopback; `THREAT_MODEL.md` skeleton
- `cronduit check <config.toml>` subcommand: validate config, report all effective settings with source (default/env/config), warn on risky combos

**Avoids:** Pitfall 7 (SQLite write contention — separate pools from day one), Pitfall 8 (schema parity — dual migration dirs from day one), Pitfall 14 (cross-compile — rustls deps locked from day one), Pitfall 15 (zero-config surprises — startup summary), Pitfall 18 (secrets — SecretString type), Pitfall 20 (TOML locked, no multi-format creep)

**Exit criterion:** `cronduit --config test.toml` loads config, creates DB, upserts jobs, exits cleanly with startup summary. `cronduit check test.toml` reports valid/invalid with human-readable errors.

**Research flag:** Well-documented patterns. No additional research needed.

---

### Phase B — Scheduler Core + Command/Script Executor

**Rationale:** The scheduler loop is the heart of the product. Validate cron parsing, timing semantics, graceful shutdown, and log capture before adding Docker complexity or a UI. The `@random` data model (persisted `resolved_schedule` column) should be established here even though the randomizer ships in Phase E — the schema must be stable before the UI is built on top of it.

**Delivers:**
- `schedule/cron.rs`: `croner 3.0` wrapper, next-fire computation, DST-correct with explicit configured timezone (UTC default); DST regression tests using frozen clocks on spring-forward and fall-back dates
- `scheduler/mod.rs`: hand-rolled `tokio::select!` loop; `SchedulerCmd::{Reload, RunNow, Shutdown}`; wall-clock scheduling (not monotonic sleep); clock-jump detection (>2 minute delta → warn + re-evaluate)
- `executor/command.rs` + `executor/script.rs`: `tokio::process::Command`, piped stdout/stderr
- `executor/logs.rs`: per-run log pump with bounded channel (256 lines), tail-sampling drop policy with DB marker, batch DB insert (500 lines or 250ms), decouple DB writer from any broadcast bus (bus added in Phase F)
- `db/runs.rs` + `db/logs.rs`: CRUD for job_runs and job_logs
- `shutdown.rs`: `CancellationToken` propagated to all tasks; configurable drain timeout (global default + per-job override); shutdown progress logged every 5s; second SIGTERM = immediate kill
- Per-run executor lifecycle: `trigger` column on `job_runs` (`scheduled | manual | reload`)

**Avoids:** Pitfall 5 (DST — croner + explicit TZ + UTC storage + regression tests), Pitfall 6 (`@random` schema column present, daily-cadence re-roll model established), Pitfall 18 (secrets not logged — SecretString from Phase A carries through), Pitfall 19 (configurable shutdown timeout, progress logging), Pitfall 22 (wall-clock scheduling, clock-jump detection)

**Exit criterion:** A command-type job on `*/1 * * * *` fires every minute, writes run + logs to SQLite. Ctrl+C waits for in-flight runs up to configured timeout. DST tests pass. `cargo tree -i openssl-sys` returns empty.

**Research flag:** Well-documented patterns. No additional research needed.

---

### Phase C — Web UI (Read-Only, No Live Updates Yet)

**Rationale:** Making state visible early validates that the schema is correct for rendering and surfaces schema gaps cheaply before Phase D adds Docker complexity. Tailwind + askama + axum wiring is non-trivial to establish; do it once before adding interactive features.

**Delivers:**
- `web/mod.rs`: axum router, `AppState` struct (db pool, registry, control sender, started_at)
- Tailwind standalone CLI in `build.rs`; `static/vendor/htmx.min.js` vendored (not CDN); rust-embed 8.11 for all static assets
- `askama 0.15` + `askama_web 0.15` (feature: `axum-0.8`) — NOT the deprecated `askama_axum`. `#[derive(Template, WebTemplate)]` gives `IntoResponse` automatically.
- Templates: `base.html`, `dashboard.html`, `job_detail.html`, `run_detail.html`, partial `job_row.html` (HTMX swap target), `run_row.html`
- HTMX polling on dashboard (`hx-get="/ui/jobs-table" hx-trigger="every 5s"`) and run detail status badge (every 2s while running, self-terminating); separate `/ui/...` partial routes (not same-route detection)
- `GET /health` endpoint returning scheduler status
- Structured JSON logging (`tracing-subscriber` with JSON formatter, one span per job run)
- Terminal-green design system applied (matches `design/DESIGN_SYSTEM.md` token set)
- Filter by name, sort by name/last-run/next-run/status
- Run Now button wired via `hx-post="/api/jobs/:id/run"` → `mpsc SchedulerCmd::RunNow`; manual runs go through the scheduler (not a direct executor bypass); idempotency token (UUID per click)
- Log viewer with always-HTML-escaped content; ANSI escapes parsed server-side via `ansi-to-html`; binary bytes replaced with placeholder; line-length cap (2000 chars in DOM, full line available via raw API)

**Avoids:** Pitfall 13 (log XSS — never `| safe` or `PreEscaped` on log content; XSS test in CI), Pitfall 16 (Run Now through scheduler, idempotency token), Pitfall 23 (Tailwind JIT purge against templates; binary size check in CI; `Cache-Control: immutable` on hashed assets)

**Exit criterion:** Operator opens `http://localhost:8080`, sees job list with status badges, clicks into a run, sees logs rendered correctly with ANSI colors and no XSS. Run Now button triggers execution visible on next poll. XSS test in CI passes.

**Research flag:** `askama_web 0.15` integration is new enough that checking the official examples repo before this phase starts is recommended. The rest of the patterns are well-documented.

---

### Phase D — Docker Executor

**Rationale:** This is the product's headline feature. The bollard integration is isolated enough to be built as a complete unit. The `auto_remove` race (Pitfall 3), log back-pressure (Pitfall 4), and startup reconciliation (Pitfall 10) cannot be retrofitted — they must be correct from the first Docker execution.

**Delivers:**
- `executor/docker.rs`: bollard client via `Docker::connect_with_unix_defaults()`; create container without `auto_remove=true`; start; concurrent log pump task + `wait_container`; persist exit code to DB before calling `remove_container`; explicit `remove_container(force=false)` after wait and log drain
- Pre-flight inspect for `container:<name>` jobs: if target not `running`, record `network_target_not_running` structured failure; emit `failures_total{reason="network_target_unavailable"}` metric; never silently retry into a different network
- Label every container: `cronduit.run_id=<uuid>`, `cronduit.instance=<instance_id>`
- Image pull with exponential-backoff retry (3 attempts: 1s, 5s, 25s); classify failures (network/timeout → retry; `manifest unknown`/`unauthorized` → no retry); record resolved image digest on run row; `failures_total{reason="image_pull_failed"}` distinct metric label
- All network modes: `bridge`, `host`, `none`, named network, `container:<name>`
- Volume mounts, env vars, container name per job, per-job timeout via `tokio::select!` → `docker stop`
- Startup reconciliation: at boot, query Docker for all `cronduit.run_id`-labeled containers; reconcile against DB (re-attach if still running, finalize if exited, mark `lost` if DB says running but no container found)
- Integration tests via `testcontainers-rs 0.27`: basic execution, `container:<name>` network mode (the marquee test, requires Docker socket on CI runner), exit code capture after fast-exiting containers

**Avoids:** Pitfall 2 (`container:<name>` structured failure — pre-flight, named reason, distinct metric), Pitfall 3 (`auto_remove` race — explicitly disabled; state machine: Creating→Starting→Running→Exited→LogsDrained→Removed), Pitfall 4 (log back-pressure — bounded channel, tail-sampling, DB writer decoupled from any live viewers), Pitfall 10 (startup reconciliation + container labeling), Pitfall 12 (image pull retry + failure classification + digest recording)

**Exit criterion:** A docker-type job with `network="container:vpn"` runs, logs are fully captured, container is removed (via explicit bollard call), exit code is recorded correctly. Integration tests pass including the `container:<name>` network mode test. A container that exits in <50ms has its exit code reliably captured across 100 repeated runs.

**Research flag:** The exact bollard API call sequence for concurrent log pump + `wait_container` + explicit remove should be prototyped as a standalone spike before full implementation. The `container:<name>` testcontainer test requires Docker socket access on the CI runner (GitHub Actions ubuntu-latest has Docker available).

---

### Phase E — Config Reload, `@random` Resolution, and Manual Run Polish

**Rationale:** `@random` is the second marquee differentiator. It depends on Phase B's schema foundation and Phase D's executor being solid. Config reload correctness (debounced, atomic, non-cancelling of in-flight runs) is required before the product is production-ready for real homelab use.

**Delivers:**
- `config/watch.rs`: `notify` crate + SIGHUP + `POST /api/reload`; 500ms debounce (handles editor truncate-write patterns); parse to staging structure — if parse fails, log error, keep old config entirely; single unified `do_reload()` function for all three entry points
- Scheduler reload: diff old vs. new registry; `enabled=false` for removed jobs (history preserved); in-flight runs complete under old config (reloads affect only future fires — document this explicitly)
- `schedule/random.rs`: `@random` field resolver; slot-based algorithm (divide 24h into N slots each of size 24h/N, place job's random offset inside its slot with jitter — guarantees min-gap satisfaction if feasibility passes); deterministic RNG keyed by `(date, job_name)` for reproducible debugging
- Feasibility check at config-load time: if `N_random_jobs * min_gap > 24h`, fail loudly with a clear error message ("@random min_gap of 90m cannot be satisfied for 20 jobs in 24h — max 16 jobs at this gap")
- Re-roll cadence: once per calendar day at 00:00 configured TZ; config reloads do NOT re-roll unless the job's `schedule` field changed; restarts do NOT re-roll unless the resolution has expired
- Persistence: `schedule_resolutions(job_id, resolved_cron, resolved_at, expires_at, reason)` table; scheduler clock reads `resolved_schedule` exclusively — `@random` never reaches croner
- UI surfaces: job detail shows "Schedule: `@random` (today resolved to `14 17 * * *`, re-rolls at 00:00 tomorrow)"; dashboard badge distinguishes `@random` jobs from fixed-schedule jobs
- Structured log event on every re-roll: `{event:"random_resolved", job, previous, next, reason}`
- Metrics: `cronduit_random_resolutions_total{job, reason}`, `cronduit_random_feasibility_failures_total`
- Run Now idempotency token (UUID per click, repeat submissions within a window return same run ID); manual runs do NOT consume `@random` daily slots; double-click cannot spawn two containers
- UI "Settings" page: last config reload timestamp, last reload error if any, current effective bind / timezone / retention

**Avoids:** Pitfall 6 (`@random` — persisted resolutions, daily cadence, slot algorithm, feasibility check, UI surfacing, structured events), Pitfall 9 (config reload — 500ms debounce, staging parse, no in-flight cancellation, atomic apply under lock), Pitfall 16 (Run Now idempotency token, through-scheduler path, manual runs don't consume random slots)

**Exit criterion:** Edit config → SIGHUP → new jobs appear in UI, removed jobs go gray (history intact), resolved_schedule retained for unchanged random jobs. Job with `@random` shows its resolved time in the UI. Setting `random_min_gap` to an infeasible value (e.g., 20 jobs with 90m gap in 24h) produces a clear startup error, not a hang or silent drop.

**Research flag:** The `@random` slot-based constraint algorithm is original design territory — no prior art. A design review of `schedule/random.rs` before committing to the implementation is recommended. Specific edge cases to verify: (1) jobs with mixed random + non-random fields (e.g., `@random 14 * * 1-5` — minute is random but hour is fixed), (2) feasibility when randomized jobs span different days of the week, (3) re-roll expiry boundary behavior at midnight.

---

### Phase F — Live Events, Metrics, Retention, and Release Engineering

**Rationale:** The product is functionally complete after Phase E. This phase adds the observability layer that makes it production-quality, the release engineering for public OSS distribution, and the security documentation that makes it responsible to ship.

**Delivers:**
- `events/bus.rs`: `tokio::sync::broadcast::channel(1024)` wiring all emitters (executor, scheduler, config reload); `state.events.send(ev).ok()` (not `.unwrap()` — sender returns Err when zero subscribers)
- SSE endpoint `GET /events/runs/:id/logs`: HTMX `hx-ext="sse" sse-connect="..."` on run detail log tail; SSE subscribers filter by `run_id`; slow subscribers drop (by design — DB writer is authoritative); upgrade from HTMX polling on run detail
- `metrics/` module: `cronduit_jobs_total` (gauge), `cronduit_runs_total{job,status}` (counter), `cronduit_run_duration_seconds{job}` (histogram), `cronduit_failures_total{job,reason}` (reason is a closed enum: `image_pull_failed | network_target_unavailable | timeout | exit_nonzero | abandoned | unknown`), `cronduit_scheduler_up` (gauge); no `run_id` in any label; `GET /metrics` Prometheus text format via `metrics-exporter-prometheus`
- `db/retention.rs`: daily pruner; batched deletes (1000 rows per batch, small sleep between); separate `log_retention` (default 30 days) and `run_retention` (default 365 days); WAL `PRAGMA wal_checkpoint(TRUNCATE)` after large prunes; metric for prune duration and rows affected
- Multi-arch Docker image via `cargo-zigbuild` cross-compilation (amd64 + arm64 in a single builder stage); `FROM alpine:3.20` runtime; no QEMU emulation; `cargo-chef` for dep-layer caching
- GitHub Actions CI: fmt + clippy + nextest + SQLite tests + Postgres tests (testcontainers) + both-arch Docker build on every PR; arm64 integration test on main/tag push
- `THREAT_MODEL.md` (complete): what mounting the docker socket grants, what happens if web UI is reachable by untrusted client, what happens if config file is edited by an attacker, what happens if a malicious image is scheduled
- README security section above the fold: "Cronduit requires read-write access to the Docker socket. This is equivalent to root on the host. Do not expose the web UI to any network you do not fully trust."
- `docker-compose.example.yml` with `expose:` not `ports:` for the web UI
- `cargo deny` config forbidding `openssl-sys` transitively
- Binary size check in CI (alert if >50 MB)
- Example deployment configs: `examples/homelab-compose/`, `examples/reverse-proxy/`, `examples/bare-metal-systemd/`

**Avoids:** Pitfall 1 (security posture — THREAT_MODEL.md complete, bind default enforced since Phase A, README security section), Pitfall 4 (log back-pressure — SSE subscribers disposable, DB writer authoritative and decoupled), Pitfall 11 (retention batching, separate log/run retention configs), Pitfall 14 (cross-compile via rustls, cargo deny for openssl), Pitfall 17 (metrics cardinality — closed enum for `reason`, no `run_id` label, documented scrape guidance)

**Exit criterion:** Opening a running job's detail page shows log lines streaming in real time via SSE. `/metrics` exposes all required counters with correct types. Old runs prune on schedule with no write contention spikes in logs. Multi-arch Docker image builds and runs correctly on both amd64 and arm64. CI is green. `THREAT_MODEL.md` and README security section are complete and reviewed.

**Research flag:** Multi-arch cross-compilation pipeline (`cargo-zigbuild` + docker buildx) should be verified with a manual build before wiring into CI. The `TARGETPLATFORM`/`TARGETARCH` ARG handling in multi-stage Dockerfiles has subtle ordering requirements worth a standalone test.

---

### Phase Ordering Rationale

- **A before everything:** Config parser and DB schema (including write pool separation, dual migrations) are shared dependencies. Wrong assumptions here propagate everywhere.
- **B before C:** The scheduler must be real before building a UI on top of it. The schema must be stable (including `resolved_schedule`) before templates are built against it.
- **C before D:** UI validation of command/script paths catches schema gaps cheaply before Docker adds complexity. Produces a demo-able artifact for early feedback.
- **D as an isolated unit:** Docker execution is complex enough (bollard, `wait_container` race, container labeling, startup reconciliation) that it deserves focused attention without UI distractions.
- **E after D:** `@random` resolution references executor state (manual re-roll must track runs). Config reload safety requires knowing which jobs are in-flight. This is the riskiest phase; having D solid first reduces risk significantly.
- **F last:** Release engineering and observability do not affect correctness. Adding them early creates churn as interfaces stabilize. Exception: the security posture elements (default bind, startup nag, THREAT_MODEL.md skeleton) must start in Phase A — only the complete documentation ships in Phase F.

### Research Flags

Phases needing deeper investigation or design review during planning:
- **Phase D (Docker Executor):** The exact bollard API call sequence for concurrent log pump + `wait_container` + explicit remove should be prototyped as a standalone spike. Verify the `container:<name>` testcontainers test works on the CI environment before committing to it as a hard requirement.
- **Phase E (`@random` algorithm):** Original design territory, no prior art. Design review of `schedule/random.rs` before implementation. Focus on: mixed random+non-random fields, multi-day-of-week feasibility, midnight boundary behavior.

Phases with well-documented patterns (no additional research needed):
- **Phase A:** TOML + sqlx + clap + tracing is a well-worn Rust daemon stack. Dual migration directories are a documented sqlx pattern.
- **Phase B:** tokio scheduler loops, `tokio::process::Command` with piped I/O, `CancellationToken` graceful shutdown — all have official documentation and community examples.
- **Phase C:** axum + rust-embed + HTMX polling have official examples. `askama_web 0.15` follows the same pattern as the deprecated `askama_axum`; check the official examples repo once at the start of the phase.
- **Phase F:** GitHub Actions workflows, `cargo-zigbuild` multi-arch, Prometheus text format — standard.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All crate versions verified against crates.io and official sources. `croner 3.0` (2026-04-08), `askama_web 0.15` (2026-03-24), `bollard 0.20` (March 2026), `sqlx 0.8.6`, `toml 1.1.2` (2026-04-01). `serde-yaml` deprecation independently confirmed via GitHub archive flag. |
| Features | HIGH | Competitor analysis thorough: ofelia, Cronicle, dkron, docker-crontab, Cronmaster, GoCron. Feature gaps are documented in ofelia's issue tracker. PROJECT.md out-of-scope list confirmed correct by research. |
| Architecture | HIGH | All patterns are established Rust async daemon idioms. Hand-rolled scheduler loop and bollard integration are well-understood. Anti-patterns (`tokio-cron-scheduler`, holding registry lock across await, direct bollard stream to SSE) are verified and documented. |
| Pitfalls | HIGH (bollard/SQLite/DST: multiple independent sources, known ecosystem bugs confirmed in moby issue tracker); MEDIUM (`@random` algorithm: original design territory, no cross-reference possible) |

**Overall confidence:** HIGH

### Gaps to Address

- **`@random` mixed-field edge cases:** Jobs with a mix of random and non-random fields (e.g., `@random 14 * * 1-5` — minute random, hour fixed) are not fully specified. The slot-based algorithm handles the all-random case cleanly; the partial-random case needs explicit design before Phase E planning.
- **Renamed job semantics:** If a user renames a job in config (name is the primary key for sync), the current sync engine would treat it as delete + create, losing run history. This needs an explicit decision in Phase A planning — document the behavior, and consider whether a `previous_name` field or a UI warning is warranted.
- **Log viewer pagination UX:** FEATURES.md flags that dashboard UX for 100+ jobs and run detail for 100K+ log lines needs a design decision before Phase C. The schema supports pagination; the template design does not yet specify pagination vs. virtual scroll vs. load-more.
- **"Running" state recovery label:** DB rows stuck in `running` after a crash are addressed by Phase D's startup reconciliation, but the exact status label (`orphaned`, `interrupted`, `lost`) and UI treatment need a decision before Phase C renders run history.
- **TOML config update to lock TOML:** PROJECT.md currently marks the config format as "soft-locked (research will validate)". Research has validated it. The key decisions table and constraints section should be updated to reflect that TOML is locked with no planned YAML support.

---

## Sources

### Primary (HIGH confidence — versions and behaviors verified directly)
- `croner` crate docs and CHANGELOG — 3.0.1 confirmed current (2026-04-08); DST behavior documented
- `askama` / `askama_web` crate docs (0.15.x) — `askama_axum` deprecation notice confirmed in crate description
- `bollard` crate docs (0.20.2) — network mode API, log stream, wait_container behavior
- `sqlx` crate docs (0.8.6) — pool configuration, offline mode, WAL pragma support
- `toml` crate docs (1.1.2, 2026-04-01) — TOML 1.1 spec tracking confirmed
- `serde-yaml` GitHub repository — archive flag confirmed set; crate description reads "+deprecated"
- ofelia GitHub (mcuadros/ofelia, netresearch fork) — `container:<name>` network mode gap confirmed; run history model documented
- moby issue tracker (#50326, #8441) — `wait_container`/`auto_remove` race confirmed
- docker-py issue tracker (#2655) — `auto_remove` race cross-reference

### Secondary (MEDIUM confidence — community consensus, multiple sources agree)
- Cronicle GitHub and homepage — feature analysis, deployment model, Node.js runtime footprint
- dkron homepage — distributed scheduler scope confirmed
- Cronmaster, GoCron documentation — web UI feature comparison
- Awesome Self-Hosted list — market landscape
- Self-hosted homelab stack 2026 (elest.io) — user preference and adoption signals
- `cargo-zigbuild` documentation — multi-arch cross-compilation approach

### Tertiary (Inference / original design)
- `@random` slot-based constraint algorithm — Cronduit-original design; no prior art; the correctness of the approach is reasoned from first principles but has not been cross-checked against an existing implementation

---
*Research completed: 2026-04-09*
*Ready for roadmap: yes*
