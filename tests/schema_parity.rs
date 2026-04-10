//! tests/schema_parity.rs
//!
//! Structural parity between migrations/sqlite and migrations/postgres.
//! Surfaced via `just schema-diff`. Runs in every CI matrix cell.
//!
//! Failure of this test is a HARD STOP — do not merge.
//!
//! Addresses Pitfall 8 (SQLite/Postgres schema drift) and T-01-12.

use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{PgPool, Row, SqlitePool};
use std::collections::{BTreeMap, BTreeSet};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Clone)]
struct Column {
    name: String,
    normalized_type: String,
    not_null: bool,
}

#[derive(Debug, Default)]
struct Schema {
    tables: BTreeMap<String, BTreeSet<Column>>,
    indexes: BTreeMap<String, BTreeSet<String>>,
}

/// Whitelist: normalize SQLite and Postgres type names to a shared token.
/// Each branch MUST carry a comment explaining why the normalization is
/// semantically safe. Unknown types panic so reviewers get a clear signal.
fn normalize_type(raw: &str) -> String {
    let upper = raw.trim().to_ascii_uppercase();
    // Strip modifiers like "INTEGER(10)" or "CHARACTER VARYING(255)".
    let base = upper.split('(').next().unwrap_or(&upper).trim();
    match base {
        // SQLite INTEGER PRIMARY KEY rowid == Postgres BIGSERIAL (both Rust i64).
        // SQLite INTEGER FK/size columns == Postgres BIGINT (both i64 in sqlx).
        // Postgres information_schema reports "bigint" for BIGSERIAL columns.
        "INTEGER" | "BIGINT" | "BIGSERIAL" | "INT8" => "INT64".to_string(),
        // Postgres SMALLINT used for the 0/1 `enabled` flag; SQLite stores it
        // as INTEGER (normalized above). We never see a raw SMALLINT on SQLite.
        "SMALLINT" | "INT2" => "INT16".to_string(),
        // Exit code column: Postgres INTEGER (4-byte), SQLite INTEGER (alias for i64).
        // Both map to sqlx i32 when using `get::<i32>`. Safe normalization.
        "INT" | "INT4" => "INT32".to_string(),
        // Text-ish types share semantics on both backends.
        "TEXT" | "VARCHAR" | "CHARACTER VARYING" | "CHAR" | "CHARACTER" => "TEXT".to_string(),
        other => panic!(
            "unknown column type `{other}` — add to normalize_type whitelist with a justification comment"
        ),
    }
}

