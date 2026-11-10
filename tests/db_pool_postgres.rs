//! DB-02 / DB-03 smoke test: DbPool handles postgres:// URLs and migrations
//! run idempotently against a real Postgres via the Bollard fixture.

#[path = "support/containers.rs"]
mod containers;

use cronduit::db::{DbBackend, DbPool};

#[tokio::test]
async fn db_pool_connects_and_migrates_against_postgres() {
    let container = containers::TestContainer::postgres()
        .await
        .expect("start isolated Postgres fixture");
    let url = container.url();

    let pool = DbPool::connect(url).await.expect("DbPool::connect");
    assert_eq!(pool.backend(), DbBackend::Postgres);
    pool.migrate().await.expect("first migrate");
    pool.migrate().await.expect("second migrate (idempotent)");
    pool.close().await;
}
