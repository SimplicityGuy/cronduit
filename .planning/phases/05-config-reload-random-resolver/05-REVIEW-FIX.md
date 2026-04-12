---
phase: 05-config-reload-random-resolver
fixed_at: 2026-04-11T00:00:00Z
review_path: .planning/phases/05-config-reload-random-resolver/05-REVIEW.md
iteration: 1
findings_in_scope: 5
fixed: 5
skipped: 0
status: all_fixed
---

# Phase 05: Code Review Fix Report

**Fixed at:** 2026-04-11
**Source review:** .planning/phases/05-config-reload-random-resolver/05-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 5
- Fixed: 5
- Skipped: 0

## Fixed Issues

### WR-01: `sync_config_to_db` fetches each job from DB twice per sync cycle

**Files modified:** `src/scheduler/sync.rs`
**Commit:** e90f9c3
**Applied fix:** Added a `existing_cache` HashMap that stores the DB lookup results from the first pass (batch-input building loop). The second loop now reads from the cache via `existing_cache.remove()` instead of issuing a second `get_job_by_name` query per job. This eliminates N redundant DB round-trips per sync cycle and removes the theoretical race window between the two reads.

### WR-02: `do_reroll` silently skips in-memory update when job is not in the `jobs` map

**Files modified:** `src/scheduler/reload.rs`
**Commit:** e32d339
**Applied fix:** Added an `else` branch to the `if let Some(mem_job) = jobs.get_mut(&job_id)` check that logs a `tracing::warn` when the in-memory update is skipped. The warning includes the `job_id` and describes that the DB was updated but the scheduler will use the stale schedule until the next full reload.

### WR-03: File watcher drops the `ReloadResult` response entirely

**Files modified:** `src/scheduler/reload.rs`
**Commit:** 171a681
**Applied fix:** Changed the file watcher to await the oneshot response channel after sending the reload command. On error status, logs a `tracing::warn` with the error message. On channel drop, logs a `tracing::debug`. This makes file-watch reload outcomes fully observable in tracing output.

### WR-04: `resolve_schedule` silently falls through to an unvalidated "best effort" on the final retry

**Files modified:** `src/scheduler/random.rs`
**Commit:** 4cb5ed9
**Applied fix:** Computed the final `resolve_fields` result into a `last_attempt` variable and included it as the `resolved` field in the existing `tracing::warn` log line. This makes the actual unvalidated value visible in tracing output for diagnosis when the fallback path is exercised.

### WR-05: `check_schedule` uses `"0"` as universal @random stand-in, invalid for day-of-month and month fields

**Files modified:** `src/config/validate.rs`
**Commit:** 0a21c30
**Applied fix:** Replaced the universal `"0"` substitution with a `RANDOM_FALLBACKS` array containing field-specific valid values: `["0", "0", "1", "1", "0"]` (minute=0, hour=0, dom=1, month=1, dow=0). The `.enumerate()` iterator now indexes into the fallback array per field position, with a safe `.get().unwrap_or("0")` for out-of-bounds protection.

## Skipped Issues

None -- all in-scope findings were fixed.

---

_Fixed: 2026-04-11_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
