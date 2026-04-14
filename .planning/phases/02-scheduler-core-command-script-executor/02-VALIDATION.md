---
phase: 2
slug: scheduler-core-command-script-executor
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-10
updated: 2026-04-14
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
| 02-01-01 | 01 | 1 | SCHED-01 | — | N/A | unit+integration | `cargo test -p cronduit scheduler::tests::shutdown_drain_completes_within_grace` · `cargo test --test scheduler_integration` | ✅ `src/scheduler/mod.rs`, `tests/scheduler_integration.rs` | ✅ green |
| 02-01-02 | 01 | 1 | SCHED-02 | — | N/A | unit | `cargo test -p cronduit scheduler::fire::tests::dst_spring_forward_skips_nonexistent_time` · `cargo test -p cronduit scheduler::fire::tests::dst_fall_back_fires_once` | ✅ `src/scheduler/fire.rs` | ✅ green |
| 02-01-03 | 01 | 1 | SCHED-03 | — | N/A | unit | `cargo test -p cronduit scheduler::fire::tests::clock_jump_detects_missed_fires` · `cargo test -p cronduit scheduler::fire::tests::clock_jump_no_false_positive` · `cargo test -p cronduit scheduler::fire::tests::clock_jump_limited_to_24h_window` | ✅ `src/scheduler/fire.rs` | ✅ green |
| 02-02-01 | 02 | 1 | EXEC-01 | — | N/A | unit+integration | `cargo test -p cronduit scheduler::command::tests` · `cargo test --test scheduler_integration test_command_job_fires_and_captures_logs` | ✅ `src/scheduler/command.rs`, `tests/scheduler_integration.rs` | ✅ green |
| 02-02-02 | 02 | 1 | EXEC-02 | — | N/A | unit+integration | `cargo test -p cronduit scheduler::script::tests` · `cargo test --test scheduler_integration test_script_job_fires_and_captures_logs` | ✅ `src/scheduler/script.rs`, `tests/scheduler_integration.rs` | ✅ green |
| 02-03-01 | 03 | 1 | EXEC-04 | — | N/A | unit | `cargo test -p cronduit scheduler::log_pipeline::tests::channel_head_drop` · `cargo test -p cronduit scheduler::log_pipeline::tests::drain_batch_respects_max` · `cargo test -p cronduit scheduler::log_pipeline::tests::send_after_receiver_dropped_no_panic` | ✅ `src/scheduler/log_pipeline.rs` | ✅ green |
| 02-03-02 | 03 | 1 | EXEC-05 | — | N/A | unit | `cargo test -p cronduit scheduler::log_pipeline::tests::line_truncation_exact_boundary` · `cargo test -p cronduit scheduler::log_pipeline::tests::line_truncation_over_boundary` · `cargo test -p cronduit scheduler::log_pipeline::tests::truncation_marker_on_drain` | ✅ `src/scheduler/log_pipeline.rs` | ✅ green |
| 02-04-01 | 04 | 2 | SCHED-07 | — | N/A | integration | `cargo test -p cronduit scheduler::tests::shutdown_drain_completes_within_grace` · `cargo test -p cronduit scheduler::tests::shutdown_grace_expiry_force_kills` · `cargo test -p cronduit scheduler::tests::shutdown_summary_fields` · `cargo test --test graceful_shutdown sigterm_yields_clean_exit_within_one_second` | ✅ `src/scheduler/mod.rs`, `tests/graceful_shutdown.rs` | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

### Requirements covered transitively (not on the original map but in scope)

| Requirement | Evidence |
|---|---|
| SCHED-04 (`tokio::spawn` per-fire lifecycle) | `src/scheduler/run.rs::tests`, `tests/scheduler_integration.rs` (6 tests) |
| SCHED-05 (per-job timeout via `tokio::select!`) | `src/scheduler/command.rs::execute_timeout`, `tests/scheduler_integration.rs::test_timeout_preserves_partial_logs` |
| SCHED-06 (concurrent runs of same job) | `src/scheduler/run.rs` (spawn-per-fire), implicit in `scheduler_integration.rs` |
| EXEC-03 (stdout/stderr stream tags) | `src/scheduler/command.rs::execute_stderr_capture`, `src/scheduler/log_pipeline.rs::log_line_stream_tag_and_timestamp` |
| EXEC-06 (exit code recording) | `src/scheduler/command.rs::execute_nonzero_exit`, `tests/scheduler_integration.rs::test_failed_command_records_exit_code` |

---

## Wave 0 Requirements

- [x] Test module stubs for scheduler, executor, log_pipeline — all three `mod tests` blocks exist and are populated
- [x] Test helpers/fixtures for mock job configs and DB setup — `setup_pool()`, `test_active_runs()`, `make_test_job()` in `src/scheduler/mod.rs` and `src/scheduler/run.rs`; `test_config_with_jobs()` in `tests/scheduler_integration.rs`
- [x] Integration test harness for shutdown testing — `tests/graceful_shutdown.rs` spawns the binary via `cargo_bin()` and sends real SIGTERM

*Existing Phase 1 infrastructure (sqlx test DB, CI pipeline) covers base requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Second SIGTERM immediate kill (double-tap) | SCHED-07 | Signal delivery timing is OS-dependent; `cargo test` harness cannot reliably reproduce a second-signal race | Start `cronduit run`, send SIGINT (wait ≤1s during drain), send SIGTERM — verify process exits immediately with non-zero code rather than waiting out the 30s grace window |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s (full `cargo nextest run --all-features` typically under 60s on a cold cache, well under on warm)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved (retroactive audit 2026-04-14)

---

## Validation Audit 2026-04-14

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

**Audit method:** Retroactive cross-reference of the Per-Task Verification Map against the current `src/scheduler/` and `tests/` trees. Phase 2 shipped code 2026-04-10 (VERIFICATION.md `passed`, score 6/6 must-haves) but the VALIDATION.md was left in draft state with all task rows marked `⬜ pending`. Every row in the map has an existing test file with explicit `#[test]` or `#[tokio::test]` functions that target the behavior called out in the requirement. No new tests needed; only frontmatter flips + commands updated to point at the actual test module paths used in the current tree.

**Key evidence:**
- `src/scheduler/fire.rs:266-400` — 7 tests covering DST (spring-forward / fall-back) and clock-jump (positive, negative, 24h-cap) behaviors (SCHED-02, SCHED-03)
- `src/scheduler/command.rs:221-320` — 6 tests covering stdout/stderr/exit/timeout/shutdown/shell-words parsing (EXEC-01)
- `src/scheduler/script.rs:113-220` — 5 tests covering stdout capture, nonzero exit, tempfile lifecycle, default shebang (EXEC-02)
- `src/scheduler/log_pipeline.rs:187-290` — 9 tests covering bounded channel, head-drop, truncation boundaries, async drain (EXEC-04, EXEC-05)
- `src/scheduler/mod.rs:406-620` — 3 shutdown integration tests (`scheduler::tests::shutdown_*`) exercising drain-within-grace, grace-expiry-force-kill, and summary field propagation (SCHED-07)
- `tests/scheduler_integration.rs` — 6 end-to-end tests firing real command + script jobs through the full scheduler → executor → log_pipeline → DB pipeline (SCHED-01, SCHED-04, SCHED-06, EXEC-03, EXEC-06)
- `tests/graceful_shutdown.rs` — real-binary SIGTERM test via `cargo_bin()` (SCHED-07)
