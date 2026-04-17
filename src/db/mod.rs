//! Database pool abstraction for SQLite (split read/write) and Postgres.
//!
//! Addresses Pitfall 7 (SQLite writer contention) via `max_connections=1`
//! on the writer pool + WAL + busy_timeout=5000.
//!
//! D-13: Split migration directories are compile-time required because
//! `sqlx::migrate!(PATH)` is a macro whose path is baked into the binary.

pub mod migrate_backfill;
pub mod queries;
pub use queries::{
    DashboardJob, DbJob, DbLogLine, DbRun, DbRunDetail, Paginated, disable_missing_jobs,
    finalize_run, get_dashboard_jobs, get_enabled_jobs, get_job_by_id, get_job_by_name,
    get_log_lines, get_run_by_id, get_run_history, insert_log_batch, insert_running_run,
    update_resolved_schedule, upsert_job,
};

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
use std::str::FromStr;
use std::time::Duration;
use url::Url;

#[derive(Clone, Debug)]
pub enum DbPool {
    Sqlite {
        write: SqlitePool, // max_connections = 1  (Pitfall 7)
        read: SqlitePool,  // max_connections = 8
    },
    Postgres(PgPool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbBackend {
    Sqlite,
    Postgres,
}

impl DbPool {
    pub fn backend(&self) -> DbBackend {
        match self {
            DbPool::Sqlite { .. } => DbBackend::Sqlite,
            DbPool::Postgres(_) => DbBackend::Postgres,
        }
    }

    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        // sqlite::memory: and sqlite:path use ":" not "://", so check prefix first.
        if database_url.starts_with("sqlite:") {
            return Self::connect_sqlite(database_url).await;
        }
        let scheme = database_url.split_once("://").map(|(s, _)| s);
        match scheme {
            Some("postgres") | Some("postgresql") => Self::connect_postgres(database_url).await,
            Some(other) => {
                anyhow::bail!(
                    "unsupported database scheme `{other}://` — use `sqlite://` or `postgres://`"
                )
            }
            None => anyhow::bail!("invalid DATABASE_URL: missing scheme"),
        }
    }

    async fn connect_sqlite(url: &str) -> anyhow::Result<Self> {
        let base = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_millis(5000))
            .foreign_keys(true);

        let write = SqlitePoolOptions::new()
            .max_connections(1) // Pitfall 7: single writer
            .min_connections(1)
            .connect_with(base.clone())
            .await?;

        let read = SqlitePoolOptions::new()
            .max_connections(8)
            .min_connections(1)
            .connect_with(base)
            .await?;

        Ok(DbPool::Sqlite { write, read })
    }

    async fn connect_postgres(url: &str) -> anyhow::Result<Self> {
        let opts = PgConnectOptions::from_str(url)?;
        let pool = PgPoolOptions::new()
            .max_connections(16)
            .connect_with(opts)
            .await?;
        Ok(DbPool::Postgres(pool))
    }

    /// Idempotent migration runner. Safe to call on every startup.
    ///
    /// Phase 11 ordering (D-12/D-13): conditional two-pass strategy. File 3
    /// (20260418_000003) flips `job_runs.job_run_number` to NOT NULL. If the
    /// DB has pre-existing rows where that column is NULL, file 3 would fail
    /// if applied straight through. To handle both fresh installs and
    /// upgrade-in-place, `migrate()`:
    ///
    ///   1. Checks whether file 3's NOT NULL constraint is safe to apply now
    ///      via `file3_can_apply_now()` (safe when the table doesn't exist,
    ///      the column doesn't exist and the table is empty, or the NULL
    ///      count is zero).
    ///   2. If safe: single-pass — `sqlx::migrate!` applies every file, then
    ///      the backfill orchestrator runs and short-circuits on its sentinel.
    ///   3. If not safe: two-pass — apply only files up through version
    ///      `FILE2_VERSION` (the backfill marker); run the orchestrator to
    ///      fill every NULL; then a SECOND `sqlx::migrate!` call picks up
    ///      file 3 safely.
    ///
    /// All three migration files live in the same directory
    /// (`migrations/{sqlite,postgres}/`); selective application is handled in
    /// Rust via `migrate_up_to_backfill_marker()`.
    ///
    /// Runs BEFORE the HTTP listener binds — no concurrent writers (D-12).
    pub async fn migrate(&self) -> anyhow::Result<()> {
        let file3_safe = self.file3_can_apply_now().await.unwrap_or(false);

        if file3_safe {
            // Single-pass path: fresh install OR prior-successful backfill.
            // sqlx applies every migration file; the orchestrator then
            // short-circuits via sentinel / zero-null-count.
            match self {
                DbPool::Sqlite { write, .. } => {
                    sqlx::migrate!("./migrations/sqlite").run(write).await?;
                }
                DbPool::Postgres(pool) => {
                    sqlx::migrate!("./migrations/postgres").run(pool).await?;
                }
            }
            migrate_backfill::backfill_job_run_number(self).await?;
        } else {
            // Two-pass path: upgrade-in-place with NULL rows. Apply files 1+2
            // only, run the orchestrator to fill every NULL, then apply file 3.
            self.migrate_up_to_backfill_marker().await?;
            migrate_backfill::backfill_job_run_number(self).await?;
            match self {
                DbPool::Sqlite { write, .. } => {
                    sqlx::migrate!("./migrations/sqlite").run(write).await?;
                }
                DbPool::Postgres(pool) => {
                    sqlx::migrate!("./migrations/postgres").run(pool).await?;
                }
            }
        }
        Ok(())
    }

