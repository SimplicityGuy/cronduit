# Cronduit

## What This Is

Cronduit is a self-hosted cron job scheduler with a web UI, built for Docker-native homelab environments. It's a single Rust binary that runs recurrent tasks — local commands, inline scripts, or ephemeral Docker containers — and gives operators a terminal-green web dashboard to see exactly what ran, when, and how it went.

## Core Value

**One tool that both runs recurrent jobs reliably AND makes their state observable through a web UI.** If everything else is cut, the scheduler must (1) execute jobs on time with full Docker networking support (especially `--network container:<name>` for VPN setups) and (2) let the operator see pass/fail, logs, and timing from a browser.

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

(None yet — ship to validate)

### Active

<!-- Current scope. Building toward these. Hypotheses until shipped. -->

**Scheduler core (Rust)**
- [ ] Parse standard 5-field cron expressions and schedule jobs reliably with async runtime (tokio)
- [ ] `@random` field support — any cron field can be randomized at startup, persisted until restart or re-randomize
- [ ] Configurable minimum gap between randomized jobs on the same day (e.g., `random_min_gap = "90m"`)
- [ ] Idempotent startup — first run creates tables, subsequent runs are safe no-ops
- [ ] Graceful shutdown — wait for running jobs with configurable timeout
- [ ] Config reload via SIGHUP or API endpoint without full restart

**Job execution**
- [ ] Run local shell commands (`type = "command"`)
- [ ] Run inline scripts, shell or python-style (`type = "script"`)
- [ ] Spawn Docker containers via the Docker API using `bollard` (NOT the docker CLI)
- [ ] Support all Docker network modes: `bridge`, `host`, `none`, `container:<name>`, named networks
- [ ] Custom per-job container name
- [ ] Volume mounts, environment variables, auto-remove (`--rm`) on completion
- [ ] Auto-pull image if not present locally
- [ ] Per-job execution timeout

**Configuration**
- [ ] File-based config is the source of truth (hand-written, no ofelia importer)
- [ ] `[defaults]` section applies to all jobs unless overridden per-job
- [ ] Per-job override of any default field (including `use_defaults = false` to ignore defaults entirely)
- [ ] Startup sync: create missing jobs, update changed jobs, disable removed jobs, preserve history for removed jobs
- [ ] Environment variable interpolation (`${ENV_VAR}`) — no plaintext secrets in config
- [ ] Config file mounted read-only
- [ ] Primary format: TOML (see Key Decisions — research will validate vs alternatives)

**Persistence**
- [ ] SQLite default, zero-config (via `sqlx`)
- [ ] PostgreSQL optional for shared infrastructure
- [ ] Auto-create tables on first run, built-in migrations
- [ ] Store: job definitions, run history (start/end/duration/exit_code/status), run logs (stdout/stderr)
- [ ] Configurable log retention (default 90 days)

**Web UI**
- [ ] Tailwind CSS styled to the Cronduit design system (terminal-green, monospace — see `design/DESIGN_SYSTEM.md`)
- [ ] Embedded static assets served by the Rust backend (`rust-embed`) — single binary
- [ ] Server-rendered HTML (askama or maud) with HTMX-style live updates; no SPA framework
- [ ] Dashboard: list of all jobs, recent-run grid, next run time, last-run status badge
- [ ] Job detail: full resolved config, run history table, per-run start/end/duration/exit_code/status
- [ ] Run detail: stdout/stderr logs, metadata (image, container ID, network, exit code, duration)
- [ ] Settings/status page: scheduler uptime, DB connection, config file path, next reload time
- [ ] Manual "Run Now" button per job
- [ ] Auto-refresh / live updates for running jobs
- [ ] Filter/search jobs by name; sort by name, last run, next run, status

**Operational**
- [ ] Structured JSON logs to stdout (`tracing`) for Docker log collection
- [ ] `GET /health` returning scheduler status
- [ ] `GET /metrics` exposing Prometheus-compatible metrics (`jobs_total`, `runs_total`, `run_duration_seconds`, `failures_total`)

**Packaging & deployment**
- [ ] Ships as a Docker image (multi-arch preferred) and a single static-ish binary
- [ ] Ships with an example `docker-compose.yml` that mounts the Docker socket, the config file, and a data volume

**Quality & release engineering**
- [ ] Unit and integration tests (job execution, Docker spawning, config parsing, sync behavior, scheduler correctness)
- [ ] GitHub Actions CI: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, container build
- [ ] README documentation sufficient for a new user to self-host from scratch

