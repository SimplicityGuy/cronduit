//! Phase 14 Wave 0 — red-bar Postgres parity tests for the bulk toggle feature.
//!
//! Mirrors the DB-layer tests in `tests/v11_bulk_toggle.rs` but exercises the
//! Postgres backend via `testcontainers-modules::postgres::Postgres`. Same
//! red-bar contract: every test fails to compile (or runtime-fails) until
//! Plans 02 (migration + struct) and 03 (queries) land.
//!
//! Coverage map:
//!
//! | Test                                        | Plan | Mirrors SQLite test |
//! |---------------------------------------------|------|---------------------|
//! | upsert_invariant_pg                         | 02+03 | upsert_invariant     |
//! | disable_missing_clears_override_pg          | 03   | disable_missing_clears_override |
//! | dashboard_filter_pg                         | 03   | dashboard_filter     |
//! | bulk_set_override_pg                        | 03   | (direct query)       |
//! | get_overridden_jobs_alphabetical_pg         | 03   | get_overridden_jobs_alphabetical |
//!
//! Harness pattern is verbatim from `tests/dashboard_jobs_pg.rs` (the
//! testcontainers Postgres template established by quick task 260421-nn3).

use cronduit::db::queries;
use cronduit::db::{DbBackend, DbPool};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::ContainerAsync;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

/// Acquire an ephemeral Postgres pool with all migrations applied.
/// The returned `ContainerAsync` MUST be held alive until the test finishes —
/// dropping it tears down the container and breaks the pool.
async fn pg_pool() -> (ContainerAsync<Postgres>, DbPool) {
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
    (container, pool)
}

/// Seed a single job and return its id (Postgres parity of
/// `tests/v11_bulk_toggle.rs::seed_job`).
async fn seed_job(pool: &DbPool, name: &str) -> i64 {
    queries::upsert_job(
        pool,
        name,
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo hi"}"#,
        &format!("hash-{name}"),
        300,
        "[]",
    )
    .await
    .expect("upsert job")
}

#[tokio::test]
async fn upsert_invariant_pg() {
    // T-V11-BULK-01: upsert_job MUST NOT touch enabled_override on Postgres.
    let (_container, pool) = pg_pool().await;

    let id = seed_job(&pool, "alpha").await;
    let affected = queries::bulk_set_override(&pool, &[id], Some(0))
        .await
        .expect("bulk_set_override");
    assert_eq!(affected, 1);

    queries::upsert_job(
        &pool,
        "alpha",
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo CHANGED"}"#,
        "hash-alpha-v2",
        600,
        "[]",
    )
    .await
    .expect("re-upsert job");

    let job = queries::get_job_by_id(&pool, id)
        .await
        .expect("get_job_by_id")
        .expect("job exists");
    assert_eq!(
        job.enabled_override,
        Some(0),
        "Postgres: upsert_job MUST NOT touch enabled_override (T-V11-BULK-01)"
    );

    pool.close().await;
}

#[tokio::test]
async fn disable_missing_clears_override_pg() {
    // ERG-04 / D-13: jobs removed from config must lose BOTH enabled AND
    // enabled_override on Postgres (parity with SQLite).
    let (_container, pool) = pg_pool().await;

    let id_keep = seed_job(&pool, "keepme").await;
    let id_drop = seed_job(&pool, "dropme").await;
    queries::bulk_set_override(&pool, &[id_keep, id_drop], Some(0))
        .await
        .expect("bulk_set_override");

    let _ = queries::disable_missing_jobs(&pool, &["keepme".to_string()])
        .await
        .expect("disable_missing_jobs");

    let dropped = queries::get_job_by_id(&pool, id_drop)
        .await
        .expect("get_job_by_id")
        .expect("dropme exists");
    assert!(!dropped.enabled, "Postgres: dropme must be disabled");
    assert_eq!(
        dropped.enabled_override, None,
        "Postgres: disable_missing_jobs MUST clear enabled_override (ERG-04)"
    );

    let kept = queries::get_job_by_id(&pool, id_keep)
        .await
        .expect("get_job_by_id")
        .expect("keepme exists");
    assert!(kept.enabled, "Postgres: keepme must stay enabled");
    assert_eq!(
        kept.enabled_override,
        Some(0),
        "Postgres: keepme's override is preserved"
    );

    pool.close().await;
}

