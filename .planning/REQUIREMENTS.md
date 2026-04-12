# Requirements: Cronduit

**Defined:** 2026-04-09
**Core Value:** One tool that both runs recurrent jobs reliably AND makes their state observable through a web UI.

> Source documents: `docs/SPEC.md` (authoritative v1 spec), `.planning/PROJECT.md` (locked decisions and constraints), `.planning/research/SUMMARY.md` (research synthesis), `.planning/research/FEATURES.md`, `.planning/research/PITFALLS.md`, `.planning/research/ARCHITECTURE.md`, `.planning/research/STACK.md`.

## v1 Requirements

Requirements for the initial release. Each maps to a roadmap phase via the Traceability table at the bottom of this document.

### Foundation (FOUND)

- [ ] **FOUND-01**: The project compiles as a single Cargo workspace targeting Rust edition 2021 (or 2024 if `bollard`/`sqlx` both compile cleanly), with `tokio` as the async runtime
- [ ] **FOUND-02**: `cronduit` accepts CLI flags via `clap` for `--config <path>`, `--bind <addr:port>`, `--database-url <url>`, and `--log-format <json|text>`
- [ ] **FOUND-03**: `cronduit --check <config>` validates a config file (parse + cron expression validation + network mode validation) and exits non-zero on any error without touching the database
- [ ] **FOUND-04**: Structured JSON logs are written to stdout via `tracing` + `tracing-subscriber` for Docker log collection
- [ ] **FOUND-05**: A `SecretString` newtype wraps any secret-bearing config field; its `Debug` impl never prints the value
- [ ] **FOUND-06**: `cargo tree -i openssl-sys` returns empty (rustls-only TLS stack); enforced by a CI check
- [ ] **FOUND-07**: `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo test` all pass on every PR via GitHub Actions
- [ ] **FOUND-08**: CI matrix runs against `linux/amd64` + `linux/arm64` and against both SQLite and PostgreSQL backends on every PR
- [ ] **FOUND-09**: A multi-arch Docker image (`linux/amd64` + `linux/arm64`) is built via `cargo-zigbuild` (no QEMU) and tagged on every push to `main`
- [ ] **FOUND-10**: `README.md` leads with a SECURITY section explaining the Docker socket threat model, the loopback default, and the no-auth-in-v1 stance; a `THREAT_MODEL.md` document captures the full threat model
- [ ] **FOUND-11**: All diagrams in repository documentation, READMEs, and PR descriptions are authored as mermaid code blocks (no ASCII art)

### Configuration (CONF)

- [ ] **CONF-01**: Operator authors job definitions in a TOML config file with a top-level `[server]`, optional `[defaults]`, and one or more `[[jobs]]` tables
- [ ] **CONF-02**: `${ENV_VAR}` references in any config string field are interpolated from the process environment at parse time; missing required vars fail loudly with the field path in the error message
- [ ] **CONF-03**: A `[defaults]` section provides default values for `image`, `network`, `volumes`, `delete`, `timeout`, and `random_min_gap` that apply to every job unless overridden
- [ ] **CONF-04**: A per-job `use_defaults = false` disables defaults entirely for that job
- [ ] **CONF-05**: Each `[[jobs]]` entry requires `name`, `schedule`, and exactly one of `command`, `script`, or `image`; the parser rejects any other combination with a clear error
- [ ] **CONF-06**: A job-level field always overrides the corresponding `[defaults]` field
- [ ] **CONF-07**: The config file is mounted read-only inside the Cronduit container in the example `docker-compose.yml`
- [ ] **CONF-08**: Cron expressions are parsed via `croner` 3.0 with a mandatory `[server].timezone` setting (no implicit host timezone fallback)
- [ ] **CONF-09**: Standard 5-field cron expressions (`minute hour dom month dow`) are accepted, including the standard step/range/list syntax and croner's `L`/`#`/`W` extensions
- [ ] **CONF-10**: Job names are unique within a config file; duplicate names fail to parse with both line numbers reported

