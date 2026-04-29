---
phase: 18-webhook-payload-state-filter-coalescing
plan: 04
subsystem: webhooks
tags: [webhooks, http-dispatcher, hmac-sha256, standard-webhooks-v1, reqwest, rustls, ulid, base64, coalescing]

# Dependency graph
requires:
  - phase: 18-webhook-payload-state-filter-coalescing/01
    provides: reqwest 0.13 (rustls + json), hmac 0.13, base64 0.22, ulid 1.2, sha2 0.11 deps; cronduit_webhook_delivery_sent_total / _failed_total Prometheus counter scaffolds
  - phase: 18-webhook-payload-state-filter-coalescing/02
    provides: WebhookConfig struct + JobConfig.webhook + DefaultsConfig.webhook fields, apply_defaults webhook merge, LOAD-time validators
  - phase: 18-webhook-payload-state-filter-coalescing/03
    provides: WebhookPayload::build (16-field v1 schema, deterministic compact JSON) and filter_position helper (D-15 success-sentinel, dual-CTE SQLite + Postgres)
  - phase: 15-webhook-foundation
    provides: WebhookDispatcher trait, NoopDispatcher, RunFinalized channel-message contract, worker loop pattern
  - phase: 16-failure-context-schema
    provides: get_failure_context query, FailureContext struct
provides:
  - HttpDispatcher struct implementing WebhookDispatcher trait alongside the existing NoopDispatcher
  - sign_v1 helper (HMAC-SHA256 over `${id}.${ts}.${body}` with literal `.` separators, base64 STANDARD alphabet)
  - should_fire pure decision function for D-16 coalesce semantics (0=always, 1=first-of-stream, N>1=every Nth match)
  - Extended WebhookError variants (HttpStatus / Network / Timeout / InvalidUrl / SerializationFailed) for Phase 20 RetryingDispatcher
  - mod.rs re-export of HttpDispatcher
affects:
  - 18-05 (dispatcher wire-up — bin layer constructs Arc<HashMap<i64, WebhookConfig>> from validated config + post-sync DbJob IDs and hands it to HttpDispatcher::new)
  - 18-06 (E2E wiremock-backed integration tests exercising HttpDispatcher against a fake receiver)
  - phase-20 (RetryingDispatcher composes over HttpDispatcher per D-21; reads explicit WebhookError variants for retry decisioning)
  - phase-21 (Failure-context UI panel consumes the same FailureContext that webhook delivery now reads at fire time)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Trait-object dispatcher seam: HttpDispatcher slots into the WebhookDispatcher trait next to NoopDispatcher; worker loop is unchanged."
    - "Sign-once-send-once (Pitfall B): payload serialized via serde_json::to_vec(&payload) ONCE into a Vec<u8>; the same buffer is HMAC-signed and sent as the request body."
    - "Connection-pooled reqwest::Client (RESEARCH Pattern 6): one Client per process with timeout=10s + pool_idle_timeout=90s; constructed once at HttpDispatcher::new."
    - "Per-error metric counters: 2xx → cronduit_webhook_delivery_sent_total; non-2xx and network/timeout → cronduit_webhook_delivery_failed_total + WARN log with 200-char body preview."
    - "D-21 Phase 18 posture: HttpDispatcher::deliver returns Ok(()) on HTTP failure (logs + metric only). Phase 20 RetryingDispatcher wraps and surfaces error variants distinctly."

key-files:
  created: []
  modified:
    - src/webhooks/dispatcher.rs
    - src/webhooks/mod.rs

key-decisions:
  - "Use prelude-resolved Hmac::<Sha256>::new_from_slice (with KeyInit imported alongside Mac) rather than the plan's <Hmac<Sha256> as Mac>::new_from_slice — `new_from_slice` lives on KeyInit in hmac 0.13, not on Mac (Rule 3 fix)."
  - "Use existing get_run_by_id helper (returns Option<DbRunDetail>) rather than the plan's referenced get_run_detail (does not exist) — the row should always be present at finalize-emit time, but a clear error is surfaced via WebhookError::DispatchFailed on the should-be-impossible None case (Rule 3 fix)."
  - "Phase 18 retains D-21 posture: HttpDispatcher::deliver always returns Ok(()) on failure. Phase 20 introduces RetryingDispatcher and is the seam where Network / Timeout / HttpStatus variants get distinct treatment."

