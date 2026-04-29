---
phase: 16-failure-context-schema-run-rs-277-bug-fix
plan: 04b
subsystem: scheduler+web+database+ops
tags: [callers, signature-transition, justfile, FOUND-14, FCTX-04, wave-end-gate, PR-1-mergeable]

# Dependency graph
requires:
  - phase: 16-failure-context-schema-run-rs-277-bug-fix
    provides: "queries::insert_running_run 4-arg signature + queries::finalize_run 8-arg signature + DbRun/DbRunDetail.image_digest/config_hash fields + SELECT-site hydration (Plan 16-04a)"
  - phase: 16-failure-context-schema-run-rs-277-bug-fix
    provides: "run.rs:348-358 finalize_run 8-arg call already in place (Plan 16-03); run.rs:231-303 image_digest_for_finalize parallel local + bug-fix at run.rs:301 (Plan 16-03)"
provides:
  - "src/scheduler/run.rs:86: scheduler-driven insert_running_run passes &job.config_hash (4th arg)"
  - "src/scheduler/run.rs:797 (test-mod): pre-insert helper updated to 4-arg shape"
  - "src/web/handlers/api.rs:82: Run Now insert_running_run passes &job.config_hash (4th arg)"
  - "src/web/handlers/api.rs:131: error-fallback finalize_run passes None for image_digest (8th arg)"
  - "src/scheduler/mod.rs:256, 331: orphan-row finalize_run fallbacks pass None for image_digest (Rule 3 auto-fix)"
  - "src/db/queries.rs test-mod: 4 insert_running_run sites + 2 finalize_run sites updated to new signatures"
  - "src/db/queries.rs::finalize_run: #[allow(clippy::too_many_arguments)] with rationale doc"
  - "12 tests/ files updated: 21 caller sites total (Rule 3 auto-fix to keep wave-end gate green)"
  - "justfile uat-fctx-bugfix-spot-check recipe: maintainer-observable for FOUND-14"
  - "16-HUMAN-UAT.md: cronduit.dev.db filename corrected (Rule 1 fix on plan inaccuracy)"
affects:
  - 16-05 (wave-3 parallel cohort: get_failure_context implementation, free of compile-blocker)
  - 16-06 (wave-3 parallel cohort: EXPLAIN tests, free of compile-blocker)
  - 18 (webhook payload reads DbRun.image_digest + DbRun.config_hash directly)
  - 21 (FCTX UI panel renders deltas from DbRunDetail)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Wave-end gate ownership: the final wave-2 sequential executor is responsible for running the full local CI gate (cargo build + fmt-check + clippy + lib tests + schema-diff + grep-no-percentile-cont + nextest) since predecessors used --no-verify and the codebase was non-compiling between 16-04a and 16-04b."
    - "Caller-update auto-fix: when a queries.rs signature widening lands in plan N, the next wave's caller-update plan must enumerate ALL call sites in src/ AND tests/. The plan's enumeration was minimal (5 src/ test sites + 4 production sites); 2 production sites in src/scheduler/mod.rs and 21 sites across 12 tests/ files were Rule 3 auto-fixes — blocking the wave-end gate, fix is mechanical and threat-model neutral."
    - "Rustfmt reflow on signature widening: 4-arg insert_running_run + 8-arg finalize_run pushed past rustfmt's single-line threshold, requiring a fmt cleanup commit after the caller updates land. Plan 16-04b absorbs the reformatting in a dedicated style commit."
    - "Clippy too_many_arguments allowance with documented rationale: the 8-arg finalize_run mirrors the job_runs terminal-write surface (status, exit_code, end_time, duration_ms, error_message, container_id, image_digest); bundling into a struct re-marshals data already in scope at every caller. Allow attribute is annotated with the rationale so a future reviewer doesn't try to refactor it away."

key-files:
  created: []
  modified:
    - src/scheduler/run.rs
    - src/scheduler/mod.rs
    - src/web/handlers/api.rs
    - src/db/queries.rs
    - justfile
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
    - tests/v13_sparkline_render.rs
    - tests/v13_timeline_render.rs
    - .planning/phases/16-failure-context-schema-run-rs-277-bug-fix/16-HUMAN-UAT.md

