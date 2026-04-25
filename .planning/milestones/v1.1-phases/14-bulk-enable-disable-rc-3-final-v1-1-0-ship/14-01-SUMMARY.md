---
phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship
plan: 01
subsystem: testing
tags: [rust, axum, sqlx, testcontainers, tdd, phase-14, wave-0, red-bar]

requires:
  - phase: 13-observability-polish-rc-2
    provides: "Wave-0-first TDD pattern (timeline tests landed before timeline impl); test/handler harness templates (stop_handler.rs)"
provides:
  - "tests/v11_bulk_toggle.rs — 15 SQLite #[tokio::test] cases enumerating every DB-14 + ERG-01..04 + T-V11-BULK-01 invariant"
  - "tests/v11_bulk_toggle_pg.rs — 5 Postgres parity #[tokio::test] cases mirroring the DB-layer invariants on testcontainers-postgres"
  - "Red-bar scoreboard: 28 compile errors (SQLite) + 21 compile errors (Postgres) naming exactly the symbols Plans 02/03/04/06 must add"
affects: [14-02, 14-03, 14-04, 14-06]

tech-stack:
  added: []
  patterns:
    - "Red-bar Wave 0 — tests reference symbols not yet in src/ so compile errors drive the implementation task list"
    - "Mock scheduler harness with Reload-arrival Instant tracking (extension of stop_handler.rs's Stop-mocking pattern)"
    - "Body-encoding pattern: literal `&` separators + repeated `job_ids=` keys to exercise the axum_extra::Form / serde_html_form path (Landmine §1 regression guard)"

key-files:
  created:
    - "tests/v11_bulk_toggle.rs"
    - "tests/v11_bulk_toggle_pg.rs"
  modified: []

key-decisions:
  - "Postgres parity tests live in a separate file (tests/v11_bulk_toggle_pg.rs) rather than gated within v11_bulk_toggle.rs — matches the existing tests/dashboard_jobs_pg.rs precedent and keeps SQLite-only test runs fast"
  - "No #[cfg(feature = \"pg-integration\")] gate — testcontainers' own Docker-API error surface is the skip mechanism on hosts without Docker (consistent with dashboard_jobs_pg.rs / v13_timeline_explain.rs)"
  - "reload_invariant test uses parse_and_validate + tempfile (the established pattern from tests/reload_inflight.rs L41-50) rather than constructing Config directly — Config / JobConfig do not implement Default, and the parse path is the production code path"
  - "handler_partial_invalid_toast_uses_rows_affected exact-string assertion locks UI-SPEC primary-count semantics: `\"2 jobs disabled. (1 not found)\"` for selection=[1,2,9999] — primary count = rows_affected, NOT selection_size"
  - "Mock scheduler records Reload arrival Instants into Arc<tokio::sync::Mutex<Vec<Instant>>>; handler_fires_reload_after_update asserts the recorded instant precedes the post-handler DB-snapshot instant — Landmine §6 ordering guard"

patterns-established:
  - "Pattern (Wave 0 red-bar): the test file imports symbols that don't yet exist; the resulting compile error list IS the task list for downstream waves. Plans 02-06 each clear specific error codes (E0432 → handler/struct exports; E0425 → query helper functions; E0609 → struct field; E0603 → make SettingsPage pub)"
  - "Pattern (Postgres parity twin file): for backend-touching code, write the SQLite + Postgres tests as twin files in the same wave; exact-name suffix `_pg` makes the mirror obvious in cargo test output"

requirements-completed: []  # T-V11-BULK-01, DB-14, ERG-01..04 are NOT yet completed — this plan only adds the failing tests that will lock those requirements once Plans 02-06 turn the bar green

duration: ~17 min
completed: 2026-04-22
---

# Phase 14 Plan 01: Wave 0 Red-Bar Tests for Bulk Toggle Summary

**20 failing #[tokio::test] cases across two files now name every symbol Waves 1-4 must implement; the 49 compile errors are the scoreboard.**

## Performance