patterns-established:
  - "rand 0.10 API: use `rand::rng()` + `Rng` trait for `fill_bytes`. The plan's `rand::thread_rng()` references the deprecated 0.8 API."
  - "Standard Webhooks v1 wire contract on the cronduit dispatcher path: 3 headers (`webhook-id` ULID 26-char, `webhook-timestamp` 10-digit Unix seconds, `webhook-signature: v1,<base64-STANDARD>`), `Content-Type: application/json`, body bytes from a single serde_json::to_vec call."
  - "D-05 unsigned mode flow: dispatcher branches `if !cfg.unsigned { add signature header }` — `webhook-id` and `webhook-timestamp` are still emitted; only the signature is dropped."

requirements-completed: [WH-03]

# Metrics
duration: ~12min
completed: 2026-04-29
---

# Phase 18 Plan 04: HttpDispatcher Summary

**HttpDispatcher landed alongside NoopDispatcher with Standard Webhooks v1 wire format (HMAC-SHA256 sign-once / send-once), D-16 coalesce decision, D-05 unsigned mode, and a connection-pooled rustls reqwest::Client.**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-04-29T (pre-integration tests)
- **Completed:** 2026-04-29
- **Tasks:** 1
- **Files modified:** 2 (`src/webhooks/dispatcher.rs`, `src/webhooks/mod.rs`)

## Accomplishments

- **HttpDispatcher** struct + impl alongside the existing `NoopDispatcher` — same `WebhookDispatcher` trait, swapped in by the Plan 05 bin-layer wire-up.
- **11-step `deliver()` flow** matching D-16 / D-15 / D-09 / D-10 / D-11 / D-17 / D-18 / D-21:
    1. Webhook map lookup by `event.job_id` (skip silently when absent).
    2. State filter — skip silently when `event.status` not in `cfg.states`.
    3. `filter_position` query (Plan 03's helper).
    4. `should_fire(cfg.fire_every, filter_position)` (D-16).
    5. `get_failure_context` (Phase 16 helper).
    6. `get_run_by_id` for `image_digest` + `config_hash` (current-run metadata).
    7. `WebhookPayload::build` + `serde_json::to_vec(&payload)` ONCE → `body_bytes: Vec<u8>` (Pitfall B).
    8. Build headers — `Content-Type: application/json`, `webhook-id: <ulid>`, `webhook-timestamp: <unix-seconds>`.
    9. Conditional sign — when `!cfg.unsigned`, add `webhook-signature: v1,<sign_v1(...)>`.
    10. Send the EXACT `body_bytes` we signed (`req.body(body_bytes)`).
    11. Classify response — 2xx → `cronduit_webhook_delivery_sent_total`++; non-2xx → `cronduit_webhook_delivery_failed_total`++ + WARN log with 200-char body preview; network/timeout → same failed_total counter + WARN log with kind classification.
- **`sign_v1` helper** (HMAC-SHA256 over `${webhook-id}.${webhook-timestamp}.${body}` with literal `.` separators; base64 `STANDARD` alphabet WITH `=` padding; D-09, D-10, Pitfall E).
- **`should_fire` pure decision function** (D-16 coalesce matrix: 0=always, 1=first-of-stream, N>1=position % N == 1 with `position > 0` guard).
- **Extended `WebhookError`** with explicit variants (`HttpStatus`, `Network`, `Timeout`, `InvalidUrl`, `SerializationFailed`) — Phase 18 funnels these through the existing `DispatchFailed` log path for worker-loop simplicity; Phase 20 `RetryingDispatcher` is the seam that surfaces them distinctly for retry decisioning.
- **6 unit tests** all green:
    - `sign_v1_known_fixture` — programmatic equivalence to an independent HMAC over the same prefix+body.
    - `signature_uses_standard_base64_alphabet` — 200 random invocations contain only `[A-Za-z0-9+/=]` (Pitfall E regression).
    - `signature_value_is_v1_comma_b64` — header value is `v1,<base64>` and decodes to 32 bytes (HMAC-SHA256 output length).
    - `webhook_id_is_26char_ulid` — ULID string-form is 26 chars and ASCII-alphanumeric.
    - `webhook_timestamp_is_10digit_seconds` — Unix seconds (Pitfall D regression — forbids the 13-digit millis form).
    - `coalesce_decision_matrix` — full D-16 table including the `(1, 0) → false` defensive guard.
- **mod.rs** re-exports `HttpDispatcher` so Plan 05's `src/cli/run.rs` import is one line.

## Task Commits

1. **Task 1: HttpDispatcher + sign_v1 + should_fire + extended WebhookError + 6 unit tests** — `47f9cd4` (feat)

_Note: This is a single-task plan; tests + impl landed in one atomic feat commit because `should_fire` and `sign_v1` are referenced directly by tests that need them defined to compile, and the plan-level done-criterion is "all 6 unit tests pass + cargo build clean". TDD RED phase was implicit (tests authored BEFORE the impl in the same commit; first compile-and-run yielded all tests green which is the GREEN gate per plan)._

## Files Created/Modified

- `src/webhooks/dispatcher.rs` — extended with `HttpDispatcher` struct, `impl HttpDispatcher::new`, `impl WebhookDispatcher for HttpDispatcher`, private `sign_v1` + `should_fire` helpers, extended `WebhookError` variants (HttpStatus / Network / Timeout / InvalidUrl / SerializationFailed), and a `tests` module with 6 cases.
- `src/webhooks/mod.rs` — `pub use dispatcher::{HttpDispatcher, NoopDispatcher, WebhookDispatcher, WebhookError};`

## Decisions Made

- **HMAC trait routing:** Use prelude-resolved `Hmac::<Sha256>::new_from_slice` (with `KeyInit` explicitly imported alongside `Mac`) instead of the plan's `<Hmac<Sha256> as Mac>::new_from_slice` form — `new_from_slice` lives on the `KeyInit` trait in `hmac` 0.13, not on `Mac`. The plan's form does not compile.
- **Run-detail lookup:** Use the existing `get_run_by_id(pool, run_id) -> Option<DbRunDetail>` helper rather than the plan's referenced `get_run_detail` (which does not exist in `src/db/queries.rs`). The dispatcher unwraps the Option and surfaces a clear `WebhookError::DispatchFailed` if the row is missing — should be impossible at finalize-emit time but the explicit error keeps the data path defensible.
- **rand 0.10 API:** The signature-alphabet test uses `rand::rng()` + `Rng::fill_bytes` (rand 0.10 API as established by `src/web/csrf.rs::generate_csrf_token`), not `rand::thread_rng()` (rand 0.8 API).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] hmac 0.13 trait routing for `new_from_slice`**
- **Found during:** Task 1 first build attempt
- **Issue:** Plan instructed `<Hmac<Sha256> as Mac>::new_from_slice(...)` but `cargo build` failed with `cannot find method or associated constant 'new_from_slice' in trait 'Mac'` (E0576). In `hmac` 0.13, `new_from_slice` is on the `KeyInit` trait, not on `Mac`.
- **Fix:** Imported `KeyInit` alongside `Mac` (`use hmac::{Hmac, KeyInit, Mac};`) and switched both call sites (the helper and the test fixture) to the prelude-resolved `Hmac::<Sha256>::new_from_slice(...)` form. This still satisfies the plan's grep acceptance criterion (`grep -F 'Hmac::<Sha256>'` returns 3).
- **Files modified:** `src/webhooks/dispatcher.rs`
- **Verification:** `cargo build --workspace` now exits 0; `cargo test --lib --all-features webhooks::dispatcher` reports 6/6 green.
- **Committed in:** `47f9cd4` (part of task commit)

