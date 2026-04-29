# Phase 16: Failure-Context Schema + run.rs:277 Bug Fix - Research

**Researched:** 2026-04-27
**Domain:** Rust + sqlx schema migration wave + bug fix + read-only query helper (SQLite + Postgres parity)
**Confidence:** HIGH

## Summary

CONTEXT.md is comprehensive — D-01..D-15 lock the architecture. This research is a **codebase-verification pass** plus answers to the targeted questions about call-site location, line-number drift, schema-parity test shape, EXPLAIN test idioms, and migration mechanics.

**Headline findings:**

1. **Line numbers in CONTEXT.md are mostly correct, with two notable corrections.** The bug at `run.rs:301` is verified verbatim. `DockerExecResult` literal sites need a slightly different list than CONTEXT.md cites (six early-return sites + one happy-path + one test fixture). Most importantly, `insert_running_run` is NOT called from `fire.rs` — it's called from `run.rs:83` and `web/handlers/api.rs:82`, with a third caller (the api.rs error fallback at L131) that calls `finalize_run` and must also be updated.
2. **`tests/schema_parity.rs` is dynamic** — it introspects both backends and diffs, with no allowlist. Adding two new TEXT columns requires **zero changes** to this test. CONTEXT.md's "may need updating" can be resolved as "no test changes needed."
3. **Project does NOT use `sqlx::query!` macros.** All queries are `sqlx::query()` with runtime SQL strings. `cargo sqlx prepare` is NOT load-bearing. No `.sqlx/` cache exists. Adding columns has no compile-time-checking burden.
4. **No `down.sql` convention** — all 5 existing migrations are up-only. P16 follows the same shape.
5. **EXPLAIN test precedent at `tests/v13_timeline_explain.rs` is mature** — both SQLite (substring match on `idx_*`) and Postgres (JSON walk for `Index Scan` + textual fallback for `idx_*` reference) idioms already coexist in one file with documented testcontainers + `ANALYZE` setup.

**Primary recommendation:** Honor CONTEXT.md's lock verbatim, but apply six concrete corrections to the cited file/line list (Section A) so Plans 16-02..04 are exhaustive. Use a **single test file pair** for EXPLAIN (`tests/v12_fctx_explain.rs` covering both backends in one file is consistent with `v13_timeline_explain.rs`'s structure).

## Architectural Responsibility Map

This is a backend-only phase. No tier-misassignment risk — every change lives in the same tier.

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `DockerExecResult.container_id` field | Scheduler/Executor | — | Per-run state captured by `bollard::create_container` lives in the executor result struct. |
| `job_runs.image_digest` / `config_hash` columns | Database/Storage | — | Persistent per-run state; new nullable columns in the existing `job_runs` table. |
| `insert_running_run` config_hash plumbing | Scheduler → Database | — | Caller is in the scheduler tier (`run.rs::run_job` and `api.rs::run_now`); DB write is in the queries tier. |
| `finalize_run` image_digest plumbing | Scheduler → Database | — | Same call-site shape as the existing `container_id` parameter. |
| `get_failure_context` read query | Database/Storage | — | Read-only helper in `src/db/queries.rs`; consumed in P18 + P21 (no callers in P16). |
| EXPLAIN tests | Test infrastructure | Database | Asserts query plan via `EXPLAIN QUERY PLAN` (SQLite) / `EXPLAIN (FORMAT JSON)` (Postgres). |

## Project Constraints (from CLAUDE.md)

Extracted directives. The planner MUST verify task plans against this list — same authority as CONTEXT.md `<decisions>`.

- **Tech stack locked:** Rust + `bollard` (no shelling out). [VERIFIED: CLAUDE.md, PROJECT.md § Constraints]
- **Persistence locked:** `sqlx` + SQLite default + Postgres optional. Same logical schema, per-backend migration files. Separate read/write SQLite pools (WAL + busy_timeout). [VERIFIED]
- **No `percentile_cont`-style SQL functions.** CI guard via `just grep-no-percentile-cont`. [VERIFIED: justfile L197-215]
- **All diagrams are mermaid.** No ASCII art. [VERIFIED: CLAUDE.md, MEMORY.md]
- **All changes via PR on a feature branch.** No direct commits to `main`. [VERIFIED]
- **Tag and `Cargo.toml` version must match.** Already at `1.2.0` from Phase 15; no bump in P16. [VERIFIED]
- **All UAT steps reference an existing `just` recipe.** No ad-hoc `cargo`/`docker`/`curl` in UAT step text. [VERIFIED]
- **UAT validated by maintainer running locally** — never marked passed from Claude's runs. [VERIFIED]
- **Migration parity invariant:** SQLite + Postgres files in the same PR; `tests/schema_parity.rs` must remain green. [VERIFIED]

## User Constraints (from CONTEXT.md)

### Locked Decisions

D-01 — Three migration files per backend (image_digest add, config_hash add, config_hash backfill); 6 files total. NO NOT-NULL step on either column (both nullable forever). Numbering `_000005`, `_000006`, `_000007`.

D-02 — Best-effort backfill via single bulk `UPDATE job_runs SET config_hash = (SELECT j.config_hash FROM jobs j WHERE j.id = job_runs.job_id) WHERE config_hash IS NULL`. NO chunked loop; NO Rust orchestrator.

D-03 — Backfill migration carries a comment header documenting the cutoff timestamp + the convention that `end_time < cutoff AND config_hash IS NOT NULL` rows are backfilled (P21 reads this convention; P16 only deposits it).

D-04 — NO backfill for `image_digest`. Pre-v1.2 docker rows show "—" forever.

D-05 — `get_failure_context` SQL uses two CTEs (`last_success`, `streak`) + LEFT JOIN ON 1=1. Epoch sentinel `'1970-01-01T00:00:00Z'`. Standard SQL only.

D-06 — `streak_position` computed Rust-side; SQL returns counts/lookups only.

D-07 — `FailureContext` struct: `consecutive_failures: i64`, `last_success_run_id: Option<i64>`, `last_success_image_digest: Option<String>`, `last_success_config_hash: Option<String>`. Lives in `src/db/queries.rs`.

D-08 — EXPLAIN QUERY PLAN test on both backends; mirror `tests/v13_timeline_explain.rs`. Assert plan references `idx_job_runs_job_id_start`.

D-09 — Two PRs, six plans total. PR 1 = Plans 16-01..04 (migrations + bug fix + signature changes). PR 2 = Plans 16-05..06 (streak helper + EXPLAIN tests).

D-10..D-15 — Project-rule reaffirmations (PR-only, mermaid-only, just-only UAT, maintainer-validated UAT, structural-parity invariant, no-percentile_cont).

### Claude's Discretion

- Exact migration date prefix (the day Plan 16-01 lands; numbering `_000005`/`_000006`/`_000007` is fixed).
- Backfill cutoff timestamp format (RFC3339 vs ISO date).
- `FailureContext` struct location (`src/db/queries.rs` recommended; sibling file acceptable).
- `get_failure_context` final SQL shape (CTE sketch is structural; planner may inline as correlated subquery).
- EXPLAIN test phrasing (substring match for SQLite; JSON walk vs text match for Postgres — both already in `v13_timeline_explain.rs`).
- Whether `16-HUMAN-UAT.md` is needed (Phase 16 is mostly DB-internal).
- Test file names following `vNN_<feature>_<scenario>.rs` convention.
- Whether `tests/schema_parity.rs` needs updating (this research resolves: NO — see Section E).

### Deferred Ideas (OUT OF SCOPE)

