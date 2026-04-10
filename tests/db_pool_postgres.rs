//! DB-02 / DB-03 smoke test: DbPool handles postgres:// URLs and migrations
//! run idempotently against a real Postgres via testcontainers.

use cronduit::db::{DbBackend, DbPool};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

#[tokio::test]
async fn db_pool_connects_and_migrates_against_postgres() {
    let container = Postgres::default().start().await.expect("start postgres");
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let url = format!(
        "postgres://postgres:postgres@{host}:{port}/postgres"
    );

    let pool = DbPool::connect(&url).await.expect("DbPool::connect");
    assert_eq!(pool.backend(), DbBackend::Postgres);
    pool.migrate().await.expect("first migrate");
    pool.migrate().await.expect("second migrate (idempotent)");
    pool.close().await;
}
