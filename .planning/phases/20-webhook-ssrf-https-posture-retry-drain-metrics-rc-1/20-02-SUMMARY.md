---
phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
plan: 02
subsystem: webhooks
tags: [webhooks, retry, dispatcher, classification, jitter, retry-after, dlq, tokio-select, cancel-token]

# Dependency graph
requires:
  - phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
    plan: 01
    provides: WebhookDlqRow + insert_webhook_dlq_row + webhook_deliveries schema + 7 Wave 0 stub files
  - phase: 18-webhook-http-dispatch-hmac-signing
    provides: HttpDispatcher + WebhookDispatcher trait + WebhookError enum (with pre-allowed variants)
provides:
  - "RetryingDispatcher<D: WebhookDispatcher> composition newtype in src/webhooks/retry.rs"
  - "WebhookError::HttpStatus reshaped to struct variant { code, retry_after: Option<Duration> } (B1 fix)"
  - "End-to-end Retry-After honoring with cap_for_slot bounds (D-07/D-08)"
  - "Cancel-aware retry-sleep boundary writes shutdown_drain DLQ row (D-03)"
  - "DLQ row url-column lookup via shared Arc<HashMap<i64, WebhookConfig>> (B2 fix; mandates Plan 06 Option B path)"
  - "5 pub helpers reachable from integration tests: jitter, cap_for_slot, classify, parse_retry_after_from_response, DlqReason::as_str"
affects: [20-04, 20-06]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Composition newtype wrapper pattern (P18 D-21): RetryingDispatcher<D: WebhookDispatcher> impls WebhookDispatcher via trait composition. The wrapped trait stays at two lines — no trait expansion."
    - "Cancel-aware tokio::select! on every retry-sleep boundary: select! { biased; _ = sleep(d) => continue, _ = self.cancel.cancelled() => write_dlq_then_return_err }. Inner deliver is NOT cancel-wrapped (Pitfall 1) — bounded by reqwest's 10s per-attempt timeout."
    - "Paused tokio clock + select+driver test pattern: setup uses real time (DbPool, MockServer); tokio::time::pause() activates just before dispatch; a driver loop alternates yield_now + advance(500ms) to drain virtual time across schedule sleeps while dispatcher's HTTP I/O completes against in-process wiremock. Whole 11-test suite executes in ~100ms wall time despite 5+ minutes of virtual schedule time."
    - "rand 0.10 global free function for jitter: rand::random::<f64>() * 0.4 + 0.8 (Pitfall 2 — NOT gen_range / 0.9-era API)"

key-files:
  created:
    - "src/webhooks/retry.rs (618 lines: 14 in-module unit tests + RetryingDispatcher impl + 5 pub helpers + DlqReason enum + Classification enum)"
  modified:
    - "src/webhooks/dispatcher.rs (WebhookError::HttpStatus reshaped; non-2xx arm + Err arm now bubble Err variants; #[allow(dead_code)] removed from 4 variants)"
    - "src/webhooks/mod.rs (pub mod retry + pub use retry::RetryingDispatcher)"
    - "tests/v12_webhook_retry.rs (Wave 0 stub → 1 #[tokio::test])"
    - "tests/v12_webhook_retry_classification.rs (Wave 0 stub → 2 #[tokio::test])"
    - "tests/v12_webhook_retry_after.rs (Wave 0 stub → 5 tests including B1 + T-20-03 regression locks)"
    - "tests/v12_webhook_dlq.rs (Wave 0 stub → 3 tests including B2 regression lock)"
    - "tests/v12_webhook_failed_metric.rs (handle now-Err return from non-2xx; counter delta is the test's truth)"
    - "tests/v12_webhook_network_error_metric.rs (handle now-Err return from network failure; counter delta is the test's truth)"

