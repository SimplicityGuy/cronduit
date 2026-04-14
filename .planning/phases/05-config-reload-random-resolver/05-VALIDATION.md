---
phase: 5
slug: config-reload-random-resolver
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-11
updated: 2026-04-14
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
| 05-01-01 | 01 | 1 | RAND-01..05, RELOAD-05 | T-05-01 | Validate field count before resolution | unit (TDD) | `cargo test --lib scheduler::random` (14 tests incl. `resolve_single_random_minute`, `resolve_multiple_random_fields`, `validate_field_count_rejection`, `infeasible_gap_relaxes`, `batch_gap_enforcement`, `stable_across_reload`, `new_resolution_when_raw_changed`) | ✅ green |
| 05-01-02 | 01 | 1 | RELOAD-05 | T-05-02 | Sync engine resolves @random | unit | `cargo test --lib scheduler::sync` (6 tests incl. `sync_inserts_new_jobs`, `sync_updates_changed_job`, `sync_noop_same_hash`, `sync_disables_removed_job`) | ✅ green |
| 05-02-01 | 02 | 1 | RELOAD-01, RELOAD-03 | — | SIGHUP + file-watch debounce | integration | `cargo test --test reload_sighup` (2 tests) · `cargo test --test reload_file_watch` (2 tests) | ✅ green |
| 05-02-02 | 02 | 1 | RELOAD-04..07 | T-05-04..07 | Failed parse preserves config; in-flight runs not cancelled | integration | `cargo test --test reload_inflight` (1 test) · `cargo test --test reload_sighup` (preserves-on-parse-error case) | ✅ green |
| 05-03-01 | 03 | 2 | RELOAD-02, RELOAD-06 | T-05-08..10 | `POST /api/reload` wiring + scheduler channel | integration | `cargo test --test reload_api reload_response_includes_hx_refresh_header` + CSRF tests | ✅ green |
| 05-03-02 | 03 | 2 | RELOAD-02 | T-05-08..09 | CSRF on reload/reroll endpoints | unit+integration | `cargo test --lib web::csrf` (6 tests) · `cargo test --test reload_api` | ✅ green |
| 05-04-01 | 04 | 2 | RAND-06 | T-05-12..14 | Toast + settings UI | compile-check + manual | askama compile-time template check on `cargo check`; manual-only visual confirmation of toast/settings (Phase 8 walkthrough) | ✅ green (compile) / see Manual-Only |
| 05-04-02 | 04 | 2 | RAND-06 | T-05-13 | @random badge + re-roll UI | compile-check + manual | askama compile-time template check; Phase 7 PR #9 closed the do_reroll stub (`src/scheduler/reload.rs:170-172` now calls `random::resolve_schedule`); manual visual via Phase 8 walkthrough | ✅ green (compile + PR #9) / see Manual-Only |
| 05-05-01 | 05 | 3 | RELOAD-01..07, RAND-01..03, RAND-06 | — | Full integration coverage | integration | `cargo test --test reload_sighup --test reload_inflight --test reload_random_stability --test reload_file_watch` (7 tests total) | ✅ green |
| 05-05-02 | 05 | 3 | RAND-06 | — | Visual UI verification | manual | Human checkpoint — 13-item checklist from 05-05-SUMMARY.md Task 2; covered by Phase 8 walkthrough (user verbally approved); flip on-disk result fields if orchestrator decides verbal counts terminal | ⚠️ covered-wholesale-in-phase-8 |

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

**Approval:** approved (retroactive audit 2026-04-14)

---

## Validation Audit 2026-04-14

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

**Audit method:** Retroactive cross-reference against current `src/scheduler/` and `tests/` trees. Phase 5 shipped code 2026-04-12 with `05-VERIFICATION.md` status `code_complete, human_needed` (10/13 must-haves), then the three deferred gaps were closed by Phase 7 PR #9 (do_reroll stub, ReloadResult unchanged count hardcoded 0, settings card HX-Refresh header). The visual checkpoint gap was deferred to Phase 8 walkthrough and verbally approved. Test coverage is comprehensive at the unit + integration tier; UI tasks 05-04-01 and 05-04-02 are compile-checked via askama's compile-time template validation and additionally verified by Phase 8 walkthrough. Removed stale `<<<<<<< HEAD` merge-conflict marker from line 37.

**Key evidence:**
- `src/scheduler/random.rs:266+` — 14 unit tests covering field validation, single/multi-field resolution, stability-across-reload, re-resolution on raw-change, gap enforcement, infeasible-gap relaxation, croner round-trip validation (RAND-01..05)
- `src/scheduler/sync.rs:208+` — 6 unit tests covering insert/update/disable/noop sync paths + secret exclusion from config_hash (RELOAD-05)
- `tests/reload_sighup.rs` — 2 tests for SIGHUP trigger + parse-error-preserves-config (RELOAD-01, RELOAD-04)
- `tests/reload_inflight.rs` — 1 test confirming in-flight runs complete under old config (RELOAD-06)
- `tests/reload_random_stability.rs` — 2 tests confirming resolved_schedule is stable across reloads when raw schedule unchanged (RAND-02)
- `tests/reload_file_watch.rs` — 2 tests for notify-based debounced file reload (RELOAD-03)
- `tests/reload_api.rs` — POST /api/reload handler tests including `reload_response_includes_hx_refresh_header` (Phase 7 PR #9)
- `src/web/csrf.rs:75+` — 6 unit tests covering CSRF token generation, matching, mismatches, empty cases, length-mismatch (RELOAD-02 / reroll CSRF)
- `src/scheduler/reload.rs:170-172` — Phase 7 PR #9 replaced the `do_reroll` stub with a real `random::resolve_schedule` call; regression proven by `tests/reload_random_stability.rs`
- Manual-only visual checkpoint (13 items from 05-05-SUMMARY.md) covered wholesale by Phase 8 walkthrough with user verbal approval

**Maintenance note:** Status row for 05-05-02 is marked ⚠️ to flag that the visual confirmation lives in Phase 8 walkthrough approval, not in this phase's own artifacts. If the orchestrator (per v1.0 milestone audit OPS-05 partial decision) accepts verbal approval as terminal, flip to ✅ green; if per-row re-test is required, run `/gsd-verify-work 05` and flip row-by-row.
