# Feature Research

**Domain:** Self-hosted Docker-native cron scheduler with web UI
**Researched:** 2026-04-09
**Confidence:** HIGH (primary sources: competitor docs, project spec, community threads)

## Executive Summary

The self-hosted cron-scheduler space in 2025 is crowded but fragmented. Every mature player (ofelia, Cronicle, dkron, Cronmaster, docker-crontab) solves a slightly different problem, and none cleanly nail the "single-binary, Docker-native, terminal-aesthetic, observable" quadrant Cronduit is targeting.

The table stakes are well-defined: cron parsing, run history, live log viewing, manual "Run Now", a readable dashboard, and for Docker-native tools, full network mode support. The real differentiation opportunity is `@random` + min-gap (nobody else has this cleanly), `--network container:<name>` support (ofelia's biggest wart), and a single statically linked binary (ofelia has it; Cronicle and dkron don't — both require a runtime or cluster).

Cronduit should NOT build: notifications, RBAC, DAGs, multi-node, queuing, SPA. The PROJECT.md out-of-scope list is correct and defensible. The real risk is scope creep on "one more nice-to-have feature" per dashboard page — this doc tries to hard-limit that.

## Feature Landscape

### Table Stakes (Users Expect These — Missing = Dead on Arrival)

These are features where absence causes users to close the tab and never come back. Every serious competitor has them.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Standard 5-field cron parsing | Universal. Anything less looks amateur. | LOW | Use `cron` or `saffron` crate. Already in spec. |
| `@hourly` / `@daily` / `@weekly` / `@monthly` shorthand | Every cron implementation has these | LOW | Most cron crates support out of the box. |
| Dashboard listing all jobs with status | This IS the product from the user's POV | MEDIUM | askama/maud template, not an SPA. |
| Last run status badge (success / fail / running) | Primary "is anything broken?" signal | LOW | Already in spec. |
| Next run time per job | Users need to know "when will this fire?" | LOW | Computed from cron expression. |
| Per-run stdout/stderr capture + viewer | Debugging dead jobs requires logs; this is the #1 reason to have a UI at all | MEDIUM | Store in SQLite. Stream from process via `tokio`. |
| Manual "Run Now" button | ofelia, Cronicle, GoCron, Cronmaster all have it. Users expect it for testing. | MEDIUM | Needs to bypass schedule but still record to history. Race with currently-running instance must be handled. |
| Per-run exit code + duration | "Did it work?" + "How long did it take?" are baseline observability | LOW | Captured by job executor. Already in spec. |
| Run history table per job | Required to see trends, catch flaky jobs | LOW | Already in spec. |
| Full Docker network mode support (including `container:<name>`) | Table stakes for the **Docker-native** category specifically; ofelia's missing support here is the #1 reason this product exists | MEDIUM | Requires `bollard` direct API, not CLI. Locked in spec. |
| Volume mounts, env vars, image pull | Any Docker job runner without these is unusable | MEDIUM | `bollard` handles all of this. |
| Auto-remove container after run (`--rm` equivalent) | Otherwise host fills with dead containers | LOW | `HostConfig.auto_remove` in bollard. |
| Per-job timeout / kill hung jobs | Jobs WILL hang; unbounded runs wreck the scheduler | MEDIUM | tokio `timeout` + container stop via bollard. |
| Persistent run history across restarts | Users restart containers constantly; losing history is a bug | LOW | SQLite handles this. Already in spec. |
| Auto-refresh dashboard for running jobs | Cronicle, Cronmaster, GoCron all do this; expected in 2025 | LOW | HTMX `hx-trigger="every 2s"` on running rows. |
| Graceful shutdown (wait for running jobs) | SIGTERM-and-die mid-job is considered a bug | MEDIUM | Trap SIGTERM, drain, bounded timeout. Already in spec. |
| Structured logs to stdout | Docker log collection is the norm; plaintext logs break pipelines | LOW | `tracing` with JSON formatter. Already in spec. |
| Health endpoint | Compose `healthcheck:` blocks require it; without it the container shows "unhealthy" | LOW | `GET /health`. Already in spec. |
| Config file as source of truth | GitOps homelab users will not use a tool whose state is ONLY in a database | LOW | TOML in spec; sync-on-startup in spec. |
| Single-binary / single-container deployment | Homelab users expect `docker compose up` and done | MEDIUM | `rust-embed` for assets + static link. Already in spec. |

