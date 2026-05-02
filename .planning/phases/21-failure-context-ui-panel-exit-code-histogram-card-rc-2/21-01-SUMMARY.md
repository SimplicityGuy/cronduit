---
phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
plan: 01
subsystem: database
tags: [sqlx, sqlite, postgres, migrations, scheduled_for, fctx-06]

# Dependency graph
requires:
  - phase: 16-image-digest
    provides: P16 image_digest_add migration analog (header structure + nullable TEXT additive ALTER pattern)
provides:
  - "job_runs.scheduled_for TEXT NULL column on sqlite + postgres"
  - "Foundation for Wave 2 plans: insert_running_run widening, DbRunDetail field add, run_detail handler wire-up, FCTX panel FIRE SKEW row"
  - "Pattern: when sqlx migration version-prefix collides with an existing same-day file, bump the date prefix and document inline"
affects: [21-02, 21-03, 21-04, 21-05, 21-06]  # Wave 2 plans depend on this column existing

# Tech tracking
tech-stack:
  added: []  # zero new external crates (D-32 invariant preserved)
  patterns:
    - "Pair-file migration invariant (sqlite + postgres) with header cross-reference"
    - "Date-prefixed migration filename with monotonic sequence counter; sqlx parses version from leading digits before first underscore — uniqueness must hold across the integer prefix, not the full filename"

key-files:
  created:
    - migrations/sqlite/20260503_000009_scheduled_for_add.up.sql
    - migrations/postgres/20260503_000009_scheduled_for_add.up.sql
  modified: []

key-decisions:
  - "Bumped date prefix from plan-specified 20260502 to 20260503 because the existing migration 20260502_000008_webhook_deliveries_add already occupies sqlx version 20260502 (sqlx splits on first underscore — see Rule 1 deviation below)"
  - "No index, no DEFAULT, no NOT NULL — D-01/D-04/D-05 compliance (NULL is the intended legacy state forever; skew is read on a single-row select keyed by r.id)"
  - "Postgres uses ALTER TABLE ADD COLUMN IF NOT EXISTS for re-run safety; sqlite has no such guard but is protected by sqlx _sqlx_migrations ledger"

patterns-established:
  - "Pair-file invariant: any structural change to one backend's migration MUST land in the other in the same PR; tests/schema_parity.rs::normalize_type collapses TEXT-family types to TEXT so the column passes parity with zero test edits"
  - "Header comment cites D-decision IDs and the sibling pair file path for cross-discoverability"

requirements-completed: [FCTX-06]

# Metrics
duration: ~12min
completed: 2026-05-02
---

# Phase 21 Plan 01: scheduled_for migration (FCTX-06) Summary

**Additive nullable `job_runs.scheduled_for TEXT` column on sqlite + postgres backends, underpinning the Phase 21 FIRE SKEW failure-context row.**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-05-02T19:21Z (approx)
- **Completed:** 2026-05-02T19:34Z
- **Tasks:** 3 (2 file creates + 1 verification gate)
- **Files modified:** 2 (both newly created)

## Accomplishments
- sqlite migration `20260503_000009_scheduled_for_add.up.sql` adds nullable TEXT column (no index, no default, no NOT NULL)
- postgres pair file mirrors with `IF NOT EXISTS` guard for ledger-drift safety
- Filename parity preserved across `migrations/sqlite/` and `migrations/postgres/`
- Caught and fixed a sqlx version-prefix collision the plan template missed (Rule 1 auto-fix — see Deviations)
- Verified `cargo build --workspace` green and `cargo tree -i openssl-sys` empty (D-32 rustls-only invariant holds)

## Task Commits

Each task was committed atomically (per-task), and a follow-on fix commit handled the version-prefix collision:

1. **Task 1: Create sqlite migration** — `683aafa` (feat)
2. **Task 2: Create postgres migration** — `0e6474b` (feat)
3. **Rule 1 fix: rename to 20260503 prefix** — `8921348` (fix)
4. **Task 3: Wave-end gate** — verification-only, no file changes, no commit

_Note: Tasks 1 and 2 were committed at the plan-specified path `20260502_000009_*`. Task 3's wave-end gate revealed the version collision; commit `8921348` renames both files to `20260503_000009_*` and updates header cross-references. The trailing two file states are what land on the branch._

## Files Created/Modified
- `migrations/sqlite/20260503_000009_scheduled_for_add.up.sql` — additive `ALTER TABLE job_runs ADD COLUMN scheduled_for TEXT;`
- `migrations/postgres/20260503_000009_scheduled_for_add.up.sql` — `ALTER TABLE job_runs ADD COLUMN IF NOT EXISTS scheduled_for TEXT;`

