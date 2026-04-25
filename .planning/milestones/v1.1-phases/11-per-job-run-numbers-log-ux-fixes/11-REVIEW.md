---
phase: 11-per-job-run-numbers-log-ux-fixes
reviewed: 2026-04-16T12:00:00Z
depth: standard
files_reviewed: 35
files_reviewed_list:
  - migrations/postgres/20260416_000001_job_run_number_add.up.sql
  - migrations/postgres/20260417_000002_job_run_number_backfill.up.sql
  - migrations/postgres/20260418_000003_job_run_number_not_null.up.sql
  - migrations/sqlite/20260416_000001_job_run_number_add.up.sql
  - migrations/sqlite/20260417_000002_job_run_number_backfill.up.sql
  - migrations/sqlite/20260418_000003_job_run_number_not_null.up.sql
  - src/cli/run.rs
  - src/db/migrate_backfill.rs
  - src/db/mod.rs
  - src/db/queries.rs
  - src/scheduler/cmd.rs
  - src/scheduler/log_pipeline.rs
  - src/scheduler/mod.rs
  - src/scheduler/run.rs
  - src/web/handlers/api.rs
  - src/web/handlers/job_detail.rs
  - src/web/handlers/run_detail.rs
  - src/web/handlers/sse.rs
  - templates/pages/run_detail.html
  - templates/partials/run_history.html
  - templates/partials/static_log_viewer.html
  - tests/api_run_now.rs
  - tests/common/mod.rs
  - tests/common/v11_fixtures.rs
  - tests/docker_orphan_guard.rs
  - tests/schema_parity.rs
  - tests/v11_log_dedupe_benchmark.rs
  - tests/v11_log_dedupe_contract.rs
  - tests/v11_log_id_plumbing.rs
  - tests/v11_run_detail_page_load.rs
  - tests/v11_run_now_sync_insert.rs
  - tests/v11_runnum_counter.rs
  - tests/v11_runnum_migration.rs
  - tests/v11_sse_log_stream.rs
  - tests/v11_sse_terminal_event.rs
  - tests/v11_startup_assertion.rs
findings:
  critical: 0
  warning: 3
  info: 5
  total: 8
status: issues_found
---

# Phase 11: Code Review Report

**Reviewed:** 2026-04-16T12:00:00Z
**Depth:** standard
**Files Reviewed:** 35
**Status:** issues_found

## Summary

Phase 11 adds per-job run numbering, a log-id plumbing / dedupe contract, an SSE
terminal-event sentinel, and a sync-insert fix for the Run Now race. The change
set is well-scoped, carefully commented, and thoroughly covered by integration
tests (Postgres parity is documented via `#[cfg(feature = "integration")]`
gates, though Postgres coverage for the counter race, backfill resume, and file
3 NOT NULL does not run without that cargo feature).

Overall code quality is high: SQL is parameterized, locked decisions are cited
inline, and the two-pass migration orchestrator is correctly structured to
handle both fresh installs and upgrade-in-place. No critical or security issues
were found.

Three warnings concern (1) a risk that a partial-tx SQLite writer crash could
advance `jobs.next_run_number` without inserting the run row, (2) silent
swallowing of introspection errors in `file3_can_apply_now`, and (3) a cleanup
gap where the sync-inserted `job_runs` row is NOT finalized if the scheduler
task panics before `continue_run` reaches its `finalize_run` call. Info items
flag a stale Rust-stabilization comment, a potential Postgres `ORDER BY … LIMIT`
portability concern, a logical-OR vs logical-AND subtle readability issue in a
template, a test-harness design note about `#[cfg(feature = "integration")]`,
and a minor `DATABASE_URL` logging concern.

## Warnings

### WR-01: `jobs.next_run_number` counter can advance without inserting a run row on a partial SQLite writer crash

**File:** `src/db/queries.rs:298-351`
**Issue:** `insert_running_run` uses a two-statement transaction: first
`UPDATE jobs SET next_run_number = next_run_number + 1 RETURNING next_run_number - 1`,
then `INSERT INTO job_runs (..., job_run_number)`. If the process crashes (or
the writer connection dies) between the two statements **after** the sqlx
transaction has committed the UPDATE but before the INSERT hits disk, the
counter will be permanently incremented while no row is inserted — producing a
gap in `job_run_number` for that job.

SQLx's `tx.commit()` runs at line 323 AFTER both statements, so under normal
operation this is safe (both persist atomically). However, if the writer
connection is severed mid-transaction (for example: OS-level SIGKILL on the
process while the UPDATE has been flushed to the WAL but before the INSERT has
been written), SQLite's WAL recovery may commit the transaction partially.
Similar holds for Postgres though less likely given PG's stricter WAL
semantics.

The test `concurrent_inserts_distinct_numbers` (tests/v11_runnum_counter.rs:69)
proves the transaction is sound on the happy path, but does not exercise
crash-mid-tx.

