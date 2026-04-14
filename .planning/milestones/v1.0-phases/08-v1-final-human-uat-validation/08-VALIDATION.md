---
phase: 08
slug: v1-final-human-uat-validation
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-13
updated: 2026-04-14
---

# Phase 8 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> See `08-RESEARCH.md § Validation Architecture` for per-decision mapping.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` + `cargo nextest` (Rust); GitHub Actions `compose-smoke` (CI shell); manual UAT (user-driven) |
| **Config file** | `Cargo.toml`, `.github/workflows/ci.yml` (compose-smoke job) |
| **Quick run command** | `cargo nextest run --package cronduit --lib` |
| **Full suite command** | `cargo nextest run --all-targets --all-features` |
| **Estimated runtime** | ~90 seconds (unit); ~3 minutes (full with compose-smoke in CI) |

---

## Sampling Rate

- **After every task commit:** Run `cargo check --all-targets`
- **After every plan wave:** Run `cargo nextest run --all-targets`
- **Before `/gsd-verify-work`:** Full suite green + `compose-smoke` CI matrix green on both compose files
- **Max feedback latency:** 90 seconds (unit layer); 180 seconds (compose-smoke layer)

---

## Per-Task Verification Map

*Planner originally left this table as a test-type menu with no task rows. Filled in retroactively 2026-04-14 against the 5 plans that actually executed.*

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 08-01-01 | 01 | 1 | OPS-05 (quickstart alpine rebase) | — | Runtime rebased from distroless to alpine:3, UID 1000, `/data` pre-owned | docker-build + compose-smoke | `docker build -t cronduit:test .` + `ci.yml::compose-smoke` asserts all 4 jobs reach `success` within 120s on fresh build | ✅ `Dockerfile:82-112` | ✅ green (CI) |
| 08-01-02 | 01 | 1 | OPS-05 (4-job quickstart config) | — | echo-timestamp, http-healthcheck, disk-usage, hello-world all runnable under alpine:3 | compose-smoke | `ci.yml::compose-smoke` Run Now API per-job assertion (120s budget) | ✅ `examples/cronduit.toml` | ✅ green (CI) |
| 08-02-01 | 02 | 1 | OPS-04 (default compose refresh) | T-8-01 | `group_add` + `DOCKER_GID` for host socket access; no-bridge for security posture | compose-smoke-default | `ci.yml::compose-smoke[compose=docker-compose.yml]` | ✅ `examples/docker-compose.yml` | ✅ green (CI) |
| 08-02-02 | 02 | 1 | OPS-04 (secure compose + socket-proxy) | T-8-01, T-8-02 | Bollard routes via `DOCKER_HOST=tcp://dockerproxy:2375`; CONTAINERS/IMAGES/POST/DELETE allowlist | compose-smoke-secure | `ci.yml::compose-smoke[compose=docker-compose.secure.yml]` | ✅ `examples/docker-compose.secure.yml` | ✅ green (CI) |
| 08-03-01 | 03 | 1 | DOCKER-01 + OPS-02 (docker daemon pre-flight) | T-8-03 | `cronduit_docker_reachable` gauge described+registered before first scrape; pre-flight ping at boot non-fatal WARN on unreachable | unit-rs + integration-rs | `cargo test --lib scheduler::docker_daemon` (2 unit tests: `update_reachable_gauge_is_safe_without_recorder`, `preflight_ping_with_none_sets_gauge_zero_and_does_not_panic`) + `cargo test --test docker_daemon_preflight docker_daemon_preflight_gauge_lifecycle` | ✅ `src/scheduler/docker_daemon.rs`, `tests/docker_daemon_preflight.rs` | ✅ green |
| 08-04-01 | 04 | 2 | OPS-04/05 (CI compose-smoke matrix) | — | Matrix over both compose files; per-job Run Now success assertion within 120s; extended failure diagnostics | self-verifying CI | This task IS the test infrastructure used by 08-01/02 | ✅ `ci.yml:133-360` | ✅ green (CI) |
| 08-05-01 | 05 | 3 | UI-05, UI-06, UI-09, UI-12, UI-14, OPS-05 + 07-UAT re-run | — | Human UAT walkthrough orchestration — no code changes, only UAT file fixtures | manual-user-uat | USER-DRIVEN (walkthrough) | N/A (prep only) | ⚠️ covered-verbally (orchestrator decision pending per v1.0 milestone audit OPS-05 partial) |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky/unresolved*

