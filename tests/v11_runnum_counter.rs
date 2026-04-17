//! Phase 11 per-job counter tests (DB-11). Covers T-V11-RUNNUM-02/-10/-11.
//!
//! Exercises `queries::insert_running_run`'s two-statement counter tx:
//! - runnum_starts_at_1: first inserts produce job_run_number 1, 2, …
//! - insert_running_run_uses_counter_transaction: post-insert,
//!   `jobs.next_run_number` is one more than the new row's job_run_number.
//! - concurrent_inserts_distinct_numbers (T-V11-RUNNUM-10): 16-way race
//!   produces exactly the set {1..=16} with no duplicates or gaps.
//! - next_run_number_invariant (T-V11-RUNNUM-11): after the race,
//!   `jobs.next_run_number` equals MAX(job_run_number) + 1 = 17.
//!
//! Bodies land in Plan 11-05.

#![allow(clippy::assertions_on_constants)]

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
    let n2: i64 = sqlx::query_scalar("SELECT job_run_number FROM job_runs WHERE id = ?1")
        .bind(r2)
        .fetch_one(p)
        .await
        .unwrap();
    assert_eq!(n1, 1, "first insert gets job_run_number = 1");
    assert_eq!(n2, 2, "second insert gets job_run_number = 2");
}

#[tokio::test]
async fn insert_running_run_uses_counter_transaction() {
    let pool = setup_sqlite_with_phase11_migrations().await;
    let job_id = seed_test_job(&pool, "tx-job").await;
    queries::insert_running_run(&pool, job_id, "manual")
        .await
        .unwrap();
    let p = match pool.reader() {
        PoolRef::Sqlite(pp) => pp,
        _ => panic!("sqlite-only"),
    };
    let nrn: i64 = sqlx::query_scalar("SELECT next_run_number FROM jobs WHERE id = ?1")
        .bind(job_id)
        .fetch_one(p)
        .await
        .unwrap();
    assert_eq!(nrn, 2, "next_run_number incremented after one insert");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_inserts_distinct_numbers() {
    let pool = std::sync::Arc::new(setup_sqlite_with_phase11_migrations().await);
    let job_id = seed_test_job(&pool, "race-job").await;

    let mut join_set = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let pool = pool.clone();
        join_set.spawn(async move {
            queries::insert_running_run(&pool, job_id, "manual")
                .await
                .unwrap()
        });
    }
    let mut run_ids = Vec::new();
    while let Some(r) = join_set.join_next().await {
        run_ids.push(r.unwrap());
    }
    assert_eq!(run_ids.len(), 16, "16 spawns should produce 16 run ids");

    // Fetch all job_run_numbers and assert the set equals {1..=16} exactly.
    let p = match pool.reader() {
        PoolRef::Sqlite(pp) => pp,
        _ => panic!("sqlite-only"),
    };
    let mut numbers: Vec<i64> = sqlx::query_scalar(
        "SELECT job_run_number FROM job_runs WHERE job_id = ?1 ORDER BY job_run_number ASC",
    )
    .bind(job_id)
    .fetch_all(p)
    .await
    .unwrap();
    numbers.sort_unstable();
    assert_eq!(
        numbers,
        (1..=16i64).collect::<Vec<_>>(),
        "concurrent inserts produced duplicates or gaps: {:?}",
        numbers
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn next_run_number_invariant() {
    let pool = std::sync::Arc::new(setup_sqlite_with_phase11_migrations().await);
    let job_id = seed_test_job(&pool, "invariant-job").await;
    let mut join_set = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let pool = pool.clone();
        join_set.spawn(async move {
            queries::insert_running_run(&pool, job_id, "manual")
                .await
                .unwrap()
        });
    }
    while join_set.join_next().await.is_some() {}

    let p = match pool.reader() {
        PoolRef::Sqlite(pp) => pp,
        _ => panic!("sqlite-only"),
    };
    let nrn: i64 = sqlx::query_scalar("SELECT next_run_number FROM jobs WHERE id = ?1")
        .bind(job_id)
        .fetch_one(p)
        .await
        .unwrap();
    assert_eq!(
        nrn, 17,
        "next_run_number should be MAX(job_run_number)+1 = 17"
    );
}
