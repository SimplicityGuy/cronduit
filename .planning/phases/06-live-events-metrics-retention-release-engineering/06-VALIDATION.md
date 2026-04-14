---
phase: 6
slug: live-events-metrics-retention-release-engineering
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-12
updated: 2026-04-14
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + cargo nextest |
| **Config file** | `Cargo.toml` (test features), `.config/nextest.toml` |
| **Quick run command** | `just test` |
| **Full suite command** | `just test-all` |
| **Estimated runtime** | ~30 seconds (unit), ~120 seconds (integration) |

---

## Sampling Rate

- **After every task commit:** Run `just test`
- **After every plan wave:** Run `just test-all`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 06-01-01 | 01 | 1 | UI-14 | T-6-01 | SSE stream drops slow subscribers without blocking DB writer | integration | `cargo test --test sse_streaming` (4 tests: `sse_active_run_streams_log_lines`, `sse_completed_run_returns_immediate_close`, `sse_slow_subscriber_gets_skip_marker`, `sse_stream_closes_on_run_finalize`) | ✅ `tests/sse_streaming.rs` | ✅ green |
| 06-02-01 | 02 | 1 | OPS-02 | — | Metrics endpoint returns valid Prometheus text format (families described from boot) | integration | `cargo test --test metrics_endpoint metrics_families_described_from_boot metrics_endpoint_returns_prometheus_format` | ✅ `tests/metrics_endpoint.rs` | ✅ green |
| 06-02-02 | 02 | 1 | OPS-02 | — | Failure reason labels are bounded closed enum | unit | `cargo test --test metrics_endpoint failure_reason_labels_are_bounded_enum failure_reason_classification_covers_known_errors` | ✅ `tests/metrics_endpoint.rs` | ✅ green |
| 06-03-01 | 03 | 2 | DB-08 | — | Retention pruner deletes in batches + startup log + WAL checkpoint + cancellation | integration | `cargo test --test retention_integration` (6 tests incl. `retention_pruner_emits_startup_log_on_spawn`, `retention_deletes_old_logs_in_batches`, `retention_deletes_runs_after_logs_removed`, `retention_respects_cutoff_date`, `retention_wal_checkpoint_fires_after_threshold`, `retention_cancellation_stops_prune`) | ✅ `tests/retention_integration.rs` | ✅ green |
| 06-04-01 | 04 | 2 | OPS-04 | — | docker-compose.yml works for quickstart (4 jobs reach success within 120s) | CI integration | `.github/workflows/ci.yml` compose-smoke matrix job (Phase 8 criterion 4; Phase 7 Plan 01 override accepted by operator for `ports:` deviation) | ✅ ci.yml:133-360 | ✅ green (via CI) |
| 06-04-02 | 04 | 2 | OPS-05 | T-6-02 | THREAT_MODEL.md covers all four threat models | manual | Document review | N/A | ✅ manual (documented in THREAT_MODEL.md) |
| 06-05-01 | 05 | 3 | OPS-04 | — | Multi-arch Docker image builds on CI | CI integration | `.github/workflows/release.yml` buildx matrix (amd64 + arm64 via cargo-zigbuild, no QEMU) | ✅ release.yml | ✅ green (via CI) |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] SSE integration test infrastructure — `tests/sse_streaming.rs` present with 4 tests covering active/completed/slow-subscriber/finalize paths
- [x] Metrics endpoint integration infrastructure — `tests/metrics_endpoint.rs` present with 4 tests covering boot-describe, prometheus format, bounded enum labels, failure reason classification
- [x] Retention pruner integration infrastructure — `tests/retention_integration.rs` present with 6 tests covering startup log, batched deletes, runs-after-logs ordering, cutoff, WAL checkpoint, cancellation

*Existing test infrastructure from Phases 1-5 covers framework and fixtures.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Quickstart under 5 minutes | OPS-04 | End-to-end user experience timing | Clone repo, `docker compose up`, open browser, verify job runs |
| THREAT_MODEL.md completeness | OPS-05 | Document quality review | Review all four threat models for accuracy and completeness |
| README structure and clarity | OPS-04 | Subjective content quality | Read as a stranger; verify SECURITY first, quickstart works |
| SSE live log visual experience | UI-14 | Visual/UX verification | Open Run Detail during active run, verify live streaming |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s (unit suite); integration tier < 120s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved (retroactive audit 2026-04-14)

---

## Validation Audit 2026-04-14

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

**Audit method:** Retroactive cross-reference against current `tests/` tree. Phase 6 shipped code 2026-04-12; VERIFICATION.md status `passed` with 1 operator-accepted override (OPS-04 `ports:` deviation). All three test files called out in the original Per-Task Map exist and are populated beyond the minimums — Phase 7 GAP-1 (metrics families described from boot) and GAP-2 (retention pruner startup log on `cronduit.retention` target) added test coverage for regressions caught during Phase 6 UAT.

**Key evidence:**
- `tests/sse_streaming.rs` — 4 tests: `sse_active_run_streams_log_lines`, `sse_completed_run_returns_immediate_close`, `sse_slow_subscriber_gets_skip_marker` (T-6-01 secure behavior), `sse_stream_closes_on_run_finalize`
- `tests/metrics_endpoint.rs` — 4 tests: `metrics_families_described_from_boot` (Phase 7 GAP-1), `metrics_endpoint_returns_prometheus_format`, `failure_reason_labels_are_bounded_enum` (bounded-cardinality OPS-02 requirement), `failure_reason_classification_covers_known_errors`
- `tests/retention_integration.rs` — 6 tests: `retention_pruner_emits_startup_log_on_spawn` (Phase 7 GAP-2), `retention_deletes_old_logs_in_batches`, `retention_deletes_runs_after_logs_removed`, `retention_respects_cutoff_date`, `retention_wal_checkpoint_fires_after_threshold`, `retention_cancellation_stops_prune`
- `src/scheduler/run.rs` — `FailureReason` closed enum (grep-confirmed) feeds the bounded-cardinality metrics label
- `.github/workflows/ci.yml:133-360` — compose-smoke matrix asserts OPS-04 quickstart E2E on both `docker-compose.yml` and `docker-compose.secure.yml`
- `.github/workflows/release.yml` — multi-arch (amd64 + arm64) buildx matrix via cargo-zigbuild verifies the OPS-04 release-engineering criterion

**Manual-only items retained as legitimate:** README/THREAT_MODEL document-review items and the visual SSE UX verification — these are prose/UX quality gates, not code-testable behaviors. The OPS-05 quickstart timing verification was covered by the Phase 8 walkthrough (see v1.0 milestone audit — OPS-05 partial pending the orchestrator's verbal-approval decision).
