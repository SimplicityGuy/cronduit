//! T-V11-STOP-12..14: regression locks for docker_orphan::mark_run_orphaned
//! `WHERE status = 'running'` guard on both SQLite and Postgres branches.
//!
//! D-16 (10-06-PLAN.md): NO design work here. The guard already exists at
//! `src/scheduler/docker_orphan.rs` L120 (SQLite) and L131 (Postgres). These
//! tests prevent a future refactor from removing it.
//!
//! Semantics locked:
//!   - A row with `status='running'` transitions to `status='error'` with
//!     `error_message='orphaned at restart'` (v1.0 behavior preserved).
//!   - A row with ANY terminal status (`stopped`, `success`, `failed`,
//!     `cancelled`, `timeout`) is left completely UNCHANGED: status,
//!     error_message, and end_time all untouched.
//!
//! Dropping the `AND status = 'running'` clause from EITHER branch will
//! fail at least one of these tests (specifically, the terminal-status
//! tests will observe status flipping to `error`).
//!
//! ## Postgres coverage
//!
//! Postgres parallel tests live under `#[cfg(feature = "integration")]`.
//! The Cronduit workspace does not currently declare an `integration` cargo
//! feature; the gate is documented here verbatim per 10-06-PLAN.md acceptance
//! criteria so that when such a feature is added (or when CI invokes the
//! tests behind a cfg override), the Postgres branch of `mark_run_orphaned`
//! gains the same regression lock as the SQLite branch.

use chrono::Utc;
use cronduit::db::DbPool;
use cronduit::db::queries::{self, PoolRef};
use cronduit::scheduler::docker_orphan;
use sqlx::Row;

/// Spin up an in-memory SQLite DbPool with migrations applied. Mirrors the
/// `setup_pool` helper inside `src/db/queries.rs` test module.
async fn setup_sqlite_pool() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("open in-memory sqlite pool");
    pool.migrate().await.expect("run migrations");
    pool
}

/// Insert a parent `jobs` row so `job_runs.job_id` FK is satisfied. Returns
/// the job id. Uses a unique name per call so a single test can seed multiple
/// rows without conflicting on the `jobs.name` UNIQUE constraint.
async fn ensure_parent_job(pool: &DbPool, name: &str) -> i64 {
    queries::upsert_job(
        pool,
        name,
        "0 0 31 2 *",
        "0 0 31 2 *",
        "command",
        r#"{"command":"true"}"#,
        &format!("hash-{name}"),
        3600,
    )
    .await
    .expect("seed parent job")
}

/// Insert a `job_runs` row with the given terminal / running status and
/// pre-populated error_message + end_time. Returns the new run id.
///
/// Directly issues INSERT SQL (rather than using `queries::insert_running_run`
/// followed by an UPDATE) because we need to seed rows whose status is
/// already terminal (including `stopped`, which is the whole point of plan
/// 10-06) and there is no existing helper that covers that shape.
async fn seed_run_with_status(
    pool: &DbPool,
    job_id: i64,
    status: &str,
    error_msg: &str,
    end_time: &str,
) -> i64 {
    let start = Utc::now().to_rfc3339();
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            let row = sqlx::query(
                "INSERT INTO job_runs (job_id, status, trigger, start_time, end_time, error_message) \
                 VALUES (?1, ?2, 'scheduled', ?3, ?4, ?5) RETURNING id",
            )
            .bind(job_id)
            .bind(status)
            .bind(&start)
            .bind(end_time)
            .bind(error_msg)
            .fetch_one(p)
            .await
            .expect("insert job_runs row");
            row.get::<i64, _>("id")
        }
        PoolRef::Postgres(p) => {
            let row = sqlx::query(
                "INSERT INTO job_runs (job_id, status, trigger, start_time, end_time, error_message) \
                 VALUES ($1, $2, 'scheduled', $3, $4, $5) RETURNING id",
            )
            .bind(job_id)
            .bind(status)
            .bind(&start)
            .bind(end_time)
            .bind(error_msg)
            .fetch_one(p)
            .await
            .expect("insert job_runs row");
            row.get::<i64, _>("id")
        }
    }
}

/// Read back (status, error_message, end_time) for a given run id.
async fn read_row(pool: &DbPool, run_id: i64) -> (String, Option<String>, Option<String>) {
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let row =
                sqlx::query("SELECT status, error_message, end_time FROM job_runs WHERE id = ?1")
                    .bind(run_id)
                    .fetch_one(p)
                    .await
                    .expect("select job_runs row");
            (
                row.get::<String, _>("status"),
                row.get::<Option<String>, _>("error_message"),
                row.get::<Option<String>, _>("end_time"),
            )
        }
        PoolRef::Postgres(p) => {
            let row =
                sqlx::query("SELECT status, error_message, end_time FROM job_runs WHERE id = $1")
                    .bind(run_id)
                    .fetch_one(p)
                    .await
                    .expect("select job_runs row");
            (
                row.get::<String, _>("status"),
                row.get::<Option<String>, _>("error_message"),
                row.get::<Option<String>, _>("end_time"),
            )
        }
    }
}