## Decisions Made
- **Date prefix bump (20260502 → 20260503):** sqlx parses migration version from digits BEFORE the first underscore (`splitn(2, '_')` — `sqlx-core/src/migrate/source.rs:97`). The plan-specified prefix `20260502_000009` collided with the existing `20260502_000008_webhook_deliveries_add`; both resolve to integer version `20260502`, triggering `UNIQUE constraint failed: _sqlx_migrations.version` on apply. Bumping to `20260503` resolves the collision while preserving the date-prefixed lexical-sort convention. Header comments in both files inline-document the rationale so future maintainers don't reflexively "renormalize" the prefix.
- **No data migration, no index, no DEFAULT, no NOT NULL:** D-01/D-04/D-05 compliance — pre-v1.2 rows stay NULL forever; the FIRE SKEW row in the failure-context UI panel hides on NULL per UI-SPEC; skew is a single-row read keyed by `r.id` so no fleet-level filter index is justified.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] Plan-specified migration filename collided with existing migration's sqlx version**
- **Found during:** Task 3 (wave-end gate — schema_parity test surfaced `UNIQUE constraint failed: _sqlx_migrations.version` on sqlite migrate).
- **Issue:** Plan specified filename prefix `20260502_000009_scheduled_for_add.up.sql`. The existing predecessor `20260502_000008_webhook_deliveries_add.up.sql` (already on the branch from Phase 20 merge in commit e87dd42) shares the leading `20260502_` segment. sqlx parses the version using `file_name.splitn(2, '_')` — only the chars before the FIRST underscore — so both files resolved to integer version `20260502` and the second one to apply hit a UNIQUE constraint on `_sqlx_migrations.version`.
- **Fix:** Renamed both files (sqlite + postgres) from `20260502_000009_scheduled_for_add.up.sql` → `20260503_000009_scheduled_for_add.up.sql`. Updated the header `Pairs with …` cross-references in both files and added an inline note explaining the prefix bump so future maintainers don't try to "renormalize" the date back.
- **Files modified:** `migrations/sqlite/20260503_000009_scheduled_for_add.up.sql`, `migrations/postgres/20260503_000009_scheduled_for_add.up.sql` (renames + header tweak).
- **Verification:** SQLite migrate path now applies cleanly — the `tests/db_pool_sqlite.rs` suite (which runs `sqlx::migrate!("./migrations/sqlite")` internally) passes, and the prior `UNIQUE constraint failed` panic from `tests/schema_parity.rs` is gone (replaced by the unrelated `SocketNotFoundError("/var/run/docker.sock")` Postgres testcontainer error caused by the sandbox having no Docker daemon).
- **Committed in:** `8921348` (`fix(21-01): bump scheduled_for migration date prefix to 20260503`).

**Plan success-criteria impact:** The plan's `<must_haves><truths>` and `<success_criteria>` reference the literal filename `20260502_000009_scheduled_for_add.up.sql`. Those literal paths are technically violated; the SEMANTIC intent (one additive nullable TEXT column per backend, filename-parity preserved, monotonic ordering after `_000008_webhook_deliveries_add`) is fully satisfied. Wave 2 plans that hardcode a path string against this filename should reference `20260503_000009_*` instead. Recommend the plan template be updated to detect existing same-day prefixes before locking a filename.

---

**Total deviations:** 1 auto-fixed (1 bug — sqlx version collision)
**Impact on plan:** Filename-only deviation. No schema, semantic, or behavior change. Filename parity preserved across sqlite/postgres dirs. All other plan invariants (no index, no DEFAULT, no NOT NULL, header references FCTX-06 + sibling pair, parity-friendly TEXT type) intact.

## Issues Encountered

- **Schema_parity full test cannot run in this sandbox:** the test uses `testcontainers-modules::postgres::Postgres` which requires a live Docker daemon at `/var/run/docker.sock`. The sandbox has no Docker daemon running. The test was verified to NOT fail on the SQLite-side migration (the previous UNIQUE constraint panic is gone after the rename); the Postgres parity check is deferred to CI where Docker IS available. The `normalize_tests` submodule (`known_types_normalize_correctly`, `unknown_type_panics`) — which validates the TEXT-family collapse that lets `scheduled_for` pass parity with zero test edits — passes cleanly.

## User Setup Required

None — schema-only change, applied automatically on the next `cronduit` start via `sqlx::migrate!`.

## Next Phase Readiness

- **Wave 2 plans (21-02 through 21-06)** can now reference `job_runs.scheduled_for` in INSERT/SELECT queries.
- **Filename to use in downstream `@-references`:** `migrations/sqlite/20260503_000009_scheduled_for_add.up.sql` and `migrations/postgres/20260503_000009_scheduled_for_add.up.sql` (NOT the plan-frontmatter `20260502_000009_*`).
- **schema_parity full run** must pass on CI (Docker available) before merge — verifies the column appears identically across both backends.

## Threat Flags

None — additive nullable TEXT column on an existing table, no new external surface, no auth/network/file-access change. Threat register T-21-01-01 (Tampering, accept) and T-21-01-02 (Information Disclosure, accept) remain valid as written.

## Self-Check: PASSED

- `migrations/sqlite/20260503_000009_scheduled_for_add.up.sql` — FOUND (committed in 683aafa, renamed in 8921348)
- `migrations/postgres/20260503_000009_scheduled_for_add.up.sql` — FOUND (committed in 0e6474b, renamed in 8921348)
- Commit `683aafa` — FOUND in `git log --all`
- Commit `0e6474b` — FOUND in `git log --all`
- Commit `8921348` — FOUND in `git log --all`
- `cargo build --workspace` — exits 0
- `cargo tree -i openssl-sys` — returns no matching packages (D-32 invariant)
- Filename parity (`diff <(ls migrations/sqlite | sort) <(ls migrations/postgres | sort)`) — exits 0
- `tests/db_pool_sqlite.rs` suite — passes (proves sqlite migration applies cleanly end-to-end)
- `tests/schema_parity.rs::normalize_tests` — passes (proves TEXT-family collapse covers `scheduled_for`)
- `tests/schema_parity.rs::sqlite_and_postgres_schemas_match_structurally` — Docker-dependent, deferred to CI

---
*Phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2*
*Plan: 01*
*Completed: 2026-05-02*