### Persistence (DB)

- [ ] **DB-01**: SQLite is the default backend; passing `--database-url sqlite://...` or omitting the flag uses a SQLite file at the configured path with WAL mode and `busy_timeout=5000`
- [ ] **DB-02**: PostgreSQL is supported via `--database-url postgres://...` against the same logical schema (per-backend migration files where dialect requires)
- [ ] **DB-03**: Migrations run idempotently on startup via `sqlx::migrate!`; first run creates all tables, subsequent runs are no-ops
- [ ] **DB-04**: Schema includes `jobs`, `job_runs`, and `job_logs` tables matching the ER diagram in `.planning/research/ARCHITECTURE.md`
- [ ] **DB-05**: SQLite uses separate read and write `sqlx::Pool` instances (single-connection writer, multi-connection reader) to avoid writer-contention collapse under concurrent log writes
- [ ] **DB-06**: Each `jobs` row stores `schedule` (raw from config) and `resolved_schedule` (concrete cron after `@random` resolution) and a `config_hash` (SHA-256) used as the idempotency key for sync
- [ ] **DB-07**: Removed jobs are marked `enabled=0` rather than deleted; `job_runs` and `job_logs` for removed jobs remain queryable
- [x] **DB-08**: A daily retention pruner deletes `job_runs` and `job_logs` older than the configured `[server].log_retention` (default 90 days) in batched transactions

### Scheduler (SCHED)

- [ ] **SCHED-01**: A hand-rolled `tokio::select!` scheduler loop owns the cron clock; no external scheduler crate is used
- [ ] **SCHED-02**: The scheduler fires each enabled job at every match of its `resolved_schedule` in the configured timezone, including correct behavior across DST transitions (verified by a regression test suite)
- [ ] **SCHED-03**: When the wall clock jumps forward (DST or NTP correction), missed fire times in the skipped interval are not silently dropped: each is logged at WARN with the job name and the missed timestamp, and at most one catch-up run per skipped fire is enqueued
- [ ] **SCHED-04**: Each fired job runs as a `tokio::spawn`ed task that owns its lifecycle (`insert_running` → backend exec → log capture → `finalize`)
- [ ] **SCHED-05**: A per-job `timeout` is enforced via `tokio::select!`; on timeout the run is recorded as `status='timeout'` with the partial logs preserved
- [ ] **SCHED-06**: Concurrent runs of the same job are allowed (no v1 concurrency limits) and each is recorded as a separate `job_runs` row
- [ ] **SCHED-07**: Graceful shutdown on SIGINT/SIGTERM stops accepting new fires, waits up to `[server].shutdown_grace = "30s"` for in-flight runs to finish, then closes the pool and exits with code 0
- [ ] **SCHED-08**: A `cronduit.run_id=<id>` Docker label is set on every spawned container; on startup, any container matching `cronduit.run_id=*` whose `run_id` corresponds to a `job_runs` row in `status='running'` is reconciled (status set to `error` with `error_message='orphaned at restart'`)

### `@random` Scheduling (RAND)

- [ ] **RAND-01**: Any cron field in a job's `schedule` may be `@random`; the resolver picks a concrete value at sync time and persists it to `resolved_schedule`
- [ ] **RAND-02**: A randomized job's `resolved_schedule` is stable across process restarts and config reloads as long as the raw `schedule` field in config is unchanged
- [ ] **RAND-03**: A randomized job is re-randomized only when (a) it is newly added in config, (b) its `schedule` field changes in config, or (c) an explicit re-randomize is requested
- [ ] **RAND-04**: `[server].random_min_gap` (default `0s`) is enforced as a minimum spacing between fire times of randomized jobs on the same day; the resolver retries up to N attempts before logging a WARN and accepting the best candidate
- [ ] **RAND-05**: If the requested `random_min_gap` is mathematically infeasible for the number of randomized jobs, Cronduit logs a WARN at startup, relaxes the gap for overflow jobs, and continues — it does not fail to boot
- [ ] **RAND-06**: The web UI displays both the raw `schedule` and the `resolved_schedule` on the job detail page so the operator can see what `@random` resolved to

