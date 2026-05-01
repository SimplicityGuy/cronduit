---
phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
verified: 2026-05-01T00:00:00Z
status: gaps_found
score: 3/5 must-haves verified
overrides_applied: 0
re_verification: false
gaps:
  - truth: "HTTPS-required validator: An operator setting [[webhooks]] url = \"http://example.com/...\" against a non-loopback/non-RFC1918 destination sees a config-load ERROR; loopback/RFC1918/ULA HTTP allowed."
    status: failed
    reason: "BL-02 (from 20-REVIEW.md): Retry-After cap for the first inter-attempt sleep uses cap_for_slot(prev_slot=next_attempt-1) which resolves to schedule[1]*1.2=36s, contradicting the documented 360s cap in docs/WEBHOOKS.md. The validator itself (check_webhook_url) is correct and well-tested, but this truth maps to the composite WH-07 requirement and the Retry-After cap bug (BL-02) is a correctness defect inside the WH-05 retry chain that was already flagged as BLOCKER in the automated code review."
    artifacts:
      - path: "src/webhooks/retry.rs"
        issue: "compute_sleep_delay passes prev_slot=next_attempt.saturating_sub(1) to cap_for_slot; for next_attempt=1 this gives cap=36s; documented cap is 360s (schedule[next_attempt+1]*1.2). Tests at lines 535-557 regression-lock the buggy semantics."
    missing:
      - "Fix compute_sleep_delay to pass next_attempt (not next_attempt-1) to cap_for_slot, OR update docs/WEBHOOKS.md to document the tighter 36s cap and add a security rationale"
      - "Update unit test compute_sleep_delay_caps_retry_after_at_slot_cap to assert 360s cap for next_attempt=1"
      - "Update integration test receiver_429_with_retry_after_9999_is_capped accordingly"

  - truth: "Retry chain: An operator whose receiver returns 5xx three times sees three retry attempts at the locked schedule (t=0, t=30s±jitter, t=300s±jitter), with Retry-After header honored within cap; exhaustion writes a row to webhook_deliveries DLQ with no payload/headers/signature."
    status: failed
    reason: "Two BLOCKERs from 20-REVIEW.md affect this truth. BL-02: Retry-After cap is wrong for first inter-attempt sleep (36s instead of 360s). BL-03: DLQ row stores last_error=NULL for HTTP-5xx exhaustion because WebhookError::HttpStatus has no body_preview field and the match arm hard-codes last_error=None (retry.rs:361-363). The body preview captured by HttpDispatcher (dispatcher.rs:300-308) is lost before the DLQ INSERT."
    artifacts:
      - path: "src/webhooks/retry.rs"
        issue: "Line 362: last_error = None for HttpStatus variant — DLQ rows for http_5xx outcomes have no diagnostic body preview, contradicting the intent of the audit table"
      - path: "src/webhooks/dispatcher.rs"
        issue: "WebhookError::HttpStatus variant (line 37-39) has no body_preview field; body preview is read and logged but not carried back to the caller"
    missing:
      - "Add body_preview: Option<String> field to WebhookError::HttpStatus"
      - "Populate it in HttpDispatcher from the same truncated string the WARN log uses"
      - "Propagate into last_error in RetryingDispatcher::deliver match arm for HttpStatus"

  - truth: "Graceful drain: An operator sending SIGTERM with deliveries in-flight sees worker drain queue for up to webhook_drain_grace = \"30s\" (configurable), then drop remaining queued deliveries with counter increment; in-flight HTTP requests NOT cancelled mid-flight."
    status: failed
    reason: "BL-01 from 20-REVIEW.md: The webhook_deliveries table FK (run_id REFERENCES job_runs(id)) has no ON DELETE CASCADE; the retention pruner deletes job_runs BEFORE webhook_deliveries in its phase ordering (Phase 2 then Phase 4). The Phase 2 NOT EXISTS clause in delete_old_runs_batch checks only job_logs (not webhook_deliveries). When a DLQ row references an old run, the Phase 2 DELETE aborts with FK violation, the pruner logs an error and breaks the runs-delete loop, and retention permanently stops working. This is an immediate correctness failure in the core scenario (failed webhook delivery). Note: the drain behavior itself (worker_loop in worker.rs) is well-implemented and correctly wired; this BLOCKER affects the persistence layer, not the runtime drain."
    artifacts:
      - path: "migrations/sqlite/20260502_000008_webhook_deliveries_add.up.sql"
        issue: "Line 30: FOREIGN KEY (run_id) REFERENCES job_runs(id) — no ON DELETE CASCADE"
      - path: "migrations/postgres/20260502_000008_webhook_deliveries_add.up.sql"
        issue: "Line 30: FOREIGN KEY (run_id) REFERENCES job_runs(id) — no ON DELETE CASCADE"
      - path: "src/db/queries.rs"
        issue: "delete_old_runs_batch NOT EXISTS clause checks only job_logs, not webhook_deliveries; FK violation fires when any DLQ row references the run being pruned"
      - path: "src/scheduler/retention.rs"
        issue: "Phase 4 (webhook DLQ delete) runs AFTER Phase 2 (job_runs delete); wrong order given FK constraint without CASCADE"
    missing:
      - "Option A: Reorder prune phases to delete webhook_deliveries BEFORE job_runs, AND extend delete_old_runs_batch NOT EXISTS to also cover webhook_deliveries"
      - "Option B: Add ON DELETE CASCADE to FOREIGN KEY (run_id) in both migration files (requires new migration file since the table already exists in deployed instances)"
      - "Option C: Drop the FK constraint and treat run_id as a soft pointer (matches the audit-table framing)"
      - "Add a regression test that seeds job_run + webhook_deliveries, sets end_time before cutoff, and asserts no FK violation"

