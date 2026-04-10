//! Database query helpers for job CRUD operations.
//!
//! Provides pool accessor methods (`writer`/`reader`) and async query
//! functions that work against both SQLite and Postgres backends.
//!
//! Uses `sqlx::query_as` with runtime SQL strings (not the `query!` macro)
//! to avoid requiring a live DB connection at compile time.

use super::DbPool;
use sqlx::postgres::PgPool;
use sqlx::sqlite::SqlitePool;
use sqlx::{FromRow, Row};

/// Reference to a specific pool variant, returned by `writer()`/`reader()`.
pub enum PoolRef<'a> {
    Sqlite(&'a SqlitePool),
    Postgres(&'a PgPool),
}

impl DbPool {
    /// Returns a reference to the write-capable pool.
    pub fn writer(&self) -> PoolRef<'_> {
        match self {
            DbPool::Sqlite { write, .. } => PoolRef::Sqlite(write),
            DbPool::Postgres(pool) => PoolRef::Postgres(pool),
        }
    }

    /// Returns a reference to the read pool.
    pub fn reader(&self) -> PoolRef<'_> {
        match self {
            DbPool::Sqlite { read, .. } => PoolRef::Sqlite(read),
            DbPool::Postgres(pool) => PoolRef::Postgres(pool),
        }
    }
}

/// A row from the `jobs` table.
#[derive(Debug, Clone)]
pub struct DbJob {
    pub id: i64,
    pub name: String,
    pub schedule: String,
    pub resolved_schedule: String,
    pub job_type: String,
    pub config_json: String,
    pub config_hash: String,
    pub enabled: bool,
    pub timeout_secs: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Upsert a job by name. On conflict, updates all mutable fields and
/// re-enables the job. Returns the job id.
#[allow(clippy::too_many_arguments)]
pub async fn upsert_job(
    pool: &DbPool,
    name: &str,
    schedule: &str,
    resolved_schedule: &str,
    job_type: &str,
    config_json: &str,
    config_hash: &str,
    timeout_secs: i64,
) -> anyhow::Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();

    match pool.writer() {
        PoolRef::Sqlite(p) => {
            let row = sqlx::query(
                r#"INSERT INTO jobs (name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, timeout_secs, created_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?8)
                   ON CONFLICT(name) DO UPDATE SET
                       schedule = excluded.schedule,
                       resolved_schedule = excluded.resolved_schedule,
                       job_type = excluded.job_type,
                       config_json = excluded.config_json,
                       config_hash = excluded.config_hash,
                       enabled = 1,
                       timeout_secs = excluded.timeout_secs,
                       updated_at = excluded.updated_at
                   RETURNING id"#,
            )
            .bind(name)
            .bind(schedule)
            .bind(resolved_schedule)
            .bind(job_type)
            .bind(config_json)
            .bind(config_hash)
            .bind(timeout_secs)
            .bind(&now)
            .fetch_one(p)
            .await?;
            Ok(row.get::<i64, _>("id"))
        }
        PoolRef::Postgres(p) => {
            let row = sqlx::query(
                r#"INSERT INTO jobs (name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, timeout_secs, created_at, updated_at)
                   VALUES ($1, $2, $3, $4, $5, $6, 1, $7, $8, $8)
                   ON CONFLICT(name) DO UPDATE SET
                       schedule = EXCLUDED.schedule,
                       resolved_schedule = EXCLUDED.resolved_schedule,
                       job_type = EXCLUDED.job_type,
                       config_json = EXCLUDED.config_json,
                       config_hash = EXCLUDED.config_hash,
                       enabled = 1,
                       timeout_secs = EXCLUDED.timeout_secs,
                       updated_at = EXCLUDED.updated_at
                   RETURNING id"#,
            )
            .bind(name)
            .bind(schedule)
            .bind(resolved_schedule)
            .bind(job_type)
            .bind(config_json)
            .bind(config_hash)
            .bind(timeout_secs)
            .bind(&now)
            .fetch_one(p)
            .await?;
            Ok(row.get::<i64, _>("id"))
        }
    }
}

