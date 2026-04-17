---
phase: 11-per-job-run-numbers-log-ux-fixes
plan: 01
subsystem: database
tags: [benchmark, performance-gate, sqlite, sqlx, rust, insert-batch, option-a, phase-11]

# Dependency graph
requires:
  - phase: 11-00
    provides: tests/v11_log_dedupe_benchmark.rs Wave-0 #[ignore] stub + tests/common/v11_fixtures.rs (setup_sqlite_with_phase11_migrations, seed_test_job, seed_running_run, make_test_batch).
provides:
  - T-V11-LOG-02 p95 insert-latency benchmark for 64-line batch on in-memory SQLite (hard-asserts p95 < 50ms).
  - Empirical clearance of CONTEXT.md D-02 gate — Option A (insert-then-broadcast with RETURNING id) path is viable; no flip to Option B.
  - Baseline p95 measurement for downstream Plan 11-07's task-4 benchmark update (currently passes on dev hardware with ~40-75x margin under the 50ms budget).
affects: [11-02, 11-03, 11-04, 11-05, 11-06, 11-07, 11-08, 11-09, 11-10, 11-11, 11-12, 11-13]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Performance-gate spike test: sorted-percentile p95 assertion inside `#[tokio::test]` with warmup iterations, mean/p50/p95/p99 reporting via `eprintln!` + `--nocapture` for PR-visible numbers, and failure message that names the decision doc (CONTEXT.md D-02) and the fallback path (Option B)."
    - "Warmup-then-measure: 5 discarded iterations let SQLite WAL/pragmas settle before the 100-iter timed loop, mirroring standard microbench hygiene."

key-files:
  created:
    - .planning/phases/11-per-job-run-numbers-log-ux-fixes/11-01-SUMMARY.md
  modified:
    - tests/v11_log_dedupe_benchmark.rs

key-decisions:
  - "D-02 gate CLEARED with p95 ≈ 0.7-1.3ms on Darwin/M-series (release profile) — 40-75x under the 50ms budget. Phase 11 continues on Option A (insert-then-broadcast with RETURNING id). No replan, Plan 11-02 unblocked."
  - "Dropped the plan's draft `let _ = insert_log_batch(...)` lines in the warmup loop because `clippy::let_unit_value` under `-D warnings` rejects binding a `()` return. Semantically identical after fix."

patterns-established:
  - "Spike-gate test pattern: performance-gate tests live in `tests/v11_*.rs`, run under `--release`, print summary stats for PR visibility, and self-document the decision doc reference in the assert message."

requirements-completed: [UI-20]

# Metrics
duration: ~10min
completed: 2026-04-16
---

# Phase 11 Plan 01: T-V11-LOG-02 Benchmark — Option A Gate Cleared Summary

**p95 insert latency on in-memory SQLite measured at 0.7-1.3ms for a 64-line batch — 40-75× under the 50ms budget, clearing the D-02 decision gate and confirming the Option A (insert-then-broadcast with RETURNING id) path for the rest of Phase 11.**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-04-17T00:02:00Z (approx.)
- **Completed:** 2026-04-17T00:12:00Z (approx.)
- **Tasks:** 1 (single-task plan)
- **Files created:** 1 (this SUMMARY.md)
- **Files modified:** 1 (tests/v11_log_dedupe_benchmark.rs)

## Accomplishments

- Replaced Plan 11-00's Wave-0 `#[ignore]` stub in `tests/v11_log_dedupe_benchmark.rs` with the full `p95_under_50ms` benchmark body per the plan's specification.
- Benchmark exercises `cronduit::db::queries::insert_log_batch` via the canonical `v11_fixtures` path (in-memory SQLite, seeded job + running run, 64-line batches), with 5 warmup iterations and 100 timed iterations, capturing per-iter duration in µs and hard-asserting `p95 < 50_000`.
- Benchmark printed full mean/p50/p95/p99 summary via `--nocapture` for PR description.
- Empirical D-02 gate evaluation recorded (see Measurements).

## Measurements

Executed with the exact command from VALIDATION.md row 11-01-02:
```
cargo test --release --test v11_log_dedupe_benchmark p95_under_50ms -- --nocapture
```

Two consecutive runs on this worktree (Darwin 25.3.0, Apple Silicon, release profile, in-memory SQLite):

