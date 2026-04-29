---
phase: 17-custom-docker-labels-seed-001
plan: 08
subsystem: config-validation
tags: [labels, validators, lbl-04, set-diff, error-messages, gap-closure, cr-02]

# Dependency graph
requires:
  - phase: 17-02
    provides: "LBL-04 baseline validator (`check_labels_only_on_docker_jobs`) and the pin contract `lbl_04_error_does_not_leak_defaults_keys_for_non_docker_jobs` at src/config/defaults.rs:447-509"
  - phase: 17-07
    provides: "expanded check_label_key_chars docstring (sibling validator; left this plan's target function body unchanged)"
provides:
  - "Set-diff aware `check_labels_only_on_docker_jobs(job, defaults_labels, path, errors)` that distinguishes operator-set keys from defaults-merged keys"
  - "Branch A (operator-set): legacy 'Remove the `labels` block' message preserved verbatim for backwards compat"
  - "Branch B (defaults-only): NEW remediation 'set `use_defaults = false` on this job to opt out, OR remove `[defaults].labels`'"
  - "Three new unit tests pinning Branch A, Branch B, and the mixed (operator + defaults) case"
  - "One non-#[ignore] integration test pinning the binary-path Branch B behavior end-to-end through parse_and_validate"
affects: [17-VERIFICATION-gap-closure, 17-final-verification, future-readme-edits-on-labels-section]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Set-diff via `BTreeSet<&str>` against original defaults map for deterministic, no-leak error attribution after apply_defaults has merged"
    - "Two-branch validator with verbatim-legacy-text fallback for backwards-compatible diagnostic refactor"

key-files:
  created: []
  modified:
    - src/config/validate.rs
    - tests/v12_labels_use_defaults_false.rs

key-decisions:
  - "Option A (Set-diff in formatter) — apply_defaults merge gate is unchanged; the validator does the attribution. Per user's locked CR-02 decision."
  - "Branch A error text preserved verbatim for backwards compat (existing CI/operator scripts grep for 'Remove the `labels` block')"
  - "Job-type discriminator (command|script|command/script) computed from job.command/script.is_some(); fallback to combined string when neither set"
  - "BTreeSet<&str> over HashSet for operator_only_keys to keep iteration deterministic (RESEARCH Pitfall 2 — HashMap iter order is random)"

patterns-established:
  - "Diagnostic refactor pattern: when apply_defaults has merged data into a struct before a validator runs, the validator can recover the operator-only subset by set-diffing against the original [defaults] map (which apply_defaults clones from but does not consume)"
  - "Backwards-compatible validator-message refactor: keep the existing message text verbatim for the existing branch; introduce new branches with distinct messages for newly-recognized cases"

requirements-completed: [LBL-04]

# Metrics
duration: 6min
completed: 2026-04-28
---

# Phase 17 Plan 08: LBL-04 Error Attribution (CR-02 gap closure) Summary

**Set-diff in the LBL-04 validator distinguishes operator-set from defaults-merged label keys and emits two distinct error messages — preserves legacy "Remove the `labels` block" text for the operator-set case, and adds the new "set `use_defaults = false`" remediation for the defaults-only case.**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-04-29T03:28:03Z (RED commit)
- **Completed:** 2026-04-29T03:33:34Z (integration test commit)
- **Tasks:** 2 (3 commits — RED, GREEN, integration test)
- **Files modified:** 2

## Accomplishments

- Closed verification gap **CR-02** (LBL-04 error misattribution). Operators no longer see a misleading "Remove the `labels` block" error for label blocks they never wrote.
- Implemented user-locked **Option A** (set-diff in formatter): `apply_defaults` (defaults.rs:166-176) is UNCHANGED; the validator now set-diffs against `cfg.defaults.labels` to recover operator-only keys.
- Branch A (operator-set) preserves the EXISTING legacy message verbatim — backwards-compatible with existing CI grep and operator scripts.
- Branch B (defaults-only) emits the NEW remediation pointing operators at the actual fix: `use_defaults = false` (or remove `[defaults].labels`).
- Defaults keys never leak into the error message in either branch (set-diff hides them; pinned by 4 separate test assertions across 3 unit tests + 1 integration test).
- 6 LBL-04 unit tests + 1 integration test all pass (3 existing unit tests preserved + 3 new + 1 new integration test). Existing apply_defaults invariant tests (19 total) still pass. Full library suite (218 tests) green.