    /// Returns `true` if file 3's NOT NULL tightening can apply right now
    /// without constraint violation. Safe when:
    ///   - `job_runs` table does not yet exist (fresh install).
    ///   - `job_run_number` column does not yet exist AND `job_runs` is empty
    ///     (upgrade-in-place on an empty DB).
    ///   - Column exists and has zero NULL values (re-run after successful backfill).
    async fn file3_can_apply_now(&self) -> anyhow::Result<bool> {
        // Check whether job_runs table exists.
        let table_exists = match self {
            DbPool::Sqlite { read, .. } => {
                let n: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM sqlite_master \
                     WHERE type = 'table' AND name = 'job_runs'",
                )
                .fetch_one(read)
                .await
                .unwrap_or(0);
                n > 0
            }
            DbPool::Postgres(pool) => sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS ( \
                    SELECT 1 FROM information_schema.tables \
                    WHERE table_name = 'job_runs' \
                 )",
            )
            .fetch_one(pool)
            .await
            .unwrap_or(false),
        };
        if !table_exists {
            return Ok(true);
        }

        // Check whether job_run_number column exists. If not, table pre-dates
        // Phase 11. If empty, file 1 adds the column to an empty table and
        // file 3 is safe; if non-empty, file 1 adds NULLs that block file 3.
        let column_exists = match self {
            DbPool::Sqlite { read, .. } => {
                let n: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM pragma_table_info('job_runs') \
                     WHERE name = 'job_run_number'",
                )
                .fetch_one(read)
                .await
                .unwrap_or(0);
                n > 0
            }
            DbPool::Postgres(pool) => sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS ( \
                    SELECT 1 FROM information_schema.columns \
                    WHERE table_name = 'job_runs' \
                      AND column_name = 'job_run_number' \
                 )",
            )
            .fetch_one(pool)
            .await
            .unwrap_or(false),
        };
        if !column_exists {
            let row_count: i64 = match self {
                DbPool::Sqlite { read, .. } => {
                    sqlx::query_scalar("SELECT COUNT(*) FROM job_runs")
                        .fetch_one(read)
                        .await
                        .unwrap_or(0)
                }
                DbPool::Postgres(pool) => {
                    sqlx::query_scalar("SELECT COUNT(*) FROM job_runs")
                        .fetch_one(pool)
                        .await
                        .unwrap_or(0)
                }
            };
            return Ok(row_count == 0);
        }

        // Column exists — count NULLs directly.
        let null_count = queries::count_job_runs_with_null_run_number(self).await?;
        Ok(null_count == 0)
    }

    /// Applies every migration with version <= FILE2_VERSION (backfill marker),
    /// so the orchestrator can fill NULLs before file 3's NOT NULL tightening.
    ///
    /// Implementation note: `sqlx::migrate::Migration::apply` is not public in
    /// sqlx 0.8.6. We extract `migration.sql` and execute it directly, then
    /// insert the bookkeeping row into `_sqlx_migrations` matching the schema
    /// sqlx's native runner writes (version, description, success=TRUE,
    /// checksum, execution_time=-1 — `installed_on` gets the table DEFAULT).
    async fn migrate_up_to_backfill_marker(&self) -> anyhow::Result<()> {
        // FILE2_VERSION is the backfill-marker version (date-prefix of
        // `20260417_000002_job_run_number_backfill.up.sql`, parsed by
        // sqlx's splitn(2, '_') rule).
        const FILE2_VERSION: i64 = 20260417;
        match self {
            DbPool::Sqlite { write, .. } => {
                // Ensure the _sqlx_migrations bookkeeping table exists before
                // we try to INSERT into it. This mirrors the setup sqlx does
                // on the first call to Migrator::run().
                sqlx::query(
                    "CREATE TABLE IF NOT EXISTS _sqlx_migrations ( \
                        version BIGINT PRIMARY KEY, \
                        description TEXT NOT NULL, \
                        installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                        success BOOLEAN NOT NULL, \
                        checksum BLOB NOT NULL, \
                        execution_time BIGINT NOT NULL \
                     )",
                )
                .execute(write)
                .await?;
                let m: &sqlx::migrate::Migrator = &sqlx::migrate!("./migrations/sqlite");
                for migration in m.iter() {
                    if migration.version > FILE2_VERSION {
                        continue;
                    }
                    let applied: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM _sqlx_migrations WHERE version = ?1",
                    )
                    .bind(migration.version)
                    .fetch_one(write)
                    .await
                    .unwrap_or(0);
                    if applied > 0 {
                        continue;
                    }
                    // Execute the migration SQL.
                    sqlx::query(&migration.sql).execute(write).await?;
                    // Record the bookkeeping row so sqlx's second call in
                    // `migrate()` picks up only the remaining files (file 3).
                    sqlx::query(
                        "INSERT INTO _sqlx_migrations \
                         ( version, description, success, checksum, execution_time ) \
                         VALUES ( ?1, ?2, 1, ?3, -1 )",
                    )
                    .bind(migration.version)
                    .bind(&*migration.description)
                    .bind(&*migration.checksum)
                    .execute(write)
                    .await?;
                }
            }
            DbPool::Postgres(pool) => {
                sqlx::query(
                    "CREATE TABLE IF NOT EXISTS _sqlx_migrations ( \
                        version BIGINT PRIMARY KEY, \
                        description TEXT NOT NULL, \
                        installed_on TIMESTAMPTZ NOT NULL DEFAULT now(), \
                        success BOOLEAN NOT NULL, \
                        checksum BYTEA NOT NULL, \
                        execution_time BIGINT NOT NULL \
                     )",
                )
                .execute(pool)
                .await?;
                let m: &sqlx::migrate::Migrator = &sqlx::migrate!("./migrations/postgres");
                for migration in m.iter() {
                    if migration.version > FILE2_VERSION {
                        continue;
                    }
                    let applied: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM _sqlx_migrations WHERE version = $1",
                    )
                    .bind(migration.version)
                    .fetch_one(pool)
                    .await
                    .unwrap_or(0);
                    if applied > 0 {
                        continue;
                    }
                    sqlx::query(&migration.sql).execute(pool).await?;
                    sqlx::query(
                        "INSERT INTO _sqlx_migrations \
                         ( version, description, success, checksum, execution_time ) \
                         VALUES ( $1, $2, TRUE, $3, -1 )",
                    )
                    .bind(migration.version)
                    .bind(&*migration.description)
                    .bind(&migration.checksum[..])
                    .execute(pool)
                    .await?;
                }
            }
        }
        Ok(())
    }

    /// Close all underlying pools. Call during graceful shutdown.
    pub async fn close(&self) {
        match self {
            DbPool::Sqlite { write, read } => {
                write.close().await;
                read.close().await;
            }
            DbPool::Postgres(pool) => pool.close().await,
        }
    }
}

