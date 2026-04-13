---
status: complete
phase: 06-live-events-metrics-retention-release-engineering
source:
  - 06-01-SUMMARY.md
  - 06-02-SUMMARY.md
  - 06-03-SUMMARY.md
  - 06-04-SUMMARY.md
  - 06-05-SUMMARY.md
started: 2026-04-13T18:09:07Z
updated: 2026-04-13T18:32:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Cold Start Smoke Test
expected: |
  Kill any running cronduit process. Delete/move any existing SQLite DB file so startup is truly cold. Run `cargo run -- run --config examples/cronduit.toml` (or the equivalent binary). The process boots without errors, applies migrations, logs "scheduler started" / "retention pruner started" / "metrics recorder installed" (or equivalent), binds the HTTP server, and `curl http://127.0.0.1:8080/health` returns 200.
result: pass

### 2. Prometheus /metrics Endpoint
expected: |
  With cronduit running, `curl -s http://127.0.0.1:8080/metrics` returns Prometheus text format with Content-Type `text/plain`. Body contains `cronduit_scheduler_up 1`, `cronduit_jobs_total` gauge, and the metric families `cronduit_runs_total`, `cronduit_run_duration_seconds`, `cronduit_run_failures_total`. After at least one job has run, `cronduit_runs_total{...,status="success"}` should have incremented.
result: issue
reported: |
  After a successful run, `/metrics` body contains `cronduit_scheduler_up 1`,
  `cronduit_runs_total{job="sse-test-job",status="success"} 1`, and the
  `cronduit_run_duration_seconds` histogram — but `cronduit_jobs_total` gauge is
  absent. (run_failures_total counter reasonably absent before any failure, but
  jobs_total is documented as set at sync time.)
severity: major

### 3. SSE Live Log Streaming on Run Detail
expected: |
  Configure a job that runs long enough to observe (e.g., `sh -c 'for i in 1 2 3 4 5; do echo line $i; sleep 1; done'`). Trigger it (schedule or Run Now). Open the Run Detail page while it's running: a `LIVE` badge is visible and log lines appear in real time without refreshing the page (auto-scrolling). When the run completes, the live viewer is swapped out for a static log viewer via HTMX OOB swap — no full page reload, all 5 lines remain visible.
result: pass

### 4. docker-compose Quickstart
expected: |
  From a clean checkout: `cd examples && docker compose up`. The cronduit container starts, mounts the host Docker socket, binds `127.0.0.1:8080` (or the documented bind), and serves the UI at http://localhost:8080/. The two quickstart jobs (`echo-timestamp` every minute, `alpine-hello`/`hello-world` every 5 minutes) appear in the UI and produce successful run rows with log output within their cadence. No crash loops, no missing-volume errors.
result: issue
reported: |
  `docker compose up` starts the container (port 8080 mapped, container healthy) but
  `curl localhost:8080` returns `curl: (52) Empty reply from server`. Container logs
  show `"bind":"127.0.0.1:8080"` and `"job_count":1` (not 2). Root cause:
  examples/docker-compose.yml:18 mounts `./cronduit-uat.toml` (a test scratch file)
  instead of `./cronduit.toml`, and examples/cronduit-uat.toml:2 binds to
  `127.0.0.1:8080` — unreachable from Docker's published host port, which forwards
  to the container's eth0 interface, not its loopback.
severity: blocker

### 5. README Security-First Structure
expected: |
  Open `README.md`. The first H2 is a SECURITY section (covering bind default, Docker socket exposure, no auth in v1, reverse-proxy guidance). Quickstart comes after Security. The file contains a mermaid architecture diagram (not ASCII art), configuration reference for all three job types (command/script/docker), and a monitoring section listing the Prometheus metric names from plan 02.
result: pass

### 6. THREAT_MODEL.md Coverage
expected: |
  Open `THREAT_MODEL.md`. Four threat models are present with full sections: (1) Docker socket exposure, (2) untrusted web client, (3) config file tampering, (4) malicious container image. Each model has Threat / Attack Vector / Mitigations / Residual Risk / Recommendations. A consolidated STRIDE summary table is at the end. No "TBD" markers remain.
result: pass

### 7. Retention Pruner Wired at Startup
expected: |
  At cronduit startup (Test 1), tracing output on the `cronduit.retention` target confirms the retention pruner task spawned with the configured `log_retention` duration. The task is visible as a running tokio task (no panic, no immediate exit). Exact 24h behavior is not tested live — documentation says "prune fires 24h after startup, skipping initial tick."
result: issue
reported: |
  User ran `cargo run -- run` and the startup logs contain traces from cronduit.sync,
  cronduit.startup, cronduit.reload, cronduit::web, and cronduit.scheduler — but
  zero lines from the `cronduit.retention` target. Verified in code: retention_pruner
  IS spawned at src/cli/run.rs:187-189, but src/scheduler/retention.rs never emits
  a startup log. Its first tracing::info! on the cronduit.retention target happens
  inside run_prune_cycle (line 43), and the loop skips the initial tick, so no
  retention log surfaces until ~24h after startup. Operators have no way to
  confirm the pruner is wired up at boot time.
severity: minor