### Job Execution — Command/Script (EXEC)

- [ ] **EXEC-01**: Jobs of type `command` (declared via `command = "..."`) run as a local shell process via `tokio::process::Command`
- [ ] **EXEC-02**: Jobs of type `script` (declared via `script = """ ... """`) write the script body to a tempfile with the configured shebang (default `#!/bin/sh`) and execute it
- [ ] **EXEC-03**: stdout and stderr are captured line-by-line via piped `BufReader`s and written to `job_logs` with the correct `stream` tag; line ordering within each stream is preserved
- [ ] **EXEC-04**: A bounded channel decouples log producers from log writers; when the channel is full the oldest pending lines are dropped and a `[truncated N lines]` marker is inserted into `job_logs`
- [ ] **EXEC-05**: Lines longer than 16 KB are truncated with a marker; the configured cap is documented in the README
- [ ] **EXEC-06**: A successful run records `status='success'` and `exit_code=0`; non-zero exit records `status='failed'` and the actual exit code

### Job Execution — Docker (DOCKER)

- [ ] **DOCKER-01**: Jobs of type `docker` (declared via `image = "..."`) run as ephemeral containers via `bollard` connecting to `/var/run/docker.sock`
- [ ] **DOCKER-02**: All Docker network modes are supported: `bridge`, `host`, `none`, `container:<name>`, and any user-defined named network; the parser validates the syntax and the executor exercises each path
- [ ] **DOCKER-03**: Before starting a `network = "container:<name>"` job, the executor pre-flight checks that the target container exists and is running; on failure the run is recorded with a distinct `error_message='network_target_unavailable: <name>'` rather than bubbling Docker's raw error
- [ ] **DOCKER-04**: Volume mounts (`volumes = [...]`), environment variables (`env = {...}`), custom container names (`container_name = "..."`), and per-job `timeout` are all honored
- [ ] **DOCKER-05**: Container images are auto-pulled if not present locally; pull failures are retried with exponential backoff (3 attempts) and classified into a structured error
- [ ] **DOCKER-06**: Containers are spawned with **`auto_remove=false`** and explicitly removed by Cronduit after `wait_container` resolves and log draining completes (the bollard `auto_remove=true` race must be avoided)
- [ ] **DOCKER-07**: Every spawned container is labeled `cronduit.run_id=<run_id>` and `cronduit.job_name=<job_name>` (see SCHED-08 for orphan reconciliation)
- [ ] **DOCKER-08**: Container stdout/stderr are streamed via `bollard.logs(follow=true)` into the same `job_logs` pipeline as command/script backends, with chunk-based (not strictly line-based) storage to handle long output
- [ ] **DOCKER-09**: The Docker image digest used for each run is recorded in `job_runs.container_id` (or a sibling column) so post-mortem analysis can identify exactly which image ran
- [ ] **DOCKER-10**: An integration test using `testcontainers` covers the `network = "container:<name>"` path end-to-end (the marquee differentiator)

### Configuration Reload (RELOAD)

- [ ] **RELOAD-01**: A `SIGHUP` signal triggers a config reload without restarting the process
- [ ] **RELOAD-02**: A `POST /api/reload` endpoint triggers a config reload from the configured path
- [ ] **RELOAD-03**: A filesystem watcher (`notify` crate) triggers a debounced reload when the config file changes; debounce window is 500 ms to handle editor write-then-rename atomic saves
- [ ] **RELOAD-04**: A reload that fails to parse leaves the running configuration untouched and surfaces the parse error via the API response, log, and web UI toast
- [ ] **RELOAD-05**: A successful reload diffs the new config against the DB and applies create/update/disable changes idempotently using `config_hash`
- [ ] **RELOAD-06**: In-flight job runs are not cancelled on reload; they finish under their old config, and only future fires use the new config
- [ ] **RELOAD-07**: Removed jobs are marked `enabled=0` (history preserved per DB-07)