async fn introspect_sqlite(pool: &SqlitePool) -> Schema {
    let mut schema = Schema::default();

    let rows = sqlx::query(
        "SELECT name FROM sqlite_master
         WHERE type='table'
           AND name NOT LIKE 'sqlx_%'
           AND name NOT LIKE 'sqlite_%'
           AND name != '_sqlx_migrations'
         ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap();

    for row in rows {
        let tbl: String = row.get(0);

        // Columns
        let col_rows = sqlx::query(&format!("PRAGMA table_info('{tbl}')"))
            .fetch_all(pool)
            .await
            .unwrap();
        let mut col_set = BTreeSet::new();
        for c in col_rows {
            let raw_type: String = c.get("type");
            let notnull: i32 = c.get("notnull");
            let is_pk: i32 = c.get("pk");
            // SQLite quirk: `INTEGER PRIMARY KEY` columns report notnull=0
            // in PRAGMA table_info even though they can never be NULL (they
            // are the rowid alias). Treat pk > 0 as implicitly NOT NULL to
            // match Postgres BIGSERIAL PRIMARY KEY behavior.
            col_set.insert(Column {
                name: c.get::<String, _>("name"),
                normalized_type: normalize_type(&raw_type),
                not_null: notnull != 0 || is_pk > 0,
            });
        }
        schema.tables.insert(tbl.clone(), col_set);

        // Indexes (skip autoindex entries)
        let idx_rows = sqlx::query(&format!("PRAGMA index_list('{tbl}')"))
            .fetch_all(pool)
            .await
            .unwrap();
        let mut idx_set = BTreeSet::new();
        for ir in idx_rows {
            let nm: String = ir.get("name");
            if nm.starts_with("sqlite_autoindex_") {
                continue;
            }
            idx_set.insert(nm);
        }
        schema.indexes.insert(tbl, idx_set);
    }

    schema
}

async fn introspect_postgres(pool: &PgPool) -> Schema {
    let mut schema = Schema::default();

    let tbl_rows = sqlx::query(
        "SELECT table_name FROM information_schema.tables
         WHERE table_schema = 'public'
           AND table_name NOT LIKE '\\_sqlx_%'
           AND table_name <> '_sqlx_migrations'
         ORDER BY table_name",
    )
    .fetch_all(pool)
    .await
    .unwrap();

    for row in tbl_rows {
        let tbl: String = row.get(0);

        let col_rows = sqlx::query(
            "SELECT column_name, data_type, is_nullable
             FROM information_schema.columns
             WHERE table_schema = 'public' AND table_name = $1
             ORDER BY column_name",
        )
        .bind(&tbl)
        .fetch_all(pool)
        .await
        .unwrap();

        let mut col_set = BTreeSet::new();
        for c in col_rows {
            let name: String = c.get("column_name");
            let raw_type: String = c.get("data_type");
            let nullable: String = c.get("is_nullable");
            col_set.insert(Column {
                name,
                normalized_type: normalize_type(&raw_type),
                not_null: nullable == "NO",
            });
        }
        schema.tables.insert(tbl.clone(), col_set);

        let idx_rows = sqlx::query(
            "SELECT indexname FROM pg_indexes
             WHERE schemaname = 'public' AND tablename = $1
             ORDER BY indexname",
        )
        .bind(&tbl)
        .fetch_all(pool)
        .await
        .unwrap();

        let mut idx_set = BTreeSet::new();
        for ir in idx_rows {
            let nm: String = ir.get("indexname");
            // Skip implicit PK / unique constraint indexes (Postgres auto-creates them
            // with predictable names we don't declare explicitly in either migration).
            if nm.ends_with("_pkey") || nm.ends_with("_name_key") {
                continue;
            }
            idx_set.insert(nm);
        }
        schema.indexes.insert(tbl, idx_set);
    }

    schema
}

fn diff_report(sqlite: &Schema, pg: &Schema) -> Option<String> {
    let mut out = String::new();
    let sqlite_tables: BTreeSet<&String> = sqlite.tables.keys().collect();
    let pg_tables: BTreeSet<&String> = pg.tables.keys().collect();
    let only_sqlite: Vec<&&String> = sqlite_tables.difference(&pg_tables).collect();
    let only_pg: Vec<&&String> = pg_tables.difference(&sqlite_tables).collect();
    if !only_sqlite.is_empty() {
        out.push_str(&format!("Tables only in SQLite: {only_sqlite:?}\n"));
    }
    if !only_pg.is_empty() {
        out.push_str(&format!("Tables only in Postgres: {only_pg:?}\n"));
    }

    for tbl in sqlite_tables.intersection(&pg_tables) {
        let s_cols = &sqlite.tables[*tbl];
        let p_cols = &pg.tables[*tbl];
        if s_cols != p_cols {
            out.push_str(&format!(
                "\nTable `{tbl}` column diff:\n  sqlite: {s_cols:#?}\n  postgres: {p_cols:#?}\n"
            ));
        }
        let s_idx = sqlite.indexes.get(*tbl).cloned().unwrap_or_default();
        let p_idx = pg.indexes.get(*tbl).cloned().unwrap_or_default();
        if s_idx != p_idx {
            out.push_str(&format!(
                "\nTable `{tbl}` index diff:\n  sqlite: {s_idx:?}\n  postgres: {p_idx:?}\n"
            ));
        }
    }

    if out.is_empty() { None } else { Some(out) }
}

#[tokio::test]
async fn sqlite_and_postgres_schemas_match_structurally() {
    // 1. SQLite in-memory
    let sqlite = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("sqlite connect");
    sqlx::migrate!("./migrations/sqlite")
        .run(&sqlite)
        .await
        .expect("sqlite migrate");

    // 2. Postgres via testcontainers
    let pg_container = Postgres::default()
        .start()
        .await
        .expect("start postgres container");
    let host = pg_container.get_host().await.expect("get host");
    let port = pg_container
        .get_host_port_ipv4(5432)
        .await
        .expect("get port");
    let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");
    let pg = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .expect("pg connect");
    sqlx::migrate!("./migrations/postgres")
        .run(&pg)
        .await
        .expect("pg migrate");

    // 3. Introspect both
    let s_schema = introspect_sqlite(&sqlite).await;
    let p_schema = introspect_postgres(&pg).await;

    // 4. Diff
    if let Some(report) = diff_report(&s_schema, &p_schema) {
        panic!("schema drift detected:\n\n{report}");
    }

    sqlite.close().await;
    pg.close().await;
}

#[cfg(test)]
mod normalize_tests {
    use super::normalize_type;

    #[test]
    fn known_types_normalize_correctly() {
        assert_eq!(normalize_type("INTEGER"), "INT64");
        assert_eq!(normalize_type("BIGINT"), "INT64");
        assert_eq!(normalize_type("BIGSERIAL"), "INT64");
        assert_eq!(normalize_type("bigint"), "INT64");
        assert_eq!(normalize_type("SMALLINT"), "INT16");
        assert_eq!(normalize_type("smallint"), "INT16");
        assert_eq!(normalize_type("INTEGER"), "INT64");
        assert_eq!(normalize_type("integer"), "INT64");
        assert_eq!(normalize_type("TEXT"), "TEXT");
        assert_eq!(normalize_type("text"), "TEXT");
        assert_eq!(normalize_type("character varying"), "TEXT");
    }

    #[test]
    #[should_panic(expected = "add to normalize_type whitelist")]
    fn unknown_type_panics() {
        normalize_type("JSONB");
    }
}