**Score vs competitors on table stakes:**
- **ofelia** — Has most of these. Missing: `container:<name>` network mode (critical), limited run history persistence (in-memory by default), sparse run log retention.
- **Cronicle** — Has all of these but is heavy (Node.js runtime, ~300MB image), not Docker-native (containers are second-class).
- **dkron** — Has most. Distributed-first; overkill for a single homelab box. UI is thin.
- **docker-crontab** — Missing: web UI entirely, run history, log viewer. Not in the same category.
- **Cronmaster / GoCron** — Web-UI-focused, but weak on Docker-native job definition.

### Differentiators (The "Why Pick Cronduit" Features)

These are the features that justify a new entrant in a crowded space. Each one is either absent from every competitor or broken in the market leader.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **`@random` schedule field** | Nobody else does this cleanly. Lets users say "run sometime today" without thinking. Huge for batch jobs that don't need specific times but should be spread out. | MEDIUM | Resolve at startup, persist resolved value to DB, re-randomize on SIGHUP or scheduled re-randomize interval. Any of the 5 fields can be `@random`. |
| **`random_min_gap` between randomized jobs** | When you have 10 `@random` jobs on the same day, "spread them out by at least 90 min" is the natural ask. Impossible in every other scheduler. | MEDIUM | Solver runs at startup: backtracking or constraint-satisfaction over resolved times. Detect infeasibility (11 jobs in 24h with 3h gap) and log a warning. |
| **Full `--network container:<name>` support** | Direct fix for ofelia's #1 pain point. Enables VPN-gated job containers (huge in homelab land — ipleak checks, torrent downloaders, etc.) | LOW (given bollard) | Just pass `NetworkMode` through to bollard HostConfig. Free once you're not using the Docker CLI. |
| **Single static Rust binary + minimal container** | ofelia binary is ~15MB Go; Cronicle is a Node.js install tree. A <20MB scaling-to-zero container is a real selling point for resource-constrained homelab boxes. | MEDIUM | Already in the spec. musl target, `rust-embed` assets, ship `FROM scratch` or distroless. |
| **Terminal-green aesthetic** | Cronmaster looks like Bootstrap. ofelia's UI is a table. Cronicle is 2010s enterprise. A well-designed terminal aesthetic is memorable and shareable — real differentiation at launch. | MEDIUM | Already a locked constraint. Design system exists. Tailwind with custom token overrides. |
| **Config-driven with GitOps story** | ofelia supports config files but they're not the authoritative source — the `docker-labels` path is. Cronicle is UI-first (state in DB). Cronduit is file-first: commit your cron config to git and the UI is read-mostly observability. | MEDIUM | Sync logic in spec: create/update/disable on startup + SIGHUP reload. |
| **`bollard` direct API (no CLI shell-out)** | docker-crontab shells out; ofelia uses Docker API but has gaps. bollard is native Rust with full coverage. Better error surfaces, no path-dependency on `docker` binary. | LOW (library handles it) | Spec-locked. |
| **Prometheus `/metrics` endpoint out of the box** | Homelab users are increasingly on Grafana stacks. Cronicle has no metrics endpoint. ofelia has one but it's not discoverable. Baking this in + shipping an example Grafana dashboard is cheap differentiation. | LOW | `prometheus` crate or hand-rolled. Already in spec. |
| **Resolved config view per job** | "What config is ACTUALLY running after defaults are applied?" is a common debugging question. Show the fully-merged resolved job definition in the job detail page. | LOW | Cache resolved config in DB; render in job detail template. |
| **Live log streaming for running jobs** | Cronicle has this; ofelia doesn't (refresh-to-update). For long-running jobs (backups, scraping) this is the difference between "usable" and "I opened a terminal instead". | MEDIUM-HIGH | Server-Sent Events (SSE) from a subscriber on the log channel, or HTMX polling every 1s against `/jobs/:id/runs/:run_id/log?since=<offset>`. SSE is cleaner; polling is simpler. Recommend polling for v1. |

**Anchor diff: what nobody else has**
1. `@random` + `random_min_gap` — zero competitors
2. Full network mode coverage in a web-UI-equipped scheduler — only Cronduit
3. Terminal-native aesthetic + single binary + Docker-first — combinatorially unique

