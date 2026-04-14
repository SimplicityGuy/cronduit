---
phase: 4
slug: docker-executor-container-network-differentiator
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-10
audited: 2026-04-11
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + cargo nextest (integration tests gated by `#[ignore]` — run with `--ignored`) |
| **Config file** | `Cargo.toml` |
| **Quick run command** | `cargo test --lib && cargo test --test docker_config_behavior` |
| **Full suite command** | `cargo test --lib && cargo test --test docker_config_behavior && cargo test --test docker_executor -- --ignored && cargo test --test docker_container_network -- --ignored` |
| **Estimated runtime** | ~5 seconds (unit), ~120 seconds (integration with live Docker) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test --lib && cargo test --test docker_config_behavior`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 04-01-01 | 01 | 1 | DOCKER-01 | — | N/A | unit | `cargo test --lib scheduler::docker::tests` | ✅ | ✅ green |
| 04-01-02 | 01 | 1 | DOCKER-04 | — | Labels applied to all containers | unit | `cargo test --test docker_config_behavior -- docker_label_keys_match_orphan_reconciliation_filter docker_config_env_vars_format_as_key_value_strings docker_config_volumes_preserved_for_host_config docker_config_container_name_optional` | ✅ | ✅ green |
| 04-02-01 | 02 | 1 | DOCKER-05 | — | Terminal errors fail fast | unit | `cargo test --lib scheduler::docker_pull::tests` | ✅ | ✅ green |
| 04-03-01 | 03 | 2 | DOCKER-02 | — | N/A | unit | `cargo test --lib scheduler::docker_preflight::tests` | ✅ | ✅ green |
| 04-03-02 | 03 | 2 | DOCKER-03 | — | Structured pre-flight errors, no raw bollard errors | unit | `cargo test --lib scheduler::docker_preflight::tests::test_preflight_error_messages` | ✅ | ✅ green |
| 04-04-01 | 04 | 2 | DOCKER-06 | — | Exit code persisted before remove | unit | `cargo test --test docker_config_behavior -- docker_exec_result_carries_image_digest_for_db_storage` | ✅ | ✅ green |
| 04-04-02 | 04 | 2 | DOCKER-07 | — | Container labeling for tracking | unit | `cargo test --test docker_config_behavior -- docker_label_keys_match_orphan_reconciliation_filter docker_labels_do_not_contain_env_var_values` | ✅ | ✅ green |
| 04-05-01 | 05 | 3 | SCHED-08 | — | Orphans cleaned on startup | integration | `cargo test --test docker_executor -- test_docker_orphan_reconciliation --ignored` | ✅ | ✅ compiles; human at runtime |
| 04-06-01 | 06 | 3 | DOCKER-10 | — | container:\<name\> network verified end-to-end | integration | `cargo test --test docker_container_network -- test_container_network_mode --ignored` | ✅ | ✅ compiles; human at runtime |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `src/scheduler/docker.rs` — Docker executor module (419 lines, full lifecycle)
- [x] `src/scheduler/docker_log.rs` — Log streaming module
- [x] `src/scheduler/docker_pull.rs` — Image pull with retry/classification
- [x] `src/scheduler/docker_preflight.rs` — Network pre-flight validation
- [x] `src/scheduler/docker_orphan.rs` — Orphan reconciliation
- [x] Integration test infrastructure using `testcontainers` (dev-dependency)
- [x] `tests/docker_config_behavior.rs` — Behavioral unit tests (no Docker required)
- [x] `tests/docker_executor.rs` — 5 integration tests (require Docker)
- [x] `tests/docker_container_network.rs` — 2 integration tests (require Docker, marquee DOCKER-10)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| container:\<name\> network end-to-end | DOCKER-10 | Requires running Docker daemon | Run `cargo test --test docker_container_network -- --ignored --nocapture` with Docker running |
| Full executor lifecycle (echo, timeout, orphan) | DOCKER-01, DOCKER-06, SCHED-08 | Requires running Docker daemon | Run `cargo test --test docker_executor -- --ignored --nocapture` with Docker running |
| 100 repeated runs exit code reliability | DOCKER-09 | Stress test requires real Docker daemon | Run `cargo test --test docker_executor -- test_docker_basic_echo --ignored` repeatedly with Docker running |

*All behavioral properties that can be unit-tested have automated verification.*

---

## Nyquist Audit Results (2026-04-11)

**Auditor:** gsd-validate-phase agent

### Gaps Found and Resolved

| Gap | Root Cause | Resolution |
|-----|------------|------------|
| VALIDATION.md commands referenced non-existent module paths (`docker::lifecycle::tests`, `docker::network::tests`) | Original validation map was aspirational before implementation | Updated commands to match actual test paths |
| DOCKER-04 (env vars, volumes, container_name) had no dedicated behavioral test | Covered structurally by config deserialization but no assertion on formatting behavior | Added `docker_config_env_vars_format_as_key_value_strings`, `docker_config_volumes_preserved_for_host_config`, `docker_config_container_name_optional` in `tests/docker_config_behavior.rs` |
| DOCKER-07 (label key contracts) had no unit test verifying the exact label strings | Labels built inside `execute_docker` body, not extracted to a testable helper | Added `docker_label_keys_match_orphan_reconciliation_filter` and `docker_labels_do_not_contain_env_var_values` |
| DOCKER-06 (image_digest field for DB storage) had no unit test for the struct contract | `DockerExecResult.image_digest` is the critical field written to `job_runs.container_id` | Added `docker_exec_result_carries_image_digest_for_db_storage` |
| SCHED-08 / DOCKER-10 commands used `--features integration` but tests use `#[ignore]` | Mismatch between planned feature gate and implemented ignore gate | Updated commands to use `-- --ignored` pattern |

### Test File Created

`tests/docker_config_behavior.rs` — 11 behavioral unit tests, no Docker required, all green.

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or manual-only justification
- [x] Sampling continuity: no consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s for unit tests
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** nyquist_audit_complete — 2026-04-11
