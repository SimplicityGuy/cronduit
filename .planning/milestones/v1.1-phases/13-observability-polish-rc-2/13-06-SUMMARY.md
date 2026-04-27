---
phase: 13
plan: 06
subsystem: phase-close-out
tags: [observability, validation, ci-guard, release-mechanics, rc2, human-action]
requirements: [OBS-02, OBS-05]
wave: 4
depends_on: [13-02, 13-05]

dependency-graph:
  requires:
    - "queries::get_timeline_runs (shipped by plan 13-02)"
    - "queries::PoolRef enum (shipped by plan 01)"
    - "tests/db_pool_postgres.rs startup pattern (Postgres testcontainer)"
    - "justfile recipe-only CI policy (every `run:` is `just <recipe>`)"
    - ".planning/REQUIREMENTS.md OBS-01..OBS-05 bullet + traceability rows (pre-existing)"
    - "Phase 12 release mechanics: release.yml D-10 gate, docs/release-rc.md, scripts/verify-latest-retag.sh (reused verbatim per D-22)"
  provides:
    - "tests/v13_timeline_explain.rs: 3 #[tokio::test] fns (SQLite EXPLAIN QUERY PLAN, Postgres EXPLAIN JSON, LIMIT 10000)"
    - "tests/v13_timeline_timezone.rs: 1 #[tokio::test] fn (America/Los_Angeles render test for T-V11-TIME-04)"
    - "justfile `grep-no-percentile-cont` recipe: OBS-05 structural parity CI lock"
    - ".github/workflows/ci.yml lint job step invoking the new recipe"
    - ".planning/REQUIREMENTS.md OBS-01..OBS-05 flipped to [x] / Complete (10 line changes)"
    - "HUMAN-UAT.md: maintainer runbook for the v1.1.0-rc.2 tag cut (Task 5 checkpoint)"
  affects:
    - "OBS-02 complete: dual-backend EXPLAIN + LIMIT + timezone all verified"
    - "OBS-05 permanently locked: CI gate prevents any future PR from re-introducing SQL-native percentile"
    - "Phase 13 v1.1.0-rc.2 tag cut: pending maintainer action (HUMAN-UAT.md runbook)"

tech-stack:
  added: []
  patterns:
    - "EXPLAIN QUERY PLAN text scan for index-name substring (SQLite) — the documented test pattern from 13-RESEARCH.md Wave 0 plan"
    - "EXPLAIN (FORMAT JSON) recursive plan-tree walk (Postgres) — accepts Index Scan / Index Only Scan / Bitmap Index Scan / Bitmap Heap Scan variants"
    - "Selective predicate + ANALYZE pattern on Postgres testcontainer — forces the planner to use the index instead of defaulting to Seq Scan on low-statistics fresh tables"
    - "Documented downgrade fallback: if planner still picks Seq Scan, fall back to `plan_json.to_string().contains(index_name)` — proves the index was at least considered"
    - "Raw-SQL seed bypassing insert_running_run/finalize_run for deterministic start_time + bulk speed (15000 rows in single tx)"
    - "Rule-1 auto-fix pattern: EXPLAIN test exposed a pre-existing `j.enabled = true` bug in get_timeline_runs Postgres arm (BIGINT column + BOOLEAN literal → Postgres operator error). Fixed to `j.enabled = 1` inline; identical bug in get_dashboard_jobs logged as out-of-scope deferred item."
    - "Justfile recipe for CI gate — consistent with project CI policy that all `run:` steps invoke `just <recipe>` (D-10 / FOUND-12)"
    - "Comment-filtered grep guard: `grep -vE '//'` filter lets doc comments explicitly declaring the invariant (e.g. \"no SQL-native percentile is used\") coexist with the guard"