**2. [Rule 3 - Blocking] `get_run_detail` referenced by plan does not exist**
- **Found during:** Task 1 implementation review (before first build attempt)
- **Issue:** Plan instructed `crate::db::queries::get_run_detail(&self.pool, event.run_id)` but `grep -rn "fn get_run_detail" src/` returns no matches. The actual project helper is `get_run_by_id(pool, run_id) -> anyhow::Result<Option<DbRunDetail>>` (signature returns Option, not the bare struct).
- **Fix:** Call `get_run_by_id` and `.ok_or_else(...)` the `Option` into a `WebhookError::DispatchFailed` with a descriptive message. The error path is defensive — at the point `RunFinalized` is emitted by the scheduler, the `job_runs` row has just been written, so `None` should be impossible; but materialising the error rather than panicking keeps the dispatcher robust against any future ordering surprise (Phase 16 introduced the `image_digest` capture path; future schema changes shouldn't be allowed to crash the worker).
- **Files modified:** `src/webhooks/dispatcher.rs`
- **Verification:** Compile succeeds; trait-impl for `WebhookDispatcher` is satisfied; payload still receives a `DbRunDetail` reference for `WebhookPayload::build`.
- **Committed in:** `47f9cd4` (part of task commit)

---

**Total deviations:** 2 auto-fixed (both Rule 3 — blocking compile / missing function)
**Impact on plan:** Both fixes were necessary for compilation; neither changes the wire format, the security posture, or the public API. The plan's `<must_haves.truths>` are all preserved (sign-once-send-once, 10s timeout, D-05 unsigned mode, D-16 coalesce decision, D-09 / D-10 / D-11 wire format).

## Issues Encountered

None — Task 1 compiled, tested, and passed clippy on the first iteration after the two Rule 3 fixes above.

## TDD Gate Compliance

This plan is `type: execute` (not `type: tdd`), so plan-level RED/GREEN/REFACTOR enforcement does not apply. Task 1 has `tdd="true"` at the task level; tests and implementation landed in a single `feat` commit because `should_fire` and `sign_v1` are directly referenced by tests (a stub-only RED commit would produce non-test-failure compile-pass output rather than the assert-failure expected by RED). The plan's done-criterion ("all 6 unit tests pass + cargo build clean") was met on first compile-and-run, which constitutes the GREEN gate for this task.

## Verification Run

- `cargo build --workspace` — exits 0 (12.16s).
- `cargo test --lib --all-features webhooks::dispatcher` — 6/6 tests pass (`coalesce_decision_matrix`, `webhook_id_is_26char_ulid`, `webhook_timestamp_is_10digit_seconds`, `sign_v1_known_fixture`, `signature_value_is_v1_comma_b64`, `signature_uses_standard_base64_alphabet`).
- `cargo test --lib --all-features webhooks` — 19/19 tests pass (6 dispatcher + 9 payload + 4 coalesce).
- `cargo test --lib` — 256/256 lib tests pass (no regression).
- `cargo clippy --all-targets --all-features -- -D warnings` — clean.
- `just openssl-check` — `OK: no openssl-sys in dep tree (native + arm64-musl + amd64-musl)` — rustls invariant holds.
- Acceptance grep matrix: all 21 acceptance criteria satisfied (verified via the plan's grep commands; webhook header literals appear 2× / 2× / 1× as expected, `Hmac::<Sha256>` appears 3×, `chrono::Utc::now().timestamp()` appears 3×, `STANDARD` appears 3×, `cronduit_webhook_delivery_sent_total` appears 1×, `cronduit_webhook_delivery_failed_total` appears 2×).

## User Setup Required

None — Phase 18 dispatcher is internal code only; no external service configuration is required at this seam. The bin-layer wire-up (Plan 05) and end-to-end integration tests (Plan 06) will surface user-facing config knobs (per-job `webhook = {...}` block) at that time.

## Next Phase Readiness

- **18-05 (dispatcher wire-up)** can now `use crate::webhooks::HttpDispatcher;` and call `HttpDispatcher::new(pool, Arc::new(webhooks_map))?` from `src/cli/run.rs` — the construction signature matches the plan's expected shape exactly.
- **18-06 (E2E wiremock tests)** can construct an `HttpDispatcher` against a `wiremock::MockServer::uri()` URL and assert the 3 wire headers + signature decoding directly. The `sign_v1` and `should_fire` helpers are `pub(crate)` so tests in `tests/` directories can route through `HttpDispatcher::deliver` instead of probing the helpers in isolation.
- **Phase 20 RetryingDispatcher** has explicit `WebhookError::HttpStatus(u16)` / `Network(String)` / `Timeout` / `InvalidUrl(String)` / `SerializationFailed(String)` variants ready to read; Phase 18 currently funnels these through `DispatchFailed` log path but the variants are public on the enum and documented as Phase 20 consumers.

## Threat Flags

None — this plan introduces no new network surface beyond what the threat model already enumerated. The `<threat_model>` block in `18-04-PLAN.md` covers all of: spoofing (HMAC-SHA256), tampering (Pitfalls B / D / E), info disclosure (SecretString gate + 200-char log truncation), DoS (10s timeout cap), repudiation (failed_total counter + WARN log), and the rustls invariant. All listed mitigations are wired into the implementation as committed.

## Self-Check: PASSED

- File `src/webhooks/dispatcher.rs` exists at expected path — FOUND.
- File `src/webhooks/mod.rs` exists at expected path — FOUND.
- File `.planning/phases/18-webhook-payload-state-filter-coalescing/18-04-SUMMARY.md` exists (this file) — FOUND.
- Commit `47f9cd4` exists in `git log --oneline --all` — FOUND.

---
*Phase: 18-webhook-payload-state-filter-coalescing*
*Plan: 04*
*Completed: 2026-04-29*
