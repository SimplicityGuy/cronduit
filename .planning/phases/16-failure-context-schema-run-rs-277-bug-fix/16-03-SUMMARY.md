---
phase: 16-failure-context-schema-run-rs-277-bug-fix
plan: 03
subsystem: scheduler
tags: [bug-fix, run.rs-277, FOUND-14, integration-test, testcontainers]

# Dependency graph
requires:
  - phase: 16-failure-context-schema-run-rs-277-bug-fix
    provides: "DockerExecResult.container_id field (Plan 16-02) — direct prerequisite for the .container_id read at run.rs:301"
  - phase: 16-failure-context-schema-run-rs-277-bug-fix
    provides: "job_runs.image_digest column (Plan 16-01) — the test SELECTs this column post-finalize_run"
provides:
  - "src/scheduler/run.rs:231-232: parallel image_digest_for_finalize: Option<String> local declared adjacent to container_id_for_finalize"
  - "src/scheduler/run.rs:302-303: bug-fix at the docker arm — container_id_for_finalize now reads docker_result.container_id.clone() (was .image_digest); image_digest_for_finalize captures the digest separately"
  - "src/scheduler/run.rs:348-358: finalize_run invocation extended to pass image_digest_for_finalize.as_deref() as the new last positional (8th argument)"
  - "tests/v12_run_rs_277_bug_fix.rs: 4 integration tests covering T-V12-FCTX-07 (real container_id + sha256: digest), T-V12-FCTX-08 (command-run NULL digest), T-V12-FCTX-09 partial (inspect-failure persistence)"
affects: [16-04a, 16-04b]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Bug-fix-with-parallel-capture: when an existing local was misnamed (carrying the wrong field), introduce a sibling local for the formerly-shadowed field rather than rename + reshuffle. Preserves grep history of the original name (container_id_for_finalize) while making the corrected wiring explicit."
    - "Wave-coupled test landing: integration test files for a multi-plan signature transition land in the plan that owns the call-site change, with post-wave signatures, and rely on the wave-end orchestrator gate (not the per-plan verify gate) to actually exercise cargo-test."

key-files:
  created:
    - tests/v12_run_rs_277_bug_fix.rs
  modified:
    - src/scheduler/run.rs

key-decisions:
  - "Lifted Plan 16-04's eventual finalize_run signature into the test file's call sites (insert_running_run takes config_hash &str, finalize_run takes image_digest: Option<&str>). The test will not compile against this commit alone — by design — but compiles end-to-end after 16-04b lands. The wave-2 sequential context explicitly authorises this 'reserved test file with skeleton — exercise deferred to wave-end' shape (PLAN.md Task 4 note)."
  - "Wrote four test functions (Test 4 'digest_persists_across_inspect_failure' included as a non-#[ignore] SQLite-only contract test) instead of the minimal three. The fourth covers T-V12-FCTX-09 partial — the inspect_container failure path still persists a queryable row. Marginal cost (one additional in-memory test, no docker dependency) for tighter operator-observable coverage."
  - "Used the existing common::v11_fixtures::setup_sqlite_with_phase11_migrations() helper rather than re-implementing the in-memory pool setup. This fixture applies ALL migrations including 16-01's image_digest + config_hash columns, so the test SELECTs work without bespoke schema setup."

patterns-established:
  - "Bug-fix commit triplet: (a) declare new local, (b) fix the misnamed assignment + add the parallel one, (c) extend the call site. Each commit isolates one concern and is independently revertable. The same shape will scale for any future per-finalize-run signature additions."

requirements-completed: [FOUND-14]

# Metrics
duration: 3min
completed: 2026-04-28
---

# Phase 16 Plan 03: run.rs:301 bug fix + image_digest plumbing Summary

**Fixed the load-bearing v1.1 bug at `src/scheduler/run.rs:301` where `container_id_for_finalize` was silently capturing `docker_result.image_digest` instead of the real container ID. Added a parallel `image_digest_for_finalize` local so both values flow through `finalize_run`. Landed the testcontainers integration test asserting the operator-observable from Phase 16 Success Criterion 1 (`job_runs.container_id` is the real Docker container ID, not a `sha256:...` digest, for v1.2 docker runs).**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-04-28T02:51:27Z
- **Completed:** 2026-04-28T02:55:01Z
- **Tasks:** 4 / 4
- **Files modified:** 1 (`src/scheduler/run.rs`)
- **Files created:** 1 (`tests/v12_run_rs_277_bug_fix.rs`)

## Accomplishments

- Declared a parallel `image_digest_for_finalize: Option<String>` local at `src/scheduler/run.rs:232` adjacent to the existing `container_id_for_finalize` (Phase 16 FOUND-14 trailing comment).
- Fixed the bug at `src/scheduler/run.rs:302`: `container_id_for_finalize = docker_result.container_id.clone()` (corrected — was reading `.image_digest`) and added `image_digest_for_finalize = docker_result.image_digest.clone()` (new parallel capture).
- Extended the `finalize_run(...)` invocation at `src/scheduler/run.rs:348-358` to pass `image_digest_for_finalize.as_deref()` as the new last positional argument (8th total).
- Created `tests/v12_run_rs_277_bug_fix.rs` with four `#[tokio::test]` functions covering T-V12-FCTX-07, T-V12-FCTX-08, and T-V12-FCTX-09-partial.