## Task Commits

Each task was committed atomically. Per project convention this work landed on `phase-17-custom-docker-labels` feature branch via the worktree harness — no direct main commits (D-06).

1. **Task 1 RED: Add failing LBL-04 set-diff branch tests** — `fea3b6b` (test)
   - Added 3 new unit tests with the new `(job, defaults_labels, path, errors)` signature
   - Updated function signature with `_defaults_labels` (stub body still emits legacy message unconditionally)
   - Updated call site in run_all_checks to pass `cfg.defaults.as_ref().and_then(|d| d.labels.as_ref())`
   - Updated existing 3 LBL-04 tests to pass `None` for the new arg
   - RED gate verified: `lbl_04_command_job_with_defaults_only_emits_distinct_use_defaults_false_message` failed because the stub did not yet emit Branch B remediation text
2. **Task 1 GREEN: Implement LBL-04 set-diff with two distinct error messages** — `6342724` (feat)
   - Replaced stub body with set-diff against defaults_labels into BTreeSet<&str> operator_only_keys
   - Branch A (operator_only_keys non-empty): emit legacy text verbatim
   - Branch B (operator_only_keys empty AND defaults_labels Some & non-empty): emit NEW "set `use_defaults = false`" remediation with command/script job-type discriminator
   - Fast paths preserved (job.labels.is_none() OR job.image.is_some() → early return)
3. **Task 2: Add LBL-04 Branch B integration test (no Docker daemon)** — `eca5909` (test)
   - Added non-#[ignore] integration test `lbl_04_defaults_only_command_job_emits_use_defaults_false_remediation`
   - Drives full parse pipeline (interpolate -> toml -> apply_defaults -> validate) on a TOML fixture with `[defaults].labels` + bare command job
   - Asserts Branch B phrase present, Branch A phrase absent, job name present, defaults key absent (no leak)
   - Updated file-level doc comment

**Plan metadata:** SUMMARY commit (this commit) finalizes the plan.

_Note: Task 1 follows TDD (test → feat) yielding two commits; Task 2's test pins newly-shipped behavior at the integration tier._

## Files Created/Modified

- `src/config/validate.rs` — `check_labels_only_on_docker_jobs` rewritten with set-diff and two-branch error emission; signature now `(job, defaults_labels: Option<&HashMap<String, String>>, path, errors)`. Three new unit tests added in `mod tests` after the existing LBL-04 block. Existing 3 LBL-04 tests updated to pass `None`. Call site in `run_all_checks` updated to pass `cfg.defaults.as_ref().and_then(|d| d.labels.as_ref())`.
- `tests/v12_labels_use_defaults_false.rs` — One new non-#[ignore] integration test appended; file-level doc comment updated to reference plan 17-08.

## Before / After Error Messages

### Branch A — operator-set case (UNCHANGED, backwards-compatible)

Operator wrote `labels = {...}` on a command/script job:

**Before AND After:**
```
[[jobs]] `<name>`: `labels` is only valid on docker jobs (job with `image = "..."` set
either directly or via `[defaults].image`); command and script jobs cannot set `labels`
because there is no container to attach them to. Remove the `labels` block, or switch
the job to a docker job by setting `image`.
```

### Branch B — defaults-only case (NEW)

Operator set `[defaults].labels = {...}` and a command/script job has no per-job labels block (and no `use_defaults = false`):

**Before (the misleading bug — "Remove the `labels` block" told operator to remove a block they never wrote):**
```
[[jobs]] `<name>`: `labels` is only valid on docker jobs (job with `image = "..."` set
either directly or via `[defaults].image`); command and script jobs cannot set `labels`
because there is no container to attach them to. Remove the `labels` block, or switch
the job to a docker job by setting `image`.
```

