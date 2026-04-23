# Research: fix-get-dashboard-jobs-postgres-j-enable

**Researched:** 2026-04-21
**Scope:** quick-task — 2 source-line fix + one Postgres integration test.

## Feature flag

**The precedent test (`tests/v13_timeline_explain.rs::explain_uses_index_postgres`) is NOT feature-gated.** It lives in the default test suite alongside `db_pool_postgres.rs` and `schema_parity.rs`, all of which call `Postgres::default().start().await` unconditionally. Verified:

```
$ grep cfg\(feature tests/v13_timeline_explain.rs
(no matches)
```

The `integration` feature flag in `Cargo.toml` (line 131: `integration = []`) is reserved for a different class of tests — ones that assume a Docker daemon exists on the host for **scenarios beyond a testcontainer-owned Postgres** (e.g. `tests/docker_orphan_guard.rs` Postgres module, `tests/stop_executors.rs`, `tests/v11_runnum_migration.rs`). Testcontainer-Postgres-only tests run by default.

**Planner guidance:** Do NOT put `#[cfg(feature = "integration")]` on the new test. Mirror `v13_timeline_explain.rs` exactly — bare `#[tokio::test]`. The test runs on `cargo nextest run` / `cargo test` without any `--features` flag. CI already runs this tier.

The test file name should be **non-`v13`-prefixed** per CONTEXT.md discretion — recommended: `tests/dashboard_jobs_pg.rs`.

## Testcontainers harness minimum

Extracted from `v13_timeline_explain.rs::explain_uses_index_postgres` (lines 156-172) + `db_pool_postgres.rs`. Minimum reproducible setup:

```rust
use cronduit::db::queries;
use cronduit::db::{DbBackend, DbPool};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

#[tokio::test]
async fn get_dashboard_jobs_postgres_smoke() {
    // 1. Start Postgres container (Postgres::default() pulls `postgres:alpine`).
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

    // 2. Connect pool + run migrations (verifies backend is Postgres).
    let pool = DbPool::connect(&url).await.expect("DbPool::connect");
    assert_eq!(pool.backend(), DbBackend::Postgres);
    pool.migrate().await.expect("run migrations");

    // 3. Insert one enabled job via the production upsert path.
    //    Signature verified at src/db/queries.rs:57-66.
    let _job_id = queries::upsert_job(
        &pool,
        "dash-pg-smoke",
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        r#"{"command":"echo hi"}"#,
        "hash-dash-pg",
        3600,
    )
    .await
    .expect("upsert job");

    // 4. Call get_dashboard_jobs — the pre-fix version panics with
    //    "operator does not exist: bigint = boolean". Signature:
    //    get_dashboard_jobs(&DbPool, Option<&str>, &str, &str)
    //    -> anyhow::Result<Vec<DashboardJob>>.
    let result = queries::get_dashboard_jobs(&pool, None, "name", "asc").await;
    assert!(
        result.is_ok(),
        "get_dashboard_jobs must succeed on Postgres; got: {:?}",
        result.err()
    );

    pool.close().await;
}
```

**Dev-dependencies already present in `Cargo.toml` (lines 134-135) — no changes needed:**
- `testcontainers = "0.27.3"`
- `testcontainers-modules = { version = "0.15.0", features = ["postgres"] }`

