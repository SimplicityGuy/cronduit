---
phase: 17-custom-docker-labels-seed-001
plan: 07
subsystem: documentation
tags: [labels, env-var-interpolation, validators, docs, integration-tests, gap-closure, CR-01]

# Dependency graph
requires:
  - phase: 17-custom-docker-labels-seed-001
    provides: |
      Plan 17-05's tests/v12_labels_interpolation.rs harness (parse_and_validate
      pipeline + value-side test); Plan 17-02's check_label_key_chars (D-02
      strict regex); Plan 17-04's README labels subsection (env-var paragraph
      + mermaid + table).
provides:
  - Accurate README env-var interpolation paragraph (whole-file textual pass; env-set + env-unset cases enumerated)
  - "Recommended pattern" prose in README steering operators toward value-only interpolation
  - check_label_key_chars docstring describing D-02 char enforcement on resolved keys (false "Partially enforces LBL-05" claim removed)
  - interpolate::interpolate docstring stating WHOLE-FILE TEXTUAL REPLACEMENT semantics + downstream-consumer warning
  - Two new integration tests pinning env-set and env-unset key-position interpolation contracts
affects:
  - "Future phases that touch label semantics or env-var interpolation"
  - "Phase 17 verification re-run (closes gap CR-01)"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Pure parse-pipeline integration tests (no Docker daemon required) for config-load contracts"
    - "Documentation-as-source-of-truth: when implementation cannot be cheaply tightened, document the actual behavior accurately rather than maintain a false invariant"

key-files:
  created:
    - .planning/phases/17-custom-docker-labels-seed-001/17-07-SUMMARY.md
  modified:
    - README.md
    - src/config/validate.rs
    - src/config/interpolate.rs
    - tests/v12_labels_interpolation.rs

key-decisions:
  - "CR-01 fix: Option A (relax docs) per maintainer's locked decision in 17-VERIFICATION.md — chosen over Option B (AST-aware pre-interpolation key check) which would have required two-pass parsing for non-trivial implementation cost"
  - "New integration tests are NOT #[ignore] (no Docker daemon needed) — they exercise only the parse pipeline; this lets them run in CI on every push, unlike the existing #[ignore] value-side test that requires a live Docker daemon"
  - "env-unset assertion uses OR-disjunction over the two error-message paths (MissingVar from interpolate.rs OR D-02 invalid-char from validate.rs) — the actual binary path takes the MissingVar branch first, but the assertion is robust against either outcome"

patterns-established:
  - "Gap-closure plans land via PR per D-06; same atomic-commit-per-task convention as feature plans"
  - "Documentation-only deviations from prior phases land as fast-followers in the same phase (17-07/08/09) rather than re-opening earlier plans"

requirements-completed: [LBL-05]

# Metrics
duration: 11min
completed: 2026-04-29
---

# Phase 17 Plan 07: CR-01 Gap Closure (Relax LBL-05 Docs) Summary

**Replaced the README's false "Label KEYS are NEVER interpolated" absolute guarantee with accurate prose describing the implemented whole-file textual interpolation pass, dropped the matching "Partially enforces LBL-05" validator-docstring claim, and pinned both env-set and env-unset key-position interpolation contracts with two new non-ignored integration tests.**

## Performance

- **Duration:** ~11 min
- **Started:** 2026-04-29T03:07:13Z
- **Completed:** 2026-04-29T03:17:42Z
- **Tasks:** 3
- **Files modified:** 4 (README.md, src/config/validate.rs, src/config/interpolate.rs, tests/v12_labels_interpolation.rs)

## Accomplishments

- README.md env-var interpolation paragraph rewritten to match the actual whole-file textual interpolation behavior; the four cases (value-side env-set, value-side env-unset, key-side env-set, key-side env-unset) are explicitly enumerated; mermaid diagram + 3-row merge table + other rule paragraphs untouched.
- "Recommended pattern" prose added to README steering operators toward value-only `${VAR}` interpolation, with a `SUPPORTED BUT DISCOURAGED` example for the key-side case.
- `src/config/validate.rs::check_label_key_chars` docstring rewritten: drops the false "Partially enforces LBL-05's keys-not-interpolated" claim and accurately describes D-02 character enforcement on the post-interpolation resolved key string, with both env-set and env-unset cases spelled out.
- `src/config/interpolate.rs::interpolate` docstring rewritten: explicitly states the pass is `WHOLE-FILE TEXTUAL REPLACEMENT`, does NOT respect TOML structure, and warns downstream consumers not to assume keys are exempt from interpolation.
- Two new integration tests appended to `tests/v12_labels_interpolation.rs` — neither is `#[ignore]` (no Docker daemon required), both run via `cargo test --test v12_labels_interpolation -- --test-threads=1 lbl_05_key_position`:
  - `lbl_05_key_position_interpolation_env_set_resolves_to_literal_when_pattern_matches` — pins that `TEAM=ops` + `labels = { "${TEAM}" = "v" }` resolves to a literal `"ops"` key and is accepted.
  - `lbl_05_key_position_interpolation_env_unset_caught_by_strict_chars` — pins that `labels = { "${TEAM}" = "v" }` with `TEAM` unset fails at config-LOAD with the missing-env-var (or D-02 invalid-char) error mentioning `TEAM`.