key-decisions:
  - "RetryingDispatcher::new takes 4 args: inner, pool, cancel, webhooks. The 4th arg (webhooks: Arc<HashMap<i64, WebhookConfig>>) is the B2 fix — write_dlq looks up url at write time so the DLQ row's url column matches the configured webhook URL. Plan 06 must wire the SAME Arc into both HttpDispatcher::new and RetryingDispatcher::new (Option B path locked)."
  - "WebhookError::HttpStatus is a struct variant { code, retry_after: Option<Duration> } (B1 fix). HttpDispatcher parses Retry-After BEFORE consuming response body (header read non-destructive on owned response), then returns Err(HttpStatus { code, retry_after }). RetryingDispatcher's compute_sleep_delay consumes retry_after for end-to-end honoring."
  - "Schedule [0s, 30s, 300s] is hardcoded into RetryingDispatcher::new — exactly 3 attempts. Not configurable in v1.2 (operator UX simplicity). Plan locked this; future configurability is a v1.3 candidate."
  - "Inner deliver is NOT in tokio::select! (Pitfall 1) — in-flight HTTP requests run to completion bounded by reqwest's existing 10s per-attempt timeout (P18 D-18). Cancel checks happen ONLY on the sleep-between-attempts boundary."
  - "compute_sleep_delay formula: retry_after Some(ra) → min(cap_for_slot(prev_slot), max(jitter(schedule[i]), ra)); None → jitter(schedule[i]). Cap clamps even when jitter floor > Retry-After (clamps the max-of-the-pair)."
  - "DLQ INSERT failure is logged at WARN and the worker continues — never crashes (RESEARCH §4.8). The metric is the source of truth; the DLQ is the audit trail."

requirements-completed: [WH-05]

# Metrics
duration: 23min
completed: 2026-05-01
---

# Phase 20 Plan 02: RetryingDispatcher (3-attempt retry chain + Retry-After + DLQ) Summary

**Locked the heart of Phase 20 / WH-05: composed HttpDispatcher with a 3-attempt in-memory retry chain, end-to-end Retry-After honoring within cap_for_slot bounds, cancel-aware sleep boundaries, and DLQ row writes with correct url-column lookup. Reshaped WebhookError::HttpStatus to carry the receiver's Retry-After hint (B1 fix) and wired the webhooks Arc through RetryingDispatcher::new for DLQ url lookup at write time (B2 fix). 11 new integration tests + 14 in-module unit tests, all green; full webhook suite (491 integration + 271 lib tests) passes.**

## Performance

- **Duration:** ~23 min
- **Started:** 2026-05-01T20:07:02Z
- **Completed:** 2026-05-01T20:30:03Z
- **Tasks:** 3
- **Files modified:** 9 (1 created, 8 modified)
- **`src/webhooks/retry.rs` final size:** 618 lines (well above 250 minimum)

## Accomplishments

- `WebhookError::HttpStatus` reshaped from positional `(u16)` to struct variant `{ code, retry_after: Option<Duration> }` — B1 regression fix; the receiver's Retry-After hint now flows end-to-end through the dispatcher into the retry chain.
- `HttpDispatcher::deliver` non-2xx arm parses `Retry-After` BEFORE consuming the response body (non-destructive header read on the owned response) and returns `Err(WebhookError::HttpStatus { code, retry_after })` instead of swallowing as `Ok(())`. `Err(e)` arm splits on `is_timeout()` → `WebhookError::Timeout` else `WebhookError::Network(msg)`.
- `RetryingDispatcher<D: WebhookDispatcher>` composes any dispatcher with the locked schedule `[0s, 30s, 300s]` — exactly 3 attempts. Composition newtype wrapper preserves the two-line trait (P18 D-21 invariant) — the worker never learns about retries.
- 5 `pub` helpers reachable from integration tests (W2 visibility fix): `jitter(base)`, `cap_for_slot(slot, schedule)`, `classify(&err)`, `parse_retry_after_from_response(headers, url, status)`, `DlqReason::as_str()`.
- Cancel-aware retry-sleep boundary: `tokio::select! { biased; sleep(d) => continue, cancel.cancelled() => write_dlq_shutdown_drain }`. On SIGTERM mid-chain a `shutdown_drain` DLQ row is written with the actual attempts count (NOT always 3) BEFORE returning `Err(DispatchFailed("shutdown drain"))`.
- DLQ row writes are correctness-critical: `write_dlq` looks up `url` from `Arc<HashMap<i64, WebhookConfig>>` keyed by `event.job_id` (B2 fix). The `unwrap_or_default` fallback handles the should-not-happen race where webhook config was removed mid-run; emits a WARN log so operators see it. DLQ INSERT failure is logged at WARN — never crashes the worker (RESEARCH §4.8).
- 14 in-module unit tests (jitter range, cap_for_slot table, classification table, compute_sleep_delay with/without Retry-After + cap clamp, parse_retry_after integer/HTTP-date/negative/missing, dlq_reason strings, truncate_error under/over limit) — all green.
- 4 integration test files with 11 new `#[tokio::test]` cases including B1 + T-20-03 + B2 regression locks — all green.