### Web UI (UI)

- [ ] **UI-01**: An axum HTTP server serves the embedded web UI on the configured bind address; static assets are embedded via `rust-embed`
- [ ] **UI-02**: HTML templating uses `askama_web` 0.15 with the `axum-0.8` feature (NOT the deprecated `askama_axum`)
- [ ] **UI-03**: Tailwind CSS is built at compile time via the standalone Tailwind binary (no Node toolchain) and the resulting `tailwind.css` is embedded
- [ ] **UI-04**: HTMX is vendored into `assets/vendor/htmx.min.js` and embedded; the UI never loads JS or CSS from a CDN
- [ ] **UI-05**: All UI pages match the `design/DESIGN_SYSTEM.md` terminal-green palette, monospace typography, and dark/light token system
- [ ] **UI-06**: A Dashboard page lists all enabled jobs with: name, raw schedule, resolved schedule, next fire time, last run status badge, and last run timestamp
- [ ] **UI-07**: The Dashboard refreshes via HTMX polling (`hx-trigger="every 3s"`) on the table partial; next-fire countdowns are computed server-side (superseded by D-03 in Phase 3 context session — original text said client-side)
- [ ] **UI-08**: A Job Detail page shows the full resolved config, the cron expression's human-readable description (via croner), and a paginated run history table
- [ ] **UI-09**: A Run Detail page shows run metadata (start/end/duration/exit_code/status/container_id) and the captured logs with stdout/stderr distinction and ANSI color codes parsed server-side into sanitized HTML spans
- [ ] **UI-10**: All log content rendered in the UI is HTML-escaped by default; ANSI parsing is the only allowed transformation
- [ ] **UI-11**: A Settings/Status page shows scheduler uptime, DB connection status, config file path, last successful reload time, and the Cronduit version
- [ ] **UI-12**: A "Run Now" button on each job triggers a manual run via `POST /api/jobs/:id/run`; the manual run is recorded with `trigger='manual'`
- [ ] **UI-13**: The Dashboard supports filter (substring match on job name) and sort (by name, last run, next run, status) via query params
- [x] **UI-14**: The Run Detail page log viewer streams new lines via SSE (`/events/runs/:id/logs`) for in-progress runs; completed runs render statically from `job_logs`
- [ ] **UI-15**: All state-changing endpoints (`/api/reload`, `/api/jobs/:id/run`) require a CSRF token bound to the user's session cookie

### Operational Endpoints (OPS)

- [ ] **OPS-01**: `GET /health` returns `200 OK` with a JSON body `{"status":"ok","db":"ok","scheduler":"running"}` when the process is healthy; the example `docker-compose.yml` healthcheck targets this endpoint
- [x] **OPS-02**: `GET /metrics` exposes Prometheus-format metrics including `cronduit_jobs_total`, `cronduit_runs_total{status}`, `cronduit_run_duration_seconds` (histogram), and `cronduit_run_failures_total{reason}` where `reason` is a closed enum (no unbounded label cardinality)
- [ ] **OPS-03**: Cronduit defaults `[server].bind` to `127.0.0.1:8080`; on startup, if the resolved bind address is non-loopback, a WARN-level log line is emitted explaining the no-auth-in-v1 stance and recommending a reverse proxy
- [x] **OPS-04**: An example `docker-compose.yml` is shipped in the repo with the Docker socket mounted, the config file mounted read-only, and a named volume for the SQLite database
- [x] **OPS-05**: The README quickstart enables a stranger to clone the repo, run `docker compose up`, and schedule a working job in under 5 minutes

## v2 Requirements

Deferred to a future release. Tracked but not in the v1 roadmap.

### Authentication & Authorization

- **AUTH-01** (v2): Optional token-based authentication for the web UI and API endpoints
- **AUTH-02** (v2): Optional basic-auth gateway for the web UI

