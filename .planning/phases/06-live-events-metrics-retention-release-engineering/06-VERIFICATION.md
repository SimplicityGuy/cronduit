---
phase: 06-live-events-metrics-retention-release-engineering
verified: 2026-04-12T22:30:00Z
status: human_needed
score: 4/5 must-haves verified
overrides_applied: 0
gaps:
  - truth: "A stranger can clone the repo, run docker compose up with the example config, and see a running job in under 5 minutes; the example docker-compose.yml uses expose: (not ports:) for the web UI"
    status: failed
    reason: "ROADMAP SC 4 requires expose: (not ports:), but examples/docker-compose.yml uses ports: 8080:8080. Plan 04 explicitly chose ports: (D-12) for quickstart accessibility, with a comment in the file recommending expose: for production. This is an intentional deviation from the ROADMAP SC wording."
    artifacts:
      - path: "examples/docker-compose.yml"
        issue: "Uses ports: 8080:8080 instead of expose: as required by ROADMAP success criterion 4"
    missing:
      - "Either change docker-compose.yml to use expose: OR add an override to accept the intentional ports: choice"
human_verification:
  - test: "Run docker compose -f examples/docker-compose.yml up -d from the repo root (requires docker-compose.yml and cronduit.toml to be in examples/), then open http://localhost:8080 and verify the web UI loads, jobs are visible, and echo-timestamp runs on its schedule (every minute)"
    expected: "Web UI shows jobs dashboard with echo-timestamp and alpine-hello listed; echo-timestamp fires within 1 minute and shows a run in progress or completed"
    why_human: "Requires a built Docker image (ghcr.io/simplicityguy/cronduit:latest) to exist and a Docker daemon running; cannot test image pull/run programmatically in verification context"
  - test: "Start an in-progress run and open its Run Detail page, verify log lines stream in real time via SSE (LIVE badge visible, lines appear without page reload)"
    expected: "LIVE badge shown, log lines appear in the viewer as they are produced, when run completes the view transitions to static paginated logs without a full page reload"
    why_human: "Requires a running Cronduit instance with an active job; SSE streaming behavior is a live/real-time interaction that cannot be verified statically"
---

# Phase 6: Live Events, Metrics, Retention & Release Engineering Verification Report

**Phase Goal:** Turn the feature-complete binary into a shippable public OSS release: SSE log tail for in-progress runs, Prometheus /metrics with a bounded-cardinality label set, daily retention pruner, multi-arch Docker image, complete THREAT_MODEL.md, and a README quickstart that takes a stranger from `git clone` to a working scheduled job in under 5 minutes.

