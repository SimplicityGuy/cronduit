---
phase: 08-v1-final-human-uat-validation
plan: 03
subsystem: scheduler/docker + telemetry
tags: [docker, preflight, metrics, observability]
requires:
  - Phase 6 GAP-1 describe_*/register pattern in src/telemetry.rs
  - bollard::Docker::ping() (bollard 0.20)
  - metrics facade + metrics-exporter-prometheus 0.18
provides:
  - src/scheduler/docker_daemon::preflight_ping — non-fatal startup ping
  - src/scheduler/docker_daemon::update_reachable_gauge — opportunistic gauge update
  - cronduit_docker_reachable Prometheus gauge (registered + described from boot)
affects:
  - src/cli/run.rs startup sequence (one new .await after Docker client creation)
  - /metrics endpoint body (new metric family)
tech-stack:
  added: []
  patterns:
    - Reuse of Phase 6 GAP-1 describe_*/register pattern for the new gauge
    - Non-fatal preflight: Option<&Docker> + Result<_> ignored, log + gauge flip only
key-files:
  created:
    - src/scheduler/docker_daemon.rs
    - tests/docker_daemon_preflight.rs
  modified:
    - src/scheduler/mod.rs
    - src/telemetry.rs
    - src/cli/run.rs
decisions:
  - D-11 implemented: preflight_ping fires once at startup, after Docker client creation, before orphan reconciliation and before scheduler loop.
  - D-12 implemented: cronduit_docker_reachable gauge lives in Phase 6 metric family; registered at boot with value 0.0.
  - D-13 honored: preflight only at startup — scheduler hot path can call update_reachable_gauge(true) opportunistically (wired hook, not yet connected to container create path in this plan; remains available for docker.rs to invoke).
  - D-14 implemented: ping failures never propagate up — cronduit keeps booting on unreachable daemon.
  - Rejected adding an explicit tokio::time::timeout wrap around docker.ping() — bollard's HTTP client already enforces a connect timeout via connect_with_local_defaults(), and a belt-and-suspenders wrap would mask future ops signal. Threat T-08-11 documents the fallback pattern if ops reveals longer hangs in v1.1.
metrics:
  duration: ~4 minutes wall-clock (excluding cargo-nextest install)
  completed: 2026-04-13
  tasks-completed: 2
  files-created: 2
  files-modified: 3
  commits: 2
---

# Phase 8 Plan 03: Docker Daemon Pre-flight Ping Summary

**One-liner:** Non-fatal `bollard::Docker::ping()` at startup exposes a new
`cronduit_docker_reachable` Prometheus gauge and a single-line WARN with
remediation hints for unreachable-daemon scenarios (macOS, bad `group_add`,
socket-proxy down) — operators can now alert on `cronduit_docker_reachable == 0`
without parsing logs.

## Context

Phase 8 closes the v1.0 UAT-blocking gaps. Plan 03 addresses blocker #2 from
`07-UAT.md`: when bollard cannot reach `/var/run/docker.sock` (the dominant
homelab failure mode on macOS Docker Desktop), the operator currently only
discovers the problem on the first docker-job failure. The preflight ping gives
a boot-time signal both in logs (WARN) and in metrics (gauge), and keeps
cronduit bootable so command/script-only configs still work.

This plan is deliberately narrow — it does NOT touch the runtime image (Plan
08-01) or the dual compose files (Plan 08-02). It only adds the observable
signal the other plans rely on.

## File-by-File Diff Summary

### Created: `src/scheduler/docker_daemon.rs` (+88 lines)

New module with two public functions:

- `pub async fn preflight_ping(docker: Option<&Docker>)` — D-11/D-14 startup
  ping. Takes `Option<&Docker>` so the wiring point in `run.rs` can pass
  `docker.as_ref()` regardless of whether `connect_with_local_defaults()`
  succeeded. On `None` (client init failed upstream) OR `Err` from ping, logs
  WARN with remediation hints and flips the gauge to 0. On `Ok`, logs INFO and
  flips to 1. Never returns an error — always `()`.