The LBL-05 contract is now accurately documented in three load-bearing places (README prose, validator docstring, interpolate docstring) and pinned by tests that cannot regress silently. Verification gap CR-01 is closed.

## Task Commits

Each task was committed atomically (all on `phase-17-custom-docker-labels` worktree branch, `--no-verify` per parallel-executor convention):

1. **Task 1: README env-var interpolation paragraph rewrite** - `b0da597` (`docs(17-07): replace false 'keys never interpolated' README claim with accurate prose`)
2. **Task 2: Validator + interpolation docstring rewrites** - `2fc0443` (`docs(17-07): drop false 'Partially enforces LBL-05' validator claim; document whole-file textual interpolation`)
3. **Task 3: Two new integration tests pinning key-position contracts** - `4b567b1` (`test(17-07): pin env-set and env-unset key-position interpolation contracts`)

## Files Created/Modified

- `README.md` — env-var interpolation paragraph rewritten (single hunk in `### Labels` subsection, lines 254-282 post-edit); 17 insertions, 5 deletions; mermaid diagram + 3-row merge table + reserved-namespace + type-gate + size-limits + security-note paragraphs all untouched.
- `src/config/validate.rs` — `check_label_key_chars` docstring expanded from 4 lines to 22 lines with accurate D-02 description and both env-set / env-unset cases; function body byte-identical.
- `src/config/interpolate.rs` — `interpolate` docstring expanded from 6 lines to 16 lines with `WHOLE-FILE TEXTUAL REPLACEMENT` framing and downstream-consumer warning; function body byte-identical.
- `tests/v12_labels_interpolation.rs` — file-level `//!` doc comment extended with 7-line plan-17-07 reference block; two new `#[tokio::test]` functions appended after the existing value-side `#[ignore]` test (146 insertions, 0 deletions).
- `.planning/phases/17-custom-docker-labels-seed-001/17-07-SUMMARY.md` — this file (created).

## Decisions Made

- **Followed maintainer's locked CR-01 fix decision (Option A: relax docs) per `17-VERIFICATION.md` § Human Verification Required #2.** Option B (AST-aware pre-interpolation key check) would have required two-pass TOML parsing and structural rework of `interpolate::interpolate`; Option A is documentation-only and matches the existing partial enforcement. The plan was already authored under this decision; this executor honored it without re-litigation.
- **New integration tests intentionally NOT `#[ignore]`** — they exercise only the parse pipeline (`interpolate -> toml -> apply_defaults -> validate`) and require no Docker daemon. This lets them run on every CI push, unlike the existing `#[ignore]` value-side test that requires a live daemon.
- **Env-unset test assertion uses OR-disjunction over both error-message paths** (`missing environment variable` from interpolate.rs OR `invalid label keys` from validate.rs). The actual binary path takes the `MissingVar` branch first, but the assertion is robust against either outcome — and per the plan's `<behavior>` block this is the documented design (the README's "Recommended pattern" section names both fail-modes).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Adjusted `interpolate.rs` docstring line breaks so two grep-acceptance phrases match on a single line, and added a lowercase `whole-file textual replacement` reference**
- **Found during:** Task 2 verify step (initial grep checks failed for `whole-file textual replacement` and `does NOT respect TOML structure`)
- **Issue:** My first docstring draft had `it does` at line end and `NOT respect TOML structure` on the next line, so a single-line `grep -c 'does NOT respect TOML structure'` returned 0; the lowercase `whole-file textual replacement` form (Task 2 acceptance c3) was also absent — only the uppercase emphasis variant was present.
- **Fix:** Rewrote the second sentence so `does NOT respect TOML structure` appears intact on one line, and added a lowercase `whole-file textual replacement` clause earlier in the same sentence.
- **Files modified:** `src/config/interpolate.rs`
- **Verification:** All six Task 2 grep acceptance checks pass; `cargo build --lib` and `cargo test --lib check_label_key_chars / interpolate` pass; `cargo clippy --lib -- -D warnings` clean.
- **Committed in:** `2fc0443` (Task 2 commit — fix applied before commit)

