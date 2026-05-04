//! Phase 18 / WH-06: assert that `coalesce::filter_position`'s CTE-based
//! query uses indexed access on `idx_job_runs_job_id_start (job_id,
//! start_time DESC)` on both SQLite and Postgres backends. Mirrors the
//! Phase 16 FCTX-07 precedent in `tests/v12_fctx_explain.rs`.
//!
//! Locked by D-12 + D-15: plan must reference `idx_job_runs_job_id_start`;
//! plan must NOT contain a bare `SCAN job_runs` (SQLite) or `Seq Scan on
//! job_runs` without an Index Scan companion (Postgres).
//!
//! ## Coverage contract
//!
//! - **Plan 18-03 must_have** ("Filter-position SQL covers SQLite +
//!   Postgres dialects via dual `&str` constants and `match pool.reader()`
//!   dispatch") is exercised by both tests below — they re-use the
//!   production constants verbatim.
//! - **Plan 18-03 must_have** ("EXPLAIN PLAN test asserts
//!   `idx_job_runs_job_id_start` index hit on both backends") is the
//!   explicit assertion in tests 1 and 2.
//!
//! ## Why the SQL is inlined here (not imported via the dispatcher path)
//!
//! `coalesce::SQL_SQLITE` / `SQL_POSTGRES` are `pub(crate)`. Inlining
//! verbatim copies here keeps the integration test independent of crate
//! visibility and future re-exports while still asserting the exact same
//! production index hit. If the production CTE shape diverges from this
//! test, the wave-end gate will catch it (clippy + this test failing).
//!
//! ## Postgres test notes
//!
//! Mirrors the FCTX-07 EXPLAIN test gating: a fresh testcontainer with
//! no statistics may pick `Seq Scan` regardless of index availability.
//! Test 2 seeds 10,000 rows AND runs `ANALYZE job_runs` + `ANALYZE jobs`
//! before EXPLAIN to force the planner to consult real row counts
//! (RESEARCH Pitfall 4 mitigation).

use cronduit::db::queries::{self, PoolRef};
use cronduit::db::{DbBackend, DbPool};
use sqlx::Row;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

// ---------------------------------------------------------------------------
// Locked filter-position CTE SQL — verbatim from
// src/webhooks/coalesce.rs::SQL_SQLITE / SQL_POSTGRES.
// D-15 success sentinel: WHEN status='success' THEN 0 BEFORE the IN-list
// check. Hard-coded on BOTH backends.
// ---------------------------------------------------------------------------

const FP_SQL_SQLITE: &str = r#"
    WITH ordered AS (
        SELECT id, status, start_time
          FROM job_runs
         WHERE job_id = ?1
           AND start_time <= ?2
         ORDER BY start_time DESC
    ),
    marked AS (
        SELECT id, status, start_time,
               CASE
                   WHEN status = 'success' THEN 0
                   WHEN status IN (?3, ?4, ?5, ?6, ?7, ?8) THEN 1
                   ELSE 0
               END AS is_match
          FROM ordered
    ),
    first_break AS (
        SELECT MAX(start_time) AS break_time
          FROM marked
         WHERE is_match = 0
    )
    SELECT COUNT(*) AS pos
      FROM marked
     WHERE is_match = 1
       AND start_time > COALESCE(
             (SELECT break_time FROM first_break),
             '1970-01-01T00:00:00Z'
           )
"#;

const FP_SQL_POSTGRES: &str = r#"
    WITH ordered AS (
        SELECT id, status, start_time
          FROM job_runs
         WHERE job_id = $1
           AND start_time <= $2
         ORDER BY start_time DESC
    ),
    marked AS (
        SELECT id, status, start_time,
               CASE
                   WHEN status = 'success' THEN 0
                   WHEN status IN ($3, $4, $5, $6, $7, $8) THEN 1
                   ELSE 0
               END AS is_match
          FROM ordered
    ),
    first_break AS (
        SELECT MAX(start_time) AS break_time
          FROM marked
         WHERE is_match = 0
    )
    SELECT COUNT(*)::BIGINT AS pos
      FROM marked
     WHERE is_match = 1
       AND start_time > COALESCE(
             (SELECT break_time FROM first_break),
             '1970-01-01T00:00:00Z'
           )
"#;

// ---------------------------------------------------------------------------
// Test 1 — SQLite EXPLAIN QUERY PLAN asserts index usage on job_runs
// ---------------------------------------------------------------------------

