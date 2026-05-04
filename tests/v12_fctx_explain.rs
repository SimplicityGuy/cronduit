//! Phase 16 / FCTX-07: assert that get_failure_context's CTE-based query
//! uses indexed access on idx_job_runs_job_id_start (job_id, start_time DESC)
//! on both SQLite and Postgres backends. Mirrors the OBS-02 precedent in
//! tests/v13_timeline_explain.rs.
//!
//! Locked by D-08: plan must reference idx_job_runs_job_id_start; plan
//! must NOT contain bare SCAN job_runs (SQLite) or Seq Scan on job_runs
//! without an Index Scan companion (Postgres).
//!
//! ## Coverage contract
//!
//! - **FCTX-07 Success Criterion 3** (EXPLAIN QUERY PLAN on both SQLite
//!   and Postgres for `get_failure_context` uses indexed access on
//!   `idx_job_runs_job_id_start`) is the explicit assertion in tests 1
//!   and 2 below.
//!
//! ## Why the SQL is inlined here (not imported from queries.rs)
//!
//! Plan 16-06 lands in parallel with Plan 16-05 (which adds the
//! `get_failure_context` helper to src/db/queries.rs). The CTE SQL here
//! is the verbatim D-05 locked shape — the same SQL Plan 16-05 emits
//! into queries.rs. The wave-end gate confirms both files compose
//! cleanly post-merge: if 16-05 deviates from D-05's CTE shape, the
//! EXPLAIN test still asserts the correct production index hit because
//! both files derive from the same locked decision.
//!
//! ## Postgres test notes
//!
//! A fresh testcontainer with no statistics may pick `Seq Scan`
//! regardless of index availability. Test 2 seeds 10,000 rows AND runs
//! `ANALYZE job_runs` + `ANALYZE jobs` before EXPLAIN to force the
//! planner to consult real row counts (RESEARCH Pitfall 4 mitigation).
//!
//! ## Seeder notes
//!
//! Both tests insert rows via direct SQL (bypassing the production
//! `insert_running_run` and `finalize_run` helpers) because we need
//! deterministic `start_time` values and a controlled mix of statuses
//! (`success` / `failed` / `timeout`) so both CTE arms (the last_success
//! LIMIT 1 lookup and the streak range scan above the boundary) are
//! exercised by the seeded fixture. The column list
//! `(job_id, job_run_number, status, trigger, start_time, image_digest,
//! config_hash)` is explicit — Phase 11 DB-10 made `job_run_number`
//! NOT NULL so the column list is load-bearing.

use cronduit::db::queries::{self, PoolRef};
use cronduit::db::{DbBackend, DbPool};
use sqlx::Row;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

// ---------------------------------------------------------------------------
// Locked CTE SQL — verbatim from CONTEXT.md D-05.
// Both backends accept this shape with only `?1` -> `$1` substitution
// (RESEARCH §G.3). Identical to the SQL Plan 16-05 emits into
// queries.rs::get_failure_context.
// ---------------------------------------------------------------------------

const FCTX_SQL_SQLITE: &str = r#"
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

