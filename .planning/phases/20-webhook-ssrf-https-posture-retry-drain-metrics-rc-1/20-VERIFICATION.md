---
phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
verified: 2026-05-01T00:00:00Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
re_verification: true
closed_gaps:
  - truth: "Retry-After cap off-by-one (BL-02): For next_attempt=1 sleep, cap must be cap_for_slot(1)=360s per D-08"
    closing_commits:
      - sha: "e817f91"
        message: "fix(20-10): close BL-02 Retry-After cap + BL-03 DLQ body preview (WH-05)"
      - sha: "dbc3008"
        message: "test(20-10): update Retry-After bounds + add DLQ body_preview regression (WH-05)"
    summary: "20-10-SUMMARY.md"

  - truth: "DLQ last_error=NULL for http_5xx rows (BL-03): body_preview must survive dispatcher.rs → WebhookError → retry.rs → DLQ INSERT"
    closing_commits:
      - sha: "e817f91"
        message: "fix(20-10): close BL-02 Retry-After cap + BL-03 DLQ body preview (WH-05)"
      - sha: "dbc3008"
        message: "test(20-10): update Retry-After bounds + add DLQ body_preview regression (WH-05)"
    summary: "20-10-SUMMARY.md"

  - truth: "FK violation in retention pruner (BL-01): webhook_deliveries must be pruned BEFORE job_runs; NOT EXISTS guard must cover webhook_deliveries"
    closing_commits:
      - sha: "38600c9"
        message: "fix(20-11): reorder retention prune phases (BL-01 part 1)"
      - sha: "36bca9c"
        message: "fix(20-11): extend NOT EXISTS guard for webhook_deliveries (BL-01 part 2)"
      - sha: "f2bfc15"
        message: "test(20-11): regression-lock retention FK ordering (BL-01 part 3)"
    summary: "20-11-SUMMARY.md"

  - truth: "WH-08 SSRF accepted risk: THREAT_MODEL.md missing Threat Model 5 / Webhook Outbound section"
    closing_commits:
      - sha: "ae08f36"
        message: "docs(20-12): add Threat Model 5 / Webhook Outbound stub (WH-08)"
    summary: "20-12-SUMMARY.md"

deferred:
  - truth: "rc.1 tag cut"
    addressed_in: "Phase 20 Plan 09"
    evidence: "Phase goal notes rc.1 tag is deferred to maintainer per Plan 09; UAT validation is out of scope per CLAUDE.md memory rule (UAT requires user validation)"
---

# Phase 20: Webhook Security Posture / Retry / Drain / Metrics — Verification Report

**Phase Goal:** Lock the webhook security posture (HTTPS for non-local destinations, SSRF accepted-risk documented), the retry/dead-letter behavior, the graceful-shutdown drain, and the Prometheus metric family — then cut `v1.2.0-rc.1` covering the foundation block.
**Verified:** 2026-05-01T00:00:00Z
**Status:** passed
**Re-verification:** Yes — after gap closure by plans 20-10, 20-11, and 20-12

## Re-verification Summary

Previous status: `gaps_found` (3 BLOCKERS from 20-REVIEW.md + WH-08 missing documentation).
Previous score: 2/4 automatable truths verified.

All four gaps are confirmed closed in the codebase. Score: 5/5 (WH-07 and WH-11 carried from prior run; WH-05, WH-10, and WH-08 now verified).

### Regressions

None. WH-07 (`check_webhook_url` HTTPS validator) and WH-11 (Prometheus metric family) were verified in the initial run and remain intact in the current codebase.

## Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | HTTPS-required validator rejects http:// for non-loopback/non-RFC1918 (WH-07) | VERIFIED | `check_webhook_url` + `classify_http_destination` in `src/config/validate.rs:382-501`; comprehensive unit tests cover all IP families, all RFC1918/ULA ranges, loopback, and the D-21 verbatim error message — CARRIED FORWARD from initial verification |
| 2 | Retry chain: 3 attempts, Retry-After honored within cap, exhaustion writes DLQ with body_preview in last_error (WH-05) | VERIFIED | BL-02 closed: `compute_sleep_delay` at retry.rs:187 calls `cap_for_slot(next_attempt, schedule)` — not `next_attempt - 1`. No `saturating_sub(1)` found in the function. Unit tests at lines 556-609 assert 360s cap for next_attempt=1 (was 36s). BL-03 closed: `WebhookError::HttpStatus` carries `body_preview: Option<String>` field at dispatcher.rs:44; dispatcher populates it at lines 318-326; retry.rs:382 assigns `last_error = body_preview.as_ref().map(|s| truncate_error(s))`. Regression tests: `compute_sleep_delay_caps_retry_after_at_slot_cap` (360s bound), `compute_sleep_delay_first_sleep_uses_attempt_2_cap_per_d08` (350s honored), `dlq_5xx_row_has_body_preview_in_last_error` in tests/v12_webhook_dlq.rs, integration bounds in v12_webhook_retry_after.rs (700-780s for 9999-cap test, ≥680s for 350-honor test) |
| 3 | Graceful drain: SIGTERM drains queue up to drain_grace, drops remainder with counter, in-flight not cancelled; AND retention pruner does not break on DLQ FK (WH-10) | VERIFIED | BL-01 closed: retention.rs prune cycle now runs Phase 1 (job_logs) → Phase 2 (webhook_deliveries, line 93) → Phase 3 (job_runs, line 145) → Phase 4 (WAL checkpoint). Phase 2 line comes before Phase 3 line (93 < 145). `delete_old_runs_batch` SQL in BOTH SQLite (queries.rs:1496) and Postgres (queries.rs:1515) branches contains `AND NOT EXISTS (SELECT 1 FROM webhook_deliveries wd WHERE wd.run_id = jr.id)`. Migration files UNCHANGED — no `ON DELETE CASCADE` added (Option A confirmed: `grep -c 'ON DELETE CASCADE' migrations/sqlite/20260502_000008_webhook_deliveries_add.up.sql` = 0). Regression tests: `retention_webhook_fk_no_violation_when_dlq_row_references_old_run` and `retention_webhook_fk_keeps_run_when_fresh_dlq_references_it` in tests/retention_webhook_fk.rs (194 lines, substantive) |
| 4 | Metric family: cronduit_webhook_* family eagerly described at boot (WH-11) | VERIFIED | `src/telemetry.rs:148-237` describes and zero-baselines all three metric families — CARRIED FORWARD from initial verification |
| 5 | SSRF documented as accepted risk in THREAT_MODEL.md (WH-08) | VERIFIED | `## Threat Model 5: Webhook Outbound (SSRF Accepted Risk)` section exists at THREAT_MODEL.md:189, between TM4 (line 157) and STRIDE Summary (line 233). Section enumerates: (a) operator-with-UI-access threat, (b) loopback-bound default, (c) reverse-proxy fronting mitigation. Cites `src/config/validate.rs::check_webhook_url` at line 165 of TM5. Phase 24 forward pointer present with markdown link. Changelog row `Phase 20 stub | 2026-05-01` at line 300. Document `**Revision:**` header at line 3 unchanged at `2026-04-12 (Phase 6 -- complete)` per plan anti-pattern. Grep counts: `Threat Model 5` = 2, `Webhook Outbound` = 2, `check_webhook_url` = 1, `Phase 24` = 6 — all ≥1 |
| 6 | rc.1 tag cut | DEFERRED | Deferred to maintainer per Plan 09; out of scope for automated verification per CLAUDE.md memory rule (UAT requires user validation) |

**Score:** 5/5 automatable truths verified (rc.1 tag cut remains DEFERRED, not failed).

## Closed Gaps

### BL-02: Retry-After cap off-by-one — CLOSED

**Root cause:** `compute_sleep_delay` passed `next_attempt.saturating_sub(1)` to `cap_for_slot`, producing cap=36s for the first inter-attempt sleep instead of 360s per CONTEXT D-08.

**Fix (Plan 20-10):** Changed call to `cap_for_slot(next_attempt, schedule)`. Added doc comment citing D-08 and BL-02 history. Updated unit test `compute_sleep_delay_caps_retry_after_at_slot_cap` from asserting 36s to asserting 360s. Added new test `compute_sleep_delay_first_sleep_uses_attempt_2_cap_per_d08`. Updated integration test bounds in v12_webhook_retry_after.rs.

**Closing commits:** `e817f91` (fix), `dbc3008` (tests)

### BL-03: DLQ last_error=NULL for http_5xx — CLOSED

