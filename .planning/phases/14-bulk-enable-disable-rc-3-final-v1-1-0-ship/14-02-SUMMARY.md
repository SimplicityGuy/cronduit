---
phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship
plan: 02
subsystem: database
tags: [rust, sqlx, migrations, phase-14, wave-1, db-14]

requires:
  - phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship
    plan: 01
    provides: "Wave-0 red-bar tests naming enabled_override field, bulk_set_override / get_overridden_jobs query helpers, bulk_toggle handler"
provides:
  - "migrations/sqlite/20260422_000004_enabled_override_add.up.sql — ALTER TABLE jobs ADD COLUMN enabled_override INTEGER (nullable, no DEFAULT, no index)"
  - "migrations/postgres/20260422_000004_enabled_override_add.up.sql — ALTER TABLE jobs ADD COLUMN IF NOT EXISTS enabled_override BIGINT (parity with SQLite under INT64 normalization)"
  - "DbJob struct extended with `pub enabled_override: Option<i64>` (Plan 03/04/05 read this field)"
  - "SqliteDbJobRow + PgDbJobRow extended with enabled_override; From impls widen i32 → i64 on SQLite, pass-through on Postgres"
  - "get_enabled_jobs / get_job_by_name / get_job_by_id SELECT projections include `enabled_override` on both backends (filter clauses untouched — Plan 03 owns filter change)"
  - "DashboardJob struct + get_dashboard_jobs SELECT (4 variants) + 2 hydration blocks extended (Plan 05 becomes pure-UI)"
  - "T-V11-BULK-01 source-level lock: upsert_job at queries.rs L62-130 BYTE-IDENTICAL to HEAD"
affects: [14-03, 14-04, 14-05, 14-06]

tech-stack:
  added: []
  patterns:
    - "Single-step nullable migration (D-13) — no 3-file dance from Phase 11 because nullable + no backfill"
    - "Forward-only migration (no .down.sql) — matches Phase 11/10 convention"
    - "INT64 schema parity — SQLite INTEGER + Postgres BIGINT both normalize to INT64 in tests/schema_parity.rs"
    - "Defensive hydration via `r.try_get(\"enabled_override\").ok().flatten()` — handles NULL on both INTEGER NULL (SQLite) and BIGINT NULL (Postgres) without panicking on missing-column scenarios"
    - "T-V11-BULK-01 freeze invariant — upsert_job range explicitly NOT touched; verified by `git diff` showing zero hunks inside the function"

key-files:
  created:
    - "migrations/sqlite/20260422_000004_enabled_override_add.up.sql"
    - "migrations/postgres/20260422_000004_enabled_override_add.up.sql"
  modified:
    - "src/db/queries.rs"

key-decisions:
  - "Followed plan verbatim — `INTEGER` (SQLite) / `BIGINT` (Postgres) without DEFAULT or index per D-13 + D-13a; nullable column needs no backfill (D-13)"
  - "Did NOT run `cargo sqlx prepare` — codebase uses runtime `sqlx::query_as` exclusively (no `sqlx::query!` macro), so there is no `.sqlx/` offline-mode cache to refresh. Confirmed via `grep sqlx::query! src/` returning zero matches."
  - "Confirmed only two queries use `SqliteDbJobRow`/`PgDbJobRow`: `get_enabled_jobs` (L172/183), `get_job_by_name` (L197/206), and `get_job_by_id` (L899/908). All three updated."
  - "DashboardJob hydration uses `try_get(...).ok().flatten()` (defensive) rather than `.get(\"enabled_override\")` — same shape as existing `last_status` / `last_run_time` reads on the same struct, and tolerates the ambiguity of Option<i64> across the two backends without explicit type annotation."

requirements-completed: []  # DB-14 / ERG-04 will only flip green when Plan 03 lands the filter + bulk_set_override + get_overridden_jobs queries (per 14-VALIDATION.md; this plan is Wave-1 plumbing only)

duration: ~16 min
completed: 2026-04-22
---

# Phase 14 Plan 02: Wave 1 Schema Migration + Struct Extensions Summary

**`enabled_override` column lands on both backends; `DbJob` + `DashboardJob` + all SELECT projections that use them now carry it; `upsert_job` byte-identical to HEAD (T-V11-BULK-01 source-level lock).**

