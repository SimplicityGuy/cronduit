---
phase: 16
slug: failure-context-schema-run-rs-277-bug-fix
status: verified
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-27
audited: 2026-04-28
---

# Phase 16 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: `16-RESEARCH.md` § Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` / `cargo nextest` (sqlx + tokio + testcontainers integration); idiomatic Rust 2024 / rust-version 1.94.1 |
| **Config file** | `Cargo.toml` (workspace), `tests/common/` (shared fixtures) |
| **Quick run command** | `just test` |
| **Full suite command** | `just nextest` |
| **Schema parity** | `just schema-diff` |
| **Estimated runtime** | ~60 seconds (quick), ~6 minutes (full incl. Postgres testcontainers) |

---

## Sampling Rate

- **After every task commit:** Run `just test` (or targeted `cargo test --test <NAME>` for the specific test file the task added/touched).
- **After every plan wave:** Run `just nextest`.
- **Before `/gsd-verify-work`:** Full suite (incl. Postgres testcontainers) must be green; `just schema-diff` must show parity; `just grep-no-percentile-cont` must exit 0.
- **Max feedback latency:** 60 seconds for the unit/integration loop; 6 minutes for the full Postgres-included suite.

---

## Per-Task Verification Map

> Filled by `gsd-planner` during planning (Plans 16-01..16-06 each emit per-task rows).
> Plan 16-04 was split into 16-04a (queries.rs changes) + 16-04b (callers + recipe + gate)
> during the iteration-1 revision loop — the 7 plans below match the on-disk PLAN.md files.
> Pattern from `16-RESEARCH.md` § Validation Architecture → "Phase Requirements → Test Map":

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 16-01-* | 01 | 1 | FCTX-04, FOUND-14 | T-16-01-01..04 | accept (rationale in 16-SECURITY.md) | integration | `just schema-diff` + `cargo test --test migrations_idempotent` + `cargo test --test v12_fctx_config_hash_backfill` | ✅ schema_parity (7/7), ✅ migrations_idempotent (1/1), ✅ v12_fctx_config_hash_backfill (4/4) | ✅ green |
| 16-02-* | 02 | 1 | FOUND-14 | T-16-02-01..02 | accept | unit | `cargo test --lib docker::tests::test_docker_exec_result_debug` + `cargo test --lib docker::tests::wr01_inspect_failure_yields_none_not_empty_string` | ✅ both pass; WR-01 fix added the empty-string→None invariant test (commit 40b67db) | ✅ green |
| 16-03-* | 03 | 2 | FOUND-14 | T-16-03-01 (mitigate, verified by HUMAN-UAT) + T-16-03-02 (accept) | mitigate (run.rs:305-306 fix) | integration + unit (post-fix WR-02) | `cargo test --test v12_run_rs_277_bug_fix` (3 pass + 2 #[ignore] Docker-gated) + `cargo test --lib run::tests::wr02_finalize_args_wiring_*` (2 unit tests added by WR-02 fix in commit d49ac60 lock FOUND-14 wiring without Docker) | ✅ all green; WR-02 closed the standard-CI regression-coverage gap | ✅ green |
| 16-04a-* | 04a | 2 | FOUND-14, FCTX-04 | T-16-04a-01..03 | accept | unit (compile gate) | `cargo build --workspace` (signature transition completed in 16-04b) | ✅ green at wave-2 close | ✅ green |
| 16-04b-* | 04b | 2 | FOUND-14, FCTX-04 | T-16-04b-01..02 | accept | integration | full gate: `cargo build` + `just test` (388 nextest + 197 lib) + `just schema-diff` + `just grep-no-percentile-cont` + `just uat-fctx-bugfix-spot-check` (recipe added by 16-04b-T4) | ✅ recipe present (path mismatch noted in follow-up todo); full gate green | ✅ green |
| 16-05-* | 05 | 3 | FCTX-04, FCTX-07 | T-16-05-01..03 | accept | integration | `cargo test --test v12_fctx_streak` (5 streak scenarios + T-V12-FCTX-03 / T-V12-FCTX-04 write-site asserts) | ✅ 7/7 pass | ✅ green |
| 16-06-* | 06 | 3 | FCTX-07 | T-16-06-01 | accept | integration | `cargo test --test v12_fctx_explain` (SQLite + Postgres EXPLAIN plans assert idx_job_runs_job_id_start) | ✅ SQLite (1/1) + Postgres (1 ignored, testcontainers gate) | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

New test files the planner MUST create (each Plan creates the file in its first task before downstream tasks depend on it):

- [x] `tests/v12_fctx_config_hash_backfill.rs` — covers FCTX-04 backfill behavior (Plan 16-01) — **landed in commit 275db7d, 4/4 pass**
- [x] `tests/v12_run_rs_277_bug_fix.rs` — covers FOUND-14 observable using testcontainers (Plan 16-03) — **landed in commit a4a4fd1, 3 pass + 2 #[ignore]**
- [x] `tests/v12_fctx_streak.rs` — covers FCTX-07 + FCTX-04 write-site (Plans 16-04 + 16-05) — **landed in commit bc2c68b, 7/7 pass**
- [x] `tests/v12_fctx_explain.rs` — covers FCTX-07 EXPLAIN-plan assertions on both backends (Plan 16-06) — **landed in commit 603652c, 1 pass + 1 #[ignore]**

Existing infrastructure that does NOT need extension:
- `tests/schema_parity.rs` — fully dynamic introspection (RESEARCH § E confirmed). New TEXT columns auto-detected.
- `tests/migrations_idempotent.rs` — existing `pool.migrate().await; pool.migrate().await` covers re-run safety for the 3 new migration files.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Operator-visible spot-check: after a v1.2 docker run completes, `job_runs.container_id` is the real ID (not `sha256:...`) | FOUND-14 (Success Criterion 1) | UAT spot-check beyond automated test — the bug is operator-observable; maintainer should confirm on a live homelab DB once. | `just uat-fctx-bugfix-spot-check` (planner adds this recipe in Plan 16-04 if not present); recipe runs a real docker job and dumps `SELECT container_id FROM job_runs ORDER BY id DESC LIMIT 1` for visual confirmation. |

*All other phase behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (4 new test files all landed)
- [x] No watch-mode flags (`cargo test --watch` / `cargo-watch -x test` not used in plans)
- [x] Feedback latency < 60s for quick run (`just test` ~25s), < 6 min for full suite (`just nextest` ~6min)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** verified 2026-04-28 — all 7 tasks COVERED, 0 PARTIAL, 0 MISSING.

---

## Validation Audit 2026-04-28

| Metric | Count |
|--------|-------|
| Tasks total | 7 |
| COVERED | 7 |
| PARTIAL | 0 |
| MISSING | 0 |
| Resolved | 0 (no gaps to fill) |
| Escalated | 0 |

**Audit notes:**

- All 4 Wave 0 test files landed in their planned waves; all green at wave-2 / wave-3 post-merge gates.
- The post-completion code-review-fix loop (commit `d49ac60`) added `wr02_finalize_args_wiring_*` unit tests in `src/scheduler/run.rs` to lock the FOUND-14 wiring against silent regression in standard CI (closing the WR-02 advisory finding from `16-REVIEW.md`). This represents BONUS coverage beyond the original VALIDATION.md plan — strictly additive.
- The post-completion code-review-fix loop (commit `40b67db`) added `wr01_inspect_failure_yields_none_not_empty_string` unit test in `src/scheduler/docker.rs` to lock the inspect-failure→None invariant (closing WR-01).
- Manual-Only spot check (`just uat-fctx-bugfix-spot-check`) executed by maintainer 2026-04-28 against `cronduit.db` filtered to `job_type='docker'` — three consecutive runs (id=114/116/119) all show real 64-char-hex container_ids and `sha256:...` image_digests. PASS recorded in `16-HUMAN-UAT.md` with date+validator.
- Recipe-path mismatch (`uat-fctx-bugfix-spot-check` targets `cronduit.dev.db` while `cronduit run` writes to `cronduit.db`) is logged as follow-up todo `20260428T124050-just-recipe-db-path-mismatch.md` — non-blocking.
