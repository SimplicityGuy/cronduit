---
phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
plan: 10
subsystem: webhooks
tags: [webhooks, retry, dlq, gap-closure, bl-02, bl-03, body-preview, retry-after-cap]
gap_closure: true
requirements: [WH-05]
dependency_graph:
  requires:
    - "Phase 20 Plan 02 (webhook retry chain) — RetryingDispatcher + WebhookError variants"
    - "Phase 20 Plan 03 (DLQ migrations) — webhook_deliveries.last_error column"
  provides:
    - "Locked D-08 cap math in compute_sleep_delay (BL-02 closed)"
    - "body_preview propagated into webhook_deliveries.last_error for http_5xx (BL-03 closed)"
  affects:
    - "src/webhooks/dispatcher.rs (WebhookError::HttpStatus carries body_preview)"
    - "src/webhooks/retry.rs (compute_sleep_delay cap fix + match arm propagation)"
tech_stack:
  added: []
  patterns:
    - "Closed-enum DLQ reason preserved (D-11) — only `last_error` text gains content"
    - "Defense-in-depth truncation: 200-char @ HttpDispatcher, 500-char @ truncate_error"
key_files:
  created: []
  modified:
    - "src/webhooks/dispatcher.rs"
    - "src/webhooks/retry.rs"
    - "tests/v12_webhook_retry_after.rs"
    - "tests/v12_webhook_dlq.rs"
decisions:
  - "BL-02 fix is a code change, not a docs change — D-08 (cap = schedule[next_attempt+1] * 1.2) was already correct in CONTEXT.md and docs/WEBHOOKS.md. The pre-fix code passed `next_attempt - 1` which silently produced 36s for the first inter-attempt sleep instead of the documented 360s."
  - "BL-03 closed via additive change to last_error text only — no new columns, no schema/migration changes. dlq_reason closed enum unchanged per D-11."
  - "Empty body_preview maps to None (DB NULL) so 'NULL means absent' invariant holds when receivers return empty 5xx bodies."
  - "Per-task commits split by surface: src changes in one commit (BL-02 + BL-03 src), tests in a second commit. The two bugs are co-located in retry.rs and the orchestrator approved either per-task or rollup-by-surface commits."
metrics:
  duration_minutes: 13
  completed_date: 2026-05-01
  tasks_completed: 3
  files_changed: 4
  commits: 2
  lines_added: 190
  lines_removed: 39
---

# Phase 20 Plan 10: BL-02 Retry-After Cap + BL-03 DLQ Body Preview Summary

