//! Database pool abstraction for SQLite (split read/write) and Postgres.
//!
//! Addresses Pitfall 7 (SQLite writer contention) via `max_connections=1`
//! on the writer pool + WAL + busy_timeout=5000.
//!
//! D-13: Split migration directories are compile-time required because
//! `sqlx::migrate!(PATH)` is a macro whose path is baked into the binary.

pub mod queries;
pub use queries::{DbJob, disable_missing_jobs, get_enabled_jobs, get_job_by_name, upsert_job};

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
    pub async fn migrate(&self) -> anyhow::Result<()> {
        match self {
            DbPool::Sqlite { write, .. } => {
                sqlx::migrate!("./migrations/sqlite").run(write).await?;
            }
            DbPool::Postgres(pool) => {
                sqlx::migrate!("./migrations/postgres").run(pool).await?;
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
