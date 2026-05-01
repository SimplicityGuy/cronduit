---
phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
plan: 05
subsystem: webhooks
tags: [webhooks, metrics, prometheus, telemetry, labeled-counter, histogram, gauge, breaking-change]

# Dependency graph
requires:
  - phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
    plan: 02
    provides: RetryingDispatcher chain-success and chain-terminal-failure boundaries (D-22 increment sites)
  - phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
    plan: 01
    provides: Wave 0 stub `tests/v12_webhook_metrics_family.rs` (PHASE_MARKER replaced with 6 #[test] cases)
  - phase: 18-webhook-http-dispatch-hmac-signing
    provides: P18 unlabeled `_sent_total` and `_failed_total` flat counters (now REMOVED per D-22)
  - phase: 15-foundation-preamble
    provides: P15 `cronduit_webhook_delivery_dropped_total` channel-saturation counter (PRESERVED per D-26)
provides:
  - "cronduit_webhook_deliveries_total{job, status} labeled per-DELIVERY counter (closed-enum status ∈ {success, failed, dropped})"
  - "cronduit_webhook_delivery_duration_seconds{job} per-attempt HTTP histogram with operator-tuned buckets [0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]"
  - "cronduit_webhook_queue_depth gauge zero-baselined at boot (live updates land in Plan 04 worker.rs wiring)"
  - "src/telemetry.rs eager describe + zero-baseline + status-only seed loop for the new family"
  - "Closed-enum `status` invariant locked: `&'static str` literals only at all 3 increment sites in retry.rs"
  - "tests/v12_webhook_metrics_family.rs: 6 integration tests locking the boot-described HELP/TYPE/zero-baseline/bucket/queue-depth/P15-preserve contract"
affects: [20-04, 20-06, 20-07]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Per-DELIVERY metric site at OUTER RetryingDispatcher boundary, NOT per-ATTEMPT inside HttpDispatcher (D-22). The success/failed status is the OUTCOME of the full retry chain, not the outcome of each HTTP attempt. The histogram still records per-attempt at the HttpDispatcher boundary because measuring wire-level round-trip is the metric's truth."
    - "Closed-enum label invariant: `status` label values are `&'static str` literals only — `\"success\"`, `\"failed\"`, `\"dropped\"`. NEVER set from response status codes or runtime strings (T-20-05 cardinality mitigation, Pitfall 5). Reason granularity (4xx vs 5xx vs network vs timeout) lives in the `webhook_deliveries.dlq_reason` SQL audit column, NOT in metric labels."
    - "Histogram bucket configuration via `PrometheusBuilder::set_buckets_for_metric(Matcher::Full(name), &[boundaries])` chained on the builder (mirror of the existing `cronduit_run_duration_seconds` pattern at telemetry.rs:67-71). Operator-tuned 8-bucket array `[0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]` per RESEARCH §4.4: 50ms..10s; the 10s top matches reqwest's per-attempt timeout (P18 D-18)."
    - "Status-only zero-baseline seed loop (no `job` label) in `setup_metrics()` mirrors the `cronduit_runs_total` precedent at telemetry.rs:172-182. Per-job × per-status seeding is RESEARCH §4.6 Plan 06 responsibility (depends on `sync_result.jobs` being in scope at src/cli/run.rs)."
    - "Test-helper labeled-counter parser using `let-else` early-returns (`let Some(rest) = line.strip_prefix(&prefix) else { continue; }`) avoids the `clippy::collapsible_if` warning that nested `if let` would trigger. Sums values across all per-job labels to assert on the closed-enum status dimension only."

key-files:
  created:
    - "tests/v12_webhook_metrics_family.rs (153 lines: 6 integration #[test] cases — HELP+TYPE-at-boot, status-zero-seed, removed-flat-counters, histogram-buckets, queue-depth-zero, P15-preservation)"
    - ".planning/phases/20-.../deferred-items.md (logs the pre-existing fmt drift in src/db/queries.rs:1535 from Plan 01 commit ba250b3 — out of scope per deviation-rule scope-boundary)"
  modified:
    - "src/telemetry.rs (242 lines: REMOVED P18 _sent_total/_failed_total describes + zero-baselines; ADDED labeled family + histogram + gauge describes + zero-baselines; ADDED `set_buckets_for_metric` chain for the new histogram; ADDED closed-enum status seed loop; PRESERVED P15 _delivery_dropped_total verbatim)"
    - "src/webhooks/dispatcher.rs (565 lines: REMOVED 3 flat-counter `metrics::counter!` increments from success/non-2xx/Err arms of HttpDispatcher::deliver; ADDED per-attempt histogram `metrics::histogram!` recorded via `tokio::time::Instant::now()` deltas around `req.body(...).send().await`; behavior of Err returns unchanged)"
    - "src/webhooks/retry.rs (651 lines: ADDED 3 labeled-counter `metrics::counter!` increments at chain-success boundary (status=success), cancel-mid-sleep DLQ-shutdown_drain boundary (status=failed), and retry-exhausted/4xx-permanent terminal-failure boundary (status=failed))"
    - "tests/v12_webhook_success_metric.rs (177 lines: rewrote to wrap HttpDispatcher in RetryingDispatcher + assert `_deliveries_total{status=\"success\"}` delta = 1)"
    - "tests/v12_webhook_failed_metric.rs (173 lines: switched 5xx→404 receiver to short-circuit retry chain; wrap in RetryingDispatcher; assert `_deliveries_total{status=\"failed\"}` delta = 1)"
    - "tests/v12_webhook_network_error_metric.rs (186 lines: paused-clock + driver-loop pattern (same as v12_webhook_retry.rs) drains 30s + 300s schedule sleeps virtually; wrap in RetryingDispatcher; assert `_deliveries_total{status=\"failed\"}` delta = 1 after terminal failure)"
    - "tests/metrics_endpoint.rs (Rule 1 fix: replaced HELP/TYPE assertions for the removed P18 counters with HELP/TYPE assertions for the 3 new families + assertions that the P18 names are GONE)"

key-decisions:
  - "setup_metrics() visibility: ALREADY `pub` and `cronduit::telemetry` is a `pub mod` from `src/lib.rs` — integration tests at `tests/v12_webhook_metrics_family.rs` can call `cronduit::telemetry::setup_metrics()` directly. NO visibility change needed."
  - "Histogram recorded per-ATTEMPT regardless of outcome (success, non-2xx, transport error). The metric's truth is wire-level round-trip duration; meaningful for failures (timeout buckets fill at 10s = the reqwest cap)."
  - "tests/v12_webhook_failed_metric.rs switched receiver from 500 (5xx, transient → 3-attempt retry chain) to 404 (4xx-other, permanent → short-circuit after attempt 1). Without the switch, the test would need paused-clock + driver-loop plumbing to drain the 30s + 300s schedule sleeps. The 404 path is semantically equivalent for asserting `status=\"failed\"` increments at the chain-terminal boundary."
  - "tests/v12_webhook_network_error_metric.rs DOES use the paused-clock + driver-loop pattern because network errors are Transient(Network) per D-06 — they always run all 3 attempts before terminal failure. Pattern mirrored from `tests/v12_webhook_retry.rs::three_attempts_at_locked_schedule_under_paused_clock`."
  - "Pre-existing fmt drift in src/db/queries.rs:1535 (introduced by Plan 01 commit ba250b3) NOT auto-fixed in this plan — out of scope per deviation-rule scope-boundary. Logged to `deferred-items.md`."
  - "Rule 1 fix: tests/metrics_endpoint.rs HELP/TYPE assertions for the P18 `_sent_total` / `_failed_total` were broken by the D-22 BREAKING CHANGE. Replaced with HELP/TYPE assertions for the 3 new families + counter-removal assertions. The Rule 1 fix is part of the natural breaking-change adaptation surface — bundled into the Task 2 commit."

requirements-completed: [WH-11]

# Metrics
duration: 25min
completed: 2026-05-01
---

# Phase 20 Plan 05: Webhook Metrics Family Migration Summary

**Migrated the webhook metrics surface from P18's two unlabeled flat counters (`_sent_total`, `_failed_total`) to the labeled per-DELIVERY family `cronduit_webhook_deliveries_total{job, status}` plus the per-ATTEMPT histogram `_delivery_duration_seconds{job}` plus the `_queue_depth` gauge — all eagerly described + zero-baselined at boot, locking Success Criterion 4 ("an operator scraping `/metrics` sees the new `cronduit_webhook_*` family eagerly described at boot"). The P15 channel-saturation `_delivery_dropped_total` counter preserved verbatim per the D-26 semantic split. 4 tasks, 4 atomic commits, 14 webhook+metrics+retry+dlq integration tests green, full integration tier 522/522 + lib 286/286 PASS, clippy clean, `cargo tree -i openssl-sys` empty.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-05-01T20:39:56Z
- **Completed:** 2026-05-01T21:05:53Z
- **Tasks:** 3 (+1 polish commit)
- **Files modified:** 9 (1 created — `tests/v12_webhook_metrics_family.rs` from Wave 0 stub; 7 modified; 1 created — `deferred-items.md`)
- **`src/telemetry.rs` final size:** 242 lines (up from 187)

## Accomplishments

- **D-22 BREAKING CHANGE locked end-to-end:** the unlabeled P18 success-counter and failure-counter flat counters are REMOVED from `src/webhooks/dispatcher.rs` (3 call sites: success arm, non-2xx arm, transport-error arm) AND from `src/telemetry.rs` (2 describe blocks + 2 zero-baselines). Replaced by the labeled per-DELIVERY counter `cronduit_webhook_deliveries_total{job, status}` (closed-enum status ∈ {success, failed, dropped}) incremented at the OUTER `RetryingDispatcher::deliver` chain-terminal boundaries (success, cancel-mid-sleep, retry-exhausted, 4xx-permanent). The closed-enum invariant is enforced at the call site by `&'static str` literals only.
- **D-24 per-attempt histogram landed:** `cronduit_webhook_delivery_duration_seconds{job}` recorded inside `HttpDispatcher::deliver` via `tokio::time::Instant::now()` deltas around `req.body(body_bytes).send().await`. Operator-tuned bucket boundaries `[0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]` configured via a second `PrometheusBuilder::set_buckets_for_metric` chain (the 10s top matches reqwest's per-attempt timeout cap from P18 D-18). The histogram records on every attempt regardless of outcome — the metric's truth is wire-level round-trip duration.
- **D-25 queue-depth gauge zero-baselined:** `cronduit_webhook_queue_depth` gauge zero-baselined at boot in `setup_metrics()`. Live updates from `src/webhooks/worker.rs` land in Plan 04 (parallel wave 3 worktree).
- **D-26 P15 preserved verbatim:** `cronduit_webhook_delivery_dropped_total` channel-saturation counter (P15) untouched in both describe + zero-baseline. The describe text now documents the semantic split with the new `status="dropped"` row of the labeled family (drain-on-shutdown drops). Operators with dashboards keyed off `_delivery_dropped_total` see no change.
- **D-23 closed-enum status seed loop landed:** `for status in ["success", "failed", "dropped"]` pre-seed loop in `setup_metrics()` mirrors the `cronduit_runs_total` status-seed precedent (telemetry.rs:172-182). The status-only seed (no `job` label) materializes the row at boot; per-job × per-status seeding is Plan 06's responsibility per RESEARCH §4.6.
- **6 new integration tests in `tests/v12_webhook_metrics_family.rs`** lock the operator-visible /metrics contract: HELP+TYPE-at-boot for all 3 new families + the preserved P15 counter; closed-enum status zero-baseline rows; flat-counter REMOVAL regression-lock (D-22 BREAKING CHANGE protection); histogram bucket-boundary table from RESEARCH §4.4; queue-depth zero-baseline; P15 preservation lock (D-26).
- **3 existing P18 webhook metric tests migrated** to the labeled family with `RetryingDispatcher`-wrapping (chain-terminal-boundary increments). The network-error test uses the paused-clock + driver-loop pattern from `v12_webhook_retry.rs` to drain the 30s + 300s virtual schedule.

## Task Commits

Each task was committed atomically:

1. **Task 1: Replace flat counter calls in dispatcher.rs + retry.rs; add histogram around inner attempt** — `fe0fcfc` (feat)
2. **Task 2: Update src/telemetry.rs — describe + zero-baseline new families + remove flat-counter describes + histogram buckets** — `8484db0` (feat)
3. **Task 3: Update existing P18 webhook metric tests + populate v12_webhook_metrics_family.rs** — `79aee89` (test)
4. **Polish: Refine dispatcher.rs breaking-change comment to match strict acceptance-criteria grep** — `873ac6d` (chore)

## Files Created/Modified

**Created:**
- `tests/v12_webhook_metrics_family.rs` — 153 lines: 6 `#[test]` integration cases populated from the Wave 0 stub (Plan 01); locks the boot-time HELP+TYPE / status-zero-baseline / flat-counter-removal / histogram-bucket-boundary / queue-depth-zero / P15-preservation contract for the operator-visible Phase 20 metric API.
- `.planning/phases/20-.../deferred-items.md` — logs the pre-existing fmt drift in `src/db/queries.rs:1535` (Plan 01 commit ba250b3) for the phase close-out.

**Modified:**
- `src/telemetry.rs` — added `set_buckets_for_metric` chain for the new histogram; replaced P18 describe block with labeled-family + histogram + gauge describes; replaced P18 zero-baselines with histogram + gauge zero-baselines; added closed-enum status seed loop; preserved P15 `_delivery_dropped_total` describe + zero-baseline verbatim; expanded P15 describe text to document the D-26 semantic split.
- `src/webhooks/dispatcher.rs` — removed 3 flat-counter `metrics::counter!` increments (P18 D-17 sites); added per-attempt histogram `metrics::histogram!("cronduit_webhook_delivery_duration_seconds", "job" => event.job_name.clone()).record(attempt_dur)` around `req.body(body_bytes).send().await`; behavior of return values unchanged (still returns `Err(WebhookError::HttpStatus { code, retry_after })` / `Err(WebhookError::Timeout)` / `Err(WebhookError::Network(...))` per Plan 02's reshape).
- `src/webhooks/retry.rs` — added 3 labeled-counter `metrics::counter!` increments at the chain-terminal boundaries: `Ok(())` chain-success path (status="success"), `cancel.cancelled()` mid-sleep path (status="failed", before the `shutdown_drain` DLQ row), and the post-loop terminal-failure path (status="failed", after the regular DLQ row).
- `tests/v12_webhook_success_metric.rs` — wraps `HttpDispatcher` in `RetryingDispatcher`; uses a new `sum_status` helper to read `_deliveries_total{status="success"}` deltas.
- `tests/v12_webhook_failed_metric.rs` — switched receiver from `500` (transient → 3-attempt retry, slow without paused-clock) to `404` (permanent → short-circuit after attempt 1, fast); wraps in `RetryingDispatcher`; asserts `_deliveries_total{status="failed"}` delta.
- `tests/v12_webhook_network_error_metric.rs` — wraps in `RetryingDispatcher`; uses paused-clock + driver-loop pattern (mirrored from `v12_webhook_retry.rs::three_attempts_at_locked_schedule_under_paused_clock`) to drain the 30s + 300s schedule sleeps virtually since network errors classify as Transient(Network) per D-06.
- `tests/metrics_endpoint.rs` — Rule 1 fix: HELP/TYPE assertions for the removed P18 `_sent_total` / `_failed_total` replaced with HELP/TYPE assertions for the 3 new families plus assertions that the P18 names are GONE.

## Decisions Made

- **`setup_metrics()` visibility**: ALREADY `pub` (defined `pub fn setup_metrics() -> PrometheusHandle` in src/telemetry.rs); `cronduit::telemetry` is a `pub mod` exported from `src/lib.rs`. Integration tests at `tests/v12_webhook_metrics_family.rs` reach it via `use cronduit::telemetry::setup_metrics;`. NO visibility change needed.
- **`tests/v12_webhook_failed_metric.rs` switched 5xx→404 receiver** to short-circuit the retry chain. Plan 05 was originally written assuming 5xx (which would force paused-clock plumbing), but 4xx-permanent is semantically equivalent for asserting the `status="failed"` chain-terminal increment. Faster + simpler. Documented in the test's doc-comment.
- **Histogram bucket boundaries** locked to `[0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]` per RESEARCH §4.4 verbatim (not the `metrics` crate defaults).
- **Per-job × per-status seeding NOT done in this plan** — only the status-only `["success", "failed", "dropped"]` seed loop. Per-job seeding lives in `src/cli/run.rs` AFTER `sync_result.jobs` is in scope (Plan 06's responsibility per D-23 + RESEARCH §4.6). The status-only seed materializes the closed-enum rows at boot; the `{job=*, status=*}` rows materialize on first observation.
- **Comment-rewording for strict-grep acceptance criteria**: the Task 1 commit briefly mentioned the literal P18 counter substrings (`_sent_total` / `_failed_total`) in the BREAKING-CHANGE explanatory comment. Polish commit `873ac6d` reworded the comment to use abstract phrasing ("the success and failure flat counters") so the plan's acceptance-criterion `grep -c 'cronduit_webhook_delivery_sent_total' src/webhooks/dispatcher.rs` returns 0. Behavior unchanged.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] tests/metrics_endpoint.rs broken by D-22 BREAKING CHANGE**

- **Found during:** Task 2 (verifying `cargo nextest run --test metrics_endpoint`).
- **Issue:** The existing `tests/metrics_endpoint.rs::metrics_families_described_from_boot` test asserted `body.contains("# HELP cronduit_webhook_delivery_sent_total")` and `body.contains("# HELP cronduit_webhook_delivery_failed_total")` — the P18 contract that Plan 05 BREAKS by D-22.
- **Fix:** Replaced the 4 P18-counter HELP/TYPE assertions with 6 assertions for the 3 new families (`_deliveries_total` counter + `_delivery_duration_seconds` histogram + `_queue_depth` gauge), plus 2 negative assertions that the P18 names are GONE. Now this test acts as a second regression lock for the D-22 BREAKING CHANGE alongside `webhook_family_old_flat_counters_removed` in `tests/v12_webhook_metrics_family.rs`.
- **Files modified:** `tests/metrics_endpoint.rs`.
- **Commit:** `8484db0` (bundled with Task 2 — natural blast-radius surface for the breaking change).

**2. [Rule 1 - Bug] Task 1 BREAKING-CHANGE comment included literal P18 counter substrings**

- **Found during:** Final plan-level acceptance-criteria verification.
- **Issue:** The Task 1 commit's BREAKING-CHANGE comment in `src/webhooks/dispatcher.rs` mentioned the literal `_sent_total` and `_failed_total` substrings explanatorily. The plan's acceptance criterion `grep -c 'cronduit_webhook_delivery_sent_total' src/webhooks/dispatcher.rs` = 0 was satisfied (the FULL counter name doesn't appear), but the partial `_sent_total` / `_failed_total` substring grep returned 1.
- **Fix:** Reworded the comment to use abstract phrasing — "the success and failure flat counters that lived here in P18" — without the literal substrings. Behavior unchanged.
- **Files modified:** `src/webhooks/dispatcher.rs`.
- **Commit:** `873ac6d` (separate small polish commit; explicitly scoped as `chore`).

---

**Total deviations:** 2 auto-fixed (2 × Rule 1 — Bug)
**Impact on plan:** Both auto-fixes were natural blast-radius surface of the D-22 BREAKING CHANGE — the existing `metrics_endpoint.rs` test had to migrate to the new family, and the Task 1 comment had to be reworded to satisfy the strict acceptance-criteria grep. No scope creep.

## Issues Encountered

**1. Standalone `rustfmt` mis-formats the workspace's edition-2024 code.** Initial fmt cleanup was attempted via `rustfmt src/telemetry.rs tests/v12_webhook_*.rs` (standalone, no `cargo` wrapper). This used edition-2015 defaults and (a) rejected `async fn` in test files entirely, (b) re-ordered imports against the workspace convention. Reverted via `git checkout -- {files}` and re-ran `cargo fmt -- {file_list}` instead. Lesson: always prefer `cargo fmt` so the workspace's `edition = "2024"` from Cargo.toml is honored. Documented in tech-stack patterns of this Summary.

**2. `cargo fmt -- {file_list}` formats more than the listed files.** Despite passing specific file paths after `--`, `cargo fmt` formatted the entire crate (and surfaced a fmt drift in `src/db/queries.rs:1535` that's pre-existing from Plan 01 commit ba250b3). Reverted the queries.rs change with `git checkout -- src/db/queries.rs` per the deviation-rule scope-boundary. Pre-existing drift logged to `deferred-items.md`.

## User Setup Required

None - no external service configuration required.

## Threat Model Mitigations Applied

- **T-20-05 (DoS via metric label cardinality explosion):** the `status` label is a CLOSED ENUM with exactly 3 values, enforced at the call site by `&'static str` literals at all 3 increment sites in `src/webhooks/retry.rs`. Reason granularity (4xx vs 5xx vs network vs timeout) lives in the `webhook_deliveries.dlq_reason` SQL column, NOT in metric labels. The `job` dimension is bounded by configured-job-count (homelab scale: <100). Test `webhook_family_status_seed_zero_at_boot` asserts only the closed-enum status values appear at boot.
- **T-20-04 (Reliability / operator visibility):** the D-26 semantic split between P15 channel-saturation drops (`_delivery_dropped_total`) and P20 drain-on-shutdown drops (`_deliveries_total{status="dropped"}`) is preserved end-to-end. Both describe blocks document the split. Operators with dashboards keyed off `_delivery_dropped_total` for backpressure detection see no change. Test `webhook_p15_dropped_counter_preserved` locks the preservation invariant.

## Threat Flags

None — this plan adds metric describes, zero-baselines, and per-attempt/per-delivery counter increments. No new network endpoints, no new auth paths, no file access pattern changes, no schema changes at trust boundaries.

## Next Phase Readiness

- **Plan 06 (worker wire-up):** MUST add the per-job × per-status seed loop AFTER `sync_result.jobs` is in scope in `src/cli/run.rs` (D-23 + RESEARCH §4.6). The status-only seed in this plan materializes the closed-enum rows at boot; the per-job seeding in Plan 06 lights up the operator-facing dashboard from boot rather than first-observation.
- **Plan 04 (drain budget — parallel wave 3 worktree):** WILL wire the `metrics::gauge!("cronduit_webhook_queue_depth").set(rx.len() as f64)` live updates in `src/webhooks/worker.rs` at the `rx.recv()` boundary. Plan 05's zero-baseline at boot means Plan 04's first sample is an `set()` overwrite, NOT a registry-side first observation — the family is already registered.
- **Plan 07 (`docs/WEBHOOKS.md` extension):** MUST document the D-22 BREAKING CHANGE in the § Metrics section and the rc.1 release notes:
  - Flat counters `cronduit_webhook_delivery_sent_total` and `cronduit_webhook_delivery_failed_total` REMOVED.
  - Replacement query: `sum by (status) (cronduit_webhook_deliveries_total)`.
  - Histogram `_delivery_duration_seconds{job}` for per-attempt latency.
  - Gauge `_queue_depth` for backpressure visibility.
  - Operator-visible distinction: P15 `_delivery_dropped_total` (channel saturation) vs P20 `_deliveries_total{status="dropped"}` (drain-on-shutdown).
- **Plan 09 (close-out):** Pick up the pre-existing `src/db/queries.rs:1535` fmt drift logged in `deferred-items.md`. Trivially safe `cargo fmt -- src/db/queries.rs` collapses the function signature.

No blockers or concerns.

## Self-Check: PASSED

Verified files exist:
- FOUND: tests/v12_webhook_metrics_family.rs (153 lines, 6 #[test] cases)
- FOUND: .planning/phases/20-.../deferred-items.md (pre-existing fmt drift logged)
- MODIFIED: src/telemetry.rs (242 lines; +set_buckets_for_metric chain + 3 new describes + 3 new zero-baselines + status seed loop; -P18 describes + zero-baselines)
- MODIFIED: src/webhooks/dispatcher.rs (565 lines; -3 flat-counter increments + per-attempt histogram around send().await)
- MODIFIED: src/webhooks/retry.rs (651 lines; +3 labeled-counter increments at chain-terminal boundaries)
- MODIFIED: tests/v12_webhook_success_metric.rs (177 lines)
- MODIFIED: tests/v12_webhook_failed_metric.rs (173 lines; 5xx→404 receiver switch)
- MODIFIED: tests/v12_webhook_network_error_metric.rs (186 lines; paused-clock pattern)
- MODIFIED: tests/metrics_endpoint.rs (Rule 1 fix: P18 assertions migrated to new family + flat-counter-removal regression locks)

Verified commits exist:
- FOUND: fe0fcfc (Task 1 — dispatcher.rs flat counters removed + histogram added; retry.rs labeled increments)
- FOUND: 8484db0 (Task 2 — telemetry.rs describe + zero-baseline + bucket config + status seed; metrics_endpoint.rs Rule 1 fix)
- FOUND: 79aee89 (Task 3 — 3 P18 metric tests migrated + v12_webhook_metrics_family.rs populated)
- FOUND: 873ac6d (Polish — comment rewording for strict-grep acceptance criterion)

Verified plan-level acceptance criteria:
- `cargo check --lib --tests` exits 0 — VERIFIED
- `cargo nextest run --test v12_webhook_success_metric --test v12_webhook_failed_metric --test v12_webhook_network_error_metric --test v12_webhook_metrics_family --test metrics_endpoint --test v12_webhook_retry --test v12_webhook_dlq` = 14/14 PASS — VERIFIED
- `cargo nextest run --tests` = 522/522 PASS, 28 skipped (feature-gated postgres tier) — VERIFIED
- `cargo nextest run --lib` = 286/286 PASS — VERIFIED
- `cargo clippy --lib --tests -- -D warnings` clean — VERIFIED
- `cargo tree -i openssl-sys` returns "did not match any packages" (D-38 invariant intact) — VERIFIED
- P18 flat counters GONE from src/ and tests/v12_webhook_*.rs (only present in `webhook_family_old_flat_counters_removed` regression-lock assertions) — VERIFIED
- P15 `_delivery_dropped_total` PRESERVED in src/telemetry.rs (describe + zero-baseline + 3 mention sites) — VERIFIED

---
*Phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1*
*Plan: 05*
*Completed: 2026-05-01*