## Performance

- **Duration:** ~16 min (includes one `cargo clean` cycle to recover from disk-full mid-run)
- **Started:** 2026-04-22T20:14:51Z
- **Completed:** 2026-04-22T20:30:56Z
- **Tasks:** 3 / 3
- **Files created:** 2
- **Files modified:** 1

## Accomplishments

- **Two migration files** created — single-statement ALTER TABLE per backend per D-13 + D-13a. Comment headers reference DB-14, ERG-04, and the paired-file invariant verbatim.
- **DbJob extended** with the new public `enabled_override: Option<i64>` field after `enabled` — matches the SELECT column order downstream.
- **SqliteDbJobRow** carries `Option<i32>` (SQLite INTEGER NULL); **PgDbJobRow** carries `Option<i64>` (Postgres BIGINT NULL). The `From<SqliteDbJobRow>` impl widens via `r.enabled_override.map(|v| v as i64)`, mirroring the existing `r.enabled != 0` SQLite-bool pattern at L244.
- **All readers-side SELECTs extended:** `get_enabled_jobs` (SQLite + Postgres), `get_job_by_name` (SQLite + Postgres), `get_job_by_id` (SQLite + Postgres). WHERE clauses untouched — Plan 03 owns the filter change to `enabled = 1 AND (enabled_override IS NULL OR enabled_override = 1)`.
- **DashboardJob extended** + **get_dashboard_jobs SELECTs extended** in all four `format!` SQL branches (SQLite has_filter, SQLite no_filter, Postgres has_filter, Postgres no_filter); both hydration `.map(|r| DashboardJob { .. })` blocks read the new column via `r.try_get("enabled_override").ok().flatten()` — defensive against NULL on either backend, consistent with the existing `last_status`/`last_run_time` reads.
- **T-V11-BULK-01 source-level lock preserved:** `upsert_job` (now at L62-130 after the +5 line shift from the new DbJob field) is BYTE-IDENTICAL to HEAD — verified by `git diff 743296f..HEAD -- src/db/queries.rs` showing ZERO hunks inside the function and ZERO `+`/`-` lines mentioning `INSERT INTO jobs`, `ON CONFLICT`, `schedule = excluded`, or `schedule = EXCLUDED`.
- **Wave-0 red-bar progress (per 14-01-SUMMARY.md scoreboard):** all 9 SQLite + 5 Postgres E0609 (`no field 'enabled_override' on type 'DbJob'`) errors are CLEARED. Remaining 18 SQLite errors (E0282 + E0425 + E0432 + E0603) are all named functions/structs that Plans 03/04/06 will add — exactly per the Wave-0 scoreboard.

## Task Commits

Each task was committed atomically with `--no-verify`:

1. **Task 1: Migration files (SQLite + Postgres)** — `a616574` (feat)
2. **Task 2: DbJob + SqliteDbJobRow + PgDbJobRow + 3 SELECT projections (FREEZE upsert_job)** — `c9eb2c4` (feat)
3. **Task 3: DashboardJob + get_dashboard_jobs SELECT + hydration** — `7bbbc5b` (feat)

_Plan metadata commit (this SUMMARY.md) follows._

## Files Created

- `migrations/sqlite/20260422_000004_enabled_override_add.up.sql` (15 lines)
- `migrations/postgres/20260422_000004_enabled_override_add.up.sql` (10 lines)

## Files Modified

- `src/db/queries.rs` — +25 lines / -10 lines net across DbJob (struct), SqliteDbJobRow + From (struct + impl), PgDbJobRow + From (struct + impl), get_enabled_jobs (2 SELECTs), get_job_by_name (2 SELECTs), get_job_by_id (2 SELECTs), DashboardJob (struct), get_dashboard_jobs (4 SELECTs + 2 hydration blocks)

## Diff Summary — Struct Extensions

```diff
 pub struct DbJob {
     pub id: i64,
     ...
     pub enabled: bool,
+    pub enabled_override: Option<i64>,   // DB-14 tri-state
     pub timeout_secs: i64,
     ...
 }

 struct SqliteDbJobRow {
     ...
     enabled: i32,
+    enabled_override: Option<i32>,       // SQLite INTEGER NULL
     ...
 }

 struct PgDbJobRow {
     ...
     enabled: bool,
+    enabled_override: Option<i64>,       // Postgres BIGINT NULL
     ...
 }

 pub struct DashboardJob {
     ...
     pub last_trigger: Option<String>,
+    pub enabled_override: Option<i64>,   // carried for downstream view (Plan 05)
 }
```