### Anti-Features (Explicitly NOT Building)

These are features that users will request but that would either bloat scope, break the design thesis, or duplicate adjacent tools better left alone.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Built-in email / webhook / Slack notifications** | "I want to know when a job fails" | Notification routing is a whole product (see Healthchecks.io, Alertmanager). Half-doing it ships a weak feature that users will still replace. | Emit Prometheus `failures_total` metric and structured error logs. Users hook Alertmanager or loki alerts. v2 can add webhook if demand is overwhelming. |
| **Web UI authentication (basic auth / token)** | "I want to expose this on the public internet" | Building auth means session management, CSRF, password hashing, audit logs — real surface area for a security boundary we don't need yet. | Document the reverse-proxy pattern (Caddy / Traefik / nginx basic auth in front). Explicit out-of-scope in PROJECT.md. Revisit v2. |
| **User management / RBAC** | "I want different people to see different jobs" | Multi-user = auth + data segregation + audit + invites. Cronduit is a single-operator tool by design. | Not now, not ever for v1/v2. Different product. |
| **Workflow DAGs / job dependencies ("run B after A")** | "I have a pipeline" | DAGs are a different product (Airflow, Dagster, Prefect). They imply retries, backfills, data passing, lineage. | Users chain via a shell script or a single job. Refuse to drift into workflow-engine land. |
| **Job queuing / concurrency limits / shared worker pool** | "I don't want 10 jobs running at once" | Implies a scheduler + queue + worker model. Current design is "each job is independent on its own schedule". Adding queues rewrites the core. | v2+: per-job "don't run if previous instance is still running" flag is a reasonable cheap version. Not the full queue. |
| **ofelia config importer** | "I'm migrating from ofelia" | Small user base, high translation-surface area, ongoing maintenance as ofelia evolves. | One-time hand-rewrite. Document with a mapping table in README. |
| **Web-UI job creation / editing / deletion** | "I want to add a job from the browser" | Config file is source of truth (Key Decision). Editing from UI fights that, creates drift, needs auth to be safe, and turns Cronduit into Cronicle. | UI is read-mostly observability + Run Now. All job CRUD happens via the config file. |
| **SPA frontend (React/Vue/Svelte)** | "Modern apps are SPAs" | Breaks single-binary story, adds JS build pipeline, fights the terminal aesthetic. | Server-rendered HTML + HTMX for live updates. Already locked in spec. |
| **Multi-tenancy / project / team** | "I run this for multiple clients" | Different product, different threat model, different database schema. | Recommend one Cronduit per tenant. |
| **Distributed / multi-node scheduling** | "I want HA" | Distribution is dkron's entire reason to exist. Don't fight that. | Users who need HA use dkron. Cronduit is single-node forever. |
| **Backfills / "run missed jobs"** | "The server was off for 2 hours; I want missed runs to fire" | Semantics explode fast (idempotency? batching? rate limits?). Users who need this really need a workflow engine. | Explicit: missed runs are missed. Log a warning on startup if a job's `last_scheduled < now - interval`. |
| **Plug-in / custom job type system** | "What if I want to run a Lambda?" | Extension points multiply the test matrix and break "single binary that just works". | Three job types: command, script, docker. Done. |
| **Built-in web terminal / container shell** | "I want to exec into the container from the UI" | Security boundary + WebSocket + auth. Not the product. | Users `docker exec` from their host. |
| **Secrets manager integration (Vault, Doppler, SOPS)** | "I want to pull secrets at runtime" | Each integration is its own effort; the `${ENV_VAR}` interpolation path already covers 90% of users (docker-compose provides env). | Env var interpolation (in spec) is the v1 answer. Document the sops-with-docker-compose pattern in README. |
| **Graph / chart of job duration trends** | "I want to see if backups are getting slower" | A chart library adds weight, the aesthetic fights the terminal vibe, and Grafana does this better. | Prometheus metrics are the story. Ship an example Grafana dashboard JSON file. |

## Feature Dependencies

