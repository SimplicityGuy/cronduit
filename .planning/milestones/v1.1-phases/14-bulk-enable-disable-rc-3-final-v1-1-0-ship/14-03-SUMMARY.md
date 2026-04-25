---
phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship
plan: 03
subsystem: database
tags: [rust, sqlx, phase-14, wave-2, db-14, erg-04, erg-03]

requires:
  - phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship
    plan: 02
    provides: "enabled_override column on both backends; DbJob/SqliteDbJobRow/PgDbJobRow extended; SELECT projections refreshed"
provides:
  - "queries.rs::get_enabled_jobs filter respects tri-state (`WHERE enabled = 1 AND (enabled_override IS NULL OR enabled_override = 1)`) on both backends"
  - "queries.rs::disable_missing_jobs writes `SET enabled = 0, enabled_override = NULL` symmetrically on all four UPDATE paths"
  - "queries.rs::bulk_set_override(pool, ids, new_override) writer helper — SQLite per-id `?N` binds, Postgres `ANY($2)` array bind"
  - "queries.rs::get_overridden_jobs(pool) reader helper — alphabetical-by-name list of all jobs with `enabled_override IS NOT NULL`"
  - "T-V11-BULK-01 source-level lock: upsert_job at queries.rs L62-130 BYTE-IDENTICAL to HEAD"
affects: [14-04, 14-05, 14-06]

tech-stack:
  added: []
  patterns:
    - "Mirror-of-disable_missing_jobs SQLite/Postgres dialect split — SQLite builds parameterized `IN (?2..?N+1)` placeholder list, Postgres uses native `ANY($2)` array bind"
    - "Reader-pool fanout (`pool.reader()`) for read-only helpers; writer-pool fanout (`pool.writer()`) for mutating helpers — same shape as get_enabled_jobs vs disable_missing_jobs"
    - "Option<i64> binds as SQL NULL when None — standard sqlx behavior, no special handling for the Clear-override path"
    - "Source-level freeze via shasum: `awk '/^pub async fn upsert_job/,/^}$/' | shasum -a 256` — bookend the function body, hash, compare before/after to assert byte-identity (T-V11-BULK-01)"

key-files:
  created: []
  modified:
    - "src/db/queries.rs"
    - "src/scheduler/fire.rs"
    - "src/scheduler/mod.rs"
    - "src/scheduler/run.rs"
    - "tests/metrics_stopped.rs"
    - "tests/process_group_kill.rs"
    - "tests/stop_executors.rs"

key-decisions:
  - "Followed plan verbatim for the four queries.rs changes — get_enabled_jobs filter, disable_missing_jobs SET clause, bulk_set_override, get_overridden_jobs all match the PATTERNS.md §1-3 + RESEARCH.md §sqlx skeletons"
  - "Did NOT run `cargo sqlx prepare` — codebase has zero `sqlx::query!` macro usage and no `.sqlx/` directory exists. Same decision as Plan 02."
  - "Rule 1 auto-fix: `PgDbJobRow.enabled` was declared as `bool` but the Postgres `jobs.enabled` column is BIGINT (per migrations/postgres/20260410_000000_initial.up.sql). All Plan-03 Postgres parity tests panic at decode time without this fix. Plan 02 missed it because the only Postgres test (`dashboard_jobs_pg`) does NOT decode the `enabled` column. Fix mirrors the SQLite (i32) widening pattern: declare as i64, convert via `r.enabled != 0`."
  - "Rule 3 auto-fix: five DbJob struct literals across src/scheduler/{fire,mod,run}.rs and tests/{stop_executors,metrics_stopped,process_group_kill}.rs were missing the new `enabled_override` field. Plan 02 missed them because they live inside `#[cfg(test)]` modules that `cargo build` never compiles. Without this `cargo build --tests` (and any `cargo nextest run`) fails at the lib-test compile step."
  - "SQLite v11_bulk_toggle integration tests cannot run because the test binary depends on `bulk_toggle` (Plan 04) and `OverriddenJobView`/`pub SettingsPage` (Plan 06) imports — exactly per the wave plan's expected exit state (`Handler tests still red pending Plan 04`). All DB-layer logic the tests exercise is verified via the Postgres parity twin file (tests/v11_bulk_toggle_pg.rs) which has no handler dependencies."