### Out of Scope

<!-- Explicit boundaries. v2+ or never. -->

- **Web UI authentication (basic auth / token)** — deferred to v2. v1 assumes a trusted LAN deployment; if the operator needs auth, they front it with their existing reverse proxy. Revisit when first external users ask for it.
- **Multi-node / distributed scheduling** — single-node only. Distribution is a different product.
- **User management / RBAC** — single-operator tool; no user accounts in v1 or v2.
- **Workflow DAGs / job dependencies** — no "run B after A succeeds". Jobs are independent.
- **Email / webhook notifications** — post-v1 add-on; can layer on top of the metrics/log outputs.
- **Job queuing / concurrency limits** — post-v1. Each job runs on its own schedule without a shared queue.
- **Importer for existing ofelia configs** — users rewrite their schedules in Cronduit's TOML by hand. Not worth the translation surface area.
- **SPA / React frontend** — server-rendered HTML only. Keeps the single-binary story and matches the terminal aesthetic.

## Context

**Who this is for.** First user is Robert's own homelab. Long-term it's a public OSS release (repo lives at `public/cronduit`) — the tool must be shippable to outside adopters from day one, with docs and quality bar to match.

**Why it exists.** Existing schedulers don't cover the homelab Docker-native use case well:

- `ofelia` — no `--network container:<name>` support (critical for VPN-bound jobs).
- `docker-crontab` — shells out to the `docker` CLI, lacks a web UI, no persistent run history.
- `Cronicle`, `xyOps` — heavier, not designed around Docker-native job definitions.
- Host crontabs + scattered systemd timers — no unified observability.

Cronduit collapses those into one tool: define jobs in a config file, get a dashboard, reliably run containers on any Docker network mode.

**Existing artifacts already in the repo.**

- `docs/SPEC.md` — detailed technical spec that defines v1 (treated as authoritative for this milestone).
- `design/DESIGN_SYSTEM.md` — full visual identity (terminal-green palette, monospace typography, status colors, dark/light tokens).
- `design/banners|logos|favicons|showcase.html` — brand assets ready to embed in the web UI.
- `LICENSE`, `README.md`, `.gitignore` (already scoped for Rust/Docker/macOS) — boilerplate in place.
- No Rust source yet; `Cargo.toml` does not exist. Greenfield from a code standpoint.

## Constraints

- **Tech stack (locked)**: Rust backend using `bollard` for the Docker API. No CLI shelling out. No alternative languages for v1.
- **Persistence (locked)**: `sqlx` with SQLite default and PostgreSQL optional. Same schema targets both.
- **Frontend (locked)**: Tailwind CSS + server-rendered HTML templating (askama or maud) with HTMX-style live updates. No React/Vue/Svelte.
- **Config format (soft-locked)**: TOML is the working assumption; research phase will explicitly validate TOML vs. YAML (and note INI/JSON) before freezing the decision.
- **Deployment shape**: Single binary + Docker image. Cronduit itself runs inside Docker, mounting the host Docker socket.
- **Security posture**: No plaintext secrets in the config file; interpolate from env. Config mounted read-only. Web UI ships unauthenticated in v1 (see Out of Scope).
- **Quality bar**: Tests + GitHub Actions CI from phase 1. Clippy + fmt gate on CI. README sufficient for a stranger to self-host.
- **Design fidelity**: Web UI must match `design/DESIGN_SYSTEM.md` (Cronduit terminal-green brand), not ship in default Tailwind look.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust for the backend | Performance, reliability, single binary, strong async story via tokio, good Docker client (`bollard`) | — Pending |
| `bollard` Docker client (not CLI) | Direct API access = all network modes work (`container:<name>`), no shell-out fragility, better error surface | — Pending |
| SQLite default, PostgreSQL optional | Zero-config for homelabs, upgrade path for shared deployments | — Pending |
| Tailwind + server-rendered HTML (no SPA) | Matches single-binary goal, fits terminal aesthetic, no JS build complexity | — Pending |
| Web UI auth deferred to v2 | v1 assumes trusted LAN; unblocks shipping; users can front with reverse proxy | — Pending |
| Tests + CI required in phase 1 | Public OSS release — external adopters need a quality signal from day one | — Pending |
| TOML as primary config (to be validated) | Rust ecosystem maturity, cleaner than YAML for hand-written config; research phase will confirm | ⚠️ Revisit after research |
| No ofelia import path | Keeps scope small; operators rewrite schedules once | — Pending |

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
