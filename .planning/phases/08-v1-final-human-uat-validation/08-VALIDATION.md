---
phase: 08
slug: v1-final-human-uat-validation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-13
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

(Planner fills this in as tasks are created — one row per task. The test types below are the menu.)

| Test type | Used for | Automated command |
|-----------|----------|-------------------|
| unit-rs | `docker_preflight::validate_named_network` behavior, gauge register/describe wiring, `Docker::connect_with_defaults` resolution | `cargo nextest run --package cronduit preflight` |
| integration-rs | Bollard ping + gauge flip against a real daemon (feature-gated) | `cargo nextest run --features integration bollard_ping` |
| docker-build | Dockerfile alpine rebase + UID 1000 + /data ownership on fresh build | `docker build -t cronduit:test . && docker run --rm cronduit:test --version` |
| compose-smoke-default | `docker-compose.yml` matrix axis: 4 example jobs → status=success within 120s | `.github/workflows/ci.yml::compose-smoke[compose=docker-compose.yml]` |
| compose-smoke-secure | `docker-compose.secure.yml` matrix axis: same assertion via socket-proxy | `.github/workflows/ci.yml::compose-smoke[compose=docker-compose.secure.yml]` |
| manual-user-uat | Human visual/interaction verification (terminal-green theme, dark mode, Run Now toast, ANSI logs, SSE LIVE badge, auto-refresh transition, quickstart end-to-end) | USER-DRIVEN (Claude prepares fixtures only) |

---

## Wave 0 Requirements

Phase 8 is gap-closure over an already-tested codebase. Wave 0 dependencies:

- [ ] No new test framework needed — `cargo test` + `nextest` already in use since Phase 1
- [ ] No new testcontainers module needed — alpine is a public base image
- [ ] `.github/workflows/ci.yml::compose-smoke` already exists (Phase 6 gap closure) — Phase 8 **extends** it, does not create from scratch
- [ ] `examples/docker-compose.secure.yml` does NOT yet exist — creating it is a Phase 8 task, not a Wave 0 dependency
- [ ] No new Rust test harness file required; existing `tests/reload_*.rs` pattern is the template for any new Rust integration test

*Existing infrastructure covers Phase 8 automated validation.*

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

- [ ] All tasks have `<automated>` verify or are listed in the manual-only table above
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify (UAT tasks explicitly exempt — tracked separately)
- [ ] Wave 0 covers all MISSING references (confirmed above: no new infra needed)
- [ ] No `cargo watch` / watch-mode flags in plans
- [ ] Feedback latency < 180s (including compose-smoke CI extension)
- [ ] `nyquist_compliant: true` set in frontmatter once planner fills the Per-Task Verification Map

**Approval:** pending