### Notifications

- **NOTIF-01** (v2): Webhook on run completion (configurable per job, with templated payload)
- **NOTIF-02** (v2): Healthchecks.io-style ping URL on run start and completion
- **NOTIF-03** (v2): Email notifications via SMTP for run failures

### Advanced Job Features

- **ADV-01** (v2): Per-job concurrency limits (skip / queue / kill-running policies)
- **ADV-02** (v2): Job dependencies / DAG ("run B after A succeeds")
- **ADV-03** (v2): Run history replay (re-run a previous run with its captured config)

### Operational Polish

- **OPS-01** (v2): Job-level metrics labels (cardinality bounded by job count)
- **OPS-02** (v2): Backup/export of `jobs` table to a portable format
- **OPS-03** (v2): Multi-node coordination (single distributed scheduler) — explicitly not the v1 product

## Out of Scope

Explicitly excluded from the project. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Multi-node / distributed scheduling | Different product. Single-node is the entire v1 thesis. |
| User management / RBAC | Cronduit is a single-operator tool by design. No user accounts in v1 or v2. |
| Workflow DAGs / job dependencies (v1) | Out of scope for v1; tracked under ADV-02 in v2. Adds significant scheduler complexity. |
| ofelia config importer | Operators rewrite their schedules in Cronduit's TOML by hand. Not worth the translation surface area. |
| In-UI job CRUD | Config file is the source of truth. Adding UI CRUD would fight that thesis and pull Cronduit toward Cronicle's shape. |
| SPA / React frontend | Server-rendered HTML only. Preserves the single-binary story and matches the terminal aesthetic. |
| YAML / INI / JSON config formats | TOML is the locked format. `serde-yaml` is archived; YAML's required quoting around `*`/`@random` is hostile for cron configs. |
| Tailwind via CDN or Node-based build | Defeats the single-binary story; breaks in air-gapped homelabs. Standalone Tailwind binary is required. |
| Docker daemon over TCP/TLS | v1 only supports the Unix socket. TCP/TLS adds attack surface for marginal benefit at homelab scale. |
| Job duration trend charts in the UI | Prometheus + Grafana does this better. We expose `/metrics`; users bring their own dashboards. |
| Live log streaming via WebSocket | SSE is sufficient and simpler. WebSocket adds complexity for no v1 benefit. |
| Auto-remove containers via Docker's `auto_remove=true` | Races with `wait_container` and loses exit codes / logs (moby#8441). Cronduit removes containers explicitly. |

## Traceability

Every v1 requirement is mapped to exactly one phase. See `.planning/ROADMAP.md` for phase details.