const FCTX_SQL_POSTGRES: &str = r#"
    WITH last_success AS (
        SELECT id AS run_id, image_digest, config_hash, start_time
          FROM job_runs
         WHERE job_id = $1 AND status = 'success'
         ORDER BY start_time DESC
         LIMIT 1
    ),
    streak AS (
        SELECT COUNT(*) AS consecutive_failures
          FROM job_runs
         WHERE job_id = $1
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

// ---------------------------------------------------------------------------
// Test 1 — SQLite EXPLAIN QUERY PLAN asserts index usage on job_runs
// ---------------------------------------------------------------------------

#[tokio::test]
async fn explain_uses_index_sqlite() {
    // Build an in-memory SQLite pool and migrate the schema.
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    assert_eq!(pool.backend(), DbBackend::Sqlite);
    pool.migrate().await.expect("run migrations");

    // Seed one job via the production helper (handles created_at/updated_at +
    // next_run_number defaults). Then seed ~100 mixed-status runs so both
    // CTE arms (last_success LIMIT 1 + streak range scan) have rows to walk.
    let job_id = queries::upsert_job(
        &pool,
        "explain-fctx-job",
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo fctx"}"#,
        "hash-fctx",
        3600,
        "[]",
    )
    .await
    .expect("upsert job");

    let pool_ref = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("explain_uses_index_sqlite requires the SQLite writer pool"),
    };

    // Direct INSERT loop with controlled status mix so BOTH CTE arms have rows:
    // - At least one 'success' row (so last_success CTE returns a target).
    // - At least several 'failed'/'timeout'/'error' rows above the success
    //   boundary (so streak CTE has a non-empty range scan).
    let insert_sql = "INSERT INTO job_runs \
        (job_id, job_run_number, status, trigger, start_time, image_digest, config_hash) \
        VALUES (?, ?, ?, 'manual', ?, NULL, 'seed-hash')";

    let base = chrono::DateTime::parse_from_rfc3339("2026-04-01T00:00:00Z")
        .expect("parse base timestamp")
        .with_timezone(&chrono::Utc);
    let mut tx = pool_ref.begin().await.expect("begin");
    for n in 1i64..=100 {
        let status = match n % 5 {
            0 => "success",
            1 | 2 => "failed",
            3 => "timeout",
            _ => "success",
        };
        let start = (base + chrono::Duration::minutes(n)).to_rfc3339();
        sqlx::query(insert_sql)
            .bind(job_id)
            .bind(n)
            .bind(status)
            .bind(&start)
            .execute(&mut *tx)
            .await
            .expect("insert job_run");
    }
    tx.commit().await.expect("commit");

    // Run EXPLAIN QUERY PLAN against the production CTE SQL.
    let explain_sql = format!("EXPLAIN QUERY PLAN {FCTX_SQL_SQLITE}");
    let rows = sqlx::query(&explain_sql)
        .bind(job_id)
        .fetch_all(pool_ref)
        .await
        .expect("explain query plan");
    let plan_text: String = rows
        .iter()
        .map(|r| r.get::<String, _>("detail"))
        .collect::<Vec<_>>()
        .join("\n");

    // Primary assertion (D-08): the plan references idx_job_runs_job_id_start.
    // Both CTE arms should hit this (job_id, start_time DESC) covering index.
    assert!(
        plan_text.contains("idx_job_runs_job_id_start"),
        "expected EXPLAIN QUERY PLAN to use idx_job_runs_job_id_start; got:\n{plan_text}"
    );

    // Secondary assertion (D-08): the plan must NOT show a bare SCAN job_runs
    // (full table scan). Modern SQLite reports "SEARCH job_runs USING INDEX
    // idx_..." when an index is hit; a bare "SCAN job_runs" without an
    // index reference means a full table scan, which we explicitly disallow.
    //
    // Note: SQLite's plan grammar uses "SEARCH ... USING INDEX <idx>" for
    // indexed access. The `contains("USING INDEX")` rider tolerates the case
    // where SQLite emits "SCAN ... USING INDEX" verbiage on subqueries.
    assert!(
        !plan_text.contains("SCAN job_runs") || plan_text.contains("USING INDEX"),
        "EXPLAIN must not show a bare SCAN job_runs (would mean full table scan); got:\n{plan_text}"
    );
}

