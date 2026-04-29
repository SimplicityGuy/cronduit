---
phase: 16-failure-context-schema-run-rs-277-bug-fix
plan: 04a
subsystem: database
tags: [sqlx, sqlite, postgres, queries, finalize_run, insert_running_run, signature-change, FOUND-14, FCTX-04]

# Dependency graph
requires:
  - phase: 16-failure-context-schema-run-rs-277-bug-fix
    provides: "job_runs.image_digest TEXT NULL + job_runs.config_hash TEXT NULL columns (Plan 16-01)"
  - phase: 16-failure-context-schema-run-rs-277-bug-fix
    provides: "DockerExecResult.container_id field (Plan 16-02) — enables 16-03's bug fix that 16-04a's signature accepts"
  - phase: 16-failure-context-schema-run-rs-277-bug-fix
    provides: "src/scheduler/run.rs:348-356 finalize_run call site already passing 8 args (Plan 16-03)"
provides:
  - "queries::finalize_run accepts image_digest: Option<&str> as 8th positional; both backends bind it into job_runs.image_digest at position ?7/$7"
  - "queries::insert_running_run accepts config_hash: &str as 4th positional; both backends bind it into job_runs.config_hash at position ?5/$5"
  - "DbRun gains pub image_digest: Option<String> + pub config_hash: Option<String> with Phase 16 doc comments"
  - "DbRunDetail gains the same two fields with identical doc comments"
  - "get_run_history SELECT (both backends) hydrates the two new columns into DbRun"
  - "get_run_by_id SELECT (both backends, JOIN-prefixed) hydrates the two new columns into DbRunDetail"
affects:
  - 16-04b (production callers + 5 test-mod callers + just recipe + wave-end gate — must update insert_running_run + finalize_run sites + api.rs error fallback)
  - 16-05 (get_failure_context can SELECT last_success_image_digest + last_success_config_hash from job_runs)
  - 18 (webhook payload serializer reads DbRun.image_digest + DbRun.config_hash directly)
  - 21 (FCTX UI panel renders image_digest + config_hash deltas from DbRunDetail)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Per-backend SQL signature mirroring: signature additions to a queries.rs helper land in BOTH the SQLite and Postgres arms of the match pool.{writer,reader}() block in lock-step within a single commit; no in-between state where the two arms disagree."
    - "Bind-position discipline: when extending an UPDATE/INSERT statement, the new column slots in BEFORE the WHERE-clause bind (UPDATE) or BEFORE the closing paren of VALUES (INSERT); existing positional bindings stay numbered consecutively without gaps."
    - "Read-side struct widening with REQ-ID doc comments: each new Option<String> field on DbRun/DbRunDetail carries a Phase 16 + REQ-ID (FOUND-14 / FCTX-04) doc comment + a NULL-semantics note + a backfill-marker reference for downstream UI consumers."

key-files:
  created: []
  modified:
    - src/db/queries.rs

key-decisions:
  - "Doc comment on finalize_run extended in the same commit as the signature change (T1) to avoid a stale 6-field doc surviving past the 7-field signature; Phase 16 FOUND-14 + the NULL-for-command/script note land together with the new image_digest parameter."
  - "config_hash bind count grep returns 4, not 2, because 2 pre-existing .bind(config_hash) sites already exist in insert_job / update_job (jobs.config_hash writes from Phase 11). The plan's >=2 acceptance threshold accounts for this — the new INSERT binds in insert_running_run are 2 of the 4."
  - "Doc comments on the new struct fields include the BACKFILL_CUTOFF_RFC3339 marker reference (D-03) so downstream UI implementers (Phase 21) can grep DbRun/DbRunDetail definitions and find the convention without re-reading 16-CONTEXT.md."

patterns-established:
  - "Signature-change commit triplet for queries.rs helpers: (a) signature + UPDATE/INSERT statement + bind chain on both backends; (b) struct field widening with REQ-ID doc comments; (c) SELECT-side hydration on every reader site that produces the widened struct. Each step is independently verifiable via the plan's grep-based acceptance criteria."

requirements-completed: []  # FOUND-14 + FCTX-04 fully complete only after 16-04b lands callers
---

# Phase 16 Plan 04a: queries.rs signature changes (FOUND-14 + FCTX-04 DB tier) Summary