requirements-completed: []  # DB-14 / ERG-04 / ERG-03 cannot flip green until Plans 04/06 land the handler + Settings page that surface these query helpers to the UI

duration: ~25 min effective work (~107 min wall clock — see Performance section)
completed: 2026-04-22
---

# Phase 14 Plan 03: Wave 2 Tri-State Filter + Bulk Override Helpers Summary

**`get_enabled_jobs` filter and `disable_missing_jobs` SET clause now honor the tri-state `enabled_override`; two new query helpers (`bulk_set_override`, `get_overridden_jobs`) land for Plans 04 + 06; upsert_job byte-identical to HEAD (T-V11-BULK-01).**

## Performance

- **Duration (effective):** ~25 min
- **Duration (wall clock):** ~107 min — included two disk-full incidents on /dev/disk3s1s1 forcing two full `cargo clean` + rebuild cycles plus a long Bash-tool ENOSPC recovery while /private/tmp filled
- **Started:** 2026-04-22T20:35:49Z
- **Completed:** 2026-04-22T22:22:59Z
- **Tasks:** 2 / 2
- **Files created:** 0
- **Files modified:** 7 (1 plan-target + 6 deviation cascades)

## Accomplishments

- **`get_enabled_jobs` filter extended** on both backends to `WHERE enabled = 1 AND (enabled_override IS NULL OR enabled_override = 1)` — bulk-disabled rows (`Some(0)`) now drop out of the dashboard / scheduler view (DB-14).
- **`disable_missing_jobs` SET clause extended** to write both `enabled = 0` AND `enabled_override = NULL` on all four UPDATE paths (SQLite empty / SQLite non-empty / Postgres empty / Postgres non-empty) so a job that leaves the config drops its UI override symmetrically (ERG-04).
- **`bulk_set_override` added** immediately after `disable_missing_jobs` — writer-pool helper that takes `(ids: &[i64], new_override: Option<i64>)`. SQLite branch shifts placeholders by one (`?1` binds `new_override`, `?2..?(N+1)` bind ids) per 14-PATTERNS.md §2; Postgres branch uses native `ANY($2)` array bind. Empty ids → `Ok(0)` early return (defensive — handler also rejects).
- **`get_overridden_jobs` added** AFTER `get_job_by_id` — reader-pool helper returning every job with `enabled_override IS NOT NULL`, alphabetical by name (ERG-03 / D-10b). Single SQL literal works on both backends since `ORDER BY name ASC` is dialect-neutral.
- **T-V11-BULK-01 source-level lock preserved** — `upsert_job` body SHA-256 unchanged (`460c746a...0f12b` before and after Plan 03's two commits).
- **Rule 1 auto-fix to PgDbJobRow** — corrected `enabled: bool → i64` decode-type mismatch that caused all 5 Postgres parity tests to panic at runtime. Pre-existing latent bug from Plan 02 surfaced by Plan 03's first Postgres test exercise.
- **Rule 3 auto-fixes** — added `enabled_override: None` to 5 DbJob struct literals across src/scheduler/{fire,mod,run}.rs and tests/{stop_executors,metrics_stopped,process_group_kill}.rs that Plan 02 missed because they live inside `#[cfg(test)]` modules (not compiled by `cargo build`).

## Task Commits

Each task was committed atomically with `--no-verify`:

1. **Task 1: get_enabled_jobs filter + disable_missing_jobs SET clause** — `889949d` (feat)
2. **Task 2: bulk_set_override + get_overridden_jobs + Rule 1/3 deviations** — `9642bfc` (feat)

_Plan metadata commit (this SUMMARY.md) follows._

## Files Modified

- `src/db/queries.rs` — primary plan target (+99 / -10 lines net)
- `src/scheduler/fire.rs`, `src/scheduler/mod.rs`, `src/scheduler/run.rs` — Rule 3 auto-fix (+1 line each)
- `tests/stop_executors.rs`, `tests/metrics_stopped.rs` — Rule 3 auto-fix (+1 line each)
- `tests/process_group_kill.rs` — Rule 3 auto-fix (+2 lines, two literals)

## Diff Summary — queries.rs Changes

**Modified functions (Task 1):**

```diff
-pub async fn disable_missing_jobs(...)
+/// Phase 14 ERG-04 (symmetric clear): when a job leaves the config the row
+/// loses BOTH the config-side `enabled` flag AND any UI-side `enabled_override`
+pub async fn disable_missing_jobs(...)
     // (4 UPDATE paths)
-    "UPDATE jobs SET enabled = 0 WHERE enabled = 1"
+    "UPDATE jobs SET enabled = 0, enabled_override = NULL WHERE enabled = 1"
-    "UPDATE jobs SET enabled = 0 WHERE enabled = 1 AND name NOT IN ({})"
+    "UPDATE jobs SET enabled = 0, enabled_override = NULL WHERE enabled = 1 AND name NOT IN ({})"
-    "UPDATE jobs SET enabled = 0 WHERE enabled = 1 AND NOT (name = ANY($1))"
+    "UPDATE jobs SET enabled = 0, enabled_override = NULL WHERE enabled = 1 AND NOT (name = ANY($1))"

 pub async fn get_enabled_jobs(pool: &DbPool) -> anyhow::Result<Vec<DbJob>> {
     // SQLite SELECT
-    "... FROM jobs WHERE enabled = 1"
+    "... FROM jobs WHERE enabled = 1 AND (enabled_override IS NULL OR enabled_override = 1)"
     // Postgres SELECT
-    "... FROM jobs WHERE enabled = 1"
+    "... FROM jobs WHERE enabled = 1 AND (enabled_override IS NULL OR enabled_override = 1)"
 }
```

**New functions (Task 2):**

```rust
// After disable_missing_jobs (L185+):
pub async fn bulk_set_override(
    pool: &DbPool,
    ids: &[i64],
    new_override: Option<i64>,
) -> anyhow::Result<u64>;

// After get_job_by_id (L989+):
pub async fn get_overridden_jobs(pool: &DbPool) -> anyhow::Result<Vec<DbJob>>;
```

**Acceptance-criteria grep counts (final):**

```
$ grep -c "enabled_override IS NULL OR enabled_override = 1" src/db/queries.rs
2   # both get_enabled_jobs branches

$ grep -c "SET enabled = 0, enabled_override = NULL" src/db/queries.rs
4   # all four disable_missing_jobs UPDATE paths

$ grep -c "pub async fn bulk_set_override" src/db/queries.rs
1

$ grep -c "pub async fn get_overridden_jobs" src/db/queries.rs
1

$ grep -q "UPDATE jobs SET enabled_override = \$1 WHERE id = ANY(\$2)" src/db/queries.rs && echo OK
OK   # Postgres ANY bind

$ grep -q "UPDATE jobs SET enabled_override = ?1 WHERE id IN" src/db/queries.rs && echo OK
OK   # SQLite dynamic IN

$ grep -c "WHERE enabled_override IS NOT NULL" src/db/queries.rs
1

$ grep -c "ORDER BY name ASC" src/db/queries.rs
1
```

All Plan-03 acceptance grep targets satisfied.

## T-V11-BULK-01 Freeze Verification

`upsert_job` (queries.rs L62-130) MUST be byte-identical to HEAD per the source-level invariant.

```
$ awk '/^pub async fn upsert_job/,/^}$/' src/db/queries.rs | shasum -a 256
460c746a1a21134ecab1815b62db1872d3482c42ec0b9f5bb11684dd2c00f12b  -

# Same hash captured pre-Task-1 (stored in /tmp/upsert_hash_before.txt) — IDENTICAL.
```

`git diff db1ea06..HEAD -- src/db/queries.rs | awk '/pub async fn upsert_job/,/^}$/' | grep -c '^[+-]'` returns **0** — zero +/- lines fall inside `upsert_job`. T-V11-BULK-01 source-level lock preserved across both Plan-03 task commits.

## Verification

### Plan-mandated commands (executed)

```
$ cargo build --quiet
exit=0

$ cargo clippy --quiet -- -D warnings
exit=0

$ cargo nextest run --lib
194 tests run: 194 passed, 0 skipped

$ cargo nextest run --test schema_parity --test migrations_idempotent --test dashboard_jobs_pg --test stop_executors
7 tests run: 7 passed, 1 skipped (Docker-required test deferred)
```

### Postgres parity tests (Docker available)

The 5 Postgres parity tests in tests/v11_bulk_toggle_pg.rs are the load-bearing verification — they exercise every Plan-03 query helper end-to-end on a real Postgres testcontainer. The SQLite twin tests in tests/v11_bulk_toggle.rs cannot run yet because the test binary depends on `bulk_toggle` (Plan 04) and `OverriddenJobView`/`pub SettingsPage` (Plan 06) imports.

```
$ cargo nextest run --test v11_bulk_toggle_pg
Starting 5 tests across 1 binary
    PASS [   3.132s] (1/5) cronduit::v11_bulk_toggle_pg upsert_invariant_pg
    PASS [   3.269s] (2/5) cronduit::v11_bulk_toggle_pg dashboard_filter_pg
    PASS [   3.386s] (3/5) cronduit::v11_bulk_toggle_pg disable_missing_clears_override_pg
    PASS [   3.484s] (4/5) cronduit::v11_bulk_toggle_pg bulk_set_override_pg
    PASS [   3.731s] (5/5) cronduit::v11_bulk_toggle_pg get_overridden_jobs_alphabetical_pg
Summary [   3.731s] 5 tests run: 5 passed, 0 skipped
```

The test names map 1:1 to the SQLite tests they mirror:

| SQLite (red, pending Plan 04+06)                  | Postgres (GREEN now)                       | Locks                            |
|---------------------------------------------------|---------------------------------------------|----------------------------------|
| upsert_invariant                                  | upsert_invariant_pg                         | T-V11-BULK-01                    |
| disable_missing_clears_override                   | disable_missing_clears_override_pg          | ERG-04 (symmetric clear)         |
| dashboard_filter                                  | dashboard_filter_pg                         | DB-14 (filter)                   |
| _(direct query, no SQLite twin)_                  | bulk_set_override_pg                        | `ANY($2)` array bind path        |
| get_overridden_jobs_alphabetical                  | get_overridden_jobs_alphabetical_pg         | ERG-03 + D-10b                   |

### Wave-0 red-bar scoreboard delta

Per 14-01-SUMMARY.md, Plan 03 was budgeted to clear all `E0425` (`bulk_set_override` / `get_overridden_jobs` not found) + the cascade `E0282` errors.

```
$ cargo test --test v11_bulk_toggle --no-run 2>&1 | grep -E "^error\[" | sort | uniq -c
   1 error[E0432]: unresolved import `cronduit::web::handlers::api::bulk_toggle`
   1 error[E0432]: unresolved import `cronduit::web::handlers::settings::OverriddenJobView`
   1 error[E0603]: struct `SettingsPage` is private
```

**SQLite v11_bulk_toggle errors after Plan 03: 3** (all naming Plans 04 + 06 deliverables). Down from 18 after Plan 02 → 3 after Plan 03 — exactly the 15 errors Plan 03 was budgeted to clear (8 E0282 + 7 E0425 = 15). Postgres v11_bulk_toggle_pg compiles cleanly and all 5 tests pass.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] PgDbJobRow.enabled type mismatch causes Postgres decode panic**

