---
phase: 18
slug: webhook-payload-state-filter-coalescing
status: draft
nyquist_compliant: true
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

> Populated by the planner during Phase 18 planning. Each row maps `task_id → REQ-ID → automated command` from the task's `<automated>` block in its PLAN.md.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 18-01-01 | 01 | 0 | WH-01, WH-03, WH-06, WH-09 | T-18-01 | reqwest 0.13 with rustls-only feature; cargo tree -i openssl-sys empty after deps land | build | `cargo build --workspace 2>&1 \| tail -20 ; cargo tree -i openssl-sys ; echo "exit=$?"` | ❌ W0 | ⬜ pending |
| 18-01-02 | 01 | 0 | WH-09 | T-18-03 | metrics families described from boot for `cronduit_webhook_delivery_sent_total` and `_failed_total` (Pitfall 3 mitigation) | unit | `just test-unit 2>&1 \| tail -5 ; cargo test --test metrics_endpoint metrics_families_described_from_boot -- --nocapture 2>&1 \| tail -20` | ❌ W0 | ⬜ pending |
| 18-02-01 | 02 | 1 | WH-01 | T-18-07 | WebhookConfig struct with `Option<SecretString>` for secret (NOT String); webhook field absent from DockerJobConfig (5-layer parity exempt) | unit | `cargo test --lib --all-features config::tests::webhook 2>&1 \| tail -25` | ❌ W0 | ⬜ pending |
| 18-02-02 | 02 | 1 | WH-01 | — | apply_defaults webhook merge mirrors image/network replace-on-collision; no type-gate (works for command/script/docker) | unit | `cargo test --lib --all-features config::defaults 2>&1 \| tail -30` | ❌ W0 | ⬜ pending |
| 18-02-03 | 02 | 1 | WH-01 | T-18-04, T-18-05, T-18-06, T-18-08, T-18-09 | LOAD-time validators reject malformed url, unknown states (sorted offending list), secret xor unsigned, negative fire_every, empty resolved secret (Pitfall H) | unit | `cargo test --lib --all-features config::validate 2>&1 \| tail -30` | ❌ W0 | ⬜ pending |
| 18-03-01 | 03 | 1 | WH-09 | T-18-10, T-18-11, T-18-12 | WebhookPayload 16 fields in declaration order; deterministic compact JSON bytes; RFC3339 Z-suffix; null for missing optionals | unit | `cargo build --workspace 2>&1 \| tail -10 ; cargo test --lib --all-features webhooks::payload 2>&1 \| tail -25` | ❌ W0 | ⬜ pending |
| 18-03-02 | 03 | 1 | WH-06 | T-18-13, T-18-14, T-18-15, T-18-37 | filter_position dual-backend SQL with hard-coded D-15 success sentinel (CASE WHEN status='success' THEN 0 BEFORE IN-list); idx_job_runs_job_id_start hit on both backends; Pitfall I 6-placeholder bind padding | unit + integration | `cargo build --workspace 2>&1 \| tail -5 ; cargo test --lib --all-features webhooks::coalesce 2>&1 \| tail -25 ; cargo test --test v12_webhook_filter_position_explain 2>&1 \| tail -20` | ❌ W0 | ⬜ pending |
| 18-04-01 | 04 | 2 | WH-03 | T-18-16, T-18-17, T-18-18, T-18-19, T-18-20, T-18-21, T-18-22, T-18-23, T-18-24, T-18-25 | HttpDispatcher impl: 3 Standard Webhooks v1 headers, sign-once Pitfall B mitigation, base64 STANDARD alphabet, 10-digit Unix-seconds timestamp, 26-char ULID, unsigned-mode omits webhook-signature, secret never leaks via Debug/Display, rustls invariant holds | unit | `cargo build --workspace 2>&1 \| tail -10 ; cargo test --lib --all-features webhooks::dispatcher 2>&1 \| tail -25 ; cargo tree -i openssl-sys 2>&1 ; echo "openssl-exit=$?"` | ❌ W0 | ⬜ pending |
| 18-05-01 | 05 | 3 | WH-01, WH-03, WH-06, WH-09 | T-18-26, T-18-36 | Bin layer wires HttpDispatcher conditionally on configured webhooks; map built via name-keyed lookup (NOT blind zip — T-18-36 mitigation); NoopDispatcher fallback when no webhook configured | build + clippy | `cargo build --workspace 2>&1 \| tail -10 ; cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 \| tail -10` | ❌ W0 | ⬜ pending |
| 18-05-02 | 05 | 3 | WH-01, WH-03, WH-09 | T-18-28, T-18-29, T-18-30, T-18-36 | wiremock e2e: 3 headers + 16-field body + recomputed signature equals captured signature; multi-job alignment (job-1 → URL A, job-2 → URL B); unsigned omits signature; success-state filter exclusion | integration | `cargo test --test v12_webhook_delivery_e2e 2>&1 \| tail -20 ; cargo test --test v12_webhook_unsigned_omits_signature 2>&1 \| tail -15 ; cargo test --test v12_webhook_state_filter_excludes_success 2>&1 \| tail -15` | ❌ W0 | ⬜ pending |
| 18-05-03 | 05 | 3 | WH-09 | T-18-27 | counter delta-asserts: 2xx → sent +1; 5xx → failed +1; network error → failed +1 (D-17) | integration | `cargo test --test v12_webhook_success_metric 2>&1 \| tail -10 ; cargo test --test v12_webhook_failed_metric 2>&1 \| tail -10 ; cargo test --test v12_webhook_network_error_metric 2>&1 \| tail -10` | ❌ W0 | ⬜ pending |
| 18-06-01 | 06 | 3 | WH-01, WH-03, WH-06, WH-09 | T-18-31, T-18-34, T-18-35 | 4 just recipes (3 UAT + api-run-now helper); uat-webhook-fire delegates to just api-run-now (no raw curl on UAT-callable surface); examples/cronduit.toml has 3 webhook variants; mock receiver compiles with Connection: close framing | build + just-list | `cargo build --example webhook_mock_server 2>&1 \| tail -5 ; just check-config examples/cronduit.toml 2>&1 \| tail -10 ; grep -c '^uat-webhook-' justfile ; grep -c '^api-run-now ' justfile` | ❌ W0 | ⬜ pending |
| 18-06-02 | 06 | 3 | WH-01, WH-03, WH-06, WH-09 | T-18-32, T-18-33 | 18-HUMAN-UAT.md authored with 7+ scenarios; every step references a just recipe; all checkboxes unchecked (Claude does NOT pre-mark; project memory `feedback_uat_user_validates.md`) | filesystem | `test -f .planning/phases/18-webhook-payload-state-filter-coalescing/18-HUMAN-UAT.md && grep -c '\[ \] Maintainer-validated' .planning/phases/18-webhook-payload-state-filter-coalescing/18-HUMAN-UAT.md` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky · File Exists: ❌ W0 = file does not exist yet (Wave 0 / first execution must create it)*

