---
status: testing
phase: 06-live-events-metrics-retention-release-engineering
source:
  - 06-01-SUMMARY.md
  - 06-02-SUMMARY.md
  - 06-03-SUMMARY.md
  - 06-04-SUMMARY.md
  - 06-05-SUMMARY.md
started: 2026-04-13T02:00:17Z
updated: 2026-04-13T02:16:00Z
---

## Current Test

[paused — fixing 3 diagnosed bugs before resuming]

## Tests

### 1. Cold Start Smoke Test
expected: |
  Kill any running cronduit process. Delete/move any existing SQLite DB file so startup is truly cold. Run `cargo run -- run --config examples/cronduit.toml` (or the equivalent binary). The process boots without errors, applies migrations, logs "scheduler started" / "retention pruner started" / "metrics recorder installed" (or equivalent), binds the HTTP server, and `curl http://127.0.0.1:8080/health` returns 200.
result: issue
reported: |
  cargo run -- run --config examples/cronduit.toml
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.45s
       Running `target/debug/cronduit run --config examples/cronduit.toml`
  examples/cronduit.toml: error: [[jobs]] `alpine-hello` must declare exactly one of `command`, `script`, or `image` (found 2)

  1 error(s)
severity: blocker
follow_up: |
  After fixing alpine-hello, a SECOND failure surfaced: `Error: unable to open database file`.
  Root cause: default_db_url() in src/config/mod.rs:49-51 returns `sqlite:///data/cronduit.db`,
  a path that only exists inside the Docker image. Tracked as separate gap below.

### 2. Prometheus /metrics Endpoint
expected: |
  With cronduit running, `curl -s http://127.0.0.1:8080/metrics` returns Prometheus text format with Content-Type `text/plain`. Body contains `cronduit_scheduler_up 1`, `cronduit_jobs_total` gauge, and the metric families `cronduit_runs_total`, `cronduit_run_duration_seconds`, `cronduit_run_failures_total`. After at least one job has run, `cronduit_runs_total{...,status="success"}` should have incremented.
result: blocked
blocked_by: prior-phase
reason: "User reported: blocked — cronduit cannot boot due to Test 1 config validation failure, so no /metrics endpoint is reachable."

### 3. SSE Live Log Streaming on Run Detail
expected: |
  Configure a job that runs long enough to observe (e.g., `sh -c 'for i in 1 2 3 4 5; do echo line $i; sleep 1; done'`). Trigger it (schedule or Run Now). Open the Run Detail page while it's running: a `LIVE` badge is visible and log lines appear in real time without refreshing the page (auto-scrolling). When the run completes, the live viewer is swapped out for a static log viewer via HTMX OOB swap — no full page reload, all 5 lines remain visible.
result: issue
reported: "User reported during test setup: clicking Run Now on the Job Detail page does not refresh or navigate — the toast may fire, but the page stays on Job Detail with no indication a new run was created. User must manually reload to see the run appear."
severity: major

### 4. docker-compose Quickstart
expected: |
  From a clean checkout: `cd examples && docker compose up`. The cronduit container starts, mounts the host Docker socket, binds `127.0.0.1:8080` (or the documented bind), and serves the UI at http://localhost:8080/. The two quickstart jobs (`echo-timestamp` every minute, `alpine-hello` every 5 minutes) appear in the UI and produce successful run rows with log output within their cadence. No crash loops, no missing-volume errors.
result: [pending]

### 5. README Security-First Structure
expected: |
  Open `README.md`. The first H2 is a SECURITY section (covering bind default, Docker socket exposure, no auth in v1, reverse-proxy guidance). Quickstart comes after Security. The file contains a mermaid architecture diagram (not ASCII art), configuration reference for all three job types (command/script/docker), and a monitoring section listing the Prometheus metric names from plan 02.
result: [pending]

### 6. THREAT_MODEL.md Coverage
expected: |
  Open `THREAT_MODEL.md`. Four threat models are present with full sections: (1) Docker socket exposure, (2) untrusted web client, (3) config file tampering, (4) malicious container image. Each model has Threat / Attack Vector / Mitigations / Residual Risk / Recommendations. A consolidated STRIDE summary table is at the end. No "TBD" markers remain.
result: [pending]

### 7. Retention Pruner Wired at Startup
expected: |
  At cronduit startup (Test 1), tracing output on the `cronduit.retention` target confirms the retention pruner task spawned with the configured `log_retention` duration. The task is visible as a running tokio task (no panic, no immediate exit). Exact 24h behavior is not tested live — documentation says "prune fires 24h after startup, skipping initial tick."
