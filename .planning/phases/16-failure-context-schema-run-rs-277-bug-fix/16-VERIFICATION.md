---
phase: 16-failure-context-schema-run-rs-277-bug-fix
verified: 2026-04-27T00:00:00Z
human_validated: 2026-04-28
status: passed
score: 3/3 success criteria verified (with 1 advisory regression-coverage gap)
overrides_applied: 0
human_verification:
  - test: "FOUND-14 spot check on a real homelab dev DB"
    expected: "After firing a v1.2 docker job, `just uat-fctx-bugfix-spot-check` prints a row whose container_id is a real Docker container ID (12-char hex prefix or 64-char hex) and image_digest starts with sha256:; container_id MUST NOT start with sha256:."
    why_human: "16-HUMAN-UAT.md mandates maintainer validation per project rules D-12 (every UAT step uses just) + D-13 (Claude does NOT mark UAT passed from its own runs). Automated docker-gated tests exist (#[ignore] in v12_run_rs_277_bug_fix.rs) but require a Docker daemon and are not run in standard CI; the maintainer's local-DB inspection is the canonical sanity check operators will run after upgrade."
    result: "PASS — maintainer validated 2026-04-28 against cronduit.db filtered to job_type='docker'. Three consecutive spot-check-docker runs (id=114/116/119) all show real 64-char-hex container_ids and sha256:5b10f432... image_digests. Recipe-path mismatch (recipe targets cronduit.dev.db but daemon writes to cronduit.db) logged as follow-up todo 20260428T124050-just-recipe-db-path-mismatch.md."
---

# Phase 16: Failure-Context Schema + run.rs:277 Bug Fix Verification Report

**Phase Goal:** Fix the silent v1.1 `job_runs.container_id` regression and land the per-run schema columns + streak query helper that the webhook payload (Phase 18) and failure-context UI (Phase 21) both consume.

**Verified:** 2026-04-27
**Status:** human_needed (all automated criteria satisfied; FOUND-14 operator-observable requires maintainer UAT per HUMAN-UAT.md)
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (Roadmap Success Criteria)

| #   | Truth                                                                                                                                                                                                                                                                                                                              | Status     | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| --- | --- | --- | --- |
| SC1 | An operator inspecting a v1.2 docker job run via the database sees `job_runs.container_id` populated with the real Docker container ID (not a `sha256:...` image digest); historical v1.1 rows age out via the Phase 6 retention pruner.                                                                          | ✓ VERIFIED | Bug fix landed at `src/scheduler/run.rs:305-306`: `container_id_for_finalize = docker_result.container_id.clone()` (was `.image_digest`); parallel `image_digest_for_finalize = docker_result.image_digest.clone()` captures the digest. `DockerExecResult` (src/scheduler/docker.rs:63-75) carries both `image_digest: Option<String>` (L67) and `container_id: Option<String>` (L75). `finalize_run` now writes both columns at queries.rs:444-491 (L460 SQLite, L475 Postgres). HUMAN-UAT spot check required for operator-observable. NOTE: WR-02 — the regression test coverage is structurally weak (see Anti-Patterns below). |
| SC2 | An operator viewing two consecutive runs of the same job after a hot reload sees distinct `job_runs.config_hash` values when the underlying TOML actually changed (per-RUN column, not the per-JOB proxy).                                                                                                                          | ✓ VERIFIED | `insert_running_run` signature carries `config_hash: &str` as 4th positional (queries.rs:372-377); both backend INSERTs include `config_hash` in column list and bind ?5/$5 (L391-401, L416-426). Production callers `src/scheduler/run.rs:86` and `src/web/handlers/api.rs:82` pass `&job.config_hash`. Regression test `tests/v12_fctx_streak.rs::reload_changes_config_hash` (T-V12-FCTX-04) executes two `insert_running_run` calls with distinct values and asserts they round-trip distinctly — passes locally (7/7). |
| SC3 | An operator inspecting `EXPLAIN QUERY PLAN` for `get_failure_context(job_id)` on both SQLite and Postgres sees indexed access on `job_runs.job_id + start_time`; the function returns `streak_position`, `consecutive_failures`, `last_success_run_id`, `last_success_image_digest`, `last_success_config_hash` from a single SQL query. | ✓ VERIFIED | `get_failure_context` at queries.rs:681 uses two CTEs joined `LEFT JOIN last_success ON 1=1` (L706, L732) with epoch sentinel `'1970-01-01T00:00:00Z'` (L697, L723). `FailureContext` (queries.rs:636-657) carries `consecutive_failures: i64` + 3 last_success_* Option fields. `streak_position` is intentionally NOT a struct field per D-06 — caller-side computed from `consecutive_failures` (acceptable per locked decision; the roadmap SC3 prose lists it but D-06 makes it derived; `streak_position` doc comment exists at queries.rs:629). EXPLAIN test `tests/v12_fctx_explain.rs::explain_uses_index_sqlite` passes (asserts `idx_job_runs_job_id_start` substring + rejects bare `SCAN job_runs`); Postgres counterpart `#[ignore]`-gated, mirrors v13_timeline_explain.rs precedent. Single-query (one fetch_one per call) confirmed. |

