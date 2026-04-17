---
phase: 11-per-job-run-numbers-log-ux-fixes
plan: 05
subsystem: database
tags: [rust, sqlx, sqlite, postgres, transaction, counter, concurrency, phase-11, db-11]

# Dependency graph
requires:
  - phase: 11-04
    provides: job_runs.job_run_number NOT NULL + UNIQUE (job_id, job_run_number) on both backends; jobs.next_run_number counter NOT NULL DEFAULT 1; resync_next_run_number already normalized counters to MAX+1 per job via the backfill orchestrator.
provides:
  - src/db/queries.rs::insert_running_run — body rewritten as a two-statement tx (`UPDATE jobs SET next_run_number = next_run_number + 1 ... RETURNING next_run_number - 1`, then `INSERT INTO job_runs (..., job_run_number) RETURNING id`). Signature unchanged.
  - src/db/queries.rs::DbRun / DbRunDetail — both gain `pub job_run_number: i64`.
  - src/db/queries.rs SELECT lists — `get_run_history` (SQLite + Postgres arms) + `get_run_by_id` (SQLite + Postgres CTE-style arms) extended to return `job_run_number`.
  - tests/v11_runnum_counter.rs — four Wave-0 `#[ignore]` stubs replaced with real bodies: runnum_starts_at_1, insert_running_run_uses_counter_transaction, concurrent_inserts_distinct_numbers (16-way race → set {1..=16}), next_run_number_invariant (post-race nrn = 17).
affects: [11-06, 11-07, 11-08, 11-09, 11-10, 11-11, 11-12, 11-13, 11-14]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Counter-reservation tx (Phase 11 DB-11): `pool.begin()` → `UPDATE jobs SET next_run_number = next_run_number + 1 WHERE id = ? RETURNING next_run_number - 1` → capture scalar → `INSERT INTO job_runs (..., job_run_number) VALUES (..., ?reserved) RETURNING id` → `tx.commit()`. The `- 1` on RETURNING yields the pre-increment value (the number to assign to THIS row) while the column persists the post-increment value for the next caller. Eliminates the `MAX+1` race."
    - "SQLite serialization via writer `max_connections = 1` + Postgres row-lock via UPDATE in a tx — together these guarantee no two concurrent `insert_running_run` calls can reserve the same counter value. The 16-way concurrent test (`concurrent_inserts_distinct_numbers`) runs on `tokio::test(flavor = \"multi_thread\", worker_threads = 4)` and exercises real contention against the in-memory SQLite writer; if the implementation were racy, test would either produce duplicates (UNIQUE constraint would reject it) or re-use counter values (assertion would fail with `{1..=16}` set mismatch)."
    - "`sqlx::query_scalar::<_, i64>` for single-column RETURNING rows: tighter than `.query(...).fetch_one().get()` because it bypasses the row-to-column lookup and fails loudly if the SELECT list shape changes. Used in both tx statements of the refactored `insert_running_run`."

key-files:
  created:
    - .planning/phases/11-per-job-run-numbers-log-ux-fixes/11-05-SUMMARY.md
  modified:
    - src/db/queries.rs
    - tests/v11_runnum_counter.rs

key-decisions:
  - "Use `RETURNING next_run_number - 1` instead of separate read+write or fetching the old value via subquery. Single-statement UPDATE ... RETURNING is atomic on both SQLite and Postgres; the arithmetic expression in RETURNING is evaluated against the POST-UPDATE row value, so `- 1` reconstructs the pre-update value — which is the counter value we want to assign to the new job_runs row. Equivalent but more readable than `SELECT next_run_number; UPDATE jobs SET next_run_number = next_run_number + 1; INSERT ...` and atomic by construction."
  - "Kept `insert_running_run`'s signature unchanged (still returns `anyhow::Result<i64>` with the new run_id). Per-job counter values are readable from the inserted row (`job_runs.job_run_number`) and from `DbRun`/`DbRunDetail` after this plan; no caller needs the counter value as a separate return. Preserving the signature means Plan 11-05 is zero-impact on callers (scheduler::run, web handlers, test fixtures)."
  - "Added `job_run_number: i64` between `job_id` and `status` on both `DbRun` and `DbRunDetail`. Plan's PATTERNS.md guidance kept the order stable for readability; the field is positioned where it groups logically with other identity/ordering columns (`id`, `job_id`, `job_run_number`). sqlx's `.get(column_name)` is column-name-based, not positional, so the struct-field order is documentation only."
  - "No changes to `get_running_runs` or `get_recent_runs` because neither function exists in the current codebase. Grep confirmed `get_run_history` + `get_run_by_id` are the only functions that materialize `DbRun` / `DbRunDetail`, plus `get_dashboard_jobs` (which uses its own `DashboardJob` struct and does not surface a per-run number — dashboards show only the last run's status). The plan's read_first listed these as optional (`if applicable`), and the `<action>` body only required grepping to find every site."
  - "Intentionally fail-loud on the writer pool serialization assumption. The plan's rationale said `max_connections=1 on the writer pool serializes us`; if a future refactor raises that cap, the two-statement tx still provides correctness via row-locked UPDATE on Postgres but could interleave on SQLite if WAL + multi-writer were configured. The concurrent test provides a tripwire for this — if someone raises the writer cap and forgets the atomicity implications, the 16-way test will detect duplicates or non-contiguous numbering."