- **Duration:** ~17 min (includes ~2 min cargo test --no-run cycle on a cold build)
- **Started:** 2026-04-22T19:52:00Z
- **Completed:** 2026-04-22T20:09:12Z
- **Tasks:** 2 / 2
- **Files created:** 2

## Accomplishments

- **15 SQLite test cases** (`tests/v11_bulk_toggle.rs`, 612 lines) cover every DB invariant, every handler branch, and the Settings-page empty-state render — names match 14-VALIDATION.md's Per-Task Verification Map verbatim.
- **5 Postgres parity test cases** (`tests/v11_bulk_toggle_pg.rs`, 248 lines) cover the five DB-layer behaviors that must produce identical results on Postgres BIGINT columns.
- **Red-bar contract is wired:** `cargo test --test v11_bulk_toggle --test v11_bulk_toggle_pg --no-run` exits non-zero with 49 compile errors that name exactly the missing symbols (`bulk_toggle`, `OverriddenJobView`, `bulk_set_override`, `get_overridden_jobs`, `enabled_override` field, `SettingsPage` privacy). Each error becomes a green check when the corresponding plan lands.
- **No `#[ignore]` anywhere** — the failures are load-bearing; ignoring them would defeat the Nyquist feedback loop the Phase 14 planning frontmatter (`nyquist_compliant: true`) targets.

## Task Commits

Each task was committed atomically with `--no-verify`:

1. **Task 1: tests/v11_bulk_toggle.rs (15 SQLite cases)** — `76eb44d` (test)
2. **Task 2: tests/v11_bulk_toggle_pg.rs (5 Postgres parity cases)** — `52c8139` (test)

_Plan metadata commit (this SUMMARY.md) follows._

## Files Created

- `tests/v11_bulk_toggle.rs` — 612 lines, 15 `#[tokio::test]` cases, mock scheduler harness, body-construction helper
- `tests/v11_bulk_toggle_pg.rs` — 248 lines, 5 `#[tokio::test]` cases, testcontainers-postgres harness

## Test Inventory (Plan-by-Plan Mapping)

The Wave 0 plan promised every test maps 1:1 to a row in 14-VALIDATION.md's Per-Task Verification Map and to the implementation plan that turns it green. Here is the complete mapping:

| Test                                                | File                          | Locks (Req / Inv)              | Turned green by |
|-----------------------------------------------------|-------------------------------|--------------------------------|-----------------|
| `upsert_invariant`                                  | `v11_bulk_toggle.rs`          | T-V11-BULK-01                  | Plan 02 + 03    |
| `reload_invariant`                                  | `v11_bulk_toggle.rs`          | ERG-04 (reload preserves)      | Plan 03         |
| `disable_missing_clears_override`                   | `v11_bulk_toggle.rs`          | ERG-04 (symmetric clear)       | Plan 03         |
| `dashboard_filter`                                  | `v11_bulk_toggle.rs`          | DB-14 (filter)                 | Plan 03         |
| `handler_csrf`                                      | `v11_bulk_toggle.rs`          | ERG-01 (CSRF gate)             | Plan 04         |
| `handler_disable`                                   | `v11_bulk_toggle.rs`          | ERG-01 (disable path)          | Plan 04         |
| `handler_enable`                                    | `v11_bulk_toggle.rs`          | ERG-01 + D-05 (clear-on-enable)| Plan 04         |
| `handler_partial_invalid`                           | `v11_bulk_toggle.rs`          | D-12                           | Plan 04         |
| `handler_partial_invalid_toast_uses_rows_affected`  | `v11_bulk_toggle.rs`          | UI-SPEC primary-count          | Plan 04         |
| `handler_dedupes_ids`                               | `v11_bulk_toggle.rs`          | D-12a                          | Plan 04         |
| `handler_rejects_empty`                             | `v11_bulk_toggle.rs`          | UI-SPEC + Landmine §9          | Plan 04         |
| `handler_accepts_repeated_job_ids`                  | `v11_bulk_toggle.rs`          | Landmine §1 (axum_extra::Form) | Plan 04         |
| `handler_fires_reload_after_update`                 | `v11_bulk_toggle.rs`          | ERG-01 + Landmine §6 (ordering)| Plan 04         |
| `get_overridden_jobs_alphabetical`                  | `v11_bulk_toggle.rs`          | ERG-03 + D-10b                 | Plan 03         |
| `settings_empty_state_hides_section`                | `v11_bulk_toggle.rs`          | ERG-03 + D-10a                 | Plan 06         |
| `upsert_invariant_pg`                               | `v11_bulk_toggle_pg.rs`       | T-V11-BULK-01 (Postgres)       | Plan 02 + 03    |
| `disable_missing_clears_override_pg`                | `v11_bulk_toggle_pg.rs`       | ERG-04 (Postgres)              | Plan 03         |
| `dashboard_filter_pg`                               | `v11_bulk_toggle_pg.rs`       | DB-14 (Postgres)               | Plan 03         |
| `bulk_set_override_pg`                              | `v11_bulk_toggle_pg.rs`       | ANY($2) array bind path        | Plan 03         |
| `get_overridden_jobs_alphabetical_pg`               | `v11_bulk_toggle_pg.rs`       | ERG-03 + D-10b (Postgres)      | Plan 03         |