Backfilled-row UI marker rendering (P21); webhook payload serialization (P18 / WH-09); FCTX UI panel (P21); chunked-loop backfill (v1.3 if scaling pain emerges); `config_hash_backfilled BOOLEAN` column (rejected); `last_success_start_time` field on FailureContext (P21 fetches via `get_run_by_id`); window-function SQL shape (rejected); NOT NULL step on either column (rejected); per-job `streak_position` column (derived, not stored); `cargo-deny` blocking promotion (P24); `webhook_drain_grace = "30s"` (P20).

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FOUND-14 | `DockerExecResult` carries both `container_id` and `image_digest`; `finalize_run` populates `job_runs.container_id` with the real container ID and `job_runs.image_digest` with the digest | Sections A, F (verified bug at run.rs:301; six early-return sites; finalize_run signature shape) |
| FCTX-04 | New `job_runs.config_hash TEXT NULL` per-run column; written from `insert_running_run` at fire time; conservative backfill from `jobs.config_hash` | Sections A, B, C (verified `DbJob.config_hash` already in scope at call sites; migration-numbering sequence confirmed; backfill SQL is portable) |
| FCTX-07 | `get_failure_context` single SQL query with EXPLAIN-verified indexed access on `idx_job_runs_job_id_start` on both backends | Section D (verified index exists at `migrations/sqlite/20260410_000000_initial.up.sql:45` and Postgres equivalent; `v13_timeline_explain.rs` is the proven test pattern) |

## Standard Stack

No new dependencies. P16 is pure DB-schema + Rust-source delta on top of the v1.2.0 stack already locked in PROJECT.md / CLAUDE.md.

### Already in use (relevant subset)

| Library | Version | Purpose | Why Relevant |
|---------|---------|---------|--------------|
| `sqlx` | 0.8.x | Async DB | Per-backend SQL branching pattern already established in `queries.rs::insert_running_run` / `finalize_run` [VERIFIED: queries.rs L368-468]. |
| `bollard` | 0.20.x | Docker API | `create_container` returns `ContainerCreateResponse { id, ... }`; the `id` field is the real container ID currently captured into a local at `docker.rs:186-190` and used for `start_container`/`inspect_container` calls but never returned from `execute_docker` [VERIFIED]. |
| `chrono` | 0.4.x | Timestamps | Used for `start_time` RFC3339 strings in `job_runs` [VERIFIED]. |
| `anyhow` | 1.0.x | Error aggregation | `get_failure_context -> anyhow::Result<FailureContext>` follows the existing convention [VERIFIED: queries.rs `insert_running_run`/`finalize_run` signatures]. |
| `testcontainers` / `testcontainers-modules` | 0.27.x / 0.15.x | Real-Postgres integration tests | Used by `tests/v13_timeline_explain.rs::explain_uses_index_postgres`; mirror that setup for the FCTX EXPLAIN test [VERIFIED]. |

**Installation:** No `cargo add` needed.

**Version verification:** No version verification needed since no new dependencies.

## Architecture Patterns

### System Architecture Diagram

```mermaid
flowchart TB
    subgraph fire_path[Fire path -- per-run config_hash capture]
        SCH[Scheduler tick / SchedulerCmd]
        RUN[run.rs::run_job<br/>L83 insert_running_run]
        API[web/handlers/api.rs::run_now<br/>L82 insert_running_run]
        IRR[(insert_running_run<br/>queries.rs L368<br/>+config_hash param)]
    end

    subgraph exec_path[Exec path -- container_id + image_digest capture]
        DEXEC[docker.rs::execute_docker<br/>L78-L417]
        DR[(DockerExecResult<br/>L62-L68<br/>+container_id field)]
        FIN[finalize_run<br/>queries.rs L424<br/>+image_digest param]
    end

    subgraph read_path[Read path -- failure context]
        GFC[(get_failure_context<br/>queries.rs NEW)]
        FC[FailureContext struct<br/>NEW]
    end

    subgraph db[(SQLite default / Postgres optional)]
        JR[(job_runs table<br/>+image_digest col<br/>+config_hash col)]
        IDX[idx_job_runs_job_id_start<br/>job_id, start_time DESC]
    end

    SCH --> RUN
    SCH -.RunNowWithRunId.-> RUN
    API --> IRR
    RUN -- DbJob.config_hash --> IRR
    IRR -- INSERT row + config_hash --> JR
    RUN --> DEXEC
    DEXEC -- container_id from create_container L186 --> DR
    DEXEC -- image_digest from inspect_container L240 --> DR
    DR --> FIN
    FIN -- UPDATE container_id, image_digest --> JR
    GFC -- WITH last_success / streak CTE --> JR
    JR -.. uses ..-> IDX
    GFC --> FC

    classDef new fill:#0a3d0a,stroke:#00ff7f,stroke-width:2px,color:#e0ffe0
    classDef changed fill:#3d2a1a,stroke:#ffbf7f,stroke-width:2px,color:#ffe0c0
    class GFC,FC new
    class IRR,FIN,DR changed
```

### Recommended Project Structure

No new directories. P16 changes existing files only.

```
src/
├── scheduler/
│   ├── docker.rs          # +container_id field on DockerExecResult; populate at L186
│   └── run.rs             # +image_digest_for_finalize local; fix L301 bug; pass config_hash + image_digest to finalize_run
├── db/
│   └── queries.rs         # +config_hash param on insert_running_run; +image_digest param on finalize_run; +DbRun.image_digest, .config_hash; +DbRunDetail.image_digest, .config_hash; +SELECT column lists; +get_failure_context + FailureContext struct
└── web/handlers/
    └── api.rs             # +finalize_run error fallback at L131 must pass image_digest=None
migrations/
├── sqlite/
│   ├── 20260427_000005_image_digest_add.up.sql       # NEW
│   ├── 20260428_000006_config_hash_add.up.sql        # NEW
│   └── 20260429_000007_config_hash_backfill.up.sql   # NEW
└── postgres/
    ├── 20260427_000005_image_digest_add.up.sql       # NEW
    ├── 20260428_000006_config_hash_add.up.sql        # NEW
    └── 20260429_000007_config_hash_backfill.up.sql   # NEW
tests/
├── v12_run_rs_277_bug_fix.rs          # NEW (Plan 16-03; testcontainers integration)
├── v12_fctx_streak.rs                  # NEW (Plan 16-05; CTE correctness across 5 scenarios)
└── v12_fctx_explain.rs                 # NEW (Plan 16-06; mirrors v13_timeline_explain shape — both backends in one file)
```

### Pattern 1: Per-backend SQL with separate `match pool.{reader,writer}()` arms

**What:** Every query in `src/db/queries.rs` matches on `PoolRef::Sqlite(p)` vs `PoolRef::Postgres(p)`, with `?N` placeholders for SQLite and `$N` for Postgres. The body is otherwise identical.

**When to use:** Any query that hits both backends. P16 follows this for `insert_running_run`, `finalize_run`, the SELECTs in `get_run_history` / `get_run_by_id`, and the new `get_failure_context`.

**Example:** [VERIFIED: src/db/queries.rs L437-465 finalize_run]
```rust
match pool.writer() {
    PoolRef::Sqlite(p) => {
        sqlx::query("UPDATE job_runs SET ... = ?1, ... = ?2 WHERE id = ?N")
            .bind(...).execute(p).await?;
    }
    PoolRef::Postgres(p) => {
        sqlx::query("UPDATE job_runs SET ... = $1, ... = $2 WHERE id = $N")
            .bind(...).execute(p).await?;
    }
}
```

### Pattern 2: Three-file migration per backend with up-only SQL

**What:** Each migration is a pair of `.up.sql` files (SQLite + Postgres). No `.down.sql` files exist anywhere in the project. SQLite uses bare `ALTER TABLE ADD COLUMN`; Postgres uses `ALTER TABLE ADD COLUMN IF NOT EXISTS`.

**Verified template:** [VERIFIED: migrations/sqlite/20260422_000004_enabled_override_add.up.sql L14 vs migrations/postgres/20260422_000004_enabled_override_add.up.sql L10]
- SQLite: `ALTER TABLE jobs ADD COLUMN enabled_override INTEGER;`
- Postgres: `ALTER TABLE jobs ADD COLUMN IF NOT EXISTS enabled_override BIGINT;`

