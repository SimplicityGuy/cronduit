---
phase: 7
slug: v1-cleanup-bookkeeping
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-12
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
| 7-01-01 | 01 | 1 | OPS-04 | — | docker-compose.yml comment strengthens loopback/expose guidance | manual-review | `docker compose -f examples/docker-compose.yml config` (YAML validity) | ✅ | ⬜ pending |
| 7-01-02 | 01 | 1 | OPS-04 | — | 06-VERIFICATION.md `overrides:` block records D-12 acceptance | yaml-lint | `python3 -c "import yaml,sys; yaml.safe_load(open('.planning/phases/06-live-events-metrics-retention-release-engineering/06-VERIFICATION.md').read().split('---')[1])"` | ✅ | ⬜ pending |
| 7-02-01 | 02 | 2 | OPS-04 + bookkeeping | — | REQUIREMENTS.md traceability table has Evidence column + flipped REQ-IDs cite real verification files | grep cross-check | `for req in $(awk '/Status: Complete/{print $1}' .planning/REQUIREMENTS.md); do grep -l "$req.*SATISFIED" .planning/phases/*/0?-VERIFICATION.md \|\| echo "MISSING: $req"; done` | ✅ | ⬜ pending |
| 7-03-01 | 03 | 1 | bookkeeping | — | 05-VERIFICATION.md `re_verification:` block valid YAML, status flipped to `code_complete, human_needed` | yaml-lint | `python3 -c "import yaml; yaml.safe_load(open('.planning/phases/05-config-reload-random-resolver/05-VERIFICATION.md').read().split('---')[1])"` | ✅ | ⬜ pending |
| 7-04-01 | 04 | 1 | RELOAD-* regression | — | POST /api/reload returns header `HX-Refresh: true` | integration (HTTP) | `cargo test --test reload_api reload_response_includes_hx_refresh_header` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

> Plan/wave numbers above are placeholders — final assignments live in the PLAN.md files. The cross-cutting rule: every Complete-flip task in plan 02 depends on plan 01 (D-06), and the D-09/D-10 annotation in plan 03 must land before D-03's strict cross-check runs against RAND-03.

---

## Wave 0 Requirements

- [ ] `tests/reload_api.rs` — NEW FILE for D-14 HX-Refresh regression test
- [ ] No framework or fixture installs needed; `tower 0.5 + util`, `axum`, and the existing dev-dependency lineup already cover the HTTP-layer test pattern (`tower::ServiceExt::oneshot`)

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

- [ ] All code-touching tasks have `<automated>` verify or Wave 0 dependencies (D-14 only)
- [ ] All doc-touching tasks have a `yaml-lint` or `grep` verify command
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify (Phase 7 is small enough that this trivially holds)
- [ ] Wave 0 covers the only MISSING test file (`tests/reload_api.rs`)
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s for full suite
- [ ] `nyquist_compliant: true` set in frontmatter once plans land

**Approval:** pending