| Run | mean | p50   | p95     | p99     | verdict            |
| --- | ---- | ----- | ------- | ------- | ------------------ |
| 1   | 476us | 459us | **678us** | 855us  | PASS (73x margin)  |
| 2   | 678us | 617us | **1247us** | 1499us | PASS (40x margin)  |

**Budget:** p95 < 50,000 µs (50 ms).
**Observed:** p95 between ~680 µs and ~1.25 ms across runs. Both runs clear the gate by >40x.

**Decision:** Option A path **remains viable**. Phase 11 continues to Plan 11-02 (schema migration for `job_run_number`). No flip to Option B (monotonic `seq: u64` column) required.

Note on CI portability: CI runners (GitHub Actions linux/amd64 shared runners) are typically 2-4x slower than Darwin/M-series on disk-sensitive workloads, which would put the upper-bound p95 at ~5 ms — still an order of magnitude under the budget. Plan 11-07 re-runs the benchmark against the updated `insert_log_batch` signature (returning `Vec<i64>`) and will catch any regression introduced by the RETURNING id change.

## Task Commits

Each task was committed atomically:

1. **Task 1: T-V11-LOG-02 benchmark harness — p95 < 50ms gate** — `53f9a2f` (test)

## Files Created/Modified

- `tests/v11_log_dedupe_benchmark.rs` (MODIFIED) — Wave-0 stub replaced with the full `p95_under_50ms` benchmark body. `#[ignore]` attribute removed; `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`; 5-iter warmup + 100-iter timed loop; sorted-percentile assertion; mean/p50/p95/p99 summary printed to stderr.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-01-SUMMARY.md` (NEW) — this file.

## Decisions Made

1. **D-02 gate CLEARED — Option A path retained.** Two benchmark runs produced p95 = 678 µs and p95 = 1247 µs respectively, both 40-75x under the 50 ms budget. No flip to Option B required. Plan 11-02 is unblocked.

2. **Dropped the plan's draft `let _ = insert_log_batch(...)` in the warmup loop.** `cronduit`'s CI gate is `cargo clippy --all-targets --all-features -- -D warnings`, which rejects `let_unit_value` on a `()`-returning call. The plan's draft body had `let _ = insert_log_batch(&pool, run_id, &batch).await.unwrap();` in the warmup loop; this was rewritten to drop the `let _ =`. Semantically identical (both forms run the future to completion and unwrap the `Result`), and now clippy-clean.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Tailwind binary missing — release build panic**
- **Found during:** Task 1 (first `cargo test --release ...` invocation)
- **Issue:** The plan's required verify command (`cargo test --release --test v11_log_dedupe_benchmark p95_under_50ms -- --nocapture`) failed with a build-script panic: `Tailwind binary not found at bin/tailwindcss — refusing to build release without compiled CSS`. `build.rs` hard-panics in release builds if `bin/tailwindcss` is missing (policy guard to prevent unstyled images). The worktree is a fresh git checkout with `bin/` gitignored, so the binary wasn't present.
- **Fix:** Ran `just tailwind` once to download `tailwindcss` v4.2.2 into `bin/tailwindcss` and regenerate `assets/static/app.css`. After this, `cargo test --release` completed successfully.
- **Files modified:** None committed. `bin/tailwindcss` is `.gitignore`d. `assets/static/app.css` was byte-level changed by re-minification, but since it's a cosmetic re-build of checked-in content (not a task artifact) I reverted it with `git checkout -- assets/static/app.css` to keep the commit scope pure. Subsequent `just tailwind` invocations by any developer regenerate the identical output.
- **Verification:** `cargo test --release --test v11_log_dedupe_benchmark p95_under_50ms -- --nocapture` passes.
- **Committed in:** N/A — no lasting changes.

**2. [Rule 3 - Blocking] Clippy `-D warnings` rejects `let _ = insert_log_batch(...).unwrap()` (let_unit_value lint)**
- **Found during:** Task 1 (first `cargo clippy --tests --test v11_log_dedupe_benchmark -- -D warnings`)
- **Issue:** The plan's draft warmup loop body reads `let _ = insert_log_batch(&pool, run_id, &batch).await.unwrap();`. Because `insert_log_batch` currently returns `anyhow::Result<()>`, the RHS after `.unwrap()` is `()`, which trips `clippy::let_unit_value` under the project's `-D warnings` policy.
- **Fix:** Rewrote that single warmup line to `insert_log_batch(&pool, run_id, &batch).await.unwrap();`. Semantically identical; now clippy-clean. Note: once Plan 11-07 flips the return type to `anyhow::Result<Vec<i64>>`, this lint goes away because `unwrap()` then returns `Vec<i64>` (non-unit) — but the `let _ =` is still unnecessary so the simpler form remains correct.
- **Files modified:** `tests/v11_log_dedupe_benchmark.rs` (one line in the warmup loop).
- **Verification:** `cargo clippy --tests --test v11_log_dedupe_benchmark -- -D warnings` passes; `cargo test --release ...` still passes with the full percentile summary.
- **Committed in:** `53f9a2f` (Task 1 commit).

---

**Total deviations:** 2 auto-fixed (both Rule 3 - Blocking).
**Impact on plan:** Both deviations were infrastructure/style glitches — neither changed semantics, scope, or the plan's decision boundary. The plan's intent (p95 < 50ms gate) and the benchmark's behavior are exactly as written.

## Threat Flags

None — the benchmark uses in-memory SQLite with synthetic data and introduces no new attack surface. Plan's threat model (T-11-01-01 "n/a") remains accurate.

## Issues Encountered

None beyond the two auto-fixed deviations above.

## TDD Gate Compliance

The plan has `tdd="true"` on Task 1, but Task 1 is architecturally a spike-gate — it measures an already-shipped production function (`insert_log_batch`) rather than driving new production code. The Wave-0 `#[ignore]` stub satisfied the nyquist-rule RED gate; the real benchmark body that gates Option A is the "test" step of this plan. There is no separate `feat()` GREEN commit because no production code shipped in this plan — the function under measurement was already in place. Downstream Plan 11-07 will produce the `feat()` GREEN commit when it changes `insert_log_batch`'s return type to `Vec<i64>` and updates this benchmark accordingly.