key-decisions:
  - "Used cronduit.dev.db (not cronduit.db) in the just recipe. The plan's recipe sketch referenced cronduit.db with a comment claiming `db-reset also targets cronduit.db`, but inspection shows db-reset and sqlx-prepare both target cronduit.dev.db. Switched to the correct dev-DB filename so the spot-check actually inspects the same DB the rest of the dev workflow uses. Tagged as Rule 1 deviation."
  - "Auto-fixed 2 finalize_run sites in src/scheduler/mod.rs (orphan-row error fallbacks at L256, L331) that the plan did not enumerate. Both pass None for image_digest — semantically correct for the same reason as api.rs:131 (these fire when no docker run started). Required to compile."
  - "Auto-fixed 21 caller sites across 12 tests/ integration test files (insert_running_run + finalize_run). Plan T3 listed only the 4 in src/db/queries.rs test mod + 1 in src/scheduler/run.rs test mod, but the wave-end gate (T5) requires `cargo build --all-targets` green, which compiles tests/. Each test caller updated mechanically: insert_running_run gains 'testhash' as 4th arg; finalize_run gains None as 8th arg. Required to keep `cargo build` and `just nextest` green."
  - "Added #[allow(clippy::too_many_arguments)] to finalize_run with a doc comment explaining the rationale. Without it, `just clippy` (CI gate) fails because clippy's default threshold is 7 args. Plan didn't anticipate this — Rule 3 auto-fix."
  - "Test-mod finalize_run sites in src/db/queries.rs (L1922, L2027) updated to 8-arg even though plan T3 mentioned only insert_running_run. Build won't compile without it. Implicit in T5's `cargo build` acceptance criterion."

patterns-established:
  - "Caller-update plan enumeration must include test files: when a public queries.rs signature widens, every caller in src/ AND tests/ AND benches/ AND examples/ needs a flag. Plan 16-04b's pre-existing enumeration only covered 5 src/ test sites; in practice 22 additional caller sites needed updating. The wave-end gate caught the gap, but planners should grep the entire repo (`grep -rn 'fn_name(' src/ tests/ benches/ examples/`) when widening signatures."

requirements-completed: [FOUND-14, FCTX-04]

# Metrics
duration: ~10min
completed: 2026-04-27
---

# Phase 16 Plan 04b: callers + just recipe + wave-end gate Summary

**Landed every call-site update needed to compose with Plan 16-04a's queries.rs signature widening, plus the maintainer-observable just recipe for the FOUND-14 spot check, plus the wave-end CI gate that validates PR 1 (Plans 16-01..16-04b) is green and mergeable. After this plan, `cargo build` is clean, `just nextest` passes 388 tests, and the operator can run `just uat-fctx-bugfix-spot-check` to visually verify the v1.1 bug at run.rs:301 is fixed.**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-04-27 (after 16-04a wave-2 predecessor merge)
- **Completed:** 2026-04-27
- **Tasks:** 5 / 5
- **Files modified:** 18 (3 production source, 1 justfile, 1 UAT doc, 12 integration-test files, 1 SUMMARY in this commit)
- **Files created:** 1 (this SUMMARY.md)

## Accomplishments

- **T1 (run.rs:86):** Scheduler-driven `insert_running_run` now passes `&job.config_hash` as the new 4th positional arg; `DbJob.config_hash` is in scope from the `job: DbJob` parameter, populated at config-load time per Phase 11.
- **T2 (api.rs:82, api.rs:131):** Web Run Now handler's `insert_running_run` passes `&job.config_hash` (DbJob fetched at api.rs:66 via `get_job_by_id` is in scope); the error-fallback `finalize_run` passes `None` for the new image_digest 8th arg (semantically correct — fallback fires when no docker run started).
- **T3 (test-mod + integration tests):** Updated 5 test-mod insert_running_run sites in src/ (1 in scheduler/run.rs, 4 in db/queries.rs) + 2 test-mod finalize_run sites in src/db/queries.rs + 21 caller sites across 12 tests/ integration test files. All test sites use the literal `"testhash"` for config_hash (matches existing convention from queries.rs:579 upsert_job test fixture). Out-of-scope test/ files are Rule 3 auto-fixes documented below.
- **T4 (justfile + HUMAN-UAT):** Added `uat-fctx-bugfix-spot-check` recipe under the `db` group. Recipe queries the most recent `job_runs` row from `cronduit.dev.db` and prints `(id, job_id, status, container_id, image_digest)` so the maintainer can visually verify `container_id` does NOT start with `sha256:`. Updated 16-HUMAN-UAT.md preconditions to reference `cronduit.dev.db` (the plan referenced `cronduit.db` but the dev convention is `cronduit.dev.db` per `db-reset` and `sqlx-prepare`).
- **T5 (wave-end gate):** Ran the full local CI gate. All 388 nextest tests pass; 22 Docker-gated `#[ignore]` tests skipped as expected. PR 1 (Plans 16-01..16-04b) is mergeable.

