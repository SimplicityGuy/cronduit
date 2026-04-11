---
phase: 5
slug: config-reload-random-resolver
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-11
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) + cargo-nextest (CI) |
| **Config file** | none — standard Cargo test infrastructure |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo nextest run --all-features` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo nextest run --all-features`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 05-01-01 | 01 | 1 | RELOAD-01 | T-05-01 | SIGHUP handler validates sender | integration | `cargo test --test reload_sighup` | ❌ W0 | ⬜ pending |
| 05-01-02 | 01 | 1 | RELOAD-02 | T-05-02 | CSRF protection on POST | unit | `cargo test scheduler::reload::tests::api_reload` | ❌ W0 | ⬜ pending |
| 05-01-03 | 01 | 1 | RELOAD-03 | — | N/A | integration | `cargo test --test reload_file_watch` | ❌ W0 | ⬜ pending |
| 05-01-04 | 01 | 1 | RELOAD-04 | — | Failed parse preserves running config | unit | `cargo test scheduler::reload::tests::failed_parse_noop` | ❌ W0 | ⬜ pending |
| 05-01-05 | 01 | 1 | RELOAD-05 | — | N/A | unit | `cargo test scheduler::sync::tests::reload_diff` | ❌ W0 | ⬜ pending |
| 05-01-06 | 01 | 1 | RELOAD-06 | — | N/A | integration | `cargo test --test reload_inflight` | ❌ W0 | ⬜ pending |
| 05-01-07 | 01 | 1 | RELOAD-07 | — | N/A | unit | `cargo test scheduler::sync::tests::sync_disables_removed_job` | ✅ | ⬜ pending |
| 05-02-01 | 02 | 1 | RAND-01 | — | N/A | unit | `cargo test scheduler::random::tests::resolve_random` | ❌ W0 | ⬜ pending |
| 05-02-02 | 02 | 1 | RAND-02 | — | N/A | unit | `cargo test scheduler::random::tests::stable_across_reload` | ❌ W0 | ⬜ pending |
| 05-02-03 | 02 | 1 | RAND-03 | — | N/A | unit | `cargo test scheduler::random::tests::rerandom_on_change` | ❌ W0 | ⬜ pending |
| 05-02-04 | 02 | 1 | RAND-04 | — | N/A | unit | `cargo test scheduler::random::tests::min_gap_enforced` | ❌ W0 | ⬜ pending |
| 05-02-05 | 02 | 1 | RAND-05 | — | N/A | unit | `cargo test scheduler::random::tests::infeasible_gap` | ❌ W0 | ⬜ pending |
| 05-03-01 | 03 | 2 | RAND-06 | — | N/A | manual-only | Visual inspection of job detail page | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/scheduler/random.rs` — `@random` resolver module with unit test stubs
- [ ] `src/scheduler/reload.rs` — `do_reload()` module with unit test stubs
- [ ] `tests/reload_sighup.rs` — SIGHUP integration test stub
- [ ] `tests/reload_file_watch.rs` — file watcher integration test stub
- [ ] `tests/reload_inflight.rs` — in-flight run survival integration test stub

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Job Detail page shows raw + resolved schedule | RAND-06 | Visual UI layout | 1. Create job with `@random` schedule. 2. Open job detail page. 3. Verify both raw and resolved schedules visible with proper labels. 4. Verify `@random` badge on dashboard list. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