**Root cause:** `WebhookError::HttpStatus` had no `body_preview` field; HttpDispatcher computed the truncated preview but discarded it; RetryingDispatcher hard-coded `last_error = None` for the HttpStatus match arm.

**Fix (Plan 20-10):** Added `body_preview: Option<String>` field to `WebhookError::HttpStatus` (dispatcher.rs:44). HttpDispatcher now passes `body_preview: body_preview_opt` at lines 323-327 (empty string maps to None). RetryingDispatcher `HttpStatus` arm at retry.rs:382 now assigns `last_error = body_preview.as_ref().map(|s| truncate_error(s))`. New regression test `dlq_5xx_row_has_body_preview_in_last_error` in tests/v12_webhook_dlq.rs asserts non-NULL `last_error` and body content.

**Closing commits:** `e817f91` (fix), `dbc3008` (tests)

### BL-01: FK violation in retention pruner — CLOSED

**Root cause:** Retention pruner deleted `job_runs` (Phase 2) BEFORE `webhook_deliveries` (Phase 4). The NOT EXISTS guard in `delete_old_runs_batch` checked only `job_logs`. Any DLQ row referencing an old run caused FK violation, breaking the loop and permanently halting retention.

**Fix (Plan 20-11, Option A):** Reordered `run_prune_cycle` phases: logs (Phase 1) → webhook_deliveries (Phase 2, line 93) → job_runs (Phase 3, line 145) → WAL checkpoint (Phase 4). Extended `delete_old_runs_batch` NOT EXISTS clause in BOTH SQLite (line 1496) and Postgres (line 1515) branches to also guard against webhook_deliveries references (defense in depth). Migration files UNCHANGED — no `ON DELETE CASCADE` added; FK contract preserved per audit-table framing (CONTEXT D-10/D-12). New test file `tests/retention_webhook_fk.rs` (194 lines) with two regression tests.

**Closing commits:** `38600c9` (phase reorder), `36bca9c` (NOT EXISTS extension), `f2bfc15` (regression tests), `6c08cf0`/`c10e2ad` (fmt cleanup)

### WH-08: THREAT_MODEL.md Webhook Outbound section — CLOSED

**Root cause:** THREAT_MODEL.md had no Threat Model 5 / Webhook Outbound section (last revised at Phase 6 completion, April 12). WH-08 requires the document to enumerate the SSRF threat, v1.2 mitigations, and accepted residual risk.

**Fix (Plan 20-12):** Added `## Threat Model 5: Webhook Outbound (SSRF Accepted Risk)` section at lines 189-231 between TM4 (line 157) and STRIDE Summary (line 233). Section enumerates all three REQUIREMENTS.md WH-08 items, cites `src/config/validate.rs::check_webhook_url`, cross-references TM2 for loopback rationale, documents the v1.3-deferred allow/block-list filter as accepted residual risk, and includes a Phase 24 close-out forward pointer. Changelog row added at line 300. Document `**Revision:**` header intentionally unchanged (Phase 24 owns the next bump). Words-only — no diagrams of any kind.

**Closing commit:** `ae08f36`

## Required Artifacts