**Verified:** 2026-04-12T22:30:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SSE streaming for in-progress runs; static viewer for completed runs; slow subscribers drop lines without blocking DB writer | VERIFIED | `src/web/handlers/sse.rs` implements full SSE handler with broadcast recv loop, RecvError::Lagged produces skip marker, RecvError::Closed sends run_complete event. `src/scheduler/run.rs` inserts broadcast_tx at run start (line 100), publishes in log_writer_task (line 324), removes after finalize (lines 265-266). `templates/pages/run_detail.html` conditionally renders SSE vs static based on `is_running` flag. No `window.location.reload` found. |
| 2 | GET /metrics exposes Prometheus text format with 4 metric families and closed-enum failure reason labels | VERIFIED | `src/web/handlers/metrics.rs` renders via `state.metrics_handle.render()` with correct Content-Type header. `src/telemetry.rs` has `setup_metrics()` with homelab-tuned buckets `[1.0, 5.0, 15.0, 30.0, 60.0, 300.0, 900.0, 1800.0, 3600.0]`. `src/scheduler/run.rs` has `counter!("cronduit_runs_total"`, `histogram!("cronduit_run_duration_seconds"`, `counter!("cronduit_run_failures_total"`. `src/scheduler/sync.rs` has `gauge!("cronduit_jobs_total")`. FailureReason enum with 6 variants (ImagePullFailed, NetworkTargetUnavailable, Timeout, ExitNonzero, Abandoned, Unknown). |
| 3 | Daily retention pruner deletes data older than log_retention in batched transactions with WAL checkpoint | VERIFIED | `src/scheduler/retention.rs` exists with BATCH_SIZE=1000, BATCH_SLEEP=100ms, WAL_CHECKPOINT_THRESHOLD=10000. FK-safe ordering (logs before runs). CancellationToken checked between batches. `src/db/queries.rs` has `delete_old_logs_batch`, `delete_old_runs_batch`, `wal_checkpoint` with PRAGMA wal_checkpoint(TRUNCATE). Pruner spawned in `src/cli/run.rs` (line 188). |
| 4 | docker-compose.yml uses expose: (not ports:) for the web UI | FAILED | `examples/docker-compose.yml` uses `ports: "8080:8080"`. ROADMAP SC 4 explicitly requires `expose:`. Plan 04 intentionally chose `ports:` (D-12) for quickstart accessibility, with a comment in the file recommending `expose:` for production. |
| 5 | THREAT_MODEL.md complete; README security above fold; multi-arch Docker builds via cargo-zigbuild and publishes on every push to main | VERIFIED | `THREAT_MODEL.md` has all four models (Docker Socket at line 43, Untrusted Client at line 85, Config Tamper at line 121, Malicious Image at line 153), multiple mermaid diagrams. `README.md` has `## Security` as first H2 (line 19). `.github/workflows/ci.yml` builds and pushes to GHCR on main push via `just image-push`. Dockerfile uses cargo-zigbuild (line 27). `.github/workflows/release.yml` also handles tag releases. |

