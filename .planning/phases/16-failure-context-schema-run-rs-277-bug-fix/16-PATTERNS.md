# Phase 16: Failure-Context Schema + run.rs:277 Bug Fix - Pattern Map

**Mapped:** 2026-04-27
**Files analyzed:** 24 (6 NEW migrations, 4 NEW tests, 4 MODIFIED source files, 10 in-file modifications)
**Analogs found:** 24 / 24 (every file has a concrete in-tree analog)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `migrations/sqlite/2026MMDD_000005_image_digest_add.up.sql` | migration (schema-add) | DDL | `migrations/sqlite/20260422_000004_enabled_override_add.up.sql` | exact (single nullable ALTER, nullable forever) |
| `migrations/postgres/2026MMDD_000005_image_digest_add.up.sql` | migration (schema-add, parity) | DDL | `migrations/postgres/20260422_000004_enabled_override_add.up.sql` | exact |
| `migrations/sqlite/2026MMDD_000006_config_hash_add.up.sql` | migration (schema-add) | DDL | `migrations/sqlite/20260422_000004_enabled_override_add.up.sql` | exact |
| `migrations/postgres/2026MMDD_000006_config_hash_add.up.sql` | migration (schema-add, parity) | DDL | `migrations/postgres/20260422_000004_enabled_override_add.up.sql` | exact |
| `migrations/sqlite/2026MMDD_000007_config_hash_backfill.up.sql` | migration (data-backfill) | DML (UPDATE) | `migrations/sqlite/20260417_000002_job_run_number_backfill.up.sql` | partial (file-position match; backfill is plain SQL, not the marker-only Rust orchestrator pattern) |
| `migrations/postgres/2026MMDD_000007_config_hash_backfill.up.sql` | migration (data-backfill, parity) | DML (UPDATE) | `migrations/postgres/20260417_000002_job_run_number_backfill.up.sql` | partial |
| `src/scheduler/docker.rs` (MODIFY) | scheduler / executor result struct | request-response | self (struct extension; in-file precedent for `image_digest` field) | exact |
| `src/scheduler/run.rs` (MODIFY L231 + L301 + L348) | scheduler (lifecycle wiring) | request-response | self (existing `container_id_for_finalize` local + `finalize_run` call) | exact |
| `src/db/queries.rs::insert_running_run` (MODIFY ~L368) | db / write helper | CRUD (insert) | self (existing two-statement counter tx — extend bind list only) | exact |
| `src/db/queries.rs::finalize_run` (MODIFY ~L424) | db / write helper | CRUD (update) | self (existing 6-bind UPDATE — extend to 8 binds) | exact |
| `src/db/queries.rs::DbRun` / `DbRunDetail` (MODIFY) | db / read-side struct | CRUD (read) | `Phase 11 DbRun.job_run_number` field-add precedent (same struct, two PRs ago) | exact |
| `src/db/queries.rs::get_run_history`, `get_run_by_id` (MODIFY) | db / read helper | CRUD (read) | self (existing column-list extensions across both backend arms) | exact |
| `src/db/queries.rs::FailureContext` (NEW struct) | db / read-side struct | CRUD (read) | `src/db/queries.rs::DashboardJob` (L535-550) | exact (read-only result struct, `#[derive(Debug, Clone)]`, lives next to its query helper) |
| `src/db/queries.rs::get_failure_context` (NEW fn) | db / read query helper | CRUD (read) | `src/db/queries.rs::get_run_by_id` (L1124-L1180) | exact (sql_sqlite + sql_postgres locals, `match pool.reader()` arms, same `r.get(...)` hydration shape) |
| `src/web/handlers/api.rs:131-140` (MODIFY) | web handler / error fallback | request-response | self (the call site — pass `None` for `image_digest`) | exact |
| `tests/v12_run_rs_277_bug_fix.rs` (NEW) | integration test (testcontainers + DB) | event-driven | `tests/docker_executor.rs` | exact (real Docker daemon, `#[ignore]` gate, `execute_docker` end-to-end, DB assertion) |
| `tests/v12_fctx_streak.rs` (NEW) | integration test (DB query correctness) | CRUD (read) | `tests/v11_runnum_counter.rs` | exact (in-memory SQLite + `setup_sqlite_with_phase11_migrations()` fixture, `queries::*` direct calls, scenario-based assertions) |
| `tests/v12_fctx_explain.rs` (NEW) | integration test (EXPLAIN, dual backend) | CRUD (read) | `tests/v13_timeline_explain.rs` | exact (single file, `explain_uses_index_sqlite` + `explain_uses_index_postgres` functions, ANALYZE+JSON-walk+textual fallback) |
| `tests/v12_fctx_config_hash_backfill.rs` (NEW) | integration test (migration-effect) | DML | `tests/v11_runnum_migration.rs` | exact (in-memory SQLite, fixture seeds rows BEFORE the backfill migration runs, asserts post-state) |
| `tests/schema_parity.rs` (NO CHANGES) | structural-parity invariant | introspection | self | n/a — RESEARCH §E confirmed dynamic introspection auto-covers two new TEXT columns |

## Pattern Assignments

### Migration files — single nullable ALTER (Plan 16-01, files 005 and 006)

**Target paths:**
- `migrations/sqlite/2026MMDD_000005_image_digest_add.up.sql`
- `migrations/postgres/2026MMDD_000005_image_digest_add.up.sql`
- `migrations/sqlite/2026MMDD_000006_config_hash_add.up.sql`
- `migrations/postgres/2026MMDD_000006_config_hash_add.up.sql`

**Analog:** `migrations/sqlite/20260422_000004_enabled_override_add.up.sql` + paired Postgres file

**SQLite full-file pattern** (`migrations/sqlite/20260422_000004_enabled_override_add.up.sql` lines 1-15):

```sql
-- Phase 14: jobs.enabled_override tri-state column (DB-14, ERG-04).
--
-- Nullable INTEGER: NULL = follow config `enabled` flag (no override);
-- 0 = force disabled (written by POST /api/jobs/bulk-toggle with action=disable);
-- 1 = force enabled (reserved — v1.1 UI never writes this; defensive rendering only).
--
-- Pairs with migrations/postgres/20260422_000004_enabled_override_add.up.sql.
-- Any structural change MUST land in both files in the same PR;
-- tests/schema_parity.rs normalize_type collapses INTEGER + BIGINT to INT64.
--
-- Idempotency: sqlx _sqlx_migrations tracking. No backfill needed —
-- NULL is the correct initial state for every existing row (D-13).

ALTER TABLE jobs ADD COLUMN enabled_override INTEGER;
```