## Task Commits

| # | Task | Commit | Type |
|---|------|--------|------|
| 1 | Add image_digest_for_finalize parallel local at run.rs:231 | `42c8901` | feat |
| 2 | Fix the bug at run.rs:301 and populate image_digest_for_finalize | `f578ca1` | fix |
| 3 | Update finalize_run call-site at run.rs:348-356 to pass image_digest_for_finalize | `4d69003` | feat |
| 4 | Create testcontainers integration test tests/v12_run_rs_277_bug_fix.rs | `a4a4fd1` | test |

## Files Created/Modified

### Modified

- **`src/scheduler/run.rs`** (3 surgical edits)
  - L231-232 (T1): parallel `image_digest_for_finalize` local declared with matching `Option<String> = None` shape.
  - L302-303 (T2): bug-fix at the docker arm — `container_id_for_finalize` now correctly reads `docker_result.container_id.clone()`; `image_digest_for_finalize` newly captures `docker_result.image_digest.clone()`.
  - L348-358 (T3): `finalize_run(...)` invocation extended with the new 8th positional argument `image_digest_for_finalize.as_deref()`.

### Created

- **`tests/v12_run_rs_277_bug_fix.rs`** (4 test functions, ~350 lines)
  - `docker_run_writes_real_container_id_not_digest` — `#[ignore]`-gated; T-V12-FCTX-07 (operator-observable bug-fix assertion `!cid.starts_with("sha256:")`).
  - `docker_run_writes_image_digest_as_sha256` — `#[ignore]`-gated; T-V12-FCTX-07 (digest captured separately as `sha256:...`).
  - `command_run_leaves_image_digest_null` — runs in standard CI (no Docker daemon required); T-V12-FCTX-08.
  - `digest_persists_across_inspect_failure` — runs in standard CI; T-V12-FCTX-09-partial (inspect-container failure path still persists queryable row).

## Three edits to run.rs (before/after)

### L231-232 — declare locals

**Before:**
```rust
    let mut container_id_for_finalize: Option<String> = None;
```

**After:**
```rust
    let mut container_id_for_finalize: Option<String> = None;
    let mut image_digest_for_finalize: Option<String> = None;  // Phase 16 FOUND-14
```

### L302-303 — bug fix + parallel capture

**Before:**
```rust
                container_id_for_finalize = docker_result.image_digest.clone();
                docker_result.exec
```

**After:**
```rust
                container_id_for_finalize = docker_result.container_id.clone();   // Phase 16 FOUND-14: was incorrectly .image_digest
                image_digest_for_finalize = docker_result.image_digest.clone();   // Phase 16 FOUND-14: NEW parallel capture
                docker_result.exec
```

### L348-358 — finalize_run call-site

**Before:**
```rust
    if let Err(e) = finalize_run(
        &pool,
        run_id,
        status_str,
        exec_result.exit_code,
        start,
        exec_result.error_message.as_deref(),
        container_id_for_finalize.as_deref(),
    )
    .await
```

**After:**
```rust
    if let Err(e) = finalize_run(
        &pool,
        run_id,
        status_str,
        exec_result.exit_code,
        start,
        exec_result.error_message.as_deref(),
        container_id_for_finalize.as_deref(),
        image_digest_for_finalize.as_deref(),   // Phase 16 FOUND-14: new last positional
    )
    .await
```

## Four test functions in tests/v12_run_rs_277_bug_fix.rs

| # | Function | Gate | Test ID | Asserts |
|---|----------|------|---------|---------|
| 1 | `docker_run_writes_real_container_id_not_digest` | `#[ignore]` (Docker daemon required) | T-V12-FCTX-07 | After a real alpine echo run, `job_runs.container_id` is `Some(_)` AND does NOT start with `sha256:` (the operator-observable bug fix). |
| 2 | `docker_run_writes_image_digest_as_sha256` | `#[ignore]` (Docker daemon required) | T-V12-FCTX-07 | After the same kind of run, `job_runs.image_digest` is `Some(_)` AND starts with `sha256:`. |
| 3 | `command_run_leaves_image_digest_null` | Standard CI (no Docker) | T-V12-FCTX-08 | A non-docker `finalize_run` invocation leaves `job_runs.image_digest` and `container_id` both NULL. |
| 4 | `digest_persists_across_inspect_failure` | Standard CI (no Docker) | T-V12-FCTX-09 (partial) | When `image_digest` is `None` (inspect_container failed) but `container_id` is `Some(_)`, the row is still persisted with status=success and queryable. |

