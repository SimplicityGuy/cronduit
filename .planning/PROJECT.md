# Cronduit

## Current State

**Shipped:** v1.0.0 (2026-04-14), patched to v1.0.1 (2026-04-14) — single-binary Rust cron scheduler with terminal-green HTMX web UI, full Docker-API job execution including `--network container:<name>`, `@random` schedule resolver, hot config reload, Prometheus metrics, SSE log tail, multi-arch (amd64+arm64) GHCR release, and a documented threat model. 86/86 v1 requirements complete; audit verdict `passed`. v1.0.1 follow-up (PR #22) closed four post-ship gaps: GHCR OCI manifest annotations, `cmd`-on-non-docker validator, `delete = false` honored at cleanup, Debian 13 (trixie) builder, MIT license metadata. See [`MILESTONES.md`](MILESTONES.md) for the full v1.0 summary.

**Next milestone:** v1.1 — Operator Quality of Life (in progress; 3 of 5 phases complete).

**v1.1 rc.1 cut pending (as of 2026-04-18):** Phases 10 (Stop-a-Running-Job + Hygiene), 11 (Per-Job Run Numbers + Log UX), and 12 (Docker Healthcheck) are implementation-complete on branch `gsd/phase-12-context`. Phase 12 landed OPS-06/07/08 (new `cronduit health` CLI, Dockerfile HEALTHCHECK, compose-smoke CI workflow, release.yml rc-tag gating, and the maintainer rc-cut runbook). The `v1.1.0-rc.1` tag cut is a maintainer action (signed key, per Phase 12 D-13) queued for after the Phase 12 PR merges to `main`.

## What This Is

Cronduit is a self-hosted cron job scheduler with a web UI, built for Docker-native homelab environments. It's a single Rust binary that runs recurrent tasks — local commands, inline scripts, or ephemeral Docker containers — and gives operators a terminal-green web dashboard to see exactly what ran, when, and how it went.

## Core Value

**One tool that both runs recurrent jobs reliably AND makes their state observable through a web UI.** If everything else is cut, the scheduler must (1) execute jobs on time with full Docker networking support (especially `--network container:<name>` for VPN setups) and (2) let the operator see pass/fail, logs, and timing from a browser.

## Current Milestone: v1.1 — Operator Quality of Life

**Goal:** Make v1.0's feature surface genuinely pleasant to live with day-to-day, shipped iteratively via `v1.1.0-rc.N` releases as each chunk lands.

**Theme:** Longer-lived, bug-fix-and-polish milestone. No net-new capability surface area — every item either fixes a v1.0 behavior the operator has already hit in practice or makes existing information easier to see and act on. Feature additions (webhooks, queuing) are explicitly deferred to v1.2.

**Target features:**

*Bug fixes (from v1.0.1 post-ship operator experience)*
- Stop a running job — new `stopped` status, single hard kill, works for command/script/docker
- Log refresh: lines rendering out of order after job completes
- Log refresh: transient "error getting logs" that resolves on manual refresh
- Log backfill on navigation — returning to a running job page should show prior lines from DB then attach to live SSE
- Per-job run numbers — schema column + idempotent backfill migration on startup

*Observability polish (highest-leverage subset)*
- Run timeline view (gantt-style, last 24h / 7d, color-coded by status)
- Per-job success-rate badge / sparkline on dashboard cards
- Per-job duration trend (p50/p95) on the job detail page

*Ergonomics*
- Bulk enable/disable from dashboard with checkbox multi-select (design question — where does disabled state live given the read-only config file — resolved at phase-plan time)

**Release strategy:** Iterative `v1.1.0-rc.1`, `v1.1.0-rc.2`, ... cut at chunky checkpoints (after bug-fix block, after observability block, after ergonomics). `:latest` GHCR tag stays at `v1.0.1` until final `v1.1.0`. Each rc gets `:v1.1.0-rc.N` plus a rolling `:rc` tag. Tag format uses semver pre-release notation (dot before `rc.N`).

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

### Active

<!-- Current scope. Building toward these. Hypotheses until shipped. -->

**v1.1 — Operator Quality of Life** (see `REQUIREMENTS.md` for the full testable list with REQ-IDs once generated)

*Scheduler / execution*
- [ ] Operator can stop a running job from the UI; run terminates with a new `stopped` status (distinct from `cancelled`/`failed`/`timeout`) — force kill, single hard stop
- [ ] Run records have a per-job sequential number (`job_run_number`) alongside the existing global ID, backfilled on startup for existing rows via an idempotent migration

*Deployment / packaging*
- [ ] `docker compose up` with the shipped quickstart compose file reports the cronduit container as `healthy`, not `unhealthy`. Root cause hypothesis: busybox `wget --spider` misparses chunked-encoded responses from axum's `/health` handler even though the endpoint returns 200 + valid JSON. Fix path (TBD at phase-plan time): ship a `cronduit health` CLI subcommand that self-checks the local `/health` endpoint, OR embed a `HEALTHCHECK` directive in the Dockerfile, OR change the wget invocation pattern — whichever gives the smallest image + most reliable signal.

*Log tail / UX*
- [ ] Navigating back to a running job's page shows prior log lines from DB, then attaches to the live SSE stream without a gap
- [ ] Log lines remain in chronological order across the live→static transition when a run completes
- [ ] Transient "error getting logs" race on page load is eliminated (no manual refresh required)

*Observability polish*
- [ ] Dashboard has a run timeline view (gantt-style, last 24h / 7d, color-coded by status)
- [ ] Each dashboard job card shows a success-rate badge and a short sparkline (rolling window)
- [ ] Job detail page shows duration trend (p50/p95) over the last N runs

*Ergonomics*
- [ ] Operator can multi-select jobs from the dashboard and bulk enable/disable them (design: where "disabled" state lives given read-only config file — resolved at phase-plan time)

### Future Requirements

<!-- Scoped but not in the current milestone. Target versions are intent, not contracts — they may shift when that milestone is actually kicked off. -->

**v1.2 — Feature expansion (tentative)**
- Webhooks on job state transitions (failure/success, per-job URL config, secret-aware)
- Job concurrency limits and queuing (deep scheduler-core change; affects the `tokio::select!` loop + persistence + fairness)
- Failure clustering / "what changed" context on run detail (first-failure timestamp, config-last-modified, image-pulled-at)
- Per-job exit-code histogram on job detail page
- Cross-run log search across retention window
- Job tagging / grouping (`tags = ["backup", "weekly"]` in job config; dashboard filter chips)

**v1.3 — Operational ergonomics deepening (tentative)**
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

**Codebase state at v1.0.0.**
- ~14,000 lines of Rust (~10k src, ~4k tests)
- Edition 2024, rust-version 1.94.1
- Tech stack: tokio, axum 0.8, askama_web 0.15, sqlx 0.8, bollard 0.20, croner 3.0, Tailwind (standalone binary), HTMX 2.0.4 (vendored)
- 49 plans executed across 9 phases between 2026-04-08 and 2026-04-14 (6 calendar days)
- CI matrix: `linux/{amd64,arm64} × {SQLite, Postgres}` + compose-smoke quickstart regression
- Release artifacts: multi-arch image at `ghcr.io/SimplicityGuy/cronduit:v1.0.0` + linux/{amd64,arm64} binaries

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
*Last updated: 2026-04-18 — Phases 10, 11, 12 implementation-complete on `gsd/phase-12-context`. Phase 12 delivered OPS-06/07/08 (cronduit health subcommand, Dockerfile HEALTHCHECK, compose-smoke CI). `v1.1.0-rc.1` tag cut queued for after PR merge (maintainer action). Next: Phase 13 (observability polish + rc.2). Previous update: 2026-04-14 — v1.1 milestone kicked off.*
