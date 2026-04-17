//! Phase 11 migration integration tests (DB-09, DB-10, DB-12).
//!
//! Wave-0 stubs. Each `#[ignore]` test is filled in by its owning plan:
//!   - migration_01_*   — Plan 11-02  (COMPLETE)
//!   - migration_02_*   — Plan 11-03  (ACTIVE)
//!   - migration_03_*   — Plan 11-04

// Wave-0 stubs: each #[ignore] test has an `assert!(true, "stub — see Plan ...")`
// body so the owning plan is recorded inline and the files compile. Owners
// replace the assertion with real logic when they land.
#![allow(clippy::assertions_on_constants)]

mod common;

use std::io;
use std::sync::{Arc, Mutex};

use common::v11_fixtures::{
    seed_null_runs, seed_test_job, setup_sqlite_before_file3_migrations,
    setup_sqlite_with_phase11_migrations,
};
use cronduit::db::DbPool;
use cronduit::db::migrate_backfill;
use cronduit::db::queries::{self, PoolRef};
use tracing::instrument::WithSubscriber;
use tracing_subscriber::fmt::MakeWriter;

/// Drops the `_v11_backfill_done` sentinel so subsequent calls to the
/// orchestrator can execute. Necessary because `setup_sqlite_with_phase11_migrations`
/// runs `DbPool::migrate()` which already executes the orchestrator and marks
/// the sentinel on a fresh (empty) database.
async fn reset_sentinel(pool: &DbPool) {
    let sqlite_pool = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only fixture"),
    };
    sqlx::query("DROP TABLE IF EXISTS _v11_backfill_done")
        .execute(sqlite_pool)
        .await
        .expect("drop sentinel");
}

/// Tracing capture writer used by migration_02_logs_progress.
#[derive(Clone, Default)]
struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

impl io::Write for CapturedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for CapturedWriter {
    type Writer = Self;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[tokio::test]
async fn migration_01_add_nullable_columns() {
    // File 1 asserts the **nullable** column shape — so we use the
    // pre-file-3 fixture (applies files 0, 1, 2 but NOT file 3's NOT NULL
    // tightening). Post-file-3 the nullable assertion at line `jrn.2, 0`
    // would fail because the constraint is now NOT NULL.
    let pool = setup_sqlite_before_file3_migrations().await;
    let sqlite_pool = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };

    // PRAGMA table_info returns (cid, name, type, notnull, dflt_value, pk)
    let jobs_info: Vec<(String, String, i64, Option<String>)> =
        sqlx::query_as("SELECT name, type, \"notnull\", dflt_value FROM pragma_table_info('jobs')")
            .fetch_all(sqlite_pool)
            .await
            .expect("pragma jobs");

    let nrn = jobs_info
        .iter()
        .find(|r| r.0 == "next_run_number")
        .expect("jobs.next_run_number exists");
    assert_eq!(nrn.1.to_uppercase(), "INTEGER", "next_run_number type");
    assert_eq!(nrn.2, 1, "next_run_number must be NOT NULL");
    assert_eq!(nrn.3.as_deref(), Some("1"), "next_run_number DEFAULT 1");

    let job_runs_info: Vec<(String, String, i64, Option<String>)> = sqlx::query_as(
        "SELECT name, type, \"notnull\", dflt_value FROM pragma_table_info('job_runs')",
    )
    .fetch_all(sqlite_pool)
    .await
    .expect("pragma job_runs");

    let jrn = job_runs_info
        .iter()
        .find(|r| r.0 == "job_run_number")
        .expect("job_runs.job_run_number exists");
    assert_eq!(jrn.1.to_uppercase(), "INTEGER", "job_run_number type");
    assert_eq!(
        jrn.2, 0,
        "job_run_number must be nullable in file 1 (file 3 tightens)"
    );
}

#[tokio::test]
async fn migration_01_idempotent() {
    let pool = setup_sqlite_with_phase11_migrations().await;
    // Second migrate() is a no-op because sqlx records applied migrations.
    pool.migrate().await.expect("re-migrate is a no-op");
    // Verify columns still exist after the second run.
    let sqlite_pool = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('jobs') WHERE name = 'next_run_number'",
    )
    .fetch_one(sqlite_pool)
    .await
    .expect("count next_run_number column");
    assert_eq!(count, 1);
}

// ── Plan 11-03 migration_02_* (Rust-side backfill orchestrator) ──────────