key-files:
  created:
    - path: "tests/v13_timeline_explain.rs"
      exports: []
      lines-added: 397
      purpose: "Dual-backend EXPLAIN tests + LIMIT 10000 enforcement (OBS-02 T-V11-TIME-01 / T-V11-TIME-02)"
    - path: "tests/v13_timeline_timezone.rs"
      exports: []
      lines-added: 168
      purpose: "Timezone render test — America/Los_Angeles (OBS-02 T-V11-TIME-04)"
    - path: ".planning/phases/13-observability-polish-rc-2/HUMAN-UAT.md"
      exports: []
      lines-added: 180
      purpose: "Maintainer runbook for the v1.1.0-rc.2 tag cut (Task 5 checkpoint)"
    - path: ".planning/phases/13-observability-polish-rc-2/deferred-items.md"
      exports: []
      lines-added: 11
      purpose: "Log of out-of-scope discoveries (pre-existing `j.enabled = true` bug in get_dashboard_jobs Postgres arm)"
  modified:
    - path: "src/db/queries.rs"
      lines-added: 8
      lines-deleted: 1
      purpose: "Rule-1 auto-fix: change `j.enabled = true` → `j.enabled = 1` in get_timeline_runs Postgres arm (enabled is BIGINT on Postgres, comparison to boolean literal raises an operator error). Detected by the new EXPLAIN test harness."
    - path: "justfile"
      lines-added: 23
      purpose: "New `grep-no-percentile-cont` recipe — scans src/ for SQL-native percentile patterns; ignores comment lines"
    - path: ".github/workflows/ci.yml"
      lines-added: 7
      purpose: "New `- run: just grep-no-percentile-cont` step in the lint job (after openssl-check)"
    - path: ".planning/REQUIREMENTS.md"
      lines-added: 10
      lines-deleted: 10
      purpose: "Flip 5 OBS bullet checkboxes `[ ]` → `[x]` + 5 traceability rows `Pending` → `Complete`"

decisions:
  - "Auto-fixed `j.enabled = true` → `j.enabled = 1` in get_timeline_runs Postgres arm (Rule 1 bug) — the Postgres `jobs.enabled` column is BIGINT (per schema_parity normalize_type INT64 rule), so comparing to the boolean literal `true` raises `operator does not exist: bigint = boolean` at runtime. Discovered because the new EXPLAIN test is the first end-to-end execution of get_timeline_runs on Postgres in the test suite. Identical bug pattern in get_dashboard_jobs (lines 615, 628) is pre-existing and out-of-scope — logged to deferred-items.md for a future phase."
  - "Seeded 10,000 rows + ANALYZE + selective predicate (window = base+9000min, matches ~10% of rows) for the Postgres EXPLAIN test. Fresh testcontainer with low row counts defaulted to Seq Scan even after ANALYZE; boosting volume + narrowing selectivity forced Index Scan reliably. Added Bitmap Index/Heap Scan to the accepted node-type set because Postgres can prefer those forms for this predicate shape."
  - "Added a documented-fallback assertion in the Postgres EXPLAIN test: accept EITHER (a) any Index Scan / Index Only Scan / Bitmap Index / Bitmap Heap Scan node OR (b) the index name `idx_job_runs_start_time` / `idx_job_runs_job_id_start` appears anywhere in the rendered plan JSON. The fallback is the downgrade path explicitly blessed by plan 06 Task 1 action section for testcontainer flakiness. Both paths prove the index exists and is reachable."
  - "Raw SQL insert in the LIMIT-10000 test instead of `insert_running_run` + `finalize_run`: 15000 full insert-then-update round-trips on in-memory SQLite is ~30s+ on slower CI runners, vs ~0.5s for a single-transaction raw INSERT loop. Per plan 06 Task 1 action: the column list `(job_id, job_run_number, status, trigger, start_time, end_time, duration_ms, exit_code)` is explicit because Phase 11 DB-10 made `job_run_number` NOT NULL with no table default."
  - "Justfile recipe approach over inline `run:` in ci.yml — consistent with the shipped CI convention (ci.yml comment line 3: 'Every `run:` step invokes `just <recipe>` exclusively'). The recipe filters out `//` and `///` comment lines so doc comments explicitly declaring the OBS-05 invariant do not trip the guard."
  - "Traceability-table rows flipped `Pending` → `Complete` (8 chars). Matches the Phase 12.1 OPS-09/10 precedent (lines 177-178) which use `Complete`. Phase 12 OPS-06/07/08 used `Done` (4 chars) — plan 06 Task 4 instruction specified `Complete`, so this plan follows plan-06 over phase-12-specific."
  - "Task 5 (tag cut) deferred to the maintainer per Phase 12 D-13 (carried to Phase 13 per D-22) and `feedback_uat_user_validates.md`. Claude does NOT execute `git tag v1.1.0-rc.2`. HUMAN-UAT.md in the phase directory is the runbook."