/// Strip username + password from a URL-style database connection string.
/// Robust against credentials containing `@` / `?` / `/` chars where a regex
/// would misparse. Falls back to "<unparseable>" on parse error.
pub fn strip_db_credentials(database_url: &str) -> String {
    Url::parse(database_url)
        .map(|mut u| {
            let _ = u.set_password(None);
            let _ = u.set_username("");
            u.to_string()
        })
        .unwrap_or_else(|_| "<unparseable>".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_creds_postgres() {
        let out = strip_db_credentials("postgres://user:pass@host:5432/mydb");
        assert!(!out.contains("user"));
        assert!(!out.contains("pass"));
        assert!(out.contains("host"));
        assert!(out.contains("mydb"));
    }

    #[test]
    fn strip_creds_sqlite_is_unchanged() {
        let out = strip_db_credentials("sqlite:///data/cronduit.db");
        assert_eq!(out, "sqlite:///data/cronduit.db");
    }

    #[tokio::test]
    async fn sqlite_connect_rejects_bad_scheme() {
        let r = DbPool::connect("mysql://anything").await;
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("unsupported"));
    }

    #[tokio::test]
    async fn sqlite_memory_connects_and_migrates() {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        assert_eq!(pool.backend(), DbBackend::Sqlite);
        // Migrate twice to prove idempotency (DB-03).
        pool.migrate().await.unwrap();
        pool.migrate().await.unwrap();
        pool.close().await;
    }
}