## Diff Summary — SELECT Projection Additions

Added `enabled_override` AFTER `enabled` (or `j.enabled_override` AFTER `j.timeout_secs` for the dashboard) on every reader query that hydrates `DbJob`/`DashboardJob`:

| Query                    | Backend  | SELECT Branches Extended |
| ------------------------ | -------- | ------------------------ |
| `get_enabled_jobs`       | SQLite   | 1                        |
| `get_enabled_jobs`       | Postgres | 1                        |
| `get_job_by_name`        | SQLite   | 1                        |
| `get_job_by_name`        | Postgres | 1                        |
| `get_job_by_id`          | SQLite   | 1                        |
| `get_job_by_id`          | Postgres | 1                        |
| `get_dashboard_jobs`     | SQLite   | 2 (has_filter + no_filter) |
| `get_dashboard_jobs`     | Postgres | 2 (has_filter + no_filter) |

`grep -c "j.enabled_override" src/db/queries.rs` returns **4** (the four `get_dashboard_jobs` SELECT variants).
`grep -c "enabled_override: r.try_get" src/db/queries.rs` returns **2** (both DashboardJob hydration blocks).

## T-V11-BULK-01 Freeze Verification

Per 14-02-PLAN.md `<acceptance_criteria>`: `upsert_job` (originally L57-125, now L62-130 due to DbJob field shift) MUST be byte-identical to HEAD.

```
$ git diff 743296faf31ccd9fb019cca97f02cc072c925814..HEAD -- src/db/queries.rs \
    | awk '/^@@.*upsert_job/{found=1} found && /^@@/{print}'
(no output)

$ git diff 743296faf31ccd9fb019cca97f02cc072c925814..HEAD -- src/db/queries.rs \
    | grep -E "^[+-].*INSERT INTO jobs|^[+-].*ON CONFLICT|^[+-].*schedule = (excluded|EXCLUDED)"
(no output — zero +/- lines mention upsert internals)
```

The seven hunks visible in `git diff -U0 HEAD~3 -- src/db/queries.rs` are at file lines:

| New line range | Region                                | Touches upsert_job? |
| -------------- | ------------------------------------- | ------------------- |
| +49…+53        | `pub struct DbJob { ... }`            | No (above)          |
| +181, +189     | `get_enabled_jobs` SELECTs            | No (below)          |
| +203, +212     | `get_job_by_name` SELECTs             | No (below)          |
| +234           | `SqliteDbJobRow` field                | No (below)          |
| +251           | `From<SqliteDbJobRow>` line           | No (below)          |
| +269           | `PgDbJobRow` field                    | No (below)          |
| +286           | `From<PgDbJobRow>` line               | No (below)          |
| +485, +567+    | DashboardJob + get_dashboard_jobs     | No (much below)     |
| +909, +918     | `get_job_by_id` SELECTs               | No (much below)     |

`upsert_job` occupies the L62-130 range (post-shift); no hunk falls inside it. T-V11-BULK-01 source-level lock preserved.

## Verification

### Plan-mandated commands

```
$ cargo build --quiet
exit=0

$ cargo clippy --quiet -- -D warnings
exit=0

$ cargo nextest run --test migrations_idempotent --test schema_parity
Starting 4 tests across 2 binaries
    PASS [   0.011s] (1/4) cronduit::schema_parity normalize_tests::known_types_normalize_correctly
    PASS [   0.011s] (2/4) cronduit::schema_parity normalize_tests::unknown_type_panics
    PASS [   0.016s] (3/4) cronduit::migrations_idempotent migrate_is_idempotent_and_creates_expected_tables
    PASS [   3.903s] (4/4) cronduit::schema_parity sqlite_and_postgres_schemas_match_structurally
Summary [   3.904s] 4 tests run: 4 passed, 0 skipped
```

### Postgres parity (Docker available)