### Test Type Reference

| Test type | Used for | Automated command |
|-----------|----------|-------------------|
| unit-rs | Gauge register/describe wiring, `Docker::connect_with_defaults` resolution, pre-flight `None`-handle path | `cargo nextest run --lib scheduler::docker_daemon` |
| integration-rs | Gauge lifecycle across pre-flight boundary | `cargo nextest run --test docker_daemon_preflight` |
| docker-build | Dockerfile alpine rebase + UID 1000 + /data ownership on fresh build | `docker build -t cronduit:test . && docker run --rm cronduit:test --version` |
| compose-smoke-default | `docker-compose.yml` matrix axis: 4 example jobs → status=success within 120s | `.github/workflows/ci.yml::compose-smoke[compose=docker-compose.yml]` |
| compose-smoke-secure | `docker-compose.secure.yml` matrix axis: same assertion via socket-proxy | `.github/workflows/ci.yml::compose-smoke[compose=docker-compose.secure.yml]` |
| manual-user-uat | Human visual/interaction verification (terminal-green theme, dark mode, Run Now toast, ANSI logs, SSE LIVE badge, auto-refresh transition, quickstart end-to-end) | USER-DRIVEN (Claude prepares fixtures only) |

---

## Wave 0 Requirements

Phase 8 is gap-closure over an already-tested codebase. Wave 0 dependencies:

- [x] No new test framework needed — `cargo test` + `nextest` already in use since Phase 1
- [x] No new testcontainers module needed — alpine is a public base image
- [x] `.github/workflows/ci.yml::compose-smoke` already exists (Phase 6 gap closure) — Phase 8 **extended** it via the `compose` matrix axis (docker-compose.yml + docker-compose.secure.yml)
- [x] `examples/docker-compose.secure.yml` created during Plan 08-02
- [x] `tests/docker_daemon_preflight.rs` created during Plan 08-03 for the gauge-lifecycle integration test

*Existing infrastructure plus Phase 8's own deliverables cover all automated validation.*

---

## Manual-Only Verifications

Per project memory rule **"UAT requires user validation"**, Claude MUST NOT mark any of these `pass` from its own runs. Claude prepares fixtures; user clicks + types result.

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Terminal-green theme rendering in browser | UI-05 | Visual/subjective — palette compliance to `design/DESIGN_SYSTEM.md` | User navigates to dashboard, checks color tokens against design system, records `pass` / `issue` in `03-HUMAN-UAT.md` |
| Dark/light mode toggle persistence across page reloads | UI-06 | Cross-session state — localStorage survival can't be headlessly asserted with confidence | User toggles theme, hard-refreshes, confirms selected mode persists in new tab, records in `03-HUMAN-UAT.md` |
| Run Now toast appearance + auto-dismiss | UI-09 | Visual timing + HX-Trigger response path | User clicks Run Now on a job, confirms green toast appears within 500ms and auto-dismisses within 5s, records in `03-HUMAN-UAT.md` |
| ANSI log rendering with stdout/stderr distinction | UI-12 | Color rendering + stderr styling are visually inspected | User opens Run Detail for a job with known stderr output, confirms stdout is default-color and stderr is red (or equivalent brand-accent), records in `03-HUMAN-UAT.md` |
| Quickstart end-to-end (clone → docker compose up → dashboard loads → first echo-timestamp run visible) | OPS-05 | End-user workflow spanning multiple tools (git, docker, browser) | User follows README quickstart from a fresh clone on their machine, confirms first `echo-timestamp` run appears in history within ~60s of compose up, records in new `06-HUMAN-UAT.md` |
| SSE live log streaming (LIVE badge → real-time lines → transition to static viewer on completion) | UI-14 | Server-Sent Events observation is inherently real-time; automated assertions drop race conditions | User triggers a long-running job, opens Run Detail, confirms LIVE badge visible, watches log lines stream, confirms transition to static viewer on completion without page reload, records in `06-HUMAN-UAT.md` |
| Plan 07-05 auto-refresh (10+ Run Now clicks → RUNNING → terminal transitions without manual reload within ~2s) | n/a (Phase 7 blocked-test re-run) | HTMX polling transition needs sustained RUNNING state + multi-second observation | User clicks Run Now 10+ times on `http-healthcheck` or `disk-usage` job, confirms list updates automatically, confirms polling stops when list is idle (check DevTools Network tab), records in `07-UAT.md` (flip `blocker` → `pass` or `issue`, add `re_tested_at`) |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or are listed in the manual-only table above
- [x] Sampling continuity: no 3 consecutive tasks without automated verify (UAT tasks explicitly exempt)
- [x] Wave 0 covers all MISSING references
- [x] No `cargo watch` / watch-mode flags in plans
- [x] Feedback latency < 180s (including compose-smoke CI extension)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved (retroactive audit 2026-04-14)