**After (Branch B — names the actual fix):**
```
[[jobs]] `<name>`: this is a command job; labels are docker-only. `[defaults].labels`
is set and would be merged into this job by `apply_defaults` — set `use_defaults = false`
on this job to opt out, OR remove `[defaults].labels`.
```

### Mixed case — operator + defaults inheritance

Operator wrote `labels = {operator.key = ...}` on a command job AND `[defaults].labels = {inherited.from.defaults = ...}`. After apply_defaults, `job.labels` contains both keys.

**Behavior:** Branch A wins (operator_only_keys is non-empty); legacy message fires; the inherited defaults key (`inherited.from.defaults`) MUST NOT appear in the error text — set-diff hides it. Pinned by `lbl_04_command_job_with_mixed_operator_and_defaults_emits_legacy_message_only_for_operator_keys`.

## Tests Added (4 total)

Three new unit tests in `src/config/validate.rs` `mod tests`:

1. `lbl_04_command_job_with_operator_set_labels_emits_legacy_message` — Branch A; operator-set, no defaults; legacy text present, Branch B phrase absent.
2. `lbl_04_command_job_with_defaults_only_emits_distinct_use_defaults_false_message` — Branch B; defaults-only on command job; Branch B phrase present, Branch A phrase absent, defaults key absent (no leak), `command job` discriminator present.
3. `lbl_04_command_job_with_mixed_operator_and_defaults_emits_legacy_message_only_for_operator_keys` — Mixed; Branch A wins; defaults key absent from error (set-diff hides it).

One new integration test in `tests/v12_labels_use_defaults_false.rs`:

4. `lbl_04_defaults_only_command_job_emits_use_defaults_false_remediation` — End-to-end through `parse_and_validate`; non-#[ignore] (no Docker daemon); asserts Branch B phrase present, Branch A phrase absent, job name present, defaults key absent.

## apply_defaults Semantics — UNCHANGED

The `apply_defaults` function at `src/config/defaults.rs:108-182` is **unmodified** by this plan. The labels merge block at lines 166-176 still merges `defaults.labels` into command/script jobs unconditionally (no `is_non_docker` gate), as required by:

- Pin contract `lbl_04_error_does_not_leak_defaults_keys_for_non_docker_jobs` (defaults.rs:447-509) — passes unchanged
- Invariant test `apply_defaults_merges_labels_per_job_wins` — passes unchanged
- Invariant test `apply_defaults_use_defaults_false_replaces_labels` — passes unchanged

The set-diff happens downstream in the validator. The validator reads `cfg.defaults.labels` (which `apply_defaults` clones from but does not consume) and computes `operator_only_keys = job.labels.keys() - defaults_labels.keys()`. This recovers the operator-only key set after the merge, satisfying the test contract that pinned this design at plan 17-02 time.

## Decisions Made

- **Option A (Set-diff in formatter) over Option B (gate the merge in apply_defaults).** User-locked. Option B would have required changing apply_defaults's labels merge to gate on `is_non_docker`, which would have broken the pin contract `lbl_04_error_does_not_leak_defaults_keys_for_non_docker_jobs` and the apply_defaults invariant tests. Option A keeps apply_defaults stable and moves the attribution into the validator where the diagnostic message is emitted.
- **Branch A text preserved verbatim.** Existing CI grep, operator scripts, and any tests that check for "Remove the `labels` block" continue to work. The plan's must-haves enumerate this as a backwards-compat truth.
- **`BTreeSet<&str>` for operator_only_keys.** Even though the current implementation does not enumerate these keys in the error message, using a deterministic ordering future-proofs any downstream change that does want to print them (RESEARCH Pitfall 2 — HashMap iteration is non-deterministic).
- **Job-type discriminator computed inline in Branch B.** No new helper extracted; the three-arm match (`command` | `script` | `command/script` fallback) is small and self-documenting.