#[tokio::test]
async fn migration_02_backfill_completes() {
    // Backfill tests need the column NULLABLE so `seed_null_runs` can insert
    // NULL job_run_number rows for the orchestrator to fill — use the
    // pre-file-3 fixture.
    let pool = setup_sqlite_before_file3_migrations().await;
    reset_sentinel(&pool).await;

    // Seed 3 jobs with 10, 8, 7 NULL job_run_number rows respectively.
    let j1 = seed_test_job(&pool, "job-one").await;
    let j2 = seed_test_job(&pool, "job-two").await;
    let j3 = seed_test_job(&pool, "job-three").await;
    seed_null_runs(&pool, j1, 10).await;
    seed_null_runs(&pool, j2, 8).await;
    seed_null_runs(&pool, j3, 7).await;

    migrate_backfill::backfill_job_run_number(&pool)
        .await
        .expect("backfill");

    assert_eq!(
        queries::count_job_runs_with_null_run_number(&pool)
            .await
            .unwrap(),
        0,
        "no NULL job_run_number rows remain after backfill"
    );

    let sqlite_pool = match pool.reader() {
        PoolRef::Sqlite(p) => p,
        _ => unreachable!(),
    };
    for (job_id, expected_n) in [(j1, 10i64), (j2, 8), (j3, 7)] {
        let rows: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT id, job_run_number FROM job_runs WHERE job_id = ?1 ORDER BY id ASC",
        )
        .bind(job_id)
        .fetch_all(sqlite_pool)
        .await
        .expect("fetch job rows");
        assert_eq!(rows.len() as i64, expected_n);
        for (i, (_id, run_num)) in rows.iter().enumerate() {
            assert_eq!(
                *run_num,
                (i + 1) as i64,
                "per-job numbering 1..N by id ASC (job_id={job_id})"
            );
        }
    }
}

#[tokio::test]
async fn migration_02_logs_progress() {
    // Seed enough rows that the orchestrator must emit >= 2 batch INFO lines.
    // BATCH_SIZE = 10_000, so 25_000 rows → 3 batches.
    let captured = CapturedWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(captured.clone())
        .with_max_level(tracing::Level::INFO)
        .with_target(true)
        .with_ansi(false)
        .finish();

    // Pre-file-3 state — see migration_02_backfill_completes for rationale.
    let pool = setup_sqlite_before_file3_migrations().await;
    reset_sentinel(&pool).await;

    let j1 = seed_test_job(&pool, "log-job-one").await;
    let j2 = seed_test_job(&pool, "log-job-two").await;
    let j3 = seed_test_job(&pool, "log-job-three").await;
    // 25k total — small enough to keep the test fast (~1s on SQLite in-memory)
    // but large enough to guarantee 3 batches.
    seed_null_runs(&pool, j1, 9_000).await;
    seed_null_runs(&pool, j2, 8_000).await;
    seed_null_runs(&pool, j3, 8_000).await;

    let fut = async {
        migrate_backfill::backfill_job_run_number(&pool)
            .await
            .expect("backfill");
    }
    .with_subscriber(subscriber);
    fut.await;

    let output = String::from_utf8(captured.0.lock().unwrap().clone())
        .expect("captured tracing output is utf8");

    assert!(
        output.contains("cronduit.migrate"),
        "expected target `cronduit.migrate` in captured output, got: {output}"
    );
    // Match the D-13 shape: at least 2 batch INFO lines.
    let batch_lines = output
        .lines()
        .filter(|l| l.contains("cronduit.migrate") && l.contains("job_run_number backfill: batch"))
        .count();
    assert!(
        batch_lines >= 2,
        "expected >= 2 batch INFO lines in captured output, got {batch_lines}; output: {output}"
    );
    // Final completion line.
    assert!(
        output.contains("job_run_number backfill: complete"),
        "expected final `complete` INFO line; output: {output}"
    );
    // D-13 structured-field checks on a batch line.
    assert!(
        output.contains("batch="),
        "expected batch=N field; output: {output}"
    );
    assert!(
        output.contains("rows_done="),
        "expected rows_done=N field; output: {output}"
    );
    assert!(
        output.contains("rows_total="),
        "expected rows_total=N field; output: {output}"
    );
    assert!(
        output.contains("elapsed_ms="),
        "expected elapsed_ms=N field; output: {output}"
    );
}

