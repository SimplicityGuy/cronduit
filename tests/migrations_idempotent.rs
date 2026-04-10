//! DB-03: migrations run idempotently on startup.

use cronduit::db::DbPool;
use sqlx::Row;

#[tokio::test]
async fn migrate_is_idempotent_and_creates_expected_tables() {
    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    pool.migrate().await.expect("first migrate");
    pool.migrate().await.expect("second migrate (idempotent)");

    let DbPool::Sqlite { read, .. } = &pool else {
        panic!("expected sqlite pool");
    };

    let rows = sqlx::query(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlx_%' AND name NOT LIKE 'sqlite_%' ORDER BY name"
    )
    .fetch_all(read)
    .await
    .unwrap();

    let names: Vec<String> = rows.iter().map(|r| r.get::<String, _>(0)).collect();
    assert!(
        names.contains(&"jobs".to_string()),
        "missing jobs: {names:?}"
    );
    assert!(
        names.contains(&"job_runs".to_string()),
        "missing job_runs: {names:?}"
    );
    assert!(
        names.contains(&"job_logs".to_string()),
        "missing job_logs: {names:?}"
    );

    // Confirm config_hash column exists on jobs (D-15).
    let cols = sqlx::query("PRAGMA table_info('jobs')")
        .fetch_all(read)
        .await
        .unwrap();
    let col_names: Vec<String> = cols.iter().map(|r| r.get::<String, _>("name")).collect();
    assert!(
        col_names.contains(&"config_hash".to_string()),
        "jobs.config_hash missing: {col_names:?}"
    );

    pool.close().await;
}
