---
phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
plan: 01
subsystem: database
tags: [webhooks, dlq, sqlx, retention, migrations, sqlite, postgres, scaffolding]

# Dependency graph
requires:
  - phase: 15-foundation-preamble
    provides: webhook delivery worker scaffolding + bounded mpsc channel
  - phase: 19-webhook-hmac-signing-receiver-examples
    provides: stable Phase 19 baseline (PR #53 squash-commit) + existing v12_webhook_*.rs test layout to mirror
provides:
  - "Paired SQLite + Postgres `webhook_deliveries` DLQ table migrations (additive, IF NOT EXISTS, schema-parity green)"
  - "`pub struct WebhookDlqRow` (9 fields, 1:1 with D-10) in src/db/queries.rs"
  - "`pub async fn insert_webhook_dlq_row` dual-dialect helper (sqlite + postgres)"
  - "`pub async fn delete_old_webhook_deliveries_batch` dual-dialect retention helper"
  - "Phase 4 webhook_deliveries delete loop in src/scheduler/retention.rs (mirrors Phase 1/2 batch pattern; WAL sum widened)"
  - "7 compiling Wave 0 test stub files in tests/v12_webhook_*.rs (retry, retry_classification, retry_after, drain, dlq, https_required, metrics_family)"
affects: [20-02, 20-03, 20-04, 20-05]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Dual-dialect helper pattern (match pool.writer() { Sqlite => ?N | Postgres => $N }) reused for INSERT and DELETE on webhook_deliveries"
    - "Phase 4 retention loop mirrors Phase 1/Phase 2 (BATCH_SIZE=1000, BATCH_SLEEP=100ms, cancel-check between batches, error/break on Err)"
    - "Wave 0 stub files: header comment + #[allow(dead_code)] PHASE_MARKER const so cargo discovery sees a compiling test binary; downstream plans append #[tokio::test] async fns"

key-files:
  created:
    - "migrations/sqlite/20260502_000008_webhook_deliveries_add.up.sql"
    - "migrations/postgres/20260502_000008_webhook_deliveries_add.up.sql"
    - "tests/v12_webhook_retry.rs"
    - "tests/v12_webhook_retry_classification.rs"
    - "tests/v12_webhook_retry_after.rs"
    - "tests/v12_webhook_drain.rs"
    - "tests/v12_webhook_dlq.rs"
    - "tests/v12_webhook_https_required.rs"
    - "tests/v12_webhook_metrics_family.rs"
  modified:
    - "src/db/queries.rs"
    - "src/scheduler/retention.rs"

key-decisions:
  - "Phase 4 placement: AFTER Phase 2 runs delete and BEFORE Phase 3 WAL checkpoint, so the WAL threshold sums all three deletes (per CONTEXT D-14 + PATTERNS.md guidance)"
  - "Index strategy: single index on last_attempt_at only — NO composite (job_id, last_attempt_at) index in v1.2 (D-11)"
  - "Schema fields: exactly the 9 columns from CONTEXT D-10 verbatim; NO payload/headers/signature columns (D-12 secret/PII hygiene; receivers re-derive payload from run_id)"
  - "Retention pruner reuses [server].log_retention as the cutoff knob — no second config field for webhook DLQ retention (D-14)"
  - "WebhookDlqRow uses owned Strings for cross-await safety; constructor (Plan 02) is responsible for the <=500 char truncation of last_error per D-10"

patterns-established:
  - "Migration sequence _000008: continues the additive-migration date-and-counter naming pattern set by _000005, _000006, _000007"
  - "Dual-dialect INSERT helper: 9 placeholders (?1..?9 / $1..$9), bind by value/ref ordering matches the column tuple"
  - "Dual-dialect DELETE-batch helper: SQLite uses rowid IN (SELECT rowid ... LIMIT N), Postgres uses id IN (SELECT id ... LIMIT N)"

requirements-completed: [WH-05]

# Metrics
duration: 9min
completed: 2026-05-01
---

# Phase 20 Plan 01: Persistence + Retention + Wave 0 Test Scaffolding Summary

**Locked the `webhook_deliveries` DLQ table contract on both SQLite and Postgres, added dual-dialect insert/retention helpers, extended the daily pruner with a Phase 4 webhook_deliveries delete loop, and seeded 7 compiling Wave 0 test stub files so downstream Phase 20 plans can append `#[tokio::test]` functions without cargo-discovery flapping.**

## Performance

- **Duration:** ~9 min
- **Started:** 2026-05-01T19:51:04Z
- **Completed:** 2026-05-01T19:59:59Z
- **Tasks:** 4
- **Files modified:** 11 (9 created, 2 modified)

## Accomplishments

- DLQ schema is now operator-visible on first `cronduit run` post-merge in BOTH backends (CONTEXT D-10 schema verbatim, D-11 single-index strategy, D-12 no-payload/no-headers/no-signature hygiene, D-13 paired-files-in-same-PR pattern)
- `WebhookDlqRow` + `insert_webhook_dlq_row` + `delete_old_webhook_deliveries_batch` ready for Plan 02's `RetryingDispatcher` and Plan 04's drain logic to consume — no schema-shape uncertainty downstream
- Daily retention pruner now deletes `webhook_deliveries` rows older than `[server].log_retention`; WAL checkpoint threshold sums all three Phase deletes (logs + runs + webhook_dlq); final tracing log gains `webhook_dlq_deleted` field
- 7 Wave 0 stub files compile cleanly under `cargo check --tests`, each with a `PHASE_MARKER` constant pointing to the Phase 20 / WH-* requirement they will cover

## Task Commits

Each task was committed atomically:

1. **Task 1: Create paired SQLite + Postgres webhook_deliveries migration files** — `c6de21a` (feat)
2. **Task 2: Add WebhookDlqRow struct + insert_webhook_dlq_row + delete_old_webhook_deliveries_batch helpers** — `ba250b3` (feat)
3. **Task 3: Extend retention pruner with Phase 4 webhook_deliveries delete loop** — `613717e` (feat)
4. **Task 4: Create 7 Wave 0 test stub files in tests/v12_webhook_*.rs** — `68c4bf7` (test)

## Files Created/Modified

**Created:**
- `migrations/sqlite/20260502_000008_webhook_deliveries_add.up.sql` — DLQ table create (INTEGER PK AUTOINCREMENT, 9 cols, FKs, single index on `last_attempt_at`)
- `migrations/postgres/20260502_000008_webhook_deliveries_add.up.sql` — Postgres mirror (BIGSERIAL PK, BIGINT FK cols, identical column-order otherwise)
- `tests/v12_webhook_retry.rs` — Wave 0 stub for WH-05 retry chain
- `tests/v12_webhook_retry_classification.rs` — Wave 0 stub for D-06 classification table
- `tests/v12_webhook_retry_after.rs` — Wave 0 stub for D-07/D-08 Retry-After
- `tests/v12_webhook_drain.rs` — Wave 0 stub for WH-10 drain budget
- `tests/v12_webhook_dlq.rs` — Wave 0 stub for D-10 DLQ rows
- `tests/v12_webhook_https_required.rs` — Wave 0 stub for WH-07 HTTPS validator
- `tests/v12_webhook_metrics_family.rs` — Wave 0 stub for WH-11 labeled metric family

**Modified:**
- `src/db/queries.rs` — added `pub struct WebhookDlqRow` + `insert_webhook_dlq_row` + `delete_old_webhook_deliveries_batch` (113 inserted lines after `delete_old_runs_batch`, before `wal_checkpoint`)
- `src/scheduler/retention.rs` — new Phase 4 webhook_deliveries delete loop at lines 132–172; WAL sum at line 175 widened to include `total_webhook_dlq_deleted`; final tracing log at lines 192–198 gains `webhook_dlq_deleted` field

## Decisions Made

None new — followed plan and CONTEXT D-10..D-14 verbatim. The plan locked all material decisions (schema columns, index strategy, migration naming `_000008`, retention placement after Phase 2 / before Phase 3, Wave 0 stub format) before execution started.

## Deviations from Plan

None — plan executed exactly as written.

The plan's per-task `<read_first>` lists were comprehensive enough that no auto-fix rules fired. The existing `delete_old_logs_batch` and `delete_old_runs_batch` helpers were perfect reference shapes (lines 1434–1511 in src/db/queries.rs); the existing Phase 1 and Phase 2 retention loops at retention.rs lines 55–130 were a perfect template for Phase 4. All `cargo check`, `cargo nextest`, and grep acceptance criteria passed on first run.

## Issues Encountered

None.

The only minor observation: `cargo nextest run --lib scheduler::retention` reports "no tests to run" because the retention module currently has no inline `#[cfg(test)]` block. This is expected (Phase 1/2 deletes likewise have no inline unit tests; coverage lives in integration tests under `tests/`). The retention pruner's behavior is implicitly verified by `cargo check --lib` (compiles + types align with `delete_old_webhook_deliveries_batch`'s signature) and the broader 257-test `cargo nextest run --lib` suite which passed end-to-end after Task 3.

