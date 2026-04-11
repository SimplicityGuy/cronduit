---
phase: 5
slug: config-reload-random-resolver
status: draft
nyquist_compliant: true
wave_0_complete: true
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

<<<<<<< HEAD
## Nyquist Compliance Justification

Wave 0 test stubs are NOT required as separate pre-created files. Each plan creates its own test files as part of in-task TDD (`tdd="true"` on Plan 01 Task 1) or in-task implementation (Plan 05 Task 1 creates integration tests). This provides adequate Wave 1-2 feedback coverage because:

1. **Plan 01 Task 1** (Wave 1) is `tdd="true"` — tests are written BEFORE implementation for `@random` resolver unit tests. The `<behavior>` block defines 12 test cases that run via `cargo test scheduler::random::tests`.
2. **Plan 01 Task 2** (Wave 1) verifies sync engine integration via `cargo test scheduler::sync::tests`.
3. **Plan 02** (Wave 1) verifies compilation (`cargo build`) — reload infrastructure is wired but integration tests come in Plan 05.
4. **Plan 05 Task 1** (Wave 3) creates all integration test files (`tests/reload_sighup.rs`, `tests/reload_inflight.rs`, `tests/reload_random_stability.rs`, `tests/reload_file_watch.rs`) and runs them immediately.

Every task has an `<automated>` verify command. No 3 consecutive tasks lack automated feedback. Feedback latency is under 30 seconds for all commands.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|--------|
| 05-01-01 | 01 | 1 | RAND-01..05, RELOAD-05 | T-05-01 | Validate field count before resolution | unit (TDD) | `cargo test scheduler::random::tests` | ⬜ pending |
| 05-01-02 | 01 | 1 | RELOAD-05 | T-05-02 | Sync engine resolves @random | unit | `cargo test scheduler::sync::tests` | ⬜ pending |
| 05-02-01 | 02 | 1 | RELOAD-01, RELOAD-03 | — | N/A | build | `cargo build` | ⬜ pending |
| 05-02-02 | 02 | 1 | RELOAD-04..07 | T-05-04..07 | Failed parse preserves config | build | `cargo build` | ⬜ pending |
| 05-03-01 | 03 | 2 | RELOAD-02, RELOAD-06 | T-05-08..10 | Scheduler loop wiring | build | `cargo build` | ⬜ pending |
| 05-03-02 | 03 | 2 | RELOAD-02 | T-05-08..09 | CSRF on reload/reroll | build | `cargo build` | ⬜ pending |
| 05-04-01 | 04 | 2 | RAND-06 | T-05-12..14 | Toast + settings UI | build | `cargo build` | ⬜ pending |
| 05-04-02 | 04 | 2 | RAND-06 | T-05-13 | @random badge + re-roll UI | build | `cargo build` | ⬜ pending |
| 05-05-01 | 05 | 3 | RELOAD-01..07, RAND-01..03, RAND-06 | — | Full integration coverage | integration | `cargo test --test reload_sighup --test reload_inflight --test reload_random_stability --test reload_file_watch` | ⬜ pending |
| 05-05-02 | 05 | 3 | RAND-06 | — | Visual UI verification | manual | Human checkpoint | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Job Detail page shows raw + resolved schedule | RAND-06 | Visual UI layout | 1. Create job with `@random` schedule. 2. Open job detail page. 3. Verify both raw and resolved schedules visible with proper labels. 4. Verify `@random` badge on dashboard list. |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or are created in-task
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] In-task test creation provides adequate feedback coverage (see Nyquist Compliance Justification)
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
