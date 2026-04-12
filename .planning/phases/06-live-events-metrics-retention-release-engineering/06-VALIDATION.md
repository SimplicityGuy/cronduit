---
phase: 6
slug: live-events-metrics-retention-release-engineering
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-12
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
| 06-01-01 | 01 | 1 | UI-14 | T-6-01 / — | SSE stream drops slow subscribers without blocking DB writer | integration | `cargo test sse_` | ❌ W0 | ⬜ pending |
| 06-02-01 | 02 | 1 | OPS-02 | — | Metrics endpoint returns valid Prometheus text format | integration | `cargo test metrics_` | ❌ W0 | ⬜ pending |
| 06-02-02 | 02 | 1 | OPS-02 | — | Failure reason labels are bounded closed enum (6 values) | unit | `cargo test metrics_labels` | ❌ W0 | ⬜ pending |
| 06-03-01 | 03 | 2 | DB-08 | — | Retention pruner deletes in batches, no write contention | integration | `cargo test retention_` | ❌ W0 | ⬜ pending |
| 06-04-01 | 04 | 2 | OPS-04 | — | docker-compose.yml works for quickstart | manual | N/A | N/A | ⬜ pending |
| 06-04-02 | 04 | 2 | OPS-05 | T-6-02 / — | THREAT_MODEL.md covers all four threat models | manual | N/A | N/A | ⬜ pending |
| 06-05-01 | 05 | 3 | OPS-04 | — | Multi-arch Docker image builds on CI | integration | `just image` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] SSE integration test stubs for UI-14
- [ ] Metrics endpoint test stubs for OPS-02
- [ ] Retention pruner test stubs for DB-08

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

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