**queries.rs accepts the new positional parameters and exposes the new struct fields for both image_digest (FOUND-14) and config_hash (FCTX-04) — the load-bearing single-file seam where the schema substrate (Plan 16-01), the docker.rs container_id field (Plan 16-02), and the run.rs:301 bug fix (Plan 16-03) all converge into actual DB writes via signatures + struct widening. Compile failure is EXPECTED at this plan's close until 16-04b lands the production callers + test callers + wave-end gate.**

## Performance

- **Duration:** ~4 min
- **Started:** 2026-04-28T02:58:43Z
- **Completed:** 2026-04-28T03:02:42Z
- **Tasks:** 5 / 5
- **Files modified:** 1 (`src/db/queries.rs`)
- **Files created:** 0

## Accomplishments

- `queries::finalize_run` signature gains `image_digest: Option<&str>` as the 8th positional parameter; both SQLite and Postgres UPDATE statements bind the new column at position ?7/$7 (the WHERE-clause placeholder bumps from ?7/$7 to ?8/$8).
- `queries::insert_running_run` signature gains `config_hash: &str` as the 4th positional parameter; both SQLite and Postgres INSERT statements include `config_hash` in the column list and bind it at position ?5/$5.
- `DbRun` and `DbRunDetail` each gain `pub image_digest: Option<String>` and `pub config_hash: Option<String>` fields with identical Phase 16 doc comments referencing FOUND-14, FCTX-04, the BACKFILL_CUTOFF_RFC3339 marker (D-03), and the NULL-semantics convention.
- `get_run_history` SELECT (both backends) appends `, image_digest, config_hash` to the projection and hydrates the two new fields into `DbRun` via `r.get(...)`.
- `get_run_by_id` SQL literals (both backends, with `r.` JOIN-alias prefix) append `r.image_digest, r.config_hash` to the projection; both hydration arms hydrate the two new fields into `DbRunDetail`.

## Task Commits

| # | Task | Commit | Type |
|---|------|--------|------|
| 1 | Extend `finalize_run` with `image_digest: Option<&str>` (8th positional) | `c50f55e` | feat |
| 2 | Extend `insert_running_run` with `config_hash: &str` (4th positional) | `d1d0fd1` | feat |
| 3 | Add `image_digest` + `config_hash` fields to `DbRun` and `DbRunDetail` | `0411022` | feat |
| 4 | Hydrate the two new fields in `get_run_history` (SQLite + Postgres) | `ba239a7` | feat |
| 5 | Hydrate the two new fields in `get_run_by_id` (SQLite + Postgres) | `74a7fc5` | feat |

All five commits use `--no-verify` per the wave-2 sequential-executor policy (the wave-end gate runs once after 16-04b lands callers).

## Files Created/Modified

### Modified

- **`src/db/queries.rs`** (5 surgical edits across 5 commits)
  - **L368-L378 (T2):** `insert_running_run` signature widened with `config_hash: &str`; doc comment cites Phase 16 FCTX-04 + reload-mid-fire correctness rationale.
  - **L388-L397 + L414-L423 (T2):** Both backend INSERT statements add `config_hash` to the column list (`(job_id, status, trigger, start_time, job_run_number, config_hash)`) and a new `?5`/`$5` bind to VALUES; `.bind(config_hash)` lands after `.bind(reserved)` in both arms.
  - **L424-L437 (T1):** `finalize_run` signature widened with `image_digest: Option<&str>` as the 8th positional; doc comment extended to list image_digest in the SET column list.
  - **L447-L450 + L463-L466 (T1):** Both backend UPDATE statements add `, image_digest = ?7` (SQLite) / `, image_digest = $7` (Postgres) to the SET list; WHERE-clause placeholder bumps to `?8`/`$8`; `.bind(image_digest)` slots between `.bind(container_id)` and `.bind(run_id)` in both arms.
  - **L552-L598 (T3):** `DbRun` (after L566 `error_message`) and `DbRunDetail` (after L583 `error_message`) each append two `Option<String>` fields (`image_digest`, `config_hash`) with Phase 16 + REQ-ID + BACKFILL_CUTOFF_RFC3339 marker doc comments.
  - **L1091 + L1115-L1116 + L1127 + L1136-L1137 (T4):** `get_run_history` SQLite + Postgres SELECT projections gain `, image_digest, config_hash`; both hydration `.map(|r| DbRun {..})` arms gain `image_digest: r.get("image_digest")` + `config_hash: r.get("config_hash")` lines (with Phase 16 FOUND-14 / FCTX-04 trailing comments).
  - **L1147-L1148 + L1156-L1157 + L1180-L1181 + L1198-L1199 (T5):** `get_run_by_id` `sql_sqlite` + `sql_postgres` raw-string literals gain `r.image_digest, r.config_hash` (note the `r.` prefix because both literals JOIN `job_runs r` with `jobs j`); both hydration `Some(DbRunDetail {..})` arms gain the same two `r.get(...)` lines.