deferred:
  - truth: "rc.1 tag cut"
    addressed_in: "Phase 20 Plan 09"
    evidence: "Phase goal notes rc.1 tag is deferred to maintainer per Plan 09; UAT validation is out of scope per CLAUDE.md memory rule (UAT requires user validation)"
---

# Phase 20: Webhook Security Posture / Retry / Drain / Metrics — Verification Report

**Phase Goal:** Lock the webhook security posture (HTTPS for non-local destinations, SSRF accepted-risk documented), the retry/dead-letter behavior, the graceful-shutdown drain, and the Prometheus metric family — then cut `v1.2.0-rc.1` covering the foundation block.
**Verified:** 2026-05-01T00:00:00Z
**Status:** gaps_found — 3 BLOCKERS from 20-REVIEW.md confirmed in codebase
**Re-verification:** No — initial verification

## Step 0: Previous Verification

No previous VERIFICATION.md found. Initial mode.

## Step 2: Must-Haves

Source: Phase 20 success criteria as specified in the verification prompt, cross-referenced with REQUIREMENTS.md WH-05, WH-07, WH-08, WH-10, WH-11.

The rc.1 tag cut (Success Criterion 5) is explicitly deferred to the maintainer per Plan 09; it is excluded from automated verification per the UAT memory rule.

## Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | HTTPS-required validator rejects http:// for non-loopback/non-RFC1918 (WH-07) | ✓ VERIFIED | `check_webhook_url` + `classify_http_destination` in `src/config/validate.rs:382-501`; comprehensive unit tests at lines 1448-1623 cover all IP families, all RFC1918/ULA ranges, loopback, and the D-21 verbatim error message |
| 2 | Retry chain: 3 attempts, Retry-After honored within cap, exhaustion writes DLQ (WH-05) | ✗ FAILED | BL-02: Retry-After cap is 36s for first sleep instead of documented 360s. BL-03: DLQ last_error=NULL for HTTP-5xx outcomes |
| 3 | Graceful drain: SIGTERM drains queue up to drain_grace, drops remainder with counter, in-flight not cancelled (WH-10) | ✗ FAILED | BL-01: FK violation in retention pruner permanently breaks retention after first DLQ row referencing a prunable run |
| 4 | Metric family: cronduit_webhook_* family eagerly described at boot (WH-11) | ✓ VERIFIED | `src/telemetry.rs:148-237` describes and zero-baselines all three metric families; per-job × per-status seed in `src/cli/run.rs:161-170` |
| 5 | SSRF documented as accepted risk in THREAT_MODEL.md (WH-08) | ✗ FAILED | `THREAT_MODEL.md` last revised 2026-04-12 (Phase 6 complete); no Threat Model 5 / Webhook Outbound section exists; SSRF is entirely absent from the document |
| 6 | rc.1 tag cut | DEFERRED | Deferred to maintainer per Plan 09; out of scope for automated verification |

**Score:** 2/4 automatable truths verified (excluding the deferred rc.1 tag). SSRF documentation (WH-08) is a third blocker beyond the code review BLOCKERs.

## Requirement Coverage