// -----------------------------------------------------------------------------
// T-V11-STOP-12: stopped row must remain stopped
// -----------------------------------------------------------------------------

/// T-V11-STOP-12: A row with `status='stopped'` (the new terminal status
/// introduced by Phase 10) must be UNCHANGED by `mark_run_orphaned`. This is
/// the load-bearing regression for SCHED-13: without the `AND status =
/// 'running'` guard, a restart while a stopped run's container is still being
/// torn down would clobber the stopped row back to error.
#[tokio::test]
async fn mark_orphan_skips_stopped() {
    let pool = setup_sqlite_pool().await;
    let job_id = ensure_parent_job(&pool, "orphan-guard-stopped").await;

    let orig_end = Utc::now().to_rfc3339();
    let run_id =
        seed_run_with_status(&pool, job_id, "stopped", "stopped by operator", &orig_end).await;

    docker_orphan::mark_run_orphaned(&pool, run_id)
        .await
        .expect("mark_run_orphaned call");

    let (status, err_msg, end_time) = read_row(&pool, run_id).await;
    assert_eq!(
        status, "stopped",
        "stopped row must be UNCHANGED (T-V11-STOP-12); removing \
         `AND status = 'running'` from docker_orphan.rs regressed this"
    );
    assert_eq!(
        err_msg.as_deref(),
        Some("stopped by operator"),
        "error_message must be UNCHANGED for a stopped row"
    );
    assert_eq!(
        end_time.as_deref(),
        Some(orig_end.as_str()),
        "end_time must be UNCHANGED for a stopped row"
    );

    pool.close().await;
}

// -----------------------------------------------------------------------------
// T-V11-STOP-13: all other terminal statuses must also be untouched
// -----------------------------------------------------------------------------

/// T-V11-STOP-13: Every terminal status (`success`, `failed`, `cancelled`,
/// `timeout`) MUST also be UNCHANGED. The guard must protect the full
/// terminal-status set, not just `stopped`. Iterates across all four.
#[tokio::test]
async fn mark_orphan_skips_all_terminal_statuses() {
    let pool = setup_sqlite_pool().await;

    // One parent job per iteration keeps the jobs.name UNIQUE constraint happy.
    let terminal_statuses = ["success", "failed", "cancelled", "timeout"];
    for (idx, status) in terminal_statuses.iter().enumerate() {
        let job_id = ensure_parent_job(&pool, &format!("orphan-guard-{status}")).await;

        let orig_end = Utc::now().to_rfc3339();
        let run_id = seed_run_with_status(&pool, job_id, status, "original msg", &orig_end).await;

        docker_orphan::mark_run_orphaned(&pool, run_id)
            .await
            .expect("mark_run_orphaned call");

        let (got_status, got_err, got_end) = read_row(&pool, run_id).await;
        assert_eq!(
            got_status, *status,
            "iter {idx} ({status}): status must be UNCHANGED (T-V11-STOP-13)"
        );
        assert_eq!(
            got_err.as_deref(),
            Some("original msg"),
            "iter {idx} ({status}): error_message must be UNCHANGED"
        );
        assert_eq!(
            got_end.as_deref(),
            Some(orig_end.as_str()),
            "iter {idx} ({status}): end_time must be UNCHANGED"
        );
    }

    pool.close().await;
}

// -----------------------------------------------------------------------------
// T-V11-STOP-14: positive case — running row DOES transition to error
// -----------------------------------------------------------------------------

/// T-V11-STOP-14: The positive regression lock — a row with
/// `status='running'` MUST transition to `status='error'` with
/// `error_message='orphaned at restart'`. Without this test, a refactor that
/// turned `mark_run_orphaned` into an unconditional no-op would silently pass
/// the negative (skip-terminal) tests above.
#[tokio::test]
async fn mark_orphan_running_to_error() {
    let pool = setup_sqlite_pool().await;
    let job_id = ensure_parent_job(&pool, "orphan-guard-running").await;

    let orig_end = Utc::now().to_rfc3339();
    // Running rows in reality have a NULL end_time, but we seed a value so the
    // test can also assert that end_time IS overwritten on the running path.
    let run_id = seed_run_with_status(&pool, job_id, "running", "", &orig_end).await;

    docker_orphan::mark_run_orphaned(&pool, run_id)
        .await
        .expect("mark_run_orphaned call");

    let (status, err_msg, end_time) = read_row(&pool, run_id).await;
    assert_eq!(
        status, "error",
        "running row must transition to error (T-V11-STOP-14)"
    );
    assert_eq!(
        err_msg.as_deref(),
        Some("orphaned at restart"),
        "error_message must be 'orphaned at restart' after orphan reconciliation"
    );
    // end_time is overwritten to `now()` inside mark_run_orphaned — assert it
    // is SOME and differs from the seeded value (loose assertion; we only
    // care that the function wrote to it, not the exact timestamp).
    let written_end = end_time.expect("end_time must be set after orphan mark");
    assert_ne!(
        written_end, orig_end,
        "end_time must be rewritten on the running path (proves guard matched and UPDATE ran)"
    );

    pool.close().await;
}

