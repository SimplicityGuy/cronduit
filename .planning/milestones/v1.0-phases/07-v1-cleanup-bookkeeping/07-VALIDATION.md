---
phase: 7
slug: v1-cleanup-bookkeeping
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-12
updated: 2026-04-14
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution. Phase 7 is bookkeeping + 1 regression test, so validation is intentionally minimal.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo-nextest (preferred) / `cargo test` |
| **Config file** | `Cargo.toml` dev-dependencies (tower 0.5 + util, axum, etc. — already present) |
| **Quick run command** | `cargo test --test reload_api reload_response_includes_hx_refresh_header` |
| **Full suite command** | `cargo nextest run` (or `cargo test --all`) |
| **Estimated runtime** | ~5s for the new test in isolation; ~60s for the full suite |

---

## Sampling Rate

- **After every task commit:** Run `cargo build --tests` (cheap compile-only sanity check; new test file must compile)
- **After D-14 lands:** Run `cargo test --test reload_api reload_response_includes_hx_refresh_header`
- **After every plan wave:** Run `cargo nextest run` (or `cargo test --all`)
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** ~10s for the new test, ~60s for the full suite

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 7-01-01 | 01 | 1 | OPS-04 | — | docker-compose.yml comment strengthens loopback/expose guidance | manual-review | `docker compose -f examples/docker-compose.yml config` (YAML validity) — verified green in 07-VERIFICATION.md commands_run | ✅ | ✅ green |
| 7-01-02 | 01 | 1 | OPS-04 | — | 06-VERIFICATION.md `overrides:` block records D-12 acceptance | yaml-lint | `python3 -c "import yaml,sys; yaml.safe_load(open('.planning/phases/06-live-events-metrics-retention-release-engineering/06-VERIFICATION.md').read().split('---')[1])"` — override block present, `accepted_by: SimplicityGuy 2026-04-13` | ✅ | ✅ green |
| 7-02-01 | 02 | 2 | OPS-04 + bookkeeping | — | REQUIREMENTS.md traceability table has Evidence column + flipped REQ-IDs cite real verification files | grep cross-check | 85/86 flipped to `Complete` with Evidence column per Phase 7 D-03 strict cross-check against SATISFIED rows; OPS-05 deferred to Phase 8 | ✅ | ✅ green |
| 7-03-01 | 03 | 1 | bookkeeping | — | 05-VERIFICATION.md `re_verification:` block valid YAML, status flipped to `code_complete, human_needed` | yaml-lint | 05-VERIFICATION.md frontmatter contains `re_verification:` block with 4 `gap_resolutions` entries (PR #9 closures + Phase 8 deferral) | ✅ | ✅ green |
| 7-04-01 | 04 | 1 | RELOAD-* regression | — | POST /api/reload returns header `HX-Refresh: true` | integration (HTTP) | `cargo test --test reload_api reload_response_includes_hx_refresh_header` | ✅ `tests/reload_api.rs` | ✅ green |
| 7-05-01 | 05 | 1 | UI-08 (Job Detail auto-refresh) | — | `GET /partials/jobs/:job_id/runs` returns conditional-polling wrapper that stops once runs are terminal | integration (HTTP) | `cargo test --test job_detail_partial run_history_partial_renders_badges_and_enables_polling_while_running · cargo test --test job_detail_partial run_history_partial_stops_polling_when_all_runs_terminal` (3 tests total) | ✅ `tests/job_detail_partial.rs` | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

> Plan/wave numbers above are placeholders — final assignments live in the PLAN.md files. The cross-cutting rule: every Complete-flip task in plan 02 depends on plan 01 (D-06), and the D-09/D-10 annotation in plan 03 must land before D-03's strict cross-check runs against RAND-03.

---

## Wave 0 Requirements

- [x] `tests/reload_api.rs` — created during Plan 04 (D-14); 1 test `reload_response_includes_hx_refresh_header` passes
- [x] `tests/job_detail_partial.rs` — created during Plan 05 (D-16); 2 tests covering conditional HTMX polling and stop-when-all-terminal
- [x] No framework or fixture installs needed; `tower 0.5 + util`, `axum`, and the existing dev-dependency lineup cover the HTTP-layer test pattern (`tower::ServiceExt::oneshot`)

*All other Phase 7 work is doc/config edits and uses no test fixtures.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Strengthened docker-compose.yml comment communicates the security posture clearly | OPS-04 | Comment quality is a human judgment call (clarity, tone, no ASCII art) | Reviewer reads `examples/docker-compose.yml` top comment block; confirms (1) loud `ports: 8080:8080` security warning, (2) `THREAT_MODEL.md` reference, (3) `expose:` snippet shown for production, (4) no ASCII art, (5) usage lines below preserved |
| 81→~84 REQ-ID flips reflect true SATISFIED status | bookkeeping | Strict D-03 cross-check is mechanical but requires per-row judgment for PARTIAL/FAILED items (e.g., RAND-03, CONF-07) | Reviewer spot-checks 5 random flipped rows: confirms each Evidence path resolves and the cited file contains the REQ-ID in a SATISFIED row |
| 05-VERIFICATION.md re_verification annotation cites real PR-#9 line numbers | bookkeeping | Auditing line-number citations is a one-time manual check | Reviewer runs `git show 8b69cb8 -- src/scheduler/reload.rs src/web/handlers/api.rs` and confirms the cited lines exist in that commit |

---

## Validation Sign-Off

- [x] All code-touching tasks have `<automated>` verify (D-14 reload_api + D-16 job_detail_partial)
- [x] All doc-touching tasks have a `yaml-lint` or `grep` verify command
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers the MISSING test files (`tests/reload_api.rs`, `tests/job_detail_partial.rs`)
- [x] No watch-mode flags
- [x] Feedback latency < 60s for full suite
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved (retroactive audit 2026-04-14)

---

## Validation Audit 2026-04-14

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

**Audit method:** Retroactive cross-reference against current repo state. Phase 7 shipped 2026-04-13 with VERIFICATION.md status `human_needed` (score 5/5 — the single human item was the Job Detail Run History browser UAT, which was deferred to Phase 8 and then covered by the Phase 8 walkthrough). The original VALIDATION.md tracked 5 tasks (7-01-01..7-04-01); this audit adds 7-05-01 for Plan 05 (Job Detail Run History partial endpoint) which was added to the phase after the original validation draft.

**Key evidence:**
- `tests/reload_api.rs::reload_response_includes_hx_refresh_header` — 1 test, passes; asserts POST /api/reload response carries `HX-Refresh: true` via `tower::ServiceExt::oneshot`
- `tests/job_detail_partial.rs` — 2 tests: `run_history_partial_renders_badges_and_enables_polling_while_running` (asserts `hx-trigger` present when any run is still running) and `run_history_partial_stops_polling_when_all_runs_terminal` (asserts no polling trigger when all runs are terminal)
- `examples/docker-compose.yml` — `docker compose -f examples/docker-compose.yml config` validity verified in 07-VERIFICATION.md commands_run list
- `.planning/phases/06-live-events-metrics-retention-release-engineering/06-VERIFICATION.md` — `overrides:` block present with `accepted_by: SimplicityGuy` and `accepted_at: 2026-04-13T20:45:03Z`
- `.planning/REQUIREMENTS.md` — 85/86 flipped to Complete with Evidence column per Phase 7 D-03 strict cross-check (see REQUIREMENTS.md footer lines 271-282)
- `.planning/phases/05-config-reload-random-resolver/05-VERIFICATION.md` — `re_verification:` block documents PR #9 closure of 3 of 4 deferred gaps

**Manual-only items retained as legitimate:** comment quality, spot-check of REQ-ID flip accuracy, PR-#9 line-number audit — these are one-time document-review items, not code-testable behaviors. The Job Detail Run History browser UAT in 07-UAT.md was covered by the Phase 8 walkthrough (see v1.0 milestone audit bookkeeping debt — row still shows `result: pending` pending the orchestrator's verbal-approval decision on OPS-05).