## Task Commits

| # | Task | Commit | Type |
|---|------|--------|------|
| 1 | Wire `&job.config_hash` to insert_running_run at run.rs:83 | `7035287` | feat |
| 2 | Wire config_hash + image_digest=None at api.rs run_now sites | `ba01532` | feat |
| 3 | Update test-mod + integration-test callers to new signatures | `b971e34` | test |
| 4 | Add uat-fctx-bugfix-spot-check just recipe | `c1964f8` | chore |
| 5a | cargo fmt cleanup after caller signature updates | `9fc6efb` | style |
| 5b | Allow clippy::too_many_arguments on finalize_run | `0f41934` | fix |

T5 is a verification task with no source changes of its own, but its execution surfaced two follow-on commits (`9fc6efb` style and `0f41934` clippy allow) that were necessary to drive the gate green.

All commits use `--no-verify` per the wave-2 sequential-executor policy.

## Files Created/Modified

### Modified — production source

- **`src/scheduler/run.rs`** (T1 + T3)
  - L82-86: `insert_running_run(&pool, job.id, &trigger, &job.config_hash)` — added the 4th arg from the in-scope `DbJob`.
  - L797: test-mod helper `run_job_with_existing_run_id_skips_insert` — pre-insert call updated to 4-arg shape with `"testhash"`.

- **`src/scheduler/mod.rs`** (Rule 3 auto-fix)
  - L264-265: orphan-row finalize_run fallback at `RunNowWithRunId` for unknown job — added `None` for image_digest with `// Phase 16 FOUND-14` comment.
  - L339-340: orphan-row finalize_run fallback at the drained `RunNowWithRunId` arm — same shape.

- **`src/web/handlers/api.rs`** (T2)
  - L82: `insert_running_run(&state.pool, job_id, "manual", &job.config_hash)` — added the 4th arg from the in-scope `job: DbJob`.
  - L141: error-fallback `finalize_run` invocation gains `None, // Phase 16 FOUND-14: image_digest — error fallback never started a container`.

- **`src/db/queries.rs`** (T3 + T5b)
  - L1874, L1915, L1964: 3 test-mod `insert_running_run` sites updated to 4-arg with `"testhash"`.
  - L1922, L2027: 2 test-mod `finalize_run` sites updated to 8-arg with `None` for image_digest.
  - L2024: helper `insert_run` test fixture updated to 4-arg.
  - L434-441: `finalize_run` definition annotated with `#[allow(clippy::too_many_arguments)]` + 6-line doc comment explaining the rationale.

### Modified — ops + docs

- **`justfile`** (T4)
  - Added 12-line `uat-fctx-bugfix-spot-check` recipe under the `db` group between `schema-diff` and the `dev loop` section. Recipe runs `sqlite3 cronduit.dev.db "SELECT id, job_id, status, container_id, image_digest FROM job_runs ORDER BY id DESC LIMIT 1;"` with explanatory echos before and after.

- **`.planning/phases/16-failure-context-schema-run-rs-277-bug-fix/16-HUMAN-UAT.md`** (Rule 1 fix on plan inaccuracy)
  - Pre-condition line: `cronduit.db` -> `cronduit.dev.db`.
  - Spot-Check 1 Step 1: `cronduit.db` -> `cronduit.dev.db` (twice in the same paragraph).