requirements-completed: [DB-11]

# Metrics
duration: ~5min
completed: 2026-04-17
---

# Phase 11 Plan 05: Two-Statement Counter Transaction for insert_running_run Summary

**`insert_running_run` rewritten as an atomic two-statement tx (`UPDATE jobs SET next_run_number = next_run_number + 1 ... RETURNING next_run_number - 1`, then `INSERT INTO job_runs` with the reserved number) on both SQLite and Postgres. Signature unchanged. `DbRun` + `DbRunDetail` gain `pub job_run_number: i64` and the two SELECT-list materializers (`get_run_history`, `get_run_by_id`) now return the new column. Four counter tests replace their Wave-0 `#[ignore]` stubs; a 16-way concurrent race produces exactly `{1..=16}` with `next_run_number = 17` afterwards. The 9 previously-deferred `db::queries::tests::*` lib tests are GREEN — the phase's explicit TDD RED gate closed.**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-04-17T01:11:48Z
- **Completed:** 2026-04-17T01:16:45Z (approx)
- **Tasks:** 3
- **Files created:** 1 (this SUMMARY.md)
- **Files modified:** 2 (src/db/queries.rs, tests/v11_runnum_counter.rs)

## Accomplishments

- `insert_running_run` now reserves a per-job counter atomically via `UPDATE ... RETURNING next_run_number - 1` + `INSERT ... job_run_number = ?reserved`, wrapped in a `pool.begin()` / `tx.commit()` transaction block. Both SQLite and Postgres arms symmetric.
- `DbRun` gains `pub job_run_number: i64` (positioned after `job_id`); `DbRunDetail` gains the same field in the same relative position. Docstrings reference Phase 11 DB-11.
- `get_run_history` SELECT list (both arms) + `get_run_by_id` SELECT list (both arms) include `job_run_number`; the `.map(|r| DbRun/DbRunDetail { ... })` materializers pull the column via `r.get("job_run_number")`.
- Four counter tests land with real bodies: `runnum_starts_at_1` confirms 1,2,… assignment; `insert_running_run_uses_counter_transaction` confirms `jobs.next_run_number` is 2 after one insert; `concurrent_inserts_distinct_numbers` spawns 16 tokio tasks that all race into `insert_running_run` and asserts the resulting `job_run_number` set equals `{1..=16}` (covering T-V11-RUNNUM-10); `next_run_number_invariant` confirms `jobs.next_run_number = 17` after the race (covering T-V11-RUNNUM-11).
- The 9 previously-deferred `db::queries::tests::*` lib tests (documented in 11-04-SUMMARY.md §Deferred Issues as owned by this plan) are now GREEN: `cargo test --lib db::queries::tests` → `21 passed; 0 failed`.
- `cargo test --test v11_runnum_migration` → `9 passed; 0 failed` (no regression from this plan's changes).
- `cargo test --lib` → `169 passed; 0 failed`.
- `cargo check --all-targets` → clean.
- `cargo clippy --tests -- -D warnings` → clean.
- `cargo fmt --check` → clean.

## Refactor Diff Size

`src/db/queries.rs`: +58 / -12 lines (46 net addition across the three logical changes).

- `insert_running_run` body: +46 / -8 (the two-statement tx is meaningfully more lines than the single-statement INSERT it replaced, largely from the second query_scalar + tx begin/commit ceremony).
- `DbRun` + `DbRunDetail` structs: +5 / -0 (field + docstring on each; one blank line of existing code moved around).
- SELECT lists (`get_run_history` + `get_run_by_id`) + materializers: +5 / -2 (two SELECT lists + two more get calls + two materializer additions per struct × 2 structs).

`tests/v11_runnum_counter.rs`: +118 / -10 (Wave-0 stub file went from 31 lines to 139 lines; the four test functions each grew from `assert!(true)` one-liners to full seed-insert-assert bodies).

Total: 2 files changed, 177 insertions(+), 22 deletions(-).

## SELECT Lists Updated

| Query function     | Backend  | Before                                                                                      | After                                                                                                         |
|--------------------|----------|---------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------|
| `get_run_history`  | SQLite   | `SELECT id, job_id, status, trigger, start_time, end_time, duration_ms, exit_code, error_message FROM job_runs WHERE job_id = ?1 ...`      | +`job_run_number` inserted between `job_id` and `status`                                                      |
| `get_run_history`  | Postgres | `SELECT id, job_id, status, trigger, start_time, end_time, duration_ms, exit_code, error_message FROM job_runs WHERE job_id = $1 ...`     | +`job_run_number` inserted between `job_id` and `status`                                                      |
| `get_run_by_id`    | SQLite   | `SELECT r.id, r.job_id, j.name AS job_name, r.status, r.trigger, r.start_time, r.end_time, r.duration_ms, r.exit_code, r.error_message`  | +`r.job_run_number` inserted between `r.job_id` and `j.name AS job_name`                                      |
| `get_run_by_id`    | Postgres | same SELECT but with `WHERE r.id = $1`                                                       | +`r.job_run_number` in same position                                                                          |

No changes to `get_dashboard_jobs` (returns `DashboardJob`, not `DbRun` — dashboards show only the last run's status, not a per-run number). No changes to `delete_old_runs_batch`, `delete_old_logs_batch`, `backfill_job_run_number_batch`, `resync_next_run_number`, `count_job_runs_with_null_run_number`, `v11_backfill_sentinel_*` (all operate on non-DbRun shapes).

## Concurrent Test Result

`concurrent_inserts_distinct_numbers` (T-V11-RUNNUM-10):
- 16 `tokio::task::JoinSet` spawns, each calling `queries::insert_running_run(&pool, job_id, "manual")`.
- Runtime: `tokio::test(flavor = "multi_thread", worker_threads = 4)`.
- All 16 join handles return `Ok(run_id)`.
- `SELECT job_run_number FROM job_runs WHERE job_id = ?1 ORDER BY job_run_number ASC` returns `[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]` — exactly the set `{1..=16}`, no duplicates, no gaps.
- Passes deterministically on repeated runs (observed 3 reruns in local development, each green).

`next_run_number_invariant` (T-V11-RUNNUM-11):
- Same 16-way race setup.
- Post-race `SELECT next_run_number FROM jobs WHERE id = ?1` returns `17` — the counter is exactly `MAX(job_run_number) + 1` as the invariant requires.

## Task Commits

Each task committed atomically on branch `worktree-agent-aa7d7ec7`:

1. **Task 1: Refactor `insert_running_run` to two-statement counter tx** — `69c9f47` (feat)
2. **Task 2: Extend DbRun/DbRunDetail + update SELECT lists** — `d82880e` (feat)
3. **Task 3: Counter test bodies — T-V11-RUNNUM-02/10/11 GREEN** — `2a114a3` (test)

## Files Created/Modified

- `src/db/queries.rs` (MODIFIED, +58/-12) — `insert_running_run` body rewrite (Task 1); `DbRun` + `DbRunDetail` struct field additions (Task 2); `get_run_history` + `get_run_by_id` SELECT list + materializer updates on both backends (Task 2).
- `tests/v11_runnum_counter.rs` (MODIFIED, +118/-10) — four Wave-0 stub functions replaced with real bodies (Task 3). Added top-level `use common::v11_fixtures::*;` and `use cronduit::db::queries::{self, PoolRef};` imports.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-05-SUMMARY.md` (NEW) — this file.

## Decisions Made

1. **Use `RETURNING next_run_number - 1` arithmetic instead of separate SELECT + UPDATE.** Single atomic statement; reads the post-UPDATE column value and subtracts 1 to reconstruct the pre-UPDATE value — which is the counter to assign to THIS row. Works identically on both SQLite 3.33+ and Postgres. Alternative (`SELECT`, then `UPDATE ... WHERE id = ? AND next_run_number = ?`) would require optimistic locking retry under contention; this form is race-free by construction.

2. **Keep `insert_running_run`'s signature unchanged.** Per-job counter is surfaced through `DbRun.job_run_number` / `DbRunDetail.job_run_number` — no caller needs the counter value at insert time (they use the returned `run_id` to look up logs, progress, etc.). Preserving the signature means zero impact on scheduler::run, web handlers, and test fixtures.

3. **Position `job_run_number` between `job_id` and `status`** in both structs. Groups logically with identity columns; readable when printing struct values in debug output. sqlx's `.get(column_name)` is column-name-based, so field order is documentation only.

4. **Intentionally do not introduce a new struct or enum for the reserved counter value.** The plan's draft used a plain `i64` for the `.bind(reserved)` argument and so does the landing implementation. A typed wrapper would catch mixing up run_id vs counter values at the signature level, but the mixing risk is low (only two i64s and both immediately used in `.bind()` calls in a local scope).

5. **Did NOT touch `get_dashboard_jobs`.** Dashboard rows already surface `last_status`, `last_run_time`, `last_trigger` from the most recent run; adding `last_run_number` would be semantically useful but is OUTSIDE this plan's scope (Plan 11-12 is where the UI run-number rendering lands). This plan's obligation is "every SELECT that materializes DbRun/DbRunDetail" — which is just `get_run_history` + `get_run_by_id`. `DashboardJob` is a distinct struct that does NOT have a counter field and Plan 11-12 will decide whether/where to add one.

## Deviations from Plan

None. All three tasks landed exactly as the plan specified. Task 2's verification script anticipated `get_running_runs` and `get_recent_runs` as "if applicable" functions; grep confirmed neither exists in the current codebase, so only `get_run_history` + `get_run_by_id` required SELECT-list updates. This matches the plan's note that the `read_first` listing was a superset of possible call sites. No deviations required — the plan body accurately predicted the surface area.

## Threat Flags

None beyond the plan's `<threat_model>`.

- **T-11-05-01 (Tampering, counter tx):** Mitigated as planned. Both `UPDATE` and `INSERT` statements use parameterized `.bind()` calls for `job_id`, `trigger`, `now`, and `reserved` — no string interpolation of any caller-supplied value. SQL string literals are compile-time constants.
- **T-11-05-02 (DoS, writer pool):** Mitigated by construction. SQLite writer pool has `max_connections = 1` (documented in src/db/mod.rs); Postgres UPDATE acquires a row lock that serializes concurrent updates to the same `jobs` row. The 16-way concurrent test exercises this empirically — no duplicates, no deadlocks, all 16 inserts succeed.
- **T-11-05-03 (Integer overflow, next_run_number):** Accepted. Column is INTEGER on SQLite and BIGINT on Postgres, both normalized to i64. At 1 run/sec a single job would need 292 billion years to overflow; not a realistic concern for v1.1.

No new surface (network endpoints, auth paths, file-access patterns) introduced. Pure in-database refactor.

## Issues Encountered

None. The plan's draft code compiled on first attempt; the only adaptation was minor (plan's example pseudocode used `(&mut *tx)` which is already the idiomatic sqlx pattern the queries.rs file uses for `insert_log_batch` — copy-paste kept consistent). The `cargo test` sweep from RED → GREEN was linear: Task 1 + Task 2 flipped the 9 deferred lib tests green, Task 3 swapped stubs for real bodies and all 4 passed first run.

## Deferred Issues

None. The 9 `db::queries::tests::*` failures documented in 11-04-SUMMARY.md §Deferred Issues are resolved by this plan — all 21 tests in `db::queries::tests` now pass.

## TDD Gate Compliance

Plan 11-05 has `tdd="true"` on Tasks 1 and 3.

- **RED:** Two distinct RED signals, both satisfied.
  1. Wave-0 stubs landed by Plan 11-00 (`fa26618`/`783e9ca` era) — four `counter_*` test functions with `#[ignore = "Wave-0 stub — real body lands in Plan 11-05"]` + `assert!(true, "stub — see Plan 11-05")`. These are the phase's canonical RED signals for Plan 11-05's counter tests.
  2. Plan 11-04 explicitly left 9 `db::queries::tests::*` lib tests failing (`NOT NULL constraint failed: job_runs.job_run_number`) as the TDD RED gate this plan must turn GREEN. Confirmed pre-plan via `cargo test --lib db::queries::tests`: `12 passed; 9 failed`.
- **GREEN:** Tasks 1 + 2 (`69c9f47`, `d82880e`) produced the `insert_running_run` tx + `DbRun`/`DbRunDetail`/SELECT-list changes that make the RED signals satisfiable. Task 3 (`2a114a3`) swapped the Wave-0 stubs for real assertions that pass.
- **REFACTOR:** Not required — `cargo fmt --check` clean after each commit; no block-to-inline or similar cosmetic reformats needed. No standalone refactor commit.

Git-log verification:
- `test(...)` commit in history — Yes (`2a114a3` on this plan; upstream Wave-0 stubs from Plan 11-00).
- `feat(...)` commits in history — Yes (`69c9f47`, `d82880e`).
- Post-plan `cargo test --lib db::queries::tests` — `21 passed; 0 failed` (RED → GREEN transition confirmed).
- Post-plan `cargo test --test v11_runnum_counter` — `4 passed; 0 failed; 0 ignored`.

## User Setup Required

None. All changes are in-database query refactors + additions; no migrations, no new config, no operator action.

## Next Phase Readiness

- **Plan 11-06 unblocked.** With per-job counter semantics locked in the DB + surface layer (`DbRun.job_run_number` now populated on every inserted row), downstream plans that need to display the number in the UI (Plan 11-12) or in API responses (Plan 11-06/07/08) can read the field directly without computing it on-the-fly.
- **Plan 11-12 (template rendering of `#1..#N`)** has the exact shape it needs: `DbRun.job_run_number: i64` and `DbRunDetail.job_run_number: i64` available in askama templates (via the `AppJob` / `AppRun` ViewModel layer) without further plumbing.
- **Plan 11-13 (startup assertion, D-15)** still valid — `count_job_runs_with_null_run_number` returns 0 both because of the NOT NULL constraint (Plan 11-04) and because `insert_running_run` now supplies a non-NULL value (this plan).
- **Phase 11 Success Criterion #1 (run-history shows `#1..#N` per job)** is on track — the DB + query surface contract is in place; templates will wire up in Plan 11-12.

## Self-Check: PASSED

**Files verified on disk:**
- src/db/queries.rs — FOUND (modified; two-statement tx + struct/SELECT updates)
- tests/v11_runnum_counter.rs — FOUND (modified; four Wave-0 stubs replaced with real bodies)
- .planning/phases/11-per-job-run-numbers-log-ux-fixes/11-05-SUMMARY.md — FOUND (this file)

**Commits verified:**
- 69c9f47 — FOUND (`feat(11-05): refactor insert_running_run to two-statement counter tx (DB-11)`)
- d82880e — FOUND (`feat(11-05): extend DbRun/DbRunDetail with job_run_number + update SELECT lists`)
- 2a114a3 — FOUND (`test(11-05): counter test bodies — T-V11-RUNNUM-02/10/11 GREEN`)

**Build gates verified:**
- `cargo test --test v11_runnum_counter` — PASS (`4 passed; 0 failed; 0 ignored`) — all four counter tests green.
- `cargo test --lib db::queries::tests` — PASS (`21 passed; 0 failed`) — the 9 previously-deferred tests are GREEN.
- `cargo test --test v11_runnum_migration` — PASS (`9 passed; 0 failed`) — no regression from Plan 11-04.
- `cargo test --lib` — PASS (`169 passed; 0 failed`).
- `cargo check --all-targets` — CLEAN.
- `cargo clippy --tests -- -D warnings` — CLEAN.
- `cargo fmt --check` — CLEAN.

**Plan success criteria verified:**
1. `insert_running_run` uses the two-statement counter tx on both backends — ✅ (verified by grep for `UPDATE jobs SET next_run_number = next_run_number + 1` + `RETURNING next_run_number - 1` both present twice, once per arm).
2. `DbRun` + `DbRunDetail` have `pub job_run_number: i64` — ✅ (verified: `grep -c "pub job_run_number: i64" src/db/queries.rs` → `2`).
3. All SELECT lists that produce `DbRun`/`DbRunDetail` include `job_run_number` — ✅ (`get_run_history` on both backends, `get_run_by_id` on both backends; `get_dashboard_jobs` excluded because it uses `DashboardJob`, not `DbRun`).
4. Concurrent 16-way insert produces the set `{1..=16}` — no MAX+1 race possible — ✅ (`concurrent_inserts_distinct_numbers` test asserts exactly this and passes).
5. `next_run_number` invariant holds: always equals `MAX(job_run_number) + 1` — ✅ (`next_run_number_invariant` test asserts `nrn == 17` after 16-way race, which is `MAX=16 + 1`).

---
*Phase: 11-per-job-run-numbers-log-ux-fixes*
*Completed: 2026-04-17*
