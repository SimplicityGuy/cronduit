---
phase: 2
slug: scheduler-core-command-script-executor
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-10
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + cargo nextest |
| **Config file** | `.config/nextest.toml` (if exists) or default |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo nextest run --all-features` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo nextest run --all-features`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | SCHED-01 | — | N/A | unit | `cargo test scheduler::tests` | ❌ W0 | ⬜ pending |
| 02-01-02 | 01 | 1 | SCHED-02 | — | N/A | unit | `cargo test scheduler::tests::dst` | ❌ W0 | ⬜ pending |
| 02-01-03 | 01 | 1 | SCHED-03 | — | N/A | unit | `cargo test scheduler::tests::clock_jump` | ❌ W0 | ⬜ pending |
| 02-02-01 | 02 | 1 | EXEC-01 | — | N/A | unit | `cargo test executor::tests::command` | ❌ W0 | ⬜ pending |
| 02-02-02 | 02 | 1 | EXEC-02 | — | N/A | unit | `cargo test executor::tests::script` | ❌ W0 | ⬜ pending |
| 02-03-01 | 03 | 1 | EXEC-04 | — | N/A | unit | `cargo test log_pipeline::tests` | ❌ W0 | ⬜ pending |
| 02-03-02 | 03 | 1 | EXEC-05 | — | N/A | unit | `cargo test log_pipeline::tests::truncation` | ❌ W0 | ⬜ pending |
| 02-04-01 | 04 | 2 | SCHED-07 | — | N/A | integration | `cargo test --test shutdown` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Test module stubs for scheduler, executor, log_pipeline
- [ ] Test helpers/fixtures for mock job configs and DB setup
- [ ] Integration test harness for shutdown testing

*Existing Phase 1 infrastructure (sqlx test DB, CI pipeline) covers base requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Second SIGTERM immediate kill | SCHED-07 | Signal delivery timing is OS-dependent | Send SIGINT, wait 1s, send SIGTERM — verify immediate exit |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