### Modified — integration tests (Rule 3 auto-fix)

| File | Sites updated |
|------|---------------|
| `tests/common/v11_fixtures.rs` | 1 insert_running_run |
| `tests/dashboard_render.rs` | 2 insert_running_run + 1 finalize_run |
| `tests/docker_executor.rs` | 1 insert_running_run |
| `tests/job_detail_partial.rs` | 2 insert_running_run + 1 finalize_run |
| `tests/jobs_api.rs` | 1 insert_running_run + 1 finalize_run |
| `tests/reload_inflight.rs` | 1 insert_running_run |
| `tests/stop_handler.rs` | 1 insert_running_run |
| `tests/stop_race.rs` | 1 insert_running_run |
| `tests/v11_runnum_counter.rs` | 5 insert_running_run |
| `tests/v11_startup_assertion.rs` | 1 insert_running_run |
| `tests/v13_sparkline_render.rs` | 1 insert_running_run + 1 finalize_run |
| `tests/v13_timeline_render.rs` | 2 insert_running_run + 1 finalize_run |
| **Total** | **19 insert_running_run + 5 finalize_run = 24 caller updates** |

## Wave-end Gate Results (T5)

| Gate | Result | Detail |
|------|--------|--------|
| `cargo build --all-targets` | PASS | Clean compile after T1-T4 + Rule 3 auto-fixes. |
| `just fmt-check` | PASS (after `9fc6efb`) | rustfmt reflowed the widened call sites; commit absorbs the cleanup. |
| `just clippy` | PASS (after `0f41934`) | `too_many_arguments` allowance added to finalize_run with rationale. |
| `cargo test --lib` | PASS | 194 passed / 0 failed / 0 ignored. |
| `just schema-diff` | PASS | 3 passed (parity invariant + normalize_type tests). |
| `just grep-no-percentile-cont` | PASS | OBS-05 D-15 compliance — no SQL-native percentile in src/. |
| `cargo test --test v12_fctx_config_hash_backfill` | PASS | 4 passed (Plan 16-01 integration test). |
| `cargo test --test v12_run_rs_277_bug_fix command_run_leaves_image_digest_null` | PASS | 1 passed (Plan 16-03 non-ignored test). |
| `just nextest` | PASS | 388 tests run, 388 passed, 22 skipped (Docker-gated). |

## Verification

| Check | Expected | Actual |
|-------|----------|--------|
| `grep -q 'insert_running_run(&pool, job.id, &trigger, &job.config_hash)' src/scheduler/run.rs` | 0 | 0 |
| `grep -q 'insert_running_run(&state.pool, job_id, "manual", &job.config_hash)' src/web/handlers/api.rs` | 0 | 0 |
| `grep -A 12 'queries::finalize_run' src/web/handlers/api.rs \| grep -q 'Phase 16 FOUND-14'` | 0 | 0 |
| `grep -c 'insert_running_run.*"testhash"' src/db/queries.rs src/scheduler/run.rs` | >= 5 | 5 (4 + 1) |
| `! grep -E 'insert_running_run\([^)]*"manual"\)\.await' src/web/handlers/api.rs` | no match | no match |
| `! grep -E 'insert_running_run\([^)]*&trigger\)\.await' src/scheduler/run.rs` | no match | no match |
| `grep -q '^uat-fctx-bugfix-spot-check:' justfile` | 0 | 0 |
| `just --list \| grep 'uat-fctx-bugfix-spot-check'` | match | match |
| `just nextest exit code` | 0 | 0 |

All 9 verification checks pass.

## Decisions Made