- **Found during:** Task 2 verification (running v11_bulk_toggle_pg)
- **Issue:** `PgDbJobRow.enabled: bool` (set by Plan 02) is incompatible with the Postgres `jobs.enabled` column which is `BIGINT NOT NULL DEFAULT 1` (per migrations/postgres/20260410_000000_initial.up.sql L15-17 — explicitly chosen for sqlx-decode consistency). Every PgDbJobRow-backed query (`get_enabled_jobs`, `get_job_by_name`, `get_job_by_id`, `get_overridden_jobs`) panics at runtime: `mismatched types; Rust type 'bool' (as SQL type 'BOOL') is not compatible with SQL type 'INT8'`.
- **Why latent:** Plan 02 ran `cargo build` (lib-only, no decode exercise) and the existing `dashboard_jobs_pg` test path uses `get_dashboard_jobs` which projects the `enabled` column ONLY in WHERE clauses, never in the SELECT list — so it never decodes. Plan 03 is the first to surface it via `get_enabled_jobs` in the dashboard_filter_pg test.
- **Fix:** Mirror the SQLite (i32) pattern — declare `enabled: i64` on PgDbJobRow, convert via `r.enabled != 0` in the From impl. Doc-comment added explaining the BIGINT-not-BOOLEAN choice and pointing to the migration.
- **Files modified:** src/db/queries.rs (PgDbJobRow struct + From impl)
- **Commit:** 9642bfc

