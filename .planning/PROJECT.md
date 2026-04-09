# Cronduit

## What This Is

Cronduit is a self-hosted cron job scheduler with a web UI, built for Docker-native homelab environments. It's a single tool that runs recurrent tasks (local commands, inline scripts, or ephemeral Docker containers) and shows you how they're going — replacing fragmented setups like ofelia, host crontabs, and scattered timers with one place to define, run, and observe jobs.

## Core Value

**One tool that both runs recurrent jobs and gives me total visibility into their state.** If everything else fails, the scheduler must reliably execute jobs on time AND let me see what happened (success/failure, logs, timing) through a web UI.

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

(None yet — ship to validate)

### Active

<!-- Current scope. Building toward these. These are hypotheses until shipped. -->

**Scheduler core**
- [ ] Rust backend parses 5-field cron expressions and schedules jobs reliably
- [ ] `@random` field support — any cron field can be randomized at startup, persisted until restart/re-randomize
- [ ] Configurable minimum gap between randomized jobs on the same day (e.g., 90 minutes)
- [ ] Idempotent startup — first run creates tables, subsequent runs are safe no-ops
- [ ] Graceful shutdown — wait for running jobs to complete (with configurable timeout)
- [ ] Config reload via SIGHUP or API endpoint without full restart

**Job execution**
- [ ] Run local shell commands
- [ ] Run inline scripts (shell/python/etc.)
- [ ] Spawn Docker containers via Docker API (bollard — not CLI)
- [ ] Support ALL Docker network modes: bridge, host, none, `container:<name>`, named networks
- [ ] Custom container naming per job
- [ ] Volume mounts, environment variables, auto-remove on completion
- [ ] Auto-pull images if not present locally
- [ ] Per-job execution timeout

**Configuration**
- [ ] TOML config file as source of truth (hand-written, no ofelia import)
- [ ] `[defaults]` section applies to all jobs unless overridden
- [ ] Per-job override of any default field
- [ ] Startup sync: create missing jobs, update changed jobs, disable removed jobs, preserve history for removed jobs
- [ ] Environment variable interpolation (`${ENV_VAR}`) — no plaintext secrets in config
- [ ] Config file mounted read-only

**Persistence**
- [ ] SQLite default (zero-config)
- [ ] PostgreSQL optional (for shared infrastructure)
- [ ] Auto-create tables on first run, built-in migrations
- [ ] Store: job definitions, run history (start/end/duration/exit_code/status), run logs (stdout/stderr)
- [ ] 90-day log retention (runs older than 90 days pruned — default, configurable)

**Web UI**
- [ ] Tailwind CSS, styled to the existing Cronduit design system (terminal-green, monospace)
- [ ] Embedded static assets served by the Rust backend (single binary)
- [ ] Dashboard: all jobs list, at-a-glance grid of recent runs, next run time, last run status badge
- [ ] Job detail page: full config, run history table, pass/fail per run, start/end/duration/exit_code
- [ ] Run detail page: stdout/stderr logs, metadata (image, container ID, network, exit code, duration)
- [ ] Settings/status page: scheduler uptime, DB status, config file path, next reload time
- [ ] Manual "Run Now" trigger per job
- [ ] Live/auto-refresh for running jobs
- [ ] Filter/search jobs by name; sort by name, last run, next run, status
- [ ] Server-rendered HTML with HTMX or minimal JS for live updates (no heavy framework)

**Operational**
- [ ] Structured JSON logging to stdout (Docker log collection friendly)
- [ ] `GET /health` endpoint
- [ ] `GET /metrics` Prometheus endpoint (jobs_total, runs_total, run_duration_seconds, failures_total)
- [ ] Optional basic auth / token auth for the web UI
- [ ] Single-container Docker deployment (Docker socket mounted, config mounted read-only)

### Out of Scope

<!-- Explicit boundaries. Reasoning preserved to prevent re-adding. -->

