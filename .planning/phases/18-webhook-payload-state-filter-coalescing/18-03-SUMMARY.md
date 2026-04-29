---
phase: 18-webhook-payload-state-filter-coalescing
plan: 03
subsystem: webhooks
tags: [webhooks, payload, json, hmac, coalesce, sqlx, sqlite, postgres, cte, chrono, rfc3339]

# Dependency graph
requires:
  - phase: 16-failure-context
    provides: get_failure_context helper, FailureContext struct, idx_job_runs_job_id_start index, dual-SQL CTE pattern with epoch-sentinel COALESCE
  - phase: 15-webhook-worker
    provides: WebhookDispatcher trait, NoopDispatcher, RunFinalized channel-message contract, src/webhooks/ module surface
provides:
  - WebhookPayload<'a> struct with all 16 locked v1 fields (D-06) and deterministic byte-stable serde_json output (Pitfall B)
  - WebhookPayload::build(event, fctx, run, filter_position, cronduit_version) constructor producing RFC3339 Z-suffix timestamps (Pitfall F) and compact JSON (Pitfall C)
  - coalesce::filter_position async fn returning the position of the current run within the operator's filter-matching stream (D-12, D-15)
  - Hard-coded D-15 success sentinel (CASE WHEN status='success' THEN 0 BEFORE the IN-list check) on BOTH SQLite and Postgres SQL constants
  - pad_states_to_6 helper covering Pitfall I (variable-length operator-supplied states slice padded to 6 placeholders)
  - EXPLAIN QUERY PLAN regression test asserting idx_job_runs_job_id_start hit on SQLite (>120 rows + ANALYZE) and Postgres (#[ignore]-gated testcontainer, 10 000 rows + ANALYZE)
affects: [18-04 dispatcher, 18-05 metrics, 18-06 UAT, 19-receiver-examples, 20-webhook-retry]

# Tech tracking
tech-stack:
  added: []  # No new crates — uses existing chrono, serde, serde_json, sqlx, anyhow
  patterns:
    - "16-field Standard Webhooks v1 payload struct with declaration order == JSON serialization order (Pitfall B mitigation)"
    - "Dual-backend SQL constants (SQL_SQLITE with ?N + SQL_POSTGRES with $N) dispatched via match pool.reader() {PoolRef::Sqlite|Postgres}"
    - "Hard-coded SQL safety sentinel — operator-misconfig protection via CASE expression precedence (D-15)"
    - "Bind-parameter pre-padding to a fixed N for variable-length operator slices (Pitfall I)"

key-files:
  created:
    - src/webhooks/payload.rs (275 lines, 9 unit tests)
    - src/webhooks/coalesce.rs (327 lines, 4 unit tests)
    - tests/v12_webhook_filter_position_explain.rs (356 lines, 2 EXPLAIN tests)
  modified:
    - src/webhooks/mod.rs (added pub mod coalesce/payload + pub use WebhookPayload)

key-decisions:
  - "First-break aggregate is MAX(start_time), not MIN — corrected from plan template; MAX correctly identifies the most-recent non-match before the current run (Rule 1 bug fix)"
  - "SQL constants exposed as pub(crate) (not pub) — EXPLAIN test inlines verbatim copies for crate-visibility independence"
  - "Postgres EXPLAIN test #[ignore]-gated mirroring tests/v12_fctx_explain.rs convention; SQLite test runs by default in CI"
  - "ANALYZE inserted into the SQLite EXPLAIN test seed path so the planner picks idx_job_runs_job_id_start even on 120-row fixtures"
  - "DbRunDetail fixture in payload tests constructed with full literal expansion (no #[derive(Default)] added to production struct)"

patterns-established:
  - "Webhook wire payload module shape: pub struct with all-named fields (no HashMap/BTreeMap), &'static str for locked literal fields, lifetime-borrowed &str for event-derived fields, owned String for chrono-formatted timestamps"
  - "Filter-stream-position helper shape: dual SQL constants + PoolRef dispatch + bind-padding helper + epoch-sentinel COALESCE (mirrors get_failure_context)"
  - "EXPLAIN regression test gating: SQLite default-on, Postgres #[ignore]-gated with seed-volume + ANALYZE preconditions per RESEARCH Pitfall 4"

requirements-completed: [WH-06, WH-09]

# Metrics
duration: ~22min
completed: 2026-04-29
---

# Phase 18 Plan 03: Webhook Payload + Filter-Position Helper Summary

**Locked 16-field v1 webhook payload struct with byte-stable JSON encoding plus coalesce::filter_position SQL helper (D-15 success sentinel hard-coded on both backends; idx_job_runs_job_id_start asserted via EXPLAIN regression test)**

## Performance

- **Duration:** ~22 minutes
- **Started:** 2026-04-29T20:48Z (approx)
- **Completed:** 2026-04-29T21:10Z (approx)
- **Tasks:** 2 (both committed atomically)
- **Files created:** 3
- **Files modified:** 1

## Accomplishments

- WebhookPayload<'a> struct with all 16 fields in declaration order matching D-06; serde_derive emits in declaration order so two consecutive serde_json::to_vec calls produce identical bytes (Pitfall B regression test). Compact JSON by serde_json default (Pitfall C). Timestamps use SecondsFormat::Secs + use_z=true so they end with Z, never +00:00 (Pitfall F).
- coalesce::filter_position async fn — single backwards-walking SQL query computes the operator's filter-matching stream position via three CTEs (ordered → marked → first_break). Counts back from current_start, stops at the first non-match OR the first success.
- D-15 success sentinel hard-coded in the CASE expression on BOTH SQLite and Postgres branches: `WHEN status = 'success' THEN 0` runs BEFORE the IN-list check. A success run is ALWAYS a streak break — even if the operator misconfigures `webhook.states` to include `"success"`. Locked by regression test `filter_position_treats_success_as_break_even_when_in_states` (without the sentinel this returns 3; with it, 2).
- Pitfall I bind-padding via `pad_states_to_6` — operator-supplied variable-length states slice is repeated to fill 6 placeholders; duplicates collapse harmlessly inside SQL `IN(...)`.
- 13 unit tests pass (9 payload + 4 coalesce). 2 EXPLAIN regression tests assert `idx_job_runs_job_id_start` hit on SQLite (default) and Postgres (#[ignore]-gated testcontainer).

## Task Commits

Each task committed atomically (no separate test commit because the implementation and tests were authored together — both compile/run together in the same module):

1. **Task 1: WebhookPayload struct + 9 unit tests** — `645ed12` (feat)
2. **Task 2: filter_position helper + EXPLAIN regression test** — `44ea991` (feat)

## Files Created/Modified

- **Created** `src/webhooks/payload.rs` (275 lines) — WebhookPayload<'a> struct + build() + 9 unit tests
- **Created** `src/webhooks/coalesce.rs` (327 lines) — filter_position async fn + dual-backend SQL constants (with D-15 sentinel) + pad_states_to_6 helper + 4 unit tests
- **Created** `tests/v12_webhook_filter_position_explain.rs` (356 lines) — SQLite + Postgres EXPLAIN regression tests
- **Modified** `src/webhooks/mod.rs` — added `pub mod coalesce`, `pub mod payload`, `pub use payload::WebhookPayload`

## Decisions Made

- **first_break aggregate is MAX, not MIN.** The plan template wrote `MIN(start_time)` for the most-recent non-match. That is incorrect: with MIN we'd select the OLDEST non-match, and any match older than the most-recent break would leak into the count. The correct aggregate is MAX(start_time) — the most-recent non-match before the current run. This is a Rule 1 (bug) auto-fix on the plan-template SQL; verified by the four unit tests covering the canonical scenarios (all pass with MAX, with MIN the success-as-break tests would over-count).
- **SQL constants are `pub(crate)`** rather than fully `pub`. The EXPLAIN integration test inlines verbatim copies of the constants instead of importing them — this keeps the test independent of crate visibility and future re-export changes. If the production CTE shape diverges from the test copy, the wave-end gate catches it via test failure.
- **Postgres EXPLAIN test `#[ignore]`-gated**, mirroring `tests/v12_fctx_explain.rs` precedent. SQLite EXPLAIN runs by default. Postgres testcontainer is opted in via `cargo test -- --ignored` when the operator wants the heavier integration coverage.
- **`ANALYZE` added to SQLite EXPLAIN seed path.** Even with 120 rows, SQLite's planner can pick a different access path without statistics — `ANALYZE` ensures `idx_job_runs_job_id_start` is consulted.
- **DbRunDetail test fixture is a full literal**, not `..Default::default()`. The plan template suggested optionally deriving `Default` on the production struct. We chose the safer path (no production-API change just for tests) and expanded all 13 fields explicitly with sensible defaults.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Plan template `first_break` aggregate corrected from MIN to MAX**

- **Found during:** Task 2 (coalesce.rs implementation)
- **Issue:** The plan template's SQL used `MIN(start_time)` for the `first_break` CTE that identifies the boundary above which matches should be counted. That selects the OLDEST non-match, not the MOST-RECENT one. Concrete failure mode: a `failed → success → failed → failed` trace with `states=["failed"]`. With `MIN`, `first_break` points to the oldest failed (or non-match), and the count of matches "above" it would over-count. With `MAX`, `first_break` correctly points to the most-recent non-match (the success in the middle), and only the latest two failed runs are counted.
- **Fix:** Use `MAX(start_time) AS break_time` on both SQLite and Postgres branches, then count rows whose `start_time > COALESCE(break_time, '1970-01-01T00:00:00Z')`.
- **Files modified:** `src/webhooks/coalesce.rs`, `tests/v12_webhook_filter_position_explain.rs`
- **Verification:** All 4 coalesce unit tests pass with MAX (the basic-streak, stops-at-success, D-13, and D-15 sentinel scenarios). SQLite EXPLAIN test still asserts `idx_job_runs_job_id_start` is used.
- **Committed in:** `44ea991` (Task 2 commit).

---

**Total deviations:** 1 auto-fixed (1 Rule 1 bug)
**Impact on plan:** Single-line aggregate change in two SQL constants + test scaffold. No scope creep — this is a correctness fix on a plan template that was internally inconsistent with the locked test scenarios (D-13 and D-15 unit tests would have failed under the MIN aggregate).

## Threat Surface Scan

The threat register listed in `<threat_model>` was honored:

- **T-18-10 (byte determinism):** mitigated by `payload_serializes_deterministically_to_compact_json` test (asserts `serde_json::to_vec(&p)` returns identical bytes on repeat).
- **T-18-11 (RFC3339 drift):** mitigated by `payload_timestamps_use_z_suffix` test (asserts `started_at.ends_with('Z')` and not `+00:00`).
- **T-18-12 (compact JSON whitespace):** mitigated by `payload_serializes_deterministically_to_compact_json` test (asserts `!body.contains(b'\n')`).
- **T-18-13 (SQL injection via states):** mitigated by sqlx bind parameters (no string interpolation); the `pad_states_to_6` helper produces owned `String` values bound through `sqlx::query::bind`.
- **T-18-14 (DoS via slow query):** mitigated by single-query backwards walk + `idx_job_runs_job_id_start` covering index; SQLite EXPLAIN test asserts the index hit.
- **T-18-15 (placeholder mismatch):** mitigated by Pitfall I bind-padding — the SQL has exactly 6 IN-list placeholders, and `pad_states_to_6` always produces exactly 6 strings.
- **T-18-37 (operator success-in-states misconfig):** mitigated by D-15 hard-coded sentinel in CASE expression on BOTH backends; locked by `filter_position_treats_success_as_break_even_when_in_states` regression test.

No NEW threat surface was introduced beyond what the threat register anticipated.

## Issues Encountered

- `cargo fmt --check` flagged a single-line `panic!(...)` over 100 columns; ran `cargo fmt` and recommitted via the same Task 2 commit before staging. No code semantics changed.

## User Setup Required

None — this plan ships pure-Rust modules + tests. No env vars, no external services, no schema migrations.

## Next Phase Readiness

- Plan 04 (HttpDispatcher) consumes both deliverables verbatim:
    - `WebhookPayload::build(...) -> serde_json::to_vec` produces the bytes for HMAC signing
    - `coalesce::filter_position(...)` drives the `fire_every` modular-arithmetic decision (D-16)
- No outstanding blockers. The dispatcher's only additional Plan-03-side dependency is the `cronduit_version: &'static str` constructor argument, which the dispatcher will pass as `env!("CARGO_PKG_VERSION")`.
- rustls invariant intact: `cargo tree -i openssl-sys` returns empty.
- Note for the merging orchestrator: this plan does NOT add the `ulid` crate to Cargo.toml. Plan 04 (dispatcher) is the consumer of `webhook-id` ULIDs and is responsible for adding the dep there. Plan 18-01's expected `ulid` Cargo.toml addition does not impact this plan's build because no Plan 03 code path touches ULID generation.

## Self-Check: PASSED

Verified each claim:

- `src/webhooks/payload.rs` exists: FOUND
- `src/webhooks/coalesce.rs` exists: FOUND
- `src/webhooks/mod.rs` updated with pub mod coalesce/payload + pub use WebhookPayload: FOUND
- `tests/v12_webhook_filter_position_explain.rs` exists: FOUND
- Commit `645ed12` (Task 1): FOUND in `git log`
- Commit `44ea991` (Task 2): FOUND in `git log`
- 9 payload unit tests pass: VERIFIED via `cargo test --lib --all-features webhooks::payload`
- 4 coalesce unit tests pass: VERIFIED via `cargo test --lib --all-features webhooks::coalesce`
- 1 SQLite EXPLAIN test passes (1 Postgres ignored as expected): VERIFIED via `cargo test --test v12_webhook_filter_position_explain`
- `cargo build --workspace` exits 0: VERIFIED
- `cargo clippy --all-targets --all-features -- -D warnings` exits 0: VERIFIED
- `cargo fmt --check` exits 0: VERIFIED
- `cargo tree -i openssl-sys` returns empty: VERIFIED (rustls invariant intact)

---
*Phase: 18-webhook-payload-state-filter-coalescing*
*Plan: 03*
*Completed: 2026-04-29*