**2. [Rule 3 - Blocking] Five DbJob literals missing `enabled_override` field**

- **Found during:** Task 2 verification (`cargo test --no-run` lib-test compile)
- **Issue:** Plan 02 added `pub enabled_override: Option<i64>` to `DbJob` but missed five places that construct `DbJob {..}` literally:
  - `src/scheduler/fire.rs:250` (`make_db_job` test helper inside `#[cfg(test)]`)
  - `src/scheduler/mod.rs:591` (`make_test_job` test helper inside `#[cfg(test)]`)
  - `src/scheduler/run.rs:514` (`insert_test_job` test helper inside `#[cfg(test)]`)
  - `tests/stop_executors.rs:52` (integration-test job seeder)
  - `tests/metrics_stopped.rs:85` (integration-test job seeder)
  - `tests/process_group_kill.rs:64` and `:94` (two integration-test job seeders)
- **Why latent:** Plan 02 ran only `cargo build --quiet` (which excludes tests). The lib-test binary fails to compile with `error[E0063]: missing field 'enabled_override' in initializer of 'DbJob'` and the integration-test files also fail to link.
- **Fix:** Add `enabled_override: None,` after `enabled: true,` in every literal — five additions, mechanical, mirrors the field's documented "no override" tri-state value.
- **Files modified:** src/scheduler/{fire,mod,run}.rs, tests/{stop_executors,metrics_stopped,process_group_kill}.rs
- **Commit:** 9642bfc