#[tokio::test]
async fn migration_02_resume_after_crash() {
    // Simulate a partial crash: call the batch helper ONCE to fill one chunk
    // without marking the sentinel; verify sentinel is still absent; then run
    // the full orchestrator and confirm remaining rows get backfilled without
    // double-counting (the `WHERE job_run_number IS NULL` guard makes the batch
    // restart clean).
    // Pre-file-3 state — see migration_02_backfill_completes for rationale.
    let pool = setup_sqlite_before_file3_migrations().await;
    reset_sentinel(&pool).await;

    let j1 = seed_test_job(&pool, "crash-job").await;
    // 20k rows — after first 10k-batch, 10k should remain NULL.
    seed_null_runs(&pool, j1, 20_000).await;

    // Partial-crash simulation: one batch update directly, no sentinel write.
    let rows_first_batch = queries::backfill_job_run_number_batch(&pool, 10_000)
        .await
        .expect("first batch");
    assert_eq!(rows_first_batch, 10_000, "first batch fills 10k rows");
    assert_eq!(
        queries::count_job_runs_with_null_run_number(&pool)
            .await
            .unwrap(),
        10_000,
        "10k rows remain NULL after partial-crash simulation"
    );
    assert!(
        !queries::v11_backfill_sentinel_exists(&pool).await.unwrap(),
        "sentinel MUST NOT be marked until orchestrator completes",
    );

    // Now run the full orchestrator — it should resume from the 10k NULL rows
    // and mark the sentinel when finished.
    migrate_backfill::backfill_job_run_number(&pool)
        .await
        .expect("orchestrator resume");

    assert_eq!(
        queries::count_job_runs_with_null_run_number(&pool)
            .await
            .unwrap(),
        0,
        "all rows backfilled after resume"
    );
    assert!(
        queries::v11_backfill_sentinel_exists(&pool).await.unwrap(),
        "sentinel must be present after successful orchestrator run",
    );

    // Verify NO double-counting: per-job numbering must still be contiguous 1..20000.
    let sqlite_pool = match pool.reader() {
        PoolRef::Sqlite(p) => p,
        _ => unreachable!(),
    };
    let nums: Vec<i64> =
        sqlx::query_scalar("SELECT job_run_number FROM job_runs WHERE job_id = ?1 ORDER BY id ASC")
            .bind(j1)
            .fetch_all(sqlite_pool)
            .await
            .expect("fetch run numbers");
    assert_eq!(nums.len(), 20_000);
    for (i, n) in nums.iter().enumerate() {
        assert_eq!(
            *n,
            (i + 1) as i64,
            "per-job numbering must be contiguous 1..20000 with no double-counting (i={i})"
        );
    }
}

#[tokio::test]
async fn migration_02_counter_reseed() {
    // Pre-file-3 state — see migration_02_backfill_completes for rationale.
    let pool = setup_sqlite_before_file3_migrations().await;
    reset_sentinel(&pool).await;

    let j1 = seed_test_job(&pool, "counter-job-one").await;
    let j2 = seed_test_job(&pool, "counter-job-two").await;
    let j3 = seed_test_job(&pool, "counter-no-runs").await;
    seed_null_runs(&pool, j1, 12).await;
    seed_null_runs(&pool, j2, 5).await;
    // j3 has zero runs — counter should be resynced to 1.

    migrate_backfill::backfill_job_run_number(&pool)
        .await
        .expect("backfill");

    let sqlite_pool = match pool.reader() {
        PoolRef::Sqlite(p) => p,
        _ => unreachable!(),
    };
    for (job_id, expected_next) in [(j1, 13i64), (j2, 6), (j3, 1)] {
        let next: i64 = sqlx::query_scalar("SELECT next_run_number FROM jobs WHERE id = ?1")
            .bind(job_id)
            .fetch_one(sqlite_pool)
            .await
            .expect("fetch next_run_number");
        assert_eq!(
            next, expected_next,
            "jobs.next_run_number = MAX(job_run_number)+1 per job (job_id={job_id})"
        );
    }
}

#[tokio::test]
async fn migration_02_row_number_order_by_id() {
    // Seed rows whose start_time is DESCENDING while id is ASCENDING. The
    // backfill must number by id ASC (insert order), NOT by start_time.
    // Pre-file-3 state — see migration_02_backfill_completes for rationale.
    let pool = setup_sqlite_before_file3_migrations().await;
    reset_sentinel(&pool).await;

    let j1 = seed_test_job(&pool, "order-job").await;

    let sqlite_pool = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    // Four rows — ids 1..=4 (ascending), start_time 2026-04-16T04:00..01:00 (descending).
    let mut row_ids: Vec<i64> = Vec::new();
    for hour in (1..=4).rev() {
        let ts = format!("2026-04-16T{hour:02}:00:00Z");
        let row = sqlx::query(
            "INSERT INTO job_runs (job_id, status, trigger, start_time) \
             VALUES (?1, 'success', 'schedule', ?2) RETURNING id",
        )
        .bind(j1)
        .bind(&ts)
        .fetch_one(sqlite_pool)
        .await
        .expect("insert reverse-time row");
        use sqlx::Row;
        row_ids.push(row.get::<i64, _>("id"));
    }

    migrate_backfill::backfill_job_run_number(&pool)
        .await
        .expect("backfill");

    // Assert numbering is by id ASC (insert order), NOT by start_time.
    let rows: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT id, job_run_number, start_time FROM job_runs \
         WHERE job_id = ?1 ORDER BY id ASC",
    )
    .bind(j1)
    .fetch_all(sqlite_pool)
    .await
    .expect("fetch rows");
    assert_eq!(rows.len(), 4);
    for (i, (_id, run_num, _ts)) in rows.iter().enumerate() {
        assert_eq!(
            *run_num,
            (i + 1) as i64,
            "job_run_number assigned by id ASC, not start_time (row i={i})"
        );
    }
    // Double-check start_time is actually descending — validates the test setup.
    assert!(
        rows[0].2 > rows[1].2,
        "setup sanity: start_time is descending while id is ascending"
    );
}