```
┌─────────────────────┐
│ Cron parser         │◄─── Foundation for everything
└─────────┬───────────┘
          │
          ▼
┌─────────────────────┐
│ Scheduler loop      │
└─────────┬───────────┘
          │
          ├──────► Job executor (command, script, docker)
          │              │
          │              ├──► bollard integration
          │              │        │
          │              │        └──► Network mode support
          │              │                  │
          │              │                  └──► container:<name> (THE diff)
          │              │
          │              └──► Log capture (stdout/stderr streaming)
          │                         │
          │                         ▼
          │                 ┌─────────────────┐
          │                 │ Persistence     │ ◄── SQLite + migrations
          │                 └────────┬────────┘
          │                          │
          │                          ▼
          │                 ┌─────────────────┐
          │                 │ Web UI          │
          │                 └────────┬────────┘
          │                          │
          │                          ├──► Dashboard list
          │                          ├──► Job detail (needs resolved config)
          │                          ├──► Run detail (needs logs)
          │                          ├──► Run Now (needs scheduler + executor)
          │                          └──► Auto-refresh (needs HTMX + polling endpoint)
          │
          ├──────► @random resolver
          │              │
          │              └──► random_min_gap solver (depends on @random)
          │
          ├──────► Config parser (TOML)
          │              │
          │              └──► Sync-on-startup (create/update/disable/preserve)
          │                          │
          │                          └──► Config reload (SIGHUP + API)
          │
          └──────► Graceful shutdown (depends on in-flight run tracking)

┌─────────────────────┐
│ /metrics endpoint   │◄── Enhances observability (independent)
└─────────────────────┘

┌─────────────────────┐
│ /health endpoint    │◄── Required for compose healthcheck (independent)
└─────────────────────┘
```

### Dependency Notes

- **Run Now requires scheduler + executor + history writer:** Cannot ship "Run Now" without at least the full command-execution path wired through persistence. This means the dashboard's Run Now button is gated on the entire executor milestone.
- **Live log streaming requires log capture + persistence first:** Don't try to wire SSE/HTMX live logs before you can reliably capture+persist stdout/stderr. Polling on a committed log table is simpler than a shared in-memory channel.
- **`random_min_gap` requires `@random`:** Can't solve gap constraints without resolved randomized times. Ship `@random` first, gap solver second.
- **Sync-on-startup depends on config parser and persistence:** Three-way merge (config ⨯ DB ⨯ running) can't be built without both sides solid.
- **Web UI depends on persistence model being stable:** If the DB schema is churning, templates are churning too. Lock `job_runs` / `job_logs` columns before heavy UI work.
- **Metrics and health are independent:** Can be shipped at any point. Recommend shipping them early so ops tooling works during dev.
- **Graceful shutdown depends on job-run tracking:** Need to know what's in-flight to drain it. Should be shipped alongside the executor, not retrofitted later.
- **Conflict — in-UI job editing vs config-as-source-of-truth:** These are incompatible. Choosing config-as-truth (as the spec does) means the UI must never offer create/edit/delete. Any future Add-from-UI feature breaks the design thesis.

## MVP Definition

### Launch With (v1) — The minimum to call this a product

These are the items where absence means "this isn't ready yet".

- [ ] **Standard cron parsing + tokio-based scheduler loop** — nothing exists without this
- [ ] **Three job types: command, script, docker** — with bollard for docker and full network mode support
- [ ] **`container:<name>` network mode specifically** — this is the raison d'être
- [ ] **TOML config with `[defaults]` + `[[jobs]]` + `use_defaults = false`** — source of truth
- [ ] **Env var interpolation (`${ENV_VAR}`)** — no plaintext secrets, table stakes for self-hosting
- [ ] **SQLite persistence with auto-migrations** — zero-config default
- [ ] **Run history: start/end/duration/exit_code/status** — the core observability data
- [ ] **Stdout/stderr log capture + persistence** — debugging requires this
- [ ] **Dashboard: job list + status badges + next run + last run** — the headline page
- [ ] **Job detail: resolved config + run history table** — the drill-down page
- [ ] **Run detail: logs + metadata** — the deep-dive page
- [ ] **Manual "Run Now" button** — expected; low complexity once executor is solid
- [ ] **Auto-refresh running rows (HTMX polling, 2s interval)** — "is it done yet?"
- [ ] **Filter by name, sort by name/last-run/next-run/status** — 50+ jobs need this
- [ ] **Graceful shutdown with configurable drain timeout** — operational table stakes
- [ ] **`/health` + `/metrics` endpoints** — operational table stakes
- [ ] **Structured JSON logs to stdout** — Docker log collection
- [ ] **Sync-on-startup: create/update/disable, preserve history** — config-as-truth workflow
- [ ] **`@random` + `random_min_gap`** — the differentiator; ship at launch or it's a missed narrative
- [ ] **Per-job timeout** — unbounded jobs are a bug factory
- [ ] **Single binary + multi-arch Docker image + example compose file** — deployment story
- [ ] **Tailwind-themed UI matching `design/DESIGN_SYSTEM.md`** — terminal-green or bust
- [ ] **PostgreSQL as documented alternative** — optional, same schema, flipped by `DATABASE_URL`

