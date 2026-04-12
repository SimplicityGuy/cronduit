# Phase 6: Live Events, Metrics, Retention & Release Engineering - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-12
**Phase:** 06-live-events-metrics-retention-release-engineering
**Areas discussed:** SSE log streaming, Metrics label design, README quickstart & docker-compose, Release & tagging strategy

---

## SSE Log Streaming

| Option | Description | Selected |
|--------|-------------|----------|
| Drop lines silently | Slow subscribers miss lines with no indication. Simplest — DB writer is authoritative. | |
| Drop + insert marker | Insert a synthetic '[skipped N lines]' event into the SSE stream. | ✓ |
| Backpressure (bounded buffer) | Buffer up to N events per subscriber before dropping. More complex, more memory. | |

**User's choice:** Drop + insert marker
**Notes:** Operator awareness of missed lines is important; full log always available on reload.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Auto-close on completion | Send 'run_complete' event with exit status, then close stream. | ✓ |
| Stay open indefinitely | Keep streaming after run completes. Client must detect and disconnect. | |

**User's choice:** Auto-close on completion
**Notes:** Clean lifecycle, no zombie connections.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Broadcast channel | One tokio::sync::broadcast per active run, multiple SSE subscribers tap in. | ✓ |
| Independent streams per subscriber | Each SSE connection gets its own log channel. | |

**User's choice:** Broadcast channel
**Notes:** Memory-efficient, consistent view across tabs, natural fit with LogSender pattern.

---

| Option | Description | Selected |
|--------|-------------|----------|
| HTMX swap on completion event | SSE sends 'run_complete'; hx-trigger swaps to static paginated view. | ✓ |
| Full page reload on completion | SSE sends 'run_complete'; JS does window.location.reload(). | |

**User's choice:** HTMX swap on completion event
**Notes:** Seamless transition, no jarring reload.

---

## Metrics Label Design

| Option | Description | Selected |
|--------|-------------|----------|
| Execution-agnostic reasons | Spec's set: image_pull_failed, network_target_unavailable, timeout, exit_nonzero, abandoned, unknown. Fixed cardinality of 6. | ✓ |
| Type-prefixed reasons | E.g. docker_image_pull_failed. Doubles cardinality, couples to executor types. | |
| Minimal set (3 reasons) | Just: timeout, failed, unknown. Loses diagnostic value. | |

**User's choice:** Execution-agnostic reasons
**Notes:** Universal across command/script/docker types, keeps cardinality fixed.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Homelab-tuned buckets | 1s, 5s, 15s, 30s, 60s, 300s, 900s, 1800s, 3600s | ✓ |
| Prometheus defaults | 0.005 through 10. Tuned for HTTP, not cron jobs. | |
| You decide | Claude picks defaults. | |

**User's choice:** Homelab-tuned buckets
**Notes:** Good granularity where most homelab jobs land (1s health checks to 1h backups).

---

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, job label on run metrics | cronduit_runs_total{job="backup-db",status="success"}. Bounded by job count. | ✓ |
| No job label, aggregate only | No job dimension. Can't alert on specific jobs. | |
| Optional via config flag | Default off, config enables it. Adds complexity. | |

**User's choice:** Yes, job label on run metrics
**Notes:** Enables per-job alerting, cardinality bounded by job count (5-50 typical).

---

| Option | Description | Selected |
|--------|-------------|----------|
| Open, no auth | Standard Prometheus pattern. Consistent with v1 no-auth stance. | ✓ |
| Separate bind address | /metrics on different port for independent firewalling. | |
| Behind CSRF | Breaks Prometheus scraping. | |

**User's choice:** Open, no auth
**Notes:** Standard convention for metrics endpoints.

---

| Option | Description | Selected |
|--------|-------------|----------|
| README snippet | Fenced YAML block in README. Low maintenance. | |
| Shipped prometheus.yml example | examples/prometheus.yml file in repo. | ✓ |
| Both | README snippet + example file. | |

**User's choice:** Shipped prometheus.yml example
**Notes:** Structured file for easy copy-paste into existing Prometheus setups.

---

## README Quickstart & docker-compose

| Option | Description | Selected |
|--------|-------------|----------|
| Simple echo + Docker hello-world | Two jobs: command echo + alpine hello. Covers both types, runs in seconds. | ✓ |
| Health-check style curl job | Docker job curling a public URL. Requires internet. | |
| You decide | Claude picks examples. | |

**User's choice:** Simple echo + Docker hello-world
**Notes:** Covers both execution types with instant feedback, no external dependencies.

---

| Option | Description | Selected |
|--------|-------------|----------|
| ports: for quickstart | Map 8080:8080 for immediate browser access. | ✓ |
| expose: only (per roadmap) | Requires reverse proxy. Breaks 5-minute promise. | |
| Both in separate files | docker-compose.yml (ports) + docker-compose.prod.yml (expose). | |

**User's choice:** ports: for quickstart
**Notes:** Production expose: recommendation noted in compose comments.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Security → Quickstart → Config → Metrics | SECURITY first, then 3-step quickstart, then reference sections. | ✓ |
| Quickstart first, security inline | Hooks reader first but buries security stance. | |
| You decide | Claude structures README. | |

**User's choice:** Security → Quickstart → Config → Metrics
**Notes:** Consistent with Phase 1 decision to lead with security section.

---

## Release & Tagging Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| SemVer tags on GitHub Release | Git tag v0.1.0 → CI pushes :0.1.0 + :0.1 + :0 + :latest. | ✓ |
| CalVer (date-based) | Tags like 2026.04.0. Less conventional for Rust CLIs. | |
| You decide | Claude picks versioning scheme. | |

**User's choice:** SemVer tags on GitHub Release
**Notes:** Standard OCI convention, manual release via GitHub Releases.

---

| Option | Description | Selected |
|--------|-------------|----------|
| v0.1.0 | Signals 'usable but expect breaking changes'. Room to iterate. | ✓ |
| v1.0.0 | Signals stability. High expectations from day one. | |

**User's choice:** v0.1.0
**Notes:** Standard for initial OSS releases.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Manual release notes | Hand-written in GitHub Releases. Higher quality. | |
| Auto-generated from conventional commits | git-cliff or GitHub auto-generate. Less effort. | ✓ |
| Both | Auto-generate draft, hand-edit before publish. | |

**User's choice:** Auto-generated from conventional commits
**Notes:** Less manual effort per release.

---

## Claude's Discretion

- Retention pruner scheduling (daily timing, batch size, WAL checkpoint mechanics)
- SSE broadcast buffer sizing and backpressure tuning
- THREAT_MODEL.md structure and depth
- Docker image OCI labels/metadata
- Changelog tooling choice (git-cliff vs GitHub auto-generate)

## Deferred Ideas

None — discussion stayed within phase scope.