// ── Plan 11-04 migration_03_* (file 3: NOT NULL tightening + unique index) ──

#[tokio::test]
async fn migration_03_sqlite_table_rewrite() {
    // After file 3 runs via DbPool::migrate(), job_run_number is NOT NULL.
    // A direct INSERT with NULL job_run_number MUST fail.
    let pool = setup_sqlite_with_phase11_migrations().await;
    let sqlite_pool = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only"),
    };
    let job_id = seed_test_job(&pool, "nn-job").await;
    let now = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        "INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number) \
         VALUES (?1, 'running', 'manual', ?2, NULL)",
    )
    .bind(job_id)
    .bind(&now)
    .execute(sqlite_pool)
    .await;
    assert!(
        result.is_err(),
        "NULL job_run_number must be rejected after file 3"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.to_lowercase().contains("not null"),
        "expected NOT NULL constraint violation, got: {err_msg}"
    );
}

#[tokio::test]
async fn migration_03_sqlite_indexes_preserved() {
    // The 12-step table rewrite drops + recreates `job_runs`. All three indexes
    // (two carried forward + the new UNIQUE (job_id, job_run_number)) must exist.
    let pool = setup_sqlite_with_phase11_migrations().await;
    let sqlite_pool = match pool.reader() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only"),
    };
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='job_runs'",
    )
    .fetch_all(sqlite_pool)
    .await
    .expect("fetch index names");
    assert!(
        names.iter().any(|n| n == "idx_job_runs_job_id_start"),
        "expected idx_job_runs_job_id_start to survive rewrite; got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "idx_job_runs_start_time"),
        "expected idx_job_runs_start_time to survive rewrite; got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "idx_job_runs_job_id_run_number"),
        "expected file-3's UNIQUE idx_job_runs_job_id_run_number; got: {names:?}"
    );
}

#[cfg(feature = "integration")]
#[tokio::test]
async fn migration_03_postgres_not_null() {
    // Integration-gated: spin up a Postgres testcontainer, run DbPool::migrate,
    // and assert that a NULL-job_run_number insert is rejected. Follows the
    // testcontainer pattern used in tests/schema_parity.rs.
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::postgres::Postgres as PgImage;

    let node = PgImage::default()
        .start()
        .await
        .expect("start postgres testcontainer");
    let host = node.get_host().await.expect("host");
    let port = node.get_host_port_ipv4(5432).await.expect("port");
    let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");

    let pool = DbPool::connect(&url).await.expect("connect postgres");
    pool.migrate().await.expect("apply migrations");

    let pg_pool = match pool.writer() {
        PoolRef::Postgres(p) => p.clone(),
        _ => panic!("postgres-only"),
    };

    // Seed a job so the job_id FK is satisfied.
    let now = chrono::Utc::now().to_rfc3339();
    let job_id: i64 = sqlx::query_scalar(
        "INSERT INTO jobs \
         (name, schedule, resolved_schedule, job_type, config_json, config_hash, \
          timeout_secs, created_at, updated_at) \
         VALUES ('pg-nn-job', '* * * * *', '* * * * *', 'command', '{}', '0', 60, $1, $1) \
         RETURNING id",
    )
    .bind(&now)
    .fetch_one(&pg_pool)
    .await
    .expect("seed job");

    // Attempt an INSERT with NULL job_run_number — must be rejected.
    let result = sqlx::query(
        "INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number) \
         VALUES ($1, 'running', 'manual', $2, NULL)",
    )
    .bind(job_id)
    .bind(&now)
    .execute(&pg_pool)
    .await;
    assert!(
        result.is_err(),
        "NULL job_run_number must be rejected after file 3 on Postgres"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.to_lowercase().contains("not null") || err_msg.to_lowercase().contains("not-null"),
        "expected NOT NULL constraint violation on Postgres, got: {err_msg}"
    );

    pool.close().await;
}