**Fix:** This is unlikely to cause a correctness bug in practice because the
two statements are inside a single `tx.begin()/tx.commit()` pair, so the
partial-commit scenario is genuinely rare. However, add a comment documenting
the rare-but-possible gap behavior so operators who see a missing `#N` in the
UI know it's not a data-integrity issue. Consider:

```rust
// NOTE: under abnormal termination mid-transaction (e.g., SIGKILL or disk
// failure between the UPDATE and the INSERT), SQLite WAL recovery may roll
// back the UPDATE with the INSERT, leaving the counter and the row set
// consistent. If the tx is committed partially (pathological but possible
// with torn writes), a gap appears in per-job run numbers. This is cosmetic,
// not a data-integrity bug — the UNIQUE (job_id, job_run_number) index
// guarantees no duplicates can occur.
```

Alternatively, move the counter UPDATE to happen INSIDE the INSERT via a
single `INSERT … SELECT next_run_number FROM jobs WHERE id = ?1 RETURNING
job_run_number` then `UPDATE jobs` in a second step — but that's a more
invasive change and the current approach is already covered by the UNIQUE
index invariant.

### WR-02: `file3_can_apply_now` silently swallows errors via `unwrap_or` fallbacks

**File:** `src/db/mod.rs:163-226`
**Issue:** Every introspection query uses `.unwrap_or(0)` / `.unwrap_or(false)`
to treat query errors as "safe/empty". This means:

1. If the SQLite `sqlite_master` query fails transiently (e.g., pool
   exhaustion, busy timeout), `table_exists` defaults to `false`, causing the
   migrator to claim file 3 is safe to apply when it may not be.
2. If the Postgres `information_schema.tables` query fails (e.g., connection
   reset), the same unsafe default fires.
3. The outer `file3_can_apply_now().await.unwrap_or(false)` at line 123 wraps
   ALL failures into the "two-pass" branch, which is the safer default — but
   the inner `unwrap_or(0)` at line 171, 183, 199, 215 can produce the WRONG
   answer (true) on error, overriding the outer safe-fallback.

Specifically: if `sqlite_master` query errors, `table_exists = false` →
`return Ok(true)` (line 185), meaning file 3 is applied directly, which will
fail on an upgrade-in-place DB with NULL rows.

**Fix:** Propagate errors with `?` instead of swallowing them. If introspection
fails, the orchestrator should fail-fast rather than guess:

```rust
let n: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM sqlite_master \
     WHERE type = 'table' AND name = 'job_runs'",
)
.fetch_one(read)
.await?;  // <-- propagate error rather than unwrap_or(0)
n > 0
```

At minimum, the outer call site at line 123 should be updated to
`file3_can_apply_now().await?` so the error surfaces, instead of silently
picking the two-pass branch (which is the safer default but masks a real DB
problem).

### WR-03: Pre-inserted `job_runs` row is not cleaned up if scheduler task panics before `continue_run` reaches `finalize_run`

**File:** `src/web/handlers/api.rs:59-124`, `src/scheduler/run.rs:122-139`
**Issue:** The new UI-19 sync-insert path works like this:
1. API handler inserts `job_runs` row with `status='running'`.
2. API handler dispatches `SchedulerCmd::RunNowWithRunId { job_id, run_id }`.
3. Scheduler task calls `run_job_with_existing_run_id` → `continue_run`.
4. `continue_run` executes the job and calls `finalize_run` at line 324-341.

The handler correctly handles the "channel closed" case (line 98-123) by
finalizing the row as error. The scheduler arm also handles the "unknown
job_id" case (line 247-256) by finalizing orphans. However, there is NO
handler for the case where the scheduler task ITSELF panics between receiving
the command and reaching `finalize_run`. The parent scheduler loop catches the
panic in `join_set.join_next()` (mod.rs:175-183) with a tracing::error, but
does not finalize the orphaned `job_runs` row — it remains `status='running'`
forever until `mark_run_orphaned` runs at next startup.

The `docker_orphan::reconcile_orphans` call at startup (src/cli/run.rs:216)
will eventually mark it as `orphaned at restart` but only on the next restart
— so a panicked run leaves a row stuck in `running` until restart.

**Fix:** Wrap `continue_run` in a `tokio::spawn`-caught panic guard that
finalizes the row as error on unwind. Or, in the scheduler's `join_set.join_next()`
arm (mod.rs:167-184), when `result` is an `Err(e)`, look up the run_id from
the join error context and finalize the row:

```rust
Err(e) => {
    tracing::error!(
        target: "cronduit.scheduler",
        error = %e,
        "run task panicked"
    );
    // TODO: we currently lose the run_id on panic — consider wrapping
    // continue_run in a panic hook that finalizes the row before re-raising.
}
```

Note: on SchedulerLoop shutdown (mod.rs:496-519), aborted tasks via
`join_set.abort_all()` also leave pre-inserted rows in `running` state. These
are recovered by `mark_run_orphaned` on next startup, which is arguably
acceptable — but document the invariant.