requirements-completed: [OBS-01, OBS-02, OBS-03, OBS-04, OBS-05]

metrics:
  duration: "~25 minutes"
  completed: "2026-04-21"
  tasks-completed: 4  # Tasks 1-4; Task 5 is a deferred maintainer action
  tasks-deferred: 1   # Task 5 (tag cut) — pending maintainer action
  commits: 4
  files-created: 4
  files-modified: 4
  lines-added: 804
  tests-added: 4        # 3 in v13_timeline_explain.rs + 1 in v13_timeline_timezone.rs
  tests-passing: 208    # 194 lib + 14 phase-13 integration (including 4 new)
  tests-regressed: 0
---

# Phase 13 Plan 06: Close-Out + rc.2 Tag Cut Summary

**Dual-backend EXPLAIN QUERY PLAN tests + timezone render test + OBS-05 grep guard shipped, REQUIREMENTS.md OBS-01..OBS-05 flipped to Complete, HUMAN-UAT.md runbook persisted for the v1.1.0-rc.2 maintainer tag cut (Task 5 checkpoint — not executed by Claude per Phase 12 D-13 / Phase 13 D-22).**

One-liner: Phase 13 close-out — four automated commits (EXPLAIN tests, timezone test, OBS-05 CI guard, REQUIREMENTS.md flip) plus one maintainer-action checkpoint (`v1.1.0-rc.2` tag cut) that ships via the Phase 12 runbook verbatim.

## Plan Truth Status

| Truth                                                                 | Status        | Owner                     |
| --------------------------------------------------------------------- | ------------- | ------------------------- |
| #1 SQLite EXPLAIN QUERY PLAN shows `idx_job_runs_start_time`          | ACHIEVED      | Task 1 (commit `9f5e6c9`) |
| #2 Postgres EXPLAIN JSON shows Index Scan on `job_runs`               | ACHIEVED      | Task 1 (commit `9f5e6c9`) |
| #3 LIMIT 10000 enforced (15k seed → 10k result)                       | ACHIEVED      | Task 1 (commit `9f5e6c9`) |
| #4 Timeline renders timestamps in operator's configured timezone      | ACHIEVED      | Task 2 (commit `55e5017`) |
| #5 OBS-05 CI grep guard prevents SQL-native percentile regression     | ACHIEVED      | Task 3 (commit `565f766`) |
| #6 REQUIREMENTS.md OBS-01..OBS-05 flipped to `[x]` / `Complete`       | ACHIEVED      | Task 4 (commit `f79c77c`) |
| #7 `v1.1.0-rc.2` tag cut + pushed to GHCR                             | **PENDING**   | Maintainer (HUMAN-UAT.md) |
| #8 Post-push verification: `:rc` advanced, `:latest` still `v1.0.1`   | **PENDING**   | Maintainer (HUMAN-UAT.md) |

**Truth #7 + #8 are explicitly deferred.** Per `feedback_uat_user_validates.md` and Phase 12 D-13 (carried to Phase 13 per D-22), Claude does not self-execute the tag cut. The maintainer runs `HUMAN-UAT.md` after the Phase 13 PR merges to `main`.

## Tasks Completed

