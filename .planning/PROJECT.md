# Cronduit

## Current State

**Shipped:** `v1.1.0` on 2026-04-23 — Operator Quality of Life polish milestone on top of the v1.0.1 codebase. Six phases (10, 11, 12, 12.1 inserted, 13, 14), 52 plans, 33/33 v1.1 requirements Complete. Adds a new `stopped` status with a per-run stop button wired through all three executors, per-job run numbers (`#1, #2, …`) backfilled via an idempotent three-file migration, zero-gap log backfill on navigate-back with id-based SSE dedupe, a new `/timeline` gantt page, dashboard sparklines + success-rate badges, job-detail p50/p95 duration trends, a CSRF-gated bulk enable/disable UX backed by a tri-state `jobs.enabled_override` column, a working out-of-the-box `docker compose up` healthcheck via a new `cronduit health` CLI + Dockerfile HEALTHCHECK, and a locked six-tag GHCR contract (`:X.Y.Z`, `:X.Y`, `:X`, `:latest`, `:rc`, `:main`). `:latest` promoted from `:1.0.1` to `:1.1.0` on both archs at final tag. No net-new external dependencies (one `rand 0.8 → 0.9` hygiene bump); one new nullable DB column. See [`MILESTONES.md`](../MILESTONES.md) and [`.planning/MILESTONES.md`](MILESTONES.md) for full history.

**Prior:** `v1.0.0` (2026-04-14) + `v1.0.1` patch (2026-04-14) — single-binary Rust cron scheduler with terminal-green HTMX web UI, full Docker-API job execution including `--network container:<name>`, `@random` schedule resolver, hot config reload, Prometheus metrics, SSE log tail, multi-arch (amd64+arm64) GHCR release, and a documented threat model. 86/86 v1 requirements complete; audit verdict `passed`.

**Next milestone:** v1.2 — Operator Integration & Insight (in progress; kicked off 2026-04-25). Goal: make cronduit a participant in the operator's broader infrastructure — push notifications outward via webhooks, expose richer failure context inward, and let operators organize and integrate via tags and Docker labels. Five features in scope: webhook notifications, failure context on run detail, per-job exit-code histogram, job tagging/grouping, custom Docker labels (SEED-001). Cross-run log search and job concurrency/queuing punted to v1.3.

## What This Is

Cronduit is a self-hosted cron job scheduler with a web UI, built for Docker-native homelab environments. It's a single Rust binary that runs recurrent tasks — local commands, inline scripts, or ephemeral Docker containers — and gives operators a terminal-green web dashboard to see exactly what ran, when, and how it went.

## Core Value

**One tool that both runs recurrent jobs reliably AND makes their state observable through a web UI.** If everything else is cut, the scheduler must (1) execute jobs on time with full Docker networking support (especially `--network container:<name>` for VPN setups) and (2) let the operator see pass/fail, logs, and timing from a browser.

## Current Milestone: v1.2 — Operator Integration & Insight

**Goal:** Make cronduit a participant in the operator's broader infrastructure — push notifications outward via webhooks, expose richer failure context inward, and let operators organize and integrate their fleet via tags and Docker labels.