### Add Shortly After Launch (v1.x) — Real user feedback driven

These are features that will probably come up within the first weeks of public use.

- [ ] **Live log tail via HTMX polling on running runs** — cheap upgrade from auto-refresh, high perceived quality
- [ ] **Log retention cleanup job** — default 90 days (already in spec); ship the cleanup worker
- [ ] **Per-job "skip if previous still running" flag** — cheap concurrency protection without a full queue
- [ ] **Config validation command (`cronduit check config.toml`)** — users will want CI validation
- [ ] **Example Grafana dashboard JSON** — `/metrics` is only useful if people can plug into it
- [ ] **README "migrating from ofelia" section with mapping table** — cheap marketing, no importer code
- [ ] **Timezone support in UI (local / server / UTC toggle)** — ofelia has this; users will ask
- [ ] **Copy-to-clipboard on cron expressions, run IDs, container IDs** — tiny quality-of-life wins

### Future Consideration (v2+) — Only if demand emerges

These are real user needs that could ship eventually but don't belong in v1.

- [ ] **Web UI authentication (basic auth or bearer token)** — deferred per PROJECT.md; revisit when first external user explicitly asks
- [ ] **Webhook / email notifications on failure** — currently covered by metrics; add if Prometheus path proves insufficient for non-Grafana users
- [ ] **Retry on failure with backoff** — interesting but semantically loaded; needs a clear mental model first
- [ ] **Per-job concurrency limit (N instances simultaneous)** — mini-queue; ships only if "skip if running" proves insufficient
- [ ] **Resource limits (cpu / memory) for Docker jobs** — bollard supports it trivially; add when asked
- [ ] **Backfill missed runs on startup** — controversial semantics; punt until someone argues for it
- [ ] **Export run history as JSON/CSV** — nice for auditing; low priority
- [ ] **Dark/light theme toggle** — design system supports both already; just wire the toggle
- [ ] **API-driven config modification** — only if config-as-file-of-truth proves too restrictive for real workflows

### Never (explicit non-goals)

- User management / RBAC / multi-tenancy
- Distributed / multi-node / HA
- Workflow DAGs / dependencies / backfills
- SPA frontend
- Plug-in / extension system for custom job types
- In-UI job CRUD

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|-----------|---------------------|----------|
| Cron parser + scheduler loop | HIGH | LOW | P1 |
| Command + script + docker executor | HIGH | MEDIUM | P1 |
| bollard + full network modes | HIGH | LOW (library) | P1 |
| `container:<name>` network mode | HIGH | LOW | P1 (diff) |
| TOML config + defaults + overrides | HIGH | LOW | P1 |
| Env var interpolation | HIGH | LOW | P1 |
| SQLite + migrations | HIGH | LOW | P1 |
| Run history persistence | HIGH | LOW | P1 |
| Log capture + persistence | HIGH | MEDIUM | P1 |
| Dashboard | HIGH | MEDIUM | P1 |
| Job detail page | HIGH | LOW | P1 |
| Run detail page | HIGH | LOW | P1 |
| Run Now button | HIGH | MEDIUM | P1 |
| HTMX auto-refresh | MEDIUM | LOW | P1 |
| Filter + sort | MEDIUM | LOW | P1 |
| Per-job timeout | HIGH | MEDIUM | P1 |
| Graceful shutdown | HIGH | MEDIUM | P1 |
| `/health` + `/metrics` | MEDIUM | LOW | P1 |
| Structured JSON logs | MEDIUM | LOW | P1 |
| Sync-on-startup | HIGH | MEDIUM | P1 |
| `@random` | HIGH | MEDIUM | P1 (diff) |
| `random_min_gap` | HIGH | MEDIUM | P1 (diff) |
| Terminal-green Tailwind UI | HIGH | MEDIUM | P1 |
| Single-binary Docker image | HIGH | MEDIUM | P1 |
| PostgreSQL support | MEDIUM | LOW (sqlx) | P1 |
| Config reload (SIGHUP) | MEDIUM | LOW | P1 |
| Example compose file + README | HIGH | LOW | P1 |
| Live log tail (polling) | MEDIUM | LOW | P2 |
| Log retention cleanup | MEDIUM | LOW | P2 |
| "Skip if running" flag | MEDIUM | LOW | P2 |
| Config validation CLI | MEDIUM | LOW | P2 |
| Grafana dashboard JSON | LOW | LOW | P2 |
| Timezone toggle | LOW | LOW | P2 |
| Web UI auth | MEDIUM | HIGH | P3 (v2) |
| Webhook notifications | MEDIUM | MEDIUM | P3 (v2) |
| Retry with backoff | MEDIUM | MEDIUM | P3 (v2) |
| Resource limits | LOW | LOW | P3 (v2) |