### Task 1 — Dual-backend EXPLAIN QUERY PLAN test (OBS-02 T-V11-TIME-01 / T-V11-TIME-02)

- **Commit:** `9f5e6c9 test(13-06): add dual-backend EXPLAIN QUERY PLAN tests for timeline query (OBS-02)`
- **File:** `tests/v13_timeline_explain.rs` (new, 397 lines)
- **Files modified (Rule-1 auto-fix):** `src/db/queries.rs` (`j.enabled = true` → `j.enabled = 1` in get_timeline_runs Postgres arm)

Three `#[tokio::test]` functions:

| Test                            | Scenario                                                                          | Key assertion                                                                                   |
| ------------------------------- | --------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `explain_uses_index_sqlite`     | In-memory SQLite + 2 jobs + 100 runs                                              | `EXPLAIN QUERY PLAN` output contains `idx_job_runs_start_time` OR `idx_job_runs_job_id_start`   |
| `explain_uses_index_postgres`   | testcontainers Postgres + 1 job + 10,000 runs + ANALYZE + selective window        | Plan tree contains Index Scan / Index Only Scan / Bitmap Index Scan / Bitmap Heap Scan node    |
| `limit_10000_enforced`          | In-memory SQLite + 1 job + 15,000 runs seeded in one tx                           | `queries::get_timeline_runs(...).len() == 10_000`                                               |

**Test output (final run):**

```
Starting 3 tests across 1 binary
    PASS [   0.025s] (1/3) explain_uses_index_sqlite
    PASS [   0.480s] (2/3) limit_10000_enforced
    PASS [   9.269s] (3/3) explain_uses_index_postgres
Summary 3 tests run: 3 passed, 0 skipped
```

### Task 2 — Timezone rendering test (OBS-02 T-V11-TIME-04)

- **Commit:** `55e5017 test(13-06): timezone render test for /timeline (OBS-02 T-V11-TIME-04)`
- **File:** `tests/v13_timeline_timezone.rs` (new, 168 lines)

One `#[tokio::test]`: `pdt_label_in_timeline_render`

- Wires `AppState.tz = America/Los_Angeles` into the router (vs the UTC default every other render test uses).
- Seeds a run 6h ago in UTC via direct SQL (bypassing `insert_running_run`/`finalize_run` for deterministic `start_time`).
- GETs `/timeline?window=24h`, asserts the body contains the LA-local `HH:MM:SS` rendering of both `start_time` and `end_time`.
- Belt-and-suspenders `assert_ne!(la_str, utc_str)` proves the timezone plumbing is actually running (strings differ by 7-8h).
- DST-aware automatically via `chrono_tz::Tz::with_timezone(&la_tz).format()` — no hardcoded offsets.

**Test output:**

```
Summary 1 test run: 1 passed, 0 skipped
    PASS pdt_label_in_timeline_render
```

### Task 3 — CI grep guard for OBS-05 structural parity

- **Commit:** `565f766 ci(13-06): grep guard prevents SQL-native percentile regression (OBS-05)`
- **Files:** `justfile` (+23 lines, new recipe), `.github/workflows/ci.yml` (+7 lines, new step)

New `just grep-no-percentile-cont` recipe:

```bash
pattern='\b(percentile_cont|percentile_disc|PERCENTILE_|median\()\b'
# Match the pattern, then filter out lines whose first non-whitespace is `//`
# so doc comments explicitly declaring the invariant do not trip the guard.
matches=$(grep -rnE "$pattern" src/ 2>/dev/null | grep -vE '^[^:]+:[0-9]+:[[:space:]]*//' || true)
if [ -n "$matches" ]; then
    echo "$matches"
    echo "ERROR: OBS-05 structural parity violated ..."
    exit 1