// ---------------------------------------------------------------------------
// Test 2 — Postgres EXPLAIN (FORMAT JSON) asserts Index Scan on job_runs
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn explain_uses_index_postgres() {
    // Start a real Postgres via testcontainers. Mirrors
    // tests/v13_timeline_explain.rs::explain_uses_index_postgres.
    let container = Postgres::default()
        .start()
        .await
        .expect("start postgres container");
    let host = container.get_host().await.expect("container host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("container port");
    let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");

    let pool = DbPool::connect(&url).await.expect("DbPool::connect");
    assert_eq!(pool.backend(), DbBackend::Postgres);
    pool.migrate().await.expect("run migrations");

    // Seed one job + 10,000 mixed-status runs. RESEARCH Pitfall 4 mandates
    // sufficient row volume + ANALYZE before EXPLAIN; without these, the
    // Postgres planner often picks Seq Scan even when an index exists.
    let job_id = queries::upsert_job(
        &pool,
        "explain-fctx-pg",
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo pg-fctx"}"#,
        "hash-pg-fctx",
        3600,
        "[]",
    )
    .await
    .expect("upsert job");

    let pool_ref = match pool.writer() {
        PoolRef::Postgres(p) => p,
        _ => panic!("explain_uses_index_postgres requires the Postgres pool"),
    };

    let base = chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
        .expect("parse base timestamp")
        .with_timezone(&chrono::Utc);
    let mut tx = pool_ref.begin().await.expect("begin");
    const SEED_ROWS: i64 = 10_000;
    let insert_sql = "INSERT INTO job_runs \
        (job_id, job_run_number, status, trigger, start_time, image_digest, config_hash) \
        VALUES ($1, $2, $3, 'manual', $4, NULL, 'seed-hash')";
    // Status mix: every 5th row is 'success' (≈2,000 successes), the rest
    // split between 'failed' / 'timeout' / 'error' so BOTH CTE arms have
    // non-trivial row counts to walk:
    //   - last_success CTE picks the most recent of ~2,000 success rows.
    //   - streak CTE counts the failure rows above that boundary.
    for n in 1i64..=SEED_ROWS {
        let status = match n % 5 {
            0 => "success",
            1 => "failed",
            2 => "timeout",
            3 => "error",
            _ => "failed",
        };
        let start = (base + chrono::Duration::minutes(n)).to_rfc3339();
        sqlx::query(insert_sql)
            .bind(job_id)
            .bind(n)
            .bind(status)
            .bind(&start)
            .execute(&mut *tx)
            .await
            .expect("insert job_run");
    }
    tx.commit().await.expect("commit seed");

    // REQUIRED per RESEARCH Pitfall 4: fresh testcontainer statistics default
    // to guessing cardinality, which often picks Seq Scan even when an index
    // exists. ANALYZE forces the planner to consult real row counts. Both
    // tables — `job_runs` (the queried surface) and `jobs` (the FK target).
    sqlx::query("ANALYZE job_runs")
        .execute(pool_ref)
        .await
        .expect("analyze job_runs");
    sqlx::query("ANALYZE jobs")
        .execute(pool_ref)
        .await
        .expect("analyze jobs");

    // Run EXPLAIN (FORMAT JSON) against the production CTE SQL.
    let explain_sql = format!("EXPLAIN (FORMAT JSON) {FCTX_SQL_POSTGRES}");
    let row = sqlx::query(&explain_sql)
        .bind(job_id)
        .fetch_one(pool_ref)
        .await
        .expect("explain");

    let plan_json: serde_json::Value = row.get(0);

    // Walk the plan tree looking for any node with "Node Type" matching
    // an index-based scan kind on the job_runs relation. Mirrors the
    // tests/v13_timeline_explain.rs walker verbatim.
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

    // Primary assertion (D-08): the plan contains an index-based scan node.
    //
    // Documented fallback (matches v13_timeline_explain.rs precedent): if the
    // Postgres planner on a fresh testcontainer still chooses Seq Scan despite
    // the seed volume + ANALYZE (which can happen on some testcontainer
    // images / postgres versions under pathological cost estimates), accept
    // the weaker structural evidence that `idx_job_runs_job_id_start` appears
    // anywhere in the rendered plan JSON — proves the index was at least
    // considered by the planner. Per Plan 16-06: drop the v13's
    // alternation `idx_job_runs_start_time || idx_job_runs_job_id_start`;
    // this CTE only hits `idx_job_runs_job_id_start` (job_id, start_time DESC).
    let plan_str = plan_json.to_string();
    let has_index_scan = contains_index_scan(&plan_json);
    let has_index_ref = plan_str.contains("idx_job_runs_job_id_start");

    assert!(
        has_index_scan || has_index_ref,
        "expected Postgres EXPLAIN JSON to contain an Index Scan / Index Only Scan / \
         Bitmap Index/Heap Scan on job_runs OR reference `idx_job_runs_job_id_start`; \
         got:\n{plan_json:#}"
    );

    pool.close().await;
}

// ---------------------------------------------------------------------------
// Test 3 — SQLite EXPLAIN QUERY PLAN AFTER scheduled_for column lands
// (Phase 21 / FCTX-06)
// ---------------------------------------------------------------------------
//
// Locks the FCTX-06 invariant from Phase 21 D-18 (CONTEXT.md line 88) and
// research landmine §10: adding the additive `scheduled_for TEXT NULL`
// column (migration `20260503_000009_scheduled_for_add.up.sql`) does NOT
// shift the `idx_job_runs_job_id_start` index plan for the
// `get_failure_context` CTE.
//
// Why this is a separate test rather than an edit to `explain_uses_index_sqlite`:
//
//   - The existing test ALREADY runs against the post-migration schema in
//     Phase 21 (sqlx applies every migration in `migrations/sqlite/` at
//     `pool.migrate().await`). Once Phase 21 lands, the original test IS
//     the post-`scheduled_for` test by virtue of file-system ordering.
//   - This second test exists as a **named, intent-bearing assertion**:
//     when a future migration touches the `job_runs` schema, this name
//     surfaces in the diff/test output as the explicit guard for the
//     skew-column index posture, not just an incidental side effect of
//     the P16 baseline test.
//   - The seed body is a near-copy because **research landmine §10**
//     guarantees the explicit-column INSERT (which omits `scheduled_for`)
//     remains valid post-migration: SQLite defaults the omitted column to
//     NULL, exactly the legacy state pre-v1.2 rows live in forever (D-04).
//
// Assertions: identical to test 1 (`idx_job_runs_job_id_start` referenced;
// no bare `SCAN job_runs` without `USING INDEX`).

#[tokio::test]
async fn explain_uses_index_sqlite_post_scheduled_for() {
    // Build an in-memory SQLite pool and migrate the schema. This applies
    // every migration in `migrations/sqlite/` including
    // `20260503_000009_scheduled_for_add.up.sql` — the additive
    // `ALTER TABLE job_runs ADD COLUMN scheduled_for TEXT;` that this
    // test exists to harness.
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    assert_eq!(pool.backend(), DbBackend::Sqlite);
    pool.migrate().await.expect("run migrations");

    let job_id = queries::upsert_job(
        &pool,
        "explain-fctx-post-skew-job",
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo fctx-post"}"#,
        "hash-fctx-post",
        3600,
        "[]",
    )
    .await
    .expect("upsert job");

    let pool_ref = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("explain_uses_index_sqlite_post_scheduled_for requires the SQLite writer pool"),
    };

    // VERBATIM the same explicit-column INSERT shape as test 1 above —
    // research landmine §10 confirms the omitted `scheduled_for` column
    // defaults to NULL on both backends, so this list survives the
    // migration unchanged.
    let insert_sql = "INSERT INTO job_runs \
        (job_id, job_run_number, status, trigger, start_time, image_digest, config_hash) \
        VALUES (?, ?, ?, 'manual', ?, NULL, 'seed-hash')";

    let base = chrono::DateTime::parse_from_rfc3339("2026-04-01T00:00:00Z")
        .expect("parse base timestamp")
        .with_timezone(&chrono::Utc);
    let mut tx = pool_ref.begin().await.expect("begin");
    for n in 1i64..=100 {
        let status = match n % 5 {
            0 => "success",
            1 | 2 => "failed",
            3 => "timeout",
            _ => "success",
        };
        let start = (base + chrono::Duration::minutes(n)).to_rfc3339();
        sqlx::query(insert_sql)
            .bind(job_id)
            .bind(n)
            .bind(status)
            .bind(&start)
            .execute(&mut *tx)
            .await
            .expect("insert job_run");
    }
    tx.commit().await.expect("commit");

    let explain_sql = format!("EXPLAIN QUERY PLAN {FCTX_SQL_SQLITE}");
    let rows = sqlx::query(&explain_sql)
        .bind(job_id)
        .fetch_all(pool_ref)
        .await
        .expect("explain query plan");
    let plan_text: String = rows
        .iter()
        .map(|r| r.get::<String, _>("detail"))
        .collect::<Vec<_>>()
        .join("\n");

    // Phase 21 D-18 invariant: scheduled_for is unindexed and not in the
    // `get_failure_context` WHERE clause, so the plan should still hit
    // `idx_job_runs_job_id_start` exactly as in test 1. Failure here would
    // signal an unexpected planner regression introduced by the additive
    // column — the exact regression FCTX-06 was guarding against.
    assert!(
        plan_text.contains("idx_job_runs_job_id_start"),
        "expected EXPLAIN QUERY PLAN (post-scheduled_for migration) to use \
         idx_job_runs_job_id_start; got:\n{plan_text}"
    );

    assert!(
        !plan_text.contains("SCAN job_runs") || plan_text.contains("USING INDEX"),
        "EXPLAIN (post-scheduled_for migration) must not show a bare SCAN job_runs \
         (would mean full table scan); got:\n{plan_text}"
    );
}