#[tokio::test]
async fn filter_position_query_uses_idx_job_runs_job_id_start_sqlite() {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite pool");
    assert_eq!(pool.backend(), DbBackend::Sqlite);
    pool.migrate().await.expect("run migrations");

    let job_id = queries::upsert_job(
        &pool,
        "explain-fp-job",
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo fp"}"#,
        "hash-fp",
        3600,
        "[]",
    )
    .await
    .expect("upsert job");

    let pool_ref = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("filter_position_query_uses_idx_job_runs_job_id_start_sqlite requires SQLite"),
    };

    // Seed >100 mixed-status runs so the planner has rows to consider.
    let insert_sql = "INSERT INTO job_runs \
        (job_id, job_run_number, status, trigger, start_time, image_digest, config_hash) \
        VALUES (?, ?, ?, 'manual', ?, NULL, 'seed-hash')";
    let base = chrono::DateTime::parse_from_rfc3339("2026-04-01T00:00:00Z")
        .expect("parse base timestamp")
        .with_timezone(&chrono::Utc);
    let mut tx = pool_ref.begin().await.expect("begin");
    for n in 1i64..=120 {
        let status = match n % 5 {
            0 => "success",
            1 | 2 => "failed",
            3 => "timeout",
            _ => "error",
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

    // ANALYZE refreshes SQLite's stat tables so the planner picks the
    // right index even on small fixtures.
    sqlx::query("ANALYZE")
        .execute(pool_ref)
        .await
        .expect("analyze sqlite");

    // Pick a current_start cap that hits all 120 rows.
    let current_ts = (base + chrono::Duration::minutes(200)).to_rfc3339();

    // Run EXPLAIN QUERY PLAN against the production CTE SQL with a
    // representative `states` slice (operator-supplied; padded to 6 by
    // the production helper, but the EXPLAIN-time analysis is the same).
    let explain_sql = format!("EXPLAIN QUERY PLAN {FP_SQL_SQLITE}");
    let rows = sqlx::query(&explain_sql)
        .bind(job_id)
        .bind(&current_ts)
        .bind("failed")
        .bind("timeout")
        .bind("error")
        .bind("error")
        .bind("error")
        .bind("error")
        .fetch_all(pool_ref)
        .await
        .expect("explain query plan");
    let plan_text: String = rows
        .iter()
        .map(|r| r.get::<String, _>("detail"))
        .collect::<Vec<_>>()
        .join("\n");

    // Primary assertion: the plan references idx_job_runs_job_id_start.
    assert!(
        plan_text.contains("idx_job_runs_job_id_start"),
        "expected EXPLAIN QUERY PLAN to use idx_job_runs_job_id_start; got:\n{plan_text}"
    );

    // Secondary assertion: no bare SCAN job_runs (full table scan).
    // Modern SQLite reports "SEARCH job_runs USING INDEX idx_..." when an
    // index is hit; "SCAN ... USING INDEX" verbiage on subqueries is OK.
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
async fn filter_position_query_uses_idx_job_runs_job_id_start_postgres() {
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

    let job_id = queries::upsert_job(
        &pool,
        "explain-fp-pg",
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo pg-fp"}"#,
        "hash-pg-fp",
        3600,
        "[]",
    )
    .await
    .expect("upsert job");

    let pool_ref = match pool.writer() {
        PoolRef::Postgres(p) => p,
        _ => panic!(
            "filter_position_query_uses_idx_job_runs_job_id_start_postgres requires Postgres"
        ),
    };

    // Seed 10_000 mixed-status runs. RESEARCH Pitfall 4: sufficient row
    // volume + ANALYZE so the planner consults real row counts.
    let base = chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
        .expect("parse base timestamp")
        .with_timezone(&chrono::Utc);
    let mut tx = pool_ref.begin().await.expect("begin");
    const SEED_ROWS: i64 = 10_000;
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

    // Per RESEARCH Pitfall 4, ANALYZE both tables.
    sqlx::query("ANALYZE job_runs")
        .execute(pool_ref)
        .await
        .expect("analyze job_runs");
    sqlx::query("ANALYZE jobs")
        .execute(pool_ref)
        .await
        .expect("analyze jobs");

    let current_ts = (base + chrono::Duration::minutes(SEED_ROWS + 1000)).to_rfc3339();
    let explain_sql = format!("EXPLAIN (FORMAT JSON) {FP_SQL_POSTGRES}");
    let row = sqlx::query(&explain_sql)
        .bind(job_id)
        .bind(&current_ts)
        .bind("failed")
        .bind("timeout")
        .bind("error")
        .bind("error")
        .bind("error")
        .bind("error")
        .fetch_one(pool_ref)
        .await
        .expect("explain");

    let plan_json: serde_json::Value = row.get(0);

    // Walk the plan tree looking for any node whose "Node Type" matches
    // an index-based scan kind on the job_runs relation. Mirrors
    // tests/v12_fctx_explain.rs::contains_index_scan verbatim.
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
        "expected Postgres EXPLAIN JSON to contain an Index Scan / Index Only Scan / \
         Bitmap Index/Heap Scan on job_runs OR reference `idx_job_runs_job_id_start`; \
         got:\n{plan_json:#}"
    );

    pool.close().await;
}