---

## Validation Audit 2026-04-14

| Metric | Count |
|--------|-------|
| Gaps found | 1 (Per-Task Verification Map was never filled — left as planner menu stub) |
| Resolved | 1 (filled retroactively from the 5 plans that actually executed) |
| Escalated | 0 |

**Audit method:** Retroactive cross-reference against `src/scheduler/docker_daemon.rs`, `tests/docker_daemon_preflight.rs`, `examples/cronduit.toml`, `examples/docker-compose.yml`, `examples/docker-compose.secure.yml`, `Dockerfile`, and `.github/workflows/ci.yml` (compose-smoke matrix job 133-360).

The original VALIDATION.md was created 2026-04-13 during discuss-phase and left the Per-Task Verification Map as a test-type menu with the note "Planner fills this in as tasks are created — one row per task." The planner never filled it. The phase executed anyway and `08-VERIFICATION.md` confirmed 10/12 must-haves verified (status `human_needed` for 2 UAT-policy items that are tracked in the v1.0 milestone audit as OPS-05 bookkeeping debt, not code gaps).

**Key evidence:**
- `src/scheduler/docker_daemon.rs::tests` — 2 unit tests (`update_reachable_gauge_is_safe_without_recorder`, `preflight_ping_with_none_sets_gauge_zero_and_does_not_panic`)
- `tests/docker_daemon_preflight.rs::docker_daemon_preflight_gauge_lifecycle` — 1 integration test
- `Dockerfile:82-112` — alpine:3 rebase with UID 1000 pre-owning `/data`
- `examples/cronduit.toml` — 4 runnable quickstart jobs (echo-timestamp, http-healthcheck, disk-usage, hello-world)
- `examples/docker-compose.yml` — default compose with `group_add` + `DOCKER_GID`
- `examples/docker-compose.secure.yml` — socket-proxy sidecar with CONTAINERS/IMAGES/POST/DELETE allowlist
- `.github/workflows/ci.yml:133-360` — compose-smoke matrix over both compose files, Run Now API per-job success assertion within 120s

**Manual-only items retained as legitimate:** the 7 UAT behaviors in the Manual-Only table are all true user-validation items (visual theme, dark mode persistence, toast timing, ANSI rendering, quickstart E2E timing, SSE observation, HTMX polling observation). Per project policy `feedback_uat_user_validates.md`, Claude does not flip `result: pending` fields — the user verbally approved the walkthrough in 08-05-SUMMARY.md but the on-disk fields in 03/06/07/08-HUMAN-UAT.md remain pending pending the orchestrator's decision on verbal-vs-per-row (see v1.0 milestone audit for full context). Row 08-05-01 is marked ⚠️ to reflect this.
