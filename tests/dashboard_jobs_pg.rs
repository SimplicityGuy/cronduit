//! Quick task 260421-nn3: Postgres regression test for
//! `queries::get_dashboard_jobs` — guards against the BIGINT-vs-boolean
//! `operator does not exist` error that was silently breaking the dashboard
//! on the Postgres backend. See .planning/quick/260421-nn3-*/ for context.
//!
//! Scope (per CONTEXT.md D-2: locked 2A no-error-only): assert `Ok(_)` only.
//! Row correctness / ORDER BY / EXPLAIN semantics are out of scope — the
//! bug is a SQL-syntax-level error; `Ok(_)` trivially proves the fix.

use cronduit::db::queries;
use cronduit::db::{DbBackend, DbPool};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

#[tokio::test]
async fn get_dashboard_jobs_postgres_smoke() {
    // Mirror of tests/v13_timeline_explain.rs::explain_uses_index_postgres.
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

    // Seed one enabled job via the production upsert path.
    let _job_id = queries::upsert_job(
        &pool,
        "dash-pg-smoke",
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo hi"}"#,
        "hash-dash-pg",
        3600,
    )
    .await
    .expect("upsert job");

    // Unfiltered path — exercises the buggy line 628 (pre-fix: errors).
    let result = queries::get_dashboard_jobs(&pool, None, "name", "asc").await;
    assert!(
        result.is_ok(),
        "get_dashboard_jobs (unfiltered) must succeed on Postgres; got: {:?}",
        result.err()
    );

    // Filtered path — exercises the buggy line 615 (pre-fix: errors).
    let result_filtered = queries::get_dashboard_jobs(&pool, Some("dash"), "name", "asc").await;
    assert!(
        result_filtered.is_ok(),
        "get_dashboard_jobs (filtered) must succeed on Postgres; got: {:?}",
        result_filtered.err()
    );

    pool.close().await;
}
