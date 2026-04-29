---
phase: 18
slug: webhook-payload-state-filter-coalescing
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-29
---

# Phase 18 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo nextest` (project-locked per CLAUDE.md tech stack); fallback to `cargo test` if nextest is not installed |
| **Config file** | `Cargo.toml` (workspace test settings); `.config/nextest.toml` if present |
| **Quick run command** | `just test-unit` (unit tests only — NO `#[ignore]` integration tests) |
| **Full suite command** | `just test` (unit + integration; `--ignored` covers webhook integration tier) |
| **Estimated runtime** | ~30s quick, ~3m full (includes wiremock integration tests for webhook delivery) |

---

## Sampling Rate

- **After every task commit:** Run `just test-unit`
- **After every plan wave:** Run `just test`
- **Before `/gsd-verify-work`:** Full suite must be green AND `cargo tree -i openssl-sys` must return empty (rustls-only invariant)
- **Max feedback latency:** ~30s for unit, ~3m for full

---

## Per-Task Verification Map

> Populated by the planner during Phase 18 planning. Each PLAN.md task gets a row mapping `task_id → REQ-ID → automated command`. The planner is the source of truth — this section is a placeholder until plans are written.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 18-01-01 | 01 | 1 | WH-01 | — | LOAD-time validator rejects malformed `webhook.url` / unknown `webhook.states` value / empty `states = []` / `secret` xor `unsigned` constraint with grep-checkable error message | unit | `just test-unit -- config::validate::webhook` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

*Planner expansion target — RESEARCH.md § Validation Architecture lists ~30 named tests across:*
- *Config validators (unit) — URL parsing, state-name allowlist, completeness (secret xor unsigned), `fire_every >= 0`, defaults merge*
- *Payload encoder (unit) — 15-field schema, RFC3339 with `Z` suffix, `null` for missing optional fields, key ordering determinism*
- *Signature builder (unit) — HMAC-SHA256 over `${id}.${ts}.${body}` with literal `.` separators, base64 STANDARD with padding*
- *Filter-matching position computer (unit + DB integration) — single-SQL backwards walk, `IN (...)` bind padding, `success` resets*
- *HttpDispatcher (integration via `wiremock`) — single-attempt, 10s timeout, headers present (`webhook-id`, `webhook-timestamp`, `webhook-signature`), body matches, signature verifies, `unsigned = true` omits header, failure increments `cronduit_webhook_delivery_failed_total`*
- *End-to-end (integration) — config → load → worker → wiremock receiver — verify on `failed → success → failed` sequence with `states = ["failed"]` only the second `failed` fires (filter-position == 1 after the `success` non-match)*

---

## Wave 0 Requirements

> Wave 0 in Phase 18 is the test-infrastructure scaffolding the planner identifies. RESEARCH.md recommends `wiremock = "0.6"` as a dev-dependency for HTTP receiver mocking.

- [ ] Add `wiremock = "0.6"` to `[dev-dependencies]` in `Cargo.toml` (Phase 18's primary integration mock; researcher pinned 0.6.5 as current)
- [ ] Create `tests/webhooks_integration.rs` (or `tests/webhooks/` module under `src/webhooks/` with `#[cfg(test)] mod tests`) — house wiremock-driven integration tests; the planner picks the exact path
- [ ] Create `tests/common/webhook_helpers.rs` (or equivalent) — shared fixtures: signature-verifying mock handler, HMAC oracle helper for assertions
- [ ] Confirm `just test-unit` and `just test` recipes exist in `justfile`; if missing, add them as Wave 0 prerequisite (per project memory `feedback_uat_use_just_commands.md` — UAT must reference existing `just` recipes)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| End-to-end webhook delivery to a real receiver (`webhook.site`, ngrok-tunneled localhost, or operator's own httpbin) — verify headers, body, signature in a real HTTP path | WH-03, WH-09 | Project memory `feedback_uat_user_validates.md` blocks Claude from marking UAT passed from its own runs; real-world receiver semantics (TLS handshake variance, real network latency, real-world signature-verification libs) are out of automated scope | `just demo-webhook-failing` (NEW recipe planner adds) — runs cronduit pointed at a webhook URL the operator supplies via env, lets a `* * * * *` failing job fire, operator inspects the receiver-side delivery log for: 4 v1 fields in payload, `webhook-signature: v1,...`, base64 padding, single delivery on first failure of streak |
| `fire_every = 0` legacy mode delivers every failure on a `* * * * *` failing job (operator confirms 30+ deliveries in 30 minutes) | WH-06 | Same UAT-user-validates rule; long-running observation | `just demo-webhook-fire-every-zero` (NEW recipe) — config has `fire_every = 0`, operator watches receiver count grow |
| `${WEBHOOK_SECRET}` env-var interpolation produces the actual secret in the signing path (operator sets env var, sees signature verify on receiver side) | WH-01 | Operator-side signature verification with their library of choice (Python `standardwebhooks`, Go `standardwebhooks`, Node `standardwebhooks`); part of WH-04 receiver-example phase but Phase 18 needs operator-confirm that the secret round-trip works | `just demo-webhook-signed-receiver` (NEW recipe) — operator runs a tiny verifier Python script against the live delivery |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies (planner fills the per-task map above)
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (wiremock dep, test module, just recipes)
- [ ] No watch-mode flags (`--watch`, `cargo watch` forbidden in CI/automated commands per project precedent)
- [ ] Feedback latency < 30s for unit / < 3m for full
- [ ] `nyquist_compliant: true` set in frontmatter (after planner expands the map and Wave 0 lands)
- [ ] `cargo tree -i openssl-sys` empty after `reqwest` + `hmac` + `sha2` + `base64` + `ulid` additions (rustls-only project lock)

**Approval:** pending