**Score:** 3/3 success criteria verified

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | ----------- | ------ | ------- |
| `migrations/sqlite/20260427_000005_image_digest_add.up.sql` | SQLite `ALTER TABLE job_runs ADD COLUMN image_digest TEXT` | ✓ VERIFIED | Present (L18); no `IF NOT EXISTS` (SQLite-incompatible). |
| `migrations/postgres/20260427_000005_image_digest_add.up.sql` | Postgres `ALTER TABLE job_runs ADD COLUMN IF NOT EXISTS image_digest TEXT` | ✓ VERIFIED | Present (L17). |
| `migrations/sqlite/20260428_000006_config_hash_add.up.sql` | SQLite `ALTER TABLE job_runs ADD COLUMN config_hash TEXT` | ✓ VERIFIED | Present (L24). Note: filename uses `20260428` not `20260427` per Plan 16-01 Rule 1 deviation (sqlx splitn on `_` requires unique date prefixes). |
| `migrations/postgres/20260428_000006_config_hash_add.up.sql` | Postgres `ALTER TABLE job_runs ADD COLUMN IF NOT EXISTS config_hash TEXT` | ✓ VERIFIED | Present (L18). |
| `migrations/sqlite/20260429_000007_config_hash_backfill.up.sql` | Bulk UPDATE backfill + `BACKFILL_CUTOFF_RFC3339` marker | ✓ VERIFIED | Marker present at L3: `BACKFILL_CUTOFF_RFC3339: 2026-04-27T00:00:00Z`. UPDATE statement: `UPDATE job_runs SET config_hash = (SELECT j.config_hash FROM jobs j WHERE j.id = job_runs.job_id) WHERE config_hash IS NULL;`. |
| `migrations/postgres/20260429_000007_config_hash_backfill.up.sql` | Same on Postgres | ✓ VERIFIED | Same marker + same UPDATE shape. |
| `src/scheduler/docker.rs::DockerExecResult.container_id` | `pub container_id: Option<String>` field | ✓ VERIFIED | Present at L75; `image_digest: Option<String>` at L67. |
| `src/scheduler/run.rs:305-306` | Bug fix reading `.container_id` not `.image_digest` | ✓ VERIFIED | L305: `container_id_for_finalize = docker_result.container_id.clone()`. L306: `image_digest_for_finalize = docker_result.image_digest.clone()`. Parallel locals declared at L234-235. |
| `src/scheduler/run.rs:348-358` finalize_run call site | 8 positional args including `image_digest_for_finalize.as_deref()` | ✓ VERIFIED | New 8th positional present at L361. |
| `src/db/queries.rs::finalize_run` | 8-arg signature with `image_digest: Option<&str>` | ✓ VERIFIED | Signature L444-453; doc comment cites Phase 16 FOUND-14; `#[allow(clippy::too_many_arguments)]` on the function. |
| `src/db/queries.rs::insert_running_run` | 4-arg signature with `config_hash: &str` | ✓ VERIFIED | Signature L372-377. Both backends INSERT `config_hash` at position 5 (L392-401 SQLite, L416-426 Postgres). |
| `src/db/queries.rs::DbRun.image_digest / DbRun.config_hash` | Both Option<String> fields with Phase 16 doc comments | ✓ VERIFIED | L592, L597. |
| `src/db/queries.rs::DbRunDetail.image_digest / DbRunDetail.config_hash` | Both fields | ✓ VERIFIED | L617, L622. |
| `src/db/queries.rs::FailureContext` | 4-field struct with `#[derive(Debug, Clone)]` | ✓ VERIFIED | Struct L636-657 (4 fields: consecutive_failures: i64, last_success_run_id: Option<i64>, last_success_image_digest: Option<String>, last_success_config_hash: Option<String>); `#[derive(Debug, Clone)]` at L635. Each field carries `#[allow(dead_code)]` per Plan 16-05 + "Phase 18+ consumes" doc note. |
| `src/db/queries.rs::get_failure_context` | `pub async fn get_failure_context(pool, job_id) -> Result<FailureContext>` with CTE shape | ✓ VERIFIED | Function L681-755 with both backend arms; CTE shape matches D-05; `LEFT JOIN last_success ON 1=1` at L706/L732; epoch sentinel `'1970-01-01T00:00:00Z'` at L697/L723. |
| `tests/v12_fctx_config_hash_backfill.rs` | Backfill correctness + idempotency + orphan-handling tests | ✓ VERIFIED | 4 tests pass (4/4). |
| `tests/v12_run_rs_277_bug_fix.rs` | Bug-fix regression tests | ⚠️ ORPHANED | File exists with 4 tests; 2 non-ignored pass. **Caveat (WR-02):** non-ignored tests call `finalize_run` directly — they do NOT exercise `run.rs:305-306` wiring. See Anti-Patterns below. |
| `tests/v12_fctx_streak.rs` | 5 streak scenarios + 2 FCTX-04 write-site tests | ✓ VERIFIED | 7/7 tests pass. |
| `tests/v12_fctx_explain.rs` | SQLite + Postgres EXPLAIN tests | ✓ VERIFIED | SQLite test passes; Postgres `#[ignore]`-gated (matches v13 precedent). Asserts `idx_job_runs_job_id_start`. |
| `justfile::uat-fctx-bugfix-spot-check` | Recipe for HUMAN-UAT spot check | ✓ VERIFIED | Recipe present at justfile:260; queries `cronduit.dev.db` (correct dev convention). |