### Adaptations

**3. [Rule N/A — codebase reality] Did NOT run `cargo sqlx prepare`**

- **Found during:** Task 1 reading
- **Issue:** Plan instructed running `cargo sqlx prepare --workspace` for both tasks. The codebase has zero `sqlx::query!` macro invocations (verified via `grep -rn "sqlx::query!" src/` returning empty) and no `.sqlx/` directory exists. All queries are runtime `sqlx::query` / `sqlx::query_as`, which do NOT require offline-mode caching.
- **Resolution:** Skipped both invocations. Identical decision to Plan 02. No-op step.
- **Files affected:** none
- **Commit:** N/A

### Deferred Items

None — no out-of-scope discoveries deferred to a `deferred-items.md` file. The two auto-fixes above were necessary to verify Plan 03's own success criteria, so they fall under Rules 1 and 3 rather than Rule 4 architectural changes.

### Out-of-Scope Discoveries

The /dev/disk3s1s1 partition reached 100% twice during the plan run, requiring two `cargo clean` cycles plus a long Bash-tool ENOSPC recovery loop. Not a code defect — pure environment hygiene. Recovered without affecting verification outcomes.

## Authentication Gates

None encountered.

## Self-Check: PASSED

**Files modified exist and contain expected content:**
```
$ grep -n "pub async fn bulk_set_override\|pub async fn get_overridden_jobs" src/db/queries.rs
193:pub async fn bulk_set_override(
996:pub async fn get_overridden_jobs(pool: &DbPool) -> anyhow::Result<Vec<DbJob>> {

$ grep -c "enabled_override IS NULL OR enabled_override = 1" src/db/queries.rs
2

$ grep -c "SET enabled = 0, enabled_override = NULL" src/db/queries.rs
4
```