For TEXT columns (P16's case), both backends use plain `TEXT`; `tests/schema_parity.rs::normalize_type` accepts `TEXT | VARCHAR | CHARACTER VARYING | CHAR | CHARACTER → TEXT` so the columns will pass parity automatically.

### Pattern 3: EXPLAIN tests with one file containing both backends

**What:** `tests/v13_timeline_explain.rs` ships SQLite and Postgres EXPLAIN tests as two `#[tokio::test]` functions in a single file [VERIFIED: tests/v13_timeline_explain.rs L46 + L157]. P16 should follow this — `tests/v12_fctx_explain.rs` with `explain_uses_index_sqlite` and `explain_uses_index_postgres` functions.

**SQLite idiom** [VERIFIED: tests/v13_timeline_explain.rs L130-149]:
```rust
let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
let rows = sqlx::query(&explain_sql).bind(...).fetch_all(pool_ref).await.unwrap();
let plan_text: String = rows.iter().map(|r| r.get::<String, _>("detail"))
    .collect::<Vec<_>>().join("\n");
assert!(plan_text.contains("idx_job_runs_job_id_start"),
        "expected EXPLAIN to use the index; got:\n{plan_text}");
```

**Postgres idiom** [VERIFIED: tests/v13_timeline_explain.rs L254-319]:
```rust
let explain_sql = format!("EXPLAIN (FORMAT JSON) {sql}");
// 1. Seed enough rows + run ANALYZE to defeat fresh-testcontainer cardinality guesses.
// 2. Walk the plan JSON tree for "Node Type" matching Index Scan / Index Only Scan /
//    Bitmap Index Scan / Bitmap Heap Scan.
// 3. Documented fallback: accept textual presence of "idx_job_runs_job_id_start"
//    anywhere in the plan as proof the index is reachable. This guards against
//    fresh-testcontainer Seq Scan flake without weakening the regression lock.
```

### Anti-Patterns to Avoid

- **Calling `cargo sqlx prepare` after the migration:** project does NOT use `query!`/`query_as!` macros [VERIFIED: only one mention in queries.rs L6 doc comment confirming "Uses `sqlx::query_as` with runtime SQL strings (not the `query!` macro)"]; no `.sqlx/` cache exists. The `just sqlx-prepare` recipe exists for hypothetical future use but is not load-bearing for P16. Plans should NOT include a sqlx-prepare step.
- **Adding `tests/schema_parity.rs` allowlist entries:** the parity test is dynamic introspection [VERIFIED: tests/schema_parity.rs L41-62 normalize_type covers TEXT already]. Adding two TEXT columns requires zero test changes.
- **Using `IF NOT EXISTS` on the SQLite `ALTER TABLE`:** SQLite's `ALTER TABLE ADD COLUMN` does NOT support `IF NOT EXISTS` (Postgres does). Idempotency on SQLite is provided by sqlx's `_sqlx_migrations` table. [CITED: SQLite official docs — ALTER TABLE syntax]
- **Marker-file pattern for the backfill** (v1.1 `_000002_job_run_number_backfill.up.sql` is `SELECT 1; -- no-op` because the real backfill runs in Rust). P16's D-02 explicitly chose **plain SQL** bulk UPDATE — no Rust orchestrator. The file contains real SQL.
- **Running EXPLAIN on Postgres without `ANALYZE` first:** fresh testcontainer statistics default to "guess cardinality" → planner often picks Seq Scan even with an index present. v13_timeline_explain.rs L227-234 documents this and runs `ANALYZE` after seed.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| EXPLAIN test setup | Custom plan-parser | Idiom from `tests/v13_timeline_explain.rs` | Battle-tested across both backends; testcontainer + ANALYZE + fallback assertion already proven. |
| Postgres testcontainer wiring | Manual Docker spin-up | `testcontainers_modules::postgres::Postgres` | Already in scope across the integration test suite. |
| Backfill orchestration | Rust loop with progress logs | Single SQL `UPDATE` (D-02) | Bulk UPDATE on a homelab DB <100k rows completes in milliseconds. v1.1's chunked Rust orchestrator (`migrate_backfill.rs`) was needed for `NOT NULL` semantics; D-01 explicitly rejects the NOT NULL step here. |
| Run-row hydration | Per-call manual struct construction | Existing `DbRun`/`DbRunDetail` patterns at queries.rs L1070, L1104, L1146, L1165 | Pattern is established; just append `r.get("image_digest")` and `r.get("config_hash")` to the existing `.map(\|r\| ...)` calls. |
| Streak math in SQL | Window functions / `FILTER` clauses | CTE sketch in D-05; count-since-last-success | D-05 explicitly rejected the window-function shape due to SQLite/Postgres `FILTER` edge cases. CTE is portable. |
| Schema-parity allowlist updates | New entries in normalize_type | Nothing — `TEXT` already accepted | normalize_type collapses `TEXT \| VARCHAR \| CHARACTER VARYING \| CHAR \| CHARACTER → "TEXT"` already. |

**Key insight:** Most of P16's "build" work is mechanical because every shape (per-backend SQL branching, `Option<&str>` parameter additions, `Option<String>` struct fields, EXPLAIN test idioms, three-file migration sequencing) already exists at well-known sites in the codebase.

## Runtime State Inventory

This is a code+schema phase, not a rename/migration phase, BUT the bug-fix half (FOUND-14) has historical-state implications worth surfacing:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | `job_runs.container_id` rows from v1.0/v1.1 currently store `sha256:...` digests instead of real container IDs (the v1.0 bug). [VERIFIED: src/scheduler/run.rs:301 stores `docker_result.image_digest.clone()` into `container_id_for_finalize`] | **No action in P16.** Per FOUND-14 + CONTEXT.md: historical rows age out via Phase 6 retention pruner (90-day default). README/THREAT_MODEL note is deferred to Phase 24. |
| Live service config | None — no Cronduit-managed external service stores phase-specific state. | None — verified by grep across `src/` for any external-service registration. |
| OS-registered state | None — Cronduit does not register OS-level tasks. | None — verified by absence of any systemd unit / Docker socket-side state. |
| Secrets/env vars | None — P16 introduces no new env vars or secrets. | None. |
| Build artifacts / installed packages | The `cronduit:1.2.0` Docker image is built fresh per release; no stale artifact concern. | None — every `just image` rebuilds from source. |

**The canonical question — after every file change, what runtime systems still have old data?**
Answer: only `job_runs.container_id` rows from v1.0/v1.1. These are intentionally allowed to age out via retention (FOUND-14 / CONTEXT.md).

## Common Pitfalls

### Pitfall 1: Forgetting the `finalize_run` error-fallback caller in `web/handlers/api.rs:131`

**What goes wrong:** Plan 16-04 changes `finalize_run`'s signature to add `image_digest: Option<&str>`. CONTEXT.md cites `src/scheduler/run.rs:348` as the caller, but there's a SECOND caller — the api.rs error-fallback when the scheduler channel is closed [VERIFIED: src/web/handlers/api.rs:131-140]. Missing this caller = compile error.
**How to avoid:** Plan 16-04's task list MUST enumerate BOTH callers explicitly: (a) `src/scheduler/run.rs:348-356` happy path, (b) `src/web/handlers/api.rs:131-140` shutdown-fallback. Both pass `None` for image_digest in the api.rs case (the run never started a container).
**Warning signs:** Compile failure in the api module; cargo clippy on plan 16-04.

### Pitfall 2: Forgetting the run.rs:794 + queries.rs test calls of `insert_running_run`

**What goes wrong:** Plan 16-04 changes `insert_running_run`'s signature to add `config_hash: &str`. There are two test-call sites: `src/scheduler/run.rs:794` (`run_job_with_existing_run_id_skips_insert` test pre-inserts a row) and several in `src/db/queries.rs` tests (L1833, L1874, L1923, L1983 — all in `mod tests`). [VERIFIED via grep] All must pass a test-fixture `config_hash` value.
**How to avoid:** Plan 16-04's task list includes "update all test-side `insert_running_run` calls in src/ to pass a fixture `&str` (e.g., `"testhash"`)."
**Warning signs:** `cargo test` compilation errors after the signature change.

### Pitfall 3: SQLite `ALTER TABLE ADD COLUMN` does not support `IF NOT EXISTS`

**What goes wrong:** Copying the Postgres-side `ADD COLUMN IF NOT EXISTS` pattern into the SQLite migration produces a syntax error on first run. [CITED: SQLite docs — ALTER TABLE]
**How to avoid:** SQLite migration uses `ALTER TABLE job_runs ADD COLUMN image_digest TEXT;` (no IF NOT EXISTS). Postgres uses `ALTER TABLE job_runs ADD COLUMN IF NOT EXISTS image_digest TEXT;`. sqlx's `_sqlx_migrations` tracking provides idempotency on SQLite. [VERIFIED: pattern in `migrations/sqlite/20260422_000004_enabled_override_add.up.sql` vs Postgres pair]
**Warning signs:** Migration test crash on second `pool.migrate()` call; `tests/migrations_idempotent.rs::migrate_is_idempotent_and_creates_expected_tables` fails.

### Pitfall 4: Postgres EXPLAIN flake on fresh testcontainer

**What goes wrong:** Postgres planner without `ANALYZE` defaults to guessing cardinality and often picks Seq Scan even when an index exists. Test fails non-deterministically.
**How to avoid:** Mirror `v13_timeline_explain.rs::explain_uses_index_postgres` setup: seed >1000 rows with selective predicate, run `ANALYZE job_runs` AND `ANALYZE jobs`, use `EXPLAIN (FORMAT JSON)`, walk the plan tree for `Index Scan`/`Index Only Scan`/`Bitmap Index Scan`/`Bitmap Heap Scan` AND accept textual `idx_job_runs_job_id_start` presence as a documented fallback. [VERIFIED: tests/v13_timeline_explain.rs L227-319]
**Warning signs:** Flaky CI on the postgres EXPLAIN test cell; "Seq Scan" in the failure dump.

### Pitfall 5: `bollard 0.20` `create_container` returns `id: String`, not `Option<String>`

**What goes wrong:** Plumbing the new `container_id` field into `DockerExecResult` as `Option<String>` requires wrapping in `Some(container_id.clone())` at the happy path. Wrong wrapping = `Option<String>` mismatch with the assignment.
**How to avoid:** [VERIFIED: src/scheduler/docker.rs:186-190] `let container_id = match docker.create_container(...).await { Ok(response) => response.id, Err(e) => ... }`. The local `container_id: String` is non-optional. Plan 16-02 captures `Some(container_id.clone())` in the happy-path return at L413-416 and `container_id: None` in all early-return literals (six sites — see Section A).
**Warning signs:** Type mismatch errors in docker.rs after the field add.

### Pitfall 6: `image_digest` may be `String::new()` on inspect_container failure

**What goes wrong:** Current code at `docker.rs:240-251` falls back to `String::new()` when `inspect_container` fails [VERIFIED]. The field type is currently `Option<String>` so `Some(String::new())` is technically a "successful but empty" digest. After P16 wires this to `job_runs.image_digest`, an empty string would be DB-stored — visually indistinguishable from a real digest in raw SQL inspection.
**How to avoid:** Plan 16-02 should consider mapping `String::new()` → `None` at the return site (`if image_digest.is_empty() { None } else { Some(image_digest) }`). Out-of-scope tightening, but flag for the planner — it's a 3-line change in the same plan and prevents downstream UI ambiguity.
**Warning signs:** UI shows blank rows when failure-context is rendered for runs where inspect_container succeeded fast-followed by container removal.

### Pitfall 7: Backfill cutoff comment must use a deterministic format

**What goes wrong:** D-03 requires the backfill migration to document the cutoff timestamp in the file header for P21's UI marker. Inconsistent format (RFC3339 vs ISO date vs Unix) makes P21's parser brittle.
**How to avoid:** Pick RFC3339 UTC (matches existing `start_time` column convention). Format: `-- BACKFILL_CUTOFF_RFC3339: 2026-04-XX T00:00:00Z` as a structured comment line P21 can grep for.
**Warning signs:** P21 cannot reliably parse the cutoff; UI marker logic forks per-format.

## Code Examples

Verified patterns from the existing codebase:

### Adding a column on both backends

```sql
-- migrations/sqlite/20260427_000005_image_digest_add.up.sql
ALTER TABLE job_runs ADD COLUMN image_digest TEXT;

-- migrations/postgres/20260427_000005_image_digest_add.up.sql
ALTER TABLE job_runs ADD COLUMN IF NOT EXISTS image_digest TEXT;
```
Source: [VERIFIED: migrations/sqlite/20260422_000004_enabled_override_add.up.sql + Postgres pair]

### Bulk single-statement backfill (D-02 pattern, NEW for P16)

```sql
-- migrations/sqlite/20260429_000007_config_hash_backfill.up.sql
-- BACKFILL_CUTOFF_RFC3339: 2026-04-XXT00:00:00Z
-- (Marker per Phase 16 D-03; Phase 21's UI panel reads this convention to
--  distinguish backfilled rows from authentic per-run captures.)
UPDATE job_runs
   SET config_hash = (SELECT j.config_hash FROM jobs j WHERE j.id = job_runs.job_id)
 WHERE config_hash IS NULL;

-- Postgres equivalent uses the same SQL — both backends accept this UPDATE shape.
```

### `finalize_run` signature extension

[VERIFIED: existing src/db/queries.rs L424-468 pattern]

```rust
pub async fn finalize_run(
    pool: &DbPool,
    run_id: i64,
    status: &str,
    exit_code: Option<i32>,
    start_instant: tokio::time::Instant,
    error_message: Option<&str>,
    container_id: Option<&str>,
    image_digest: Option<&str>,        // NEW (Plan 16-04)
) -> anyhow::Result<()> {
    // ...
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            sqlx::query(
                "UPDATE job_runs SET status = ?1, exit_code = ?2, end_time = ?3, \
                 duration_ms = ?4, error_message = ?5, container_id = ?6, \
                 image_digest = ?7 WHERE id = ?8",
            )
            .bind(status).bind(exit_code).bind(&now).bind(duration_ms)
            .bind(error_message).bind(container_id).bind(image_digest).bind(run_id)
            .execute(p).await?;
        }
        PoolRef::Postgres(p) => {
            // Same shape with $1..$8 placeholders.
        }
    }
    Ok(())
}
```

### `get_failure_context` shape (D-05 sketch)

```rust
pub async fn get_failure_context(
    pool: &DbPool,
    job_id: i64,
) -> anyhow::Result<FailureContext> {
    let sql_sqlite = r#"
        WITH last_success AS (
            SELECT id AS run_id, image_digest, config_hash, start_time
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
        SELECT streak.consecutive_failures,
               last_success.run_id        AS last_success_run_id,
               last_success.image_digest  AS last_success_image_digest,
               last_success.config_hash   AS last_success_config_hash
          FROM streak
          LEFT JOIN last_success ON 1=1
    "#;
    // Postgres variant identical except $1 placeholders.
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let row = sqlx::query(sql_sqlite).bind(job_id).fetch_one(p).await?;
            Ok(FailureContext {
                consecutive_failures: row.get("consecutive_failures"),
                last_success_run_id: row.get("last_success_run_id"),
                last_success_image_digest: row.get("last_success_image_digest"),
                last_success_config_hash: row.get("last_success_config_hash"),
            })
        }
        PoolRef::Postgres(p) => { /* same with $1 */ }
    }
}
```

Note: `LEFT JOIN ... ON 1=1` returns one row even when `last_success` is empty — the three `last_success_*` columns return as NULL via sqlx → `Option<...>`, which matches `FailureContext`'s `Option<i64>` / `Option<String>` field types.

## State of the Art

This is a Rust + sqlx project; no major framework "old vs new" questions arise in P16. The schema and query approach is the same shape as v1.0/v1.1.

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `cron` crate (Phase 1 candidate) | `croner 3.0` (locked) | v1.0 | N/A in P16 |
| `askama_axum` (deprecated) | `askama_web 0.15` with `axum-0.8` feature (locked) | v1.0 | N/A in P16 |
| Three-file migration with NOT NULL step (v1.1 `job_run_number` precedent) | Three-file migration WITHOUT NOT NULL step (P16 D-01) | P16 | Pattern shift — mirror v1.1's file naming + per-backend parallelism but skip the NOT NULL tightening because both new columns are nullable forever. |

**Deprecated/outdated for this phase:**

- v1.1's chunked Rust-side backfill orchestrator (`src/db/migrate_backfill.rs`) — D-02 rejects this for P16 in favor of plain SQL. Rust orchestrator stays in place for `job_run_number` migration; P16 does not modify it.

## Assumptions Log

All major claims in this research are VERIFIED against the live codebase via `Read`/`Grep` tool calls during this session. The few ASSUMED claims:

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `EXPLAIN QUERY PLAN` text output is stable enough for substring assertion across SQLite minor versions (3.35+ is bundled with Rust's `sqlx` SQLite feature). | Pitfall 4 / Pattern 3 | Plan 16-06 SQLite test could flake on a future SQLite bump if the plan-text format changes. Mitigation: v13_timeline_explain.rs has run green for ~2 weeks across the same SQLite version range — regression risk is low. [ASSUMED based on training data] |
| A2 | Postgres `EXPLAIN (FORMAT JSON)` schema (specifically the `Node Type` field) is stable across Postgres 13–17 (testcontainers default tag). | Pitfall 4 / Pattern 3 | Plan 16-06 Postgres test could break on a Postgres major version bump if `Node Type` semantics change. Mitigation: v13_timeline_explain.rs's textual fallback (substring match for `idx_job_runs_*`) is the documented cushion. [ASSUMED based on training data] |
| A3 | `bollard 0.20` `ContainerCreateResponse.id` is always non-empty when the call returns `Ok(_)`. | Pitfall 5 | If bollard returns an empty `id` on success (extremely unlikely), the persisted `job_runs.container_id` would be empty-string, indistinguishable from NULL. Mitigation: defensive check `if id.is_empty() { None } else { Some(id) }` in Plan 16-02. [ASSUMED based on bollard source-code reading from training; not freshly verified this session] |

If any of A1/A2/A3 turn out to be wrong, the affected plan can ship a defensive fallback in the same PR — none of these would block P16.

## Open Questions (RESOLVED)

1. **Should Plan 16-02 also tighten the `image_digest = String::new()` fallback to `None`?**
   - What we know: `docker.rs:240-251` falls back to `String::new()` on `inspect_container` error. CONTEXT.md does not address this explicitly.
   - What's unclear: Whether the planner treats this as "in scope for P16" or "deferred to P21 (UI rendering normalizes)."
   - Recommendation: Include in Plan 16-02 — it's a 3-line change on the same touched lines, and prevents an empty-string image_digest from being persisted into `job_runs.image_digest`. P21's UI is then guaranteed `Option<String>` semantics with no empty-string special case.
   - **Resolution:** Adopted recommendation — Plan 16-02 (T1, Step 3) tightens the fallback to `None` per recommendation; persisted into `job_runs.image_digest` as `Option<String>` semantics with no empty-string special case.

2. **Should the EXPLAIN tests live in one file (`tests/v12_fctx_explain.rs` with both backends) or two files (`_sqlite.rs` + `_pg.rs`)?**
   - What we know: `tests/v13_timeline_explain.rs` uses ONE file with two `#[tokio::test]` functions. CONTEXT.md D-08 says "one test file per backend" but offers planner discretion.
   - What's unclear: Which idiom the planner prefers.
   - Recommendation: Single file (`tests/v12_fctx_explain.rs`) to mirror v13_timeline_explain.rs precedent. Reduces file count, keeps the parallel SQLite/Postgres assertions side-by-side for review.
   - **Resolution:** Single file adopted in Plan 16-06 — `tests/v12_fctx_explain.rs` carries both backend `#[tokio::test]` functions side-by-side, mirroring `tests/v13_timeline_explain.rs`.

3. **What test-fixture value should new `insert_running_run` calls in test code pass for `config_hash`?**
   - What we know: Plan 16-04 changes the signature; multiple test sites must be updated.
   - What's unclear: Whether to use a literal `"testhash"` (matches existing convention at queries.rs:579) or to compute via `compute_config_hash` (more realistic).
   - Recommendation: Use literal `"testhash"` for Plan 16-04 test sites unchanged from the `upsert_job` helper convention. New tests in Plan 16-05 (streak-correctness) should use distinct hash values per scenario where the test asserts on `last_success_config_hash`.
   - **Resolution:** Adopted recommendation — Plan 16-04b (T3) uses the literal `"testhash"` for all 5 mechanical test-site updates; Plan 16-05 (T3) uses distinct hash values (`"test-config-A"`, `"v1"`, `"v2"`) per scenario where assertion needs distinguishability.

## Environment Availability

P16 has no new external dependencies. The dependencies it touches are all already verified working in v1.0/v1.1 CI:

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Docker daemon | testcontainers (FOUND-14 integration test, EXPLAIN Postgres test) | Required for CI | matches CI runners | None — P16 cannot ship without Docker available for the integration tests |
| `cargo-nextest` | `just nextest` (CI gate) | ✓ | (matches v1.1 CI) | `cargo test` (slower) |
| Postgres image (`postgres:latest` via testcontainers-modules) | EXPLAIN postgres test, schema_parity test | ✓ | (matches existing tests) | None |
| SQLite (in-memory via sqlx) | All SQLite-side tests | ✓ | (bundled in sqlx feature) | None |

**Missing dependencies with no fallback:** None — every dependency P16 needs is already in the v1.1 CI matrix.

**Missing dependencies with fallback:** None.

## Validation Architecture

Workflow `nyquist_validation: true` per `.planning/config.json`. This section is required.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` / `cargo nextest` (sqlx + tokio + testcontainers integration); idiomatic Rust 2024 / rust-version 1.94.1 |
| Config file | `Cargo.toml` (workspace), `nextest.toml` (if present), `tests/common/` (shared fixtures) |
| Quick run command | `just test` or `cargo test --test <NAME>` |
| Full suite command | `just nextest` |
| Schema parity | `just schema-diff` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FOUND-14 | `DockerExecResult.container_id` is captured from `create_container` and persisted to `job_runs.container_id` (real ID, not sha256:...) | integration (testcontainers) | `cargo test --test v12_run_rs_277_bug_fix` | ❌ Wave 0 (Plan 16-03) |
| FOUND-14 | `DockerExecResult.image_digest` is captured from `inspect_container` and persisted to `job_runs.image_digest` | integration (testcontainers) | `cargo test --test v12_run_rs_277_bug_fix` | ❌ Wave 0 (Plan 16-03) |
| FOUND-14 | All `DockerExecResult` early-return literals carry `container_id: None` | unit (compile + existing test_docker_exec_result_debug) | `cargo test --lib` | ✅ existing (extend at docker.rs:553) |
| FCTX-04 | `job_runs.image_digest TEXT NULL` and `job_runs.config_hash TEXT NULL` columns exist on both backends and pass parity | integration | `just schema-diff` (existing dynamic introspection) | ✅ tests/schema_parity.rs |
| FCTX-04 | Migration is idempotent across re-runs | integration | `cargo test --test migrations_idempotent` | ✅ existing |
| FCTX-04 | Backfill migration sets `config_hash` from `jobs.config_hash` for all pre-existing `job_runs` rows | integration | `cargo test --test v12_fctx_config_hash_backfill` | ❌ Wave 0 (Plan 16-01) |
| FCTX-04 | New runs after upgrade have `config_hash` matching the resolved job's hash at fire time (T-V12-FCTX-03) | integration | `cargo test --test v12_fctx_streak::write_site_captures_config_hash` | ❌ Wave 0 (Plan 16-04) |
| FCTX-04 | Reload between fires changes `config_hash` for subsequent runs (T-V12-FCTX-04) | integration | `cargo test --test v12_fctx_streak::reload_changes_config_hash` | ❌ Wave 0 (Plan 16-04) |
| FCTX-07 | `get_failure_context` returns correct `consecutive_failures` for 5 streak scenarios (T-V12-FCTX-12) | integration | `cargo test --test v12_fctx_streak` | ❌ Wave 0 (Plan 16-05) |
| FCTX-07 | `get_failure_context` returns `last_success_run_id = None` when job has never succeeded (T-V12-FCTX-13) | integration | `cargo test --test v12_fctx_streak::no_successes_returns_none` | ❌ Wave 0 (Plan 16-05) |
| FCTX-07 | `EXPLAIN QUERY PLAN` on SQLite uses `idx_job_runs_job_id_start` (T-V12-FCTX-15) | integration | `cargo test --test v12_fctx_explain explain_uses_index_sqlite` | ❌ Wave 0 (Plan 16-06) |
| FCTX-07 | `EXPLAIN (FORMAT JSON)` on Postgres uses `Index Scan` / `Bitmap Index Scan` on `idx_job_runs_job_id_start` (T-V12-FCTX-15) | integration | `cargo test --test v12_fctx_explain explain_uses_index_postgres` | ❌ Wave 0 (Plan 16-06) |

**Verification-lock test ID mapping (T-V12-FCTX-01..09 from PITFALLS.md):**
- T-V12-FCTX-01 (migration `config_hash` exists nullable) → schema parity test + `migrations_idempotent.rs` extension
- T-V12-FCTX-02 (backfill from current `jobs.config_hash`) → new `v12_fctx_config_hash_backfill.rs` (Plan 16-01)
- T-V12-FCTX-03 (write site captures hash at fire time) → `v12_fctx_streak.rs` (Plan 16-04)
- T-V12-FCTX-04 (reload changes hash mid-fires) → `v12_fctx_streak.rs` (Plan 16-04)
- T-V12-FCTX-05 (UI delta — defer to P21)
- T-V12-FCTX-06 (UI marker — defer to P21)
- T-V12-FCTX-07 (docker run captures `image_digest` non-null sha256:) → `v12_run_rs_277_bug_fix.rs` (Plan 16-03)
- T-V12-FCTX-08 (command run leaves `image_digest = NULL`) → `v12_run_rs_277_bug_fix.rs` (Plan 16-03)
- T-V12-FCTX-09 (digest changes across daemon pulls — testcontainers integration) → `v12_run_rs_277_bug_fix.rs` (Plan 16-03)
- T-V12-FCTX-12 (streak correctness 5 scenarios) → `v12_fctx_streak.rs` (Plan 16-05)
- T-V12-FCTX-13 (no-successes-ever) → `v12_fctx_streak.rs` (Plan 16-05)
- T-V12-FCTX-14 (retention-pruned graceful) → `v12_fctx_streak.rs` (Plan 16-05)
- T-V12-FCTX-15 (EXPLAIN both backends) → `v12_fctx_explain.rs` (Plan 16-06)

### Sampling Rate

- **Per task commit:** `cargo test --test <NAME>` (5-30s)
- **Per wave merge:** `just nextest` + `just schema-diff` (~2 min)
- **Phase gate:** Full `just ci` green before `/gsd-verify-work` (~5-10 min)

### Wave 0 Gaps

The following test files do NOT exist yet and must be created by the listed plan:
- [ ] `tests/v12_fctx_config_hash_backfill.rs` — covers FCTX-04 backfill scenario (Plan 16-01)
- [ ] `tests/v12_run_rs_277_bug_fix.rs` — covers FOUND-14 docker-run real-container-id observable (Plan 16-03; testcontainers integration)
- [ ] `tests/v12_fctx_streak.rs` — covers FCTX-04 write site + FCTX-07 streak correctness, all 5 scenarios (Plans 16-04 + 16-05)
- [ ] `tests/v12_fctx_explain.rs` — covers FCTX-07 EXPLAIN on both backends (Plan 16-06)

No framework install needed — all tooling already in v1.1 CI.

## Security Domain

`security_enforcement` is implicit (CLAUDE.md threat-model-first posture). P16 is backend internal; no new external surface. Brief assessment:

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | P16 changes no auth surface; web UI remains unauthenticated per v1 posture (PROJECT.md). |
| V3 Session Management | no | No session changes. |
| V4 Access Control | no | No new endpoints; `get_failure_context` is callable only from internal Rust code in P16 (P18/P21 wire it up). |
| V5 Input Validation | yes | `get_failure_context` takes `job_id: i64` — bound parameter; SQL injection protected by sqlx parameterization. The new `image_digest`/`config_hash` columns store strings sourced from `bollard::inspect_container` (trusted Docker daemon) and `compute_config_hash` (internal SHA-256 hex), respectively — no user-controlled paths. |
| V6 Cryptography | no | `compute_config_hash` already uses SHA-256 (locked v1.0). No new crypto. |

### Known Threat Patterns for the v1.2 stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| SQL injection via `job_id` parameter | Tampering | sqlx `bind(job_id)` parameterization (already used for every existing query in `queries.rs`) |
| Unbounded resource consumption from large `consecutive_failures` count | DoS | `streak` CTE counts via SQL `COUNT(*)` with index lookup — O(N) walk over the failed-runs range, bounded by retention (90-day default). No Rust-side iteration. |
| Information disclosure via `image_digest` | Information Disclosure | Image digests are not secrets (every `docker pull` exposes them). The `THREAT_MODEL.md` v1 posture documents that `job_runs` content is operator-visible by design. |

No new threats introduced by P16.

## Specific Phase Findings

### A. Live-codebase alignment with CONTEXT.md's line-number assertions

[All findings VERIFIED via `Read` against the current working tree on branch `phase-16-context`.]

| CONTEXT.md citation | Current reality | Drift? |
|---------------------|-----------------|--------|
| `run.rs:231` declares `container_id_for_finalize` local | `let mut container_id_for_finalize: Option<String> = None;` at run.rs:**231** | None |
| `run.rs:301` is the bug site | `container_id_for_finalize = docker_result.image_digest.clone();` at run.rs:**301** | None — bug verbatim |
| `run.rs:348-356` finalize_run invocation | `if let Err(e) = finalize_run(...).await { ... }` spanning L348-357 | None |
| `docker.rs:62-68` DockerExecResult struct | Struct at L62-68, fields `exec: ExecResult` + `image_digest: Option<String>` | None |
| `docker.rs:186-187` create_container site | `let container_id = match docker.create_container(...).await { Ok(response) => response.id, ... }` at L186-206 | Slight — actual block extends to L206 (early-return on Err); the `container_id` String is bound at L190 |
| `docker.rs:240-251` inspect_container image_digest capture | `let image_digest = match docker.inspect_container(&container_id, None).await { Ok(info) => info.image.unwrap_or_default(), Err(e) => ... String::new() }` at L240-251 | None |
| `docker.rs:413-417` happy-path return | `DockerExecResult { exec: exec_result, image_digest: Some(image_digest) }` at L413-416 | Minor — block ends at L416 (no L417); negligible |
| `docker.rs:553+` test fixture | `let result = DockerExecResult { exec: ExecResult { exit_code: Some(0), status: RunStatus::Success, error_message: None }, image_digest: Some("sha256:abc123".to_string()), };` at L553-560 | None |
| `queries.rs:368` insert_running_run | `pub async fn insert_running_run(pool: &DbPool, job_id: i64, trigger: &str) -> anyhow::Result<i64>` at L368 | None |
| `queries.rs:424` finalize_run | `pub async fn finalize_run(pool: &DbPool, run_id: i64, status: &str, exit_code: Option<i32>, start_instant: tokio::time::Instant, error_message: Option<&str>, container_id: Option<&str>) -> anyhow::Result<()>` at L424-432 | None |
| `queries.rs:439` SQLite UPDATE in finalize_run | `"UPDATE job_runs SET status = ?1, exit_code = ?2, end_time = ?3, duration_ms = ?4, error_message = ?5, container_id = ?6 WHERE id = ?7"` at L439-440 | None |
| `queries.rs:453` Postgres UPDATE in finalize_run | `"UPDATE job_runs SET status = $1, ... WHERE id = $7"` at L453-454 | None |
| `queries.rs:554` DbRun struct | `pub struct DbRun { ... }` at L554-567 | None |
| `queries.rs:571` DbRunDetail struct | `pub struct DbRunDetail { ... }` at L571-584 | None |
| Initial migration index `idx_job_runs_job_id_start` | `CREATE INDEX IF NOT EXISTS idx_job_runs_job_id_start ON job_runs(job_id, start_time DESC);` at `migrations/sqlite/20260410_000000_initial.up.sql:45` (NOT L46) and at `migrations/postgres/20260410_000000_initial.up.sql:38` | **Minor drift** — CONTEXT.md says L46; actual is L45 (SQLite) / L38 (Postgres). Not load-bearing. |

#### A.1 Exhaustive `DockerExecResult` literal sites (correction)

CONTEXT.md cites "L97-141, 233, 307-313, 553+, 413-417". Actual sites that need `container_id: None` (or `Some(...)` for happy path):

| Line | Site | Action in Plan 16-02 |
|------|------|---------------------|
| L97-104 | Config-parse error early return | `container_id: None` |
| L118-125 | Pre-flight network validation early return | `container_id: None` |
| L135-142 | Image-pull error early return | `container_id: None` |
| L197-205 | Container-create error early return | `container_id: None` |
| L229-236 | Container-start error early return | `container_id: None` |
| L413-416 | Happy-path return after wait/timeout/cancel | `container_id: Some(container_id.clone())` |
| L553-560 | Test fixture in `test_docker_exec_result_debug` | `container_id: Some("test-id".to_string())` (or similar fixture) |

**Note:** CONTEXT.md's "L307-313" reference does NOT correspond to a `DockerExecResult` literal. The lines L307-313 in the current code are inside the `tokio::select!` arm waiting on `wait_container` fallback — no struct literal there. CONTEXT.md likely meant L229-236 (the start-container early-return). Plan 16-02's task list MUST use the seven sites above.

### B. `insert_running_run` caller location for `config_hash` plumbing

**CRITICAL DRIFT.** CONTEXT.md says `src/scheduler/fire.rs (or wherever insert_running_run is invoked)`. Reality:

| Caller | File:Line | Notes |
|--------|-----------|-------|
| Scheduler-driven path | `src/scheduler/run.rs:83` | Inside `run_job(... job: DbJob, ...)` — `DbJob.config_hash: String` is in scope from L47 of queries.rs. Pass `&job.config_hash`. |
| UI Run Now path | `src/web/handlers/api.rs:82` | Inside `run_now` handler. `job: DbJob` is fetched at L66 via `get_job_by_id`; `job.config_hash` is in scope. Pass `&job.config_hash`. |
| Test calls | `src/scheduler/run.rs:794`, `src/db/queries.rs:1833`, L1874, L1923, L1983 | All in `mod tests`. Pass `"testhash"` literal to match existing conventions. |

`fire.rs` does NOT contain `insert_running_run` — it's the BinaryHeap fire-queue module, unrelated. Plan 16-04's task list must be updated to reference `run.rs:83` and `api.rs:82` as the production call sites.

`compute_config_hash` exists at `src/config/hash.rs:16` [VERIFIED]. `JobConfig.config_hash` flows in from `sync_config_to_db` at `src/scheduler/sync.rs:121-138` via `compute_config_hash(job)` and is persisted to `jobs.config_hash` via `upsert_job` [VERIFIED]. By the time `run_job` runs, `DbJob.config_hash` is populated; no recomputation needed.

### C. Three-file migration shape — exact numbering and naming

Migration directory contents [VERIFIED 2026-04-27]:

```
migrations/sqlite/
  20260410_000000_initial.up.sql
  20260416_000001_job_run_number_add.up.sql
  20260417_000002_job_run_number_backfill.up.sql
  20260418_000003_job_run_number_not_null.up.sql
  20260422_000004_enabled_override_add.up.sql
migrations/postgres/  (same five files, parallel content)
```

`_000005`, `_000006`, `_000007` are the next free numbers. Date prefix is the planner's pick (the day Plan 16-01 lands). Today is 2026-04-27. **No `down.sql` convention exists** — no migration in the project has a `down.sql` companion. P16 follows the up-only pattern.

**File naming convention pattern** (from existing files):
- `{YYYYMMDD}_{NNNNNN}_{descriptive_snake_case}.up.sql`
- Examples: `20260416_000001_job_run_number_add`, `20260417_000002_job_run_number_backfill`, `20260418_000003_job_run_number_not_null`, `20260422_000004_enabled_override_add`

P16's three files match this pattern verbatim per CONTEXT.md D-01.

### D. EXPLAIN test precedent — concrete shape

`tests/v13_timeline_explain.rs` exists [VERIFIED] and contains BOTH SQLite (`explain_uses_index_sqlite`) and Postgres (`explain_uses_index_postgres`) tests in one file. Key idioms:

**SQLite setup:** `DbPool::connect("sqlite::memory:")` + `pool.migrate()` + seed >100 runs across 2 jobs. Use `format!("EXPLAIN QUERY PLAN {sql}")`. Collect `r.get::<String, _>("detail")` rows into a string. Substring assert on `idx_job_runs_*`.

**Postgres setup:** Real testcontainer via `Postgres::default().start()`. Migrate. Seed 10,000 rows. **Run `ANALYZE job_runs` AND `ANALYZE jobs`** (mandatory — fresh testcontainer cardinality guess defeats the planner). Use `format!("EXPLAIN (FORMAT JSON) {sql}")`. Walk plan JSON tree for `Node Type` matching `Index Scan`/`Index Only Scan`/`Bitmap Index Scan`/`Bitmap Heap Scan`. Documented fallback: textual presence of `idx_job_runs_*` in plan JSON serialization.

**Both Postgres assertions accepted**:
```rust
assert!(has_index_scan || has_index_ref,
        "expected ... Index Scan / ... Bitmap Index/Heap Scan ... OR reference `idx_*`");
```

This is the proven pattern Plan 16-06 mirrors. Recommendation: single file `tests/v12_fctx_explain.rs` with both `#[tokio::test]` functions matches the v13 precedent. [Note: D-08's "one test file per backend" wording is permissive — planner discretion per CONTEXT.md.]

### E. Schema parity test — extension shape

`tests/schema_parity.rs` is **fully dynamic** [VERIFIED L41-188]. It introspects both backends via `PRAGMA table_info` (SQLite) and `information_schema.columns` (Postgres), normalizes types (`TEXT | VARCHAR | CHARACTER VARYING | CHAR | CHARACTER → "TEXT"` already covered), and diffs the schemas.

**Adding two new TEXT columns requires ZERO test changes.** The new columns will appear in both backends' introspection, the diff will be empty, and the test stays green automatically. CONTEXT.md's "may need updating" can be definitively resolved as "no — verified dynamic introspection."

Plan 16-01's task list should NOT include "update tests/schema_parity.rs". The test is already complete coverage.

### F. SELECT sites that hydrate run rows

Exhaustive enumeration of every site that constructs a `DbRun` or `DbRunDetail`:

| Function | File:Line | Side | Action in Plan 16-04 |
|----------|-----------|------|---------------------|
| `get_run_history` SQLite SELECT | `src/db/queries.rs:1059-1066` | SQL | Extend column list: append `, image_digest, config_hash` |
| `get_run_history` SQLite hydration | `src/db/queries.rs:1070-1082` | Rust | Append `image_digest: r.get("image_digest"), config_hash: r.get("config_hash"),` |
| `get_run_history` Postgres SELECT | `src/db/queries.rs:1093-1100` | SQL | Same column list extension |
| `get_run_history` Postgres hydration | `src/db/queries.rs:1104-1115` | Rust | Same `.get()` calls |
| `get_run_by_id` SQLite SQL literal | `src/db/queries.rs:1125-1131` | SQL | Append `, r.image_digest, r.config_hash` to column list |
| `get_run_by_id` Postgres SQL literal | `src/db/queries.rs:1132-1138` | SQL | Same |
| `get_run_by_id` SQLite hydration | `src/db/queries.rs:1146-1158` | Rust | Append `image_digest: r.get("image_digest"), config_hash: r.get("config_hash"),` |
| `get_run_by_id` Postgres hydration | `src/db/queries.rs:1165-1177` | Rust | Same |

**Total: 4 SELECT statements + 4 hydration blocks.** No `DbRun`/`DbRunDetail` references exist in `src/web/templates/` or any web module other than `api.rs` (which only references the type by name in a doc comment, not a hydration site) [VERIFIED via grep].

`finalize_run` has TWO callers needing signature update:
1. `src/scheduler/run.rs:348-356` — happy path (pass `image_digest_for_finalize.as_deref()`)
2. `src/web/handlers/api.rs:131-140` — error fallback when scheduler channel closed (pass `None`)

### G. Pitfalls / landmines specific to the locked plan

**G.1 — sqlx + ALTER TABLE ADD COLUMN:** The project does NOT use `sqlx::query!` macros [VERIFIED via grep — only one mention at queries.rs:6 explicitly stating "Uses `sqlx::query_as` with runtime SQL strings (not the `query!` macro)"]. No `.sqlx/` cache exists. Adding columns via migration has zero compile-time-checking implications. **No `cargo sqlx prepare` step needed in any plan.** [VERIFIED]

**G.2 — Postgres `image_digest` text type:** No indexed-comparison concerns. `image_digest` is stored as `TEXT` (variable-length, unbounded). It is NOT indexed in P16 (the `idx_job_runs_job_id_start` index covers `job_id, start_time` — image_digest is a payload column). Storing `sha256:...` strings (~71 chars) is well within Postgres TEXT bounds. [VERIFIED]

**G.3 — Postgres backfill UPDATE:** The single bulk UPDATE `UPDATE job_runs SET config_hash = (SELECT j.config_hash FROM jobs j WHERE j.id = job_runs.job_id) WHERE config_hash IS NULL;` has standard Postgres semantics:
- **Locking:** Postgres uses MVCC; the UPDATE acquires row-level write locks per affected row. On a homelab DB <100k rows, this completes in seconds without contention. No table lock.
- **Statement timeout:** Default Postgres `statement_timeout = 0` (no timeout). If an operator has set a per-database timeout, the UPDATE may fail on >1M rows. No P16 mitigation needed; D-02 explicitly defers chunked-loop to v1.3.
- **Long transaction concerns:** sqlx's migration driver wraps each migration file in a transaction; the UPDATE will hold its locks until commit. For 10k–100k rows, well under acceptable.
- No need to split. CONTEXT.md D-02 is correct for the homelab target. [VERIFIED via Postgres documented MVCC behavior + ASSUMED based on training for typical Postgres tuning]

**G.4 — EXPLAIN output stability:**
- **SQLite `EXPLAIN QUERY PLAN`** output has been stable since SQLite 3.7+ (2010); the `detail` column format is documented and substring-matchable. Plan 16-06 SQLite test using `plan_text.contains("idx_job_runs_job_id_start")` is robust across SQLite minor bumps. [ASSUMED A1]
- **Postgres `EXPLAIN (FORMAT JSON)`** schema is stable across Postgres 9.4+. The `Node Type` field has been present since the JSON format was introduced. The textual fallback (`plan_str.contains("idx_*")`) is the documented safety net for any minor format shifts. [ASSUMED A2]

### H. `just` recipe inventory relevant to P16

Existing recipes (CLAUDE.md memory `feedback_uat_use_just_commands` requires UAT to use these):

| Recipe | Purpose | P16 use |
|--------|---------|---------|
| `just build` | `cargo build --all-targets` | Pre-test compile check |
| `just test` | `cargo test --all-features` | Quick test run during development |
| `just nextest` | `cargo nextest run --all-features --profile ci` | CI gate; per-wave merge |
| `just clippy` | Lint with `-D warnings` | CI gate |
| `just fmt-check` | Verify formatting | CI gate |
| `just schema-diff` | `cargo test --test schema_parity -- --nocapture` | Phase gate (verifies new columns parity) |
| `just openssl-check` | Verify rustls-only TLS stack | CI gate (no P16 risk) |
| `just grep-no-percentile-cont` | Guard against SQL-native percentile | CI gate (D-15 — P16's CTE complies) |
| `just deny` | `cargo deny check` | CI gate (non-blocking until P24) |
| `just db-reset` | Delete dev SQLite DB | Local migration testing |
| `just migrate` | Alias for `just dev` (migrations run on daemon startup) | Local migration testing |
| `just check-config PATH` | Validate a config file | Not P16 |
| `just dev` | Single-process dev loop | Local testing |
| `just dev-ui` | Tailwind watch + cargo watch | Not P16 |
| `just sqlx-prepare` | Regenerate `.sqlx/` cache | Not load-bearing in P16 (no `query!` macros) |
| `just ci` | Full ordered chain | Phase gate before `/gsd-verify-work` |

**Missing recipes that may be needed:**
- A bug-fix-specific recipe like `just test-bug-277` is NOT needed — the standard `cargo test --test v12_run_rs_277_bug_fix` is sufficient and matches existing convention.
- A migration-only recipe like `just migrate-sqlite-only` does NOT exist; `just migrate` (alias for `just dev`) runs all migrations on startup. P16 UAT can use `just migrate` if a UAT step is needed.

**For UAT (D-12 / D-13 enforcement):** All UAT steps in any `16-HUMAN-UAT.md` (if produced) MUST reference recipes in the table above. No ad-hoc `cargo`/`docker`/`curl` commands. If a UAT step needs DB inspection (e.g., `SELECT container_id FROM job_runs ORDER BY id DESC LIMIT 1`), the planner must add a new `just` recipe (e.g., `just inspect-last-run`) — flag this if it appears in P16 scope.

### I. Validation Architecture (Nyquist)

See dedicated `## Validation Architecture` section above.

## Sources

### Primary (HIGH confidence)
- `src/scheduler/run.rs` (read 1-915) — verified bug at L301, finalize_run invocation at L348, all DbJob field usage
- `src/scheduler/docker.rs` (read 1-566) — verified DockerExecResult struct, all literal sites, image_digest capture at L240
- `src/db/queries.rs` (read selected sections L100-L267, L368-L530, L552-L594, L1040-L1180, L1818+) — verified all signatures, struct definitions, and SELECT column lists
- `src/web/handlers/api.rs` (read L60-L148) — discovered second `finalize_run` caller at L131
- `src/scheduler/fire.rs` (read L1-60) — confirmed it does NOT contain `insert_running_run`
- `src/config/hash.rs` (read L1-35) — verified `compute_config_hash` exists
- `src/scheduler/sync.rs` (grep) — verified `config_hash` plumbing into `DbJob`
- `migrations/sqlite/` and `migrations/postgres/` (full directory listing + read of initial + 000004 + 000002) — verified migration sequence, naming, no down.sql, idempotency patterns
- `tests/schema_parity.rs` (read 1-294) — verified dynamic-introspection design; no allowlist needed
- `tests/v13_timeline_explain.rs` (read 1-389) — verified EXPLAIN idiom for both backends
- `tests/migrations_idempotent.rs` (read 1-50) — verified existing `pool.migrate().await; pool.migrate().await` idempotency pattern
- `justfile` (read 1-367) — full recipe inventory
- `.planning/research/PITFALLS.md` (read § 44, 45, 46) — verified test ID conventions
- `.planning/PROJECT.md` (full) — verified locked stack
- `CLAUDE.md` (full) — verified project constraints
- `MEMORY.md` (full) — verified mermaid + just + UAT-validation rules

### Secondary (MEDIUM confidence)
- SQLite EXPLAIN QUERY PLAN format stability — based on documented behavior since SQLite 3.7, observed working in `v13_timeline_explain.rs` for ~2 weeks
- Postgres EXPLAIN JSON Node Type schema stability — based on documented Postgres behavior since 9.4, with built-in textual fallback

### Tertiary (LOW confidence)
- None — every cited source was verified in this research session.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; all existing libraries verified at locked versions in PROJECT.md
- Architecture: HIGH — every line number and call-site verified against the live codebase
- Pitfalls: HIGH — pitfalls 1, 2, 3 are codebase-grounded; 4, 5 are CONTEXT.md-grounded; 6, 7 are research-derived
- Section A line-number drift: HIGH — direct read confirmation
- Section B caller correction: HIGH — direct grep confirmation that fire.rs is unrelated
- Section E parity-test resolution: HIGH — read of normalize_type confirmed TEXT coverage
- Section G postgres backfill behavior: MEDIUM — based on documented Postgres MVCC + ASSUMED for typical operator-tuning concerns

**Research date:** 2026-04-27
**Valid until:** 2026-05-04 (1 week — codebase is stable; only risk is a parallel PR shifting line numbers, which CONTEXT.md's structure-references already partially insulate against)