**Score:** 4/5 truths verified (Truth 4 failed due to ports: vs expose: deviation)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/web/handlers/sse.rs` | SSE log streaming handler | VERIFIED | 150 lines, exports `sse_logs`, contains `Lagged`, `skipped`, `run_complete`, `html_escape`, `format_log_line_html` |
| `templates/pages/run_detail.html` | Run Detail page with SSE/static conditional rendering | VERIFIED | Contains `sse-connect`, `cd-badge--running`, `LIVE`, `Waiting for output...`, `run_complete`, `htmx.ajax` targeting `/partials/runs/` |
| `tests/sse_streaming.rs` | SSE integration test stubs | VERIFIED | 4 test stubs: `sse_active_run_streams_log_lines`, `sse_completed_run_returns_immediate_close`, `sse_slow_subscriber_gets_skip_marker`, `sse_stream_closes_on_run_finalize` — intentional `todo!()` stubs |
| `src/web/handlers/metrics.rs` | Prometheus /metrics HTTP handler | VERIFIED | Exports `metrics_handler`, uses `state.metrics_handle.render()`, correct Content-Type |
| `src/telemetry.rs` | Metrics recorder setup with PrometheusHandle | VERIFIED | Contains `PrometheusBuilder`, `setup_metrics`, homelab histogram buckets |
| `examples/prometheus.yml` | Ready-to-use Prometheus scrape config | VERIFIED | Contains `cronduit` job_name, scrape_interval 15s |
| `tests/metrics_endpoint.rs` | Metrics endpoint integration test stubs | VERIFIED | 4 test stubs — intentional `todo!()` stubs |
| `src/scheduler/retention.rs` | Retention pruner background task | VERIFIED | Exports `retention_pruner`, BATCH_SIZE=1000, BATCH_SLEEP=100ms, WAL_CHECKPOINT_THRESHOLD=10000, CancellationToken |
| `src/db/queries.rs` | Batched delete queries for retention | VERIFIED | Contains `delete_old_logs_batch`, `delete_old_runs_batch`, `wal_checkpoint` |
| `tests/retention_integration.rs` | Retention pruner integration test stubs | VERIFIED | 5 test stubs — intentional `todo!()` stubs |
| `examples/docker-compose.yml` | Quickstart docker-compose configuration | PARTIAL | Contains `docker.sock`, `8080:8080`, `cronduit.toml:...ro`, `cronduit-data` volume — but uses `ports:` not `expose:` |
| `README.md` | Complete README with security section and quickstart | VERIFIED | `## Security` is first H2 (line 19), contains `## Quickstart`, `## Configuration`, `## Monitoring` |
| `THREAT_MODEL.md` | Complete threat model document | VERIFIED | Contains Docker Socket, Untrusted Client, Config Tamper, Malicious Image models; multiple mermaid diagrams |
| `.github/workflows/release.yml` | Release CI workflow triggered by v* tags | VERIFIED | Triggers on `push: tags: ['v*']`, uses `docker/build-push-action@v6`, 4 image tags, `orhun/git-cliff-action@v4`, `softprops/action-gh-release@v2` |
| `cliff.toml` | git-cliff configuration for changelog generation | VERIFIED | Contains `conventional_commits = true`, commit_parsers for feat, fix, refactor, docs, etc. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/scheduler/run.rs` | `AppState.active_runs` | broadcast::Sender stored in HashMap on run start, removed on finalize | WIRED | `active_runs.write().await.insert(run_id, broadcast_tx.clone())` at line 100; `active_runs.write().await.remove(&run_id)` at line 265 |
| `src/web/handlers/sse.rs` | `AppState.active_runs` | subscribe to broadcast channel by run_id | WIRED | `active.get(&run_id).map(|tx| tx.subscribe())` at line 36 |
| `src/scheduler/run.rs` | metrics crate macros | counter!/histogram!/gauge! calls at run finalization | WIRED | Lines 257-261: `counter!("cronduit_runs_total"`, `histogram!("cronduit_run_duration_seconds"`, `counter!("cronduit_run_failures_total"` |
| `src/web/handlers/metrics.rs` | PrometheusHandle | handle.render() returns text format | WIRED | `state.metrics_handle.render()` at line 13 |
| `src/scheduler/retention.rs` | `src/db/queries.rs` | batched delete queries called in loop | WIRED | `queries::delete_old_logs_batch` and `queries::delete_old_runs_batch` called in run_prune_cycle |
| `src/scheduler/mod.rs` | `src/scheduler/retention.rs` | tokio::spawn retention_pruner at scheduler start | WIRED | `pub mod retention` in mod.rs; `tokio::spawn(crate::scheduler::retention::retention_pruner(...))` in cli/run.rs line 188 |
| `examples/docker-compose.yml` | `examples/cronduit.toml` | config mount binding | WIRED | `./cronduit.toml:/etc/cronduit/config.toml:ro` at line 18 |
| `.github/workflows/release.yml` | `docker/build-push-action@v6` | GitHub Action builds and pushes multi-arch image | WIRED | `uses: docker/build-push-action@v6` at line 64, `platforms: linux/amd64,linux/arm64` |
| `.github/workflows/release.yml` | `cliff.toml` | git-cliff action generates changelog body | WIRED | `uses: orhun/git-cliff-action@v4` with `config: cliff.toml` |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|--------------------|--------|
| `src/web/handlers/sse.rs` | log_line events | `tokio::sync::broadcast::Receiver<LogLine>` — populated by `log_writer_task` in run.rs which reads from the live log pipeline | Yes — log_writer_task receives from LogReceiver (run output), publishes to broadcast_tx | FLOWING |
| `src/web/handlers/metrics.rs` | metrics body | `PrometheusHandle.render()` — populated by `metrics::counter!/histogram!/gauge!` macros throughout scheduler lifecycle | Yes — macros called at finalize_run, sync_jobs | FLOWING |
| `src/scheduler/retention.rs` | cutoff, rows_deleted | `queries::delete_old_logs_batch` / `delete_old_runs_batch` — reads from SQLite `job_logs`/`job_runs` tables | Yes — DELETE queries with cutoff binding against real DB tables | FLOWING |

### Behavioral Spot-Checks

Step 7b: SKIPPED (requires running server — all checks depend on a live Cronduit process with Docker daemon)

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|------------|------------|-------------|--------|---------|
| UI-14 | 06-01-PLAN.md | Run Detail page SSE streaming for in-progress runs | SATISFIED | `src/web/handlers/sse.rs`, broadcast wiring in `run.rs`, conditional template rendering in `run_detail.html` |
| OPS-02 | 06-02-PLAN.md | GET /metrics with four Prometheus metric families and closed-enum reason label | SATISFIED | `src/web/handlers/metrics.rs`, `src/telemetry.rs`, `src/scheduler/run.rs` instrumentation, FailureReason enum |
| DB-08 | 06-03-PLAN.md | Daily retention pruner in batched transactions | SATISFIED | `src/scheduler/retention.rs`, `src/db/queries.rs` retention queries, spawned in `src/cli/run.rs` |
| OPS-04 | 06-04-PLAN.md & 06-05-PLAN.md | Example docker-compose.yml with socket mount, read-only config, named SQLite volume | PARTIALLY SATISFIED | docker-compose.yml has all required elements but uses `ports:` instead of ROADMAP-specified `expose:` |
| OPS-05 | 06-04-PLAN.md | README quickstart for clone-to-running-job in under 5 minutes | NEEDS HUMAN | README has 3-step quickstart; actual 5-minute test requires human with Docker environment |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `tests/sse_streaming.rs` | 15, 23, 31, 39 | `todo!("Implement SSE...")` | Info | Intentional Wave 0 Nyquist compliance stubs — not blockers |
| `tests/metrics_endpoint.rs` | 14, 23, 32, 38 | `todo!("Implement metrics...")` | Info | Intentional Wave 0 Nyquist compliance stubs — not blockers |
| `tests/retention_integration.rs` | 15, 24, 32, 39, 47 | `todo!("Implement retention...")` | Info | Intentional Wave 0 Nyquist compliance stubs — not blockers |

All `todo!()` stubs are documented intentional placeholders per the plans' "Wave 0 Nyquist" policy. They compile but panic if run, which is the expected behavior. No unintentional stubs found in production code paths.

### Human Verification Required

#### 1. Quickstart End-to-End Test

**Test:** Clone the repo, place `examples/cronduit.toml` alongside `examples/docker-compose.yml`, run `docker compose -f examples/docker-compose.yml up -d`, open http://localhost:8080
**Expected:** Web UI loads showing jobs dashboard with echo-timestamp and alpine-hello; echo-timestamp fires within ~1 minute showing a completed run in the run history
**Why human:** Requires a built and published Docker image (`ghcr.io/simplicityguy/cronduit:latest`) and a running Docker daemon; cannot pull/run containers in verification context

#### 2. SSE Live Log Streaming

**Test:** Trigger a long-running job (e.g., one that sleeps for 30 seconds), open its Run Detail page immediately
**Expected:** LIVE badge visible; log lines stream in as they are produced without page reloads; when run completes, the view transitions to static paginated log viewer seamlessly via HTMX swap
**Why human:** Requires a live Cronduit instance with an active job; SSE streaming behavior is inherently real-time and cannot be statically verified

### Gaps Summary

One gap was found against the ROADMAP success criteria:

**ports: vs expose: in docker-compose.yml**

ROADMAP SC 4 explicitly states the quickstart docker-compose.yml should use `expose:` (not `ports:`). The actual file uses `ports: "8080:8080"`. This is an intentional deviation — Plan 04 decision D-12 deliberately chose `ports:` for quickstart accessibility (a stranger needs to reach the UI at localhost:8080 without additional configuration). The file includes a comment explaining to use `expose:` for production.

This looks intentional. To accept this deviation, add to this VERIFICATION.md frontmatter:

```yaml
overrides:
  - must_have: "example docker-compose.yml uses expose: (not ports:) for the web UI"
    reason: "Plan 04 D-12 explicitly chose ports: 8080:8080 for quickstart accessibility. The file includes comments recommending expose: for production. A stranger following the quickstart needs direct port access at localhost:8080."
    accepted_by: "your-name"
    accepted_at: "2026-04-12T22:30:00Z"
```

---

_Verified: 2026-04-12T22:30:00Z_
_Verifier: Claude (gsd-verifier)_
