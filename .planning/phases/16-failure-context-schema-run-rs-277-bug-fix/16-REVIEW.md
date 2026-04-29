---
phase: 16-failure-context-schema-run-rs-277-bug-fix
reviewed: 2026-04-27T00:00:00Z
depth: standard
files_reviewed: 28
files_reviewed_list:
  - justfile
  - migrations/postgres/20260427_000005_image_digest_add.up.sql
  - migrations/postgres/20260428_000006_config_hash_add.up.sql
  - migrations/postgres/20260429_000007_config_hash_backfill.up.sql
  - migrations/sqlite/20260427_000005_image_digest_add.up.sql
  - migrations/sqlite/20260428_000006_config_hash_add.up.sql
  - migrations/sqlite/20260429_000007_config_hash_backfill.up.sql
  - src/db/queries.rs
  - src/scheduler/docker.rs
  - src/scheduler/mod.rs
  - src/scheduler/run.rs
  - src/web/handlers/api.rs
  - tests/common/v11_fixtures.rs
  - tests/dashboard_render.rs
  - tests/docker_executor.rs
  - tests/job_detail_partial.rs
  - tests/jobs_api.rs
  - tests/reload_inflight.rs
  - tests/stop_handler.rs
  - tests/stop_race.rs
  - tests/v11_runnum_counter.rs
  - tests/v11_startup_assertion.rs
  - tests/v12_fctx_config_hash_backfill.rs
  - tests/v12_fctx_explain.rs
  - tests/v12_fctx_streak.rs
  - tests/v12_run_rs_277_bug_fix.rs
  - tests/v13_sparkline_render.rs
  - tests/v13_timeline_render.rs
findings:
  blocker: 0
  warning: 4
  info: 5
  total: 9
status: issues_found
---

# Phase 16: Code Review Report

**Reviewed:** 2026-04-27T00:00:00Z
**Depth:** standard
**Files Reviewed:** 28
**Status:** issues_found

## Summary

Phase 16's core deliverables — the `run.rs:301` bug fix (container_id_for_finalize now correctly receives `docker_result.container_id` instead of `image_digest`), the per-run `image_digest` and `config_hash` schema columns, the FCTX-07 `get_failure_context` helper, and the bulk `config_hash` backfill — are correctly implemented at the SQL and Rust call-site level. Migration files are paired SQLite/Postgres, idempotency is preserved per backend, the `LEFT JOIN ... ON 1=1` CTE shape and epoch sentinel match D-05, and the `BACKFILL_CUTOFF_RFC3339` marker is present in both backfill files.

However, four real defects warrant fixing before this code ships:

1. The inspect-failure path in `src/scheduler/docker.rs` writes an empty string into `job_runs.image_digest` instead of NULL, breaking the "captured vs uncaptured" binary the schema design depends on.
2. The bug-fix regression test that proves `container_id` is no longer the digest is `#[ignore]`-gated behind a real Docker daemon, so a future regression of `run.rs:305` would silently pass standard CI.
3. The `BACKFILL_CUTOFF_RFC3339` marker is set to today's date (`2026-04-27T00:00:00Z`); rows that ended after midnight UTC today but before the migration ran are mis-classified by Phase 21's heuristic.
4. The backfill comment block in `migrations/*/20260429_000007_config_hash_backfill.up.sql` describes a heuristic (`end_time < BACKFILL_CUTOFF`) that the SQL does not actually implement — the SQL uses `WHERE config_hash IS NULL`, with no end_time filter.

Plus minor info-level items (stale comment in `tests/docker_executor.rs`, fragile test seed format, etc.).

The TLS posture is preserved (no `openssl-sys` introductions) and the clippy-too-many-arguments allowance on `finalize_run` (8 args) is justified and acceptable. No SQL injection or auth bypass concerns. No security findings.

## Warnings

### WR-01: Inspect-failure path stores empty-string image_digest, not NULL

**File:** `src/scheduler/docker.rs:253-264, 428`
**Issue:** When `docker.inspect_container()` fails OR returns `info.image == None`, the local `image_digest` is set to `String::new()` and then wrapped at line 428 as `image_digest: Some(image_digest)` — i.e. `Some("")`. This empty-string value flows through `run.rs:306` into `finalize_run`'s `image_digest: Option<&str>` parameter as `Some("")`, which sqlx binds as the literal empty string in the DB. The schema design (and the FCTX-07 query) relies on the binary distinction "image_digest IS NULL" (no digest captured) vs. "image_digest LIKE 'sha256:%'" (digest captured). An empty string is neither, and downstream consumers that filter on `image_digest IS NOT NULL` will treat the empty-string row as "captured" when in fact the digest is missing.

The test `digest_persists_across_inspect_failure` in `tests/v12_run_rs_277_bug_fix.rs:303` simulates the path by manually passing `None` to `finalize_run`, so it does NOT catch the actual production behavior produced by `execute_docker`.