**Theme:** Expand — net-new operator-facing surface (substantially more new capability than v1.1's polish milestone). No scheduler-core refactor; primarily additive on top of the v1.1.0 codebase. Iterative `v1.2.0-rc.N` cadence expected with multiple rc cuts.

**Target features (5):**

*Outbound integration*
- Webhook notifications on job state transitions — per-job URL + state-filter list (e.g. `["failed", "timeout", "stopped"]`); optional HMAC signing key per job; 3 attempts with exponential backoff retry. Configurable in `[defaults]` and per `[[jobs]]`; `use_defaults = false` disables defaults fallback (parallels SEED-001 / Docker labels override pattern).
- Custom Docker labels on spawned containers (SEED-001) — `labels` map in `[defaults]` and per `[[jobs]]`, plumbed through to bollard `Config::labels`. Merge semantics, `cronduit.*` reserved namespace, and type-gating (docker-only) all locked at seed time. Same `[defaults]` + per-job + `use_defaults = false` override pattern as webhooks.

*Insight on existing runs*
- Failure context on run detail — time-based deltas (first-failure timestamp, consecutive-failure streak, link to last successful run) plus image-digest delta plus config-hash delta. Requires recording image digest at run-start (new column on `job_runs`).
- Per-job exit-code histogram on job detail page — new card showing exit-code distribution over the last N runs (parallels the v1.1 p50/p95 duration card). Bucketed by exit code; inline server-rendered.

*Organization*
- Job tagging / grouping — `tags = ["backup", "weekly"]` on jobs; dashboard adds filter chips. UI-only — does NOT affect webhooks, search, or metrics labels (avoids unbounded Prometheus cardinality).

**Release strategy:** Iterative `v1.2.0-rc.1`, `v1.2.0-rc.2`, ... cut at chunky checkpoints (likely after each functional block). `:latest` GHCR tag stays at `v1.1.0` until final `v1.2.0`. Phase numbering continues from v1.1 (which ended at Phase 14, with 12.1 inserted) — v1.2 starts at Phase 15.

**Punted to v1.3 at kickoff:**

- Cross-run log search across retention window — design ambiguity around naive LIKE vs SQLite FTS5 / Postgres tsvector engine choice; let v1.2 ship and observe usage data first.
- Job concurrency limits and queuing — deep scheduler-core change (`tokio::select!` loop + persistence + fairness); too risky to bundle with v1.2's expand-shape work. Already on the v1.3 candidate list.

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

**Scheduler core (Rust)** — v1.0
- ✓ Parse standard 5-field cron expressions and schedule jobs reliably with async runtime (tokio) — v1.0 (Phase 2, via `croner` 3.0)
- ✓ `@random` field support — any cron field can be randomized at startup, persisted until restart or re-randomize — v1.0 (Phase 5, slot-based resolver)
- ✓ Configurable minimum gap between randomized jobs on the same day (`random_min_gap = "90m"`) — v1.0 (Phase 5)
- ✓ Idempotent startup — first run creates tables, subsequent runs are safe no-ops — v1.0 (Phase 1)
- ✓ Graceful shutdown — wait for running jobs with configurable timeout — v1.0 (Phase 2, double-signal SIGINT/SIGTERM drain)
- ✓ Config reload via SIGHUP or API endpoint without full restart — v1.0 (Phase 5; SIGHUP + `POST /api/reload` + debounced file-watch)

**Job execution** — v1.0
- ✓ Run local shell commands (`type = "command"`) — v1.0 (Phase 2, via `shell-words`)
- ✓ Run inline scripts (`type = "script"`) — v1.0 (Phase 2, via tempfile + shebang)
- ✓ Spawn Docker containers via the Docker API using `bollard` (NOT the docker CLI) — v1.0 (Phase 4, `bollard` 0.20)
- ✓ Support all Docker network modes: `bridge`, `host`, `none`, `container:<name>`, named networks — v1.0 (Phase 4, `container:<name>` validated by `testcontainers` integration test)
- ✓ Custom per-job container name — v1.0 (Phase 4)
- ✓ Volume mounts, environment variables, `--rm` semantics on completion — v1.0 (Phase 4, via explicit post-drain remove to avoid moby#8441)
- ✓ Auto-pull image if not present locally — v1.0 (Phase 4, with 3-attempt exponential backoff)
- ✓ Per-job execution timeout — v1.0 (Phase 2/4)

**Configuration** — v1.0
- ✓ File-based config is the source of truth (hand-written, no ofelia importer) — v1.0 (Phase 1)
- ✓ `[defaults]` section applies to all jobs unless overridden per-job — v1.0 (Phase 1)
- ✓ Per-job override of any default field (including `use_defaults = false`) — v1.0 (Phase 1)
- ✓ Startup sync: create missing jobs, update changed jobs, disable removed jobs, preserve history — v1.0 (Phase 1/5)
- ✓ Environment variable interpolation (`${ENV_VAR}`) — no plaintext secrets in config — v1.0 (Phase 1, `SecretString` wrapper)
- ✓ Config file mounted read-only — v1.0 (Phase 1; documented in quickstart)
- ✓ Primary format: TOML — v1.0 (Phase 1, locked by research)

**Persistence** — v1.0
- ✓ SQLite default, zero-config (via `sqlx`) — v1.0 (Phase 1, separate read/write WAL pools)
- ✓ PostgreSQL optional for shared infrastructure — v1.0 (Phase 1, structural parity tested via testcontainers)
- ✓ Auto-create tables on first run, built-in migrations — v1.0 (Phase 1)
- ✓ Store job definitions, run history, run logs — v1.0 (Phase 1/2)
- ✓ Configurable log retention (default 90 days) — v1.0 (Phase 6, daily pruner with batched deletes + WAL checkpoint)

**Web UI** — v1.0
- ✓ Tailwind CSS styled to the Cronduit design system (terminal-green, monospace) — v1.0 (Phase 3, standalone Tailwind binary, no Node)
- ✓ Embedded static assets served by the Rust backend (`rust-embed`) — single binary — v1.0 (Phase 3)
- ✓ Server-rendered HTML (`askama_web` 0.15) with HTMX — v1.0 (Phase 3, vendored HTMX 2.0.4)
- ✓ Dashboard: list of all jobs, recent-run grid, next run, last-run status badge — v1.0 (Phase 3)
- ✓ Job detail: full resolved config, run history, per-run metadata — v1.0 (Phase 3)
- ✓ Run detail: stdout/stderr logs, metadata — v1.0 (Phase 3, ANSI rendering)
- ✓ Settings/status page — v1.0 (Phase 3)
- ✓ Manual "Run Now" button per job — v1.0 (Phase 3, via `mpsc` channel into scheduler)
- ✓ Auto-refresh / live updates for running jobs — v1.0 (Phase 3 polling + Phase 6 SSE log tail + Phase 7 run-history partial polling fix)
- ✓ Filter/search jobs by name; sort by name, last run, next run, status — v1.0 (Phase 3, parameterized filter + whitelisted sort)

**Operational** — v1.0
- ✓ Structured JSON logs to stdout (`tracing`) for Docker log collection — v1.0 (Phase 1)
- ✓ `GET /health` returning scheduler status — v1.0 (Phase 3, with DB connectivity check)
- ✓ `GET /metrics` exposing Prometheus-compatible metrics — v1.0 (Phase 6, five eagerly-described families on bounded-cardinality labels)

**Packaging & deployment** — v1.0
- ✓ Ships as a multi-arch Docker image (amd64+arm64) and a single static-ish binary — v1.0 (Phase 1/6, via `cargo-zigbuild`, no QEMU)
- ✓ Ships with example `docker-compose.yml` mounting Docker socket, config, data volume — v1.0 (Phase 6/7/8; quickstart expanded to 4 example jobs in Phase 8)

**Quality & release engineering** — v1.0
- ✓ Unit and integration tests — v1.0 (49 plans across 9 phases; `testcontainers` for Docker + Postgres integration)
- ✓ GitHub Actions CI: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, multi-arch container build — v1.0 (Phase 1/9, just-only enforcement, openssl-check guard, `compose-smoke` job validating both compose examples)
- ✓ README documentation sufficient for a new user to self-host — v1.0 (Phase 6/8, SECURITY-first README + `THREAT_MODEL.md`, validated by Phase 8 user walkthrough)

**Scheduler / execution — v1.1**
- ✓ Operator can stop a running job from the UI — `stopped` status distinct from `cancelled`/`failed`/`timeout`, single hard kill, works identically for command/script/docker executors — v1.1 (Phase 10, SCHED-09..14)
- ✓ Run records carry a per-job sequential number (`job_run_number`) alongside the global ID, backfilled on upgrade via an idempotent three-file migration — v1.1 (Phase 11, DB-09..13)

**Deployment / packaging — v1.1**
- ✓ `docker compose up` with the shipped quickstart compose file reports the cronduit container as `healthy` out of the box — new `cronduit health` CLI + Dockerfile HEALTHCHECK (removed the busybox `wget --spider` dependency entirely) — v1.1 (Phase 12, OPS-06..08)
- ✓ GHCR `:latest` tag only tracks released non-rc stable tags; new `:main` floating tag for bleeding-edge main builds; six-tag contract (`:X.Y.Z`, `:X.Y`, `:X`, `:latest`, `:rc`, `:main`) documented in README — v1.1 (Phase 12.1, OPS-09..10)

**Log tail / UX — v1.1**
- ✓ Navigating back to a running job's page shows prior log lines from DB then attaches to the live SSE stream without a gap (id-based dedupe via `data-max-id`) — v1.1 (Phase 11, UI-17, UI-18)
- ✓ Log lines remain in chronological order across the live→static transition when a run completes — v1.1 (Phase 11, UI-18)
- ✓ Transient "error getting logs" race on page load eliminated — sync insert on the API handler thread before returning the response — v1.1 (Phase 11, UI-19)
- ✓ Per-job run numbers shown as the primary identifier in the run-history partial and run-detail breadcrumb; global id kept as a troubleshooting hint — v1.1 (Phase 11, UI-16, UI-20)

**Observability polish — v1.1**
- ✓ `/timeline` gantt-style run timeline (24h default, 7d toggle); single SQL query bounded by `LIMIT 10000`; EXPLAIN-verified on both SQLite and Postgres — v1.1 (Phase 13, OBS-01, OBS-02)
- ✓ Dashboard sparklines + success-rate badges on every job card (`N=5` minimum, `stopped` excluded from denominator) — v1.1 (Phase 13, OBS-03)
- ✓ Job detail page p50/p95 duration trends over the last 100 successful runs (`N=20` minimum) via Rust-side `stats::percentile` — v1.1 (Phase 13, OBS-04, OBS-05)
- ✓ SQL-native percentile functions (`percentile_cont`) NOT used — structural parity locked via CI grep guard — v1.1 (Phase 13, OBS-05)

**Ergonomics — v1.1**
- ✓ Operator can multi-select jobs on the dashboard and bulk enable/disable them via a CSRF-gated `POST /api/jobs/bulk-toggle`; disabled state persists in a tri-state `jobs.enabled_override` column that survives config reloads (Airflow-style override model) — v1.1 (Phase 14, ERG-01, ERG-02, DB-14)
- ✓ Settings page shows a "Currently overridden" audit section so operators never forget about manually-disabled jobs — v1.1 (Phase 14, ERG-03)
- ✓ Reload (SIGHUP / API / file-watch) does NOT reset `enabled_override`; `upsert_job` never touches the column; `disable_missing_jobs` clears it at the same time as `enabled=0` — v1.1 (Phase 14, ERG-04)

**Foundation / hygiene — v1.1**
- ✓ `rand` crate bumped from `0.8` to `0.9` across all call sites (`@random` slot picker, CSRF token gen) — v1.1 (Phase 10, FOUND-12)
- ✓ `Cargo.toml` version bumped from `1.0.1` to `1.1.0` on the first v1.1 commit; rc tags use semver pre-release format (`v1.1.0-rc.1` etc.) — v1.1 (Phase 10, FOUND-13)

### Active

<!-- Current scope. Building toward these. Hypotheses until shipped. -->

**v1.2 — Operator Integration & Insight** (see `REQUIREMENTS.md` for the full testable list with REQ-IDs once generated)

*Outbound integration*
- [ ] Webhook notifications on job state transitions — per-job URL + state-filter list; HMAC signing; 3-attempt exponential backoff retry; `[defaults]` fallback with per-job override and `use_defaults = false` disable
- [ ] Custom Docker labels on spawned containers (SEED-001) — `labels` map in `[defaults]` and per `[[jobs]]`; merge semantics + `cronduit.*` reserved namespace + type-gating locked at seed time

*Insight on existing runs*
- [ ] Failure context on run detail — time-based deltas (first-failure timestamp, streak, last-success link) + image-digest delta + config-hash delta; new `job_runs.image_digest` column with backfill
- [ ] Per-job exit-code histogram on job detail page — new card showing distribution over the last N runs

*Organization*
- [ ] Job tagging / grouping — `tags = ["backup", "weekly"]`; UI-only filter chips on dashboard; does NOT affect webhooks, search, or metrics labels

### Future Requirements

<!-- Scoped but not in the current milestone. Target versions are intent, not contracts — they may shift when that milestone is actually kicked off. -->

**v1.3 — Search + concurrency + ergonomics deepening (tentative)**
- Cross-run log search across retention window — engine choice (naive LIKE vs SQLite FTS5 / Postgres tsvector) deferred from v1.2 kickoff for usage-data-driven decision
- Job concurrency limits and queuing (deep scheduler-core change; affects the `tokio::select!` loop + persistence + fairness)
- Snooze a job for a duration (`until tomorrow 8am`, `for 2 hours`) without editing the config; auto-re-enable
- Run history filters (status, date range, exit code) and sortable columns

**v1.4 — UX polish (tentative)**
- Job duplicate-as-snippet (UI emits a TOML block to paste into the config)
- Fuzzy job search (`back` → `backup-postgres`)

### Out of Scope

<!-- Explicit boundaries. v2+ or never. -->

- **Web UI authentication (basic auth / token)** — deferred to v2. v1 assumes a trusted LAN deployment; if the operator needs auth, they front it with their existing reverse proxy. Revisit when first external users ask for it.
- **Multi-node / distributed scheduling** — single-node only. Distribution is a different product.
- **User management / RBAC** — single-operator tool; no user accounts in v1 or v2.
- **Workflow DAGs / job dependencies** — no "run B after A succeeds". Jobs are independent.
- **Email notifications** — post-v1 add-on; can layer on top of the metrics/log outputs. Webhook notifications have been promoted to Future Requirements (v1.2); email notifications remain out of scope entirely (operators can wire a webhook → email bridge if they want it).
- **Ad-hoc one-shot runs not defined in the config** — config remains the single source of truth for what runs. Adding a UI form that accepts arbitrary commands/images would create a blast-radius surface that pairs poorly with v1's unauthenticated posture.
- **Importer for existing ofelia configs** — users rewrite their schedules in Cronduit's TOML by hand. Not worth the translation surface area.
- **SPA / React frontend** — server-rendered HTML only. Keeps the single-binary story and matches the terminal aesthetic.

## Context

**Who this is for.** First user is Robert's own homelab. v1.0 ships as public OSS at `github.com/SimplicityGuy/cronduit` — the tool is intended for outside adopters from day one, with docs and quality bar to match.

**Codebase state at v1.1.0 (2026-04-23).**
- ~14,500 lines of Rust in `src/` + expanded integration test suite (new `tests/stop_executors.rs`, `tests/process_group_kill.rs`, `tests/metrics_stopped.rs`, `tests/v11_bulk_toggle.rs` + `_pg.rs`, `tests/v13_timeline_explain.rs`, `tests/v13_timeline_timezone.rs`, `tests/dashboard_jobs_pg.rs` among others)
- Edition 2024, rust-version 1.94.1
- Tech stack unchanged from v1.0: tokio, axum 0.8, askama_web 0.15, sqlx 0.8, bollard 0.20, croner 3.0, Tailwind (standalone binary; upgraded v3 → v4 at start of v1.1), HTMX 2.0.4 (vendored). One dependency bump: `rand 0.8 → 0.9`.
- 101 plans executed across 15 phases to date (v1.0 Phases 1–9: 49 plans; v1.1 Phases 10, 11, 12, 12.1, 13, 14: 52 plans)
- CI matrix: `linux/{amd64,arm64} × {SQLite, Postgres}` + compose-smoke quickstart regression + OBS-05 `grep-no-percentile-cont` structural-parity gate
- Release artifacts at v1.1.0: multi-arch image at `ghcr.io/SimplicityGuy/cronduit:v1.1.0` with tag-family expansion to `:X.Y.Z`, `:X.Y`, `:X`, `:latest`, `:rc`, `:main`

**Codebase state at v1.0.0.**
- ~14,000 lines of Rust (~10k src, ~4k tests)
- 49 plans across 9 phases in 6 calendar days (2026-04-08 → 2026-04-14)
- CI matrix + release artifacts as above (v1.0.0/v1.0.1 tags)

**Why it exists.** Existing schedulers don't cover the homelab Docker-native use case well:

- `ofelia` — no `--network container:<name>` support (critical for VPN-bound jobs).
- `docker-crontab` — shells out to the `docker` CLI, lacks a web UI, no persistent run history.
- `Cronicle`, `xyOps` — heavier, not designed around Docker-native job definitions.
- Host crontabs + scattered systemd timers — no unified observability.

Cronduit collapses those into one tool: define jobs in a config file, get a dashboard, reliably run containers on any Docker network mode.

**Validated v1.0 hypotheses.**
- The "single binary + docker-compose mount" deployment shape works end-to-end on Docker-on-macOS (the hardest target for the Docker socket path), validated by the Phase 8 operator walkthrough.
- The `metrics` facade + `metrics-exporter-prometheus` decoupling holds up — six cronduit families are eagerly described at boot with bounded-cardinality labels.
- Hand-rolling the scheduler loop on `tokio::select!` (instead of `tokio-cron-scheduler`) was the right call — needed for `@random` + `random_min_gap` + in-flight survival across reload.
- The `bollard` `auto_remove=false` + explicit post-drain remove pattern is required to avoid losing exit codes on container teardown (moby#8441 confirmed during Phase 4).

**Validated v1.1 hypotheses.**
- Polish-and-fix milestones land safely on top of a shipped v1.0 without refactoring the scheduler core — Phase 14's tri-state `enabled_override` avoided any `SchedulerCmd` loop changes, just a `Reload` fire after DB mutation.
- Option A (insert-then-broadcast with `RETURNING id`) is cheap enough for log dedupe — T-V11-LOG-02 benchmark confirmed p95 insert latency < 50ms for 64-line batches on SQLite.
- Three-file migrations (add nullable → backfill → add NOT NULL) are the right shape for schema-tightening changes — partial-failure recovery was never tested in anger during v1.1 but the shape remains the defensible default.
- Rust-side percentile computation (not `percentile_cont`) is acceptable for dashboard-sized windows — performance dominated by the `LIMIT 100` scan, not the percentile math.
- UAT-driven rc-loop (rc.3 → rc.6 on Phase 14) catches real operator-visible bugs (dashboard reflection, timeline bar CSS, `just` recipes, self-polling partials) that unit + integration tests missed. Worth the four extra rc cuts.
- Maintainer-action tag cuts (D-13) scale cleanly — rc.1 through v1.1.0 all landed without the maintainer fighting the workflow.

## Constraints

- **Tech stack (locked)**: Rust backend using `bollard` for the Docker API. No CLI shelling out. No alternative languages.
- **Persistence (locked)**: `sqlx` with SQLite default and PostgreSQL optional. Same logical schema, per-backend migration files where dialect requires. Separate read/write SQLite pools (WAL + busy_timeout).
- **Frontend (locked)**: Tailwind CSS + server-rendered HTML via `askama_web` 0.15 with the `axum-0.8` feature (NOT the deprecated `askama_axum` crate). HTMX-style live updates. No React/Vue/Svelte.
- **Config format (locked)**: TOML. `serde-yaml` is archived on GitHub and the YAML ecosystem is fragmented; research phase confirmed TOML is the right call.
- **Cron crate (locked)**: `croner` 3.0 — DST-aware (Vixie-cron-aligned), supports `L`/`#`/`W` modifiers, has human-readable descriptions. NOT the `cron` crate or `saffron` (abandoned 2021).
- **TLS / cross-compile (locked)**: rustls everywhere. `cargo tree -i openssl-sys` must return empty. Multi-arch (amd64 + arm64) via `cargo-zigbuild`, not QEMU emulation.
- **Deployment shape**: Single binary + Docker image. Cronduit itself runs inside Docker, mounting the host Docker socket.
- **Security posture**: No plaintext secrets in the config file; interpolate from env, wrap in a `SecretString` type. Config mounted read-only. **Default bind `127.0.0.1:8080`**; loud startup warning if bind is non-loopback. Web UI ships unauthenticated in v1 — operators are expected to either keep Cronduit on loopback / trusted LAN or front it with their existing reverse proxy. Threat model documented in `THREAT_MODEL.md`; README leads with a security section.
- **Quality bar**: Tests + GitHub Actions CI from phase 1. Clippy + fmt gate on CI. CI matrix covers `linux/amd64 + linux/arm64 × SQLite + Postgres`. README sufficient for a stranger to self-host.
- **Design fidelity**: Web UI must match `design/DESIGN_SYSTEM.md` (Cronduit terminal-green brand), not ship in default Tailwind look.
- **Documentation**: All diagrams in any project artifact (planning docs, README, PR descriptions, code comments) must be authored as mermaid code blocks. No ASCII art diagrams.
- **Workflow**: All changes land via pull request on a feature branch. No direct commits to `main`.
- **Versioning**: Git tag and the `version` in `Cargo.toml` (and any other code/build version strings) must always match. Prefer full three-part semver (`v1.0.0`).

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust for the backend | Performance, reliability, single binary, strong async story via tokio, good Docker client (`bollard`) | ✓ Settled (v1.0) — 14k LOC, ~7s release-mode build, 6-day v1.0 turnaround |
| `bollard` Docker client (not CLI) | Direct API access = all network modes work (`container:<name>`), no shell-out fragility, better error surface | ✓ Settled (v1.0) — `container:<name>` validated by `testcontainers` integration test |
| SQLite default, PostgreSQL optional | Zero-config for homelabs, upgrade path for shared deployments | ✓ Settled (v1.0) — structural parity tested both backends |
| Separate read/write SQLite pools (WAL + busy_timeout) | Avoids writer-contention collapse under concurrent log writes | ✓ Settled (v1.0, Phase 1) |
| `croner` 3.0 for cron parsing | DST-aware (Vixie-aligned), supports `L`/`#`/`W`, has human-readable descriptions; `cron` crate is too limited and `saffron` is abandoned | ✓ Settled (v1.0) — DST spring-forward case fixed mid-Phase-2 |
| Hand-roll the scheduler loop on `tokio::select!` | `tokio-cron-scheduler` lacks SQLite persistence and would create a dual source of truth with our `jobs` table | ✓ Settled (v1.0) — required for `@random` + in-flight survival across reload |
| `askama_web` 0.15 with `axum-0.8` feature | `askama_axum` is deprecated; `askama_web` is the supported integration crate | ✓ Settled (v1.0, Phase 3) |
| HTMX polling for dashboard, SSE only for log tail | Keeps SSE subscriber count low; polling is debuggable and cache-friendly | ✓ Settled (v1.0, Phase 6) |
| Tailwind + server-rendered HTML (no SPA) | Matches single-binary goal, fits terminal aesthetic, no JS build complexity | ✓ Settled (v1.0) |
| Tailwind via standalone binary (no Node) | Preserves single-binary toolchain story | ✓ Settled (v1.0, Phase 3) |
| `rust-embed` 8.x for static assets | Debug-mode disk-read makes the inner dev loop 10× faster than `include_dir` | ✓ Settled (v1.0) |
| TOML as the locked config format | `serde-yaml` is archived on GitHub; YAML's required quoting around `*`/`@random` is hostile for cron configs | ✓ Settled by research |
| rustls everywhere (zero `openssl-sys`) | Cross-compile cleanliness for musl/arm64; `cargo-zigbuild` build path | ✓ Settled (v1.0, openssl-check guard in CI) |
| Default bind `127.0.0.1`, loud warning on non-loopback | "No auth in v1" must be paired with safe-by-default network exposure | ✓ Settled (v1.0, Phase 1) |
| Never `auto_remove=true` on bollard containers | Races with `wait_container` and loses exit codes / truncates logs (moby#8441) | ✓ Settled (v1.0, Phase 4) — explicit post-drain remove pattern |
| Label every spawned container `cronduit.run_id=<id>` | Required for orphan reconciliation on restart; otherwise DB rows stick in `status=running` | ✓ Settled (v1.0, Phase 4) |
| Web UI auth deferred to v2 | v1 assumes loopback / trusted LAN / reverse-proxy fronting; threat model documented in `THREAT_MODEL.md` | ✓ Settled — revisit in v2 if external users ask |
| Tests + CI required in phase 1 | Public OSS release — external adopters need a quality signal from day one | ✓ Settled — green CI throughout v1.0 |
| CI matrix: amd64 + arm64 × SQLite + Postgres from phase 1 | Schema parity and cross-compile breakage are otherwise discovered too late | ✓ Settled (v1.0, Phase 1) |
| All diagrams must be mermaid; no ASCII art | Renders natively on GitHub; diff-friendly; readable on narrow viewports | ✓ Settled |
| All changes land via PR on a feature branch | No direct commits to `main` | ✓ Settled |
| No ofelia import path | Keeps scope small; operators rewrite schedules once | ✓ Settled |
| `auto_remove=false` + explicit `wait_container` then remove (Phase 4) | Avoid moby#8441 race that truncates logs / loses exit codes | ✓ Settled (v1.0) |
| Distroless → `alpine:3` runtime image (Phase 8) | Quickstart needed busybox for command/script example jobs to work end-to-end on first `docker compose up`; distroless lacks shell utilities the example jobs depend on | ✓ Settled (v1.0, Phase 8) — UID/GID 1000 preserved |
| Tag and `Cargo.toml` version must match (always full semver) | Operator support / reproducibility — `cronduit --version` must equal the git tag | ✓ Settled (v1.0.0) |
| Phase 9 has no v1 REQ-IDs ("n/a — operational hygiene phase") | Phase 9 added after v1 requirement set was locked; backfilling synthetic CI-* IDs would distort the traceability table | ✓ Settled (v1.0, Phase 9) — documented in `v1.0-MILESTONE-AUDIT.md` |
| `RunControl` abstraction for Stop (not `kill_on_drop(true)`) | `kill_on_drop(true)` would orphan shell-pipeline grandchildren; `.process_group(0)` + `libc::kill(-pid, SIGKILL)` is the correct pattern (Research Correction #1) | ✓ Settled (v1.1, Phase 10) — process-group regression lock in `tests/process_group_kill.rs` |
| `stopped` is a distinct terminal status, not `cancelled` | Operators need to tell "operator killed this" apart from "shutdown drained this" in both UI and metrics | ✓ Settled (v1.1, Phase 10) — metrics eagerly-declared label, CSS token, THREAT_MODEL note |
| Dedicated `jobs.next_run_number` counter column (not `MAX + 1`) | Identical on SQLite + Postgres, avoids dialect-specific locking, race-free by design | ✓ Settled (v1.1, Phase 11) — DB-11 locked by T-V11-RUNNUM-10..11 |
| Three-file per-backend migrations for `job_run_number` (add nullable → backfill → NOT NULL) | Partial-failure recovery requires the split; combined migrations are unrecoverable | ✓ Settled (v1.1, Phase 11) — DB-10 pattern reused in `jobs.enabled_override` Phase 14 migration |
| Log dedupe: Option A (insert-then-broadcast with `RETURNING id`) | Benchmark T-V11-LOG-02 confirmed p95 insert latency < 50ms for 64-line batches on SQLite | ✓ Settled (v1.1, Phase 11) — UI-20 |
| `/timeline` is a separate page, single SQL query bounded by `LIMIT 10000` | Dashboard query must stay tight; N+1 per-job scan would explode on large job fleets | ✓ Settled (v1.1, Phase 13) — OBS-01, OBS-02, EXPLAIN verified on both backends |
| Rust-side percentile (no `percentile_cont`) | Structural parity requires identical code path on SQLite + Postgres | ✓ Settled (v1.1, Phase 13) — OBS-05 locked by `just grep-no-percentile-cont` CI guard |
| `jobs.enabled_override` nullable tri-state (NULL = follow config, 0 = force disabled, 1 = force enabled) | Airflow-style; preserves config-source-of-truth semantics; `upsert_job` never touches the column | ✓ Settled (v1.1, Phase 14) — DB-14 locked by T-V11-BULK-01 |
| Bulk disable does NOT terminate running jobs | Toast communicates this explicitly; operators use the Stop button (Phase 10) for forceful termination | ✓ Settled (v1.1, Phase 14) — ERG-02 |
| No confirmation dialog for Stop or Bulk Disable | Consistent with Run Now (no confirmation); toast-only UX across all three | ✓ Settled (v1.1, Phases 10 + 14) |
| `cronduit health` CLI + Dockerfile `HEALTHCHECK` (replaces busybox `wget --spider`) | Removes an ecosystem dependency with observable parsing quirks on chunked axum responses | ✓ Settled (v1.1, Phase 12) — OPS-06, OPS-07 |
| D-10: `release.yml` rc-tag gating so rc pushes do NOT move `:latest` | rc tags are pre-releases by semver; `:latest` is the non-rc-stable contract | ✓ Settled (v1.1, Phase 12) — OPS-07 |
| D-13: rc tag cuts are maintainer-action, NOT `workflow_dispatch` | Signing-key trust anchor lives on the maintainer's workstation, not in a GHA runner identity | ✓ Settled (v1.1, Phase 12) — carried forward to every subsequent rc/stable cut |
| Six-tag GHCR contract (`:X.Y.Z`, `:X.Y`, `:X`, `:latest`, `:rc`, `:main`) | Documented negative-space: no `:edge`, `:nightly`, `:dev`, or per-branch tags | ✓ Settled (v1.1, Phase 12.1) — OPS-09, OPS-10, README six-row table |
| `:main` floating tag: multi-arch build on every main push | Bleeding-edge operators pin `:main`; parity with release.yml build plumbing | ✓ Settled (v1.1, Phase 12.1) — OPS-10 |
| Retroactive `:latest` retag via `docker buildx imagetools create` (not rebuild) | Manifest-list re-pointing is instant and idempotent; preserves per-platform layer digests | ✓ Settled (v1.1, Phase 12.1) — OPS-09 retroactive half |
| UAT-driven rc loop (rc.3 → rc.6 on Phase 14) | Each UAT pass surfaces real operator-visible bugs that unit/integration tests missed; fixes land in-cycle, not on main | ✓ Settled (v1.1, Phase 14) — four fix PRs (#39, #40, #41) before `v1.1.0` tag |
| `mark_run_orphaned` `WHERE status = 'running'` guard locked in by test | Research Correction #4 — without the guard, restart would overwrite `stopped`/`success`/`failed`/`timeout` rows | ✓ Settled (v1.1, Phase 10) — SCHED-13 |
| Tailwind v3 → v4 migration landed at start of v1.1 | Dep refresh + Tailwind upgrade as a single PR avoided mixed-state churn later in the milestone | ✓ Settled (v1.1, PR #26) |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-04-25 — v1.2 milestone "Operator Integration & Insight" kicked off via `/gsd-new-milestone`. Five features: webhooks (override pattern), failure context on run detail (time + image-digest + config-hash deltas), per-job exit-code histogram, job tagging (UI-only), custom Docker labels (SEED-001). Cross-run log search and job concurrency/queuing punted to v1.3. Phase numbering continues from v1.1's last (Phase 14) → v1.2 starts at Phase 15. Previous: 2026-04-24 — v1.1 milestone closed.*