## Before / After Diffs

### `finalize_run` signature (T1)

**Before:**
```rust
/// Finalize a job run by updating its status, exit_code, end_time, duration_ms, error_message, and container_id.
pub async fn finalize_run(
    pool: &DbPool,
    run_id: i64,
    status: &str,
    exit_code: Option<i32>,
    start_instant: tokio::time::Instant,
    error_message: Option<&str>,
    container_id: Option<&str>,
) -> anyhow::Result<()>
```

**After:**
```rust
/// Finalize a job run by updating its status, exit_code, end_time, duration_ms, error_message, container_id, and image_digest.
/// Phase 16 FOUND-14: image_digest captured from `inspect_container` post-start; NULL for command/script jobs.
pub async fn finalize_run(
    pool: &DbPool,
    run_id: i64,
    status: &str,
    exit_code: Option<i32>,
    start_instant: tokio::time::Instant,
    error_message: Option<&str>,
    container_id: Option<&str>,
    image_digest: Option<&str>, // Phase 16 FOUND-14
) -> anyhow::Result<()>
```

### `insert_running_run` signature (T2)

**Before:**
```rust
pub async fn insert_running_run(pool: &DbPool, job_id: i64, trigger: &str) -> anyhow::Result<i64>
```

**After:**
```rust
/// ... existing doc paragraphs ...
///
/// Phase 16 FCTX-04: `config_hash` is captured at fire time (BEFORE the executor
/// spawns) and bound into the new `job_runs.config_hash` column so a
/// reload-mid-fire still reflects the run's actual config rather than the latest.
pub async fn insert_running_run(
    pool: &DbPool,
    job_id: i64,
    trigger: &str,
    config_hash: &str, // Phase 16 FCTX-04
) -> anyhow::Result<i64>
```

### SQLite INSERT statement (T2)

**Before:**
```rust
"INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number) \
 VALUES (?1, 'running', ?2, ?3, ?4) RETURNING id"
```

**After:**
```rust
"INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number, config_hash) \
 VALUES (?1, 'running', ?2, ?3, ?4, ?5) RETURNING id"
```
(Postgres mirrors with `$N` placeholders.)

### SQLite UPDATE statement (T1)

**Before:**
```rust
"UPDATE job_runs SET status = ?1, exit_code = ?2, end_time = ?3, duration_ms = ?4, error_message = ?5, container_id = ?6 WHERE id = ?7"
```

**After:**
```rust
"UPDATE job_runs SET status = ?1, exit_code = ?2, end_time = ?3, duration_ms = ?4, error_message = ?5, container_id = ?6, image_digest = ?7 WHERE id = ?8"
```
(Postgres mirrors with `$N` placeholders.)

### `DbRun` / `DbRunDetail` field additions (T3)

Both structs gain (after `pub error_message: Option<String>`):
```rust
/// Phase 16 FOUND-14: image digest from post-start `inspect_container`. NULL for
/// command/script jobs (no image), pre-v1.2 docker rows (capture site landed in v1.2).
pub image_digest: Option<String>,
/// Phase 16 FCTX-04: per-run config_hash captured at fire time by
/// `insert_running_run`. NULL for pre-v1.2 rows whose backfill found no matching
/// `jobs.config_hash`. See migration `*_000007_config_hash_backfill.up.sql` for
/// the BACKFILL_CUTOFF_RFC3339 marker (D-03).
pub config_hash: Option<String>,
```

## Four SELECT-Site Updates (line numbers + diff narration)

