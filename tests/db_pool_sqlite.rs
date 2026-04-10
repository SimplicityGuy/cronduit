//! Pitfall 7 guard: confirm the writer pool really has WAL mode, a 5 s
//! busy_timeout, NORMAL synchronous mode, and foreign keys enabled.

use cronduit::db::DbPool;
use tempfile::TempDir;

#[tokio::test]
async fn sqlite_writer_pragmas_match_expectations() {
    // Use a file-backed DB so WAL mode actually sticks (in-memory reports "memory").
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("cronduit.dev.db");
    let url = format!("sqlite://{}", db_path.display());

    let pool = DbPool::connect(&url).await.unwrap();
    let DbPool::Sqlite { write, .. } = &pool else {
        panic!("expected sqlite pool");
    };

    let jm: (String,) = sqlx::query_as("PRAGMA journal_mode")
        .fetch_one(write)
        .await
        .unwrap();
    assert_eq!(jm.0.to_lowercase(), "wal");

    let bt: (i64,) = sqlx::query_as("PRAGMA busy_timeout")
        .fetch_one(write)
        .await
        .unwrap();
    assert_eq!(bt.0, 5000);

    // PRAGMA synchronous: 0=OFF, 1=NORMAL, 2=FULL, 3=EXTRA
    let sync: (i64,) = sqlx::query_as("PRAGMA synchronous")
        .fetch_one(write)
        .await
        .unwrap();
    assert_eq!(sync.0, 1, "expected synchronous=NORMAL (1)");

    let fk: (i64,) = sqlx::query_as("PRAGMA foreign_keys")
        .fetch_one(write)
        .await
        .unwrap();
    assert_eq!(fk.0, 1);

    pool.close().await;
}

#[tokio::test]
async fn sqlite_write_pool_has_single_connection() {
    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    let DbPool::Sqlite { write, read, .. } = &pool else {
        panic!("expected sqlite pool");
    };
    // sqlx doesn't expose max_connections directly, but size() returns current
    // connections. With min_connections(1), size should be 1 after connect.
    assert_eq!(write.size(), 1, "writer pool should have 1 connection");
    assert!(
        read.size() >= 1,
        "reader pool should have at least 1 connection"
    );
    pool.close().await;
}
