# Phase 6: Live Events, Metrics, Retention & Release Engineering - Context

**Gathered:** 2026-04-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Turn the feature-complete binary into a shippable public OSS release. This phase adds: SSE log tail for in-progress runs, Prometheus `/metrics` endpoint, daily retention pruner, multi-arch Docker image with version tagging, complete `THREAT_MODEL.md`, and a README quickstart that takes a stranger from clone to running job in under 5 minutes.

Requirements covered: UI-14, OPS-02, OPS-04, OPS-05, DB-08.

</domain>

<decisions>
## Implementation Decisions

### SSE Log Streaming (UI-14)
- **D-01:** Slow SSE subscribers get a synthetic `[skipped N lines]` marker inserted into the stream when they fall behind. The DB writer is authoritative — the full log is always available on page reload.
- **D-02:** SSE connections auto-close when the run completes. Send a final `run_complete` event with exit status, then close the stream. Client-side HTMX swaps in the static paginated log view.
- **D-03:** Use a `tokio::sync::broadcast` channel per active run for fan-out. Multiple SSE subscribers (browser tabs) tap into the same broadcast. Natural fit with existing `LogSender`/`LogReceiver` pattern from Phase 2.
- **D-04:** Run Detail page transitions from SSE (live) to static (completed) via HTMX swap on the `run_complete` event — no full page reload. Seamless transition preserving context.

### Prometheus Metrics (OPS-02)
- **D-05:** Failure reason labels use the execution-agnostic closed enum from the spec: `image_pull_failed`, `network_target_unavailable`, `timeout`, `exit_nonzero`, `abandoned`, `unknown`. Cardinality fixed at 6. No job-type prefixing.
- **D-06:** Histogram buckets for `cronduit_run_duration_seconds` are homelab-tuned: `[1, 5, 15, 30, 60, 300, 900, 1800, 3600]` seconds. Covers quick health checks through long backup jobs.
- **D-07:** Per-run metrics (`cronduit_runs_total`, `cronduit_run_duration_seconds`, `cronduit_run_failures_total`) include a `job` label. Cardinality bounded by job count (typically 5-50 in a homelab). Enables per-job alerting without log parsing.
- **D-08:** `/metrics` endpoint is open and unauthenticated, consistent with v1's no-auth stance and standard Prometheus target conventions. Operators protect via network controls.
- **D-09:** Ship an `examples/prometheus.yml` file with a ready-to-use scrape config for Cronduit. Discoverable in the repo alongside the docker-compose example.

### Retention Pruner (DB-08)
- **D-10:** Uses the `[server].log_retention` config field already defined in Phase 1 (default `"90d"`, `humantime_serde` type). The pruner task itself is implemented in this phase.

### README & docker-compose (OPS-04, OPS-05)
- **D-11:** Quickstart docker-compose ships two example jobs: a command job that echoes a timestamp (instant feedback) and a Docker job pulling `alpine` to run `echo hello`. Covers both execution types, runs in seconds.
- **D-12:** Quickstart docker-compose uses `ports: 8080:8080` so a stranger can open `localhost:8080` immediately. The roadmap's `expose:` recommendation is noted in comments for production deployments.
- **D-13:** README structure: SECURITY section first (per Phase 1 decision), then 3-step quickstart (clone, `docker compose up`, open browser), then configuration reference, then metrics/monitoring guidance.

