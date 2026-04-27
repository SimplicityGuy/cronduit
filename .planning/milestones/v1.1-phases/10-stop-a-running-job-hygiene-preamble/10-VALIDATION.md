---
phase: 10
slug: stop-a-running-job-hygiene-preamble
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-15
---

# Phase 10 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution. Populated from the Validation Architecture section of `10-RESEARCH.md`. The planner must fill the Per-Task Verification Map after PLAN.md files are written.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` + `cargo-nextest` (workspace default) |
| **Config file** | `.config/nextest.toml` (if added in Wave 0) |
| **Quick run command** | `cargo nextest run --package cronduit --lib` |
| **Full suite command** | `cargo nextest run --all-features --profile ci` |
| **Estimated runtime** | ~90s unit / ~180s full (race test dominates) |

---

## Sampling Rate

- **After every task commit:** Run `cargo nextest run --package cronduit --lib`
- **After every plan wave:** Run `cargo nextest run --all-features --profile ci`
- **Before `/gsd-verify-work`:** Full suite must be green (including the 1000-iteration race test T-V11-STOP-04)
- **Max feedback latency:** 90 seconds for unit tier

---

## Per-Task Verification Map

*Populated by the planner after PLAN.md files exist. Must contain one row per task and reference T-V11-STOP-01..16 test IDs from 10-RESEARCH.md.*

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 10-XX-XX | XX | N | REQ-XX | T-V11-STOP-NN / — | {secure behavior} | unit / integration | `cargo nextest run ...` | ✅ / ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Enumerated from 10-RESEARCH.md §Validation Architecture — Wave 0 gap list. The planner must create PLAN.md tasks covering each of these BEFORE any Stop feature code lands.

- [ ] `tests/scheduler/stop_race.rs` — new integration test module for T-V11-STOP-04..06 (merged `RunEntry` lifecycle race tests using `#[tokio::test(start_paused = true)]` with a 1000-iteration loop)
- [ ] `tests/scheduler/stop_executors.rs` — new integration test module for T-V11-STOP-09..11 (command / script / docker Stop round-trip)
- [ ] `tests/scheduler/process_group_kill.rs` — new integration test module for T-V11-STOP-07..08 (regression lock for `.process_group(0)` + `libc::kill(-pid, SIGKILL)`)
- [ ] `tests/scheduler/docker_orphan_guard.rs` — new integration test module for T-V11-STOP-12..14 (`WHERE status = 'running'` guard regression lock on SQLite + Postgres)
- [ ] `src/scheduler/control.rs` unit test block — new file with `RunControl` / `StopReason` unit tests (T-V11-STOP-01..03)
- [ ] `tests/web/stop_handler.rs` — new integration test module for T-V11-STOP-15..16 (`POST /api/runs/{run_id}/stop` handler including CSRF path, 503 shutdown path, race-case silent refresh)
- [ ] `.config/nextest.toml` — add `ci` profile with serial group for the docker integration tests if not present
- [ ] Fixture helpers in `tests/common/mod.rs` for creating throwaway `RunEntry` map state without a full scheduler spin-up

*If all Wave 0 items already exist, mark `wave_0_complete: true`.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Stop button visual placement + hover tint on run-detail page | SCHED-14, 10-UI-SPEC | Interaction contract; Playwright not in stack | Start dev server, navigate to a running job's detail page, verify the Stop button appears in the header action slot with neutral outline, hover tints toward `--cd-status-stopped` |
| `stopped` badge renders correctly on dashboard, run-history, and run-detail pages for all three executors | SCHED-09, 10-UI-SPEC | Cross-page visual verification | After stopping a command, script, and docker run, open dashboard + run-history + run-detail; confirm slate badge with "STOPPED" label on each |
| Race-case silent refresh (no toast) | D-07 | UI principle verification | Stop a run while it naturally finalizes (requires timing); confirm the page refreshes and shows the natural terminal status with no toast |
| `cronduit --version` reports `1.1.0` | FOUND-13 | CLI output inspection | Build binary, run `./target/debug/cronduit --version`, verify output is exactly `1.1.0` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (8 items above)
- [ ] No watch-mode flags (no `cargo watch` in task commands)
- [ ] Feedback latency < 90s for unit tier
- [ ] `nyquist_compliant: true` set in frontmatter after planner maps every task

**Approval:** pending
