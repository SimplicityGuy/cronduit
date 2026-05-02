---
phase: 21
slug: failure-context-ui-panel-exit-code-histogram-card-rc-2
status: draft
nyquist_compliant: false
wave_0_complete: true
created: 2026-05-01
---

# Phase 21 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (preferred: `cargo nextest run`) |
| **Config file** | `Cargo.toml` (`[dev-dependencies]`); `tests/` for integration; in-module `#[cfg(test)] mod tests` for unit |
| **Quick run command** | `just test-fast` (project-defined) or `cargo nextest run --no-fail-fast --partition count:1/4` |
| **Full suite command** | `just ci-test` or `cargo nextest run --all-features` |
| **Estimated runtime** | ~60–90s on amd64 (sqlite); ~120–180s with postgres feature |

---

## Sampling Rate

- **After every task commit:** Run targeted `cargo nextest run -E 'test(/v12_fctx_panel|v12_exit_histogram|v12_fctx_explain|exit_buckets/)'` (≤30s)
- **After every plan wave:** Run `cargo nextest run --all-features` (full suite)
- **Before `/gsd-verify-work`:** Full suite green on both sqlite and postgres feature sets
- **Max feedback latency:** 30 seconds for targeted run; 180 seconds for full suite

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 21-01-01 | 01 | 1 | FCTX-06 | — | Migration is one-file additive; legacy NULL preserved | integration | `cargo nextest run -E 'test(/migration/)'` | ✅ existing | ⬜ pending |
| 21-02-01 | 02 | 2 | FCTX-06 | — | Scheduler writes scheduled_for at fire-decision; trigger-aware | integration | `cargo nextest run -E 'test(/v12_fctx_panel/)'` | ❌ W0 | ⬜ pending |
| 21-03-01 | 03 | 2 | EXIT-01..05 | — | Status-discriminator-wins classifier; success excluded | unit | `cargo nextest run -E 'test(/exit_buckets::tests/)'` | ❌ W0 (in-module) | ⬜ pending |
| 21-04-01 | 04 | 2 | FCTX-01..03,05 | — | Soft-fail on DB error; askama escaping | integration | `cargo nextest run -E 'test(/v12_fctx_panel/)'` | ❌ W0 | ⬜ pending |
| 21-05-01 | 05 | 2 | EXIT-01..05 | — | Histogram aggregator wired; below-N=5 empty state | integration | `cargo nextest run -E 'test(/v12_exit_histogram/)'` | ❌ W0 | ⬜ pending |
| 21-06-01 | 06 | 2 | FCTX-01..03,05,06 + EXIT-01..05 | — | UI-SPEC class names + auto-escape; print-mode opens panel | integration | `cargo nextest run -E 'test(/v12_fctx_panel|v12_exit_histogram/)'` | ❌ W0 | ⬜ pending |
| 21-07-01 | 07 | 3 | FCTX-01..03,05,06 | — | All 5 row gates; never-succeeded; soft-fail | integration | `cargo nextest run -E 'test(/v12_fctx_panel/)'` | ❌ W0 | ⬜ pending |
| 21-08-01 | 08 | 3 | EXIT-01..05 | — | 10-bucket coverage; 137 dual-classifier; success-rate formula | integration | `cargo nextest run -E 'test(/v12_exit_histogram/)'` | ❌ W0 | ⬜ pending |
| 21-09-01 | 09 | 3 | FCTX-06 | — | scheduled_for column doesn't shift index plans | integration | `cargo nextest run -E 'test(/v12_fctx_explain/)'` | ✅ extend P16 | ⬜ pending |
| 21-10-01 | 10 | 3 | All | — | just recipes runnable; recipe-calls-recipe pattern | manual | `just --list \| grep uat-fctx-panel\|uat-exit-histogram\|uat-fire-skew` | ❌ W0 | ⬜ pending |
| 21-11-01 | 11 | 4 | All | — | Maintainer UAT; rc.2 tag cut runbook | manual | autonomous=false (maintainer-validated) | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

*Per project memory `feedback_uat_user_validates.md`: maintainer validates Wave 4 manually; Claude does NOT mark UAT passed.*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements:
- `cargo test` / `cargo nextest` already wired in CI matrix (`linux/{amd64,arm64} × {SQLite, Postgres}`)
- `tests/v12_fctx_explain.rs` exists from P16 (extended in Wave 3)
- `tests/v12_fctx_*.rs` fixture/seed helper precedent from P16

New test files created during execution (Wave 2/3, NOT Wave 0):
- [ ] `tests/v12_fctx_panel.rs` — covers FCTX-01..03,05,06 (Wave 3)
- [ ] `tests/v12_exit_histogram.rs` — covers EXIT-01..05 (Wave 3)
- [ ] In-module `#[cfg(test)] mod tests` in `src/web/exit_buckets.rs` — covers `categorize` 10-bucket coverage (Wave 2)

*Existing `tests/v12_fctx_explain.rs` is extended in Wave 3, not rewritten.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Print-mode opens FCTX panel | FCTX-01 (UI-SPEC § Interaction) | `@media print` requires real print preview | `just dev` → load failed-run-detail page → browser print preview → confirm panel expanded |
| Mobile viewport stacking (panel rows 1-col below 640px; histogram horizontal scroll) | FCTX/EXIT (UI-SPEC § Layout) | Real viewport rendering | `just uat-fctx-a11y` (umbrella recipe per research D-7) |
| Light-mode rendering | FCTX/EXIT (UI-SPEC § Color) | Real prefers-color-scheme toggle | `just uat-fctx-a11y` |
| Keyboard-only navigation (Tab + Space/Enter on summary; Tab onto bars; tooltip on focus) | FCTX/EXIT (UI-SPEC § Accessibility) | Real keyboard interaction | `just uat-fctx-a11y` |
| `v1.2.0-rc.2` tag cut + GHCR image publication | rc.2 tag cut commitment | Maintainer-only release-engineering action | `21-RC2-PREFLIGHT.md` (autonomous=false; reuses `docs/release-rc.md` verbatim) |
| `:latest` GHCR tag stays at v1.1.0 after rc.2 push | rc.2 tag cut commitment | GHCR API check after push | Maintainer verifies post-tag |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (no Wave 0 plans needed — infrastructure exists)
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s for targeted; < 180s for full
- [ ] `nyquist_compliant: true` set in frontmatter (after planner finalizes)

**Approval:** pending