Closes BL-02 (off-by-one in compute_sleep_delay's cap_for_slot index) and BL-03 (DLQ rows for http_5xx had last_error=NULL because body_preview was discarded between HttpDispatcher and RetryingDispatcher) — both gap entries against WH-05 in 20-VERIFICATION.md. Source-only fix; no migrations, no version bump, rustls invariant intact.

## What Was Built

### BL-02: Retry-After cap off-by-one (CONTEXT D-08)

`src/webhooks/retry.rs` — `compute_sleep_delay` now passes `next_attempt` (not `next_attempt - 1`) to `cap_for_slot`. Doc comment rewritten to cite D-08 verbatim and document the pre-fix history so a future maintainer cannot accidentally regress the cap by re-introducing `saturating_sub(1)`.

| Scenario | Pre-fix | Post-fix |
|---|---|---|
| `Retry-After: 9999`, sleep before attempt 2 (next_attempt=1) | 36s (cap_for_slot(0)) | 360s (cap_for_slot(1)) |
| `Retry-After: 350`, sleep before attempt 2 (next_attempt=1) | 36s (silently truncated) | 350s (honored) |
| `Retry-After: 9999`, sleep before attempt 3 (next_attempt=2) | 360s | 360s (last-slot fallback, unchanged) |

In-module unit tests:
- `compute_sleep_delay_caps_retry_after_at_slot_cap` — flipped from regression-locking the BUG (36s) to regression-locking the FIX (360s).
- `compute_sleep_delay_first_sleep_uses_attempt_2_cap_per_d08` — NEW. Locks the Retry-After:350 → 350s contract end-to-end.

Integration test in `tests/v12_webhook_retry_after.rs`:
- `receiver_429_with_retry_after_9999_is_capped` — assertion bound updated from `≤ 450s` (based on 36+360=396s) to `≥ 700s ∧ ≤ 780s` (based on 360+360=720s with 60s driver slack).
- `receiver_429_with_retry_after_header_extends_sleep_to_hint_within_cap` — assertion bound updated from `≥ 380s` to `≥ 680s` (350s honored on BOTH inter-attempt sleeps post-fix instead of just the second).

### BL-03: DLQ body preview lost between dispatcher and retry layer (CONTEXT D-10)

`src/webhooks/dispatcher.rs`:
- `WebhookError::HttpStatus` gains `body_preview: Option<String>` field. Documented as: truncated (≤200 chars) response body preview captured at dispatch time, propagated into `webhook_deliveries.last_error` for dlq_reason='http_5xx'.
- The `Ok(resp)` non-2xx arm at lines 289-322 now propagates the same truncated string the WARN log already produces (no double read; single `resp.text().await` call at line 300 feeds both the log and the new field). Empty string maps to `None` so the DB column is NULL when the receiver sent no body.

`src/webhooks/retry.rs`:
- The HttpStatus match arm in `RetryingDispatcher::deliver` (line 360+) destructures the new field and assigns `last_error = body_preview.as_ref().map(|s| truncate_error(s))`. truncate_error caps at 500 chars (defense-in-depth above HttpDispatcher's 200).
- `classify_response_table` test helper updated to construct the new field as `body_preview: None` (no behavior change — the helper is for classification, which doesn't read body_preview).

`tests/v12_webhook_dlq.rs` — new `dlq_5xx_row_has_body_preview_in_last_error` regression test: spins a wiremock returning 503 + recognizable body; exhausts the chain under `tokio::time::pause()` + driver loop; reads the webhook_deliveries row directly via sqlx; asserts:
- `dlq_reason = 'http_5xx'`
- `last_status = Some(503)`
- `last_error.is_some()` (BL-03 regression lock)
- `last_error` contains a substring of the response body
- `last_error.chars().count() <= 500` (truncation honored)

## Files Modified

| File | Change | Lines |
|---|---|---|
| `src/webhooks/dispatcher.rs` | WebhookError::HttpStatus.body_preview field + populate site | +18 / -2 |
| `src/webhooks/retry.rs` | compute_sleep_delay cap fix + match-arm propagation + 2 unit tests + helper update | +63 / -15 |
| `tests/v12_webhook_retry_after.rs` | post-fix elapsed-time bounds (720s ± slack) | +33 / -22 |
| `tests/v12_webhook_dlq.rs` | new dlq_5xx_row_has_body_preview_in_last_error test | +76 / -0 |

## Verification

| Gate | Status |
|---|---|
| `cargo build -p cronduit` (lib only) | PASSED — confirmed during Task 1 (`Finished dev profile in 1m 06s`) |
| `cargo fmt --all -- --check` | PASSED — exit 0 |
| `cargo tree -i openssl-sys` | PASSED — empty (rustls invariant intact, D-38) |
| `cargo build -p cronduit --tests` | NOT RUN — see Deferred Issues |
| `cargo clippy --all-targets --all-features -- -D warnings` | NOT RUN — see Deferred Issues |
| `cargo test -p cronduit --lib compute_sleep_delay` | NOT RUN — see Deferred Issues |
| `cargo test --test v12_webhook_retry_after` | NOT RUN — see Deferred Issues |
| `cargo test --test v12_webhook_dlq` | NOT RUN — see Deferred Issues |

## Commits

| Hash | Message |
|---|---|
| `6254994` | `fix(20-10): close BL-02 Retry-After cap + BL-03 DLQ body preview (WH-05)` |
| `9b036f8` | `test(20-10): update Retry-After bounds + add DLQ body_preview regression (WH-05)` |

(Final SUMMARY commit is added separately per orchestrator commit protocol.)

## Decisions Made

1. **Single-PR scope honored.** Plan declared `files_modified` for exactly four files; the executor confirmed grep across `src/` + `tests/` found only one external usage site (a doc comment in `tests/v12_webhook_retry_after.rs:4`) and no other constructions of `WebhookError::HttpStatus`. The internal `classify` match in `retry.rs:83` already used `..` so the field add is non-breaking there.
2. **Two atomic commits, split by surface.** Source changes (BL-02 + BL-03 src) committed first; test updates committed second. The plan's `<commit>` blocks were absent and the bugs are co-located, so this aligns with the orchestrator's "rollup-by-surface OR per-task" allowance. Each commit message names the closed BLOCKER explicitly.
3. **Empty body → DB NULL.** `body_preview: None` flows through to `last_error: None` so the existing "NULL means absent" invariant in webhook_deliveries.last_error holds for http_5xx rows where the receiver responds with an empty body.
4. **Defense-in-depth truncation kept at two layers.** Even though HttpDispatcher caps at 200 chars before constructing the error, RetryingDispatcher still passes through `truncate_error` (500-char cap) when persisting. This keeps the DB-write surface bounded regardless of how WebhookError::HttpStatus is constructed in future call sites.
5. **No `tools/` or `docs/` writes.** The plan's anti_patterns_to_avoid section explicitly forbade editing `docs/WEBHOOKS.md` to document the buggy 36s cap — D-08 was right; the code was wrong. No documentation drift.

## Deviations from Plan

### Auto-fixed Issues

None. The plan's `<read_first>` blocks accurately described the code. No Rule 1/2/3 fixes were needed inside the implementation. The line numbers quoted in the plan matched the worktree to within ±5 lines.

### Worktree Bootstrap

The orchestrator-spawned worktree branch `worktree-agent-a018919badf16a577` was rooted at `main` (commit `852daec`), which predates phase 20. The phase 20 planning artifacts and source code (the entirety of the prior 16 commits in `phase-20-gap-closure`) were not yet merged into the worktree. The executor reset the worktree branch onto `phase-20-gap-closure` (commit `b494ade`) so the plan's referenced files (`.planning/phases/20-.../*.md`, the existing `src/webhooks/retry.rs` BL-02 buggy state, etc.) were physically present. This is a one-time bootstrap action; no source files were modified by it.

## Deferred Issues

**Cannot run cargo test / cargo clippy / cargo build --tests in this executor session due to disk exhaustion on `/System/Volumes/Data` (100% full, 122Mi free at peak).**

The macOS volume hosting `/Users` filled to 100% during compilation of test artifacts. The cronduit lib binary built successfully (`cargo build -p cronduit` returned 0), but compiling the per-test-file integration test binaries triggered repeated `rustc-LLVM ERROR: IO failure on output stream: No space left on device` and `ld: write() failed, errno=28`. The executor cleared the worktree's `target/debug/incremental` directory twice but cargo immediately refilled it. Other large `target/` directories on the host (`/Users/Robert/Code/public/cronduit/target` = 33GB, `/Users/Robert/Code/public/discogsography/target` = 12GB) are owned by parallel work and were left untouched per worktree-isolation guarantees.

**What this means for verification:**
- `cargo build -p cronduit` (lib only) confirmed the source compiles and all `WebhookError::HttpStatus` construction sites are updated. Search across `src/` + `tests/` returned only the expected sites (dispatcher.rs, retry.rs in 3 places, test fixture helper at retry.rs:476).
- `cargo fmt --all -- --check` returned exit 0 after `cargo fmt --all` reformatted two cosmetic lines in the new test code (block-style `assert!` parameters); the fmt fixup is included in the test commit.
- `cargo tree -i openssl-sys` returned `did not match any packages` — rustls invariant (D-38) intact; no new dependencies introduced.

**What still needs validation (next executor run / CI / verifier):**
- `cargo build -p cronduit --tests` compiles cleanly (the new test in `tests/v12_webhook_dlq.rs` reuses existing helpers and the new field destructure in `tests/v12_webhook_retry_after.rs` is a doc-only reference, so no breakage is expected).
- `cargo clippy --all-targets --all-features -- -D warnings` returns 0.
- `cargo test -p cronduit --lib compute_sleep_delay` — should pass all 4 tests (pre-existing `compute_sleep_delay_no_retry_after_uses_jitter`, pre-existing `compute_sleep_delay_honors_retry_after_within_cap` which uses next_attempt=2 unchanged, updated `compute_sleep_delay_caps_retry_after_at_slot_cap` asserting 360s, new `compute_sleep_delay_first_sleep_uses_attempt_2_cap_per_d08`).
- `cargo test --test v12_webhook_retry_after` — should pass with the updated 700-780s bounds for the 9999-cap test and ≥680s for the 350-honor test. Note: the `receiver_429_with_retry_after_header_extends_sleep_to_hint_within_cap` test originally took ~30s of real time at virtual-clock advance rate of 0.5s per yield_now(). With the new ~720s expected virtual time the test will take ~24s of real time. No real-clock regression is expected.
- `cargo test --test v12_webhook_dlq dlq_5xx_row_has_body_preview_in_last_error` — should pass; the test reuses the same setup_test_db / build_dispatcher helpers as the 3 existing dlq tests.

**Recommendation:** the orchestrator (or maintainer running locally) should free disk space (suggest cleaning `~/Code/public/cronduit/target/debug/incremental` which is ~14GB and has no impact on local development beyond a one-time recompile) and re-run the verification gates listed above before merging the gap-closure PR. The source-level correctness is high-confidence — the changes are mechanical (add a field; flip a function argument from `n-1` to `n`; update test assertions to the new constants) — but no test executions were performed in this session.

## Self-Check: PASSED

| Claim | Verification |
|---|---|
| `src/webhooks/dispatcher.rs` modified | `git show 6254994 -- src/webhooks/dispatcher.rs` — present |
| `src/webhooks/retry.rs` modified | `git show 6254994 -- src/webhooks/retry.rs` — present |
| `tests/v12_webhook_retry_after.rs` modified | `git show 9b036f8 -- tests/v12_webhook_retry_after.rs` — present |
| `tests/v12_webhook_dlq.rs` modified | `git show 9b036f8 -- tests/v12_webhook_dlq.rs` — present |
| Commit `6254994` exists | `git log --oneline | grep 6254994` — present |
| Commit `9b036f8` exists | `git log --oneline | grep 9b036f8` — present |
| `cargo tree -i openssl-sys` empty | confirmed in this session (D-38 intact) |
| `cargo fmt --all -- --check` clean | confirmed in this session (exit 0) |

## Pointer to Orchestrator

This plan unblocks `/gsd-verify-phase 20` re-run for WH-05 truth #2 (retry chain with Retry-After cap + DLQ row carries diagnostic last_error). Remaining 20-VERIFICATION.md gaps:
- BL-01 (graceful drain FK / retention pruner ordering, WH-08-adjacent): closes in 20-11.
- WH-08 composite (other open verification gaps): closes in 20-12.

The combined gap-closure PR (plans 10 + 11 + 12) is what the verifier will re-score against. After all three land, the WH-05 / WH-08 truths should flip to `passed`.