---

## Wave 0 Requirements

> Wave 0 in Phase 18 is the test-infrastructure scaffolding the planner identifies. RESEARCH.md recommends `wiremock = "0.6"` as a dev-dependency for HTTP receiver mocking. This list flips to `wave_0_complete: true` AFTER Plan 01 lands at execute time.

- [ ] Add `wiremock = "0.6"` to `[dev-dependencies]` in `Cargo.toml` (Phase 18's primary integration mock; researcher pinned 0.6.5 as current) — Plan 01 Task 1
- [ ] Add `reqwest 0.13` (rustls + json features), `hmac = "0.13"`, `base64 = "0.22"`, `ulid = "1.2"` runtime deps to `Cargo.toml` — Plan 01 Task 1
- [ ] Describe + zero-baseline `cronduit_webhook_delivery_sent_total` and `cronduit_webhook_delivery_failed_total` in `src/telemetry.rs::setup_metrics` — Plan 01 Task 2
- [ ] Add `just test-unit` recipe to `justfile` — Plan 01 Task 2
- [ ] Confirm `just test` recipe exists in `justfile` (already present from Phase 15/16/17) — verified pre-Phase-18

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| End-to-end webhook delivery to a real receiver (`webhook.site`, ngrok-tunneled localhost, or operator's own httpbin) — verify headers, body, signature in a real HTTP path | WH-03, WH-09 | Project memory `feedback_uat_user_validates.md` blocks Claude from marking UAT passed from its own runs; real-world receiver semantics (TLS handshake variance, real network latency, real-world signature-verification libs) are out of automated scope | `just uat-webhook-mock` + `just dev` + `just uat-webhook-fire <JOB>` + `just uat-webhook-verify` (Plan 06 — see 18-HUMAN-UAT.md Scenarios 1, 2) |
| `fire_every = 0` legacy mode delivers every failure on a `* * * * *` failing job (operator confirms 30+ deliveries in 30 minutes) | WH-06 | Same UAT-user-validates rule; long-running observation | 18-HUMAN-UAT.md Scenario 5 (`just uat-webhook-fire wh-example-fire-every-zero` repeated) |
| `${WEBHOOK_SECRET}` env-var interpolation produces the actual secret in the signing path (operator sets env var, sees signature verify on receiver side) | WH-01 | Operator-side signature verification with their library of choice (Python `standardwebhooks`, Go `standardwebhooks`, Node `standardwebhooks`); part of WH-04 receiver-example phase but Phase 18 needs operator-confirm that the secret round-trip works | 18-HUMAN-UAT.md Scenario 6 (`just check-config examples/cronduit.toml` with various env-var states) |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies (planner-filled per-task map above)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (wiremock dep, runtime deps, telemetry describe, just-recipe wiring)
- [x] No watch-mode flags (`--watch`, `cargo watch` forbidden in CI/automated commands per project precedent)
- [x] Feedback latency < 30s for unit / < 3m for full
- [x] `nyquist_compliant: true` set in frontmatter (every task has an `<automated>` command and Wave 0 prerequisites are enumerated above)
- [ ] `wave_0_complete: true` — flips during execute when Plan 01 lands
- [ ] `cargo tree -i openssl-sys` empty after `reqwest` + `hmac` + `sha2` + `base64` + `ulid` additions (rustls-only project lock) — verified at execute time

**Approval:** pending (auto-approved at planner sign-off; maintainer re-confirms during `/gsd-verify-work`)
</content>
</invoke>