### 8. Release Workflow File
expected: |
  `.github/workflows/release.yml` exists and is structurally valid YAML. Workflow triggers on `v*` tag push, uses `docker/build-push-action@v6`, builds `linux/amd64,linux/arm64`, injects OCI labels (source, description, licenses, version, revision), invokes `git-cliff` for changelog, and creates a GitHub Release with the changelog body. `cliff.toml` exists with conventional commit parsers for feat/fix/refactor/perf/test/docs/ci. `justfile` has a `release` recipe.
result: pass

## Summary

total: 8
passed: 5
issues: 3
pending: 0
skipped: 0

## Gaps

- truth: "After startup (config sync), /metrics must expose `cronduit_jobs_total` gauge reflecting the number of configured jobs, as documented in plan 02 SUMMARY (src/scheduler/sync.rs: 'cronduit_jobs_total gauge after sync')."
  status: failed
  reason: "User reported: /metrics body after a successful sse-test-job run contains scheduler_up, runs_total, and run_duration_seconds histogram, but cronduit_jobs_total gauge is absent. Counter families that lazily register on first observation are acceptable, but jobs_total should be set at config sync time and therefore present from startup."
  severity: major
  test: 2
  artifacts: []
  missing: []

- truth: "The Job Detail Run History card must auto-refresh while runs are in-flight so completed runs transition from RUNNING to SUCCESS/FAILED without a manual page reload."
  status: failed
  reason: "User reported mid-UAT: clicking Run Now ~15 times in succession correctly appends new RUNNING rows (thanks to the earlier HX-Refresh fix), but the Run History stays frozen at RUNNING after the jobs finish — only a manual browser reload shows them transitioning to SUCCESS. Screenshot: ~15 RUNNING rows stacked on top of three SUCCESS rows several seconds after completion."
  severity: major
  test: out-of-band (filed during Test 4 setup)
  routed_to: .planning/phases/07-v1-cleanup-bookkeeping/07-05-PLAN.md
  artifacts:
    - src/web/handlers/job_detail.rs
    - templates/pages/job_detail.html
  missing: []

- truth: "`docker compose up` from the documented quickstart (`cd examples && docker compose up`) must serve the Cronduit UI at http://localhost:8080/ with the two documented quickstart jobs (echo-timestamp, alpine-hello/hello-world) visible."
  status: failed
  reason: |
    examples/docker-compose.yml:18 mounts `./cronduit-uat.toml:/etc/cronduit/config.toml:ro`
    instead of `./cronduit.toml`. cronduit-uat.toml:2 sets `bind = "127.0.0.1:8080"`,
    which inside a Docker container is only reachable from within that container —
    Docker's `-p 8080:8080` port publishing forwards to the container's eth0
    interface, not its loopback, so the host sees `curl: (52) Empty reply from server`.
    The container logs confirm this: `"bind":"127.0.0.1:8080"` and `"job_count":1`
    (cronduit-uat.toml has one sse-test-job, not the two documented quickstart jobs).
    cronduit-uat.toml appears to be a local testing scratch file that should not
    be referenced by the quickstart docker-compose.yml.
  severity: blocker
  test: 4
  fix_sketch: |
    1. Change examples/docker-compose.yml:18 to mount `./cronduit.toml` (the
       file that already has `bind = "0.0.0.0:8080"` at line 16 and the two
       documented quickstart jobs).
    2. Either delete examples/cronduit-uat.toml if it is purely a local dev
       artifact, or keep it but rename with a `.local.toml` suffix and add to
       .gitignore so it cannot leak into the quickstart path again.
    3. Consider adding a CI smoke that actually brings up the quickstart
       compose file and asserts `curl -sSf localhost:8080/health` returns 200 —
       this class of bug (config mount path + inner bind) is exactly what the
       cold-start smoke test was meant to catch but can only catch against the
       file the quickstart actually mounts.
  artifacts:
    - examples/docker-compose.yml
    - examples/cronduit-uat.toml
    - examples/cronduit.toml
  missing: []

- truth: "At cronduit startup, the retention pruner must emit a tracing log on the `cronduit.retention` target confirming the task was spawned with the configured `log_retention` duration, so operators can verify on boot that retention is wired up (without waiting 24h for the first prune cycle)."
  status: failed
  reason: |
    retention_pruner() IS spawned at src/cli/run.rs:187-189 — verified in code —
    but src/scheduler/retention.rs never emits a tracing log at task-spawn time.
    Its first info-level trace on the cronduit.retention target is at line 43
    inside run_prune_cycle ("retention prune cycle started"), and the scheduler
    loop skips the initial tick, so the first retention log does not surface
    until ~24h after startup. User startup logs confirm this: cronduit.sync,
    cronduit.startup, cronduit.reload, cronduit::web, and cronduit.scheduler
    all emit at boot; cronduit.retention emits nothing. Functional behavior is
    correct (the task is running), but observability is broken — an operator
    cannot tell from boot-time logs whether retention is wired up.
  severity: minor
  test: 7
  fix_sketch: |
    Add a single `tracing::info!(target: "cronduit.retention", retention_secs = ?retention.as_secs(), "retention pruner started")` line at the top of retention_pruner() in src/scheduler/retention.rs, before the interval loop. Mirror the scheduler's existing "scheduler started" pattern.
  artifacts:
    - src/scheduler/retention.rs
  missing: []