| Artifact | Status | Details |
|----------|--------|---------|
| `src/webhooks/retry.rs` | VERIFIED | BL-02 fix at line 187 (`cap_for_slot(next_attempt, schedule)`); BL-03 fix at line 382 (`last_error = body_preview.as_ref().map(...)`); new unit tests at lines 556-609 |
| `src/webhooks/dispatcher.rs` | VERIFIED | `body_preview: Option<String>` field in `WebhookError::HttpStatus` at line 44; populated at lines 318-326 |
| `src/scheduler/retention.rs` | VERIFIED | Phase 2 `delete_old_webhook_deliveries_batch` at line 93; Phase 3 `delete_old_runs_batch` at line 145 (correct order: 93 < 145) |
| `src/db/queries.rs` | VERIFIED | NOT EXISTS guard covers webhook_deliveries in SQLite (line 1496) and Postgres (line 1515) |
| `tests/retention_webhook_fk.rs` | VERIFIED | NEW — 194 lines; two regression tests: `retention_webhook_fk_no_violation_when_dlq_row_references_old_run` and `retention_webhook_fk_keeps_run_when_fresh_dlq_references_it` |
| `tests/v12_webhook_dlq.rs` | VERIFIED | New `dlq_5xx_row_has_body_preview_in_last_error` test present |
| `tests/v12_webhook_retry_after.rs` | VERIFIED | Updated bounds: `receiver_429_with_retry_after_9999_is_capped` asserts ≥700s ∧ ≤780s; `receiver_429_with_retry_after_header_extends_sleep_to_hint_within_cap` asserts ≥680s |
| `THREAT_MODEL.md` | VERIFIED | TM5 section at lines 189-231; all 4 grep targets return ≥1; ordering TM4(157) < TM5(189) < STRIDE(233) confirmed |
| `migrations/sqlite/20260502_000008_webhook_deliveries_add.up.sql` | VERIFIED UNCHANGED | `ON DELETE CASCADE` count = 0; Option A invariant confirmed |
| `migrations/postgres/20260502_000008_webhook_deliveries_add.up.sql` | VERIFIED UNCHANGED | `ON DELETE CASCADE` count = 0; Option A invariant confirmed |

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `compute_sleep_delay` | `cap_for_slot` | `cap_for_slot(next_attempt, schedule)` — D-08 formula | VERIFIED | retry.rs:187 confirmed; `saturating_sub(1)` form absent from function |
| `HttpDispatcher::deliver` non-2xx arm | `WebhookError::HttpStatus` | `body_preview: body_preview_opt` | VERIFIED | dispatcher.rs:323-327 |
| `WebhookError::HttpStatus` | `RetryingDispatcher::deliver` HttpStatus arm | `last_error = body_preview.as_ref().map(...)` | VERIFIED | retry.rs:382 |
| `run_prune_cycle` Phase 2 | `delete_old_webhook_deliveries_batch` | Direct call at line 93, BEFORE Phase 3 | VERIFIED | Line 93 < line 145 confirmed |
| `delete_old_runs_batch` | `webhook_deliveries` | `NOT EXISTS (SELECT 1 FROM webhook_deliveries wd WHERE wd.run_id = jr.id)` | VERIFIED | queries.rs lines 1496 (SQLite) and 1515 (Postgres) |
| `THREAT_MODEL.md § Threat Model 5` | `src/config/validate.rs::check_webhook_url` | Inline citation at TM5 § Mitigations | VERIFIED | `grep -c 'check_webhook_url' THREAT_MODEL.md` = 1 |
| `THREAT_MODEL.md § Threat Model 5` | ROADMAP.md Phase 24 | Markdown link + forward-pointer subsection | VERIFIED | `grep -c 'Phase 24' THREAT_MODEL.md` = 6 |

## Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `RetryingDispatcher::deliver` | `last_error` for HttpStatus | `body_preview` field of `WebhookError::HttpStatus`, populated by `HttpDispatcher` from `resp.text().await` | Yes — live response body, truncated to 200 chars then 500 chars | FLOWING (BL-03 closed) |
| `delete_old_runs_batch` | rows deleted | SQLite/PG DELETE with extended NOT EXISTS | Yes — no FK violation possible when webhook_deliveries phase runs first | FLOWING (BL-01 closed) |
| `compute_sleep_delay` | `cap` for Retry-After | `cap_for_slot(next_attempt, schedule)` | Yes — 360s for slot 1, matching D-08 | FLOWING (BL-02 closed) |

## Behavioral Spot-Checks

| Behavior | Evidence | Status |
|----------|----------|--------|
| Retry-After cap = 360s for next_attempt=1 | `cap_for_slot(next_attempt, schedule)` at retry.rs:187; unit test asserts `Duration::from_secs_f64(360.0)` at line 573 | PASS (code + unit test) |
| DLQ last_error non-NULL for http_5xx | `last_error = body_preview.as_ref().map(...)` at retry.rs:382; `dlq_5xx_row_has_body_preview_in_last_error` regression test | PASS (code + integration test) |
| Retention phase order correct | Phase 2 webhook DLQ at line 93 < Phase 3 job_runs at line 145 in retention.rs | PASS (code) |
| NOT EXISTS guard covers webhook_deliveries | Confirmed in both SQLite (line 1496) and Postgres (line 1515) | PASS (code) |
| Migration files unchanged (Option A) | `ON DELETE CASCADE` count = 0 in both migration files | PASS (grep) |
| THREAT_MODEL.md TM5 section exists | TM5 at line 189, all required grep targets ≥1 | PASS (grep) |
| TM5 in correct position (TM4 < TM5 < STRIDE) | Line numbers 157 < 189 < 233 | PASS (grep) |
| Revision header unchanged in THREAT_MODEL.md | Line 3: `**Revision:** 2026-04-12 (Phase 6 -- complete)` | PASS (grep) |
| No migration or Cargo.toml changes since b494ade | `git diff --stat b494ade..HEAD -- migrations/ Cargo.toml Cargo.lock` = empty | PASS (git) |

