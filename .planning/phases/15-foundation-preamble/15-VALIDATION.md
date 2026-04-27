---
phase: 15
slug: foundation-preamble
status: verified
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-25
audited: 2026-04-26
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
| 15-01-T1 | 15-01 | 1 | FOUND-15 | T-15-01-01 (accept) | `cronduit --version` reports `1.2.0` | smoke | `cargo build && ./target/debug/cronduit --version \| grep -q '1.2.0'` | ✅ existing binary | ✅ green |
| 15-02-T1 | 15-02 | 2 | FOUND-16 | T-15-02-02, T-15-02-03 | `just deny` recipe is invokable AND CI lint job step has `continue-on-error: true` (rc.1 warn-only posture; recipe exit code is intentionally surfaced, not zero-gated) | smoke + grep | `just --evaluate deny >/dev/null` (recipe defined) AND `grep -A2 'just deny' .github/workflows/ci.yml \| grep -q 'continue-on-error: true'` | ✅ recipe + CI step | ✅ green (substantive verification: SECURITY.md T-15-02-02 / T-15-02-03) |
| 15-02-T2 | 15-02 | 2 | FOUND-16 | T-15-02-01 | License allowlist matches v1.0/v1.1 posture (5 SPDX IDs) | unit (grep) | `grep -cE 'MIT\|Apache-2.0\|BSD-3-Clause\|ISC\|Unicode-DFS-2016' deny.toml` returns 5 | ✅ deny.toml:32-38 | ✅ green |
| 15-05-T1 | 15-05 | 5 | WH-02 / T-V12-WH-03 | T-15-04-01 (DoS via stalled receiver) | Scheduler keeps firing on time when worker is stalled (no drift > 1s) | integration | `cargo nextest run --test v12_webhook_scheduler_unblocked` | ✅ tests/v12_webhook_scheduler_unblocked.rs | ✅ green (5.016s) |
| 15-05-T2 | 15-05 | 5 | WH-02 / T-V12-WH-04 | T-15-04-01 (DoS via queue saturation) | Channel saturation increments drop counter, scheduler unaffected | integration | `cargo nextest run --test v12_webhook_queue_drop` | ✅ tests/v12_webhook_queue_drop.rs | ✅ green (2/2 passed) |
| 15-05-T3 | 15-05 | 5 | WH-02 / D-11 | T-15-03-02 (Operator alerting blind spot, Pitfall 3) | `cronduit_webhook_delivery_dropped_total` HELP/TYPE present at boot | integration | `cargo nextest run --test metrics_endpoint metrics_families_described_from_boot` | ✅ tests/metrics_endpoint.rs:73-78 | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Wave 0 (test scaffolding) MUST land before or alongside the wave-2 worker plans:

- [x] `tests/v12_webhook_scheduler_unblocked.rs` — covers WH-02 / T-V12-WH-03 (NEW file, 121 LOC). Landed in plan 15-05.
- [x] `tests/v12_webhook_queue_drop.rs` — covers WH-02 / T-V12-WH-04 (NEW file, 192 LOC). Landed in plan 15-05. The `channel_with_capacity(usize)` helper landed in `src/webhooks/worker.rs` as a public function (not `pub(crate)` / `#[cfg(test)]` gated — see plan 15-03).
- [x] Extension to `tests/metrics_endpoint.rs::metrics_families_described_from_boot` — two new HELP/TYPE assertions for `cronduit_webhook_delivery_dropped_total` at lines 73-78. Landed in plan 15-05.
- [x] No framework install needed — `cargo-nextest` is already wired in via `just nextest` and CI's `taiki-e/install-action@v2 with: tool: nextest,cargo-zigbuild`.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| The cargo-deny PR check appears in the `lint` job's status list as a row that shows yellow on advisory hits per `continue-on-error: true` | FOUND-16 | CI status surfaces are not directly assertable from the test runner; the row name and color require visual inspection of a real PR | After 15-02 lands on a feature branch, open the PR and confirm the `lint (fmt + clippy + openssl-sys guard)` job's check-run includes a `Run just deny` step that reports yellow on at least one warn-level finding (or green if the dep tree is clean). |
| Drop-counter overflow scenario (operator pushes > 1024 webhooks in a burst, observes the warn log + counter increment without scheduler stall) | WH-02 success criterion #4 | T-V12-WH-04 covers the unit-level claim with a smaller channel; the operator-observable behavior at the production capacity (1024) is worth one HUMAN-UAT entry to confirm the production sizing | Per `15-HUMAN-UAT.md` if the planner produces one. |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (3 file additions/extensions above — all landed in plan 15-05)
- [x] No watch-mode flags (cargo nextest is one-shot)
- [x] Feedback latency < 10s per-task / 120s per-wave (quick run: 5.017s; full nextest: ~90s)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** verified 2026-04-26

---

## Validation Audit 2026-04-26

| Metric | Count |
|--------|-------|
| Per-task rows | 6 |
| Initial COVERED | 5 |
| Initial PARTIAL (calibration) | 1 |
| Recalibrated | 1 |
| Final COVERED | 6 |
| Final MISSING / Manual-Only | 0 / 2 |
| Resolved | 1 (15-02-T1 row recalibrated to match rc.1 warn-only design intent) |
| Escalated | 0 |
| Run by | gsd-validate-phase orchestrator (inline recalibration, no auditor agent spawn needed) |

**Notes:**

- Row 15-02-T1 (`just deny`) initial assertion was "exits 0 (with warnings tolerated)" — calibrated for v1.2 final, not rc.1. Recalibrated to assert (a) recipe is defined AND (b) CI step has `continue-on-error: true`. The substantive rc.1 posture verification (advisories visible, license allowlist enforced, no job-level `continue-on-error`, no deprecated v0.19 keys) lives in `15-SECURITY.md` T-15-02-02 / T-15-02-03 / T-15-02-04. Phase 24 will flip this row's assertion back to "exits 0" when the gate-flip removes `continue-on-error: true` and resolves the advisory + license backlog.
- All 4 Wave 0 integration tests (`v12_webhook_scheduler_unblocked` × 1; `v12_webhook_queue_drop` × 2; `metrics_endpoint::metrics_families_described_from_boot` × 1) executed via `cargo nextest` and reported PASS in 5.017s combined.
- One Manual-Only row remains for the operator-scale "1024 webhooks in a burst" test, to be exercised against a running rc.1 build during `/gsd-verify-work`. This is by design — the unit test exercises the same property at unit-test resolution (channel capacity 4 + 20 events) per ROADMAP § Phase 15 Success Criteria #4.