- **Email/webhook notifications** — deferred to v2; day-one flow is pull-based ("I check the dashboard"). Re-evaluate once the dashboard UX is proven.
- **Ofelia config import tool** — user is fine hand-writing a fresh `cronduit.toml` once. Lower-value than shipping the core.
- **Multi-node / distributed scheduling** — homelab use case is single-node. Adds complexity that would dilute the core.
- **User management / RBAC** — single-operator tool. Optional basic/token auth covers the access control need.
- **Workflow DAGs / job dependencies** — jobs are independent in v1. Cron semantics, not workflow engine semantics.
- **Job queuing / concurrency limits** — v2 feature. v1 assumes jobs don't interfere (user responsibility to schedule sensibly).
- **Unbounded log retention** — explicitly capped at 90 days to keep SQLite performant and UI responsive.

## Context

**Motivation:** User is currently using ofelia in a homelab setup but hits two walls: (1) ofelia has no UI, so there's no easy visibility into what ran, what failed, or what a run's output was; (2) ofelia's configuration model is inflexible — especially around Docker networking nuances and sharing defaults across jobs. Other alternatives (Cronicle, xyOps, docker-crontab) each fail on one or more of: Docker networking support (`--network container:X`), config-file-driven job definitions, bootstrap complexity, or UI quality.

**Real-world test jobs** (what "it works" means on day one):
- Periodic IP check against `ipinfo.io` through a VPN container (`--network container:vpn`)
- Weekly rclone backup
- Service health-check / restart on threshold
- Custom container runs for web scraping (arbitrary user image, with env vars and volumes)

**Existing assets in this repo:**
- `docs/SPEC.md` — detailed technical spec (strong draft, open to challenge)
- `design/DESIGN_SYSTEM.md` — complete visual identity: terminal-green palette, monospace typography, status color system, dark/light mode tokens
- `design/showcase.html` — visual reference
- `design/banners/`, `design/logos/`, `design/favicons/` — brand assets ready to use

**Open research questions** (to be investigated in research phase):
- How do similar tools handle randomized cron field scheduling? Is `@random` a known pattern or novel?
- bollard API stability and idiomatic patterns for container lifecycle (pull → create → start → wait → remove)
- SQLite vs Postgres performance for append-heavy log tables at homelab scale
- HTMX patterns for live-refreshing tables without flicker

## Constraints

- **Tech stack (backend)**: Rust — chosen for performance, reliability, and single-binary deployment. Suggested crates from spec: tokio, bollard, sqlx, axum, askama/maud, rust-embed, tracing, clap, serde+toml. Open to substitutions if research surfaces better options.
- **Tech stack (frontend)**: Server-rendered HTML + Tailwind + HTMX. No SPA frameworks. Static assets embedded in the binary via rust-embed.
- **Deployment**: Must run as a single Docker container. Docker socket access required (r/w). Config file mounted read-only. SQLite DB in a persistent volume.
- **Database**: SQLite must work zero-config. Postgres must be drop-in switchable via DATABASE_URL.
- **Security**: No plaintext secrets in config — only `${ENV_VAR}` references. Web UI auth is optional but must exist as a capability. Docker socket exposure is accepted as inherent to the problem (documented risk).
- **Storage**: Log retention capped at 90 days by default to prevent unbounded DB growth.
- **Design**: Web UI must match the existing Cronduit design system — terminal-green primary, monospace everywhere, dark/light mode support.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust backend (vs Go/Node) | Performance, single-binary deployment, strong Docker ecosystem via bollard. Spec already commits to Rust. | — Pending |
| TOML config format (vs YAML/JSON) | Spec proposes TOML; human-friendly, comments supported, widely used in Rust ecosystem (Cargo.toml). | — Pending |
| HTMX + server-rendered (vs SPA) | Simplifies deployment (single binary), removes build toolchain for JS, matches "terminal-native" design ethos. | — Pending |
| SQLite default, Postgres opt-in | Homelab default is zero-config; Postgres covers shared infra case. Same sqlx codepath. | — Pending |
| 90-day log retention | User wants "all history" but unbounded growth breaks SQLite perf and UI. 90 days is the practical compromise. | — Pending |
| No ofelia import tool | User happy to hand-write once. Saves scope without losing adoption. | — Pending |
| Push notifications deferred to v2 | Day-one flow is "I stare at the dashboard"; don't build notification infra until dashboard UX is validated. | — Pending |
| bollard (Docker API) vs Docker CLI | Native Rust, no CLI dependency, better error handling, supports `container:<name>` networking properly. | — Pending |

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
*Last updated: 2026-04-09 after initialization*