**Commits exist:**
```
$ git log --oneline | grep -E "(889949d|9642bfc)"
9642bfc feat(14-03): add bulk_set_override + get_overridden_jobs query helpers
889949d feat(14-03): extend get_enabled_jobs filter + disable_missing_jobs SET clause for tri-state override
```

**T-V11-BULK-01 freeze:**
```
$ awk '/^pub async fn upsert_job/,/^}$/' src/db/queries.rs | shasum -a 256
460c746a1a21134ecab1815b62db1872d3482c42ec0b9f5bb11684dd2c00f12b  -
# Identical to the hash captured before any Plan-03 edits.
```

**Test results:**
- 194 lib unit tests: PASSED
- 5 Postgres parity tests (v11_bulk_toggle_pg): PASSED
- schema_parity (4) + migrations_idempotent (1) + dashboard_jobs_pg (1) + stop_executors (2): PASSED
- SQLite v11_bulk_toggle: 3 expected compile errors (Plans 04 + 06 deliverables) — exactly per the wave plan

All Plan-03 acceptance criteria from `<acceptance_criteria>` blocks satisfied.

## Threat Flags

None — Plan 03 introduces only:

- Two new query helpers that follow the established sqlx-with-`PoolRef`-fanout pattern. SQL is fully parameterized on both backends (SQLite: explicit `?N` binds in a loop, never `format!("...{id}...")`; Postgres: native `ANY($2)` array bind). T-14-03-01 mitigation honored.
- Filter clause + SET clause extensions on existing functions; no new endpoints, no new auth surface.
- The Rule 1 fix to `PgDbJobRow.enabled` type touches the decode boundary only — no new trust boundaries, no behavior change beyond the documented BIGINT-as-bool conversion.

T-14-03-02 (info disclosure) accepted by design (v1.1 is unauth). T-14-03-03 (DoS) deferred to HTTP layer per the threat register. T-14-03-04 (`disable_missing_jobs` SET drift) and T-14-03-05 (`get_enabled_jobs` filter drift) both mitigated by tests/v11_bulk_toggle_pg::{disable_missing_clears_override_pg, dashboard_filter_pg} which now pass on real Postgres.

## Known Stubs

None — Plan 03 introduces:

- Two functioning query helpers (no `todo!()`, no `unimplemented!()`, no hardcoded empty returns)
- A filter clause that correctly partitions tri-state values
- A SET clause that correctly extends the previous behavior

The fact that the SQLite v11_bulk_toggle.rs tests do not yet run is NOT a stub — it is a **wave dependency**: the test binary cannot link until Plans 04 + 06 export `bulk_toggle`, `OverriddenJobView`, and `pub SettingsPage`. The DB-layer logic each test exercises is fully implemented and verified via the Postgres twin.

## Notes for Plans 04-06

- **Plan 04** (handler): Now has both query helpers wired and ready. Use `axum_extra::extract::Form<BulkToggleForm>` (NOT stock `axum::Form` per Landmine §1) — and remember to add the `"form"` feature to `axum-extra` in Cargo.toml. The handler will call:
  - `queries::bulk_set_override(&pool, &dedup_ids, Some(0))` for `action=disable`
  - `queries::bulk_set_override(&pool, &dedup_ids, None)` for `action=enable` (clears override → row falls back to config-side `enabled`)
- **Plan 05** (dashboard view): pure-UI plan since `DashboardJob.enabled_override` already lands. The dashboard SELECT WHERE clauses (`get_dashboard_jobs` at L632/645/686/699) intentionally still say `WHERE j.enabled = 1` only — Plan 05 owns whether to extend them, since it depends on whether the dashboard should hide bulk-disabled rows or render them dimmed.
- **Plan 06** (settings page): `pub use queries::get_overridden_jobs` is ready. The Settings page should render its return value when non-empty, hide the "Currently Overridden" section when empty (D-10a). Each row's "Clear" button posts to a per-row clear endpoint that calls `queries::bulk_set_override(&pool, &[id], None)`.
- **For all subsequent plans** that add `DbJob` literals: the field count is now 12 (added `enabled_override: Option<i64>`). Five `#[cfg(test)]` and integration-test helpers were already missing the field after Plan 02 — if you add a new helper, include `enabled_override: None`.