**Postgres parity** (`migrations/postgres/20260422_000004_enabled_override_add.up.sql` lines 1-11):

```sql
-- Phase 14: jobs.enabled_override tri-state column (DB-14, ERG-04).
--
-- Nullable BIGINT: matches SQLite INTEGER under the INT64 normalization rule
-- in tests/schema_parity.rs. NULL = follow config; 0 = force disabled;
-- 1 = force enabled (reserved — v1.1 UI never writes this).
--
-- Pairs with migrations/sqlite/20260422_000004_enabled_override_add.up.sql.
-- Any structural change MUST land in both files in the same PR.

ALTER TABLE jobs ADD COLUMN IF NOT EXISTS enabled_override BIGINT;
```

**What's the same:** Header-comment shape (Phase tag, requirement IDs, nullable rationale, paired-file note, schema_parity invariant note, idempotency note). Single-statement ALTER. Postgres uses `IF NOT EXISTS`, SQLite does not (RESEARCH Pitfall 3). One blank line between header comment and DDL.

**What differs for Phase 16:**
1. Target table is `job_runs`, not `jobs`.
2. Column type is `TEXT` (both backends — RESEARCH Pattern 2 confirms `tests/schema_parity.rs::normalize_type` collapses TEXT-family types to "TEXT").
3. Header cites Phase 16 + REQ-IDs (FOUND-14 for image_digest, FCTX-04 for config_hash) + D-01/D-04 nullable-forever rationale.

---

### Migration file — three-file pattern, file 007 backfill (Plan 16-01)

**Target paths:**
- `migrations/sqlite/2026MMDD_000007_config_hash_backfill.up.sql`
- `migrations/postgres/2026MMDD_000007_config_hash_backfill.up.sql`

**Analog:** `migrations/sqlite/20260417_000002_job_run_number_backfill.up.sql` (file-position match — File 2 of a three-file sequence)

**Analog full file** (lines 1-16):

```sql
-- Phase 11: per-job run numbering (DB-12) — SQLite.
--
-- File 2 of 3: chunked backfill — MARKER ONLY.
-- sqlx::migrate! runs this file's static SELECT only; the actual backfill
-- is orchestrated from Rust by src/db/migrate_backfill.rs which runs AFTER
-- sqlx::migrate!'s first pass and BEFORE its second pass applies file 3
-- (Plan 11-04 adds the second pass in DbPool::migrate).
--
-- Rationale: sqlx::migrate! supports only static SQL; the 10k-row batching
-- loop + per-batch INFO progress log (D-13) lives in Rust. This marker
-- ensures sqlx-tracker records file 2 as applied so re-runs skip it cleanly
-- while the Rust orchestrator's sentinel-table (`_v11_backfill_done`) +
-- `WHERE job_run_number IS NULL` guard provides idempotent partial-crash
-- recovery.

SELECT 1;  -- no-op
```

**What's the same:** File-position labeling ("File N of 3"), per-backend pairing comment, Phase + REQ-ID header, idempotency rationale.

**What differs for Phase 16 (D-02 explicitly REJECTS the marker-only Rust-orchestrator pattern):**
- File contains a real bulk `UPDATE` statement, not `SELECT 1`. No Rust orchestrator.
- Header carries the `BACKFILL_CUTOFF_RFC3339` structured-comment marker (RESEARCH Pitfall 7) so Phase 21's UI can detect backfilled rows.
- Header rationale shifts from "chunked-Rust-marker" to "single bulk UPDATE — homelab DBs <100k rows complete in milliseconds".
- Identical SQL on both backends (per CONTEXT.md D-02): `UPDATE job_runs SET config_hash = (SELECT j.config_hash FROM jobs j WHERE j.id = job_runs.job_id) WHERE config_hash IS NULL;`

---

### `DockerExecResult` field-add (Plan 16-02)

**Target:** `src/scheduler/docker.rs` L62-L68 (struct) + 7 literal sites

**Analog:** Self — the existing `image_digest` field is the in-file precedent. The struct already shows the exact extension pattern.

**Current struct definition** (`src/scheduler/docker.rs` lines 61-68):

```rust
/// Result of a Docker job execution, extending `ExecResult` with container metadata.
#[derive(Debug)]
pub struct DockerExecResult {
    /// Standard execution result (exit code, status, error message).
    pub exec: ExecResult,
    /// Image digest from `inspect_container` after start (DOCKER-09).
    pub image_digest: Option<String>,
}
```

**Authoritative `container_id` capture site** (lines 186-206 — already in scope as a String):

```rust
// Create the container.
let container_id = match docker
    .create_container(create_options, container_body)
    .await
{
    Ok(response) => response.id,
    Err(e) => {
        sender.send(make_log_line(
            "system",
            format!("[docker create error: {e}]"),
        ));
        sender.close();
        return DockerExecResult {
            exec: ExecResult {
                exit_code: None,
                status: RunStatus::Error,
                error_message: Some(format!("failed to create container: {e}")),
            },
            image_digest: None,
        };
    }
};
```

**Happy-path return** (lines 413-416):

```rust
DockerExecResult {
    exec: exec_result,
    image_digest: Some(image_digest),
}
```

**Test fixture pattern** (lines 552-560 — `test_docker_exec_result_debug`):

```rust
#[test]
fn test_docker_exec_result_debug() {
    let result = DockerExecResult {
        exec: ExecResult {
            exit_code: Some(0),
            status: RunStatus::Success,
            error_message: None,
        },
        image_digest: Some("sha256:abc123".to_string()),
    };
```