### Release Engineering
- **D-14:** SemVer tags on GitHub Release. Pushing a git tag like `v0.1.0` triggers CI to build and push `ghcr.io/*/cronduit:0.1.0` + `:0.1` + `:0` + `:latest`. Standard OCI multi-tag convention.
- **D-15:** First release is `v0.1.0` — signals "usable but expect breaking changes". Gives room to iterate on config format and API before committing to stability.
- **D-16:** Changelog auto-generated from conventional commits (e.g., `git-cliff` or GitHub's auto-generate feature). Less manual effort per release.

### Claude's Discretion
- Retention pruner scheduling strategy (fixed daily time vs interval, batch size, WAL checkpoint mechanics)
- SSE broadcast buffer sizing and backpressure tuning
- THREAT_MODEL.md structure and depth (four models specified: Docker socket, untrusted-client, config-tamper, malicious-image)
- Docker image OCI labels and metadata beyond version tags
- Changelog tooling choice (git-cliff vs GitHub auto-generate vs other)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Specifications
- `docs/SPEC.md` — Authoritative v1 spec; defines all behavior including SSE, metrics, retention
- `.planning/REQUIREMENTS.md` — Requirements UI-14, OPS-02, OPS-04, OPS-05, DB-08 with acceptance criteria

### Design & Security
- `design/DESIGN_SYSTEM.md` — Terminal-green brand; all UI additions (SSE log viewer) must match
- Phase 1 deferred `THREAT_MODEL.md` skeleton — Phase 6 completes it with Docker socket, untrusted-client, config-tamper, malicious-image models

### Prior Phase Context
- `.planning/phases/01-foundation-security-posture-persistence-base/01-CONTEXT.md` — D-17 (CI image tagging), D-30 (THREAT_MODEL deferred), D-31 (docker-compose deferred), D-32 (log_retention config field)
- `.planning/phases/02-scheduler-core-command-script-executor/02-CONTEXT.md` — Log pipeline bounded channel design (256-line head-drop)
- `.planning/phases/03-read-only-web-ui-health-endpoint/03-CONTEXT.md` — D-05/D-06 (Run Detail log viewer pagination), SSE explicitly deferred to Phase 6

### Research & Pitfalls
- `.planning/research/PITFALLS.md` — Pitfall 1 (Docker socket security), Pitfall 4 (log back-pressure), Pitfall 11 (retention under load), Pitfall 17 (metrics cardinality)
- `.planning/research/ARCHITECTURE.md` — ER diagram, component architecture

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/scheduler/log_pipeline.rs` — `LogSender`/`LogReceiver` with head-drop bounded channel, `drain_batch_async()`. Direct bridge point for SSE streaming via broadcast channel.
- `src/telemetry.rs` — Tracing subscriber initialization. Metrics instrumentation points follow the same structured pattern.
- Axum router patterns — `/api/*` for POST actions, `/partials/*` for HTMX fragments, tower-http middleware stack. `/metrics` and `/events/*` SSE routes plug into this.
- CSRF middleware already applied — `/metrics` should be excluded (read-only, Prometheus scraping).
- `CancellationToken` for graceful shutdown — retention pruner and SSE streams hook into this for clean teardown.

### Established Patterns
- Just-only CI: all Phase 6 recipes (release image, publish) must go through justfile
- Structured JSON logging via tracing: all new components (SSE handler, pruner, metrics) emit tracing events
- `SchedulerCmd` enum for web-to-scheduler signaling (Phase 3/5 pattern)
- Split migration directories (`migrations/sqlite/` + `migrations/postgres/`): any Phase 6 schema changes go to both

### Integration Points
- SSE route: `/events/runs/:id/logs` — new axum handler returning `Sse<impl Stream<Item = Event>>`
- Metrics route: `/metrics` — new axum handler returning Prometheus text format
- Pruner: background `tokio::spawn` task in scheduler loop, triggered daily
- Docker image CI: extend existing `just image` recipe for version tagging on git tag push
- README: extend existing structure with quickstart, config reference, monitoring sections

</code_context>

<specifics>
## Specific Ideas

- Quickstart must work in under 5 minutes for a stranger who has Docker installed — the two example jobs (echo + alpine hello-world) provide instant feedback
- `examples/prometheus.yml` shipped as a file (not just a README snippet) for easy copy-paste into existing Prometheus setups
- SSE `[skipped N lines]` marker ensures operators know when they've missed output, but the DB is always the authoritative source
- First release tagged `v0.1.0` to set expectations for early adopters

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 06-live-events-metrics-retention-release-engineering*
*Context gathered: 2026-04-12*