| Site | Line(s) | What changed |
|------|---------|--------------|
| `get_run_history` SQLite SELECT | 1091 | Appended `, image_digest, config_hash` to the projection (column list extension only; no WHERE/ORDER changes) |
| `get_run_history` SQLite hydration | 1110-1116 | Appended `image_digest: r.get("image_digest")` + `config_hash: r.get("config_hash")` lines inside the `.map(|r| DbRun { .. })` block |
| `get_run_history` Postgres SELECT | 1127 | Same as SQLite (no `r.` prefix needed; this is the unprefixed projection) |
| `get_run_history` Postgres hydration | 1146-1152 | Same two `.get(...)` lines as SQLite |
| `get_run_by_id` `sql_sqlite` literal | 1163 | Wrapped: appended `r.image_digest, r.config_hash` (note the `r.` prefix because the SELECT JOINs `job_runs r` with `jobs j`) |
| `get_run_by_id` `sql_postgres` literal | 1171 | Same as `sql_sqlite` (placeholder differs but column-list extension is identical) |
| `get_run_by_id` SQLite hydration | 1199-1200 | Appended two `r.get(...)` lines inside the `Some(DbRunDetail { .. })` block |
| `get_run_by_id` Postgres hydration | 1219-1220 | Same two `r.get(...)` lines as SQLite |

## Verification

| Check | Expected | Actual |
|-------|----------|--------|
| `grep -q 'image_digest: Option<&str>' src/db/queries.rs` | exit 0 | exit 0 |
| `grep -q 'image_digest = ?7' src/db/queries.rs` | exit 0 | exit 0 |
| `grep -q 'image_digest = \$7' src/db/queries.rs` | exit 0 | exit 0 |
| `grep -c '\.bind(image_digest)' src/db/queries.rs` | >= 2 | 2 |
| `grep -q 'WHERE id = ?8' src/db/queries.rs` | exit 0 | exit 0 |
| `grep -q 'WHERE id = \$8' src/db/queries.rs` | exit 0 | exit 0 |
| `grep -q 'config_hash: &str' src/db/queries.rs` | exit 0 | exit 0 |
| `grep -q 'job_run_number, config_hash' src/db/queries.rs` | exit 0 | exit 0 |
| `grep -c '\.bind(config_hash)' src/db/queries.rs` | >= 2 | 4 (2 pre-existing in insert_job/update_job + 2 new in insert_running_run) |
| `DbRun` contains `image_digest: Option<String>` + `config_hash: Option<String>` | yes | yes |
| `DbRunDetail` contains `image_digest: Option<String>` + `config_hash: Option<String>` | yes | yes |
| `grep -c 'image_digest, config_hash FROM job_runs' src/db/queries.rs` | >= 2 | 2 (get_run_history SQLite + Postgres) |
| `grep -c 'r.image_digest, r.config_hash' src/db/queries.rs` | >= 2 | 2 (get_run_by_id sql_sqlite + sql_postgres) |
| `grep -c 'image_digest: r.get("image_digest")' src/db/queries.rs` | >= 4 | 4 (2 from get_run_history + 2 from get_run_by_id) |
| `grep -c 'config_hash: r.get("config_hash")' src/db/queries.rs` | >= 4 | 4 (same split) |

All 15 verification checks pass.

## Decisions Made

- **Doc comment on `finalize_run` updated in the same commit as the signature change (T1).** The plan recommended a fold-in to avoid a stale 6-field doc surviving past the 7-field signature; followed exactly. The new doc references Phase 16 FOUND-14 + the NULL-for-command/script semantic.
- **Reformatted `insert_running_run` signature to multi-line layout for the new param (T2).** The original was a single-line declaration; adding a 4th param + a trailing `// Phase 16 FCTX-04` comment justified the reflow to multi-line so each param is on its own line. Mirrors the style of `finalize_run` after T1's widening.
- **`config_hash` bind count grep returns 4, not 2.** The plan's acceptance criterion noted this explicitly: "this counts insert_running_run sites; later tasks add more bind sites for SELECT hydration is via .get not .bind, so 2 is the minimum." The other 2 `.bind(config_hash)` calls are pre-existing in `insert_job` (~L95) and `update_job` (~L122) — they bind `config_hash` to the `jobs` table column from Phase 11. Both contribute to the >= 2 acceptance threshold; no deviation.
- **BACKFILL_CUTOFF_RFC3339 marker reference deposited in DbRun/DbRunDetail doc comments (T3).** Phase 21's UI implementers can grep the struct definitions to find the marker convention without re-reading 16-CONTEXT.md. The reference points to `*_000007_config_hash_backfill.up.sql` (the wildcard accommodates the actual filename `20260429_000007_config_hash_backfill.up.sql` from Plan 16-01's deviation).