| Requirement | Phase | Status |
|-------------|-------|--------|
| FOUND-01 | Phase 1 | Pending |
| FOUND-02 | Phase 1 | Pending |
| FOUND-03 | Phase 1 | Pending |
| FOUND-04 | Phase 1 | Pending |
| FOUND-05 | Phase 1 | Pending |
| FOUND-06 | Phase 1 | Pending |
| FOUND-07 | Phase 1 | Pending |
| FOUND-08 | Phase 1 | Pending |
| FOUND-09 | Phase 1 | Pending |
| FOUND-10 | Phase 1 | Pending |
| FOUND-11 | Phase 1 | Pending |
| CONF-01 | Phase 1 | Pending |
| CONF-02 | Phase 1 | Pending |
| CONF-03 | Phase 1 | Pending |
| CONF-04 | Phase 1 | Pending |
| CONF-05 | Phase 1 | Pending |
| CONF-06 | Phase 1 | Pending |
| CONF-07 | Phase 1 | Pending |
| CONF-08 | Phase 1 | Pending |
| CONF-09 | Phase 1 | Pending |
| CONF-10 | Phase 1 | Pending |
| DB-01 | Phase 1 | Pending |
| DB-02 | Phase 1 | Pending |
| DB-03 | Phase 1 | Pending |
| DB-04 | Phase 1 | Pending |
| DB-05 | Phase 1 | Pending |
| DB-06 | Phase 1 | Pending |
| DB-07 | Phase 1 | Pending |
| DB-08 | Phase 6 | Complete |
| SCHED-01 | Phase 2 | Pending |
| SCHED-02 | Phase 2 | Pending |
| SCHED-03 | Phase 2 | Pending |
| SCHED-04 | Phase 2 | Pending |
| SCHED-05 | Phase 2 | Pending |
| SCHED-06 | Phase 2 | Pending |
| SCHED-07 | Phase 2 | Pending |
| SCHED-08 | Phase 4 | Pending |
| RAND-01 | Phase 5 | Pending |
| RAND-02 | Phase 5 | Pending |
| RAND-03 | Phase 5 | Pending |
| RAND-04 | Phase 5 | Pending |
| RAND-05 | Phase 5 | Pending |
| RAND-06 | Phase 5 | Pending |
| EXEC-01 | Phase 2 | Pending |
| EXEC-02 | Phase 2 | Pending |
| EXEC-03 | Phase 2 | Pending |
| EXEC-04 | Phase 2 | Pending |
| EXEC-05 | Phase 2 | Pending |
| EXEC-06 | Phase 2 | Pending |
| DOCKER-01 | Phase 4 | Pending |
| DOCKER-02 | Phase 4 | Pending |
| DOCKER-03 | Phase 4 | Pending |
| DOCKER-04 | Phase 4 | Pending |
| DOCKER-05 | Phase 4 | Pending |
| DOCKER-06 | Phase 4 | Pending |
| DOCKER-07 | Phase 4 | Pending |
| DOCKER-08 | Phase 4 | Pending |
| DOCKER-09 | Phase 4 | Pending |
| DOCKER-10 | Phase 4 | Pending |
| RELOAD-01 | Phase 5 | Pending |
| RELOAD-02 | Phase 5 | Pending |
| RELOAD-03 | Phase 5 | Pending |
| RELOAD-04 | Phase 5 | Pending |
| RELOAD-05 | Phase 5 | Pending |
| RELOAD-06 | Phase 5 | Pending |
| RELOAD-07 | Phase 5 | Pending |
| UI-01 | Phase 3 | Pending |
| UI-02 | Phase 3 | Pending |
| UI-03 | Phase 3 | Pending |
| UI-04 | Phase 3 | Pending |
| UI-05 | Phase 3 | Pending |
| UI-06 | Phase 3 | Pending |
| UI-07 | Phase 3 | Pending |
| UI-08 | Phase 3 | Pending |
| UI-09 | Phase 3 | Pending |
| UI-10 | Phase 3 | Pending |
| UI-11 | Phase 3 | Pending |
| UI-12 | Phase 3 | Pending |
| UI-13 | Phase 3 | Pending |
| UI-14 | Phase 6 | Complete |
| UI-15 | Phase 3 | Pending |
| OPS-01 | Phase 3 | Pending |
| OPS-02 | Phase 6 | Complete |
| OPS-03 | Phase 1 | Pending |
| OPS-04 | Phase 6 | Complete |
| OPS-05 | Phase 6 | Complete |

**Coverage:**
- v1 requirements: 86 total
- Mapped to phases: 86
- Unmapped: 0 ✓

**Distribution by phase:**
- Phase 1 (Foundation, Security Posture & Persistence Base): 29 requirements
- Phase 2 (Scheduler Core & Command/Script Executor): 13 requirements
- Phase 3 (Read-Only Web UI & Health Endpoint): 15 requirements
- Phase 4 (Docker Executor & container-network Differentiator): 11 requirements
- Phase 5 (Config Reload & `@random` Resolver): 13 requirements
- Phase 6 (Live Events, Metrics, Retention & Release Engineering): 5 requirements

---
*Requirements defined: 2026-04-09*
*Last updated: 2026-04-09 after roadmap creation — traceability table populated with full 86/86 mapping*