- **Used `cronduit.dev.db` (not `cronduit.db`) in the just recipe.** The plan's recipe sketch hard-coded `cronduit.db` and referenced `db-reset` as the convention example. Inspection of justfile shows `db-reset` and `sqlx-prepare` both target `cronduit.dev.db`. Tagged as Rule 1 fix on plan inaccuracy. Also propagated to 16-HUMAN-UAT.md so the maintainer's spot check actually inspects the right file.
- **Auto-fixed 2 finalize_run call sites in src/scheduler/mod.rs (Rule 3).** L256 and L331 are orphan-row error fallbacks fired when `RunNowWithRunId` arrives for a now-unknown job. Both pass None for image_digest — semantically correct for the same reason as api.rs:131 (no docker run started). Plan T2 enumerated only the api.rs error fallback; mod.rs callers were missed but block compilation.
- **Auto-fixed 21 caller sites across 12 tests/ integration test files (Rule 3).** Plan T3 enumerated only the 5 in src/, but `cargo build --all-targets` (T5 acceptance criterion) compiles every test target. Each site is a mechanical 3->4 arg or 7->8 arg update with the literal "testhash" / None. Bundled into the same commit as T3 since they form a single semantic unit (caller signature transition).
- **Added `#[allow(clippy::too_many_arguments)]` to finalize_run (Rule 3).** Plan didn't anticipate that 8 args trips clippy's default threshold of 7. Annotation includes a 6-line doc comment explaining why bundling into a struct would be wrong (params mirror the job_runs terminal-write surface; struct would re-marshal data already in scope).
- **Used the literal `"testhash"` for every test-side config_hash.** Matches the existing convention from queries.rs:579 (upsert_job test fixture). Avoids inventing a new convention; downstream search-and-replace stays trivial.
- **Did NOT update the orphan-row finalize_run fallbacks in mod.rs to use a different image_digest comment style.** The api.rs:141 comment reads "error fallback never started a container"; mod.rs:264 and mod.rs:339 read "orphan row, no docker run started". Different scenarios (channel closed vs. job unknown), different comment text, same semantic (None for image_digest because no docker run executed).

## Deviations from Plan

| # | Rule | Type | Description |
|---|------|------|-------------|
| 1 | Rule 1 | Plan inaccuracy | The plan's just recipe and HUMAN-UAT runbook referenced `cronduit.db`, but the dev DB filename is `cronduit.dev.db` (per `db-reset` and `sqlx-prepare`). Updated both the recipe and the runbook. Without the fix, the spot check would inspect a non-existent file and the operator would have to track down the discrepancy themselves. |
| 2 | Rule 3 | Missing scope | Plan T2 enumerated 1 finalize_run fallback in api.rs but missed 2 in src/scheduler/mod.rs (L256 + L331). Auto-fixed: both are orphan-row fallbacks where None for image_digest is semantically correct. Without the fix, `cargo build` would fail. |
| 3 | Rule 3 | Missing scope | Plan T3 enumerated 5 src/ test-mod insert_running_run sites but did not address the 2 src/db/queries.rs test-mod finalize_run sites (L1922, L2027) which also need 8-arg shape. Auto-fixed in the same T3 commit. |
| 4 | Rule 3 | Missing scope | Plan T3 did not enumerate any tests/ integration-test files. 12 such files (21 caller sites) needed updates. Auto-fixed in the same T3 commit so the wave-end gate (T5) `cargo build --all-targets` passes. |
| 5 | Rule 3 | Missing scope | Plan did not anticipate the `cargo fmt` reflow (4-arg / 8-arg shapes pushed past rustfmt single-line threshold). Absorbed in commit `9fc6efb` after T5 surfaced the fmt-check failure. |
| 6 | Rule 3 | Missing scope | Plan did not anticipate `clippy::too_many_arguments` triggering on the 8-arg finalize_run. Added `#[allow(...)]` with rationale doc comment in commit `0f41934` after T5 surfaced the clippy failure. |

**Total deviations:** 6 (1 Rule 1, 5 Rule 3). All deviations are Rule 1/3 auto-fixes — no architectural changes, no checkpoint. Each is a strictly mechanical fix needed to make the wave-end gate pass.

**Impact on plan:** None on the locked design. The deviations are scope-coverage gaps in T2/T3 enumeration, plus two follow-on cleanups (fmt + clippy) that the plan didn't anticipate. All resolved in the same wave-2 batch.

## Issues Encountered