**Total: 20 named tests** — exceeds the planned `15 + 5 = 20` minimum.

## Red-Bar Scoreboard (Baseline)

`cargo test --test v11_bulk_toggle --no-run` (top of compile output, 2026-04-22):

```
warning: cronduit@1.1.0: Tailwind binary not found at bin/tailwindcss — run `just tailwind` to build CSS
   Compiling cronduit v1.1.0
error[E0432]: unresolved import `cronduit::web::handlers::api::bulk_toggle`
  --> tests/v11_bulk_toggle.rs:45:5
   |
45 | use cronduit::web::handlers::api::bulk_toggle;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no `bulk_toggle` in `web::handlers::api`

error[E0432]: unresolved import `cronduit::web::handlers::settings::OverriddenJobView`
  --> tests/v11_bulk_toggle.rs:46:41
   |
46 | use cronduit::web::handlers::settings::{OverriddenJobView, SettingsPage};
   |                                         ^^^^^^^^^^^^^^^^^ no `OverriddenJobView` in `web::handlers::settings`

error[E0425]: cannot find function `bulk_set_override` in module `queries`
error[E0425]: cannot find function `get_overridden_jobs` in module `queries`
error[E0609]: no field `enabled_override` on type `DbJob`
error[E0603]: struct `SettingsPage` is private
```

**SQLite test compile-error class breakdown** (`grep -E "^error\[" | sort | uniq -c`):

| Count | Error code | Meaning                                                            | Cleared by |
|------:|------------|--------------------------------------------------------------------|------------|
|     9 | E0609      | `no field 'enabled_override' on type 'DbJob'`                       | Plan 02 (struct + migration) |
|     8 | E0282      | `type annotations needed` (cascade from missing `bulk_set_override`)| Plan 03 (queries) |
|     7 | E0425      | `cannot find function 'bulk_set_override' in module 'queries'`      | Plan 03 |
|     1 | E0425      | `cannot find function 'get_overridden_jobs' in module 'queries'`    | Plan 03 |
|     1 | E0432      | `unresolved import 'api::bulk_toggle'`                              | Plan 04 (handler) |
|     1 | E0432      | `unresolved import 'settings::OverriddenJobView'`                   | Plan 06 (settings page) |
|     1 | E0603      | `struct 'SettingsPage' is private`                                  | Plan 06 (make `pub`) |
| **28**| total      |                                                                    |            |

**Postgres test compile-error class breakdown:**

