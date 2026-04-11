---
phase: 3
slug: read-only-web-ui-health-endpoint
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-10
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
| 03-01-01 | 01 | 1 | UI-10 | T-03-01 | XSS: `<script>` in logs renders as escaped text | integration | `cargo test --test xss_log_safety` | ❌ W0 | ⬜ pending |
| 03-01-02 | 01 | 1 | UI-15 | T-03-02 | CSRF: mismatched tokens rejected | unit | `cargo test web::csrf::tests` | ❌ W0 | ⬜ pending |
| 03-02-01 | 02 | 1 | UI-12 | T-03-03 | Run Now goes through scheduler command channel | unit | `cargo test web::handlers::api::tests` | ❌ W0 | ⬜ pending |
| 03-02-02 | 02 | 1 | OPS-01 | — | Health endpoint returns correct JSON | integration | `cargo test --test health_endpoint` | ❌ W0 | ⬜ pending |
| 03-03-01 | 03 | 1 | UI-07 | — | N/A | integration | `cargo test --test dashboard_partial` | ❌ W0 | ⬜ pending |
| 03-03-02 | 03 | 1 | UI-06 | — | N/A | integration | `cargo test --test dashboard_render` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/xss_log_safety.rs` — stubs for UI-10 (XSS prevention CI test)
- [ ] `tests/health_endpoint.rs` — stubs for OPS-01
- [ ] `src/web/csrf.rs` unit tests — stubs for UI-15
- [ ] `src/web/handlers/api.rs` unit tests — stubs for UI-12

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

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