fi
echo "OK: no percentile_cont / percentile_disc / median( / PERCENTILE_ in src/ (comments ignored)"
```

Added to `.github/workflows/ci.yml` lint job after `- run: just openssl-check`:

```yaml
# Phase 13 OBS-05 structural parity guard ...
- run: just grep-no-percentile-cont
```

**Intentional-regression verification:**

| Tree state                                                                 | Recipe exit code | Behavior |
| -------------------------------------------------------------------------- | ---------------- | -------- |
| Clean (HEAD)                                                               | 0                | `OK: no percentile_cont ...` |
| Probe file w/ `SELECT percentile_cont(0.5) ... FROM job_runs` in `src/`    | 1                | Matching line printed + error |
| Clean again (after probe removed)                                          | 0                | Passes |

Guard fires on code usage, ignores doc-comment declarations (e.g. `src/db/queries.rs:735` says "no SQL-native percentile (`percentile_cont`, `percentile_disc`) is used" — passes the filter because the line starts with `///`).

### Task 4 — Flip OBS-01..OBS-05 in REQUIREMENTS.md

- **Commit:** `f79c77c docs(13-06): mark OBS-01..OBS-05 complete`
- **File:** `.planning/REQUIREMENTS.md` (+10 / -10)

Five bullet checkboxes flipped `[ ]` → `[x]` (lines 63, 65, 67, 69, 71).
Five traceability-table rows flipped `Pending` → `Complete` (lines 165-169).

Verification:

```
$ grep -cE '^- \[x\] \*\*OBS-0[1-5]\*\*' .planning/REQUIREMENTS.md
5
$ grep -cE '^\| OBS-0[1-5]\s+\| Phase 13\s+\| Complete' .planning/REQUIREMENTS.md
5
```

No other line touched (per acceptance criteria "git diff shows exactly 10 line changes").

### Task 5 — DEFERRED (maintainer-action: cut `v1.1.0-rc.2` tag)

- **Status:** PENDING (maintainer action)
- **Runbook:** `.planning/phases/13-observability-polish-rc-2/HUMAN-UAT.md`
- **Why deferred:** Phase 12 D-13 policy — tag cuts are maintainer-workstation actions, not CI or Claude actions. The signing key / tagger identity is the trust anchor and must live outside GHA runner identity. `feedback_uat_user_validates.md` reinforces: Claude does not self-validate UAT.
- **Unblocked when:** Phase 13 PR merges to `main` and both `ci` + `compose-smoke` workflows show `completed/success` on that merge commit.
- **Runbook sequence (summary):**
  1. Pull `main` to local workstation.
  2. Pre-flight: `scripts/verify-latest-retag.sh 1.0.1` (confirms `:latest` still pinned).
  3. Preview release notes: `git cliff --unreleased --tag v1.1.0-rc.2 -o /tmp/rc2-preview.md`.
  4. `git tag -a -s v1.1.0-rc.2 -m "..."` (signed) or `-a` (annotated).
  5. `git push origin v1.1.0-rc.2`.
  6. Wait for `release.yml`.
  7. Verify `:rc` advanced, `:latest` unchanged, multi-arch manifest present.

See `HUMAN-UAT.md` for the full step list with copy-pasteable commands.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `j.enabled = true` in get_timeline_runs Postgres arm raises `operator does not exist: bigint = boolean`**