## Info

### IN-01: Stale comment about `i64::div_ceil` stabilization

**File:** `src/db/migrate_backfill.rs:67-68`
**Issue:** The comment reads `"i64::div_ceil is still unstable on stable Rust 1.94."`
— `i64::div_ceil` was actually stabilized for primitive signed integer types
in Rust 1.79 (June 2024). On Rust 1.85+ (edition 2024, per project
Cargo.toml), it is available and would simplify the expression.
**Fix:** Replace the manual ceiling divide with the stdlib method and drop the
comment:

```rust
let batches_est = total.div_ceil(BATCH_SIZE);
```

### IN-02: SQLite `DELETE … LIMIT` requires a compile-time flag; `UPDATE … LIMIT` does not exist in standard SQLite

**File:** `src/db/queries.rs:920-950` (delete_old_logs_batch, delete_old_runs_batch)
**Issue:** The retention deletion queries use `DELETE FROM job_logs WHERE rowid IN (SELECT … LIMIT ?2)`
which relies on `SQLITE_ENABLE_UPDATE_DELETE_LIMIT`. This is not compiled in
by default on the `libsqlite3-sys` shipped by sqlx. The current code uses the
safer sub-SELECT form (`WHERE rowid IN (SELECT …)`), which works universally
— so this is not a bug; just flagging it for awareness since the pattern
looks similar to the disallowed `DELETE … LIMIT`.

Note: This is pre-existing code, not Phase 11 new code. Included here only
because Phase 11 touches `queries.rs` extensively.

**Fix:** No action needed. Consider adding a short comment noting "uses
sub-SELECT, not DELETE … LIMIT, for SQLite portability".

### IN-03: `format!(r#"data: {{"run_id": {}}}"#, ...)` hand-crafts JSON

**File:** `src/scheduler/run.rs:373-378`, `src/web/handlers/sse.rs:65`
**Issue:** The `__run_finished__` sentinel payload is built via string
formatting: `format!(r#"{{"run_id": {}}}"#, line.line)`. Since `line.line` is
the run_id rendered as a decimal string (from `run_id.to_string()` in run.rs:376),
this is safe — run_id is an `i64`, which cannot produce JSON-breaking chars.
However, future extensions (adding a `job_name` field, etc.) would make this
injection-prone.
**Fix:** Optional improvement; use `serde_json::json!`:

```rust
let data = serde_json::json!({"run_id": line.line.parse::<i64>().unwrap_or(0)}).to_string();
yield Ok(Event::default().event("run_finished").data(data));
```

Not urgent; current code is safe because of the `run_id: i64` invariant.

### IN-04: Integration-gated Postgres tests do not run in the default CI matrix

**File:** `tests/docker_orphan_guard.rs:284-375`, `tests/v11_runnum_migration.rs:479-538`
**Issue:** Both files contain `#[cfg(feature = "integration")]` blocks that
never execute because the workspace's Cargo.toml does not declare an
`integration` feature. The comments at docker_orphan_guard.rs:22-26 and
v11_runnum_migration.rs:479 explicitly acknowledge this. The result is that
Phase 11's Postgres coverage for:

- `mark_run_orphaned` SKIP-terminal-statuses guard
- file 3 NOT NULL constraint on Postgres
- counter-race and backfill-resume semantics on Postgres

… is currently not exercised in CI despite Postgres being a LOCKED supported
backend. `tests/schema_parity.rs` does run a real Postgres testcontainer
(line 236-244), so schema structure parity is verified. But behavioral parity
is not.

**Fix:** Either (a) add an `integration` feature to `Cargo.toml` and a CI job
that runs `cargo nextest run --features integration`, or (b) port the
Postgres tests to use an unconditional testcontainer (like
`tests/schema_parity.rs` already does). Option (b) is more work but matches
the "Postgres-parity gate on every CI run" constraint in CLAUDE.md.

### IN-05: `strip_db_credentials` elides the path for non-URL-shaped strings

**File:** `src/db/mod.rs:353-361`, called from `src/cli/run.rs:124`
**Issue:** `strip_db_credentials("sqlite:memory:")` returns `"sqlite:memory:"`
unchanged (correct), but `strip_db_credentials("some invalid string")`
returns `"<unparseable>"`. In the startup-log emission at cli/run.rs:124,
this means a typo'd or malformed `DATABASE_URL` silently logs
`"<unparseable>"` rather than the actual value, which could make
troubleshooting harder. Not a security issue — on the contrary, it's more
secure because a malformed secret won't leak.
**Fix:** Consider logging a DEBUG or TRACE-level hint when `strip_db_credentials`
falls back to `<unparseable>`, so an operator can correlate a config-parse
failure with what they pasted into the config file. Not urgent.

---

_Reviewed: 2026-04-16T12:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