result: [pending]

### 8. Release Workflow File
expected: |
  `.github/workflows/release.yml` exists and is structurally valid YAML. Workflow triggers on `v*` tag push, uses `docker/build-push-action@v6`, builds `linux/amd64,linux/arm64`, injects OCI labels (source, description, licenses, version, revision), invokes `git-cliff` for changelog, and creates a GitHub Release with the changelog body. `cliff.toml` exists with conventional commit parsers for feat/fix/refactor/perf/test/docs/ci. `justfile` has a `release` recipe.
result: [pending]

## Summary

total: 8
passed: 0
issues: 2
pending: 5
skipped: 0
blocked: 1

## Gaps

- id: GAP-1
  truth: "The documented quickstart command `cargo run -- run --config examples/cronduit.toml` must boot cronduit successfully on a clean checkout."
  status: failed
  reason: |
    examples/cronduit.toml:41-46 — the `alpine-hello` job declares BOTH `image = "alpine:latest"`
    AND `command = "echo 'Hello from an Alpine container!'"`. `JobConfig` in src/config/mod.rs:74-94
    exposes `command`, `script`, and `image` as three mutually-exclusive backend selectors
    (the validator enforces exactly-one), and there is no `args` / `docker_cmd` field to override
    a container's CMD. An image job that wants specific output must bake it into the image.
  severity: blocker
  test: 1
  fix: |
    Change `alpine-hello` to use `hello-world:latest` (the canonical Docker intro image — prints
    a visible greeting from its built-in ENTRYPOINT, no command override needed), and rename
    the job to `hello-world` for clarity. Drop the `command` line.
  artifacts:
    - examples/cronduit.toml
  missing: []

- id: GAP-2
  truth: "Running cronduit locally via `cargo run -- run --config examples/cronduit.toml` must open its SQLite database without requiring /data to exist on the host."
  status: failed
  reason: |
    src/config/mod.rs:49-51 — default_db_url() returns `sqlite:///data/cronduit.db`. /data is a
    Docker-image convention (see examples/docker-compose.yml:19 which mounts `cronduit-data:/data`),
    not a path that exists on developer machines. examples/cronduit.toml does not set an explicit
    `database_url`, so it falls through to this default and dies with `error returned from database:
    (code: 14) unable to open database file`. The first five Phase 6 tests that require a running
    cronduit are all blocked behind this.
  severity: blocker
  test: 1
  fix: |
    Add an explicit `database_url` line to examples/cronduit.toml using the env-var interpolation
    pattern the project already supports: `database_url = "${DATABASE_URL}"`. examples/docker-compose.yml
    already exports `DATABASE_URL=sqlite:///data/cronduit.db` at line 22, so the Docker path is
    unchanged. Add a README snippet showing local dev:
    `DATABASE_URL=sqlite://./cronduit.db?mode=rwc cargo run -- run --config examples/cronduit.toml`.
    This is the pattern CLAUDE.md documents: env-var interpolation, no `${VAR:-default}` syntax.
  artifacts:
    - examples/cronduit.toml
    - README.md
  missing: []

- id: GAP-3
  truth: "Clicking Run Now on a Job Detail page must give the user visible confirmation that the run was created — either by refreshing the page to show the new run row, or by navigating to the new Run Detail page."
  status: failed
  reason: |
    src/web/handlers/api.rs:26-76 — the `run_now` handler returns `(HxResponseTrigger::normal([event]),
    StatusCode::OK)` with only a `showToast` HX-Trigger event. No HX-Refresh, no HX-Redirect. Compare
    to the reload handler at src/web/handlers/api.rs:175-177 and the schedule-refresh handler at
    src/web/handlers/api.rs:259-262, both of which correctly emit `HX-Refresh: true` after state
    changes. This is the same class of bug that Phase 5 UAT filed against the reload button and PR #9
    fixed in-place — the Run Now button was simply never covered by that fix.
  severity: major
  test: 3
  fix: |
    Add `HX-Refresh: true` to the run_now handler's success response, matching the pattern already
    established at lines 175-177 and 259-262 of the same file. This is the minimal fix: after the
    Run Now POST, HTMX will reload the current page (Job Detail), and the new run row will appear
    in the runs list. A full HX-Redirect to /runs/{new_run_id} would be nicer but requires the
    scheduler command channel to return the new run_id back through a oneshot, which is a larger
    refactor. HX-Refresh is consistent with the reload card pattern and unblocks UAT Test 3.
  artifacts:
    - src/web/handlers/api.rs
  missing: []