## Verification Run

```
cargo check --all-targets        # PASS (warning only: tailwind binary not built — not a code issue)
cargo nextest run --test schema_parity   # 3/3 PASS
cargo nextest run --lib                  # 257/257 PASS
cargo tree -i openssl-sys                # "did not match any packages" (rustls invariant intact, D-38)
git diff a59a0331..HEAD -- Cargo.toml    # empty (D-38 — no new external crates)
```

## Threat Model Mitigations Applied

- **T-20-02 (Information Disclosure / DLQ schema):** Both migration files have NO `payload`, `headers`, or `signature` columns. Verified by `grep -vE '^--' migrations/.../...sql | grep -E 'payload|signature|headers|body'` returning empty. The only matches in the full file `grep` are inside the comment block where the policy is documented.
- **T-20-04 (Reliability / pruner config sprawl):** Phase 4 pruner reuses the existing `[server].log_retention` knob — no second config field; one cadence to reason about.
- **T-20-05 (DoS / unbounded row growth):** Daily Phase 4 prune (BATCH_SIZE=1000, BATCH_SLEEP=100ms) bounds growth; single index on `last_attempt_at` keeps the cutoff query plan efficient.

## Threat Flags

None — this plan only adds an additive table + helpers + retention extension. No new network endpoints, no new auth paths, no file access pattern changes.

