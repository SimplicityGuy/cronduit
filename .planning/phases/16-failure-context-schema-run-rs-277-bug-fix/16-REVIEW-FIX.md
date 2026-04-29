---
phase: 16-failure-context-schema-run-rs-277-bug-fix
fixed_at: 2026-04-28T00:00:00Z
review_path: .planning/phases/16-failure-context-schema-run-rs-277-bug-fix/16-REVIEW.md
iteration: 1
findings_in_scope: 4
fixed: 4
skipped: 0
status: all_fixed
---

# Phase 16: Code Review Fix Report

**Fixed at:** 2026-04-28
**Source review:** `.planning/phases/16-failure-context-schema-run-rs-277-bug-fix/16-REVIEW.md`
**Iteration:** 1
**Branch:** `phase-16-context`

**Summary:**
- Findings in scope (Critical + Warning): 4
- Fixed: 4
- Skipped: 0
- Info findings (IN-01..IN-05): out of scope per `fix_scope=critical_warning`; deferred for future hygiene pass.

All four warnings from the post-completion code review on Phase 16 have been
addressed. The phase deliverables (run.rs:301 wiring, image_digest /
config_hash columns, FCTX-07 helper, bulk backfill) remain functionally
correct; this iteration tightens the schema-design contract for the
inspect-failure path, locks the FOUND-14 wiring against silent regression in
standard CI, and untangles a documentation/SQL contract drift in the backfill
migrations.

TLS posture preserved: `cargo tree -i openssl-sys` returns empty after fixes
(rustls everywhere). Full lib + integration test suite (`cargo test --tests`)
green: 197 lib tests + integration tests all pass with zero failures. Clippy
clean (`cargo clippy --lib --tests -- -D warnings`).

## Fixed Issues

### WR-01: Inspect-failure path stores empty-string image_digest, not NULL

**Files modified:** `src/scheduler/docker.rs`
**Commit:** `40b67db`
**Applied fix:**
- Changed local `image_digest` binding in `execute_docker` from `String` to
  `Option<String>`. The success arm now uses
  `info.image.filter(|s| !s.is_empty())` so an inspect that returns
  `info.image == Some("")` collapses to `None`. The error arm returns `None`
  directly (was `String::new()`).
- Removed the `Some(image_digest)` wrapper at the success-path return site;
  the value flows through directly because it is already `Option<String>`.
- Added a structural unit test
  `wr01_inspect_failure_yields_none_not_empty_string` in `docker.rs::tests`
  that asserts all four inspect-shape variants (error, `info.image == None`,
  `info.image == Some("")`, real digest) collapse correctly, and that the
  resulting `DockerExecResult.image_digest.as_deref()` is `None` for the
  inspect-failure path — locking the schema-design contract that
  `image_digest IS NULL` means "no digest captured" (never `''`).

Schema invariant preserved end-to-end: an inspect failure now writes SQL
NULL into `job_runs.image_digest` instead of the empty string, so any
downstream consumer (FCTX-07 query, Phase 21 panel) that filters on
`image_digest IS NOT NULL` correctly classifies the row as uncaptured.

### WR-02: Load-bearing bug-fix regression test gated behind #[ignore]

**Files modified:** `src/scheduler/run.rs`
**Commit:** `d49ac60`
**Applied fix:**
- Extracted a tiny pure-Rust helper `finalize_args_from_docker_result(&DockerExecResult) -> (Option<String>, Option<String>)` that owns the FOUND-14 wiring assignment.
- Replaced the inline `container_id_for_finalize = ...` / `image_digest_for_finalize = ...` assignment in the docker arm of `continue_run` with a single call to the helper. Behavior is byte-identical at the call site.
- Added two `#[test]` cases in `run.rs::tests` (no Docker daemon required, run in standard `cargo test`):
  * `wr02_finalize_args_wiring_locks_found14_against_silent_regression` — constructs a synthetic `DockerExecResult` with distinguishable values (`container_id = "abc123-real-container-id"` and `image_digest = "sha256:deadbeef"`) and asserts the helper returns each value in the right tuple position. Includes an explicit `assert!(!cid.starts_with("sha256:"))` so a future swap that re-introduces FOUND-14 fails with a clear message.
  * `wr02_finalize_args_wiring_passes_none_through_unchanged` — locks the WR-01 None-vs-empty-string contract end-to-end across both the inspect-failure shape (real cid, None digest) and the pre-create-container failure shape (both None).