| REQ-ID | Description | Status | Evidence |
|--------|-------------|--------|----------|
| WH-05 | 3-attempt retry with full-jitter, DLQ on exhaustion | ✗ FAILED | BL-02 (Retry-After cap wrong) + BL-03 (last_error=NULL for 5xx DLQ rows) |
| WH-07 | HTTPS required for non-loopback/non-RFC1918 | ✓ VERIFIED | `check_webhook_url` + `classify_http_destination` fully implemented and tested |
| WH-08 | SSRF documented as accepted risk in THREAT_MODEL.md | ✗ FAILED | THREAT_MODEL.md has no Threat Model 5; last revision is Phase 6 (2026-04-12) |
| WH-10 | Graceful drain: configurable drain_grace, drops with counter, in-flight not cancelled | ✗ FAILED | Runtime behavior implemented correctly; BL-01 FK violation breaks retention permanently once DLQ has any row |
| WH-11 | cronduit_webhook_* Prometheus metric family eagerly described | ✓ VERIFIED | All three families (deliveries_total, delivery_duration_seconds, queue_depth) described + zero-baselined at boot |

## Required Artifacts

| Artifact | Status | Details |
|----------|--------|---------|
| `src/webhooks/retry.rs` | ✓ EXISTS + WIRED | RetryingDispatcher implemented; compose pattern correct; BL-02 cap bug and BL-03 last_error=None are internal correctness defects |
| `src/webhooks/worker.rs` | ✓ EXISTS + WIRED | Drain state machine correctly implemented: biased select!, 3-arm form, drain_deadline None/Some, try_recv drop loop |
| `src/config/validate.rs` | ✓ EXISTS + WIRED | check_webhook_url + classify_http_destination; called from run_all_checks; comprehensive tests |
| `src/telemetry.rs` | ✓ EXISTS + WIRED | cronduit_webhook_deliveries_total, cronduit_webhook_delivery_duration_seconds, cronduit_webhook_queue_depth all described and zero-baselined |
| `src/cli/run.rs` | ✓ EXISTS + WIRED | RetryingDispatcher constructed and wired; webhook_drain_grace threaded; per-job × per-status seed loop present |
| `migrations/sqlite/20260502_000008_webhook_deliveries_add.up.sql` | ✓ EXISTS — BLOCKER | Table exists, schema correct, NO ON DELETE CASCADE on run_id FK (BL-01) |
| `migrations/postgres/20260502_000008_webhook_deliveries_add.up.sql` | ✓ EXISTS — BLOCKER | Same FK issue as SQLite migration |
| `THREAT_MODEL.md` | ✗ MISSING SECTION | Phase 6 revision (2026-04-12) has no Threat Model 5 / Webhook Outbound; WH-08 not satisfied |

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/cli/run.rs` | `RetryingDispatcher` | `crate::webhooks::RetryingDispatcher::new(http, pool, cancel.child_token(), webhooks_arc)` | ✓ WIRED | Lines 316-322 |
| `src/cli/run.rs` | `spawn_worker` | `crate::webhooks::spawn_worker(webhook_rx, dispatcher, cancel.child_token(), cfg.server.webhook_drain_grace)` | ✓ WIRED | Lines 338-343 |
| `RetryingDispatcher::deliver` | `queries::insert_webhook_dlq_row` | DLQ write on terminal failure | ✓ WIRED | retry.rs:281 |
| `worker_loop` Arm 3 | `metrics::counter!(cronduit_webhook_deliveries_total, status=dropped)` | try_recv loop | ✓ WIRED | worker.rs:171-175 |
| `delete_old_runs_batch` | `webhook_deliveries` | NOT EXISTS guard | ✗ NOT WIRED | BL-01: NOT EXISTS only checks job_logs; webhook_deliveries FK causes hard error |
| `THREAT_MODEL.md` | Webhook Outbound SSRF section | Phase 20 / WH-08 | ✗ NOT WIRED | Section does not exist |

## Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `RetryingDispatcher::deliver` | `last_error` for HttpStatus | `WebhookError::HttpStatus` variant | No — hard-coded None | ✗ HOLLOW for 5xx DLQ rows (BL-03) |
| `worker_loop` queue_depth gauge | `rx.len()` | mpsc channel | Yes | ✓ FLOWING |
| `delete_old_runs_batch` | rows affected | SQLite/PG DELETE | Yes, but fails with FK violation when DLQ rows exist | ✗ DISCONNECTED (BL-01) |

## Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| HTTPS validator rejects http://example.com | Unit tests in validate.rs | 20 test functions pass per 20-REVIEW.md confirmation | ✓ PASS (unit) |
| Metric families present in telemetry.rs | Grep for describe_counter!/describe_histogram!/describe_gauge! | All 3 families found with descriptions and zero-baselines | ✓ PASS |
| webhook_drain_grace config field exists | Grep src/config/mod.rs | `pub webhook_drain_grace: Duration` at line 53, default 30s at line 77 | ✓ PASS |
| DLQ table FK has ON DELETE CASCADE | Inspect migrations | No ON DELETE CASCADE found on either migration | ✗ FAIL (BL-01) |
| THREAT_MODEL.md contains Threat Model 5 | Grep for "Threat Model 5" or "SSRF" | Zero matches in current THREAT_MODEL.md | ✗ FAIL (WH-08) |

## Anti-Patterns Found

| File | Lines | Pattern | Severity | Impact |
|------|-------|---------|----------|--------|
| `src/webhooks/retry.rs` | 161-179 | `compute_sleep_delay` passes `prev_slot = next_attempt.saturating_sub(1)` instead of `next_attempt` to `cap_for_slot` | Blocker | Retry-After honored at 36s cap instead of documented 360s for first sleep; receiver returning `Retry-After: 350` is silently truncated |
| `src/webhooks/retry.rs` | 360-363 | `last_error = None` hard-coded for `WebhookError::HttpStatus` match arm | Blocker | DLQ rows for http_5xx exhaustion lose body-preview diagnostic; `SELECT * FROM webhook_deliveries WHERE dlq_reason = 'http_5xx'` shows NULL last_error |
| `migrations/sqlite/20260502_000008_webhook_deliveries_add.up.sql` | 30 | `FOREIGN KEY (run_id) REFERENCES job_runs(id)` without ON DELETE CASCADE | Blocker | Any DLQ row referencing an old run permanently breaks the retention pruner's runs-delete phase |
| `migrations/postgres/20260502_000008_webhook_deliveries_add.up.sql` | 30 | Same FK issue | Blocker | Same impact on Postgres backend |
| `THREAT_MODEL.md` | (absent) | WH-08 Threat Model 5 / Webhook Outbound SSRF section missing | Blocker | WH-08 requirement not satisfied; SSRF accepted-risk not documented |
| `src/cli/run.rs` | 161-170 | Per-job seed loop fires for all jobs, not just webhook-configured jobs | Warning | Inflates cardinality: 50-job fleet with 2 webhook jobs seeds 150 zero rows instead of 6 (WR-01 from 20-REVIEW.md) |

## Gaps Summary

Three BLOCKER findings from 20-REVIEW.md (BL-01, BL-02, BL-03) are confirmed in the codebase and constitute blocking gaps. A fourth BLOCKER not in 20-REVIEW.md was found during this verification: WH-08 (SSRF accepted-risk documentation in THREAT_MODEL.md) is entirely absent — the file was last revised at Phase 6 completion and has no Threat Model 5 section.

**BL-01 (FK pruning order):** The webhook_deliveries migration creates a FOREIGN KEY to job_runs(id) without ON DELETE CASCADE. The retention pruner deletes job_runs before webhook_deliveries. The NOT EXISTS guard in delete_old_runs_batch checks only job_logs — so when any DLQ row references a to-be-pruned run, the Phase 2 DELETE fails with an FK violation, breaks out of the loop, and retention permanently stops running. This is a silent-failure mode triggered by the exact scenario this feature was designed for.

**BL-02 (Retry-After cap):** `compute_sleep_delay` uses `cap_for_slot(next_attempt - 1, schedule)` to compute the cap for the sleep before attempt 2. This resolves to `schedule[1] * 1.2 = 36s`. The documented cap in WEBHOOKS.md is `schedule[next_attempt + 1] * 1.2 = 360s`. A receiver returning `Retry-After: 350` between attempts 1 and 2 is silently capped to 36s. Unit tests lock in the buggy behavior rather than the documented contract.

**BL-03 (DLQ last_error=NULL for 5xx):** `WebhookError::HttpStatus` carries no body_preview field. The HttpDispatcher reads the response body and logs a 200-char preview, but does not propagate it through the error type. The RetryingDispatcher match arm for HttpStatus sets `last_error = None` unconditionally. DLQ rows for failed 5xx outcomes store NULL last_error, making the audit table diagnostic value minimal.

**WH-08 (SSRF not documented):** WH-08 requires THREAT_MODEL.md to gain Threat Model 5 (Webhook Outbound) enumerating: operator-with-UI-access can configure a webhook URL pointing at any internal service; cronduit is loopback-bound by default; reverse-proxy fronting with auth is the v1.2 mitigation. The current THREAT_MODEL.md has no such section (last revised Phase 6, April 12).

The phase goal as a whole is NOT achieved: three of five testable success criteria fail on correctness grounds (WH-05/WH-10 via codebase bugs, WH-08 via missing documentation). WH-07 and WH-11 are fully achieved.

---

_Verified: 2026-05-01T00:00:00Z_
_Verifier: Claude (gsd-verifier)_
