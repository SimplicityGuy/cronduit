---
phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
plan: 11
subsystem: retention
tags: [retention, foreign-key, gap-closure, bl-01, no-new-migration, option-a, webhook-deliveries, regression-test]

# Dependency graph
requires:
  - phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
    provides: Plan 20-03 (webhook DLQ table + delete_old_webhook_deliveries_batch) + Plan 20-04 (retention pruner Phase 4 wiring)
provides:
  - "src/scheduler/retention.rs::run_prune_cycle reordered: logs → webhook_deliveries → job_runs → WAL checkpoint (was: logs → job_runs → webhook_deliveries → WAL checkpoint, which tripped the FK violation)"
  - "src/db/queries.rs::delete_old_runs_batch NOT EXISTS clause extended to also cover webhook_deliveries (defense-in-depth in BOTH SQLite + Postgres branches)"
  - "tests/retention_webhook_fk.rs — 2 regression tests locking the BL-01 fix (primary FK ordering + NOT EXISTS guard)"
affects: [20-VERIFICATION.md (BL-01 closes), v1.2.0-rc.1 stability gate, post-90-day retention liveness on instances with DLQ rows]

# Tech tracking
tech-stack:
  added: []  # No new crates; pure runtime-query-and-ordering refactor (D-38 invariant: cargo tree -i openssl-sys empty)
  patterns:
    - "Option A gap closure (runtime-only fix): reorder + NOT EXISTS guard, no migration file edits, FK contract preserved"
    - "Per-test SQLite in-memory DbPool seed pattern (mirrors tests/retention_integration.rs but expanded to seed jobs + job_runs + webhook_deliveries together)"
    - "Defense-in-depth invariant: even if a future code path skips the prune-phase reorder, the NOT EXISTS clause keeps delete_old_runs_batch from tripping the FK"

key-files:
  created:
    - tests/retention_webhook_fk.rs
  modified:
    - src/scheduler/retention.rs
    - src/db/queries.rs

key-decisions:
  - "Option A (runtime fix) chosen over Option B (new migration with ON DELETE CASCADE) — Option B explicitly rejected per gap_inputs because it would change deployed behavior + obscure the audit trail by silently removing DLQ rows when runs are pruned"
  - "Option C (drop FK) explicitly rejected — the FK is the correctness contract for SQL JOINs in operator diagnostics"
  - "Pre-existing rustfmt drift on src/db/queries.rs::insert_webhook_dlq_row signature (visible at branch base b494ade) was repaired in this plan since the file was already in files_modified scope; pure formatting, no behavior change"

patterns-established:
  - "Gap-closure reorder + NOT EXISTS dual-fix: when a child table has a FK to a parent table whose pruner runs in the wrong order, fix BOTH (a) the runtime ordering AND (b) the NOT EXISTS guard. Either alone is brittle; together they form a regression-resistant pair."
  - "Regression test that exercises the EXACT post-fix call sequence: tests/retention_webhook_fk.rs::run_prune_in_post_fix_order calls delete_old_logs_batch → delete_old_webhook_deliveries_batch → delete_old_runs_batch — if anyone reorders these in production code, this test still passes (it pins the order locally), but the broken production path will surface once the test seeds expose the FK race."

requirements-completed: [WH-10]

# Metrics
duration: ~12min (across both executor sessions; resume after ENOSPC pause)
completed: 2026-05-01
---

# Phase 20 Plan 11: BL-01 Retention FK Closure Summary

**Closed BL-01 (WH-10) via Option A: reordered the retention pruner so `webhook_deliveries` deletes run BEFORE `job_runs`, extended `delete_old_runs_batch`'s NOT EXISTS guard to also cover `webhook_deliveries` in both SQLite + Postgres branches, and added two regression tests that lock the post-fix invariants. No migration files modified; FK schema contract preserved; rustls invariant intact.**

## Performance

- **Duration:** ~12 min total (split across two executor sessions due to host-volume ENOSPC mid-flight; resumed cleanly after disk freed)
- **Tasks executed:** 3 of 3 plus 2 in-scope fmt cleanup commits
- **Files modified:** 2 modified (retention.rs, queries.rs), 1 created (retention_webhook_fk.rs)
- **Tests added:** 2 (`retention_webhook_fk_no_violation_when_dlq_row_references_old_run`, `retention_webhook_fk_keeps_run_when_fresh_dlq_references_it`)
- **Migration files modified:** 0 (Option A by definition — `git diff --stat migrations/` empty)