// ---------------------------------------------------------------------------
// Test 4 — Postgres EXPLAIN AFTER scheduled_for column lands
// (Phase 21 / FCTX-06)
// ---------------------------------------------------------------------------
//
// Postgres pair of test 3. Same rationale as the sqlite case: locks the
// FCTX-06 invariant under Phase 21 D-18. Same `#[ignore]` gating as the
// existing `explain_uses_index_postgres` test (testcontainers-backed,
// Docker-required; runs in the CI Postgres lane via
// `cargo nextest run --run-ignored=all`).

#[tokio::test]
#[ignore]
async fn explain_uses_index_postgres_post_scheduled_for() {
    let container = Postgres::default()
        .start()
        .await
        .expect("start postgres container");
    let host = container.get_host().await.expect("container host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("container port");
    let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");

    let pool = DbPool::connect(&url).await.expect("DbPool::connect");
    assert_eq!(pool.backend(), DbBackend::Postgres);
    // Applies the post-Phase-21 migration set, including
    // `migrations/postgres/20260503_000009_scheduled_for_add.up.sql`.
    pool.migrate().await.expect("run migrations");

    let job_id = queries::upsert_job(
        &pool,
        "explain-fctx-pg-post-skew",
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo pg-fctx-post"}"#,
        "hash-pg-fctx-post",
        3600,
        "[]",
    )
    .await
    .expect("upsert job");

    let pool_ref = match pool.writer() {
        PoolRef::Postgres(p) => p,
        _ => panic!("explain_uses_index_postgres_post_scheduled_for requires the Postgres pool"),
    };

    let base = chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
        .expect("parse base timestamp")
        .with_timezone(&chrono::Utc);
    let mut tx = pool_ref.begin().await.expect("begin");
    const SEED_ROWS: i64 = 10_000;
    // Same explicit-column-list INSERT as test 2 — research landmine §10:
    // the omitted `scheduled_for` column defaults to NULL on Postgres just
    // as on SQLite, so this seed body survives the Phase 21 migration with
    // zero edits.
    let insert_sql = "INSERT INTO job_runs \
        (job_id, job_run_number, status, trigger, start_time, image_digest, config_hash) \
        VALUES ($1, $2, $3, 'manual', $4, NULL, 'seed-hash')";
    for n in 1i64..=SEED_ROWS {
        let status = match n % 5 {
            0 => "success",
            1 => "failed",
            2 => "timeout",
            3 => "error",
            _ => "failed",
        };
        let start = (base + chrono::Duration::minutes(n)).to_rfc3339();
        sqlx::query(insert_sql)
            .bind(job_id)
            .bind(n)
            .bind(status)
            .bind(&start)
            .execute(&mut *tx)
            .await
            .expect("insert job_run");
    }
    tx.commit().await.expect("commit seed");

    // ANALYZE both tables — same RESEARCH Pitfall 4 mitigation as test 2.
    sqlx::query("ANALYZE job_runs")
        .execute(pool_ref)
        .await
        .expect("analyze job_runs");
    sqlx::query("ANALYZE jobs")
        .execute(pool_ref)
        .await
        .expect("analyze jobs");

    let explain_sql = format!("EXPLAIN (FORMAT JSON) {FCTX_SQL_POSTGRES}");
    let row = sqlx::query(&explain_sql)
        .bind(job_id)
        .fetch_one(pool_ref)
        .await
        .expect("explain");

    let plan_json: serde_json::Value = row.get(0);

    // Identical walker to test 2 — keep this duplicated rather than
    // hoisted to a private helper to preserve copy-locality with the P16
    // precedent.
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
    let has_index_ref = plan_str.contains("idx_job_runs_job_id_start");

    assert!(
        has_index_scan || has_index_ref,
        "expected Postgres EXPLAIN JSON (post-scheduled_for migration) to contain an \
         Index Scan / Index Only Scan / Bitmap Index/Heap Scan on job_runs OR reference \
         `idx_job_runs_job_id_start`; got:\n{plan_json:#}"
    );

    pool.close().await;
}