## Requirements Coverage

| REQ-ID | Description | Status | Evidence |
|--------|-------------|--------|----------|
| WH-05 | 3-attempt retry with full-jitter, Retry-After honored within D-08 cap, DLQ on exhaustion with body preview | SATISFIED | BL-02 + BL-03 both closed by plan 20-10; unit + integration tests lock post-fix semantics |
| WH-07 | HTTPS required for non-loopback/non-RFC1918 | SATISFIED | `check_webhook_url` + `classify_http_destination`; comprehensive unit tests — CARRIED FORWARD |
| WH-08 | SSRF documented as accepted risk in THREAT_MODEL.md | SATISFIED | TM5 section at lines 189-231; all three WH-08 enumerations present; Phase 24 forward pointer; WH-07 code path cited |
| WH-10 | Graceful drain: configurable drain_grace, drops with counter, in-flight not cancelled; retention pruner does not break on DLQ FK | SATISFIED | BL-01 closed by plan 20-11; runtime drain behavior unchanged; retention phase order + NOT EXISTS guard + regression tests |
| WH-11 | cronduit_webhook_* Prometheus metric family eagerly described | SATISFIED | All three families described + zero-baselined at boot — CARRIED FORWARD |

## Anti-Patterns Scan (Gap-Closure Plans Only)

No new anti-patterns introduced by plans 20-10/20-11/20-12. The plans are correctness-only changes:
- 20-10: mechanical field add + argument flip + test assertion update. No new `return null` or stub patterns.
- 20-11: reorder of existing function calls + SQL clause extension. No new logic paths that could stub.
- 20-12: pure documentation addition. No code changed.

WR-01 (per-job metric seed fires for all jobs regardless of webhook config) remains a WARNING from the initial verification — carried forward. This is a non-blocking informational finding.

## Human Verification Required

None. All four previously-failing items are verifiable programmatically. The rc.1 tag cut remains deferred to the maintainer per Plan 09 and the project memory rule (UAT requires user validation).

**Note for maintainer:** Plans 20-10 and 20-11 both document a disk-exhaustion (ENOSPC) condition that prevented full `cargo test` runs during the executor sessions. The source-level changes are mechanical and high-confidence; `cargo build -p cronduit` (lib only) passed in both sessions; `cargo fmt --all -- --check` and `cargo tree -i openssl-sys` both passed. Before merging the gap-closure PR, the maintainer should run:
- `cargo test --lib webhooks::retry::tests::compute_sleep_delay_caps_retry_after_at_slot_cap`
- `cargo test --lib webhooks::retry::tests::compute_sleep_delay_first_sleep_uses_attempt_2_cap_per_d08`
- `cargo test --test v12_webhook_dlq dlq_5xx_row_has_body_preview_in_last_error`
- `cargo test --test retention_webhook_fk`
- `cargo clippy --all-targets --all-features -- -D warnings`

These are the specific tests added by the gap-closure plans. CI will catch any remaining issues.

## Deferred Items

Items not yet met but explicitly addressed in later milestone phases.

| # | Item | Addressed In | Evidence |
|---|------|-------------|----------|
| 1 | rc.1 tag cut | Phase 20 Plan 09 | Tag deferred to maintainer; UAT out of scope per CLAUDE.md memory rule |
| 2 | Full STRIDE table for TM5 / Webhook Outbound | Phase 24 | ROADMAP.md Phase 24: "THREAT_MODEL.md Threat Model 5 (Webhook Outbound) + Threat Model 6" close-out; TM5 stub explicitly forwards to Phase 24 |

---

_Verified: 2026-05-01T00:00:00Z_
_Verifier: Claude (gsd-verifier)_
_Re-verification after gap closure by plans 20-10, 20-11, 20-12_