A future copy/paste swap that re-introduces FOUND-14 must now mutate either the helper or the assertions — both noisy in code review and failing standard CI immediately. Closes the standard-CI gap that previously required a Docker daemon.

### WR-03: BACKFILL_CUTOFF_RFC3339 marker set to today, mis-classifying same-day rows

**Files modified:** `migrations/sqlite/20260429_000007_config_hash_backfill.up.sql`, `migrations/postgres/20260429_000007_config_hash_backfill.up.sql`
**Commit:** `2c371b3`
**Applied fix:**
- Bumped marker from `2026-04-27T00:00:00Z` to `2026-04-28T00:00:00Z` on both backends. The new marker sits strictly after any plausible v1.1 finish time on the deploy day, leaving no false-negative window for Phase 21's heuristic.
- Comment block explicitly documents the bump (linking to WR-03) so a future reader understands why the marker is one day in the future.

No tests reference the literal cutoff date — verified via
`grep -rn "BACKFILL_CUTOFF\|2026-04-27T00:00:00Z" src/ tests/ migrations/`.
The `tests/v12_fctx_config_hash_backfill.rs` and `tests/migrations_idempotent.rs`
suites both pass after the change.

### WR-04: Backfill migration comment describes a heuristic the SQL does not implement

**Files modified:** `migrations/sqlite/20260429_000007_config_hash_backfill.up.sql`, `migrations/postgres/20260429_000007_config_hash_backfill.up.sql`
**Commit:** `2c371b3` (committed alongside WR-03 — same files)
**Applied fix:**
- Replaced the misleading "Heuristic: rows where `end_time < BACKFILL_CUTOFF_RFC3339` ..." paragraph with a clearly delineated "WR-04 / cross-phase contract (NOT a SQL filter)" block on both backends. The new wording:
  1. States explicitly that the SQL does NOT filter on `end_time` — the UPDATE backfills every row where `config_hash IS NULL`.
  2. Identifies the marker as a forward-looking documentation token, not a predicate on this UPDATE.
  3. Documents Phase 21's heuristic as a query-side rule (`end_time < BACKFILL_CUTOFF_RFC3339 AND config_hash IS NOT NULL`), separate from the migration's own behavior.
- The SQL itself is unchanged (still `UPDATE job_runs SET config_hash = ... WHERE config_hash IS NULL;`) — only the comment changed.

A reader of the migration file now sees an accurate spec of what the SQL does versus what downstream consumers do with the marker, eliminating the previous "comment reads like a SQL spec but is actually a cross-phase contract" footgun.

## Skipped Issues

None — all four in-scope warnings were applied cleanly.

## Verification

- **Tier 1 (re-read):** every modified file re-read after edit; fix text confirmed present, surrounding code intact.
- **Tier 2 (build + tests):**
  - `cargo build --tests` — clean.
  - `cargo clippy --lib --tests -- -D warnings` — clean.
  - `cargo test --lib` — 197 passed, 0 failed.
  - `cargo test --tests` — full integration tier green (Docker-daemon-gated tests still `#[ignore]`-marked as expected; non-Docker tests all pass including `v12_fctx_config_hash_backfill`, `migrations_idempotent`, `scheduler::docker`, `scheduler::run`).
- **Tier 3 (project gates):**
  - `cargo tree -i openssl-sys` — empty (rustls posture preserved).

## Out-of-scope Info findings (acknowledged, not addressed)

- **IN-01:** stale comment in `tests/docker_executor.rs:113-115` (deferred — pure docs cleanup).
- **IN-02:** fragile fixture format string in `tests/v12_fctx_streak.rs:67-84` (deferred — dormant, only triggers if test seed size grows past 60).
- **IN-03:** discarded `_image_digest` value at `src/scheduler/docker.rs:139` (deferred — code is correct, comment-only readability nit).
- **IN-04:** `#[allow(dead_code)]` on `FailureContext` fields (deferred — accepted forward-engineering pattern; Phase 18 will consume).
- **IN-05:** duplicate field documentation between `DbRun` and `DbRunDetail` (deferred — pure style note).

These can be folded into a future hygiene pass or the Phase 21 / Phase 18 work that touches the same files.

---

_Fixed: 2026-04-28_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