/// Disable all jobs whose names are NOT in `active_names`.
/// Returns the count of rows that were disabled.
pub async fn disable_missing_jobs(pool: &DbPool, active_names: &[String]) -> anyhow::Result<u64> {
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            if active_names.is_empty() {
                let result =
                    sqlx::query("UPDATE jobs SET enabled = 0 WHERE enabled = 1")
                        .execute(p)
                        .await?;
                return Ok(result.rows_affected());
            }
            // SQLite doesn't support array binds; build a parameterized IN list.
            let placeholders: Vec<String> = (1..=active_names.len())
                .map(|i| format!("?{i}"))
                .collect();
            let sql = format!(
                "UPDATE jobs SET enabled = 0 WHERE enabled = 1 AND name NOT IN ({})",
                placeholders.join(", ")
            );
            let mut query = sqlx::query(&sql);
            for name in active_names {
                query = query.bind(name);
            }
            let result = query.execute(p).await?;
            Ok(result.rows_affected())
        }
        PoolRef::Postgres(p) => {
            if active_names.is_empty() {
                let result =
                    sqlx::query("UPDATE jobs SET enabled = 0 WHERE enabled = 1")
                        .execute(p)
                        .await?;
                return Ok(result.rows_affected());
            }
            // Postgres supports ANY($1) with array bind.
            let result = sqlx::query(
                "UPDATE jobs SET enabled = 0 WHERE enabled = 1 AND name != ALL($1)",
            )
            .bind(active_names)
            .execute(p)
            .await?;
            Ok(result.rows_affected())
        }
    }
}

/// Fetch all enabled jobs from the database.
pub async fn get_enabled_jobs(pool: &DbPool) -> anyhow::Result<Vec<DbJob>> {
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let rows = sqlx::query_as::<_, SqliteDbJobRow>(
                "SELECT id, name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, timeout_secs, created_at, updated_at FROM jobs WHERE enabled = 1",
            )
            .fetch_all(p)
            .await?;
            Ok(rows.into_iter().map(|r| r.into()).collect())
        }
        PoolRef::Postgres(p) => {
            let rows = sqlx::query_as::<_, PgDbJobRow>(
                "SELECT id, name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, timeout_secs, created_at, updated_at FROM jobs WHERE enabled = 1",
            )
            .fetch_all(p)
            .await?;
            Ok(rows.into_iter().map(|r| r.into()).collect())
        }
    }
}

/// Fetch a single job by name (used internally for tests and sync verification).
pub async fn get_job_by_name(pool: &DbPool, name: &str) -> anyhow::Result<Option<DbJob>> {
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let row = sqlx::query_as::<_, SqliteDbJobRow>(
                "SELECT id, name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, timeout_secs, created_at, updated_at FROM jobs WHERE name = ?1",
            )
            .bind(name)
            .fetch_optional(p)
            .await?;
            Ok(row.map(|r| r.into()))
        }
        PoolRef::Postgres(p) => {
            let row = sqlx::query_as::<_, PgDbJobRow>(
                "SELECT id, name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, timeout_secs, created_at, updated_at FROM jobs WHERE name = $1",
            )
            .bind(name)
            .fetch_optional(p)
            .await?;
            Ok(row.map(|r| r.into()))
        }
    }
}

// Internal row types for sqlx::FromRow mapping (SQLite uses i32/i64 for booleans).

#[derive(FromRow)]
struct SqliteDbJobRow {
    id: i64,
    name: String,
    schedule: String,
    resolved_schedule: String,
    job_type: String,
    config_json: String,
    config_hash: String,
    enabled: i32,
    timeout_secs: i64,
    created_at: String,
    updated_at: String,
}