### Key Link Verification

| From | To  | Via | Status | Details |
| ---- | --- | --- | ------ | ------- |
| `src/scheduler/run.rs:305 (the bug site)` | `src/scheduler/docker.rs::DockerExecResult.container_id` | `docker_result.container_id.clone()` | ✓ WIRED | Bug eradication grep `! grep -qE 'container_id_for_finalize\s*=\s*docker_result\.image_digest' src/scheduler/run.rs` returns 0 (the bug is GONE). |
| `src/scheduler/run.rs:361 finalize_run call site` | `src/db/queries.rs::finalize_run` (8-arg) | `image_digest_for_finalize.as_deref()` 8th positional | ✓ WIRED | Production caller at run.rs:361 passes the value; queries.rs:452 accepts it; bound at L468/L483 into UPDATE column position ?7/$7. |
| `src/scheduler/run.rs:86 + src/web/handlers/api.rs:82` | `src/db/queries.rs::insert_running_run` (4-arg) | `&job.config_hash` 4th positional | ✓ WIRED | Production callers pass DbJob.config_hash; queries.rs:376 accepts; bound at L399/L425 into INSERT column position ?5/$5. |
| `src/web/handlers/api.rs:131 error fallback` | `queries::finalize_run` | passes `None` for image_digest (no docker run started) | ✓ WIRED | Phase 16 FOUND-14 trailing comment present. |
| `src/scheduler/mod.rs:264, 339 orphan-row fallbacks` | `queries::finalize_run` | passes `None` for image_digest | ✓ WIRED | Rule 3 auto-fixes per 16-04b SUMMARY; semantically correct. |
| `tests/v12_fctx_explain.rs SQLite arm` | `EXPLAIN QUERY PLAN` against `idx_job_runs_job_id_start` | substring assertion + reject bare SCAN | ✓ WIRED | SQLite test passes locally. |
| `tests/v12_fctx_streak.rs` | `queries::get_failure_context` | direct integration call | ✓ WIRED | 7/7 tests pass. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| -------- | ------------- | ------ | ------------------ | ------ |
| `DockerExecResult.container_id` | `container_id` | `bollard::create_container().id` (docker.rs:186-190) | Yes — real string from Docker daemon | ✓ FLOWING |
| `DockerExecResult.image_digest` | `image_digest` | `bollard::inspect_container().image` (docker.rs:240-251) | Yes for happy path; **WR-01: empty string in inspect-failure path (deferred per Pitfall 6)** | ⚠️ STATIC (advisory, see Anti-Patterns) |
| `job_runs.container_id` | persisted via `finalize_run` bind position 6 | `container_id_for_finalize` from run.rs:305 | Yes — real container ID flows through | ✓ FLOWING |
| `job_runs.image_digest` | persisted via `finalize_run` bind position 7 | `image_digest_for_finalize` from run.rs:306 | Yes (Some(String)); empty-string footgun documented in WR-01 | ✓ FLOWING (advisory) |
| `job_runs.config_hash` | persisted via `insert_running_run` bind position 5 | `&job.config_hash` from production callers | Yes — real config hash | ✓ FLOWING |
| `FailureContext` (4 fields) | hydrated via `row.get(...)` from CTE result | `get_failure_context` SQL execution | Yes — `Option` variants reflect LEFT JOIN ON 1=1 NULL semantics | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Build with all targets compiles cleanly | `cargo build --tests` | `Finished dev profile [unoptimized + debuginfo] target(s) in 23.72s` | ✓ PASS |
| Backfill correctness regression | `cargo test --test v12_fctx_config_hash_backfill` | 4 passed; 0 failed | ✓ PASS |
| Bug-fix non-ignored regression | `cargo test --test v12_run_rs_277_bug_fix` | 2 passed; 2 ignored (Docker-gated) | ✓ PASS (with WR-02 caveat) |
| Streak + FCTX-04 write-site | `cargo test --test v12_fctx_streak` | 7 passed | ✓ PASS |
| EXPLAIN regression lock (SQLite) | `cargo test --test v12_fctx_explain` | SQLite arm passes; Postgres ignored | ✓ PASS |
| D-15 SQL portability gate | `just grep-no-percentile-cont` | OK: no percentile_cont / percentile_disc / median( / PERCENTILE_ in src/ | ✓ PASS |
| Schema parity invariant | `just schema-diff` | 3 passed (sqlite_and_postgres_schemas_match_structurally + 2 normalize_type) | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| ----------- | ---------- | ----------- | ------ | -------- |
| FOUND-14 | 16-01, 16-02, 16-03, 16-04a, 16-04b | run.rs:277 bug fixed: DockerExecResult carries both container_id + image_digest; finalize_run populates both DB columns; historical rows age out via Phase 6 retention. | ✓ SATISFIED | Bug-fix at run.rs:305 verified via grep; DockerExecResult fields verified; queries.rs::finalize_run signature/UPDATE verified; production callers verified. NOTE: regression test coverage gap (WR-02) — see Anti-Patterns. |
| FCTX-04 | 16-01, 16-04a, 16-04b, 16-05 | `job_runs.config_hash TEXT NULL` per-run column added; conservative backfill from `jobs.config_hash`; written from `insert_running_run` at fire time. | ✓ SATISFIED | Migration files 006/007 verified; `insert_running_run` 4-arg signature verified; production callers wire `&job.config_hash`; `tests/v12_fctx_streak.rs::write_site_captures_config_hash` (T-V12-FCTX-03) and `reload_changes_config_hash` (T-V12-FCTX-04) pass. |
| FCTX-07 | 16-05, 16-06 | `get_failure_context(job_id)` returns single struct from single SQL query; EXPLAIN on both backends uses indexed access on `job_runs.job_id + start_time`. | ✓ SATISFIED | Helper landed at queries.rs:681 with CTE + LEFT JOIN ON 1=1; FailureContext struct present; EXPLAIN-test (SQLite) asserts `idx_job_runs_job_id_start`; Postgres EXPLAIN ignored-by-design (testcontainers, matches v13 precedent). |

All 3 requirement IDs from PLAN frontmatter are present in REQUIREMENTS.md (lines 172, 195, 198) and Phase 16 in REQUIREMENTS.md does not map any additional IDs to this phase. No orphaned requirements.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| `src/scheduler/docker.rs` | ~253-264, 428 | `image_digest = String::new()` flowing to `Some("")` in inspect-failure path (WR-01 from REVIEW.md) | ⚠️ Warning | Empty-string image_digest persists to DB instead of NULL, breaking the `IS NULL` vs `IS NOT NULL` semantic the Phase 21 UI consumer will rely on. **Plan 16-02 explicitly deferred this per Pitfall 6 + Open Question 1 + planner discretion**; documented in 16-02 SUMMARY "Decisions Made". Not a Phase 16 blocker because (a) tests pass, (b) the FCTX-07 query treats `Some("")` as a non-NULL string (returns it as `last_success_image_digest`), and (c) Phase 21 will need to display `Some("")` and `None` distinguishably anyway. Surfaces as a soft data-quality issue Phase 21 must handle. |
| `tests/v12_run_rs_277_bug_fix.rs` | 78-160 (ignored), 247-291, 303-353 | Non-ignored regression tests do NOT exercise `run.rs:305-306` wiring (WR-02 from REVIEW.md) | ⚠️ Warning | The two non-#[ignore] tests (`command_run_leaves_image_digest_null`, `digest_persists_across_inspect_failure`) call `finalize_run` directly with hand-crafted `None`/`Some(...)` arguments. They do NOT trigger the `container_id_for_finalize = docker_result.container_id.clone()` assignment at run.rs:305. A future PR that swaps the two locals back (re-introducing FOUND-14) would compile and pass `cargo test`, `cargo clippy`, `just nextest`, and the entire CI chain. The bug-fix is structurally protected ONLY by the docker-gated `#[ignore]` tests, which require a local Docker daemon. **REVIEW recommendation:** add a unit-level wiring test that consumes a synthetic DockerExecResult and asserts the two locals are populated from the correct fields — locks the wiring at the function-signature level so a future swap fails noisily in standard CI. Phase 16 ships without this lock. |
| `migrations/sqlite/20260429_000007_config_hash_backfill.up.sql` + Postgres pair | L3 | `BACKFILL_CUTOFF_RFC3339: 2026-04-27T00:00:00Z` is today (WR-03 from REVIEW.md) | ⚠️ Warning | Rows that ended after 2026-04-27T00:00:00Z but before the migration ran are NOT classified as backfilled by Phase 21's heuristic, even though the migration's `UPDATE ... WHERE config_hash IS NULL` will populate them. Documentation invariant is leaky for a same-day deployment. v1.1 had no `config_hash` column, so the impact is likely zero in practice (no v1.1 row had `config_hash` to begin with), but the marker convention should not yield false negatives for any v1.2+ deployment. |
| `migrations/sqlite/20260429_000007_config_hash_backfill.up.sql` + Postgres pair | L13-17 | Comment block describes a heuristic the SQL does not implement (WR-04 from REVIEW.md) | ⚠️ Warning | Comment reads "Heuristic: rows where end_time < BACKFILL_CUTOFF AND config_hash IS NOT NULL are backfilled." A reader expects the SQL to apply this filter. Actual SQL filters only on `config_hash IS NULL`. Documentation is purely a forward-looking contract for Phase 21 — bug-prone. |
| `tests/v12_fctx_streak.rs` | 67-84 | Seed format `"2026-04-27T00:{:02}:00Z"` overflows for `time_index >= 60` (IN-02 from REVIEW.md) | ℹ️ Info | Currently dormant (max time_index = 6 in current scenarios); fragile if scenario size grows. |

Note: REVIEW.md flagged 4 warnings + 5 info; only 4 warnings carried forward as relevant (IN-04 / IN-05 / IN-01 / IN-03 are stylistic notes already accepted). The 4 warnings are all advisory — none rises to BLOCKER for Phase 16's stated goal because:
- WR-01 / WR-03 / WR-04 are documentation/data-quality issues that affect Phase 21 consumption, not Phase 16 success criteria.
- WR-02 is a regression-coverage gap, not a behavior gap (the bug fix IS in place per grep + the docker-gated tests).

The Phase 16 goal as stated in ROADMAP.md is satisfied at the codebase level. WR-02 is the most material warning — it weakens future-proofing of the bug fix without weakening the current state of the code.

### Human Verification Required

Per `.planning/phases/16-failure-context-schema-run-rs-277-bug-fix/16-HUMAN-UAT.md` (gathered 2026-04-27, status: pending_maintainer_validation):

#### 1. FOUND-14 spot check on a real homelab dev DB

**Test:**
1. Ensure a v1.2 docker job has fired against `cronduit.dev.db` (run `just dev`, then trigger a docker job via dashboard "Run Now").
2. From the repo root, run: `just uat-fctx-bugfix-spot-check`.
3. Inspect the printed `(id, job_id, status, container_id, image_digest)` row.

**Expected:**
- `container_id` is a Docker container ID — typically a 64-char hex string or 12-char hex prefix. Example: `7f4c9b...` or `f3e8d72c1a5e...`.
- Alternatively, `container_id` may be `NULL` for non-docker (`type = "command"` / `type = "script"`) runs or the `running` row of a still-in-flight docker run.
- `container_id` MUST NOT start with `sha256:` for any v1.2 docker run — that would indicate FOUND-14 regressed.
- `image_digest` for docker jobs is `Some(sha256:...)`; for command/script jobs is `NULL`; for docker jobs where `inspect_container` failed, may be `NULL` or empty (existing fallback path; advisory WR-01).

**Why human:** 16-HUMAN-UAT.md mandates maintainer validation per project rules D-12 (every UAT step uses an existing `just` recipe) and D-13 (Claude does NOT mark UAT passed from its own runs; the maintainer must run and confirm). The automated `tests/v12_run_rs_277_bug_fix.rs::docker_run_writes_real_container_id_not_digest` test exists (`#[ignore]`-gated, requires Docker daemon, mirrors this assertion against testcontainers) but is not run in standard CI; the maintainer's local-DB inspection on a real docker run is the canonical sanity check operators will run after upgrade.

### Gaps Summary

No phase-blocking gaps. All 3 roadmap success criteria are satisfied at the codebase level. Three flagged advisory warnings (WR-01, WR-03, WR-04) are documentation/data-quality issues that the maintainer or downstream phases (21) can address; one warning (WR-02) is a regression-coverage gap that does not break the current behavior but weakens future-proofing of the bug fix in standard CI.

The phase is mergeable; the operator-observable for FOUND-14 (Success Criterion 1) requires the manual UAT spot check per project policy before flipping the requirement to Validated.

### Notes for Future Phases

- **Phase 18 (webhook payload, WH-09):** Will compute `streak_position` Rust-side from `FailureContext.consecutive_failures` per D-06. Will need to document that `Some("")` from `image_digest` is operationally equivalent to `None` for the `image_digest` payload field, OR address WR-01 by tightening the inspect-failure path to write `None` (small change, ~3 lines in `src/scheduler/docker.rs`).
- **Phase 21 (FCTX UI):** Will read `BACKFILL_CUTOFF_RFC3339: 2026-04-27T00:00:00Z` marker from `migrations/*/20260429_000007_config_hash_backfill.up.sql:3` to flag pre-cutoff rows. Phase 21 should also handle the WR-03 same-day-deploy edge case (false negatives on rows ended between cutoff and migration apply time) and the WR-04 heuristic-text-vs-SQL discrepancy.

---

_Verified: 2026-04-27_
_Verifier: Claude (gsd-verifier)_