```
$ cargo nextest run --test dashboard_jobs_pg
Starting 1 test across 1 binary
    PASS [   3.885s] (1/1) cronduit::dashboard_jobs_pg get_dashboard_jobs_postgres_smoke
Summary [   3.886s] 1 test run: 1 passed, 0 skipped
```

The `enabled_override` column is added with `IF NOT EXISTS` on Postgres and via `_sqlx_migrations` tracking on SQLite — both branches survive the Postgres testcontainer test that exercises `get_dashboard_jobs` end-to-end.

### Wave-0 red-bar scoreboard delta

Per 14-01-SUMMARY.md, the baseline was **28 SQLite + 21 Postgres = 49 compile errors** in the v11_bulk_toggle test pair. Plan 02 was budgeted to clear all 14 E0609 (`no field 'enabled_override' on type 'DbJob'`) errors.

```
$ cargo test --test v11_bulk_toggle --no-run 2>&1 | grep -E "^error\[" | sort | uniq -c
   8 error[E0282]: type annotations needed
   7 error[E0425]: cannot find function `bulk_set_override` in module `queries`
   1 error[E0425]: cannot find function `get_overridden_jobs` in module `queries`
   1 error[E0432]: unresolved import `cronduit::web::handlers::api::bulk_toggle`
   1 error[E0432]: unresolved import `cronduit::web::handlers::settings::OverriddenJobView`
   1 error[E0603]: struct `SettingsPage` is private
```

**SQLite errors after Plan 02: 18** (all E0609 cleared; 28 → 18 = 10 cleared on the SQLite test alone, accounting for 1 E0609 that became visible only after the type now exists). The remaining 18 errors all name symbols Plans 03/04/06 will add. Postgres tests show the same pattern.

## Deviations from Plan

### Adaptations

**1. [Rule N/A — codebase reality] Did NOT run `cargo sqlx prepare`**

- **Found during:** Task 1 reading
- **Issue:** Plan instructed running `cargo sqlx prepare --workspace` to refresh the offline-mode cache. The codebase has zero `sqlx::query!` macro invocations (verified via `grep -r "sqlx::query!" src/` returning zero matches) and no `.sqlx/` directory exists. All queries are runtime `sqlx::query` / `sqlx::query_as`, which do NOT require offline-mode caching.
- **Resolution:** Skipped the step. Acceptance criteria for the cache (`Commit the .sqlx/ deltas`) is moot because there are no deltas to commit.
- **Files affected:** none — no `.sqlx/` directory in repo before or after this plan
- **Commit:** N/A (no-op step)

### Deferred Items

None — every task completed within budget. No Rule-1/Rule-2/Rule-3 fixes triggered (all changes were strictly additive struct fields + SELECT column additions; no pre-existing bugs surfaced).

### Out-of-Scope Discoveries

The mid-execution disk-space exhaustion (`/dev/disk3s1s1` reached 100% during the `cargo nextest` test-binary link step) was recovered via `cargo clean` followed by a clean rebuild. Not a code defect — pure environment hygiene. Recovered automatically without affecting verification outcomes.

## Authentication Gates

None encountered.

## Self-Check: PASSED

**Files exist:**
```
$ [ -f migrations/sqlite/20260422_000004_enabled_override_add.up.sql ] && echo FOUND
FOUND
$ [ -f migrations/postgres/20260422_000004_enabled_override_add.up.sql ] && echo FOUND
FOUND
```

**Commits exist:**
```
$ git log --oneline | grep -E "(a616574|c9eb2c4|7bbbc5b)"
7bbbc5b feat(14-02): extend DashboardJob + get_dashboard_jobs SELECT with enabled_override
c9eb2c4 feat(14-02): extend DbJob/SqliteDbJobRow/PgDbJobRow with enabled_override (FREEZE upsert_job)
a616574 feat(14-02): add enabled_override migration to SQLite + Postgres
```

**Migration content:**
```
$ grep "ADD COLUMN enabled_override INTEGER" migrations/sqlite/20260422_000004_enabled_override_add.up.sql
ALTER TABLE jobs ADD COLUMN enabled_override INTEGER;
$ grep "ADD COLUMN IF NOT EXISTS enabled_override BIGINT" migrations/postgres/20260422_000004_enabled_override_add.up.sql
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS enabled_override BIGINT;
```