impl From<SqliteDbJobRow> for DbJob {
    fn from(r: SqliteDbJobRow) -> Self {
        DbJob {
            id: r.id,
            name: r.name,
            schedule: r.schedule,
            resolved_schedule: r.resolved_schedule,
            job_type: r.job_type,
            config_json: r.config_json,
            config_hash: r.config_hash,
            enabled: r.enabled != 0,
            timeout_secs: r.timeout_secs,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(FromRow)]
struct PgDbJobRow {
    id: i64,
    name: String,
    schedule: String,
    resolved_schedule: String,
    job_type: String,
    config_json: String,
    config_hash: String,
    enabled: bool,
    timeout_secs: i64,
    created_at: String,
    updated_at: String,
}

impl From<PgDbJobRow> for DbJob {
    fn from(r: PgDbJobRow) -> Self {
        DbJob {
            id: r.id,
            name: r.name,
            schedule: r.schedule,
            resolved_schedule: r.resolved_schedule,
            job_type: r.job_type,
            config_json: r.config_json,
            config_hash: r.config_hash,
            enabled: r.enabled,
            timeout_secs: r.timeout_secs,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_pool() -> DbPool {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        pool.migrate().await.unwrap();
        pool
    }

    #[tokio::test]
    async fn writer_returns_write_pool_reader_returns_read_pool() {
        let pool = setup_pool().await;
        // Just verify the accessors return the correct variant without panicking.
        match pool.writer() {
            PoolRef::Sqlite(_) => {}
            PoolRef::Postgres(_) => panic!("expected SQLite writer"),
        }
        match pool.reader() {
            PoolRef::Sqlite(_) => {}
            PoolRef::Postgres(_) => panic!("expected SQLite reader"),
        }
        pool.close().await;
    }

    #[tokio::test]
    async fn upsert_inserts_new_job() {
        let pool = setup_pool().await;
        let id = upsert_job(
            &pool,
            "test-job",
            "*/5 * * * *",
            "*/5 * * * *",
            "command",
            r#"{"name":"test-job"}"#,
            "abc123",
            3600,
        )
        .await
        .unwrap();
        assert!(id > 0);

        let job = get_job_by_name(&pool, "test-job").await.unwrap().unwrap();
        assert_eq!(job.name, "test-job");
        assert_eq!(job.config_hash, "abc123");
        assert_eq!(job.job_type, "command");
        assert!(job.enabled);
        assert_eq!(job.timeout_secs, 3600);
        pool.close().await;
    }

    #[tokio::test]
    async fn upsert_updates_on_conflict() {
        let pool = setup_pool().await;
        upsert_job(
            &pool,
            "test-job",
            "*/5 * * * *",
            "*/5 * * * *",
            "command",
            r#"{"name":"test-job"}"#,
            "hash1",
            3600,
        )
        .await
        .unwrap();

        let before = get_job_by_name(&pool, "test-job").await.unwrap().unwrap();

        // Small delay to ensure updated_at differs.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        upsert_job(
            &pool,
            "test-job",
            "*/10 * * * *",
            "*/10 * * * *",
            "script",
            r#"{"name":"test-job","changed":true}"#,
            "hash2",
            7200,
        )
        .await
        .unwrap();

        let after = get_job_by_name(&pool, "test-job").await.unwrap().unwrap();
        assert_eq!(after.config_hash, "hash2");
        assert_eq!(after.job_type, "script");
        assert_eq!(after.schedule, "*/10 * * * *");
        assert_eq!(after.timeout_secs, 7200);
        assert_ne!(before.updated_at, after.updated_at);
        pool.close().await;
    }

    #[tokio::test]
    async fn upsert_noop_same_hash_still_updates() {
        // Even with the same hash, upsert always writes (by design).
        // The sync engine checks hash before calling upsert to skip no-ops.
        let pool = setup_pool().await;
        upsert_job(
            &pool,
            "test-job",
            "*/5 * * * *",
            "*/5 * * * *",
            "command",
            r#"{"name":"test-job"}"#,
            "same-hash",
            3600,
        )
        .await
        .unwrap();

        let job = get_job_by_name(&pool, "test-job").await.unwrap().unwrap();
        assert_eq!(job.config_hash, "same-hash");
        pool.close().await;
    }

    #[tokio::test]
    async fn disable_missing_jobs_disables_removed() {
        let pool = setup_pool().await;
        upsert_job(&pool, "keep", "* * * * *", "* * * * *", "command", "{}", "h1", 60)
            .await
            .unwrap();
        upsert_job(&pool, "remove", "* * * * *", "* * * * *", "command", "{}", "h2", 60)
            .await
            .unwrap();

        let disabled = disable_missing_jobs(&pool, &["keep".to_string()])
            .await
            .unwrap();
        assert_eq!(disabled, 1);

        let removed = get_job_by_name(&pool, "remove").await.unwrap().unwrap();
        assert!(!removed.enabled);

        let kept = get_job_by_name(&pool, "keep").await.unwrap().unwrap();
        assert!(kept.enabled);
        pool.close().await;
    }

    #[tokio::test]
    async fn disable_missing_jobs_empty_disables_all() {
        let pool = setup_pool().await;
        upsert_job(&pool, "a", "* * * * *", "* * * * *", "command", "{}", "h1", 60)
            .await
            .unwrap();
        upsert_job(&pool, "b", "* * * * *", "* * * * *", "command", "{}", "h2", 60)
            .await
            .unwrap();

        let disabled = disable_missing_jobs(&pool, &[]).await.unwrap();
        assert_eq!(disabled, 2);
        pool.close().await;
    }

    #[tokio::test]
    async fn get_enabled_jobs_filters_disabled() {
        let pool = setup_pool().await;
        upsert_job(&pool, "enabled1", "* * * * *", "* * * * *", "command", "{}", "h1", 60)
            .await
            .unwrap();
        upsert_job(&pool, "enabled2", "* * * * *", "* * * * *", "command", "{}", "h2", 60)
            .await
            .unwrap();
        upsert_job(&pool, "disabled", "* * * * *", "* * * * *", "command", "{}", "h3", 60)
            .await
            .unwrap();

        disable_missing_jobs(&pool, &["enabled1".to_string(), "enabled2".to_string()])
            .await
            .unwrap();

        let jobs = get_enabled_jobs(&pool).await.unwrap();
        assert_eq!(jobs.len(), 2);
        let names: Vec<&str> = jobs.iter().map(|j| j.name.as_str()).collect();
        assert!(names.contains(&"enabled1"));
        assert!(names.contains(&"enabled2"));
        assert!(!names.contains(&"disabled"));
        pool.close().await;
    }
}