#[tokio::test]
async fn dashboard_filter_pg() {
    // DB-14: get_enabled_jobs filter on Postgres mirrors SQLite behavior.
    let (_container, pool) = pg_pool().await;

    let id_a = seed_job(&pool, "A").await;
    let _id_b = seed_job(&pool, "B").await;
    let id_c = seed_job(&pool, "C").await;

    queries::bulk_set_override(&pool, &[id_a], Some(0))
        .await
        .expect("set A");
    queries::bulk_set_override(&pool, &[id_c], Some(1))
        .await
        .expect("set C");

    let mut names: Vec<String> = queries::get_enabled_jobs(&pool)
        .await
        .expect("get_enabled_jobs")
        .into_iter()
        .map(|j| j.name)
        .collect();
    names.sort();

    assert_eq!(
        names,
        vec!["B".to_string(), "C".to_string()],
        "Postgres: get_enabled_jobs must exclude override=0 rows; got {names:?}"
    );

    pool.close().await;
}

#[tokio::test]
async fn bulk_set_override_pg() {
    // Direct query test: validates the Postgres ANY($2) array bind path of
    // bulk_set_override. SQLite uses a generated IN (?2..?N) placeholder list;
    // Postgres uses ANY($2) with a `&[i64]` bind.
    let (_container, pool) = pg_pool().await;

    let id1 = seed_job(&pool, "one").await;
    let id2 = seed_job(&pool, "two").await;
    let id3 = seed_job(&pool, "three").await;

    let affected = queries::bulk_set_override(&pool, &[id1, id2, id3], Some(0))
        .await
        .expect("bulk_set_override Some(0)");
    assert_eq!(
        affected, 3,
        "Postgres: bulk_set_override must report rows_affected = 3"
    );

    for id in [id1, id2, id3] {
        let job = queries::get_job_by_id(&pool, id)
            .await
            .expect("get_job_by_id")
            .expect("job exists");
        assert_eq!(
            job.enabled_override,
            Some(0),
            "Postgres: id={id} must have enabled_override = Some(0)"
        );
    }

    // Now clear id1's override.
    let affected_clear = queries::bulk_set_override(&pool, &[id1], None)
        .await
        .expect("bulk_set_override None");
    assert_eq!(affected_clear, 1);

    let job = queries::get_job_by_id(&pool, id1)
        .await
        .expect("get_job_by_id")
        .expect("id1 exists");
    assert_eq!(
        job.enabled_override, None,
        "Postgres: id1 override must be cleared to NULL"
    );

    pool.close().await;
}

#[tokio::test]
async fn get_overridden_jobs_alphabetical_pg() {
    // D-10b: Postgres parity for alphabetical ordering.
    let (_container, pool) = pg_pool().await;

    let id_z = seed_job(&pool, "zebra").await;
    let id_a = seed_job(&pool, "alpha").await;
    let id_m = seed_job(&pool, "mango").await;
    queries::bulk_set_override(&pool, &[id_z, id_a, id_m], Some(0))
        .await
        .expect("bulk_set_override");

    let names: Vec<String> = queries::get_overridden_jobs(&pool)
        .await
        .expect("get_overridden_jobs")
        .into_iter()
        .map(|j| j.name)
        .collect();
    assert_eq!(
        names,
        vec![
            "alpha".to_string(),
            "mango".to_string(),
            "zebra".to_string()
        ],
        "Postgres: get_overridden_jobs MUST return rows in alphabetical order (D-10b)"
    );

    pool.close().await;
}