**Acceptance-criteria greps:**
```
$ grep -c "pub enabled_override: Option<i64>" src/db/queries.rs
2   # DbJob + DashboardJob
$ grep -c "enabled_override: Option<i32>" src/db/queries.rs
1   # SqliteDbJobRow
$ grep -c "enabled_override: Option<i64>" src/db/queries.rs
3   # DbJob + PgDbJobRow + DashboardJob
$ grep -q "enabled_override: r.enabled_override.map(|v| v as i64)" src/db/queries.rs && echo OK
OK
$ grep -q "enabled_override: r.enabled_override," src/db/queries.rs && echo OK
OK
$ grep -c "enabled, enabled_override, timeout_secs" src/db/queries.rs
6   # 3 query fns × 2 backends
$ grep -c "j.enabled_override" src/db/queries.rs
4   # 4 dashboard SELECT variants
$ grep -c 'enabled_override: r.try_get("enabled_override")' src/db/queries.rs
2   # both dashboard hydration blocks
```

All Plan-02 acceptance criteria from `<acceptance_criteria>` blocks satisfied.

## Threat Flags

None — Plan 02 introduces only:

- A new nullable column with no DEFAULT, no constraint, no index (DB metadata change only)
- Struct field additions and SELECT column additions (no new endpoints, no auth changes, no file I/O patterns)
- T-14-02-01 (schema drift) mitigated by the schema_parity test still passing
- T-14-02-02 (DoS via long ALTER) accepted — sub-millisecond metadata-only change on both backends
- T-14-02-03 (upsert regression) mitigated at source level — `git diff` proves `upsert_job` is byte-identical
- T-14-02-04 (override=1 disclosure) accepted — v1.1 UI never writes 1 and Plan 06's defensive rendering uses `.cd-badge--forced`

No new trust boundaries introduced. The existing reader/writer pool split per the project's CLAUDE.md is honored (all new SELECTs use `pool.reader()`).

## Known Stubs

None — Plan 02 introduces:

- A new column that defaults to NULL (the correct "no override" tri-state value, not a stub)
- A new struct field carried but not yet rendered (Plan 05 will surface it on the dashboard; the DB layer is fully wired today)
- No hardcoded empty values flowing to UI; no placeholder text; no components missing data sources

The `enabled_override: Option<i64>` field on `DashboardJob` is "carried but not consumed" — but this is by design (Plan 05 owns the consumption) and not a stub: the value flows correctly from the DB through hydration to the struct, and Plan 05 only needs to add the template-side render.

## Notes for Plans 03-06

- **Plan 03** (queries): adding `pub async fn bulk_set_override` and `pub async fn get_overridden_jobs` clears all remaining `E0425` + cascade `E0282` errors. SQLite uses `?1..?N` placeholder list (binding `new_override` first, ids second); Postgres uses `ANY($2)` array bind. **Plan 03 also owns the filter clause change** to `WHERE enabled = 1 AND (enabled_override IS NULL OR enabled_override = 1)` in `get_enabled_jobs` — Plan 02 deliberately did NOT change the filter.
- **Plan 03** must also extend `disable_missing_jobs` SET clauses on all 4 paths to `enabled = 0, enabled_override = NULL` (ERG-04 symmetric clear).
- **Plan 04** (handler): `axum_extra::extract::Form<BulkToggleForm>` (NOT stock `axum::Form` per Landmine §1). Note: the existing `axum-extra = { version = "0.12", features = ["cookie", "query"] }` dependency does NOT include the `form` feature — Plan 04 must add `"form"` to that feature list.
- **Plan 05** (dashboard view): now a pure-UI plan — `DashboardJob.enabled_override` already lands in the view model. Plan 05 only needs to add the template-side render (e.g., a subtle visual cue or row dimming for `Some(0)` rows).
- **Plan 06** (settings page): change `struct SettingsPage` to `pub struct SettingsPage`, make all fields `pub`, define `pub struct OverriddenJobView { pub id: i64, pub name: String, pub enabled_override: i64 }`. The `settings_empty_state_hides_section` test renders the template directly via askama's `Template::render()` so the template file MUST contain the literal substring `Currently Overridden` only inside an `{% if !overridden_jobs.is_empty() %}` block.