## Deviations from Plan

None — plan executed exactly as written.

The plan's intentional design (PLAN.md `<verification_criteria>`) explicitly states "Compile failure (missing call-site updates) is EXPECTED at the close of this plan; Plan 16-04b in the same Wave 2 batch resolves it before the wave-end gate runs." This is the documented Wave-2 behavior, not a deviation. The wave-2 sequential-executor context confirmed: "the codebase WILL still not fully compile after your plan finishes — that's expected. The wave-end gate is owned by 16-04b's task T5."

**Total deviations:** 0
**Impact on plan:** None.

## Issues Encountered

- **Worktree base mismatch at agent startup** — `git merge-base HEAD <expected-base>` returned `c87f12e` (Phase 15 close-out) instead of the expected `244bcc8` (16-03 wave-1 + wave-2 predecessor merge). Per the agent prompt's `<worktree_branch_check>` block, hard-reset the worktree to `244bcc8` before starting work. Verified the reset landed and that all three Wave-1/Wave-2 predecessors (16-01, 16-02, 16-03) are present in history. No data loss because this was a fresh worktree.
- **Build failure expected at this plan's close, by design.** Production callers in `src/scheduler/run.rs:83` (insert_running_run), `src/scheduler/run.rs:348` (finalize_run, already 8-arg from 16-03), `src/web/handlers/api.rs:82` (insert_running_run), and `src/web/handlers/api.rs:131` (finalize_run, currently 7-arg) all need updates that are out of scope for 16-04a (owned by 16-04b). Cargo build will fail at the call-site arity mismatch until 16-04b lands.

## User Setup Required

None — no external service configuration required. This is a pure code change in a single file.

## Next Phase Readiness

- **Plan 16-04b** is now unblocked: it owns the production-caller updates (`run.rs:83` insert_running_run + `api.rs:82` insert_running_run + `api.rs:131-140` finalize_run None fallback) + 5 test-mod callers + the new `just` recipe + the wave-end gate run. After 16-04b lands, `cargo build` becomes green again.
- **Plan 16-05** (`get_failure_context` query helper) can now SELECT `last_success_image_digest` and `last_success_config_hash` directly from `job_runs` per D-05's CTE shape — both columns are real and hydrated through `DbRun`/`DbRunDetail`.
- **Phase 18** (webhook payload, WH-09) will read `DbRun.image_digest` + `DbRun.config_hash` directly from the widened struct; no further queries.rs changes needed for that consumer.
- **Phase 21** (FCTX UI panel) will render image_digest + config_hash deltas from `DbRunDetail`; the BACKFILL_CUTOFF_RFC3339 marker reference in the doc comments points implementers at the convention.
- No new attack surface introduced. THREAT_MODEL.md unchanged. The plan's threat register (T-16-04a-01..03) remains accurate — all three threats are `accept` disposition with severity `low`, mitigated by sqlx parameterization (T-01), negligible per-row size growth (T-02), and template-side opt-in for the new fields (T-03).
- No `Cargo.toml`, dependency, or migration changes.

## Self-Check: PASSED

Verified at the end of execution:

- `src/db/queries.rs` exists — FOUND.
- All five commits present in branch:
  - `c50f55e` (T1, finalize_run signature) — FOUND.
  - `d1d0fd1` (T2, insert_running_run signature) — FOUND.
  - `0411022` (T3, DbRun + DbRunDetail field-add) — FOUND.
  - `ba239a7` (T4, get_run_history SELECT + hydration) — FOUND.
  - `74a7fc5` (T5, get_run_by_id SELECT + hydration) — FOUND.
- All 15 PLAN acceptance-criteria greps pass (per Verification table above).
- No modifications to `.planning/STATE.md` or `.planning/ROADMAP.md` — verified via `git status --short` (clean except for the SUMMARY.md being written now).
- No production-caller updates outside `src/db/queries.rs` — verified via `git log --name-only c50f55e^..HEAD` showing only `src/db/queries.rs` modified across all five commits.

---
*Phase: 16-failure-context-schema-run-rs-277-bug-fix*
*Plan: 04a — queries.rs signature changes (FOUND-14 + FCTX-04 DB tier)*
*Completed: 2026-04-28*