| Count | Error code | Meaning                                                            | Cleared by |
|------:|------------|--------------------------------------------------------------------|------------|
|     8 | E0282      | type-inference cascade from missing `bulk_set_override`             | Plan 03 |
|     7 | E0425      | `bulk_set_override` not found                                       | Plan 03 |
|     5 | E0609      | `enabled_override` field absent                                     | Plan 02 |
|     1 | E0425      | `get_overridden_jobs` not found                                     | Plan 03 |
| **21**| total      |                                                                    |            |

Combined Wave 0 baseline: **49 compile errors**, all named symbols match the Plan 02/03/04/06 task lists exactly. Plans clear errors in this order:

1. **Plan 02** (migration + DbJob field) clears all `E0609` (14 total) by adding `enabled_override` to `DbJob` + `SqliteDbJobRow` + `PgDbJobRow`.
2. **Plan 03** (queries) clears all `E0425` + cascade `E0282` (31 total) by adding `bulk_set_override` and `get_overridden_jobs`.
3. **Plan 04** (handler) clears the `bulk_toggle` `E0432` (1) by exporting `pub async fn bulk_toggle`.
4. **Plan 06** (settings) clears the remaining `OverriddenJobView` `E0432` (1) and the `SettingsPage` `E0603` (1) by adding `pub struct OverriddenJobView` and changing `struct SettingsPage` → `pub struct SettingsPage`.

When all 49 errors clear, both files compile and the 20 tests transition from compile-error to runtime green pending the actual behavior implementation.

## Verification

Both verify commands from the plan exit non-zero (intentional red bar):

```
$ cargo test --test v11_bulk_toggle --no-run
... 28 errors ...
error: could not compile `cronduit` (test "v11_bulk_toggle") due to 28 previous errors
$ echo $?
101

$ cargo test --test v11_bulk_toggle_pg --no-run
... 21 errors ...
error: could not compile `cronduit` (test "v11_bulk_toggle_pg") due to 21 previous errors
$ echo $?
101
```

Plan acceptance criteria all satisfied:

- [x] `tests/v11_bulk_toggle.rs` exists, 612 lines (≥ 200)
- [x] `grep -c "^#\[tokio::test\]" tests/v11_bulk_toggle.rs` → **15** (≥ 15)
- [x] All 15 named test functions present (verified with the plan's verbatim grep pattern)
- [x] `build_bulk_request`, `SchedulerCmd::Reload`, `enabled_override` all referenced
- [x] `cargo test --test v11_bulk_toggle --no-run` exits non-zero
- [x] No `#[ignore]` attributes
- [x] `tests/v11_bulk_toggle_pg.rs` exists with 5 named `#[tokio::test]` cases
- [x] `Postgres::default().start()`, `pool.backend(), DbBackend::Postgres`, `enabled_override` all referenced
- [x] No `#[cfg(feature = "pg-integration")]` gate (matches dashboard_jobs_pg.rs precedent)

## Deviations from Plan

### Adaptations applied (no Rule-3 fixes needed)

**1. [Rule N/A — design choice within plan latitude] reload_invariant uses `parse_and_validate` + tempfile**

- **Found during:** Task 1 implementation
- **Issue:** Plan said "call `crate::config::sync::sync_config_to_db(…)`" but `Config` and `JobConfig` do not implement `Default` and have many required fields (e.g., `ServerConfig.timezone`, `JobConfig.use_defaults`). Constructing them inline via struct literals would be brittle and ALSO produce compile errors that aren't part of the intended red-bar signal.
- **Resolution:** Followed the established pattern from `tests/reload_inflight.rs::inflight_run_survives_reload` L41-50: write a minimal TOML to `tempfile::NamedTempFile`, call `parse_and_validate`, pass `&parsed.config` to `sync_config_to_db`. Production code path; same precedent as 4 other reload tests. `tempfile` is already in `[dependencies]` (line 81 of Cargo.toml).
- **Files affected:** `tests/v11_bulk_toggle.rs` (reload_invariant test only)
- **Commit:** 76eb44d

**2. [Rule N/A — naming nuance] Used `SchedulerCmd::Reload { response_tx }` with `ReloadResult` 5-field constructor**

- **Found during:** Task 1 implementation
- **Issue:** Plan's example reply omitted the `unchanged` field of `ReloadResult` (`status, added, updated, disabled, error_message` only).
- **Resolution:** Confirmed via `src/scheduler/cmd.rs` L66-73 that `ReloadResult` has 6 fields (`status, added, updated, disabled, unchanged, error_message`); included `unchanged: 0` in the mock-scheduler reply. Without this the tests would fail to compile for an unrelated reason (missing field `unchanged`), polluting the red-bar signal.
- **Files affected:** `tests/v11_bulk_toggle.rs` (`build_bulk_app` helper)
- **Commit:** 76eb44d

No other deviations. No auth gates encountered (this is a code-only test-authoring plan).

## Authentication Gates

None encountered.

## Self-Check: PASSED

**Files exist:**
```
$ [ -f tests/v11_bulk_toggle.rs ] && echo FOUND
FOUND
$ [ -f tests/v11_bulk_toggle_pg.rs ] && echo FOUND
FOUND
```

**Commits exist:**
```
$ git log --oneline | grep -E "(76eb44d|52c8139)"
52c8139 test(14-01): add Wave 0 red-bar Postgres parity tests for bulk toggle
76eb44d test(14-01): add Wave 0 red-bar SQLite tests for bulk toggle
```

**Test counts:**
```
$ grep -c "^#\[tokio::test\]" tests/v11_bulk_toggle.rs
15
$ grep -c "^#\[tokio::test\]" tests/v11_bulk_toggle_pg.rs
5
```

**Red bar:**
```
$ cargo test --test v11_bulk_toggle --no-run; echo "exit=$?"
... 28 compile errors ...
exit=101
$ cargo test --test v11_bulk_toggle_pg --no-run; echo "exit=$?"
... 21 compile errors ...
exit=101
```

All acceptance criteria from `<acceptance_criteria>` blocks in 14-01-PLAN.md verified.

## Threat Flags

None — Wave 0 plan introduces only test files. No new network endpoints, auth paths, file access patterns, or schema changes at trust boundaries beyond what the plan's `<threat_model>` already enumerates (T-14-01-01 mitigation honored: handler-input encoding uses literal `&` separators in the `build_bulk_request` helper, not `serde_urlencoded::to_string`).

## Known Stubs

None — Wave 0 contains tests only, no stub UI components or hardcoded empty-flow values.

## Notes for Plans 02-06

- **Plan 02** (DB-14 migration + DbJob field): when adding `enabled_override` to the three structs (`DbJob` `pub`, `SqliteDbJobRow` `Option<i32>`, `PgDbJobRow` `Option<i64>`), all 14 `E0609` errors clear. The SQLite-side `From` impl widens `i32 → i64` (per 14-PATTERNS.md §5).
- **Plan 03** (queries): adding `pub async fn bulk_set_override` and `pub async fn get_overridden_jobs` clears all `E0425` + cascade `E0282`. SQLite uses `?1..?N` placeholder list (binding `new_override` first, ids second); Postgres uses `ANY($2)` array bind. Disable_missing_jobs SET clause must extend to `enabled = 0, enabled_override = NULL` on all 4 paths.
- **Plan 04** (handler): export `pub async fn bulk_toggle` from `src/web/handlers/api.rs`. Use `axum_extra::extract::Form<BulkToggleForm>` (NOT stock `axum::Form` — Landmine §1). Note: the `axum-extra = { version = "0.12", features = ["cookie", "query"] }` dependency does NOT include the `form` feature; Plan 04 must add `"form"` to that feature list.
- **Plan 06** (settings page): change `struct SettingsPage` to `pub struct SettingsPage`, make all fields `pub`, define `pub struct OverriddenJobView { pub id: i64, pub name: String, pub enabled_override: i64 }`. The `settings_empty_state_hides_section` test renders the template directly via askama's `Template::render()` so the template file MUST contain the literal substring `Currently Overridden` only inside an `{% if !overridden_jobs.is_empty() %}` block.