**Optional second assertion (CONTEXT.md Claude's Discretion item, line 615):** a second call with a filter argument exercises the filter branch:
```rust
let result_filtered = queries::get_dashboard_jobs(&pool, Some("dash"), "name", "asc").await;
assert!(result_filtered.is_ok(), "...filtered path must also succeed: {:?}", result_filtered.err());
```
Recommend including both — the delta is 3 lines and covers both buggy lines in one test file.

## jobs.enabled type confirmation

**Verified at** `migrations/postgres/20260410_000000_initial.up.sql` line 17:

```sql
-- `enabled` uses BIGINT (not BOOLEAN) to keep the sqlx decode type consistent
-- with SQLite's INTEGER. Both backends decode to i64 via sqlx. See schema_parity.rs.
enabled            BIGINT   NOT NULL DEFAULT 1,
```

The schema comment itself acknowledges this is the intentional cross-dialect choice. `WHERE j.enabled = true` on a BIGINT column raises Postgres error `operator does not exist: bigint = boolean`. The correct comparison is `j.enabled = 1` (integer literal), which matches the working SQLite arm at `src/db/queries.rs:562` and `:575`, and matches the fixed `get_timeline_runs` Postgres arm (plan 13-06, commit `9f5e6c9`).

## Exact fix pattern

Two edits in `src/db/queries.rs`, both inside the `PoolRef::Postgres(p)` match arm of `get_dashboard_jobs` (lines 603-653). Each old string includes enough surrounding context to be uniquely matched by the `Edit` tool — both appear only inside the Postgres arm because the SQLite arm uses `?1`/`?` placeholders, not `$1`.

### Edit 1 — line 615 (filtered path)

**Old:**
```
                       ) lr ON lr.job_id = j.id AND lr.rn = 1
                       WHERE j.enabled = true AND LOWER(j.name) LIKE $1
                       {order_clause}"#
```

**New:**
```
                       ) lr ON lr.job_id = j.id AND lr.rn = 1
                       WHERE j.enabled = 1 AND LOWER(j.name) LIKE $1
                       {order_clause}"#
```

### Edit 2 — line 628 (unfiltered path)

**Old:**
```
                       ) lr ON lr.job_id = j.id AND lr.rn = 1
                       WHERE j.enabled = true
                       {order_clause}"#
```

**New:**
```
                       ) lr ON lr.job_id = j.id AND lr.rn = 1
                       WHERE j.enabled = 1
                       {order_clause}"#
```

Both strings are unambiguous — they only match inside the Postgres format! block because of the `$1` placeholder (Edit 1) and the adjacency to `)}"#` without a filter clause (Edit 2).

## Pitfalls / gotchas

- **Docker daemon required at test time.** `Postgres::default().start()` pulls the `postgres:alpine` image from Docker Hub on first run — first local execution may take 10-30s while the image downloads. CI machines typically have it cached. No code-side mitigation needed; this matches the existing `v13_timeline_explain.rs` precedent and the project already tolerates it.
- **Do not add `#[cfg(feature = "integration")]`.** That gate is reserved for tests that assume a *host* Docker daemon for non-Postgres scenarios. Testcontainer-Postgres tests run by default (confirmed by `db_pool_postgres.rs`, `schema_parity.rs`, `v13_timeline_explain.rs`).
- **Do not add a `.sqlx/` offline metadata file.** The Postgres SQL in `queries.rs` uses `sqlx::query(&pg_sql)` (runtime string), not the `query!` macro, so no compile-time DB check is involved — no `cargo sqlx prepare` regeneration needed.
- **`pool.close().await` at end of test** — matches existing pattern in `v13_timeline_explain.rs:321`. Not strictly required (drop handles it) but is the house style here.
- **Fixture leakage is a non-issue.** Each test owns its own container; containers are dropped at test end via `ContainerAsync`'s Drop impl. No shared state with other tests.
- **CONTEXT.md locked "no row correctness / EXPLAIN assertions."** Keep the test strictly to `assert!(result.is_ok(), ...)`. The bug is a SQL-syntax-level error; `Ok(_)` trivially proves the bug is gone. Resist scope creep into "and also assert len == 1" — a row-count assertion entangles this regression test with unrelated ORDER BY / filter semantics that aren't part of the bug.

## RESEARCH COMPLETE

**File:** `/Users/Robert/Code/public/cronduit/.planning/quick/260421-nn3-fix-get-dashboard-jobs-postgres-j-enable/260421-nn3-RESEARCH.md`
