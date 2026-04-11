---
phase: 4
slug: docker-executor-container-network-differentiator
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-10
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + cargo nextest (integration tests gated by `--features integration`) |
| **Config file** | `Cargo.toml` (features: `integration`) |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo nextest run --all-features --profile ci` |
| **Estimated runtime** | ~30 seconds (unit), ~120 seconds (integration with testcontainers) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo nextest run --all-features --profile ci`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 04-01-01 | 01 | 1 | DOCKER-01 | — | N/A | unit | `cargo test docker::tests` | ❌ W0 | ⬜ pending |
| 04-01-02 | 01 | 1 | DOCKER-04 | — | Labels applied to all containers | unit | `cargo test docker::tests::labels` | ❌ W0 | ⬜ pending |
| 04-02-01 | 02 | 1 | DOCKER-05 | — | Terminal errors fail fast | unit | `cargo test docker::pull::tests` | ❌ W0 | ⬜ pending |
| 04-03-01 | 03 | 2 | DOCKER-02 | — | N/A | unit | `cargo test docker::network::tests` | ❌ W0 | ⬜ pending |
| 04-03-02 | 03 | 2 | DOCKER-03 | — | Structured pre-flight errors, no raw bollard errors | unit | `cargo test docker::network::tests::preflight` | ❌ W0 | ⬜ pending |
| 04-04-01 | 04 | 2 | DOCKER-06 | — | Exit code persisted before remove | unit | `cargo test docker::lifecycle::tests` | ❌ W0 | ⬜ pending |
| 04-04-02 | 04 | 2 | DOCKER-07 | — | Container labeling for tracking | unit | `cargo test docker::lifecycle::tests::labels` | ❌ W0 | ⬜ pending |
| 04-05-01 | 05 | 3 | SCHED-08 | — | Orphans cleaned on startup | integration | `cargo test --features integration orphan` | ❌ W0 | ⬜ pending |
| 04-06-01 | 06 | 3 | DOCKER-10 | — | container:\<name\> network verified end-to-end | integration | `cargo test --features integration container_network` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/scheduler/docker.rs` — Docker executor module skeleton
- [ ] Test fixtures for bollard mock responses (if unit testing without Docker)
- [ ] Integration test infrastructure using `testcontainers` (already in dev-dependencies from Phase 1)

*Existing test infrastructure (cargo test, nextest, testcontainers) covers framework requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| 100 repeated runs exit code reliability | DOCKER-09 | Stress test requires real Docker daemon | Run `cargo test --features integration -- fast_exit_reliability --ignored` with Docker running |

*All other phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