**Fix:**
```rust
// src/scheduler/docker.rs L253-264 — return Option<String>, not String:
let image_digest: Option<String> = match docker.inspect_container(&container_id, None).await {
    Ok(info) => info.image.filter(|s| !s.is_empty()),
    Err(e) => {
        tracing::warn!(
            target: "cronduit.docker",
            container_id = %container_id,
            error = %e,
            "failed to inspect container for image digest"
        );
        None
    }
};

// L428 — pass through directly, no double-Some:
DockerExecResult {
    exec: exec_result,
    image_digest, // already Option<String>, drop the Some(image_digest) wrapper
    container_id: Some(container_id.clone()),
}
```
And update `DockerExecResult.image_digest` type / call sites accordingly. Add a unit test that constructs a `DockerExecResult` simulating inspect failure and asserts the final `job_runs.image_digest` column is NULL (not empty string).

### WR-02: The load-bearing bug-fix regression test is gated behind `#[ignore]`

**File:** `tests/v12_run_rs_277_bug_fix.rs:76-160`
**Issue:** The two tests that actually prove the `run.rs:305` wiring fix — `docker_run_writes_real_container_id_not_digest` and `docker_run_writes_image_digest_as_sha256` — are `#[ignore]` and require a running Docker daemon. The two non-Docker tests (`command_run_leaves_image_digest_null`, `digest_persists_across_inspect_failure`) call `finalize_run` directly with `None` arguments and do NOT exercise the `container_id_for_finalize = docker_result.container_id` assignment in `run.rs:305-306` at all.

A future PR that mistakenly swaps the two locals back (or copy-pastes the wrong one), reintroducing FOUND-14, would pass `cargo test`, `cargo clippy`, `nextest`, and the entire `just ci` chain. The regression would only surface on a developer's local Docker run or in production. Phase 16's own context flags this as the load-bearing concern; the test coverage does not match.

**Fix:** Add a unit-level test in `src/scheduler/run.rs` (or a new `tests/v12_run_wiring.rs`) that exercises the `continue_run` docker-arm wiring without requiring a real Docker daemon. Approach: extract a small pure-Rust helper that takes a `DockerExecResult` and returns the two `Option<String>` finalize args, then unit-test that helper. This locks the wiring at the function signature level so a future swap fails to compile or produces a clear unit-test failure. Alternatively, mock the `Docker` client at the module boundary — even a thin trait that returns a synthetic `DockerExecResult` would lock the wiring.

### WR-03: BACKFILL_CUTOFF_RFC3339 marker is set to today, mis-classifying same-day rows

**File:** `migrations/sqlite/20260429_000007_config_hash_backfill.up.sql:3`, `migrations/postgres/20260429_000007_config_hash_backfill.up.sql:3`
**Issue:** The marker is `2026-04-27T00:00:00Z` — exactly the start of today (matching CLAUDE.md `currentDate`). Per the comment block, Phase 21's UI panel uses this convention to identify backfilled rows: rows with `end_time < BACKFILL_CUTOFF AND config_hash IS NOT NULL` are presumed backfilled.