**Exhaustive literal sites needing `container_id: None` (RESEARCH §A.1 corrected list — CONTEXT.md's "L307-313" reference is wrong):**

| Line | Site | Plan 16-02 action |
|------|------|---------------------|
| L97-104 | Config-parse error early return | Add `container_id: None,` |
| L118-125 | Pre-flight network validation early return | Add `container_id: None,` |
| L135-142 | Image-pull error early return | Add `container_id: None,` |
| L197-205 | Container-create error early return (BEFORE `container_id` is bound — must be `None`) | Add `container_id: None,` |
| L229-236 | Container-start error early return | Add `container_id: Some(container_id.clone())` (id bound at L190) |
| L413-416 | Happy-path return | Add `container_id: Some(container_id.clone())` |
| L552-560 | `test_docker_exec_result_debug` fixture | Add `container_id: Some("test-container-id".to_string())` |

**What's the same:** Field-add follows the exact `image_digest: Option<String>` precedent — same `Option<String>` type, same `pub` visibility, same `///` doc-comment shape.

**What differs:** New field is sourced from `let container_id = response.id` (already a `String` at L186) rather than from `inspect_container`. RESEARCH Pitfall 6 flags an optional 3-line tightening: at L240-251 map `image_digest = String::new()` to `None` to avoid persisting empty strings — planner discretion.

---

### `run.rs` bug fix + parallel local (Plan 16-03)

**Target:** `src/scheduler/run.rs` L231 (declare locals), L301 (the bug), L348-356 (finalize_run call)

**Analog:** Self — the existing `container_id_for_finalize` local is the literal subject of the fix.

**Current state — VERBATIM** (`src/scheduler/run.rs` lines 231 and 288-303):

```rust
    let mut container_id_for_finalize: Option<String> = None;

    let exec_result = match job.job_type.as_str() {
        // ... command and script arms omitted ...
        "docker" => match &docker {
            Some(docker_client) => {
                let docker_result = super::docker::execute_docker(
                    docker_client,
                    &job.config_json,
                    &job.name,
                    run_id,
                    timeout,
                    cancel,
                    sender.clone(),
                    &run_control,
                )
                .await;
                container_id_for_finalize = docker_result.image_digest.clone();  // ← THE BUG (L301)
                docker_result.exec
            }
```

**Current `finalize_run` invocation** (lines 348-357):

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
    {
```

**What changes (Plan 16-03):**
1. **L231 — add a parallel local for image_digest:**
   ```rust
   let mut container_id_for_finalize: Option<String> = None;
   let mut image_digest_for_finalize: Option<String> = None;  // NEW
   ```
2. **L301 — fix the misnamed assignment + add the parallel assignment:**
   ```rust
   container_id_for_finalize = docker_result.container_id.clone();   // FIXED — was .image_digest
   image_digest_for_finalize = docker_result.image_digest.clone();   // NEW
   ```
3. **L348-356 — pass image_digest as the new last positional** (Plan 16-04 owns the signature change; Plan 16-03 adds the argument at the call site):
   ```rust
   finalize_run(
       &pool, run_id, status_str, exec_result.exit_code, start,
       exec_result.error_message.as_deref(),
       container_id_for_finalize.as_deref(),
       image_digest_for_finalize.as_deref(),   // NEW
   )
   ```

**What's the same:** `Option<String>` shape, `.clone()` pattern, `.as_deref()` at the call site — mirror the existing `container_id_for_finalize` local exactly.

**What differs:** The bug-fix is a one-line semantic correction (`.image_digest` → `.container_id`) that depends on Plan 16-02 having added the `container_id` field to `DockerExecResult` first. Plan 16-03 must NOT land before Plan 16-02 in the same PR.

---

### `db/queries.rs::insert_running_run` signature change (Plan 16-04)

**Target:** `src/db/queries.rs` L368-L421

**Analog:** Self — the existing function body is the template. The signature change is mechanical (one new `&str` parameter, one new column in the INSERT, one new `.bind(...)` call).

**Current full function** (lines 368-421):

```rust
pub async fn insert_running_run(pool: &DbPool, job_id: i64, trigger: &str) -> anyhow::Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();

    match pool.writer() {
        PoolRef::Sqlite(p) => {
            let mut tx = p.begin().await?;
            let reserved: i64 = sqlx::query_scalar(
                "UPDATE jobs SET next_run_number = next_run_number + 1 \
                 WHERE id = ?1 RETURNING next_run_number - 1",
            )
            .bind(job_id)
            .fetch_one(&mut *tx)
            .await?;

            let run_id: i64 = sqlx::query_scalar(
                "INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number) \
                 VALUES (?1, 'running', ?2, ?3, ?4) RETURNING id",
            )
            .bind(job_id)
            .bind(trigger)
            .bind(&now)
            .bind(reserved)
            .fetch_one(&mut *tx)
            .await?;

            tx.commit().await?;
            Ok(run_id)
        }
        PoolRef::Postgres(p) => {
            // identical body with $N placeholders
        }
    }
}
```

**What changes (Plan 16-04):**
- Signature: `pub async fn insert_running_run(pool: &DbPool, job_id: i64, trigger: &str, config_hash: &str) -> anyhow::Result<i64>`
- INSERT column list: `(job_id, status, trigger, start_time, job_run_number, config_hash)`
- INSERT VALUES list (SQLite): `(?1, 'running', ?2, ?3, ?4, ?5)` (Postgres uses `$N`)
- New `.bind(config_hash)` after `.bind(reserved)` in BOTH backend arms

**Production callers needing update (RESEARCH §B — `fire.rs` is NOT a caller; CONTEXT.md is wrong on this):**
- `src/scheduler/run.rs:83` — `insert_running_run(&pool, job.id, &trigger).await` → add `&job.config_hash` (RESEARCH §B confirms `DbJob.config_hash: String` is in scope at L47 of queries.rs)
- `src/web/handlers/api.rs:82` — `queries::insert_running_run(&state.pool, job_id, "manual").await` → add `&job.config_hash` (`job` is fetched at L66 via `get_job_by_id`)

**Test callers** (RESEARCH Pitfall 2): `src/scheduler/run.rs:794`, `src/db/queries.rs:1833`, L1874, L1923, L1983 — all in `mod tests`. Pass `"testhash"` literal.

---

### `db/queries.rs::finalize_run` signature change (Plan 16-04)

**Target:** `src/db/queries.rs` L424-L468

**Analog:** Self — the existing function shows the exact 6-bind UPDATE pattern.

**Current full function** (lines 423-468):

```rust
/// Finalize a job run by updating its status, exit_code, end_time, duration_ms, error_message, and container_id.
pub async fn finalize_run(
    pool: &DbPool,
    run_id: i64,
    status: &str,
    exit_code: Option<i32>,
    start_instant: tokio::time::Instant,
    error_message: Option<&str>,
    container_id: Option<&str>,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let duration_ms = start_instant.elapsed().as_millis().min(i64::MAX as u128) as i64;

    match pool.writer() {
        PoolRef::Sqlite(p) => {
            sqlx::query(
                "UPDATE job_runs SET status = ?1, exit_code = ?2, end_time = ?3, duration_ms = ?4, error_message = ?5, container_id = ?6 WHERE id = ?7",
            )
            .bind(status)
            .bind(exit_code)
            .bind(&now)
            .bind(duration_ms)
            .bind(error_message)
            .bind(container_id)
            .bind(run_id)
            .execute(p)
            .await?;
        }
        PoolRef::Postgres(p) => {
            sqlx::query(
                "UPDATE job_runs SET status = $1, exit_code = $2, end_time = $3, duration_ms = $4, error_message = $5, container_id = $6 WHERE id = $7",
            )
            .bind(status)
            .bind(exit_code)
            .bind(&now)
            .bind(duration_ms)
            .bind(error_message)
            .bind(container_id)
            .bind(run_id)
            .execute(p)
            .await?;
        }
    }

    Ok(())
}
```

**What changes:**
- Signature: append `image_digest: Option<&str>` after `container_id`
- SQLite UPDATE statement: `... container_id = ?6, image_digest = ?7 WHERE id = ?8` (placeholder count rises 7 → 8)
- Postgres UPDATE statement: same shape with `$1..$8`
- New `.bind(image_digest)` between `.bind(container_id)` and `.bind(run_id)` in BOTH arms

**Callers needing update (RESEARCH Pitfall 1 — TWO callers, not one):**
1. `src/scheduler/run.rs:348-356` — happy path, pass `image_digest_for_finalize.as_deref()` (Plan 16-03 already prepares this local)
2. `src/web/handlers/api.rs:131-140` — error fallback, pass `None` (the run never started a container)

**Current api.rs:131 fallback site** (lines 130-140):

```rust
let _ = queries::finalize_run(
    &state.pool,
    run_id,
    "error",
    None,
    tokio::time::Instant::now(),
    Some("scheduler shutting down"),
    None,
)
.await;
```

Add a final positional `None` to this call.

---

### `DbRun` / `DbRunDetail` field-add (Plan 16-04)

**Target:** `src/db/queries.rs` L552-L584

**Analog:** Self — the existing `job_run_number: i64` field added in Phase 11 is the precedent. Same struct, same shape, two new optional fields.

**Current `DbRun`** (lines 552-567):

```rust
/// A row from job_runs for the run history view.
#[derive(Debug, Clone)]
pub struct DbRun {
    pub id: i64,
    pub job_id: i64,
    /// Per-job sequential run number (Phase 11 DB-11). Starts at 1, increments
    /// atomically via `insert_running_run`'s counter transaction.
    pub job_run_number: i64,
    pub status: String,
    pub trigger: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub duration_ms: Option<i64>,
    pub exit_code: Option<i32>,
    pub error_message: Option<String>,
}
```

**Current `DbRunDetail`** (lines 569-584):

```rust
/// A row from job_runs with the associated job name (for run detail page).
#[derive(Debug, Clone)]
pub struct DbRunDetail {
    pub id: i64,
    pub job_id: i64,
    /// Per-job sequential run number (Phase 11 DB-11). Mirrors `DbRun::job_run_number`.
    pub job_run_number: i64,
    pub job_name: String,
    pub status: String,
    pub trigger: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub duration_ms: Option<i64>,
    pub exit_code: Option<i32>,
    pub error_message: Option<String>,
}
```

**What changes (both structs):** append two `Option<String>` fields with `///` doc comments referencing Phase 16 + REQ-IDs:

```rust
    pub error_message: Option<String>,
    /// Phase 16 FOUND-14: image digest from post-start `inspect_container`. NULL for
    /// command/script jobs (no image), pre-v1.2 docker rows (capture site landed in v1.2).
    pub image_digest: Option<String>,
    /// Phase 16 FCTX-04: per-run config_hash captured at fire time by
    /// `insert_running_run`. NULL for pre-v1.2 rows whose backfill found no matching
    /// `jobs.config_hash`. See migration `*_000007_config_hash_backfill.up.sql` for
    /// the BACKFILL_CUTOFF_RFC3339 marker (D-03).
    pub config_hash: Option<String>,
```

---

### `get_run_history` and `get_run_by_id` SELECT-list extension (Plan 16-04)

**Target sites (RESEARCH §F — exhaustive enumeration):**

| Function | Lines | Side | Action |
|----------|-------|------|--------|
| `get_run_history` SQLite SELECT | L1059-L1066 | SQL string | append `, image_digest, config_hash` |
| `get_run_history` SQLite hydration | L1068-L1082 | Rust `.map(...)` | append `image_digest: r.get("image_digest"), config_hash: r.get("config_hash"),` |
| `get_run_history` Postgres SELECT | L1093-L1100 | SQL string | same |
| `get_run_history` Postgres hydration | L1102-L1116 | Rust `.map(...)` | same |
| `get_run_by_id` SQLite SQL literal | L1125-L1131 | SQL string | append `, r.image_digest, r.config_hash` |
| `get_run_by_id` Postgres SQL literal | L1132-L1138 | SQL string | same |
| `get_run_by_id` SQLite hydration | L1146-L1158 | Rust `Some(...)` | append two `r.get(...)` calls |
| `get_run_by_id` Postgres hydration | L1165-L1177 | Rust `Some(...)` | same |

**Analog:** Self — the existing SELECT and hydration arms.

**Concrete excerpt — `get_run_history` SQLite arm** (lines 1051-1085):

```rust
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let count_row = sqlx::query("SELECT COUNT(*) as cnt FROM job_runs WHERE job_id = ?1")
                .bind(job_id)
                .fetch_one(p)
                .await?;
            let total: i64 = count_row.get("cnt");

            let rows = sqlx::query(
                "SELECT id, job_id, job_run_number, status, trigger, start_time, end_time, duration_ms, exit_code, error_message FROM job_runs WHERE job_id = ?1 ORDER BY start_time DESC LIMIT ?2 OFFSET ?3",
            )
            .bind(job_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(p)
            .await?;

            let items = rows
                .into_iter()
                .map(|r| DbRun {
                    id: r.get("id"),
                    job_id: r.get("job_id"),
                    job_run_number: r.get("job_run_number"),
                    status: r.get("status"),
                    trigger: r.get("trigger"),
                    start_time: r.get("start_time"),
                    end_time: r.get("end_time"),
                    duration_ms: r.get("duration_ms"),
                    exit_code: r.get("exit_code"),
                    error_message: r.get("error_message"),
                })
                .collect();
```

**`get_run_by_id` SQL+hydration analog** (lines 1124-1158):

```rust
pub async fn get_run_by_id(pool: &DbPool, run_id: i64) -> anyhow::Result<Option<DbRunDetail>> {
    let sql_sqlite = r#"
        SELECT r.id, r.job_id, r.job_run_number, j.name AS job_name, r.status, r.trigger,
               r.start_time, r.end_time, r.duration_ms, r.exit_code, r.error_message
        FROM job_runs r
        JOIN jobs j ON j.id = r.job_id
        WHERE r.id = ?1
    "#;
    let sql_postgres = r#"
        SELECT r.id, r.job_id, r.job_run_number, j.name AS job_name, r.status, r.trigger,
               r.start_time, r.end_time, r.duration_ms, r.exit_code, r.error_message
        FROM job_runs r
        JOIN jobs j ON j.id = r.job_id
        WHERE r.id = $1
    "#;

    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let row = sqlx::query(sql_sqlite)
                .bind(run_id)
                .fetch_optional(p)
                .await?;
            Ok(row.map(|r| DbRunDetail {
                id: r.get("id"),
                job_id: r.get("job_id"),
                job_run_number: r.get("job_run_number"),
                job_name: r.get("job_name"),
                status: r.get("status"),
                trigger: r.get("trigger"),
                start_time: r.get("start_time"),
                end_time: r.get("end_time"),
                duration_ms: r.get("duration_ms"),
                exit_code: r.get("exit_code"),
                error_message: r.get("error_message"),
            }))
        }
```

**What's the same:** raw-string SQL with `?N` (sqlite) / `$N` (postgres) placeholders, single `match pool.reader()` block, identical hydration shape across arms via `.map(|r| Struct { ... })`.

**What differs:** Plan 16-04 appends `, r.image_digest, r.config_hash` to the column lists (note the `r.` prefix in `get_run_by_id` due to the JOIN alias), and appends two `r.get(...)` calls to each hydration block. No new SQL statements; no new functions.

---

### `FailureContext` struct + `get_failure_context` query (Plan 16-05) — NEW

**Target:** Append to `src/db/queries.rs` (D-07 recommends co-location with `DbRun`/`DbRunDetail`).

**Analogs (two — split by concern):**

**Analog 1 — struct shape:** `src/db/queries.rs::DashboardJob` (lines 535-550)

```rust
/// A row for the dashboard view: job + its most recent run.
#[derive(Debug, Clone)]
pub struct DashboardJob {
    pub id: i64,
    pub name: String,
    pub schedule: String,
    pub resolved_schedule: String,
    pub job_type: String,
    pub timeout_secs: i64,
    pub last_status: Option<String>,
    pub last_run_time: Option<String>,
    pub last_trigger: Option<String>,
    /// Phase 14 DB-14 tri-state override (None = config-only; Some(0) = forced disabled;
    /// Some(1) = forced enabled — defensive only). Carried for downstream view rendering
    /// (Plan 05 surfaces this on the dashboard chrome).
    pub enabled_override: Option<i64>,
}
```

**What's the same:** `#[derive(Debug, Clone)]` (RESEARCH "Established Patterns" — convention for query-result structs); `pub` fields; `///` doc comment with Phase + REQ-ID.

**Analog 2 — query function shape:** `get_run_by_id` (lines 1124-1180, full function above).

**What's the same for `get_failure_context`:**
- `pub async fn name(pool: &DbPool, ...) -> anyhow::Result<...>` signature shape
- `let sql_sqlite = r#"..."#;` + `let sql_postgres = r#"..."#;` raw-string locals
- `match pool.reader() { PoolRef::Sqlite(p) => ..., PoolRef::Postgres(p) => ... }` pattern
- `.bind(...)` per parameter; `.fetch_one(p).await?` (D-05 LEFT JOIN ON 1=1 guarantees one row)
- `.get("...")` hydration into the result struct

**What differs:** The SQL is the CTE shape from CONTEXT.md D-05 (verbatim above in CONTEXT.md and RESEARCH lines 396-420). Returns `FailureContext { consecutive_failures, last_success_run_id, last_success_image_digest, last_success_config_hash }` — three of four fields are `Option<...>` (the `LEFT JOIN ON 1=1` returns NULLs when `last_success` is empty). RESEARCH lines 422-433 has the verified Rust-side hydration sketch.

---

### `tests/v12_run_rs_277_bug_fix.rs` (NEW) — testcontainers integration test

**Target:** `tests/v12_run_rs_277_bug_fix.rs`

**Analog:** `tests/docker_executor.rs` (lines 1-115 above)

**Closest existing patterns** (`tests/docker_executor.rs` lines 35-87):

```rust
/// Connect to the local Docker daemon. Panics if unavailable.
async fn docker_client() -> Docker {
    Docker::connect_with_local_defaults()
        .expect("Docker daemon must be running for integration tests")
}

/// Set up an in-memory SQLite database with migrations applied.
async fn setup_test_db() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    pool.migrate().await.unwrap();
    pool
}

#[tokio::test]
#[ignore]
async fn test_docker_basic_echo() {
    let docker = docker_client().await;
    let (sender, receiver) = log_pipeline::channel(256);
    let cancel = CancellationToken::new();
    let control = RunControl::new(cancel.clone());
    // ... collector + execute_docker call ...
    let result = execute_docker(
        &docker,
        config_json,
        "test-echo",
        1,
        Duration::from_secs(30),
        cancel,
        sender,
        &control,
    )
    .await;
```

Note the existing comment on lines 113-115:

```rust
    // Container should be removed (execute_docker cleans up).
    // image_digest field holds the image digest, not the actual container ID,
    // so we verify cleanup indirectly: a second run with the same name should work.
```

**What's the same:** `#[tokio::test]` + `#[ignore]` gate (Docker daemon required), `Docker::connect_with_local_defaults()` helper, in-memory SQLite + `pool.migrate().await`, real `execute_docker` end-to-end invocation.

**What differs (Plan 16-03 must remove the wry comment above):**
- Test fires a real docker run via `run_job` (not raw `execute_docker`) so `finalize_run` runs and writes to `job_runs.container_id`.
- After the run, query `job_runs` and assert `container_id` does NOT start with `sha256:` (the v1.0/v1.1 bug observable).
- Assert `image_digest` IS a `sha256:` value (FOUND-14 second observable, T-V12-FCTX-07).
- For the command-job parity assertion (T-V12-FCTX-08), spawn a second run with `job_type="command"` and assert `image_digest IS NULL`.

---

### `tests/v12_fctx_streak.rs` (NEW) — query-correctness test

**Target:** `tests/v12_fctx_streak.rs`

**Analog:** `tests/v11_runnum_counter.rs` (lines 1-90 above)

**Closest existing patterns:**

```rust
mod common;

use common::v11_fixtures::*;
use cronduit::db::queries::{self, PoolRef};

#[tokio::test]
async fn runnum_starts_at_1() {
    let pool = setup_sqlite_with_phase11_migrations().await;
    let job_id = seed_test_job(&pool, "counter-job").await;
    let r1 = queries::insert_running_run(&pool, job_id, "manual")
        .await
        .unwrap();
    let r2 = queries::insert_running_run(&pool, job_id, "manual")
        .await
        .unwrap();

    let p = match pool.reader() {
        PoolRef::Sqlite(pp) => pp,
        _ => panic!("sqlite-only"),
    };
    let n1: i64 = sqlx::query_scalar("SELECT job_run_number FROM job_runs WHERE id = ?1")
        .bind(r1)
        .fetch_one(p)
        .await
        .unwrap();
    // ... assertions ...
}
```

**What's the same:** `mod common;` + reuse of `tests/common/v11_fixtures.rs::setup_sqlite_with_phase11_migrations()`, in-memory SQLite, multiple `#[tokio::test]` functions covering distinct scenarios, direct `queries::*` calls + raw `sqlx::query_scalar` for verification.

**What differs (Plan 16-05 — five scenarios per D-07):**
- Each test seeds a controlled run sequence with explicit `status` values (`success` / `failed` / `timeout` / `error`) via raw INSERT (mirroring `tests/v13_timeline_explain.rs` lines 83-113 raw-INSERT seeder pattern).
- Calls `queries::get_failure_context(&pool, job_id).await` (NEW function from Plan 16-05).
- Asserts the returned `FailureContext` matches the expected `consecutive_failures` count + `last_success_*` field values.
- Plan 16-05 may add a fixture helper `tests/common/v12_fctx_fixtures.rs` for the seed pattern.

---

### `tests/v12_fctx_explain.rs` (NEW) — EXPLAIN dual-backend

**Target:** `tests/v12_fctx_explain.rs` (single file, both backends — RESEARCH Open Question 2 recommends single file matching v13 precedent)

**Analog:** `tests/v13_timeline_explain.rs` (RESEARCH lines 222-243 + 661-674)

**SQLite idiom** (`tests/v13_timeline_explain.rs` lines 130-149):

```rust
let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
let window_start = "2020-01-01T00:00:00Z";

let rows = sqlx::query(&explain_sql)
    .bind(window_start)
    .fetch_all(pool_ref)
    .await
    .expect("explain query plan");

let plan_text: String = rows
    .iter()
    .map(|r| r.get::<String, _>("detail"))
    .collect::<Vec<_>>()
    .join("\n");

assert!(
    plan_text.contains("idx_job_runs_start_time")
        || plan_text.contains("idx_job_runs_job_id_start"),
    "expected EXPLAIN QUERY PLAN to use an index on job_runs; got:\n{plan_text}"
);
```

**Postgres idiom** (`tests/v13_timeline_explain.rs` lines 224-319):

```rust
// REQUIRED: fresh testcontainer statistics default to guessing cardinality,
// which often picks Seq Scan even when an index exists. ANALYZE forces the
// planner to consult real row counts.
sqlx::query("ANALYZE job_runs").execute(pool_ref).await.expect("analyze");
sqlx::query("ANALYZE jobs").execute(pool_ref).await.expect("analyze jobs");

let explain_sql = format!("EXPLAIN (FORMAT JSON) {pg_sql}");
// ... bind selective_window, fetch_one ...

let plan_json: serde_json::Value = row.get(0);

// Walk the plan tree looking for any node with "Node Type" matching
// "Index Scan" or "Index Only Scan" on the job_runs relation.
fn contains_index_scan(v: &serde_json::Value) -> bool {
    if let Some(node_type) = v.get("Node Type").and_then(|s| s.as_str())
        && (node_type == "Index Scan"
            || node_type == "Index Only Scan"
            || node_type == "Bitmap Index Scan"
            || node_type == "Bitmap Heap Scan")
    {
        return true;
    }
    if let Some(plans) = v.get("Plans").and_then(|p| p.as_array())
        && plans.iter().any(contains_index_scan)
    {
        return true;
    }
    if let Some(plan) = v.get("Plan")
        && contains_index_scan(plan)
    {
        return true;
    }
    if let Some(arr) = v.as_array()
        && arr.iter().any(contains_index_scan)
    {
        return true;
    }
    false
}

let plan_str = plan_json.to_string();
let has_index_scan = contains_index_scan(&plan_json);
let has_index_ref = plan_str.contains("idx_job_runs_start_time")
    || plan_str.contains("idx_job_runs_job_id_start");

assert!(
    has_index_scan || has_index_ref,
    "expected Postgres EXPLAIN JSON to contain an Index Scan / Index Only Scan / \
     Bitmap Index/Heap Scan on job_runs OR reference `idx_job_runs_start_time`; \
     got:\n{plan_json:#}"
);
```

**Postgres seed pattern** (lines 198-234) — 10,000 rows + `ANALYZE` + selective predicate:

```rust
let mut tx = pool_ref.begin().await.expect("begin");
const SEED_ROWS: i64 = 10_000;
let insert_sql = "INSERT INTO job_runs \
    (job_id, job_run_number, status, trigger, start_time, end_time, duration_ms, exit_code) \
    VALUES ($1, $2, 'success', 'scheduled', $3, $4, 60000, 0)";
for n in 1i64..=SEED_ROWS {
    let start = (base + chrono::Duration::minutes(n)).to_rfc3339();
    let end = (base + chrono::Duration::minutes(n) + chrono::Duration::seconds(60)).to_rfc3339();
    sqlx::query(insert_sql).bind(job_id).bind(n).bind(&start).bind(&end)
        .execute(&mut *tx).await.expect("insert job_run");
}
tx.commit().await.expect("commit seed");

sqlx::query("ANALYZE job_runs").execute(pool_ref).await.expect("analyze");
sqlx::query("ANALYZE jobs").execute(pool_ref).await.expect("analyze jobs");
```

**What's the same:** The entire shape — single test file, two `#[tokio::test]` functions, ANALYZE+JSON-walk+textual fallback for Postgres, substring assertion for SQLite, `Postgres::default().start()` testcontainer pattern.

**What differs (Plan 16-06):**
- Target SQL is the CTE-based `get_failure_context` query (D-05 sketch), not the timeline query.
- Both EXPLAIN assertions check for `idx_job_runs_job_id_start` (the only index this CTE should hit; `idx_job_runs_start_time` is irrelevant here — Plan 16-06 should drop the alternation).
- Seed uses status mix (`success`/`failed`/`timeout`/`error`) so both CTE branches (`last_success` LIMIT 1 + `streak` range scan) are exercised by the planner.

---

### `tests/v12_fctx_config_hash_backfill.rs` (NEW) — migration-effect test

**Target:** `tests/v12_fctx_config_hash_backfill.rs`

**Analog:** `tests/v11_runnum_migration.rs` (lines 1-100 above)

**Closest existing patterns:**

```rust
mod common;

use common::v11_fixtures::{
    seed_null_runs, seed_test_job, setup_sqlite_before_file3_migrations,
    setup_sqlite_with_phase11_migrations,
};
use cronduit::db::DbPool;
use cronduit::db::migrate_backfill;
use cronduit::db::queries::{self, PoolRef};

#[tokio::test]
async fn migration_01_add_nullable_columns() {
    // File 1 asserts the **nullable** column shape — so we use the
    // pre-file-3 fixture (applies files 0, 1, 2 but NOT file 3's NOT NULL
    // tightening). ...
    let pool = setup_sqlite_before_file3_migrations().await;
    let sqlite_pool = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };

    let jobs_info: Vec<(String, String, i64, Option<String>)> =
        sqlx::query_as("SELECT name, type, \"notnull\", dflt_value FROM pragma_table_info('jobs')")
            .fetch_all(sqlite_pool)
            .await
            .expect("pragma jobs");
    // ... assertions ...
}
```

Plus the simpler idempotency test in `tests/migrations_idempotent.rs`:

```rust
let pool = DbPool::connect("sqlite::memory:").await.unwrap();
pool.migrate().await.expect("first migrate");
pool.migrate().await.expect("second migrate (idempotent)");

let cols = sqlx::query("PRAGMA table_info('jobs')")
    .fetch_all(read).await.unwrap();
let col_names: Vec<String> = cols.iter().map(|r| r.get::<String, _>("name")).collect();
assert!(col_names.contains(&"config_hash".to_string()), "...");
```

**What's the same:** `mod common;` + fixture imports, in-memory SQLite, `PRAGMA table_info` introspection, scenario-per-`#[tokio::test]`.

**What differs (Plan 16-01):**
- Test runs `pool.migrate()` (which applies all up to and including `_000007_config_hash_backfill`) AFTER seeding `job_runs` rows whose `config_hash` is NULL (insert raw rows BEFORE migrating using a fixture that stops at `_000004` — a new helper may be required, mirroring `setup_sqlite_before_file3_migrations()` shape).
- Asserts: post-migration, every seeded `job_runs` row has `config_hash` matching the corresponding `jobs.config_hash` value.
- Second test (idempotency, T-V12-FCTX-01): re-running the migration is a no-op.
- Third test: a `job_runs` row with no matching `jobs` row (orphan) leaves `config_hash` NULL.

---

### `tests/schema_parity.rs` — NO CHANGES (RESEARCH §E)

**Target:** `tests/schema_parity.rs` (no edits)

**Analog:** Self — the existing `normalize_type` function.

**Relevant excerpt** (`tests/schema_parity.rs` lines 41-62):

```rust
fn normalize_type(raw: &str) -> String {
    let upper = raw.trim().to_ascii_uppercase();
    let base = upper.split('(').next().unwrap_or(&upper).trim();
    match base {
        "INTEGER" | "BIGINT" | "BIGSERIAL" | "INT8" => "INT64".to_string(),
        "SMALLINT" | "INT2" => "INT16".to_string(),
        "INT" | "INT4" => "INT32".to_string(),
        "TEXT" | "VARCHAR" | "CHARACTER VARYING" | "CHAR" | "CHARACTER" => "TEXT".to_string(),
        other => panic!(
            "unknown column type `{other}` — add to normalize_type whitelist with a justification comment"
        ),
    }
}
```

**Conclusion (RESEARCH §E):** Both `image_digest TEXT` (SQLite) and `image_digest TEXT` (Postgres) collapse to `"TEXT"` already. Both `config_hash TEXT` columns do the same. **Zero changes** to `tests/schema_parity.rs`. Plan 16-01's task list MUST NOT include "update schema_parity.rs" — the test is already complete coverage.

---

### `src/web/handlers/api.rs:131-140` (MODIFY) — error fallback

**Target:** `src/web/handlers/api.rs` lines 131-140

**Current state — VERBATIM** (lines 121-140):

```rust
        Err(_) => {
            // Scheduler mpsc receiver closed (shutting down). Finalize the
            // just-inserted row as error so it doesn't linger in 'running'
            // forever. T-11-06-04 mitigation.
            tracing::warn!(
                target: "cronduit.web",
                job_id,
                run_id,
                "run_now: scheduler channel closed — finalizing pre-inserted row as error"
            );
            let _ = queries::finalize_run(
                &state.pool,
                run_id,
                "error",
                None,
                tokio::time::Instant::now(),
                Some("scheduler shutting down"),
                None,
            )
            .await;
```

**What's the same:** Self-analog — the call shape stays identical.

**What differs (Plan 16-04):** Append `None` as the new last positional argument (the run never started a container, so `image_digest` is rightly `None`). RESEARCH Pitfall 1 explicitly flags this caller as the second of two `finalize_run` invocations.

---

## Shared Patterns

### Per-backend SQL with `match pool.{writer,reader}()` arms

**Source:** `src/db/queries.rs` — applied uniformly across all helpers
**Apply to:** Plan 16-04 signature changes (`insert_running_run`, `finalize_run`); Plan 16-05 new helper (`get_failure_context`)

**Excerpt** (`src/db/queries.rs::finalize_run` lines 436-465):

```rust
match pool.writer() {
    PoolRef::Sqlite(p) => {
        sqlx::query(
            "UPDATE job_runs SET status = ?1, ... WHERE id = ?N",
        )
        .bind(status).bind(...).bind(run_id)
        .execute(p).await?;
    }
    PoolRef::Postgres(p) => {
        sqlx::query(
            "UPDATE job_runs SET status = $1, ... WHERE id = $N",
        )
        .bind(status).bind(...).bind(run_id)
        .execute(p).await?;
    }
}
```

**Convention:** SQLite uses `?N` placeholders; Postgres uses `$N`. Body otherwise identical. Read paths use `pool.reader()`; write paths use `pool.writer()`. Plan 16-05's `get_failure_context` is read-only → use `pool.reader()`.

---

### Migration file header convention

**Source:** All 5 existing migrations in `migrations/sqlite/` and `migrations/postgres/`
**Apply to:** All 6 new migration files in Plan 16-01

**Required elements (verbatim from `migrations/sqlite/20260422_000004_enabled_override_add.up.sql`):**

```sql
-- Phase NN: <one-line summary> (REQ-ID-1, REQ-ID-2).
--
-- <Multi-line rationale: column type, nullability, semantics.>
--
-- Pairs with migrations/<other_backend>/<filename>.
-- Any structural change MUST land in both files in the same PR;
-- tests/schema_parity.rs <relevant normalize_type rule>.
--
-- Idempotency: <how this file is safe to re-run>.

<DDL or DML statement>;
```

**For the backfill file (file 007), additionally:**
```sql
-- BACKFILL_CUTOFF_RFC3339: 2026-MM-DDTHH:MM:SSZ
-- (Marker per Phase 16 D-03; Phase 21's UI panel reads this convention to
--  distinguish backfilled rows from authentic per-run captures.)
```
RESEARCH Pitfall 7 mandates RFC3339 UTC format for downstream parser stability.

---

### `#[derive(Debug, Clone)]` on read-side structs

**Source:** `DbJob`, `DbRun`, `DbRunDetail`, `DbLogLine`, `DashboardJob`, `Paginated<T>` — all in `src/db/queries.rs`
**Apply to:** New `FailureContext` struct (Plan 16-05)

**Excerpt:**
```rust
#[derive(Debug, Clone)]
pub struct DashboardJob { ... }

#[derive(Debug, Clone)]
pub struct DbRun { ... }

#[derive(Debug, Clone)]
pub struct DbRunDetail { ... }
```

**Convention:** Every read-side query-result struct in `src/db/queries.rs` carries `#[derive(Debug, Clone)]`. CONTEXT.md D-07 mandates the same for `FailureContext`.

---

### Test file naming `vNN_<feature>_<scenario>.rs`

**Source:** `tests/v11_*.rs`, `tests/v13_*.rs` — established convention
**Apply to:** All four new test files in Phase 16

**Existing examples:** `v11_bulk_toggle.rs`, `v11_runnum_counter.rs`, `v11_runnum_migration.rs`, `v13_timeline_explain.rs`, `v13_sparkline_render.rs`, `v12_webhook_queue_drop.rs`, `v12_webhook_scheduler_unblocked.rs`.

**Phase 16 names (RESEARCH-recommended, planner-discretion per CONTEXT.md):**
- `tests/v12_run_rs_277_bug_fix.rs` (Plan 16-03)
- `tests/v12_fctx_streak.rs` (Plan 16-05)
- `tests/v12_fctx_explain.rs` (single file, both backends — RESEARCH Open Question 2)
- `tests/v12_fctx_config_hash_backfill.rs` (Plan 16-01)

**Note:** `v12_*` prefix is consistent with the two existing v1.2 tests (`v12_webhook_queue_drop.rs`, `v12_webhook_scheduler_unblocked.rs`) deposited by Phase 15.

---

### `just` recipes for UAT (project rule D-12)

**Source:** `justfile` (full inventory)
**Apply to:** Any `16-HUMAN-UAT.md` produced by the planner; never use ad-hoc `cargo`/`docker`/`curl`.

**Recipes available for Phase 16 UAT:**

| Recipe | Use in Phase 16 |
|--------|-----------------|
| `just build` | Pre-test compile check |
| `just test` | `cargo test --all-features` — quick run during development |
| `just nextest` | `cargo nextest run --all-features --profile ci` — per-wave merge gate |
| `just clippy` | Lint with `-D warnings` — CI gate |
| `just fmt-check` | Verify formatting — CI gate |
| `just schema-diff` | `cargo test --test schema_parity -- --nocapture` — phase gate (verifies new columns parity, RESEARCH §E confirms zero changes needed) |
| `just openssl-check` | Verify rustls-only TLS stack (no P16 risk) |
| `just grep-no-percentile-cont` | OBS-05 guard — D-15 (P16's CTE complies) |
| `just deny` | `cargo deny check` (non-blocking until P24) |
| `just db-reset` | Delete dev SQLite DB (local migration testing) |
| `just migrate` | Alias for `just dev` — migrations run on daemon startup (local migration testing) |
| `just ci` | Full ordered chain — phase gate before `/gsd-verify-work` |

**Recipes NOT needed in P16 UAT:**
- `just sqlx-prepare` — RESEARCH §G.1 confirms project does NOT use `sqlx::query!` macros; no `.sqlx/` cache exists.
- `just dev-ui`, `just check-config PATH` — not P16's surface.

---

## No Analog Found

None. Every new file in Phase 16 has a concrete in-tree analog. The only "new" pattern is the **bulk-SQL backfill** in file 007, which is a deliberate departure (D-02) from the v1.1 marker-only Rust-orchestrator pattern; the planner can quote D-02 as the rationale rather than copy from `migrate_backfill.rs`.

## Metadata

**Analog search scope:** `migrations/sqlite/`, `migrations/postgres/`, `src/scheduler/`, `src/db/`, `src/web/handlers/`, `tests/`, `justfile`
**Files scanned:** ~25 (all primary; cross-checked via grep where line numbers were doubted)
**Pattern extraction date:** 2026-04-27
**Source verifications:** RESEARCH.md §A confirms every line number cited above ±1; A.1 corrects CONTEXT.md's "L307-313" reference to the actual seven `DockerExecResult` literal sites

---

## PATTERN MAPPING COMPLETE

**Phase:** 16 — Failure-Context Schema + run.rs:277 Bug Fix
**Files classified:** 24
**Analogs found:** 24 / 24

### Coverage
- Files with exact analog: 22
- Files with role-match (file-position) analog only: 2 (the two `_000007_config_hash_backfill.up.sql` files — analog v1.1 file 002 is structural-position only because D-02 explicitly rejects the marker-only Rust-orchestrator content)
- Files with no analog: 0

### Key Patterns Identified
- All migrations use the "Phase NN + REQ-ID + paired-file + idempotency" header convention; SQLite omits `IF NOT EXISTS` on `ALTER TABLE`, Postgres includes it.
- All `src/db/queries.rs` queries use the `match pool.{writer,reader}() { PoolRef::Sqlite(p) => ..., PoolRef::Postgres(p) => ... }` pattern with `?N` (sqlite) vs `$N` (postgres) placeholders.
- All read-side query-result structs (`DbRun`, `DbRunDetail`, `DashboardJob`, ...) live in `src/db/queries.rs` next to their query helpers and carry `#[derive(Debug, Clone)]`.
- All test files follow the `vNN_<feature>_<scenario>.rs` naming convention; EXPLAIN tests pair both backends in a single file (per `v13_timeline_explain.rs` precedent).
- All UAT steps must reference an existing `just` recipe per project rule D-12; `just schema-diff`, `just nextest`, `just grep-no-percentile-cont`, `just ci` cover Phase 16's gates.
- `tests/schema_parity.rs` is dynamic-introspection — adding two new TEXT columns requires zero test edits (RESEARCH §E confirmed).

### File Created
`/Users/Robert/Code/public/cronduit/.planning/phases/16-failure-context-schema-run-rs-277-bug-fix/16-PATTERNS.md`

### Ready for Planning
Pattern mapping complete. The planner can now reference exact analog line ranges and code excerpts in PLAN.md `<read_first>` and `<action>` blocks for Plans 16-01 through 16-06.
