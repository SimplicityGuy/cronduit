---
phase: 18-webhook-payload-state-filter-coalescing
verified: 2026-04-29T23:00:00Z
status: human_needed
score: 8/9 must-haves verified
overrides_applied: 0
human_verification:
  - test: "Run all 7 UAT scenarios in 18-HUMAN-UAT.md end-to-end"
    expected: "Each scenario completes with expected output (headers, payload, metrics); all 7 [ ] checkboxes flipped to [x]"
    why_human: "Per project memory feedback_uat_user_validates.md: Claude must not mark UAT passed. Scenarios require running cronduit against a live mock receiver and inspecting real HTTP headers/body."
---

# Phase 18: Webhook Payload + State-Filter + Coalescing — Verification Report

**Phase Goal:** Operators can configure per-job webhook URLs that fire on a state-filter list with edge-triggered streak coalescing; payloads adhere to the Standard Webhooks v1 spec.

**Verified:** 2026-04-29T23:00:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (from ROADMAP.md Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Operator configures per-job webhook with `states` filter; deliveries fire only on listed statuses, not success | VERIFIED | `check_webhook_block_completeness` validates states against `VALID_WEBHOOK_STATES`; `HttpDispatcher.deliver()` step 2 guards on `cfg.states.iter().any(s == event.status)`. Integration test `v12_webhook_state_filter_excludes_success.rs` asserts zero requests reach the mock server on a success run with `states=["failed"]`. |
| 2 | Default coalescing (fire_every=1) produces ONE delivery on first failure of streak; fire_every=0 restores per-failure firing | VERIFIED | `should_fire(1, 1) == true`, `should_fire(1, 2) == false` confirmed by `coalesce_decision_matrix` test. `filter_position` SQL counts consecutive matches from most-recent backwards stopping at first non-match or success. `should_fire(0, N) == true` for all N. |
| 3 | Delivered payload has 16 locked fields with `payload_version:"v1"`, `event_type:"run_finalized"`, RFC3339-Z timestamps | VERIFIED | `WebhookPayload` struct in `src/webhooks/payload.rs` has exactly 16 fields. `payload_payload_version_is_v1` and `payload_event_type_is_run_finalized` unit tests pass. `payload_timestamps_use_z_suffix` asserts `ends_with('Z')` and `!contains("+00:00")`. `payload_contains_all_16_fields` asserts all 16 JSON keys present. |
| 4 | Delivered headers include `webhook-id`, `webhook-timestamp`, `webhook-signature` (Standard Webhooks v1) on every signed delivery | VERIFIED | `HttpDispatcher.deliver()` steps 8-9 add all three headers. `webhook_delivery_e2e_signed` integration test asserts all 3 headers present; recomputes HMAC-SHA256 over `id.ts.body` and asserts signature equality. `unsigned_webhook_omits_signature_header` asserts `webhook-id` and `webhook-timestamp` still present but `webhook-signature` absent when `unsigned=true`. |

**Score:** 4/4 roadmap success criteria verified

### WH-01..WH-09 Requirement Checks (9-point verification matrix)

| Check | Status | Evidence |
|-------|--------|----------|
| **1. WH-01 config surface** | PASS | `WebhookConfig` (5 fields: url, states, secret as `Option<SecretString>`, unsigned, fire_every) present on both `JobConfig.webhook` and `DefaultsConfig.webhook` in `src/config/mod.rs`. `apply_defaults` webhook merge at `src/config/defaults.rs:192-196` — replace-on-collision, no type-gate, honors `use_defaults=Some(false)`. `check_webhook_url` rejects non-http/https schemes; `check_webhook_block_completeness` rejects empty states, signed+unsigned both-set, neither-set, negative fire_every, and empty-resolved-secret (Pitfall H). Both wired into `run_all_checks`. 19 unit tests in config module (4 parse + 4 merge + 11 validator) all pass in 256-unit-test run. |
| **2. WH-03 wire format** | PASS | `HttpDispatcher.deliver()` emits `webhook-id` (26-char ULID), `webhook-timestamp` (10-digit Unix seconds — Pitfall D), `webhook-signature: v1,<base64-STANDARD>`. `sign_v1` uses `STANDARD` alphabet (not URL-safe) — verified by `signature_uses_standard_base64_alphabet` (200 random iterations, no `-` or `_`). Signing string: `{webhook_id}.{webhook_timestamp}.{body}` with literal `.` separators. `Connection: close` framing is in the UAT mock server (`examples/webhook_mock_server.rs:91`), not in the reqwest dispatcher itself (reqwest handles TCP lifecycle natively). |
| **3. WH-06 coalesce/streak-position** | PASS | `filter_position` async fn in `src/webhooks/coalesce.rs` uses dual-backend SQL (SQLite `?N` + Postgres `$N`). `first_break` CTE uses `MAX(start_time)` — corrected from plan template's `MIN` (auto-fixed deviation, Rule 1 bug). D-15 success sentinel hard-coded: `CASE WHEN status='success' THEN 0` runs BEFORE the IN-list check on BOTH backends. `should_fire(0, N) = true` (always fire); `should_fire(1, 1) = true` (default first-of-stream); `should_fire(N>1, pos) = pos%N==1`. 4 coalesce unit tests + EXPLAIN regression test for `idx_job_runs_job_id_start` (SQLite default-on; Postgres `#[ignore]`-gated). |
| **4. WH-09 payload contract** | PASS | `WebhookPayload<'a>` in `src/webhooks/payload.rs` has exactly 16 fields in declaration order. `serde_derive` emits in declaration order — deterministic byte-stable output (Pitfall B). `payload_serializes_deterministically_to_compact_json` test asserts two consecutive `serde_json::to_vec` calls produce identical bytes and no `\n`. `payload_version = "v1"` and `event_type = "run_finalized"` are `&'static str` literals. All timestamps use `to_rfc3339_opts(SecondsFormat::Secs, true)` → Z suffix. |
| **5. Bin-layer integration** | PASS | `src/cli/run.rs:264-297` builds the per-job webhook map by NAME-keyed lookup (T-18-36 mitigation, explicit alignment). Conditional: if `webhooks.is_empty()` → `NoopDispatcher` (no reqwest::Client overhead); else → `HttpDispatcher::new(pool, Arc::new(webhooks))`. Existing `spawn_worker` call now passes the conditionally-built `dispatcher` Arc. `v12_webhook_two_jobs_distinct_urls` integration test regression-locks name-keyed routing with two MockServers. |
| **6. Test coverage** | PASS | 7 wiremock integration tests (6 from Plan 05 + `v12_webhook_filter_position_explain`) all pass. Breakdown: `v12_webhook_delivery_e2e.rs` (2 tests: signed e2e + two-jobs distinct URLs), `v12_webhook_unsigned_omits_signature.rs` (1), `v12_webhook_state_filter_excludes_success.rs` (1), `v12_webhook_success_metric.rs` (1), `v12_webhook_failed_metric.rs` (1), `v12_webhook_network_error_metric.rs` (1). Plus EXPLAIN regression test (1 SQLite pass + 1 Postgres `#[ignore]`). 19 unit tests in `src/webhooks/` (6 dispatcher + 9 payload + 4 coalesce). 256 total lib tests pass in 1.51s. |
| **7. Quality gates** | PASS | `cargo fmt --check` → exit 0. `cargo clippy --all-targets --all-features -- -D warnings` → clean (no warnings). `cargo tree -i openssl-sys` → `error: package ID specification 'openssl-sys' did not match any packages` (rustls invariant intact). `just test-unit` → 256 passed, 0 failed. |
| **8. UAT artifacts present, NOT declared passed** | PASS | `examples/webhook_mock_server.rs` exists and compiles. `examples/cronduit.toml` has 3 webhook variants (`wh-example-signed`, `wh-example-unsigned`, `wh-example-fire-every-zero`). `${WEBHOOK_SECRET}` interpolation used, no plaintext secret. 4 just recipes registered: `api-run-now`, `uat-webhook-fire`, `uat-webhook-mock`, `uat-webhook-verify`. `18-HUMAN-UAT.md` has 7 scenarios, all `[ ] Maintainer-validated` (0 checked). Per project memory `feedback_uat_user_validates.md`, Claude has not run UAT. |
| **9. Locked decisions honored** | PASS | No `openssl-sys` (rustls-only). No `askama_axum`. No `docker` CLI shelling. `WebhookConfig.secret` is `Option<SecretString>` (not `Option<String>`) — Debug/Display scrubbed. `examples/cronduit.toml` uses `${WEBHOOK_SECRET}` env-var interpolation, never a plaintext literal. Default bind remains `127.0.0.1:8080` (`default_bind()` in `src/config/mod.rs:55`). All diagrams in `src/webhooks/mod.rs` are mermaid code blocks. |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/webhooks/payload.rs` | 16-field WebhookPayload + 9 unit tests | VERIFIED | 275 lines, struct declaration order == JSON order, 9 tests |
| `src/webhooks/coalesce.rs` | filter_position + dual-SQL + 4 unit tests | VERIFIED | 327 lines, D-15 sentinel on both backends, MAX(start_time) |
| `src/webhooks/dispatcher.rs` | HttpDispatcher + sign_v1 + should_fire + 6 unit tests | VERIFIED | sign_v1 with STANDARD base64, 10s timeout, 11-step deliver() |
| `src/webhooks/mod.rs` | Re-exports HttpDispatcher, NoopDispatcher, WebhookPayload | VERIFIED | All 4 re-exports present |
| `src/config/mod.rs` | WebhookConfig struct + helpers | VERIFIED | 5-field struct with SecretString, serde defaults |
| `src/config/defaults.rs` | apply_defaults webhook merge | VERIFIED | Replace-on-collision, no type-gate, use_defaults=false short-circuits |
| `src/config/validate.rs` | check_webhook_url + check_webhook_block_completeness | VERIFIED | Both wired into run_all_checks, 11 unit tests |
| `src/cli/run.rs` | HttpDispatcher wiring with name-keyed map | VERIFIED | Lines 264-297; conditional NoopDispatcher fallback |
| `tests/v12_webhook_delivery_e2e.rs` | Signed e2e + two-jobs routing | VERIFIED | 2 tests pass |
| `tests/v12_webhook_unsigned_omits_signature.rs` | D-05 unsigned mode | VERIFIED | 1 test passes |
| `tests/v12_webhook_state_filter_excludes_success.rs` | State filter | VERIFIED | 1 test passes |
| `tests/v12_webhook_success_metric.rs` | sent_total counter | VERIFIED | 1 test passes |
| `tests/v12_webhook_failed_metric.rs` | failed_total counter | VERIFIED | 1 test passes |
| `tests/v12_webhook_network_error_metric.rs` | ECONNREFUSED counter | VERIFIED | 1 test passes (port 1 approach) |
| `tests/v12_webhook_filter_position_explain.rs` | EXPLAIN regression | VERIFIED | SQLite pass; Postgres `#[ignore]`-gated |
| `examples/webhook_mock_server.rs` | UAT mock receiver | VERIFIED | Compiles; Connection: close framing |
| `examples/cronduit.toml` | 3 webhook job variants | VERIFIED | signed / unsigned / fire_every=0 |
| `justfile` | 4 new recipes | VERIFIED | api-run-now, uat-webhook-{mock,fire,verify} |
| `.planning/phases/.../18-HUMAN-UAT.md` | 7 unchecked scenarios | VERIFIED | 7 `[ ]`, 0 `[x]` |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/cli/run.rs` | `HttpDispatcher` | name-keyed map + `crate::webhooks::HttpDispatcher::new()` | WIRED | Lines 264-297; conditional on `webhooks.is_empty()` |
| `HttpDispatcher.deliver()` | `coalesce::filter_position` | `crate::webhooks::coalesce::filter_position(&self.pool, ...)` | WIRED | dispatcher.rs:183-190 |
| `HttpDispatcher.deliver()` | `WebhookPayload::build` | `crate::webhooks::payload::WebhookPayload::build(event, &fctx, &run_detail, filter_position, ...)` | WIRED | dispatcher.rs:225-232 |
| `HttpDispatcher.deliver()` | `sign_v1` | `sign_v1(secret, &webhook_id, webhook_ts, &body_bytes)` | WIRED | dispatcher.rs:252 (conditional on `!cfg.unsigned`) |
| `WebhookConfig.secret` | `SecretString` | `Option<SecretString>` field type | WIRED | No plaintext; expose_secret() called only for HMAC key + empty-check |
| `check_webhook_url` / `check_webhook_block_completeness` | `run_all_checks` | per-job loop in validate.rs:63-64 | WIRED | Both validators in the per-job loop |
| `apply_defaults` | `webhook` field | replace-on-collision at defaults.rs:192-196 | WIRED | Only when `job.webhook.is_none()` and `!use_defaults=false` |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `HttpDispatcher.deliver()` | `filter_position` | `coalesce::filter_position` SQL query against `job_runs` | Yes — sqlx query against real DB | FLOWING |
| `HttpDispatcher.deliver()` | `fctx` | `get_failure_context(&self.pool, event.job_id)` Phase 16 helper | Yes — single SQL query with CTE | FLOWING |
| `HttpDispatcher.deliver()` | `run_detail` | `get_run_by_id(&self.pool, event.run_id)` | Yes — sqlx query | FLOWING |
| `HttpDispatcher.deliver()` | `body_bytes` | `serde_json::to_vec(&payload)` of real struct | Yes — real serialization, signed once | FLOWING |
| `WebhookPayload.tags` | `tags` | `vec![]` — empty until Phase 22 | Static empty — intentional placeholder | NOTE: schema-stable; Phase 22 populates |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| cargo fmt clean | `cargo fmt --check` | exit 0 | PASS |
| clippy clean | `cargo clippy --all-targets --all-features -- -D warnings` | Finished dev profile, no warnings | PASS |
| 256 unit tests pass | `just test-unit` | `test result: ok. 256 passed; 0 failed` | PASS |
| 6 wiremock integration tests pass | `cargo test --test v12_webhook_delivery_e2e --test v12_webhook_unsigned_omits_signature --test v12_webhook_state_filter_excludes_success --test v12_webhook_success_metric --test v12_webhook_failed_metric --test v12_webhook_network_error_metric` | 7 passed (2+1+1+1+1+1), 0 failed | PASS |
| openssl-sys absent | `cargo tree -i openssl-sys` | `error: package ID specification did not match any packages` | PASS |
| UAT checkboxes unchecked | `grep -c '^\[ \] Maintainer-validated' 18-HUMAN-UAT.md` | 7 | PASS |
| UAT checkboxes not pre-checked | `grep -c '^\[x\] Maintainer-validated' 18-HUMAN-UAT.md` | 0 | PASS |
| EXPLAIN test SQLite | `cargo test --test v12_webhook_filter_position_explain` | 1 passed, 1 ignored (Postgres) | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| WH-01 | 18-02 | Webhook config block on `[[jobs]]` and `[defaults]` with replace-on-collision merge + LOAD validators | SATISFIED | `WebhookConfig` struct, `apply_defaults` merge, `check_webhook_url` + `check_webhook_block_completeness` |
| WH-03 | 18-04 | HttpDispatcher implementing Standard Webhooks v1 wire format (3 headers, HMAC-SHA256, base64 STANDARD) | SATISFIED | `HttpDispatcher`, `sign_v1`, all 6 dispatcher unit tests + signed e2e integration test |
| WH-06 | 18-03 | Filter-position helper that decides whether webhook fires based on streak position | SATISFIED | `coalesce::filter_position`, `should_fire`, 4 coalesce unit tests, D-15 sentinel regression test |
| WH-09 | 18-03 | Deterministic 16-field WebhookPayload encoder | SATISFIED | `WebhookPayload<'a>` struct, 9 unit tests including determinism + field-order + timestamp-Z tests |

### Anti-Patterns Found

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| `src/webhooks/payload.rs` | `tags: vec![]` — static empty | INFO | Intentional placeholder; Phase 22 populates. Schema-stable — not a blocker. |
| `src/webhooks/coalesce.rs` | `#[allow(dead_code)]` on `filter_position` | INFO | Dispatcher (Plan 04) is the consumer; `allow(dead_code)` removed by usage at `dispatcher.rs:183`. Not a stub. |
| `src/webhooks/dispatcher.rs` | `#[allow(dead_code)]` on `WebhookError` variants | INFO | Phase 20 RetryingDispatcher is the consumer. Variants are defined and correct — not stubs. |
| `deferred-items.md` | Pre-existing flaky test in `tests/v12_labels_interpolation.rs` | WARNING | Pre-existing on main; not caused by Phase 18. Concurrent `unsafe env::set/remove_var` between two tests. Out of scope. |

### Human Verification Required

**UAT Status: AWAITING MAINTAINER VALIDATION**

Per project memory `feedback_uat_user_validates.md` (D-26): all 7 UAT scenario checkboxes in `18-HUMAN-UAT.md` are `[ ]` (unchecked). Claude has built and committed all UAT scaffolding but has NOT run the scenarios end-to-end. The maintainer must run each scenario, inspect the actual HTTP traffic, and flip `[ ]` to `[x]`.

#### 1. Phase 18 Human UAT Scenarios (7 scenarios)

**Test:** Follow `18-HUMAN-UAT.md` — run `just uat-webhook-mock` (terminal A), `just dev` (terminal B), then each `just uat-webhook-fire <JOB>` + `just uat-webhook-verify` (terminal C).

**Expected per scenario:**
- S1 (Signed): 3 headers present, `webhook-signature: v1,<padded-base64>`, 16-field compact JSON body with `payload_version:"v1"`, `event_type:"run_finalized"`, `streak_position:1`, `tags:[]`, timestamps ending in `Z`
- S2 (Unsigned): `webhook-id` and `webhook-timestamp` present, `webhook-signature` ABSENT
- S3 (Default coalescing): first failure delivers (streak_position=1); second failure within the streak does NOT deliver (coalesced)
- S4 (State filter): success run against `states=["failed"]` job produces zero mock server hits
- S5 (fire_every=0): every failure delivers regardless of streak position
- S6 (Env-var secret): unset env var → `cronduit check` rejects; empty string → rejects; correct value → accepts
- S7 (Metrics): `just metrics-check` shows `cronduit_scheduler_up 1`; note that `cronduit_webhook_delivery_*` counters are visible via raw `/metrics` but the existing `metrics-check` recipe greps for scheduler-up only (deferred issue per Plan 06)

**Why human:** Real HTTP header inspection, real timing verification for coalescing, real env-var manipulation — cannot be automated without running the full cronduit binary against live services. Per project memory, Claude must not mark UAT passed.

## Deviations Log (all 6 plans)

| Plan | Type | Description | Resolution |
|------|------|-------------|------------|
| 18-01 | Clarification | Plan acceptance criterion `grep -c 'rustls-tls' Cargo.toml` conflicts with the Pitfall-A annotation comment (which must mention `rustls-tls` to explain what NOT to do). The comment-line occurrence is intentional. | `grep -E '^[^#].*rustls-tls' Cargo.toml` → 0 non-comment matches. Both intents satisfied. |
| 18-02 | Rule 3 (auto-fixed) | Adding `JobConfig.webhook` / `DefaultsConfig.webhook` fields broke all existing test fixtures that constructed those structs as field-by-field literals. Plan called out `defaults.rs` but the fix was also needed in `hash.rs`, `validate.rs`, `sync.rs`, and `tests/scheduler_integration.rs`. | Mechanical `webhook: None` additions to all affected fixtures. Committed atomically in `7f60798`. |
| 18-03 | Rule 1 (auto-fixed bug) | Plan template's `first_break` CTE used `MIN(start_time)` for the most-recent non-match. Correct aggregate is `MAX(start_time)` — MIN selects the oldest non-match and over-counts matches. | Corrected to `MAX(start_time)` on both SQLite and Postgres branches. All 4 coalesce unit tests pass with MAX. Committed in `44ea991`. |
| 18-04 | Rule 3 (auto-fixed) | `hmac 0.13` puts `new_from_slice` on `KeyInit` trait, not `Mac`. Plan instructed `<Hmac<Sha256> as Mac>::new_from_slice` which does not compile. | Import `KeyInit` alongside `Mac`; use prelude-resolved `Hmac::<Sha256>::new_from_slice`. Committed in `47f9cd4`. |
| 18-04 | Rule 3 (auto-fixed) | Plan referenced `get_run_detail` which does not exist; actual helper is `get_run_by_id` returning `Option<DbRunDetail>`. | Use `get_run_by_id` + explicit `.ok_or_else(...)` for the impossible-but-defensive None case. Committed in `47f9cd4`. |
| 18-05 | Rule 3 (auto-fixed) | `drop(MockServer)` approach to simulate connection-refused was flaky on macOS (listener lingered past drop). | Switched to `http://127.0.0.1:1` (privileged port → deterministic `ECONNREFUSED`). Merged into `df499e8`. |
| 18-05 | Clarification | Plan's acceptance grep for `"payload_version":"v1"` does not match Rust source (which has backslash-escaped form). Test itself is correct at runtime. | No code change. Verified via escaped form `\"payload_version\":\"v1\"` returning 1 match. |
| 18-06 | Rule 3 (auto-fixed) | Plan `<what-built>` used `Maintainer-validated: [ ]` (label-first) but plan's `<acceptance_criteria>` grep expects `[ ] Maintainer-validated` (bracket-first). Incompatible as written. | Reordered all 7 to bracket-first, matching plan's acceptance grep and `17-HUMAN-UAT.md` precedent. |
| 18-06 | Rule 2 (auto-fixed) | Plan's action for Sub-step B did not include `use_defaults = false` on the three new wh-example-* jobs, but `[defaults].labels` (Watchtower) inherits into every job; LBL-04 validator rejects labels on command-type jobs. | Added `use_defaults = false` + explicit `timeout = "5m"` to all three, mirroring existing `echo-timestamp` / `http-healthcheck` / `disk-usage` pattern. |
| 18-06 | Deferred issue | `just metrics-check` recipe greps for `cronduit_scheduler_up` and `cronduit_runs_total` only. UAT Scenario 7 cannot surface `cronduit_webhook_delivery_*` families via this recipe. Plan explicitly said DO NOT modify existing recipes. | Deferred. Maintainer options: (a) accept S7 as scheduler liveness only (webhook counters verifiable via raw `curl /metrics`), or (b) file a follow-up plan to widen `metrics-check` regex. |

### Notable Architectural Decision

The `tags: vec![]` placeholder in `WebhookPayload` is intentional. Per the `WebhookPayload.tags` field documentation: "Empty `[]` until Phase 22 lights up real values. Schema-stable — Phase 22 cutover does NOT break receivers." This is not a stub in the sense that would block goal achievement — it is the correct schema-stable v1.2 posture where `tags` always emits `[]` until tagging (Phase 22) exists. The field IS present in the JSON output. Receivers that index on `tags` today will work without code changes after Phase 22 populates real values.

---

## Recommended Next Step

**Run the 7 UAT scenarios** from `18-HUMAN-UAT.md`:

1. `just ci` — full CI gate
2. `just openssl-check` — rustls invariant
3. `WEBHOOK_SECRET=test-secret-shh just check-config examples/cronduit.toml` — config validates
4. `just uat-webhook-mock` (terminal A) — start mock receiver
5. `export WEBHOOK_SECRET=my-test-secret-shh && just dev` (terminal B) — start cronduit
6. Scenarios 1-7 per `18-HUMAN-UAT.md`
7. Flip all `[ ]` to `[x]` after validation
8. Open PR for Phase 18

All automated checks (fmt, clippy, 256 unit tests, 7 wiremock integration tests, openssl invariant) are green. The phase is ready for UAT.

---

_Verified: 2026-04-29T23:00:00Z_
_Verifier: Claude (gsd-verifier)_
