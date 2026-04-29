# Phase 16: Failure-Context Schema + run.rs:277 Bug Fix - Context

**Gathered:** 2026-04-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Three tightly-coupled deliverables that establish the per-run failure-context substrate the webhook payload (Phase 18) and the failure-context UI panel (Phase 21) both consume:

1. **FOUND-14** — Fix the silent v1.1 bug at `src/scheduler/run.rs:301` (`container_id_for_finalize = docker_result.image_digest.clone()` — the local is named *container_id* but stores the *image digest*; passed at L355 to `finalize_run`'s `container_id` parameter, so `job_runs.container_id` has been silently storing `sha256:...` digests for docker jobs since v1.0). Add a proper `container_id: Option<String>` field to `DockerExecResult`. `finalize_run` populates `job_runs.container_id` with the real container ID and `job_runs.image_digest` with the digest. No data migration — historical deviation ages out via Phase 6 retention pruner.

2. **FCTX-04** — Add `job_runs.image_digest TEXT NULL` and `job_runs.config_hash TEXT NULL` per-run columns. `image_digest` flows from the post-start `inspect_container` site already captured in `DockerExecResult.image_digest` (`src/scheduler/docker.rs:240–251`) into `finalize_run`. `config_hash` is captured at fire time in `insert_running_run` from the in-memory `Config` (BEFORE the executor spawns, so reload-mid-fire reflects the run's actual config).

3. **FCTX-07** — Land `get_failure_context(job_id) -> FailureContext` in `src/db/queries.rs`. Single SQL query (not five round-trips). Returns `streak_position`, `consecutive_failures`, `last_success_run_id`, `last_success_image_digest`, `last_success_config_hash`. EXPLAIN QUERY PLAN on both SQLite and Postgres must use indexed access on `idx_job_runs_job_id_start (job_id, start_time DESC)` (the index already exists in the initial migration).

**Out of scope for Phase 16** (deferred to downstream phases — do not creep): failure-context UI panel rendering (Phase 21 / FCTX-01..06), webhook payload schema including `streak_position`/`consecutive_failures`/`image_digest`/`config_hash` (Phase 18 / WH-09), exit-code histogram (Phase 21 / EXIT-01..06), backfilled-row UI marker rendering (Phase 21 — Phase 16 only deposits the convention), retention-pruner changes for the historical-`container_id` deviation cleanup (already covered by v1.0 Phase 6 — no Phase 16 changes), three-file vs single-file migration ergonomics on the `image_digest` side (locked one-file).

</domain>

<decisions>
## Implementation Decisions

### Migration shape (Area 1)

- **D-01:** **Three migration files per backend, but each scoped to a single concern — NOT the structural add → backfill → NOT NULL chain.** PITFALLS §44 + §45 explicitly warn against the NOT NULL step here because both columns are meaningfully nullable forever:
  - **Image digest is nullable forever** because command/script jobs have no image and legitimately lack a digest.
  - **Config hash is nullable forever** because pre-v1.2 runs lack per-run captures (backfilled values are best-effort, not authoritative — see D-04).

  File layout per backend (`migrations/sqlite/` and `migrations/postgres/`, both with parallel content per the `tests/schema_parity.rs` invariant):

  ```
  20260427_000005_image_digest_add.up.sql        — ALTER TABLE job_runs ADD COLUMN image_digest TEXT;
  20260428_000006_config_hash_add.up.sql         — ALTER TABLE job_runs ADD COLUMN config_hash  TEXT;
  20260429_000007_config_hash_backfill.up.sql    — UPDATE job_runs SET config_hash = (SELECT j.config_hash FROM jobs j WHERE j.id = job_runs.job_id) WHERE config_hash IS NULL;
  ```

  Total: 3 files × 2 backends = 6 migration files. Numbering continues from v1.1's last migration (`20260422_000004_enabled_override_add`).

  **Rejected:**
  - **One combined ADD-COLUMN file** — collapses the two requirements (FOUND-14 image_digest plumbing and FCTX-04 config_hash plumbing) into one git-blame target; harder to attribute future regressions.
  - **v1.1 three-file pattern (add → backfill → NOT NULL)** — wrong shape because both columns stay nullable forever. Adding a future NOT NULL step would require a complex pre-step to scrub command/script rows of NULL image_digest and pre-cutoff rows of NULL config_hash; that's never going to happen.

  **Rationale for three files anyway (not just two):** keeping the `image_digest` migration separate from `config_hash` migrations preserves clean per-requirement attribution (FOUND-14 vs FCTX-04). The backfill is a separate file from the column-add so a re-run on a DB that already migrated past 005 can apply 006 + 007 cleanly.

### config_hash backfill policy (Area 2)

- **D-02:** **Best-effort backfill via single bulk UPDATE.** Migration `20260428..backfill.up.sql` runs:

  ```sql
  UPDATE job_runs
     SET config_hash = (SELECT j.config_hash FROM jobs j WHERE j.id = job_runs.job_id)
   WHERE config_hash IS NULL;
  ```

  No chunked loop in P16 — homelab DBs are unlikely to hit the >100k-row threshold where v1.1's `job_run_number` chunked-backfill pattern paid off. If a future operator hits scaling pain, v1.3 can introduce chunked backfill as a deliberate change; not worth the migration-Rust runtime in P16.

- **D-03:** **Backfilled values are semantically suspect — pre-cutoff rows reflect 'config_hash at backfill time,' not 'at run time.'** Migration file MUST carry a comment header documenting:
  - The backfill cutoff timestamp (the day the migration runs against any given DB).
  - The convention: rows where `end_time < <backfill_cutoff>` AND `config_hash IS NOT NULL` are backfilled, not authentic per-run captures.
  - Phase 21's FCTX UI is responsible for detecting backfilled rows and rendering "config history not available before upgrade" instead of a misleading "no config change since last success" comparison. Phase 16 deposits the convention; Phase 21 implements the marker.

  **Rationale:** Honest signal over filled-but-lying signal. The compromise of "best-effort with documented marker" preserves the ability to render *something* in the FCTX UI for old rows while protecting against the lie that "all old hashes are equal, therefore no config changes ever happened." The marker hook is a comment + convention — no new schema column needed.

  **Rejected:**
  - **No backfill (NULL forever)** — clean honest signal but loses the cheap UPDATE-statement win on disk-resident DBs that already have `jobs.config_hash` populated.
  - **A new `config_hash_backfilled BOOLEAN` column** — over-engineering; the `end_time < cutoff` heuristic is sufficient.

- **D-04:** **No backfill for `image_digest`.** Pre-v1.2 docker runs simply did not capture an image digest at write time. Backfilling from `jobs.config_json` (image name) would lose the digest information entirely (image name → digest requires an actual `inspect`, which we cannot do retroactively). UI for pre-v1.2 docker rows shows "—" for image_digest. Same shape as command/script rows (which legitimately lack a digest).

### `get_failure_context` SQL construction (Area 3)

- **D-05:** **CTE-based shape with two CTEs and a single SELECT join.** Locked sketch (final SQL is planner discretion, but the structural choice is set):

  ```sql
  WITH last_success AS (
    SELECT id            AS run_id,
           image_digest,
           config_hash,
           start_time
      FROM job_runs
     WHERE job_id = ?1 AND status = 'success'
     ORDER BY start_time DESC
     LIMIT 1
  ),
  streak AS (
    SELECT COUNT(*) AS consecutive_failures
      FROM job_runs
     WHERE job_id = ?1
       AND status IN ('failed', 'timeout', 'error')
       AND start_time > COALESCE(
             (SELECT start_time FROM last_success),
             '1970-01-01T00:00:00Z'
           )
  )
  SELECT
    streak.consecutive_failures,
    last_success.run_id        AS last_success_run_id,
    last_success.image_digest  AS last_success_image_digest,
    last_success.config_hash   AS last_success_config_hash
    FROM streak
    LEFT JOIN last_success ON 1=1;
  ```

  - Both CTEs hit `idx_job_runs_job_id_start (job_id, start_time DESC)` — `last_success` for the index-driven LIMIT 1, `streak` for the range scan above the last-success boundary.
  - `LEFT JOIN ... ON 1=1` returns one row even when `last_success` is empty (job has never succeeded) — `last_success_*` fields are NULL, `consecutive_failures` counts all failed/timeout/error rows.
  - `'1970-01-01T00:00:00Z'` epoch sentinel is the COALESCE fallback; `start_time` is RFC3339 TEXT per the initial migration's design notes, so string comparison is lexicographic and consistent across SQLite and Postgres.

- **D-06:** **`streak_position` is computed Rust-side, not in SQL.** The query returns `consecutive_failures: i64` and the caller (Phase 18 webhook payload, Phase 21 UI) computes `streak_position`:
  - `consecutive_failures == 1` → `streak_position = "first_failure"`
  - `consecutive_failures > 1` → `streak_position = "ongoing"`
  - `consecutive_failures == 0` → caller should not be calling `get_failure_context` (the run isn't a failure)

  Rationale: keeping the SQL purely about counts/lookups and the labeling Rust-side keeps the single-query promise (FCTX-07) clean and lets WH-06 (Phase 18 — webhook coalescing) reuse the same struct.

- **D-07:** **`FailureContext` struct shape.** Lives in `src/db/queries.rs` alongside `get_failure_context`:

  ```rust
  #[derive(Debug, Clone)]
  pub struct FailureContext {
      pub consecutive_failures: i64,
      pub last_success_run_id: Option<i64>,
      pub last_success_image_digest: Option<String>,
      pub last_success_config_hash: Option<String>,
  }
  ```

  No `streak_position` field on the struct (D-06). No `last_success_start_time` field — Phase 21 can fetch it via `get_run_by_id(last_success_run_id)` if the UI needs the full row; the streak query stays minimal.

- **D-08:** **EXPLAIN QUERY PLAN test on both backends.** Mirror v1.1's `tests/v13_timeline_explain.rs` precedent:
  - Run `EXPLAIN QUERY PLAN SELECT ...` (SQLite) / `EXPLAIN SELECT ...` (Postgres) against the get_failure_context SQL.
  - Assert the plan references `idx_job_runs_job_id_start` (SQLite) / `idx_job_runs_job_id_start` index access (Postgres) for both CTE branches.
  - Assert the plan does NOT contain `SCAN job_runs` without an index hit. Reuse the v1.1 OBS-05 `grep-no-percentile-cont`-style plain-text guard pattern for backend-agnostic phrasing.
  - One test file per backend: `tests/v12_fctx_explain_sqlite.rs` and `tests/v12_fctx_explain_pg.rs` (planner picks final names per the existing convention; `tests/v13_timeline_explain.rs` already has the SQLite/PG-paired shape to copy).

### Plan atomicity / PR shape (Area 4)

- **D-09 [informational]:** **Two PRs, six plans total.** *Revised during plan-phase iteration 1: Plan 16-04 was split into 16-04a (queries.rs signature changes only) + 16-04b (callers + recipe + gate) to satisfy the plan-checker's task-count threshold. The two-PR shape is preserved (PR 1 = 16-01..16-04b; PR 2 = 16-05..16-06); the plan count is now seven, not six. Tagged informational because the substantive constraint — 'two PRs, signature transition once per PR' — is honored, only the plan count shifted.*

  **PR 1 — Schema + bug fix wave (Plans 16-01 .. 16-04b).** One coherent commit set; `finalize_run` signature changes only once.
  - **Plan 16-01:** Three migration files per backend (image_digest add, config_hash add, config_hash backfill). `tests/schema_parity.rs` continues to assert structural parity. Migration test verifying both columns exist and are nullable.
  - **Plan 16-02:** `DockerExecResult` gains `pub container_id: Option<String>` field (`src/scheduler/docker.rs:62-68`); `execute_docker` populates it from `create_container` (`docker.rs:186`). `image_digest` field unchanged. Update the test fixture at `docker.rs:553+`.
  - **Plan 16-03:** `src/scheduler/run.rs:231..301` — rename `container_id_for_finalize` → keep the local but populate it from `docker_result.container_id.clone()` (the new field), NOT `image_digest`. Add a parallel `image_digest_for_finalize: Option<String>` local populated from `docker_result.image_digest.clone()`. Pass both to `finalize_run`.
  - **Plan 16-04:** `src/db/queries.rs::finalize_run` signature gains `image_digest: Option<&str>` parameter. Update the SQLite + Postgres UPDATE statements (L439, L453) to bind `image_digest` to the new column. `insert_running_run` (L368) gains `config_hash: &str` parameter and writes it to the new column at fire time. `DbRun` (~L554) and `DbRunDetail` (~L571) structs gain `pub image_digest: Option<String>` and `pub config_hash: Option<String>` fields. `get_run_by_id`, `get_run_history`, and any other SELECT site that hydrates a run row updates its column list accordingly. Caller `src/scheduler/run.rs::finalize_run` invocation at L348 passes `image_digest_for_finalize.as_deref()`. Caller `src/scheduler/fire.rs` (or wherever `insert_running_run` is invoked) passes the resolved-job's `config_hash`. Integration tests covering the bug-fix observable (real container_id in `job_runs.container_id`, not a sha256:...).

  **PR 2 — Streak helper (Plans 16-05 .. 16-06).**
  - **Plan 16-05:** `get_failure_context(pool, job_id) -> anyhow::Result<FailureContext>` query function in `src/db/queries.rs`. Single CTE-based SQL per D-05. `FailureContext` struct per D-07. Unit tests covering: (a) job has never succeeded, (b) job's most recent run is success, (c) one consecutive failure, (d) N consecutive failures, (e) success → fail → success → fail (streak resets to 1).
  - **Plan 16-06:** EXPLAIN QUERY PLAN tests per D-08 — one test file per backend, asserting indexed access on both CTE branches.

  **Rationale for PR shape:**
  - **PR 1's coupling is real** — the bug fix changes `finalize_run`'s signature; the schema migration also changes `finalize_run`'s signature. Splitting (a) and (b) would mean PR 1 introduces a temporary signature that PR 2 immediately rewrites. Same callsite churn within a day; bad review economy. Bundling = one clean signature transition.
  - **PR 2's separation is also real** — `get_failure_context` is read-only, has no callers in P16 (Phase 18 + Phase 21 wire it up), and the SQL-correctness review benefits from being separate from migration / wiring review.

  **Rejected:**
  - **Single PR for the full Phase 16** — too large to review carefully; Phase 15 D-12's revertability rationale applies.
  - **Three PRs (a / b / c)** — tightly coupled (a) + (b) would churn `finalize_run`'s signature twice within a day; bad shape per Phase 15 D-12's "no churn within a single phase" implicit rule.

### Project-rule reaffirmations (carried from prior phases — restated for downstream agents)

> D-10 through D-14 are project-wide rules carried from prior phases and enforced
> at the project level (CLAUDE.md, repo CI, MEMORY.md, schema parity test). They
> are tagged `[informational]` here because no single Phase 16 plan owns them —
> they apply to every plan implicitly. The decision-coverage gate skips
> `[informational]` rows. D-14 is also explicitly cited in the migration plans'
> `must_haves` as a per-plan constraint.

- **D-10 [informational]:** All changes land via PR on a feature branch. No direct commits to `main`. (Project rule.)
- **D-11 [informational]:** All diagrams in any artifact (planning docs, README, PR descriptions, code comments) are mermaid code blocks. No ASCII art. (Project rule.)
- **D-12 [informational]:** All UAT steps reference an existing `just` recipe. No ad-hoc `cargo`/`docker`/curl-URLs in UAT step text. (Project rule.)
- **D-13 [informational]:** UAT items in `16-HUMAN-UAT.md` (if produced by the planner) are validated by the maintainer running them locally — never marked passed from Claude's own runs. (Project rule.)
- **D-14:** Migration files MUST land in both `migrations/sqlite/` and `migrations/postgres/` in the same PR; `tests/schema_parity.rs` must remain green. (v1.0 Phase 1 D-16 / pattern reaffirmed; cited in 16-01's must_haves as a per-plan constraint.)
- **D-15:** No `percentile_cont`-style SQL-dialect-specific functions in `get_failure_context`. The CTE shape uses only standard SQL constructs available in both SQLite ≥ 3.25 and Postgres ≥ 12. (v1.1 OBS-05 lock — `just grep-no-percentile-cont` CI guard already enforces this codebase-wide.)

### Claude's Discretion

- **Exact migration file names** — `20260427_000005_image_digest_add.up.sql` etc. is illustrative; planner picks the actual date prefix on the day Plan 16-01 lands. Numbering must continue from `20260422_000004_enabled_override_add` (the v1.1 last migration) — `_000005`, `_000006`, `_000007` are the next free sequence numbers.
- **Backfill cutoff timestamp** — the comment in `..._config_hash_backfill.up.sql` must reference the backfill day, but planner decides the exact format ("2026-04-XX" vs Unix timestamp vs RFC3339). Phase 21's UI marker convention will read this comment / planner can also surface the cutoff via a settings table row if a comment-only convention feels too brittle. P16 only deposits the marker; P21 picks the read mechanism.
- **`FailureContext` struct location** — `src/db/queries.rs` is recommended (mirrors `DbRun`, `DbRunDetail` co-location), but a sibling file `src/db/failure_context.rs` is acceptable if the queries module is already large.
- **`get_failure_context` final SQL shape** — D-05's sketch is the structural anchor (CTE + LEFT JOIN). Planner may inline the streak CTE as a correlated subquery in the SELECT projection, or pull the `last_success.start_time` lookup into a separate scalar subquery, as long as both EXPLAIN tests still pass.
- **EXPLAIN test phrasing** — D-08 says "assert plan references the index"; planner picks string-match vs structural-walk per the v1.1 `tests/v13_timeline_explain.rs` precedent. SQLite's `EXPLAIN QUERY PLAN` output format is stable enough for substring assertion; Postgres `EXPLAIN` JSON output is more reliable than text — planner's call.
- **Whether `16-HUMAN-UAT.md` is needed.** Phase 16's deliverables are mostly DB-internal (schema, query helper) and code-internal (bug fix + signature change). The bug fix is verifiable via `sqlite3 cronduit.db "SELECT container_id FROM job_runs ORDER BY id DESC LIMIT 1"` — observably ≠ a `sha256:...` value for a docker run. Planner decides whether the maintainer-spot-check is worth a UAT runbook entry vs covered by integration tests.
- **Test file names** — `tests/v12_fctx_explain_sqlite.rs`, `tests/v12_fctx_explain_pg.rs`, `tests/v12_fctx_streak.rs`, `tests/v12_run_rs_277_bug_fix.rs` are illustrative. Planner picks final names per the existing `vNN_<feature>_<scenario>.rs` convention.
- **Schema parity test extension** — `tests/schema_parity.rs` may need new entries for the two new columns. Planner decides whether the existing test auto-detects them or needs an explicit allowlist update.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level

- `.planning/PROJECT.md` § Constraints, § Key Decisions — locked tech-stack rules (sqlx + SQLite/Postgres parity, no `percentile_cont`-style functions, structural parity invariant). § Current Milestone — v1.2 intent and where Phase 16 sits in the build order.
- `.planning/REQUIREMENTS.md` § Foundation — FOUND-14 (run.rs:301 bug fix; `DockerExecResult` carries both `container_id` and `image_digest`; `finalize_run` populates `job_runs.container_id` with real ID and `job_runs.image_digest` with digest). § Failure Context — FCTX-04 (`job_runs.config_hash TEXT NULL` per-run column; written from `insert_running_run` at fire time; conservative backfill from `jobs.config_hash`), FCTX-07 (`get_failure_context` single SQL query; EXPLAIN QUERY PLAN indexed on `job_runs.job_id + start_time`). T-V12-FCTX-01..09 are the verification-lock test identifiers spanning P16 and P21.
- `.planning/ROADMAP.md` § "Phase 16: Failure-Context Schema + run.rs:277 Bug Fix" — goal, success criteria (3 operator-observable behaviors), depends-on (Phase 15), v1.2 build-order graph (P16 must land before P18 + P21).
- `.planning/STATE.md` § Accumulated Context → Decisions — v1.2 inherited decisions (Option A for config_hash, run.rs:277 bug fix mechanics, single-query helper requirement).

### Research

- `.planning/research/SUMMARY.md` § Research-Phase Corrections — Correction 1 (run.rs:277 bug verbatim diagnosis with the misnamed local variable explanation), Correction 2 (`job_runs.config_hash` schema gap; Option A locked at requirements step).
- `.planning/research/ARCHITECTURE.md` § Failure Context — full per-feature analysis. Notes the `image_digest` write-site authoritative source decision (post-start `inspect_container` at `docker.rs:240`, NOT pre-flight pull). Lists the cross-file impact of the migration wave (`finalize_run` signature, `DbRun` / `DbRunDetail` struct shape, every SELECT site that hydrates a run row).
- `.planning/research/PITFALLS.md` Pitfall 44 (`config_hash` per-JOB vs per-RUN gap), Pitfall 45 (`image_digest` not yet persisted; nullable-forever shape), Pitfall 46 (the run.rs:277 misnaming history). § T-V12-FCTX-01..09 verification-lock test identifiers.
- `.planning/research/FEATURES.md` § Feature 3 — Failure Context — feature landscape, the streak/last-success query rationale, the `consecutive_failures` Rust-side computation rationale (rejected SQL-side window-function path).

### Phase 15 precedent (immediate predecessor in v1.2)

- `.planning/phases/15-foundation-preamble/15-CONTEXT.md` § Plan sequencing within Phase 15 (D-12) — the precedent for splitting tightly-coupled hygiene + feature work into atomic plans within a single phase. P16's two-PR shape (D-09) extends the same logic.
- `.planning/phases/15-foundation-preamble/15-CONTEXT.md` § `RunFinalized` channel-message contract (D-02) — the webhook event struct that Phase 18 will enrich with `streak_position` / `consecutive_failures` / `image_digest` / `config_hash` from the `get_failure_context` query Phase 16 lands. P16's struct fields are the exact set Phase 18's payload (WH-09) will consume.

### v1.1 migration precedents

- `.planning/milestones/v1.1-phases/11-per-job-run-numbers-log-ux-fixes/11-CONTEXT.md` (or the actual phase dir name) — three-file migration pattern for `job_run_number` (add nullable → backfill → NOT NULL). **Reference for shape only — Phase 16 explicitly REJECTS the NOT NULL step** because both new columns are nullable forever.
- `migrations/sqlite/20260416_000001_job_run_number_add.up.sql` + `..._000002_job_run_number_backfill.up.sql` + `..._000003_job_run_number_not_null.up.sql` — concrete file shape and numbering convention to mirror in Phase 16's Plan 16-01.
- `migrations/sqlite/20260422_000004_enabled_override_add.up.sql` — single-file nullable-tri-state precedent. Closer in shape to Phase 16's `image_digest` migration (one file, nullable forever).

### EXPLAIN test precedents

- `tests/v13_timeline_explain.rs` (or the actual filename produced by Phase 13) — v1.1 OBS-02 EXPLAIN QUERY PLAN test that verified the timeline query used indexed access on both backends. Phase 16's `tests/v12_fctx_explain_*.rs` test files mirror this shape — same backend pairing, same plan-string-match-or-walk pattern.
- `justfile` recipe `grep-no-percentile-cont` — the CI guard that enforces no SQL-dialect-specific percentile functions. Phase 16's CTE shape (D-05) uses only standard SQL constructs and remains compliant.

### Source files the phase touches

- `Cargo.toml` — no version bump in P16 (still `1.2.0` from Phase 15 / FOUND-15). No new dependencies expected.
- `migrations/sqlite/20260427_000005_image_digest_add.up.sql` — NEW (Plan 16-01).
- `migrations/sqlite/20260428_000006_config_hash_add.up.sql` — NEW (Plan 16-01).
- `migrations/sqlite/20260429_000007_config_hash_backfill.up.sql` — NEW (Plan 16-01).
- `migrations/postgres/20260427_000005_image_digest_add.up.sql` — NEW (Plan 16-01; structural parity).
- `migrations/postgres/20260428_000006_config_hash_add.up.sql` — NEW (Plan 16-01; structural parity).
- `migrations/postgres/20260429_000007_config_hash_backfill.up.sql` — NEW (Plan 16-01; structural parity).
- `src/scheduler/docker.rs` L62-L68 — `DockerExecResult` struct gains `pub container_id: Option<String>` field. Plan 16-02.
- `src/scheduler/docker.rs` L186-L210 — `execute_docker` populates the new field from `create_container` return value. Plan 16-02.
- `src/scheduler/docker.rs` L97-L141, L233, L307-L313 — every early-return DockerExecResult literal site gains `container_id: None,`. Plan 16-02.
- `src/scheduler/docker.rs` L413-L417 — happy-path return populates `container_id: Some(container_id.clone())`. Plan 16-02.
- `src/scheduler/docker.rs` L553+ — test fixture / mock literal updated. Plan 16-02.
- `src/scheduler/run.rs:231` — `container_id_for_finalize` local kept; populated from `docker_result.container_id.clone()` (the new field). Plan 16-03.
- `src/scheduler/run.rs:231` — NEW parallel local `image_digest_for_finalize: Option<String>` populated from `docker_result.image_digest.clone()`. Plan 16-03.
- `src/scheduler/run.rs:301` — the bug site. `container_id_for_finalize = docker_result.image_digest.clone()` becomes `container_id_for_finalize = docker_result.container_id.clone(); image_digest_for_finalize = docker_result.image_digest.clone();`. Plan 16-03.
- `src/scheduler/run.rs:348-356` — `finalize_run(...)` invocation gains `image_digest_for_finalize.as_deref()` argument. Plan 16-04.
- `src/scheduler/fire.rs` (the `insert_running_run` invocation site) — pass the resolved job's `config_hash` to `insert_running_run`. Plan 16-04.
- `src/db/queries.rs::insert_running_run` (~L368) — signature gains `config_hash: &str`; INSERT statement adds the column to the column list and bind list (both SQLite and Postgres branches). Plan 16-04.
- `src/db/queries.rs::finalize_run` (~L424) — signature gains `image_digest: Option<&str>`; UPDATE statement adds the column to the SET list (both SQLite L439 and Postgres L453 branches); bind order updated. Plan 16-04.
- `src/db/queries.rs::DbRun` (~L554) and `DbRunDetail` (~L571) — both structs gain `pub image_digest: Option<String>` and `pub config_hash: Option<String>` fields. Plan 16-04.
- `src/db/queries.rs::get_run_by_id`, `get_run_history`, and any other SELECT site that hydrates a run row — column lists updated. Plan 16-04.
- `src/db/queries.rs` — NEW `pub struct FailureContext { ... }` (D-07) and `pub async fn get_failure_context(pool: &DbPool, job_id: i64) -> anyhow::Result<FailureContext>` (D-05). Plan 16-05.
- `tests/schema_parity.rs` — may need updating to include the new columns in any explicit allowlist. Plan 16-01 / Claude's Discretion.
- `tests/v12_run_rs_277_bug_fix.rs` (or similar name) — NEW integration test asserting `job_runs.container_id` after a docker run is the real container ID, not a `sha256:...` digest. Plan 16-03.
- `tests/v12_fctx_streak.rs` — NEW unit/integration test for `get_failure_context` correctness across the 5 streak scenarios (D-07 / Plan 16-05).
- `tests/v12_fctx_explain_sqlite.rs`, `tests/v12_fctx_explain_pg.rs` — NEW EXPLAIN QUERY PLAN tests (D-08 / Plan 16-06).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`idx_job_runs_job_id_start (job_id, start_time DESC)` index** (`migrations/sqlite/20260410_000000_initial.up.sql:46`) already exists in the initial migration. Phase 16's `get_failure_context` CTE is designed to land on this index for both arms (last_success LIMIT 1 + streak range scan). No new index needed for FCTX-07's "indexed access" success criterion.
- **`DockerExecResult.image_digest` capture site** (`src/scheduler/docker.rs:240-251`) — `inspect_container(&container_id, None).await` post-`start_container` already captures the digest into the returned struct. v1.0 wired this through to `continue_run` for SSE log-frame emission, but the value is currently NEVER persisted to the DB. P16's Plan 16-04 wires it to `finalize_run` → `job_runs.image_digest`.
- **`docker.create_container` return value** (`src/scheduler/docker.rs:186-187`) — the actual container ID is bound to a local `let container_id = ...`. v1.0/v1.1 used it for `start_container` / `inspect_container` / log streaming, but never stashed it in `DockerExecResult`. P16's Plan 16-02 adds the field; the value capture is one extra `.clone()` at the existing site.
- **`finalize_run` signature precedent** (`src/db/queries.rs:424-431`) — already takes `container_id: Option<&str>` as the last positional. Adding `image_digest: Option<&str>` follows the same shape; bind-list extension is mechanical.
- **Three-file migration shape** (`migrations/sqlite/20260416_000001_job_run_number_add.up.sql` + `..._000002_..._backfill.up.sql` + `..._000003_..._not_null.up.sql`) — file naming convention, content shape, and per-backend parallelism are the reference. Phase 16 reuses the *naming and parallelism* but rejects the third NOT NULL step (D-01).
- **Single-file nullable migration shape** (`migrations/sqlite/20260422_000004_enabled_override_add.up.sql`) — closer in shape to P16's `image_digest` migration. Single ALTER TABLE; no backfill; nullable forever.
- **`tests/schema_parity.rs`** — structural-parity invariant test. New columns added to either backend MUST also be added to the other; this test is the green-or-red signal for D-14.
- **EXPLAIN-test pattern** (`tests/v13_timeline_explain.rs`) — v1.1's OBS-02 already wrote a SQLite-side EXPLAIN QUERY PLAN test with `idx_job_runs_*` substring assertion. P16 mirrors this for `get_failure_context` and adds a Postgres equivalent.
- **`compute_config_hash`** (`src/config/hash.rs`) and `crate::scheduler::sync` — the in-memory `Config` already exposes a `config_hash: String` per `JobConfig` at L93+. `insert_running_run`'s caller (in `fire.rs`) has access to the resolved job's hash at fire time without needing a DB round-trip.

### Established Patterns

- **Per-backend SQL branch** (`src/db/queries.rs::insert_running_run` ~L368-L420; `finalize_run` ~L437-L466) — the codebase splits SQLite and Postgres SQL into separate `match` arms with `?N`-style vs `$N`-style placeholders. P16's signature changes (image_digest, config_hash) MUST update both arms; the existing function bodies are the template.
- **`DbRun` / `DbRunDetail` co-location with queries** — these structs live alongside the queries that produce them. P16's `FailureContext` struct follows the same convention (D-07).
- **No `percentile_cont`-style dialect-specific SQL functions** — v1.1 OBS-05 lock; CI guard via `just grep-no-percentile-cont`. P16's CTE (D-05) uses only standard SQL.
- **`vNN_<feature>_<scenario>.rs` test naming** — `tests/v11_bulk_toggle.rs`, `tests/v13_timeline_explain.rs`, `tests/dashboard_jobs_pg.rs`. P16 follows `v12_fctx_*` and `v12_run_rs_277_bug_fix` (or similar).
- **No `auto_remove=true` on bollard containers** — v1.0 D-25 lock; D-29 explicit-post-drain-remove pattern. P16's bug fix does NOT touch the docker lifecycle; the new `container_id` field is captured at `create_container` time before the wait/drain/remove sequence (no race with cleanup).
- **`#[derive(Debug, Clone)]` on read-side structs** (DbRun, DbRunDetail, RunFinalized from Phase 15) — convention for query-result structs. `FailureContext` matches.

### Integration Points

- **`src/scheduler/docker.rs::DockerExecResult` struct** (L62-L68) — the field-add site (Plan 16-02). All early-return literal sites in `execute_docker` and the test fixture must update.
- **`src/scheduler/run.rs:231-301`** (the bug site) — the assignment fix and the parallel-local additions (Plan 16-03). Comment block at the top of `finalize_run` will need a brief note explaining the v1.0/v1.1 deviation in `job_runs.container_id` historical data.
- **`src/scheduler/run.rs:348-356`** — `finalize_run` invocation site (Plan 16-04). Add `image_digest_for_finalize.as_deref()` argument.
- **`src/scheduler/fire.rs`** (the `insert_running_run` caller) — pass `config_hash` at fire time. Caller has access to the resolved job's `config_hash` from the in-memory `Config` (no DB round-trip needed).
- **`src/db/queries.rs::insert_running_run`** (~L368) — signature + INSERT statement (Plan 16-04).
- **`src/db/queries.rs::finalize_run`** (~L424) — signature + UPDATE statement (Plan 16-04).
- **`src/db/queries.rs::DbRun`, `DbRunDetail`, `get_run_by_id`, `get_run_history`** — struct fields + SELECT column lists (Plan 16-04). Any web template that consumes `DbRun` / `DbRunDetail` (e.g., the run-detail page) will see new optional fields but should compile cleanly because the existing template fields are unchanged.
- **`migrations/{sqlite,postgres}/`** — three new files per backend (Plan 16-01). `tests/schema_parity.rs` continues to assert structural parity.
- **No webhook/UI changes in P16.** The webhook payload (Phase 18 / WH-09) and the FCTX UI panel (Phase 21 / FCTX-01..06) consume `get_failure_context` but are not wired up in Phase 16. The query helper exists in isolation, validated by its own EXPLAIN + correctness tests.
- **No retention-pruner changes.** The historical `job_runs.container_id` deviation (rows holding `sha256:...` values from v1.0/v1.1) ages out via the existing v1.0 Phase 6 pruner — no Phase 16 work.
- **No `Cargo.toml` changes.** No new dependencies.

</code_context>

<specifics>
## Specific Ideas

- **The bug at `src/scheduler/run.rs:301` is the load-bearing operator-visible defect this phase exists to fix.** "v1.1.0's `job_runs.container_id` column has been silently storing image digests for docker jobs" (research SUMMARY Correction 1). Operators inspecting the DB after v1.2 lands MUST see the real container ID for new docker runs, with old `sha256:...` rows aging out via retention. Test surface: `tests/v12_run_rs_277_bug_fix.rs` should fire a real docker run (testcontainers) and assert `job_runs.container_id` does NOT start with `sha256:`. The integration-test cost (testcontainers spin-up) is justified — this is the entire reason FOUND-14 has its own REQ-ID.
- **Two columns in one phase is an intentional bundle.** Phase 18's webhook payload (WH-09) needs `streak_position`, `consecutive_failures`, `image_digest`, `config_hash` to all flow through the same struct. Splitting `image_digest` and `config_hash` into separate phases would force two reload-mid-fire-correctness reviews. Phase 16 is the schema-rev wave; Phase 17 (Custom Docker Labels) is the config-rev wave; both feed Phase 18's payload.
- **`config_hash` capture must happen BEFORE the executor spawns**, not after. ARCHITECTURE: "The hash is captured BEFORE the executor spawns, so even if a reload happens mid-fire, the row reflects the config that the run was based on." Plan 16-04's `insert_running_run` signature change is the locked seam — `config_hash` is bound at row-insert time, NOT updated later.
- **The bundled-PR shape is asymmetric on purpose.** PR 1 (a+b) is large and review-heavy; PR 2 (c) is small and self-contained. The asymmetry is intentional: the SQL-correctness review of `get_failure_context` benefits from being a focused review without competition from migration-shape and signature-change diffs. v1.1's `get_dashboard_jobs` Postgres bug (Phase 13 deferred → Quick Task `260421-nn3`) is the cautionary tale — a single SQL bug can hide in a large PR; isolating the helper PR raises the chance of catching it before merge.
- **`streak_position` is a label, not a column.** Phase 16 stores no per-run `streak_position` value. Phase 18's webhook payload computes the label string ("first_failure" / "ongoing") from the `consecutive_failures` count. Phase 21's UI does the same. This keeps the per-run row write-once: status, exit_code, image_digest, config_hash — never updated after `finalize_run`. Streak metadata is derived at read-time from the row sequence.
- **The `FailureContext` struct is a query result, NOT a domain entity.** It does not get persisted, does not gain a row in any table, and does not get serialized into `config_json`. It is recomputed from `job_runs` on every read. WH-06 (webhook coalescing) and FCTX UI both call `get_failure_context(job_id)` afresh; no caching layer in P16.
- **The historical-`container_id` deviation cleanup is NOT urgent.** "Old runs with `container_id = sha256:...` age out via Phase 6 retention pruner; no data migration" (FOUND-14). Operators on a 90-day retention default see the deviation fully cleared by ~2026-07-26 (3 months after Phase 16 lands). The README / `THREAT_MODEL.md` Phase 24 close-out may want a one-liner note acknowledging this — but P16 ships no READMME or migration-guide changes.
- **EXPLAIN test on Postgres is structural, not performance.** The success criterion (FCTX-07.3) is "indexed access on `job_runs.job_id + start_time`" — i.e., the query plan uses the index, not that the query is fast. Postgres `EXPLAIN` (without ANALYZE) is sufficient; no need for the more expensive `EXPLAIN ANALYZE`. SQLite `EXPLAIN QUERY PLAN` is similarly structural.

</specifics>

<deferred>
## Deferred Ideas

- **Backfilled-row UI marker rendering** — Phase 21 (FCTX UI panel) is responsible for detecting `end_time < backfill_cutoff AND config_hash IS NOT NULL` rows and rendering "config history not available before upgrade" instead of a misleading delta. Phase 16 deposits the convention (D-03) but does not render it.
- **`webhook` payload fields including `streak_position` / `consecutive_failures` / `image_digest` / `config_hash`** — Phase 18 (WH-09) consumes `get_failure_context` and serializes the fields into the payload. Phase 16 ships the helper, not the payload.
- **Failure-context UI panel itself** — Phase 21 (FCTX-01 through FCTX-06). Five P1 signals; collapsed-by-default; gated to failed/timeout statuses. Not a Phase 16 surface.
- **`webhook_drain_grace = "30s"`** — Phase 20 / WH-10. Not Phase 16's territory.
- **Chunked-loop backfill for very large DBs** (≥100k rows) — D-02 explicitly chose bulk single UPDATE. If a future operator hits scaling pain, v1.3 introduces chunked backfill as a deliberate change.
- **`config_hash_backfilled BOOLEAN` column** — rejected as over-engineering; the `end_time < backfill_cutoff` heuristic plus the migration-comment convention is sufficient.
- **`last_success_start_time` field on `FailureContext`** — D-07 explicitly omits this. Phase 21 fetches the full `last_success` row via `get_run_by_id(last_success_run_id)` if needed; the streak query stays minimal.
- **Window-function shape** for `get_failure_context` (rejected D-05 alternative) — `MAX(...) FILTER (WHERE status='success') OVER (...)`. Tighter SQL but introduces a structural-parity hazard between SQLite and Postgres `FILTER` clause edge cases. CTE shape is the conservative choice.
- **NOT NULL step on either column** — explicitly rejected by D-01 + research. Both columns stay nullable forever. A future v1.3+ phase could in theory tighten one or both, but only after a deliberate scrub of pre-cutoff rows; no such tightening is in any v1.2 phase.
- **Promote `cargo-deny` from non-blocking to blocking** — Phase 24 / FOUND-16 milestone close-out. Not Phase 16's surface.
- **Per-job `streak_position` column on `job_runs`** — rejected explicitly. Streak is derived at read time, not stored.
- **`16-HUMAN-UAT.md` scope** — Claude's-discretion item per D-09 / Plan 16-04. Planner picks whether the maintainer post-migration spot check ("`SELECT container_id FROM job_runs ORDER BY id DESC LIMIT 1` is not a `sha256:...` value") warrants a runbook entry.

</deferred>

---

*Phase: 16-failure-context-schema-run-rs-277-bug-fix*
*Context gathered: 2026-04-26*
