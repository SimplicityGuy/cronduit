---
phase: 15
slug: foundation-preamble
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-25
---

# Phase 15 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: `15-RESEARCH.md` § Validation Architecture (lines 719-789).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo nextest` 0.x via `just nextest` (CI gate); plain `cargo test` for local iteration |
| **Config file** | `.config/nextest.toml` (existing — no changes needed for P15) |
| **Quick run command** | `cargo nextest run --test v12_webhook_scheduler_unblocked --test v12_webhook_queue_drop --test metrics_endpoint` |
| **Full suite command** | `just nextest` (== `cargo nextest run --all-features --profile ci`) |
| **Estimated runtime** | Quick: ~5s (3 integration tests); Full: ~90s (existing project baseline) |

---

## Sampling Rate

- **After every task commit:** Run quick command relevant to the task:
  - Plan 15-01 (Cargo bump): `cargo build && ./target/debug/cronduit --version | grep -q '1.2.0'`
  - Plan 15-02 (cargo-deny): `just deny`
  - Plan 15-03..N (worker): `cargo nextest run --test v12_webhook_scheduler_unblocked --test v12_webhook_queue_drop --test metrics_endpoint`
- **After every plan wave:** Run `just ci` (full local CI shadow: fmt-check + clippy + openssl-check + nextest + grep-no-percentile-cont + new `just deny`).
- **Before `/gsd-verify-work`:** Full suite must be green; cargo-deny step appears in PR check list; `cronduit --version` confirmed `1.2.0`.
- **Max feedback latency:** ~10 seconds (per-task) / ~120 seconds (per-wave).

---

## Per-Task Verification Map

> Filled by the planner during plan generation. Each row maps a plan task to a Wave-0 test file or extension.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| _planner-fills_ | 15-01 | 1 | FOUND-15 | — | `cronduit --version` reports `1.2.0` | smoke | `cargo build && ./target/debug/cronduit --version \| grep -q '1.2.0'` | ⬜ pending | ⬜ pending |
| _planner-fills_ | 15-02 | 1 | FOUND-16 | — | `just deny` exits 0 (with warnings tolerated) | smoke | `just deny` | ⬜ pending | ⬜ pending |
| _planner-fills_ | 15-02 | 1 | FOUND-16 | — | License allowlist matches v1.0/v1.1 posture (5 SPDX IDs) | unit (grep) | `grep -E 'MIT\|Apache-2.0\|BSD-3-Clause\|ISC\|Unicode-DFS-2016' deny.toml` returns 5 | ⬜ pending | ⬜ pending |
| _planner-fills_ | 15-03..N | 2 | WH-02 / T-V12-WH-03 | DoS via stalled receiver | Scheduler keeps firing on time when worker is stalled (no drift > 1s) | integration | `cargo nextest run --test v12_webhook_scheduler_unblocked` | ❌ W0 (NEW) | ⬜ pending |
| _planner-fills_ | 15-03..N | 2 | WH-02 / T-V12-WH-04 | DoS via queue saturation | Channel saturation increments drop counter, scheduler unaffected | integration | `cargo nextest run --test v12_webhook_queue_drop` | ❌ W0 (NEW) | ⬜ pending |
| _planner-fills_ | 15-03..N | 2 | WH-02 / D-11 | Operator alerting blind spot | `cronduit_webhook_delivery_dropped_total` HELP/TYPE present at boot | integration | extend `tests/metrics_endpoint.rs::metrics_families_described_from_boot` | ✅ EXTEND | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Wave 0 (test scaffolding) MUST land before or alongside the wave-2 worker plans:

- [ ] `tests/v12_webhook_scheduler_unblocked.rs` — covers WH-02 / T-V12-WH-03 (NEW file, ~100 LOC). See RESEARCH.md § Validation Architecture for the input setup, observed signal, and assertion threshold.
- [ ] `tests/v12_webhook_queue_drop.rs` — covers WH-02 / T-V12-WH-04 (NEW file, ~80 LOC). Requires test helper `pub fn channel_with_capacity(cap: usize)` in `src/webhooks/worker.rs` (`pub(crate)` or `#[cfg(test)]` gated) so the integration test can construct a smaller channel for saturation testing (1024 events is impractical to push synchronously).
- [ ] Extension to `tests/metrics_endpoint.rs::metrics_families_described_from_boot` — two new assertions for the drop counter (existing file, ~10 LOC).
- [ ] No framework install needed — `cargo-nextest` is already wired in via `just nextest` and CI's `taiki-e/install-action@v2 with: tool: nextest,cargo-zigbuild`.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| The cargo-deny PR check appears in the `lint` job's status list as a row that shows yellow on advisory hits per `continue-on-error: true` | FOUND-16 | CI status surfaces are not directly assertable from the test runner; the row name and color require visual inspection of a real PR | After 15-02 lands on a feature branch, open the PR and confirm the `lint (fmt + clippy + openssl-sys guard)` job's check-run includes a `Run just deny` step that reports yellow on at least one warn-level finding (or green if the dep tree is clean). |
| Drop-counter overflow scenario (operator pushes > 1024 webhooks in a burst, observes the warn log + counter increment without scheduler stall) | WH-02 success criterion #4 | T-V12-WH-04 covers the unit-level claim with a smaller channel; the operator-observable behavior at the production capacity (1024) is worth one HUMAN-UAT entry to confirm the production sizing | Per `15-HUMAN-UAT.md` if the planner produces one. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (3 file additions/extensions above)
- [ ] No watch-mode flags (cargo nextest is one-shot)
- [ ] Feedback latency < 10s per-task / 120s per-wave
- [ ] `nyquist_compliant: true` set in frontmatter once planner fills the per-task map and plans land

**Approval:** pending