Run docker-gated tests with: `cargo test --test v12_run_rs_277_bug_fix -- --ignored --nocapture --test-threads=1`. Run the standard-CI subset with: `cargo test --test v12_run_rs_277_bug_fix`.

## Decisions Made

- **Reserved-skeleton test file with post-wave-2 signatures.** Per Plan 16-03 Task 4's `<note>` element and the wave-2 sequential context: the test file lands in this plan but uses the `insert_running_run(&pool, job_id, "manual", "testhash")` (4-arg, post-16-04b) and `finalize_run(..., container_id, image_digest)` (8-arg, post-16-04a) shapes. Cargo test against this file will FAIL TO COMPILE between this plan's last commit and 16-04a's signature commit — that is by design and the wave-end orchestrator gate handles the actual cargo-test exercise.
- **Test 4 (`digest_persists_across_inspect_failure`) included even though it is "optional".** Cost is one extra in-memory SQLite test (~50 lines, no Docker dependency). Coverage is T-V12-FCTX-09-partial — the contract that `finalize_run(Some(real_cid), None)` still produces a queryable row. Worth the marginal cost to lock the shape.
- **Used the existing `common::v11_fixtures::setup_sqlite_with_phase11_migrations()` helper.** This applies all migrations including 16-01's image_digest + config_hash columns. No new test fixture was introduced, keeping the coupling to common test machinery minimal.

## Deviations from Plan

None — plan executed exactly as written.

The plan explicitly authorised the post-wave-2 signature usage in the test file (Task 4 `<note>`); the four-test inclusion (vs. minimum three) is consistent with the `<behavior>` block which lists all four including the optional Test 4. No auto-fixes were needed; no architectural decisions; no auth gates; no out-of-scope discoveries.

**Total deviations:** 0
**Impact on plan:** None.

## Issues Encountered

- **Worktree base mismatch at agent startup** — `git merge-base HEAD <expected-base>` returned `c87f12e` (an older Phase 15 close-out commit) rather than the expected `caad230`. Per the agent prompt's `<worktree_branch_check>` block, hard-reset the worktree to `caad230` before starting work. Verified the reset landed and the wave-1 plans (16-01, 16-02) were present in the history before proceeding. No data loss because this was a fresh worktree.
- **`cargo build` will fail at this plan's last commit by design.** Plan 16-03 alone takes the `finalize_run(...)` call site to 8 arguments, but `queries.rs::finalize_run`'s signature is still 7. This is documented in PLAN.md Task 3 and CONTEXT.md D-09, and is resolved by 16-04a/16-04b in the same wave-2 PR. Per the wave-2 orchestrator's `--no-verify` policy, no per-plan build gate runs.

## User Setup Required

None — no external service configuration required. This is a pure code change in two files.

## Next Phase Readiness

- **Plan 16-04a** is now unblocked: it owns the `queries.rs::finalize_run` and `insert_running_run` signature changes that 16-03 references at the call site (and that the test file uses). After 16-04a lands, `cargo build` becomes green again.
- **Plan 16-04b** (callers, recipe, gate) runs the integration test in `tests/v12_run_rs_277_bug_fix.rs` end-to-end; the test file is in place ready for that gate.
- No new attack surface introduced. THREAT_MODEL.md unchanged. The bug fix tightens correctness — the historical leak of image digests into `job_runs.container_id` is now prevented for new v1.2 rows; existing rows age out via the v1.0 Phase 6 retention pruner per the locked plan.
- No `Cargo.toml`, dependency, or migration changes.

## Self-Check: PASSED

Verified at the end of execution:

- `src/scheduler/run.rs` exists — FOUND.
- `tests/v12_run_rs_277_bug_fix.rs` exists — FOUND.
- Commit `42c8901` (T1) — FOUND in branch.
- Commit `f578ca1` (T2) — FOUND in branch.
- Commit `4d69003` (T3) — FOUND in branch.
- Commit `a4a4fd1` (T4) — FOUND in branch.
- All four PLAN acceptance-criteria greps pass:
  - `grep -q 'let mut image_digest_for_finalize: Option<String> = None;' src/scheduler/run.rs` → 0
  - `grep -q 'container_id_for_finalize = docker_result.container_id.clone' src/scheduler/run.rs` → 0
  - `! grep -qE 'container_id_for_finalize\s*=\s*docker_result\.image_digest' src/scheduler/run.rs` → 0 (the bug is GONE)
  - `grep -A 12 'if let Err(e) = finalize_run' src/scheduler/run.rs | grep -q 'image_digest_for_finalize.as_deref()'` → 0
  - `tests/v12_run_rs_277_bug_fix.rs` contains the three required `#[tokio::test]` function names + the `!cid.starts_with("sha256:")` substring + the `Phase 16 FOUND-14` reference.

---
*Phase: 16-failure-context-schema-run-rs-277-bug-fix*
*Plan: 03 — run.rs:301 bug fix + image_digest plumbing*
*Completed: 2026-04-28*