## Deviations from Plan

None — plan executed exactly as written. The only minor procedural variation: the integration test in Task 2 passed on first run because Task 1 (GREEN) had already shipped the underlying validator fix; the test commit therefore acted as a regression-pin rather than a strict TDD GREEN-following-RED moment. This matches the plan's intent (Task 2's `<behavior>` describes the test as "drives the binary path and confirms the new remediation phrase appears in stderr for the defaults-only-on-command-job case" — pin behavior, not drive new code).

## Issues Encountered

None — all tests pass on first run after each commit; clippy and fmt clean throughout.

## User Setup Required

None — purely an internal config-validation diagnostic refactor. No new dependencies, no env vars, no external services.

## Verification Gates (all green)

```
cargo build --lib            → exit 0
cargo build --tests          → exit 0
cargo clippy --all-targets -- -D warnings → exit 0
cargo fmt --check            → exit 0
cargo test --lib (218 tests) → exit 0 (all pass)
cargo test --test v12_labels_use_defaults_false → exit 0 (1 pass, 1 ignored as designed)
```

LBL-04 specific tests (7 total):
```
config::validate::tests::check_labels_only_on_docker_jobs_accepts_docker_job ... ok
config::validate::tests::check_labels_only_on_docker_jobs_rejects_on_command_job ... ok
config::validate::tests::check_labels_only_on_docker_jobs_rejects_on_script_job ... ok
config::validate::tests::lbl_04_command_job_with_operator_set_labels_emits_legacy_message ... ok
config::validate::tests::lbl_04_command_job_with_defaults_only_emits_distinct_use_defaults_false_message ... ok
config::validate::tests::lbl_04_command_job_with_mixed_operator_and_defaults_emits_legacy_message_only_for_operator_keys ... ok
config::defaults::tests::lbl_04_error_does_not_leak_defaults_keys_for_non_docker_jobs ... ok
```

apply_defaults invariant tests (19 total) all green — labels merge unchanged.

## Cross-References

- **Verification gap:** `.planning/phases/17-custom-docker-labels-seed-001/17-VERIFICATION.md` § CR-02 (LBL-04 error attribution) — closed by this plan.
- **Code review finding:** `.planning/phases/17-custom-docker-labels-seed-001/17-REVIEW.md` § CR-02 (lines 136-229) — provided the set-diff implementation sketch this plan implements.
- **Baseline plan:** `17-02-SUMMARY.md` — established the original LBL-04 validator and the pin contract `lbl_04_error_does_not_leak_defaults_keys_for_non_docker_jobs` that this plan's validator now upholds at the diagnostic layer.
- **Sibling just-merged plan:** `17-07-SUMMARY.md` — confirms 17-07 only touched the docstring of `check_label_key_chars` (sibling validator); the body of `check_labels_only_on_docker_jobs` was untouched, so this plan's pattern-find/replace targets remained intact even after 17-07's line-number shift.

## Next Phase Readiness

CR-02 closed. Phase 17 verification report (`17-VERIFICATION.md`) can move CR-02 from "open" to "closed" once this plan merges. No new blockers. The plan is independent of any in-flight wave-2 sibling work and is ready to merge as part of the gap-closure wave.

## Self-Check: PASSED

- File `src/config/validate.rs` exists and contains the new function signature, both branch messages, the call-site update, and all three new unit tests.
- File `tests/v12_labels_use_defaults_false.rs` exists, contains the new test function, and its file-level doc comment references plan 17-08.
- Commits exist:
  - `fea3b6b` test(17-08): add failing LBL-04 set-diff branch tests (RED)
  - `6342724` feat(17-08): implement LBL-04 set-diff with two distinct error messages (GREEN)
  - `eca5909` test(17-08): add LBL-04 Branch B integration test (no Docker daemon)
- All verification commands exit 0.

---
*Phase: 17-custom-docker-labels-seed-001*
*Plan: 08 (gap closure for CR-02)*
*Completed: 2026-04-28*