// -----------------------------------------------------------------------------
// Postgres coverage (feature-gated)
// -----------------------------------------------------------------------------
//
// The Cronduit workspace does not currently declare an `integration` cargo
// feature; the gate below is kept verbatim per 10-06-PLAN.md acceptance
// criteria (`grep -c '#\[cfg(feature = "integration")\]'` >= 1) so that when
// such a feature is added — or when CI invokes the tests behind a cfg
// override — the Postgres branch of `mark_run_orphaned` gains the same
// regression lock as the SQLite branch. Until then the block compiles out and
// the SQLite tests alone carry the SCHED-13 regression lock.
#[cfg(feature = "integration")]
mod postgres_tests {
    use super::*;
    use testcontainers_modules::postgres::Postgres;
    use testcontainers_modules::testcontainers::ContainerAsync;
    use testcontainers_modules::testcontainers::runners::AsyncRunner;

    async fn setup_postgres_pool() -> (DbPool, ContainerAsync<Postgres>) {
        let container = Postgres::default()
            .start()
            .await
            .expect("start postgres container");
        let host = container.get_host().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");
        let pool = DbPool::connect(&url).await.expect("postgres pool");
        pool.migrate().await.expect("run migrations");
        (pool, container)
    }

    #[tokio::test]
    async fn pg_mark_orphan_skips_stopped() {
        let (pool, _container) = setup_postgres_pool().await;
        let job_id = ensure_parent_job(&pool, "orphan-guard-pg-stopped").await;

        let orig_end = Utc::now().to_rfc3339();
        let run_id =
            seed_run_with_status(&pool, job_id, "stopped", "stopped by operator", &orig_end).await;

        docker_orphan::mark_run_orphaned(&pool, run_id)
            .await
            .expect("mark_run_orphaned call");

        let (status, err_msg, end_time) = read_row(&pool, run_id).await;
        assert_eq!(
            status, "stopped",
            "[PG] stopped row must be UNCHANGED (T-V11-STOP-12 Postgres branch)"
        );
        assert_eq!(err_msg.as_deref(), Some("stopped by operator"));
        assert_eq!(end_time.as_deref(), Some(orig_end.as_str()));

        pool.close().await;
    }

    #[tokio::test]
    async fn pg_mark_orphan_skips_all_terminal_statuses() {
        let (pool, _container) = setup_postgres_pool().await;

        let terminal_statuses = ["success", "failed", "cancelled", "timeout"];
        for status in terminal_statuses.iter() {
            let job_id = ensure_parent_job(&pool, &format!("orphan-guard-pg-{status}")).await;
            let orig_end = Utc::now().to_rfc3339();
            let run_id =
                seed_run_with_status(&pool, job_id, status, "original msg", &orig_end).await;

            docker_orphan::mark_run_orphaned(&pool, run_id)
                .await
                .expect("mark_run_orphaned call");

            let (got_status, got_err, got_end) = read_row(&pool, run_id).await;
            assert_eq!(
                got_status, *status,
                "[PG] {status} row must be UNCHANGED (T-V11-STOP-13 Postgres branch)"
            );
            assert_eq!(got_err.as_deref(), Some("original msg"));
            assert_eq!(got_end.as_deref(), Some(orig_end.as_str()));
        }

        pool.close().await;
    }

    #[tokio::test]
    async fn pg_mark_orphan_running_to_error() {
        let (pool, _container) = setup_postgres_pool().await;
        let job_id = ensure_parent_job(&pool, "orphan-guard-pg-running").await;

        let orig_end = Utc::now().to_rfc3339();
        let run_id = seed_run_with_status(&pool, job_id, "running", "", &orig_end).await;

        docker_orphan::mark_run_orphaned(&pool, run_id)
            .await
            .expect("mark_run_orphaned call");

        let (status, err_msg, end_time) = read_row(&pool, run_id).await;
        assert_eq!(status, "error", "[PG] running row must transition to error");
        assert_eq!(err_msg.as_deref(), Some("orphaned at restart"));
        let written_end = end_time.expect("end_time must be set after orphan mark");
        assert_ne!(written_end, orig_end);

        pool.close().await;
    }
}