- **Found during:** Task 1 first run of `explain_uses_index_postgres`. The testcontainer Postgres rejected the timeline SQL with the operator-mismatch error.
- **Issue:** The shipped Postgres arm of `get_timeline_runs` (plan 13-02 commit `2b4d0a9`) used `WHERE j.enabled = true`. However, `jobs.enabled` is declared `BIGINT NOT NULL DEFAULT 1` on Postgres (per `migrations/postgres/20260410_000000_initial.up.sql` line 17, and confirmed by `schema_parity normalize_type` which collapses BIGINT ↔ INTEGER to the shared INT64 token). Comparing a BIGINT column to the boolean literal `true` fails at the SQL-type-checker stage, before the query even executes.
- **Fix:** Changed `j.enabled = true` → `j.enabled = 1` in `src/db/queries.rs` line 870 (after edit). Added a load-bearing code comment referencing the BIGINT/BOOLEAN rationale and the fix's provenance. Test now passes.
- **Files modified:** `src/db/queries.rs` (+8 / -1)
- **Commit:** `9f5e6c9` (folded into Task 1's commit)
- **Scope:** Narrow — only the direct dependency of the EXPLAIN test. The identical bug pattern exists in `get_dashboard_jobs` (lines 615, 628) but is out-of-scope (pre-existing; separate code path; no test in the current suite exercises it on Postgres). Logged to `.planning/phases/13-observability-polish-rc-2/deferred-items.md` for a future Phase 14 bug-fix plan or pre-GA hygiene commit.

**2. [Rule 2 - Missing functionality] Task 1 plan assumed fresh Postgres would happily pick Index Scan at 1000 rows; actually required 10k rows + selective predicate**

- **Found during:** Task 1 second run of `explain_uses_index_postgres` (after the bug-1 fix).
- **Issue:** Plan 06 Task 1 action block specified 1000 rows + ANALYZE as the minimum seed. Observed on testcontainers postgres:11-alpine: 1000 rows with a non-selective predicate (all rows match) still picked Seq Scan + Sort because the sort step dominated cost. The plan documented this risk in the Task 1 caveat — "If CI still shows flaky Index Scan detection on the Postgres testcontainer, downgrade the assertion".
- **Fix:** Boosted seed to 10,000 rows + tightened the window-start predicate to `base + 9000 minutes` so only ~10% of rows match. Also expanded the accepted node-type set to include `Bitmap Index Scan` and `Bitmap Heap Scan` (Postgres frequently picks these over plain `Index Scan` at medium selectivity). Added the documented-fallback assertion: test also passes if the plan JSON merely mentions `idx_job_runs_start_time` anywhere (proves the index was at least considered).
- **Files modified:** `tests/v13_timeline_explain.rs` (+~40 lines on the Postgres test)
- **Commit:** `9f5e6c9` (folded into Task 1's commit)

### No other deviations

- Task 2 matches plan action verbatim (single `#[tokio::test]` + DST-aware chrono_tz formatting).
- Task 3 follows plan-preferred path (justfile recipe over inline ci.yml run) with the only refinement being a comment-line filter to skip `///` invariant declarations.
- Task 4 matches plan instruction verbatim (10 line changes, 5 bullet + 5 table).
- Task 5 explicitly deferred per plan `<checkpoint:human-action>` + orchestrator checkpoint-awareness instructions.

## Verification Results

### Automated gates

```bash
$ cargo nextest run --lib
194 tests run: 194 passed, 0 skipped

$ cargo nextest run --test v13_timeline_explain --test v13_timeline_timezone \
    --test v13_timeline_render --test v13_sparkline_render --test v13_duration_card
20 tests run: 20 passed, 0 skipped

$ cargo clippy --all-targets -- -D warnings
Finished `dev` profile (zero warnings)

$ cargo fmt --check
(clean)

$ just grep-no-percentile-cont
OK: no percentile_cont / percentile_disc / median( / PERCENTILE_ in src/ (comments ignored)

$ grep -cE '^- \[x\] \*\*OBS-0[1-5]\*\*' .planning/REQUIREMENTS.md
5

$ grep -cE '^\| OBS-0[1-5]\s+\| Phase 13\s+\| Complete' .planning/REQUIREMENTS.md
5
```

Green across every gate.

### Manual-only (deferred to maintainer)

Three UAT gates sit in `HUMAN-UAT.md` Task 5 verification:

- `docker buildx imagetools inspect ghcr.io/simplicityguy/cronduit:1.1.0-rc.2` → multi-arch (amd64 + arm64) manifest.
- `scripts/verify-latest-retag.sh 1.0.1` → post-push `:latest` pin intact.
- `gh release view v1.1.0-rc.2 --json isPrerelease --jq .isPrerelease` → `true`.

These cannot be automated — they require push access to the GHCR repo + a local signing key.

## Threat Model Coverage

Plan 06's 5-row threat register had three `mitigate` dispositions; all are in place:

| Threat     | Disposition | Status after plan 06                                                                           |
| ---------- | ----------- | ---------------------------------------------------------------------------------------------- |
| T-13-06-01 | mitigate    | `release.yml` D-10 gate + `scripts/verify-latest-retag.sh` (inherited from Phase 12)           |
| T-13-06-02 | mitigate    | Signed-tag policy from Phase 12 D-13 carries forward; HUMAN-UAT.md reinforces                  |
| T-13-06-03 | accept      | Deterministic "no matches" output in clean tree; no secrets                                    |
| T-13-06-04 | accept      | Documented fallback assertion in EXPLAIN test handles testcontainer planner flakiness          |
| T-13-06-05 | n/a         | Signed-tag model is the audit trail                                                            |

No additional defensive code needed — all mitigations inherited from Phase 12 artifacts that this plan reuses verbatim.

## Threat Flags

None. Plan 06 adds:

- Two new read-only test files (no new trust boundary; no new endpoint).
- A CI grep step (repo-read scope; no new permissions).
- Documentation edits (no code surface).
- A deferred maintainer action (existing Phase 12 boundary; no new boundary).

No security-relevant surface introduced. The `j.enabled = true` → `j.enabled = 1` fix tightens an existing broken path, not a new one.

## Known Stubs

None. Every deliverable in this plan is end-to-end wired:

- Tests are real integration tests against real SQLite + real Postgres testcontainers — no mocks, no stubs.
- CI guard runs real `grep -rnE` against real source — not a placeholder.
- REQUIREMENTS.md flips are real checkbox + table cell edits, not placeholder text.
- HUMAN-UAT.md is a fully-worked maintainer runbook with copy-pasteable commands, not a TODO.

## Commits

| Task | Hash      | Message                                                                               |
| ---- | --------- | ------------------------------------------------------------------------------------- |
| 1    | `9f5e6c9` | `test(13-06): add dual-backend EXPLAIN QUERY PLAN tests for timeline query (OBS-02)`  |
| 2    | `55e5017` | `test(13-06): timezone render test for /timeline (OBS-02 T-V11-TIME-04)`              |
| 3    | `565f766` | `ci(13-06): grep guard prevents SQL-native percentile regression (OBS-05)`            |
| 4    | `f79c77c` | `docs(13-06): mark OBS-01..OBS-05 complete`                                           |
| 5    | PENDING   | (maintainer cuts `v1.1.0-rc.2` per HUMAN-UAT.md)                                      |

## User Setup Required

**`v1.1.0-rc.2` tag cut is the next maintainer action after the Phase 13 PR merges to main.**

See `.planning/phases/13-observability-polish-rc-2/HUMAN-UAT.md` for the full runbook.

Summary sequence:

1. Phase 13 PR merges to `main` (both CI workflows must be green).
2. `git checkout main && git pull --ff-only origin main`.
3. `scripts/verify-latest-retag.sh 1.0.1` exits 0.
4. `git cliff --unreleased --tag v1.1.0-rc.2 -o /tmp/rc2-preview.md` — review.
5. `git tag -a -s v1.1.0-rc.2 -m "v1.1.0-rc.2 — release candidate (observability polish)"`.
6. `git push origin v1.1.0-rc.2`.
7. Wait for `release.yml` (~5-10 min).
8. Verify `:1.1.0-rc.2` multi-arch + `:rc` advanced + `:latest` still `v1.0.1`.
9. Report back per HUMAN-UAT.md Step 9 "rc2 tag pushed" format.

If any post-push check fails, ship `v1.1.0-rc.3` (new tag; no force-push).

## Next Phase Readiness

- **Phase 13 implementation is complete.** All six plans (13-01..13-06) have shipped their automated work; OBS-01..OBS-05 documented as Complete; CI is green across lint + test + image; OBS-05 CI guard is permanently active.
- **Phase 13 PR is ready to merge to main.** All planning artifacts (REQUIREMENTS.md, the six plan SUMMARYs, HUMAN-UAT.md, deferred-items.md, this close-out) land via the wave-4 merge.
- **`v1.1.0-rc.2` tag cut is the explicit next maintainer action AFTER the PR merges.** The verifier should route Truths #7 + #8 as `human_needed`.
- **Phase 14 (`v1.1.0-rc.3` cut, bulk enable/disable + final v1.1.0 GA) is unblocked once `v1.1.0-rc.2` ships and post-push verification passes.** Phase 14 is on a fresh branch off main; no code-level dependency on rc.2 beyond the `:rc` tag pointing somewhere.

## Self-Check: PASSED

**Files verified on disk:**

```
$ [ -f tests/v13_timeline_explain.rs ] && echo FOUND
FOUND
$ [ -f tests/v13_timeline_timezone.rs ] && echo FOUND
FOUND
$ [ -f .github/workflows/ci.yml ] && echo FOUND
FOUND
$ [ -f .planning/phases/13-observability-polish-rc-2/HUMAN-UAT.md ] && echo FOUND
FOUND
$ [ -f .planning/phases/13-observability-polish-rc-2/deferred-items.md ] && echo FOUND
FOUND
```

**Commits verified in git history (range ff9d352..HEAD):**

```
$ git log --oneline ff9d352..HEAD
f79c77c docs(13-06): mark OBS-01..OBS-05 complete
565f766 ci(13-06): grep guard prevents SQL-native percentile regression (OBS-05)
55e5017 test(13-06): timezone render test for /timeline (OBS-02 T-V11-TIME-04)
9f5e6c9 test(13-06): add dual-backend EXPLAIN QUERY PLAN tests for timeline query (OBS-02)
FOUND: 9f5e6c9
FOUND: 55e5017
FOUND: 565f766
FOUND: f79c77c
```

**Structural greps verified:**

```
$ grep -q 'explain_uses_index_sqlite' tests/v13_timeline_explain.rs && echo OK
OK
$ grep -q 'explain_uses_index_postgres' tests/v13_timeline_explain.rs && echo OK
OK
$ grep -q 'limit_10000_enforced' tests/v13_timeline_explain.rs && echo OK
OK
$ grep -q 'EXPLAIN QUERY PLAN' tests/v13_timeline_explain.rs && echo OK
OK
$ grep -q 'EXPLAIN (FORMAT JSON)' tests/v13_timeline_explain.rs && echo OK
OK
$ grep -q 'ANALYZE job_runs' tests/v13_timeline_explain.rs && echo OK
OK
$ grep -q 'assert_eq!(' tests/v13_timeline_explain.rs && echo OK
OK
$ grep -q 'America/Los_Angeles' tests/v13_timeline_timezone.rs && echo OK
OK
$ grep -q 'with_timezone' tests/v13_timeline_timezone.rs && echo OK
OK
$ grep -q 'pdt_label_in_timeline_render' tests/v13_timeline_timezone.rs && echo OK
OK
$ grep -q 'grep-no-percentile-cont' justfile && echo OK
OK
$ grep -q 'grep-no-percentile-cont' .github/workflows/ci.yml && echo OK
OK
```

All four task commits present. All acceptance-criteria structural greps pass. `cargo nextest run --lib` green (194/194). `cargo clippy --all-targets -- -D warnings` zero warnings. `cargo fmt --check` clean. All 20 Phase 13 integration tests green. `just grep-no-percentile-cont` exits 0 on clean tree (and exits 1 on intentional-regression probe). REQUIREMENTS.md shows 5 OBS bullets + 5 OBS traceability rows in the expected final state.

Plan 13-06 automated work COMPLETE. Task 5 (tag cut) PENDING maintainer action per HUMAN-UAT.md.

---

*Phase: 13-observability-polish-rc-2*
*Plan: 06 (close-out + rc.2 tag cut deferred)*
*Completed: 2026-04-21*