**2. [Rule 3 - Blocking] Added a blank `//!` line between the file-level doc-comment bullet list and the trailing prose to satisfy `clippy::doc_lazy_continuation`, then ran `cargo fmt`**
- **Found during:** Task 3 final `cargo clippy --all-targets -- -D warnings` step
- **Issue:** The new `//!` block ended with a bullet list followed immediately by a 2-line continuation paragraph at the same indent level; clippy's `doc_lazy_continuation` lint flagged the continuation lines as "doc list items without indentation" (2 errors).
- **Fix:** Inserted a blank `//!` line between the bullet list and the trailing prose so they parse as separate paragraphs. Also ran `cargo fmt`, which collapsed the `let errors = result.expect_err(...)` declaration in the env-unset test onto a single line.
- **Files modified:** `tests/v12_labels_interpolation.rs`
- **Verification:** `cargo clippy --all-targets -- -D warnings` exits 0; `cargo fmt --check` exits 0; both new tests still pass.
- **Committed in:** `4b567b1` (Task 3 commit — both fixes applied before commit)

---

**Total deviations:** 2 auto-fixed (both Rule 3 — blocking issues directly caused by my edits)
**Impact on plan:** Both fixes were docstring-formatting touch-ups required to satisfy CI gates (clippy + rustfmt) that the plan's `<verification>` block explicitly requires. No semantic change; no scope creep.

## Issues Encountered

- **Worktree base mismatch on agent startup.** The worktree branch was based on commit `d93ae3ff` instead of the expected `72d907ed`. Resolved per the `<worktree_branch_check>` protocol with a `git reset --hard 72d907ed6dd3750ca2d5f82085fc47361a4cc6dd`; verified `HEAD` matches the expected base before proceeding.

## Verification Gap Closed

- **CR-01 (LBL-05 contract):** README + validator docstrings promised "Label KEYS are NEVER interpolated"; the binary silently violated this when the env var was set (`TEAM=ops cronduit check <toml-with-${TEAM}-as-key>` exited 0 instead of 1). Per maintainer's locked Option A decision, this plan replaces the false guarantee with accurate documentation of the whole-file textual pass and pins both env-set and env-unset key-position behaviors with new integration tests. The contract documented in README.md, `check_label_key_chars` docstring, and `interpolate::interpolate` docstring is now consistent with the implementation.
- Cross-references: `17-VERIFICATION.md` § gaps[0] (CR-01 truth/reason/missing/artifacts); `17-REVIEW.md` § Critical Issues > CR-01 (lines 73-132).

## User Setup Required

None — pure documentation + integration tests; no environment variables, secrets, or external services involved.

## Next Phase Readiness

- **Phase 17 gap-closure progress:** CR-01 is closed. Plans 17-08 (CR-02 LBL-04 error attribution fix) and 17-09 (REQUIREMENTS.md bookkeeping) remain.
- **Re-verification trigger:** Once 17-08 + 17-09 land and the wave merges, the verifier should re-run the phase-17 verification report; the previous `human_needed` status should flip to `pass` for the LBL-05 (CR-01) row.
- **rc.1 readiness:** No regressions; the new tests run under existing `--test-threads=1` convention and on CI without a Docker daemon.

## Self-Check: PASSED

All claimed files and commits verified to exist:

- `README.md` (modified) — `b0da597` includes 22 +/- lines in `### Labels`
- `src/config/validate.rs` (modified) — `2fc0443`
- `src/config/interpolate.rs` (modified) — `2fc0443`
- `tests/v12_labels_interpolation.rs` (modified) — `4b567b1`
- `.planning/phases/17-custom-docker-labels-seed-001/17-07-SUMMARY.md` (this file, created in this commit)

Commits in `git log --all`:

- `b0da597` ✓
- `2fc0443` ✓
- `4b567b1` ✓

Verification gates all green: `cargo build --lib && cargo build --tests && cargo clippy --all-targets -- -D warnings && cargo fmt --check && cargo test --test v12_labels_interpolation -- --test-threads=1 lbl_05_key_position` all exit 0.

---
*Phase: 17-custom-docker-labels-seed-001*
*Plan: 07 (gap closure for CR-01 — relax LBL-05 docs)*
*Completed: 2026-04-29*