## Next Phase Readiness

- **Plan 02 (RetryingDispatcher):** Can now call `queries::insert_webhook_dlq_row(&pool, WebhookDlqRow { ... })` from `src/webhooks/retry.rs` with a fully-typed row struct — no schema-shape uncertainty.
- **Plan 03 (HTTPS validator):** Can append `#[tokio::test]` functions to `tests/v12_webhook_https_required.rs` without creating a new test binary mid-PR.
- **Plan 04 (drain budget):** Can append `#[tokio::test]` functions to `tests/v12_webhook_drain.rs` and call `insert_webhook_dlq_row` for the `dlq_reason = "shutdown_drain"` path.
- **Plan 05 (metrics migration):** Can append `#[tokio::test]` functions to `tests/v12_webhook_metrics_family.rs`.

No blockers or concerns.

## Self-Check: PASSED

Verified files exist:
- FOUND: migrations/sqlite/20260502_000008_webhook_deliveries_add.up.sql
- FOUND: migrations/postgres/20260502_000008_webhook_deliveries_add.up.sql
- FOUND: tests/v12_webhook_retry.rs
- FOUND: tests/v12_webhook_retry_classification.rs
- FOUND: tests/v12_webhook_retry_after.rs
- FOUND: tests/v12_webhook_drain.rs
- FOUND: tests/v12_webhook_dlq.rs
- FOUND: tests/v12_webhook_https_required.rs
- FOUND: tests/v12_webhook_metrics_family.rs
- MODIFIED: src/db/queries.rs (WebhookDlqRow + 2 helpers added)
- MODIFIED: src/scheduler/retention.rs (Phase 4 block + WAL sum + final log)

Verified commits exist:
- FOUND: c6de21a (Task 1 — migrations)
- FOUND: ba250b3 (Task 2 — queries.rs helpers)
- FOUND: 613717e (Task 3 — retention.rs Phase 4)
- FOUND: 68c4bf7 (Task 4 — 7 stub files)

---
*Phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1*
*Plan: 01*
*Completed: 2026-05-01*