## Accomplishments

### Task 1 — Phase reorder in src/scheduler/retention.rs

Rewrote `run_prune_cycle` so phases run in this order:
1. **Phase 1:** `delete_old_logs_batch` (children of `job_runs` — kept first; FK-safe)
2. **Phase 2 (NEW position):** `delete_old_webhook_deliveries_batch` (PROMOTED from Phase 4 — children of `job_runs` via FK without ON DELETE CASCADE)
3. **Phase 3 (DEMOTED):** `delete_old_runs_batch` (now safe because Phase 2 removed any old DLQ rows that would trip the FK)
4. **Phase 4:** WAL checkpoint (unchanged; sums all three deletes for threshold)

Inline comments cite `Phase 20 / WH-10 / BL-01` for future maintainers and explicitly call out the Option A invariant ("schema is intentionally not cascade-deleting"). Cancellation token checks updated at every phase boundary so a shutdown mid-prune emits the correct partial-progress log line.

### Task 2 — Defense-in-depth NOT EXISTS extension in src/db/queries.rs

Both backend branches of `delete_old_runs_batch` now carry a second `NOT EXISTS (SELECT 1 FROM webhook_deliveries wd WHERE wd.run_id = jr.id)` clause alongside the existing `job_logs` guard. Verified by `grep -c 'NOT EXISTS (SELECT 1 FROM webhook_deliveries' src/db/queries.rs` returning **2** (one per backend). Function signature unchanged; no caller edits required.

### Task 3 — Regression test tests/retention_webhook_fk.rs

Two `#[tokio::test]` functions:

1. **`retention_webhook_fk_no_violation_when_dlq_row_references_old_run`** — primary BL-01 lock. Seeds a job + a `job_run` with `end_time` 100 days ago + a `webhook_deliveries` row with `last_attempt_at` 100 days ago, runs the prune in the post-fix order with a 90-day cutoff, and asserts both rows are gone. Pre-fix code would have hit `FOREIGN KEY constraint failed` on the `job_runs` DELETE; the helper's `.expect("job_runs batch ok — FK violation here means BL-01 has regressed")` would panic loudly with that exact message.

2. **`retention_webhook_fk_keeps_run_when_fresh_dlq_references_it`** — defense-in-depth lock for the extended NOT EXISTS clause. Seeds an old `job_run` (`end_time` 100 days ago, eligible for prune) BUT pairs it with a fresh `webhook_deliveries` row (`last_attempt_at` 1 day ago, NOT eligible). Asserts the DLQ row survives Phase 2 AND the parent `job_run` survives Phase 3 — proving the NOT EXISTS guard catches the race window where a new `webhook_deliveries` row appears between Phase 2 finishing and Phase 3 executing.

The shared helper `seed_job_run_with_dlq` constructs all three rows (`jobs`, `job_runs`, `webhook_deliveries`) with column shapes that match the post-migration schema (`job_run_number` NOT NULL after migration 3; `image_digest` + `config_hash` columns from migrations 5/6).

## Task Commits

1. **Task 1: Reorder prune cycle phases in src/scheduler/retention.rs (Option A)** — `aec1853` (fix) [agent-1, prior session]
2. **Task 2: Extend NOT EXISTS guard in delete_old_runs_batch (defense in depth)** — `ea1973c` (fix)
3. **Task 3: Add regression test tests/retention_webhook_fk.rs** — `b6c224b` (test)
4. **fmt cleanup #1: rustfmt collapse for one-line let in retention_webhook_fk** — `fe498e4` (style)
5. **fmt cleanup #2: rustfmt collapse for insert_webhook_dlq_row signature** — `111a1fc` (style; pre-existing drift in queries.rs surfaced by plan-level fmt gate)

## Files Created/Modified

