---
phase: 3
slug: read-only-web-ui-health-endpoint
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-10
updated: 2026-04-14
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + cargo nextest (already configured) |
| **Config file** | `.config/nextest.toml` or default |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `just nextest` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `just nextest`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | UI-10 | T-03-01 | XSS: `<script>` in logs renders as escaped text | integration | `cargo test --test xss_log_safety` (7 tests) | ✅ `tests/xss_log_safety.rs` | ✅ green |
| 03-01-02 | 01 | 1 | UI-15 | T-03-02 | CSRF: mismatched tokens rejected | unit | `cargo test --lib web::csrf` (6 tests: `generate_token_is_unique`, `matching_tokens_validate`, `mismatched_tokens_reject`, `empty_cookie_rejects`, `empty_form_field_rejects`, `different_length_tokens_reject`) | ✅ `src/web/csrf.rs` | ✅ green |
| 03-02-01 | 02 | 1 | UI-12 | T-03-03 | Run Now goes through scheduler command channel | integration | `cargo test --test api_run_now` (2 tests: `run_now_dispatches_scheduler_cmd_and_returns_hx_refresh` verifies 200 + `HX-Refresh: true` header + exact `SchedulerCmd::RunNow` dispatched to mpsc; `run_now_returns_404_for_unknown_job` verifies no-command-leak on not-found path) | ✅ `tests/api_run_now.rs` (created by gsd-nyquist-auditor 2026-04-14) | ✅ green |
| 03-02-02 | 02 | 1 | OPS-01 | — | Health endpoint returns correct JSON | integration | `cargo test --test health_endpoint` (2 tests: `health_returns_200_with_ok_status`, `health_returns_json_content_type`) | ✅ `tests/health_endpoint.rs` | ✅ green |
| 03-03-01 | 03 | 1 | UI-07 | — | Server-side countdown computation for next-fire / last-run | unit | `cargo test --lib web::handlers::dashboard` (4 tests: `test_format_relative_future`, `test_format_relative_past`, `test_format_relative_past_days`, `test_format_relative_past_just_now`) | ✅ `src/web/handlers/dashboard.rs` | ✅ green |
| 03-03-02 | 03 | 1 | UI-06 | — | Dashboard lists all jobs with 6 required fields | integration | `cargo test --test dashboard_render` (2 tests: `dashboard_renders_all_jobs_with_six_required_fields` seeds 2 jobs + runs and asserts names, raw schedules, next-fire marker, success+running badges, last-run relative timestamps, Run Now controls; `dashboard_empty_state_when_no_jobs` verifies empty-state render) | ✅ `tests/dashboard_render.rs` (created by gsd-nyquist-auditor 2026-04-14) | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `tests/xss_log_safety.rs` — UI-10 XSS prevention integration test (7 tests, present since phase execution)
- [x] `tests/health_endpoint.rs` — OPS-01 integration test (2 tests, present since phase execution)
- [x] `src/web/csrf.rs::tests` — UI-15 unit tests (6 tests, present since phase execution)
- [x] `tests/api_run_now.rs` — UI-12 integration test (2 tests, created 2026-04-14 by retroactive Nyquist gap-fill)
- [x] `tests/dashboard_render.rs` — UI-06 integration test (2 tests, created 2026-04-14 by retroactive Nyquist gap-fill)

*Existing `cargo test` infrastructure covers framework setup.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Design system visual fidelity (terminal-green palette, typography) | UI-02, UI-03 | CSS visual match requires human inspection | Open dashboard in browser, compare against `design/showcase.html` side-by-side |
| Dark/light mode toggle persistence | UI-04 | localStorage behavior requires browser interaction | Toggle theme, refresh page, verify preference persists |
| HTMX 3s auto-refresh visually smooth | UI-07 | Requires observing live polling behavior | Watch dashboard for 10s, verify table updates without full-page flash |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 15s (all unit + integration tests complete in under 1s each)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved (retroactive audit 2026-04-14)

---

## Validation Audit 2026-04-14

| Metric | Count |
|--------|-------|
| Gaps found | 2 |
| Resolved | 2 (tests/api_run_now.rs + tests/dashboard_render.rs, both created by gsd-nyquist-auditor, green on iteration 1) |
| Escalated | 0 |

**Audit method:** Retroactive cross-reference against current `src/web/` and `tests/` trees. Phase 3 shipped 2026-04-11 with VERIFICATION.md status `human_needed` (4 visual UI items deferred to Phase 8 walkthrough). The original VALIDATION.md left all 6 task rows marked `⬜ pending` and `nyquist_compliant: false`. This audit found 4 of 6 task rows fully covered by existing tests and 2 real gaps:

1. **UI-12 Run Now handler** — the handler was E2E-exercised by compose-smoke CI but had no dedicated unit/handler test with a ~1s feedback loop.
2. **UI-06 Dashboard render** — the template was askama-compile-checked but no test asserted the rendered HTML contained the 6 required job fields.

Both gaps were filled by spawning `gsd-nyquist-auditor` which created `tests/api_run_now.rs` and `tests/dashboard_render.rs` from the `tests/reload_api.rs` + `tests/health_endpoint.rs` patterns. All 4 new tests passed on the first run with zero debug iterations and no implementation files touched.

**Key evidence:**
- `tests/xss_log_safety.rs` — 7 tests (UI-10)
- `src/web/csrf.rs::tests` — 6 tests covering token generation, match/mismatch, empty/length edge cases (UI-15)
- `tests/api_run_now.rs` — 2 tests: happy path (200 + HX-Refresh + `SchedulerCmd::RunNow` dispatched via mpsc recv) and 404 no-command-leak (UI-12) — **added 2026-04-14**
- `tests/health_endpoint.rs` — 2 tests (OPS-01)
- `src/web/handlers/dashboard.rs::tests` — 4 tests covering the server-side countdown helpers `format_relative_future`, `format_relative_past`, `format_relative_past_days`, `format_relative_past_just_now` (UI-07)
- `tests/dashboard_render.rs` — 2 tests: full render asserting names + raw schedules + next-fire marker + success/running badges + last-run relative timestamps + Run Now controls, plus empty-state render (UI-06) — **added 2026-04-14**

**Manual-only items retained:** Design-system visual fidelity (UI-02/03), dark/light toggle persistence (UI-04), HTMX 3s polling visual smoothness (UI-07 visual sub-check). These are inherently browser-only and were covered wholesale by Phase 8 walkthrough with user verbal approval. Per project policy `feedback_uat_user_validates.md`, on-disk `03-HUMAN-UAT.md` rows remain `result: pending` pending the orchestrator's decision on verbal-vs-per-row (see v1.0 milestone audit OPS-05 partial).