Gate sequence for this plan:
- RED: `fa26618` + `783e9ca` (Wave-0 stub from Plan 11-00 — ignored test representing pre-implementation state).
- GREEN-equivalent: `53f9a2f` (benchmark body that passes against the already-shipped `insert_log_batch`).
- REFACTOR: none required.

This matches the phase's design pattern (Plan 10 D-14 Stop spike used the same flow).

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Plan 11-02 unblocked.** D-02 gate is green; proceed with the Option A schema migration (add `job_run_number` as nullable INTEGER to `job_runs`).
- **Plan 11-07 will update this benchmark.** When `insert_log_batch` changes signature to `anyhow::Result<Vec<i64>>`, Plan 11-07 Task 4 updates the warmup/timed-loop bodies accordingly (they'll then bind the returned `Vec<i64>` or destructure it) and re-verifies the p95 budget holds.
- **CI portability concern recorded.** GitHub Actions linux/amd64 runners are typically 2-4x slower than Apple Silicon on disk-sensitive workloads. Worst-case projected CI p95 is ~5 ms, still an order of magnitude under the 50 ms budget. If the benchmark flakes on CI, the first response is to inspect the runner — not to flip the decision.

## Self-Check: PASSED

**Files verified on disk:**
- tests/v11_log_dedupe_benchmark.rs — FOUND (benchmark body replaces Wave-0 stub)
- .planning/phases/11-per-job-run-numbers-log-ux-fixes/11-01-SUMMARY.md — FOUND (this file)

**Commits verified:**
- 53f9a2f — FOUND (Task 1 benchmark body)

**Build gates verified:**
- `cargo check --tests` — CLEAN (0 errors, ignoring stale cargo-warning cache noise about tailwindcss)
- `cargo clippy --tests --test v11_log_dedupe_benchmark -- -D warnings` — CLEAN
- `cargo test --release --test v11_log_dedupe_benchmark p95_under_50ms -- --nocapture` — PASS (p95 = 678 µs run #1, 1247 µs run #2)

**Decision gate verified:**
- Both release runs have p95 < 50_000 µs by >40x margin.
- Phase 11 continues on Option A. `## PHASE REPLAN REQUIRED — Option B path` is NOT triggered.

---
*Phase: 11-per-job-run-numbers-log-ux-fixes*
*Completed: 2026-04-16*