- `pub fn update_reachable_gauge(reachable: bool)` — D-12/D-13 synchronous
  helper the docker job hot path can call on container create success/failure
  to flip the gauge without a full reload. Thin wrapper around
  `metrics::gauge!("cronduit_docker_reachable").set(value)`.

Inline unit tests (`#[cfg(test)]`):
- `preflight_ping_with_none_sets_gauge_zero_and_does_not_panic` — proves the
  function is non-fatal when no recorder is installed (metrics macros are
  no-ops).
- `update_reachable_gauge_is_safe_without_recorder` — same safety property for
  the direct update helper.

WARN template character counts (measured on the string literal, not the Rust
source line): **273 chars** for the `None` branch, **253 chars** for the `Err`
branch. Both under the 280-char plan ceiling.

### Created: `tests/docker_daemon_preflight.rs` (+65 lines)

Single `#[tokio::test]` integration test (`docker_daemon_preflight_gauge_lifecycle`)
that exercises the gauge through the real `PrometheusHandle::render()` path —
the same path `/metrics` serves. Four sequential phases inside one test because
`telemetry::setup_metrics()` uses a process-global `OnceLock` (cannot safely
run parallel tests that race on it):

1. **Registered from boot at zero** — asserts `# HELP`, `# TYPE`, and
   `cronduit_docker_reachable 0` all appear in the initial render output.
2. **`update_reachable_gauge(true)` flips to 1** — assert `cronduit_docker_reachable 1`.
3. **`update_reachable_gauge(false)` flips back to 0** — assert
   `cronduit_docker_reachable 0`.
4. **`preflight_ping(None)` forces zero even after a prior 1** — first bumps
   the gauge to 1 to prove the preflight overrides it, then runs the ping and
   asserts the gauge reads 0.

### Modified: `src/scheduler/mod.rs` (+1 line)

Added `pub mod docker_daemon;` in alphabetical order between `docker` and
`docker_log`. No other changes.

### Modified: `src/telemetry.rs` (+5 lines)

Two changes to `setup_metrics()`, matching the Phase 6 GAP-1 pattern:

1. Added `metrics::describe_gauge!("cronduit_docker_reachable", "1 if the
   docker daemon was reachable at last ping, 0 otherwise (Phase 8 D-12)")`
   after the existing `describe_counter!("cronduit_run_failures_total", ...)`.
2. Added `metrics::gauge!("cronduit_docker_reachable").set(0.0)` after the
   existing `metrics::counter!("cronduit_run_failures_total").increment(0)`.

This ensures the gauge renders HELP/TYPE lines from boot (same treatment as
the five pre-existing families) with initial value 0.

### Modified: `src/cli/run.rs` (+16 lines)

Two additions around the existing `let docker = match
bollard::Docker::connect_with_local_defaults()` block:

1. **Above** the `let docker = match ...`: a 4-line doc comment explaining
   that bollard reads `DOCKER_HOST` from the environment — this is how
   `examples/docker-compose.secure.yml` will route traffic to the
   `docker-socket-proxy` sidecar (Plan 08-02 groundwork) without any cronduit
   code change.
2. **Below** the `let docker = match ...` block, **before** the existing
   orphan reconciliation `if let Some(ref docker_client) = docker { ... }`:
   an 11-line block that invokes `crate::scheduler::docker_daemon::
   preflight_ping(docker.as_ref()).await` with a comment block explaining the
   non-fatal contract and the `DOCKER_HOST` + socket-proxy story.

The existing `let docker = match ...` expression itself is untouched — no
reordering, no changes to the `"Docker client connected"` / `"Docker client
unavailable"` log lines.

## Verification

All plan verification commands pass:

| Command | Result |
|---|---|
| `cargo nextest run --test docker_daemon_preflight docker_daemon_preflight_gauge_lifecycle` | 1 passed |
| `cargo nextest run --test metrics_endpoint metrics_families_described_from_boot` | 1 passed (no regression) |
| `cargo nextest run --package cronduit docker_daemon` | 2 passed (unit tests) |
| `cargo clippy --all-targets --tests -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| `grep -c 'pub mod docker_daemon' src/scheduler/mod.rs` | 1 |
| `grep -c 'describe_gauge!' src/telemetry.rs` | 3 |
| `grep -c 'docker_daemon::preflight_ping' src/cli/run.rs` | 1 |
| `git diff src/scheduler/docker_preflight.rs` | empty (unchanged) |
| WARN string literal length (None branch) | 273 chars (< 280) |
| WARN string literal length (Err branch) | 253 chars (< 280) |

## Threat Model Follow-through

Per the plan's `<threat_model>` register:

- **T-08-10 (Info Disclosure, WARN line includes Docker URI):** Mitigation
  accepted. The WARN template uses `error = %err` interpolation which includes
  the bollard error string (which may embed `"unix:///var/run/docker.sock"`).
  This is low-sensitivity — already in compose files and documentation.
- **T-08-11 (DoS, preflight_ping blocks startup if bollard hangs):** Mitigation
  as documented. `docker.ping().await` relies on bollard's built-in connect
  timeout inherited from `connect_with_local_defaults()`. **No explicit
  `tokio::time::timeout` wrap was added** — see Decisions section for rationale.
  If future ops reveal longer hangs, the wrap is a 3-line change.
- **T-08-12 (Repudiation, gauge is not audit-grade):** Accepted. The
  cronduit_run_failures_total counter with closed-enum reason labels remains
  the authoritative trail.

## Decisions Made

- **No `tokio::time::timeout` wrap around `docker.ping()` (default path).**
  Rationale: bollard 0.20's HTTP client already enforces a connect timeout via
  `connect_with_local_defaults()`. Adding a second timeout would mask ops
  signal in failure modes and duplicate responsibility. T-08-11 documents the
  3-line fallback if v1.1 ops reveal the default is insufficient.
- **`Option<&Docker>` signature instead of `&Option<Docker>` or `&Docker`.**
  Rationale: the caller in `run.rs` has `docker: Option<Docker>` on the stack
  and can pass `docker.as_ref()` naturally. `None` is a real and distinct
  failure case (`connect_with_local_defaults()` failed upstream) that must
  flip the gauge to 0 and log WARN, same as a failed ping — so both cases
  collapse into the same function.
- **Inline unit tests inside `docker_daemon.rs` even though an integration
  test exists.** The unit tests prove the functions are safe to call when no
  recorder is installed (early test harness scenarios, cargo check under
  `#[cfg(test)]`). The integration test proves the full exporter render path
  works end-to-end. Both are cheap and cover different signal.

## Deviations from Plan

None — plan executed exactly as written. All acceptance criteria, verification
commands, and success criteria passed on first run. No rule-1/2/3 auto-fixes
were needed.

## Links

- `[D-11]` .planning/phases/08-v1-final-human-uat-validation/08-CONTEXT.md#decisions — preflight ping semantics
- `[D-12]` .planning/phases/08-v1-final-human-uat-validation/08-CONTEXT.md#decisions — cronduit_docker_reachable gauge definition
- `[D-13]` .planning/phases/08-v1-final-human-uat-validation/08-CONTEXT.md#decisions — startup + reload fire only
- `[D-14]` .planning/phases/08-v1-final-human-uat-validation/08-CONTEXT.md#decisions — non-fatal contract
- Commits: `49fa137` (Task 1 implementation), `32b6eb5` (Task 2 integration test)

## Self-Check: PASSED

- [x] `src/scheduler/docker_daemon.rs` exists
- [x] `tests/docker_daemon_preflight.rs` exists
- [x] `src/scheduler/mod.rs`, `src/telemetry.rs`, `src/cli/run.rs` all modified
- [x] Commit `49fa137` present in `git log`
- [x] Commit `32b6eb5` present in `git log`
- [x] All plan verification commands pass
- [x] `src/scheduler/docker_preflight.rs` unchanged (0 lines diff)
- [x] WARN template strings both under 280 chars (273 / 253)