**Priority key:**
- **P1** — Must ship in v1 or product feels broken
- **P2** — Ship v1.x; low risk, low cost, clear value
- **P3** — v2+; needs more evidence or design work

## Competitor Feature Analysis

| Feature | ofelia | Cronicle | dkron | docker-crontab | Cronduit (target) |
|---------|--------|----------|-------|----------------|-------------------|
| Config-driven (file as source) | Partial (labels path is primary) | No (DB-first) | No (API-first) | Yes (crontab file) | **Yes (TOML, authoritative)** |
| Web UI | Yes (optional, enhanced in netresearch fork) | Yes (primary interface) | Yes (thin) | No | **Yes (terminal-themed)** |
| Dashboard with live updates | Partial (timezone selector, table view) | Yes (live log watcher, progress bars) | Yes (basic) | N/A | **Yes (HTMX polling)** |
| Manual "Run Now" | Yes | Yes | Yes | No | **Yes** |
| Run history persistence | In-memory by default; API exposes limited history | Yes (10k entries default, per-job log files) | Yes | No | **Yes (SQLite, configurable retention)** |
| Stdout/stderr log capture | Yes (via API) | Yes (live + stored per run) | Partial | No | **Yes (persisted per run)** |
| Live log tail / streaming | No (refresh to update) | **Yes (live watcher)** | No | No | **Yes (v1.x polling, future SSE)** |
| Full Docker network modes | **NO — `container:<name>` missing (critical gap)** | N/A (not Docker-native) | N/A | Yes (uses CLI) | **Yes (bollard direct)** |
| `--network container:<name>` | **BROKEN / missing** | N/A | N/A | Yes | **Yes (the headline diff)** |
| `@random` schedule | No | No | No | No | **Yes (unique)** |
| Min-gap constraint solver | No | No | No | No | **Yes (unique)** |
| Single binary | Yes (Go) | No (Node.js runtime) | Yes (Go) | Shell wrapper | **Yes (Rust, static)** |
| Image size | ~15MB | ~300MB | ~30MB | Small | **<30MB target** |
| SQLite + PostgreSQL | No (in-memory + JSON) | LMDB + optional SQL | BoltDB + etcd | None | **Yes (sqlx, both)** |
| Prometheus `/metrics` | Yes (partial) | No | Yes | No | **Yes** |
| Built-in notifications | Yes (Slack, StatsD middleware) | Yes (email, webhooks) | Yes | No | **No (out of scope)** |
| Web UI auth | Basic (optional) | Yes (users + RBAC) | Yes | N/A | **No (v1), reverse proxy** |
| Multi-node / HA | No | Yes (multi-worker) | **Yes (distributed is the point)** | No | **No (single node)** |
| Workflow DAGs | No | Yes (job chains) | No | No | **No** |
| Config reload without restart | Partial | Yes (DB-backed) | Yes (API) | No | **Yes (SIGHUP + API)** |
| Resolved config view | Partial (shows merged) | Yes | Yes | N/A | **Yes** |
| GitOps-friendly (commit config to git) | Partial | No | No | Yes | **Yes (first-class)** |
| Terminal / distinctive aesthetic | No (basic table) | No (2010s enterprise) | No (bootstrap) | N/A | **Yes (design system locked)** |