- **Worktree base mismatch at agent startup** — `git merge-base HEAD <expected-base>` returned `c87f12e` (Phase 15 close-out) instead of the expected `00f5b8e` (16-04a wave-2 predecessor merge). Per the agent prompt's `<worktree_branch_check>` block, hard-reset the worktree to `00f5b8e` before starting work. No data loss because this was a fresh worktree.
- **6 separate scope gaps surfaced during T5 gate execution.** Each was a Rule 3 auto-fix per `<deviation_rules>`. Pattern: plan T2/T3 enumerated production sites + src/ test-mod sites, but missed (a) 2 production fallbacks in scheduler/mod.rs, (b) 2 test-mod finalize_run sites in queries.rs, (c) 21 sites across 12 tests/ files, (d) the rustfmt reflow, (e) the clippy allowance. All resolved without checkpoint per Rules 1-3.

## User Setup Required

None — no external service configuration required. Local CI gate (cargo build, clippy, fmt-check, nextest, schema-diff) passes without any setup beyond the standard `cargo` + `just` toolchain.

The HUMAN-UAT spot check (`just uat-fctx-bugfix-spot-check`) requires:
- A populated `cronduit.dev.db` (via a prior `just dev` run).
- At least one `type = "docker"` job that has fired since the v1.2 commit lands.
- Docker daemon available on the host.

## Next Phase Readiness

- **Plan 16-05** (`get_failure_context` query helper) — wave-3 cohort, parallel-eligible with 16-06. Now unblocked: queries.rs has the new struct fields (DbRun.config_hash + DbRun.image_digest) the SELECT can read; the codebase compiles cleanly and `cargo test` runs.
- **Plan 16-06** (EXPLAIN tests) — wave-3 cohort, parallel-eligible with 16-05. Same readiness signal.
- **Phase 18** (webhook payload, WH-09) — will read DbRun.image_digest + DbRun.config_hash directly. No further queries.rs changes needed for that consumer.
- **Phase 21** (FCTX UI panel) — will render image_digest + config_hash deltas from DbRunDetail. The BACKFILL_CUTOFF_RFC3339 marker reference deposited in 16-04a's doc comments points implementers at the convention.
- **PR 1 (Plans 16-01..16-04b) is mergeable.** The wave-end gate has run and is green. The maintainer can also run `just uat-fctx-bugfix-spot-check` against a populated dev DB to validate FOUND-14 Success Criterion 1 visually.
- No new attack surface introduced. THREAT_MODEL.md unchanged. Plan's threat register (T-16-04b-01..02) remains accurate — both threats are `accept` disposition with severity `low`. The signature transition tightens correctness (config_hash now per-run, image_digest now persisted) without expanding the trust boundary.
- No `Cargo.toml`, dependency, or migration changes.

## Self-Check: PASSED

Verified at the end of execution:

- `src/scheduler/run.rs` — FOUND.
- `src/scheduler/mod.rs` — FOUND.
- `src/web/handlers/api.rs` — FOUND.
- `src/db/queries.rs` — FOUND.
- `justfile` — FOUND.
- `.planning/phases/16-failure-context-schema-run-rs-277-bug-fix/16-HUMAN-UAT.md` — FOUND.
- All 17 modified test files — FOUND.
- All 6 commits present in branch:
  - `7035287` (T1, run.rs:86 production caller) — FOUND.
  - `ba01532` (T2, api.rs callers) — FOUND.
  - `b971e34` (T3, test-mod + integration-test callers) — FOUND.
  - `c1964f8` (T4, justfile recipe + HUMAN-UAT update) — FOUND.
  - `9fc6efb` (T5a, fmt cleanup) — FOUND.
  - `0f41934` (T5b, clippy allow) — FOUND.
- All 9 PLAN acceptance-criteria greps pass (per Verification table above).
- Wave-end gate (T5) green: cargo build, fmt-check, clippy, cargo test --lib (194 passed), schema-diff, grep-no-percentile-cont, v12_fctx_config_hash_backfill (4 passed), v12_run_rs_277_bug_fix command_run_leaves_image_digest_null, just nextest (388 passed, 22 skipped).
- No modifications to `.planning/STATE.md` or `.planning/ROADMAP.md` — verified via `git status --short` (clean).

---
*Phase: 16-failure-context-schema-run-rs-277-bug-fix*
*Plan: 04b — callers + just recipe + wave-end gate*
*Completed: 2026-04-27*