## Task Commits

Each task was committed atomically:

1. **Task 1: Reshape WebhookError::HttpStatus + populate Err variants in HttpDispatcher** — `8928f90` (feat)
2. **Task 2: Implement RetryingDispatcher<D> in src/webhooks/retry.rs + module wiring (with rustfmt roll for dispatcher.rs HttpStatus declaration)** — `65ca5c0` (feat)
3. **Task 3: Append integration tests to v12_webhook_{retry,retry_classification,retry_after,dlq}.rs Wave 0 stubs** — `5cac096` (test)

## Files Created/Modified

**Created:**
- `src/webhooks/retry.rs` — 618 lines: imports + `DlqReason` enum + `Classification` enum + `classify()` + `jitter()` + `cap_for_slot()` + `parse_retry_after_from_response()` + `compute_sleep_delay()` + `truncate_error()` + `RetryingDispatcher<D>` struct + `new()` + `write_dlq()` + `WebhookDispatcher` impl with retry loop + 14 in-module `#[cfg(test)]` unit tests.

**Modified:**
- `src/webhooks/dispatcher.rs` — `WebhookError::HttpStatus` reshaped from `HttpStatus(u16)` to struct variant `HttpStatus { code: u16, retry_after: Option<std::time::Duration> }`; `#[allow(dead_code)]` removed from `HttpStatus`/`Network`/`Timeout`/`InvalidUrl` variants (4 attributes deleted); `Ok(resp)` non-2xx arm now parses `Retry-After` via `crate::webhooks::retry::parse_retry_after_from_response` BEFORE consuming body, returns `Err(WebhookError::HttpStatus { code, retry_after })`; `Err(e)` arm splits on `is_timeout()` → `Err(WebhookError::Timeout)` else `Err(WebhookError::Network(format!("{e}")))`.
- `src/webhooks/mod.rs` — added `pub mod retry;` and `pub use retry::RetryingDispatcher;`.
- `tests/v12_webhook_retry.rs` — Wave 0 stub replaced with harness + 1 `#[tokio::test]` (`three_attempts_at_locked_schedule_under_paused_clock`).
- `tests/v12_webhook_retry_classification.rs` — Wave 0 stub replaced with harness + 2 `#[tokio::test]` (`four_oh_four_writes_one_dlq_row_no_retry`, `four_oh_eight_retries_per_schedule`).
- `tests/v12_webhook_retry_after.rs` — Wave 0 stub replaced with harness + 5 tests (`receiver_429_with_retry_after_header_extends_sleep_to_hint_within_cap` (B1 lock), `receiver_429_with_retry_after_9999_is_capped` (T-20-03 lock), `receiver_200_no_sleep`, `cap_for_slot_matches_research_table`, `parse_retry_after_integer_seconds_only`).
- `tests/v12_webhook_dlq.rs` — Wave 0 stub replaced with harness + 3 tests (`dlq_row_no_payload_no_signature_columns`, `dlq_reasons_table_coverage`, `dlq_url_matches_configured_url` (B2 lock)).
- `tests/v12_webhook_failed_metric.rs` — `dispatcher.deliver(&event).await.unwrap()` → `let _ = dispatcher.deliver(&event).await;` (Phase 20 dispatcher returns Err on non-2xx so retry chain can decide; counter delta is the test's truth).
- `tests/v12_webhook_network_error_metric.rs` — same fix.

## Decisions Made

None new — followed plan and CONTEXT D-01..D-12 + RESEARCH §4.7 / §13.1 / §13.3 verbatim. The plan locked all material decisions before execution started.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Cargo build of Task 1 alone does not pass standalone**

- **Found during:** Task 1 verification.
- **Issue:** Task 1 introduces a call to `crate::webhooks::retry::parse_retry_after_from_response` in dispatcher.rs but the symbol does not exist until Task 2 creates retry.rs.
- **Fix:** Plan acknowledges this in the Task 1 acceptance criteria: "cargo check --lib exits 0 (NOTE: this requires Task 2's parse_retry_after_from_response to exist; Tasks 1+2 land together as one PR)". Committed Task 1 with the dependency clearly noted in the commit message; verified `cargo check --lib` passes after Task 2's retry.rs commit. No deviation from plan intent — only an artifact of atomic per-task commits.
- **Files modified:** None additional.
- **Commit:** `8928f90` (Task 1) + `65ca5c0` (Task 2) close the loop.

**2. [Rule 1 - Bug] Existing webhook tests broken by HttpStatus reshape**

- **Found during:** Task 1 implementation (cargo check during validation).
- **Issue:** Tests `tests/v12_webhook_failed_metric.rs::webhook_failed_metric_increments_failed_total_on_non_2xx` and `tests/v12_webhook_network_error_metric.rs::webhook_network_error_increments_failed_total` called `.unwrap()` on the dispatcher's result — broken when the non-2xx / network paths started returning `Err` per the reshape.
- **Fix:** Replaced `.unwrap()` with `let _ = ...await;`. Both tests assert counter deltas (the truth of these tests is the metric increment, not the return value). Documented inline with a Phase 20 / WH-05 comment.
- **Files modified:** `tests/v12_webhook_failed_metric.rs`, `tests/v12_webhook_network_error_metric.rs`.
- **Commit:** `8928f90` (bundled with Task 1).

**3. [Rule 3 - Blocking] Test approach: `#[tokio::test(start_paused = true)]` hangs the SQLite pool**

- **Found during:** Task 3 first test execution.
- **Issue:** Plan suggested `#[tokio::test(flavor = "current_thread", start_paused = true)]`. With `start_paused = true`, `DbPool::connect("sqlite::memory:")` hangs at the connection-pool acquire-timeout because tokio timers don't tick. Even after splitting setup before/after `tokio::time::pause()`, `current_thread` runtime starves the spawned dispatcher task when the test thread blocks on `task.await`.
- **Fix:** Replaced the plan's suggested pattern with a `tokio::select!` + driver loop pattern: `select! { r = dispatcher.deliver() => r, _ = async { loop { yield_now(); advance(500ms) } } => unreachable!() }`. Setup runs in real time; `tokio::time::pause()` activates just before dispatch; the driver loop alternates yields (let HTTP I/O complete via wiremock on the same runtime) with virtual-time advances (skip schedule sleeps). Whole 11-test suite executes in ~100ms wall time despite 5+ minutes of virtual schedule time.
- **Files modified:** All 4 plan-02 integration test files.
- **Commit:** `5cac096` (Task 3).

**4. [Rule 3 - Blocking] Missing `use sqlx::Row;` in `tests/v12_webhook_retry_after.rs`**

- **Found during:** First `cargo nextest run --test v12_webhook_retry_after`.
- **Issue:** Compilation error `no method named 'get' found for SqliteRow`. The test calls `.get("id")` on a sqlx::Row but I forgot to import the trait.
- **Fix:** Added `use sqlx::Row;` to the imports.
- **Files modified:** `tests/v12_webhook_retry_after.rs`.
- **Commit:** `5cac096` (Task 3).

## Issues Encountered

None at the implementation level — all Rules 1/3 fixes above were anticipated by the plan or trivially mechanical.

The `tokio::time::pause()` test pattern took some experimentation to land correctly. Three approaches were tried before the select+driver pattern stuck:
1. `start_paused = true` — fails: SQLite pool hangs.
2. Manual pause + sequential advances with yield_now — fails: only 2-of-3 attempts complete because the spawned dispatcher task doesn't get enough cycles.
3. `multi_thread` flavor — fails: `tokio::time::pause()` requires `current_thread`.
4. **Select+driver loop pattern — works:** dispatcher and driver race; dispatcher wins; driver runs as long as needed in virtual time.

This pattern is documented in the SUMMARY's tech-stack patterns and inline test comments so future plans can reuse it.

## Verification Run

```
cargo check --lib                            # PASS (warning only: tailwind binary not built)
cargo nextest run --lib webhooks             # 34/34 PASS (14 retry + 7 dispatcher + 13 others)
cargo nextest run --lib                      # 271/271 PASS
cargo nextest run --tests                    # 491/491 PASS (28 skipped — feature-gated postgres tier)
cargo nextest run --test v12_webhook_retry --test v12_webhook_retry_classification \
                  --test v12_webhook_retry_after --test v12_webhook_dlq
                                             # 11/11 PASS
cargo clippy --lib --tests -- -D warnings    # PASS (no new warnings)
cargo fmt -- --check                         # PASS (after rustfmt-applied dispatcher.rs HttpStatus expansion bundled into Task 2)
cargo tree -i openssl-sys                    # "did not match any packages" (D-38 invariant intact)
```

## Threat Model Mitigations Applied

- **T-20-03 (DoS via Retry-After amplification):** `cap_for_slot(prev_slot, &schedule) = schedule[prev_slot+1] * 1.2` keeps the locked schedule's per-slot worst case bounded. Receiver-controlled `Retry-After: 9999` cannot blow past the cap. End-to-end regression-locked by `tests/v12_webhook_retry_after.rs::receiver_429_with_retry_after_9999_is_capped` (asserts virtual chain duration ≤ 450s, NOT 19998s).
- **T-20-04 (Reliability / mid-chain SIGTERM-loss):** Cancel-aware `tokio::select!` at every retry-sleep boundary writes a DLQ row with `dlq_reason='shutdown_drain'` BEFORE returning `Err`. Operators query `WHERE dlq_reason='shutdown_drain'` to find SIGTERM-loss subset.
- **T-20-02 (Information Disclosure / receiver-supplied error messages):** `truncate_error` caps `last_error` at 500 chars at the call site. No payload bytes ever stored in DLQ schema (Plan 01 enforced; Plan 02 unit-tested by `dlq_row_no_payload_no_signature_columns`).
- **T-20-07 (Audit Trail Integrity / DLQ url column):** `RetryingDispatcher::write_dlq` looks up url from `Arc<HashMap<i64, WebhookConfig>>` keyed by `job_id`. End-to-end regression-locked by `tests/v12_webhook_dlq.rs::dlq_url_matches_configured_url` (B2 lock).

## Threat Flags

None — this plan modifies an existing dispatcher to bubble Err variants and adds an in-memory composition wrapper. No new network endpoints, no new auth paths, no file access pattern changes, no schema changes at trust boundaries (Plan 01 owns the schema).

## Next Phase Readiness

- **Plan 03 (HTTPS validator):** No coupling to Plan 02. Independent.
- **Plan 04 (drain budget):** Will compose with `RetryingDispatcher` — drain logic at worker shutdown will call `cancel.cancel()` on the shared CancellationToken; Plan 02's cancel-aware sleep boundary writes the `shutdown_drain` DLQ row.
- **Plan 05 (metrics labeled family):** Will replace the unlabeled `cronduit_webhook_delivery_sent_total`/`failed_total` counters in dispatcher.rs lines 257/267/289 — Plan 02 left those alone per plan instructions.
- **Plan 06 (worker wire-up):** **MUST use the Option B path** — wire the SAME `Arc<HashMap<i64, WebhookConfig>>` into both `HttpDispatcher::new` and `RetryingDispatcher::new`. The existing `src/cli/run.rs` builds the HashMap; Plan 06 wraps the HttpDispatcher in `RetryingDispatcher::new(http, pool, cancel, webhooks_arc.clone())`.

No blockers or concerns.

## Self-Check: PASSED

Verified files exist:
- FOUND: src/webhooks/retry.rs (618 lines)
- MODIFIED: src/webhooks/dispatcher.rs (HttpStatus reshape + Err propagation)
- MODIFIED: src/webhooks/mod.rs (pub mod retry + re-export)
- MODIFIED: tests/v12_webhook_retry.rs (1 #[tokio::test])
- MODIFIED: tests/v12_webhook_retry_classification.rs (2 #[tokio::test])
- MODIFIED: tests/v12_webhook_retry_after.rs (5 tests)
- MODIFIED: tests/v12_webhook_dlq.rs (3 tests)
- MODIFIED: tests/v12_webhook_failed_metric.rs (handle now-Err return)
- MODIFIED: tests/v12_webhook_network_error_metric.rs (handle now-Err return)

Verified commits exist:
- FOUND: 8928f90 (Task 1 — dispatcher.rs reshape)
- FOUND: 65ca5c0 (Task 2 — retry.rs + module wiring)
- FOUND: 5cac096 (Task 3 — 4 integration test files)

---
*Phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1*
*Plan: 02*
*Completed: 2026-05-01*