### Competitor verdicts

- **ofelia** — The closest analog. Biggest issues: broken `container:<name>` network mode, weak history persistence, generic UI. Cronduit's positioning is "ofelia done right for homelabs, with a real dashboard".
- **Cronicle** — Feature-rich but wrong shape: Node.js heavy, not Docker-native, UI-first (DB is source of truth). Appeals to a different user: multi-server ops teams, not homelab single-box owners.
- **dkron** — Distributed-first. Overkill for the Cronduit user. Cronduit should explicitly not compete here; recommend dkron for users who actually need HA.
- **docker-crontab** — Obsolete by design: CLI shell-out, no UI, no history. Cronduit targets its user base directly.
- **Healthchecks.io** — Adjacent, not competitive. Healthchecks monitors cron jobs run elsewhere; Cronduit runs cron jobs. The right story is "you can point Cronduit jobs at Healthchecks pings if you want external monitoring on top" — but don't build it in.
- **Uptime Kuma** — Related observability aesthetic (dashboard, status badges) but for uptime, not job execution. Aesthetic inspiration, not feature model.
- **Host crontab + systemd timers** — The incumbent for most users. Cronduit wins on observability and Docker support; loses on "it's already there".

## Feature Areas Requiring Design Attention in Later Phases

These aren't features to flag as "table stakes" but areas where the spec is thin and design work is needed before implementation.

1. **Log streaming approach** — Spec says "HTMX-style live updates" but doesn't specify polling vs SSE. Polling is recommended for v1 simplicity. Need a design decision before Web UI phase.
2. **Run Now race conditions** — What if a job is already running and the user clicks Run Now? Options: block, queue, spawn-parallel, spawn-with-warning. Not in spec. Default: spawn-with-warning ("a previous run is still active"), record as separate run. Needs explicit decision.
3. **`@random` re-randomization trigger** — Spec says "persist until restart or re-randomize". What causes re-randomize? Options: SIGHUP, API endpoint, scheduled (daily), manual button. Recommend: SIGHUP + API endpoint (consistent with config reload); not on UI for v1.
4. **Config sync semantics for renamed jobs** — If user renames a job in config, is it "delete old + create new" or "rename"? Cronicle treats rename as delete+create. Simpler but loses history. Needs explicit decision and documentation.
5. **Log retention enforcement timing** — Batched nightly job? On every write? Recommend: lazy cleanup on startup + hourly background task. Not in spec.
6. **Dashboard density for 100+ jobs** — Pagination? Virtual scroll? Filter is table stakes but pagination UX needs design. Flag for UI phase.
7. **"Running" state recovery after crash** — If Cronduit crashes mid-run, the DB has rows in "running" state that are stale. Startup should mark them as "orphaned" or "interrupted". Not in spec.

## Sources

- Project spec: `docs/SPEC.md` (authoritative)
- Project context: `.planning/PROJECT.md`
- [ofelia GitHub (mcuadros)](https://github.com/mcuadros/ofelia)
- [ofelia GitHub (netresearch enhanced fork)](https://github.com/netresearch/ofelia)
- [ofelia jobs documentation](https://github.com/mcuadros/ofelia/blob/master/docs/jobs.md)
- [Cronicle GitHub](https://github.com/jhuckaby/Cronicle)
- [Cronicle WebUI docs](https://github.com/jhuckaby/Cronicle/blob/master/docs/WebUI.md)
- [Cronicle homepage](https://cronicle.net/)
- [dkron homepage](https://dkron.io/)
- [Healthchecks.io docs](https://healthchecks.io/docs/)
- [Cronmaster (noted.lol)](https://noted.lol/cronmaster/)
- [GoCron (noted.lol)](https://noted.lol/go-cron/)
- [Cron alternatives comparison (cronradar)](https://cronradar.com/comparisons/cron-alternatives)
- [Self-hosted homelab stack 2026 (elest.io)](https://blog.elest.io/the-2026-homelab-stack-what-self-hosters-are-actually-running-this-year/)
- [Awesome Self-Hosted list](https://github.com/awesome-selfhosted/awesome-selfhosted)

---
*Feature research for: self-hosted Docker-native cron scheduler (Cronduit)*
*Researched: 2026-04-09*