But the migration itself runs at some moment T > the cutoff (since it's "today midnight UTC"). Any run that ended between `2026-04-27T00:00:00Z` and the moment the migration actually executes (likely several hours later, given typical deploy timing) will:
1. Have its NULL `config_hash` populated by this migration's UPDATE (because `WHERE config_hash IS NULL` matches).
2. NOT be classified as backfilled by Phase 21's heuristic (because `end_time` is on or after the cutoff).

Result: a window of "true authentic captures" mixed with "actually backfilled but Phase 21 thinks it's authentic" rows for any `2026-04-27` v1.1 deployment.

In v1.1 (which lacked the column entirely), this is likely zero-impact because no rows had been written via `insert_running_run` with `config_hash` yet — the first such row exists only after Phase 16 ships. But the documentation invariant is leaky.

**Fix:** Pick a cutoff strictly greater than any possible v1.1 finish time — e.g., the migration's deploy date + 1 day, or a timestamp that's clearly "after everyone has rolled out." For example:
```sql
-- BACKFILL_CUTOFF_RFC3339: 2026-04-28T00:00:00Z
```
Or document explicitly that the marker is "lower-bound, may yield false negatives near the cutoff hour." Cross-reference Phase 21's plan to confirm the heuristic tolerates this.

### WR-04: Backfill migration comment describes a heuristic the SQL does not implement

**File:** `migrations/sqlite/20260429_000007_config_hash_backfill.up.sql:13-17`, `migrations/postgres/20260429_000007_config_hash_backfill.up.sql:13-17`
**Issue:** The comment says: "Heuristic: rows where `end_time < BACKFILL_CUTOFF_RFC3339` AND `config_hash IS NOT NULL` AFTER this migration are backfilled (semantically suspect ...)". A reader of the migration file naturally expects the SQL to implement this `end_time < CUTOFF` filter as part of the UPDATE. The actual SQL is:

```sql
UPDATE job_runs
   SET config_hash = (SELECT j.config_hash FROM jobs j WHERE j.id = job_runs.job_id)
 WHERE config_hash IS NULL;
```

There is no `end_time < CUTOFF` predicate. The marker is purely a forward-looking documentation token for Phase 21's UI to interpret POST-migration; it is not gating WHICH rows the migration touches. This is unintuitive and bug-prone — the comment block reads like a SQL spec but is actually a cross-phase contract.

**Fix:** Reword the comment to make the contract explicit and untangle "what the SQL does" from "what Phase 21 reads":
```sql
-- BACKFILL_CUTOFF_RFC3339: 2026-04-28T00:00:00Z
-- (Marker per D-03; Phase 21 reads this comment to identify backfilled rows.
--  Phase 21 heuristic: a row is "presumed backfilled" iff
--    end_time < BACKFILL_CUTOFF_RFC3339 AND config_hash IS NOT NULL.
--  This SQL does NOT filter on end_time — it backfills every row where
--  config_hash IS NULL. The cutoff is purely a UI-side identifier.)
```

## Info

### IN-01: Stale comment in `tests/docker_executor.rs` references the now-fixed bug

**File:** `tests/docker_executor.rs:113-115`
**Issue:** The comment reads: "image_digest field holds the image digest, not the actual container ID, so we verify cleanup indirectly: a second run with the same name should work." This is describing the v1.0/v1.1 bug that Phase 16 fixed. Now that `DockerExecResult` has both `image_digest` AND `container_id`, the comment is obsolete and misleading to a future reader.

**Fix:** Replace with: "Container removal is asserted indirectly by `delete = true` semantics — a second run of the same job with the same `container_name` must succeed, which requires the prior container to have been removed. Direct removal assertion is in `test_docker_orphan_reconciliation` below."

### IN-02: `tests/v12_fctx_streak.rs` seed format breaks for `time_index >= 60`

**File:** `tests/v12_fctx_streak.rs:67-84`
**Issue:** The fixture format string is `format!("2026-04-27T00:{:02}:00Z", time_index)`. For `time_index >= 60`, this yields `"2026-04-27T00:60:00Z"`, `"2026-04-27T00:61:00Z"`, etc. — invalid RFC3339 (minutes ≤ 59). Currently the tests only seed up to time_index=6, so this is dormant, but the fixture is fragile if someone increases the test scenario size.

**Fix:** Use a wider clock or compute the timestamp from a base + offset:
```rust
let base = chrono::DateTime::parse_from_rfc3339("2026-04-27T00:00:00Z").unwrap();
let start_time = (base + chrono::Duration::seconds(time_index)).to_rfc3339();
```

### IN-03: `_image_digest` discarded value at `src/scheduler/docker.rs:139`

**File:** `src/scheduler/docker.rs:139`
**Issue:** `let _image_digest = match super::docker_pull::ensure_image(docker, &config.image).await { Ok(digest) => digest, ... }` — the digest from `ensure_image` is captured then discarded (the leading underscore suppresses the unused-variable warning). The actual image digest used downstream is computed from `inspect_container` post-start (line 253). The intent is clear (different lifecycle stage; post-start is the authoritative one) but a future reader will wonder why two digest extractions exist. The bound name `_image_digest` also suggests "this is a real digest I'm keeping" rather than "this is a side-effect-only call."

**Fix:** Replace the binding with `let _ = ...` to make the discard explicit, and add a one-line comment:
```rust
// Pull the image if missing. The returned digest is intentionally discarded —
// we re-extract via `inspect_container` post-start so the recorded digest
// reflects what the container actually ran with (DOCKER-09).
let _ = super::docker_pull::ensure_image(docker, &config.image).await
    .map_err(|e| { ... })?;  // or keep the existing match form with `Ok(_) =>`.
```

### IN-04: `FailureContext` carries `#[allow(dead_code)]` on every field

**File:** `src/db/queries.rs:636-657`
**Issue:** Every field of `FailureContext` and the `get_failure_context` function itself carries `#[allow(dead_code)]` with a "Phase 18+ consumes" comment. This is honest forward-engineering, but it does mean the field-level data flow is uncovered until Phase 18 lands. If the `LEFT JOIN ... ON 1=1` somehow returns multi-row results in some unforeseen edge case (it shouldn't — both CTEs are aggregations or LIMIT 1), the bug would surface only when Phase 18 starts dereferencing the values.

**Fix:** Acceptable as-is; flagging for awareness. Optionally add one assertion to `get_failure_context` that `streak.consecutive_failures` is non-negative and `last_success_run_id` is `None` iff `last_success_image_digest.is_none() && last_success_config_hash.is_none()` — this turns "Phase 18+ consumes" into "Phase 16 also asserts internal consistency." Cheap insurance.

### IN-05: `DbRun` and `DbRunDetail` carry duplicate field documentation

**File:** `src/db/queries.rs:577-623`
**Issue:** `DbRun.image_digest`, `DbRun.config_hash`, `DbRunDetail.image_digest`, `DbRunDetail.config_hash` all share verbatim doc comments referencing the BACKFILL_CUTOFF_RFC3339 marker. The two structs differ only in whether `job_name` (joined from `jobs`) is included; the column-level docs are duplicated.

**Fix:** Acceptable as-is — the two structs are intentionally distinct DTOs and the field docs match the SQL. If you ever consolidate via a shared trait or `From` impl, dedupe then. Pure style note.

---

_Reviewed: 2026-04-27T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