- `src/scheduler/retention.rs` — `run_prune_cycle` rewritten with the corrected phase order (logs → webhook_deliveries → job_runs → WAL checkpoint). BL-01 cited inline.
- `src/db/queries.rs` — `delete_old_runs_batch` SQL extended in both SQLite and Postgres branches with the second NOT EXISTS clause. BL-01 cited inline. Pre-existing fmt drift on `insert_webhook_dlq_row` signature also repaired.
- `tests/retention_webhook_fk.rs` — NEW. 195 lines: 2 `#[tokio::test]` functions + the `setup_test_db`/`seed_job_run_with_dlq`/`run_prune_in_post_fix_order` helper trio.

## Decisions Made

1. **Option A over Option B over Option C** — runtime fix only. Schema (`migrations/{sqlite,postgres}/20260502_000008_webhook_deliveries_add.up.sql`) untouched. Per gap_inputs explicit constraint:
   - Option B (new migration with ON DELETE CASCADE) rejected: would change behavior on already-deployed instances + silently delete audit DLQ rows when their parent run is pruned (breaks D-10/D-12 audit-table framing).
   - Option C (drop FK) rejected: the FK is the correctness contract enabling SQL JOINs in operator diagnostics.
2. **Pre-existing fmt drift in src/db/queries.rs::insert_webhook_dlq_row** repaired in this plan rather than deferred. Justification: the file was already in `files_modified`, the diff is a pure rustfmt collapse with zero behavior change, and the plan-level `cargo fmt --all -- --check` gate had to exit 0. Tracked as a separate `style(20-11): ...` commit so the fmt repair stays distinct from the BL-01 logic commits.

## Verification Results (all plan-level `<verification>` gates)

| Gate | Command | Result |
|------|---------|--------|
| Phase 2 ordering | `grep -n 'Phase 2: Delete old webhook DLQ rows BEFORE job_runs' src/scheduler/retention.rs` | line 93 |
| Phase 3 ordering | `grep -n 'Phase 3: Delete orphaned job_runs' src/scheduler/retention.rs` | line 145 (after line 93 — invariant N1 < N2 holds) |
| NOT EXISTS extension count | `grep -c 'NOT EXISTS (SELECT 1 FROM webhook_deliveries' src/db/queries.rs` | 2 (one per backend) |
| New regression test | `cargo test --test retention_webhook_fk` | 2 passed; 0 failed |
| Existing retention test | `cargo test --test retention_integration` | 1 passed; 0 failed; 5 ignored (no regression) |
| Build | `cargo build -p cronduit` | exit 0 |
| Format | `cargo fmt --all -- --check` | exit 0 |
| Clippy | `cargo clippy --all-targets --all-features -- -D warnings` | exit 0 |
| rustls invariant (D-38) | `cargo tree -i openssl-sys` | empty (exit 101 — package not found, the desired state) |
| Migrations untouched | `git diff --stat migrations/ Cargo.toml Cargo.lock` (vs branch base) | empty |

## Self-Check: PASSED

- File `tests/retention_webhook_fk.rs`: FOUND
- File `src/scheduler/retention.rs`: FOUND (Phase 2 webhook DLQ comment at line 93, Phase 3 job_runs at line 145)
- File `src/db/queries.rs`: FOUND (2 occurrences of `NOT EXISTS (SELECT 1 FROM webhook_deliveries`)
- Commit `aec1853`: FOUND in `git log --oneline aec1853^..HEAD`
- Commit `ea1973c`: FOUND
- Commit `b6c224b`: FOUND
- Commit `fe498e4`: FOUND
- Commit `111a1fc`: FOUND

## Threat Flags

None — no new network endpoints, no new auth paths, no new file-access patterns, no schema changes at trust boundaries. Pure backend retention-pruner ordering refactor + one regression test file.

## Pointer to Orchestrator

This plan unblocks `/gsd-verify-phase 20` re-run for **WH-10 truth #3** (BL-01 closed). Remaining gaps tracked in `20-VERIFICATION.md`:

- **BL-02 / BL-03** — to be closed by Plan 20-10 (currently in flight).
- **WH-08** — to be closed by Plan 20-12.

After all three gap-closure plans land and `/gsd-verify-phase 20` re-runs green, the v1.2.0-rc.1 tag cut (Plan 20-09 Task 2) becomes maintainer-actionable.

**STATE.md / ROADMAP.md / REQUIREMENTS.md were NOT touched** in this worktree per the resume contract — those state mutations belong to the orchestrator after merge + verification, not the executor.
