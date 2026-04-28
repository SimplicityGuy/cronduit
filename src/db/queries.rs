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
    /// Phase 14 DB-14 tri-state override:
    /// - `None` = follow config `enabled` flag (no override)
    /// - `Some(0)` = force disabled (written by POST /api/jobs/bulk-toggle action=disable)
    /// - `Some(1)` = force enabled (reserved; v1.1 UI never writes this — defensive only)
    pub enabled_override: Option<i64>,
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
///
/// Phase 14 ERG-04 (symmetric clear): when a job leaves the config the row
/// loses BOTH the config-side `enabled` flag AND any UI-side `enabled_override`
/// — otherwise a previously bulk-disabled job could "stick" through reload.
pub async fn disable_missing_jobs(pool: &DbPool, active_names: &[String]) -> anyhow::Result<u64> {
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            if active_names.is_empty() {
                let result = sqlx::query(
                    "UPDATE jobs SET enabled = 0, enabled_override = NULL WHERE enabled = 1",
                )
                .execute(p)
                .await?;
                return Ok(result.rows_affected());
            }
            // SQLite doesn't support array binds; build a parameterized IN list.
            let placeholders: Vec<String> =
                (1..=active_names.len()).map(|i| format!("?{i}")).collect();
            let sql = format!(
                "UPDATE jobs SET enabled = 0, enabled_override = NULL WHERE enabled = 1 AND name NOT IN ({})",
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
                let result = sqlx::query(
                    "UPDATE jobs SET enabled = 0, enabled_override = NULL WHERE enabled = 1",
                )
                .execute(p)
                .await?;
                return Ok(result.rows_affected());
            }
            // Postgres supports ANY($1) with array bind.
            let result = sqlx::query(
                "UPDATE jobs SET enabled = 0, enabled_override = NULL WHERE enabled = 1 AND NOT (name = ANY($1))",
            )
            .bind(active_names)
            .execute(p)
            .await?;
            Ok(result.rows_affected())
        }
    }
}

/// Set `enabled_override` to a single value for multiple jobs in one UPDATE (Phase 14 DB-14).
///
/// `new_override`:
///   - `Some(0)` → force disabled (written by POST /api/jobs/bulk-toggle with action=disable)
///   - `Some(1)` → force enabled (reserved — v1.1 UI never writes this)
///   - `None`    → clear override (written by action=enable and the settings per-row Clear)
///
/// Returns the count of rows updated. Caller computes
/// `(not_found = ids.len() - rows_affected)` for partial-failure toast (D-12).
pub async fn bulk_set_override(
    pool: &DbPool,
    ids: &[i64],
    new_override: Option<i64>,
) -> anyhow::Result<u64> {
    if ids.is_empty() {
        return Ok(0);
    }
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            // SQLite: ?1 binds new_override; ids use ?2..?(N+1).
            let placeholders: Vec<String> = (2..=ids.len() + 1).map(|i| format!("?{i}")).collect();
            let sql = format!(
                "UPDATE jobs SET enabled_override = ?1 WHERE id IN ({})",
                placeholders.join(", ")
            );
            let mut q = sqlx::query(&sql).bind(new_override);
            for id in ids {
                q = q.bind(id);
            }
            let result = q.execute(p).await?;
            Ok(result.rows_affected())
        }
        PoolRef::Postgres(p) => {
            let result = sqlx::query("UPDATE jobs SET enabled_override = $1 WHERE id = ANY($2)")
                .bind(new_override)
                .bind(ids)
                .execute(p)
                .await?;
            Ok(result.rows_affected())
        }
    }
}

/// Fetch all enabled jobs from the database.
///
/// Phase 14 DB-14 (tri-state filter): a row counts as "enabled" only if its
/// config-side `enabled = 1` AND its UI-side override is not the explicit
/// disable sentinel (`Some(0)`). NULL or `Some(1)` both pass through.
pub async fn get_enabled_jobs(pool: &DbPool) -> anyhow::Result<Vec<DbJob>> {
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let rows = sqlx::query_as::<_, SqliteDbJobRow>(
                "SELECT id, name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, enabled_override, timeout_secs, created_at, updated_at FROM jobs WHERE enabled = 1 AND (enabled_override IS NULL OR enabled_override = 1)",
            )
            .fetch_all(p)
            .await?;
            Ok(rows.into_iter().map(|r| r.into()).collect())
        }
        PoolRef::Postgres(p) => {
            let rows = sqlx::query_as::<_, PgDbJobRow>(
                "SELECT id, name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, enabled_override, timeout_secs, created_at, updated_at FROM jobs WHERE enabled = 1 AND (enabled_override IS NULL OR enabled_override = 1)",
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
                "SELECT id, name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, enabled_override, timeout_secs, created_at, updated_at FROM jobs WHERE name = ?1",
            )
            .bind(name)
            .fetch_optional(p)
            .await?;
            Ok(row.map(|r| r.into()))
        }
        PoolRef::Postgres(p) => {
            let row = sqlx::query_as::<_, PgDbJobRow>(
                "SELECT id, name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, enabled_override, timeout_secs, created_at, updated_at FROM jobs WHERE name = $1",
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
    enabled_override: Option<i32>,
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
            enabled_override: r.enabled_override.map(|v| v as i64),
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
    // `jobs.enabled` is BIGINT (not BOOLEAN) on Postgres — see
    // migrations/postgres/20260410_000000_initial.up.sql L15-17. Decode as i64
    // and convert in the From impl, mirroring the SQLite (i32) pattern above.
    // Without this, every PgDbJobRow-backed query (get_enabled_jobs,
    // get_job_by_name, get_job_by_id, get_overridden_jobs) panics at decode
    // time with "Rust type `bool` is not compatible with SQL type `INT8`".
    enabled: i64,
    enabled_override: Option<i64>,
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
            enabled: r.enabled != 0,
            enabled_override: r.enabled_override,
            timeout_secs: r.timeout_secs,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// Insert a new job_runs row with status='running'. Returns the new run id.
///
/// Phase 11 DB-11: uses a two-statement transaction to atomically reserve a
/// per-job sequential `job_run_number` from `jobs.next_run_number` and insert
/// the new `job_runs` row with that number. The `UPDATE ... RETURNING
/// next_run_number - 1` pattern gives us the pre-increment value (the number
/// to assign to THIS row) while persisting the post-increment value for the
/// next caller.
///
/// Concurrency: on SQLite the writer pool has `max_connections = 1` so the
/// two statements are effectively serialized; on Postgres the tx block + row
/// lock from UPDATE guarantees no two callers can read the same counter.
/// Replaces the former `MAX + 1` race-prone pattern.
///
/// Phase 16 FCTX-04: `config_hash` is captured at fire time (BEFORE the executor
/// spawns) and bound into the new `job_runs.config_hash` column so a
/// reload-mid-fire still reflects the run's actual config rather than the latest.
pub async fn insert_running_run(
    pool: &DbPool,
    job_id: i64,
    trigger: &str,
    config_hash: &str, // Phase 16 FCTX-04
) -> anyhow::Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();

    match pool.writer() {
        PoolRef::Sqlite(p) => {
            let mut tx = p.begin().await?;
            let reserved: i64 = sqlx::query_scalar(
                "UPDATE jobs SET next_run_number = next_run_number + 1 \
                 WHERE id = ?1 RETURNING next_run_number - 1",
            )
            .bind(job_id)
            .fetch_one(&mut *tx)
            .await?;

            let run_id: i64 = sqlx::query_scalar(
                "INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number, config_hash) \
                 VALUES (?1, 'running', ?2, ?3, ?4, ?5) RETURNING id",
            )
            .bind(job_id)
            .bind(trigger)
            .bind(&now)
            .bind(reserved)
            .bind(config_hash) // Phase 16 FCTX-04: NEW bind, position ?5
            .fetch_one(&mut *tx)
            .await?;

            tx.commit().await?;
            Ok(run_id)
        }
        PoolRef::Postgres(p) => {
            let mut tx = p.begin().await?;
            let reserved: i64 = sqlx::query_scalar(
                "UPDATE jobs SET next_run_number = next_run_number + 1 \
                 WHERE id = $1 RETURNING next_run_number - 1",
            )
            .bind(job_id)
            .fetch_one(&mut *tx)
            .await?;

            let run_id: i64 = sqlx::query_scalar(
                "INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number, config_hash) \
                 VALUES ($1, 'running', $2, $3, $4, $5) RETURNING id",
            )
            .bind(job_id)
            .bind(trigger)
            .bind(&now)
            .bind(reserved)
            .bind(config_hash) // Phase 16 FCTX-04: NEW bind, position $5
            .fetch_one(&mut *tx)
            .await?;

            tx.commit().await?;
            Ok(run_id)
        }
    }
}

/// Finalize a job run by updating its status, exit_code, end_time, duration_ms, error_message, container_id, and image_digest.
/// Phase 16 FOUND-14: image_digest captured from `inspect_container` post-start; NULL for command/script jobs.
///
/// `#[allow(clippy::too_many_arguments)]`: the 8-arg shape mirrors the
/// `job_runs` row's terminal write surface (status, exit_code, end_time,
/// duration_ms, error_message, container_id, image_digest). Bundling these
/// into a struct would re-marshal data that is already in scope at every
/// caller; the param list IS the schema. Phase 16 FOUND-14 widened from 7
/// to 8 to add `image_digest` alongside `container_id`.
#[allow(clippy::too_many_arguments)]
pub async fn finalize_run(
    pool: &DbPool,
    run_id: i64,
    status: &str,
    exit_code: Option<i32>,
    start_instant: tokio::time::Instant,
    error_message: Option<&str>,
    container_id: Option<&str>,
    image_digest: Option<&str>, // Phase 16 FOUND-14
) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let duration_ms = start_instant.elapsed().as_millis().min(i64::MAX as u128) as i64;

    match pool.writer() {
        PoolRef::Sqlite(p) => {
            sqlx::query(
                "UPDATE job_runs SET status = ?1, exit_code = ?2, end_time = ?3, duration_ms = ?4, error_message = ?5, container_id = ?6, image_digest = ?7 WHERE id = ?8",
            )
            .bind(status)
            .bind(exit_code)
            .bind(&now)
            .bind(duration_ms)
            .bind(error_message)
            .bind(container_id)
            .bind(image_digest) // Phase 16 FOUND-14: NEW bind, position ?7
            .bind(run_id)
            .execute(p)
            .await?;
        }
        PoolRef::Postgres(p) => {
            sqlx::query(
                "UPDATE job_runs SET status = $1, exit_code = $2, end_time = $3, duration_ms = $4, error_message = $5, container_id = $6, image_digest = $7 WHERE id = $8",
            )
            .bind(status)
            .bind(exit_code)
            .bind(&now)
            .bind(duration_ms)
            .bind(error_message)
            .bind(container_id)
            .bind(image_digest) // Phase 16 FOUND-14: NEW bind, position $7
            .bind(run_id)
            .execute(p)
            .await?;
        }
    }

    Ok(())
}

/// Insert a batch of log lines into job_logs and return the persisted ids.
///
/// Each tuple is `(stream, ts, line)`. The returned `Vec<i64>` contains the
/// `job_logs.id` of each inserted row in the same order as the input slice.
///
/// Phase 11 D-01 / UI-20 (Option A): uses per-line `INSERT ... RETURNING id`
/// inside a single transaction so callers (`log_writer_task`) can zip the ids
/// with the input batch and broadcast `LogLine { id: Some(id), .. }`. The
/// single-tx discipline preserves the D-03 throughput contract: exactly one
/// `tx.begin()` + one `tx.commit()` per call, no per-line fsync. The
/// `RETURNING id` shape mirrors `insert_running_run` (L298-351).
pub async fn insert_log_batch(
    pool: &DbPool,
    run_id: i64,
    lines: &[(String, String, String)],
) -> anyhow::Result<Vec<i64>> {
    if lines.is_empty() {
        return Ok(Vec::new());
    }

    let mut ids = Vec::with_capacity(lines.len());

    match pool.writer() {
        PoolRef::Sqlite(p) => {
            let mut tx = p.begin().await?;
            for (stream, ts, line) in lines {
                let id: i64 = sqlx::query_scalar(
                    "INSERT INTO job_logs (run_id, stream, ts, line) \
                     VALUES (?1, ?2, ?3, ?4) RETURNING id",
                )
                .bind(run_id)
                .bind(stream)
                .bind(ts)
                .bind(line)
                .fetch_one(&mut *tx)
                .await?;
                ids.push(id);
            }
            tx.commit().await?;
        }
        PoolRef::Postgres(p) => {
            let mut tx = p.begin().await?;
            for (stream, ts, line) in lines {
                let id: i64 = sqlx::query_scalar(
                    "INSERT INTO job_logs (run_id, stream, ts, line) \
                     VALUES ($1, $2, $3, $4) RETURNING id",
                )
                .bind(run_id)
                .bind(stream)
                .bind(ts)
                .bind(line)
                .fetch_one(&mut *tx)
                .await?;
                ids.push(id);
            }
            tx.commit().await?;
        }
    }

    Ok(ids)
}

// ── Dashboard / UI query types ───────────────────────────────────────────

/// A row for the dashboard view: job + its most recent run.
#[derive(Debug, Clone)]
pub struct DashboardJob {
    pub id: i64,
    pub name: String,
    pub schedule: String,
    pub resolved_schedule: String,
    pub job_type: String,
    pub timeout_secs: i64,
    pub last_status: Option<String>,
    pub last_run_time: Option<String>,
    pub last_trigger: Option<String>,
    /// Phase 14 DB-14 tri-state override (None = config-only; Some(0) = forced disabled;
    /// Some(1) = forced enabled — defensive only). Carried for downstream view rendering
    /// (Plan 05 surfaces this on the dashboard chrome).
    pub enabled_override: Option<i64>,
}

/// A row from job_runs for the run history view.
#[derive(Debug, Clone)]
pub struct DbRun {
    pub id: i64,
    pub job_id: i64,
    /// Per-job sequential run number (Phase 11 DB-11). Starts at 1, increments
    /// atomically via `insert_running_run`'s counter transaction.
    pub job_run_number: i64,
    pub status: String,
    pub trigger: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub duration_ms: Option<i64>,
    pub exit_code: Option<i32>,
    pub error_message: Option<String>,
    /// Phase 16 FOUND-14: image digest from post-start `inspect_container`. NULL for
    /// command/script jobs (no image), pre-v1.2 docker rows (capture site landed in v1.2).
    pub image_digest: Option<String>,
    /// Phase 16 FCTX-04: per-run config_hash captured at fire time by
    /// `insert_running_run`. NULL for pre-v1.2 rows whose backfill found no matching
    /// `jobs.config_hash`. See migration `*_000007_config_hash_backfill.up.sql` for
    /// the BACKFILL_CUTOFF_RFC3339 marker (D-03).
    pub config_hash: Option<String>,
}

/// A row from job_runs with the associated job name (for run detail page).
#[derive(Debug, Clone)]
pub struct DbRunDetail {
    pub id: i64,
    pub job_id: i64,
    /// Per-job sequential run number (Phase 11 DB-11). Mirrors `DbRun::job_run_number`.
    pub job_run_number: i64,
    pub job_name: String,
    pub status: String,
    pub trigger: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub duration_ms: Option<i64>,
    pub exit_code: Option<i32>,
    pub error_message: Option<String>,
    /// Phase 16 FOUND-14: image digest from post-start `inspect_container`. NULL for
    /// command/script jobs (no image), pre-v1.2 docker rows (capture site landed in v1.2).
    pub image_digest: Option<String>,
    /// Phase 16 FCTX-04: per-run config_hash captured at fire time by
    /// `insert_running_run`. NULL for pre-v1.2 rows whose backfill found no matching
    /// `jobs.config_hash`. See migration `*_000007_config_hash_backfill.up.sql` for
    /// the BACKFILL_CUTOFF_RFC3339 marker (D-03).
    pub config_hash: Option<String>,
}

/// Phase 16 FCTX-07: failure-context query result. Returned by
/// `get_failure_context(job_id)`; consumed by the Phase 18 webhook payload
/// (WH-09) and the Phase 21 failure-context UI panel (FCTX-01..06).
///
/// `streak_position` is computed Rust-side from `consecutive_failures`
/// (D-06): consecutive_failures == 1 -> "first_failure"; > 1 -> "ongoing";
/// == 0 -> caller should not be calling this (the run isn't a failure).
///
/// `last_success_*` fields are NULL when the job has never succeeded
/// (the LEFT JOIN ON 1=1 returns one row with NULL last_success columns).
#[derive(Debug, Clone)]
pub struct FailureContext {
    /// Number of failed/timeout/error runs since the last success (or all
    /// failure-status runs if the job has never succeeded).
    // Phase 18+ consumes (webhook payload WH-09 + Phase 21 FCTX UI panel).
    #[allow(dead_code)]
    pub consecutive_failures: i64,
    /// Run ID of the most recent successful run, or None if the job has
    /// never succeeded.
    // Phase 18+ consumes (webhook payload WH-09 + Phase 21 FCTX UI panel).
    #[allow(dead_code)]
    pub last_success_run_id: Option<i64>,
    /// Image digest of the most recent successful run, or None if no
    /// success exists or the success was a non-docker job.
    // Phase 18+ consumes (webhook payload WH-09 + Phase 21 FCTX UI panel).
    #[allow(dead_code)]
    pub last_success_image_digest: Option<String>,
    /// Config hash of the most recent successful run, or None if no
    /// success exists or the success row was pre-v1.2 with no backfill match.
    // Phase 18+ consumes (webhook payload WH-09 + Phase 21 FCTX UI panel).
    #[allow(dead_code)]
    pub last_success_config_hash: Option<String>,
}

/// Phase 16 FCTX-07: single-query helper for failure-context computation.
///
/// Returns a `FailureContext` containing:
/// - `consecutive_failures`: count of failed/timeout/error runs since the
///   most recent success (or count of all failure-status runs if the job
///   has never succeeded).
/// - `last_success_run_id` + `last_success_image_digest` +
///   `last_success_config_hash`: metadata of the most recent success
///   (NULL fields when the job has never succeeded).
///
/// Implementation uses two CTEs (`last_success` LIMIT 1 + `streak` count)
/// joined via `LEFT JOIN ... ON 1=1` so a single fetch_one returns one row
/// even when no success exists. Both CTE arms hit
/// `idx_job_runs_job_id_start (job_id, start_time DESC)` -- verified by
/// the EXPLAIN tests in Plan 16-06.
///
/// Standard SQL only (D-15 -- no percentile_cont, no FILTER, no window
/// functions). Epoch sentinel `'1970-01-01T00:00:00Z'` used in COALESCE
/// to handle the never-succeeded case; matches the start_time RFC3339 TEXT
/// convention from the initial migration so lexicographic comparison is
/// portable across SQLite and Postgres.
#[allow(dead_code)] // Phase 18+ consumes (webhook payload WH-09 + Phase 21 FCTX UI panel).
pub async fn get_failure_context(
    pool: &DbPool,
    job_id: i64,
) -> anyhow::Result<FailureContext> {
    let sql_sqlite = r#"
        WITH last_success AS (
            SELECT id AS run_id, image_digest, config_hash, start_time
              FROM job_runs
             WHERE job_id = ?1 AND status = 'success'
             ORDER BY start_time DESC
             LIMIT 1
        ),
        streak AS (
            SELECT COUNT(*) AS consecutive_failures
              FROM job_runs
             WHERE job_id = ?1
               AND status IN ('failed', 'timeout', 'error')
               AND start_time > COALESCE(
                     (SELECT start_time FROM last_success),
                     '1970-01-01T00:00:00Z'
                   )
        )
        SELECT
            streak.consecutive_failures,
            last_success.run_id        AS last_success_run_id,
            last_success.image_digest  AS last_success_image_digest,
            last_success.config_hash   AS last_success_config_hash
          FROM streak
          LEFT JOIN last_success ON 1=1
    "#;
    let sql_postgres = r#"
        WITH last_success AS (
            SELECT id AS run_id, image_digest, config_hash, start_time
              FROM job_runs
             WHERE job_id = $1 AND status = 'success'
             ORDER BY start_time DESC
             LIMIT 1
        ),
        streak AS (
            SELECT COUNT(*) AS consecutive_failures
              FROM job_runs
             WHERE job_id = $1
               AND status IN ('failed', 'timeout', 'error')
               AND start_time > COALESCE(
                     (SELECT start_time FROM last_success),
                     '1970-01-01T00:00:00Z'
                   )
        )
        SELECT
            streak.consecutive_failures,
            last_success.run_id        AS last_success_run_id,
            last_success.image_digest  AS last_success_image_digest,
            last_success.config_hash   AS last_success_config_hash
          FROM streak
          LEFT JOIN last_success ON 1=1
    "#;

    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let row = sqlx::query(sql_sqlite).bind(job_id).fetch_one(p).await?;
            Ok(FailureContext {
                consecutive_failures: row.get("consecutive_failures"),
                last_success_run_id: row.get("last_success_run_id"),
                last_success_image_digest: row.get("last_success_image_digest"),
                last_success_config_hash: row.get("last_success_config_hash"),
            })
        }
        PoolRef::Postgres(p) => {
            let row = sqlx::query(sql_postgres).bind(job_id).fetch_one(p).await?;
            Ok(FailureContext {
                consecutive_failures: row.get("consecutive_failures"),
                last_success_run_id: row.get("last_success_run_id"),
                last_success_image_digest: row.get("last_success_image_digest"),
                last_success_config_hash: row.get("last_success_config_hash"),
            })
        }
    }
}

/// A row from job_logs.
#[derive(Debug, Clone)]
pub struct DbLogLine {
    pub id: i64,
    pub stream: String,
    pub ts: String,
    pub line: String,
}

/// Paginated result with total count.
#[derive(Debug, Clone)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub total: i64,
}

/// Fetch enabled jobs with their most recent run status for the dashboard.
///
/// T-03-04: Filter uses parameterized LIKE query. Sort uses whitelist match.
pub async fn get_dashboard_jobs(
    pool: &DbPool,
    filter: Option<&str>,
    sort: &str,
    order: &str,
) -> anyhow::Result<Vec<DashboardJob>> {
    // Build ORDER BY from whitelist — never interpolate user input into SQL.
    let order_clause = match (sort, order) {
        ("name", "desc") => "ORDER BY j.name DESC",
        ("name", _) => "ORDER BY j.name ASC",
        ("last_run", "desc") => "ORDER BY lr.start_time DESC NULLS LAST",
        ("last_run", _) => "ORDER BY lr.start_time ASC NULLS LAST",
        ("status", "desc") => "ORDER BY lr.status DESC NULLS LAST",
        ("status", _) => "ORDER BY lr.status ASC NULLS LAST",
        ("next_run", _) => "ORDER BY j.name ASC", // placeholder — actual next_run sort applied post-query in Rust
        (_, "desc") => "ORDER BY j.name DESC",
        _ => "ORDER BY j.name ASC",
    };

    let has_filter = filter.is_some_and(|f| !f.is_empty());

    let base_sql = if has_filter {
        format!(
            r#"SELECT j.id, j.name, j.schedule, j.resolved_schedule, j.job_type, j.timeout_secs, j.enabled_override,
                      lr.status AS last_status, lr.start_time AS last_run_time, lr.trigger AS last_trigger
               FROM jobs j
               LEFT JOIN (
                   SELECT job_id, status, start_time, trigger,
                          ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY start_time DESC) AS rn
                   FROM job_runs
               ) lr ON lr.job_id = j.id AND lr.rn = 1
               WHERE j.enabled = 1 AND LOWER(j.name) LIKE ?1
               {order_clause}"#
        )
    } else {
        format!(
            r#"SELECT j.id, j.name, j.schedule, j.resolved_schedule, j.job_type, j.timeout_secs, j.enabled_override,
                      lr.status AS last_status, lr.start_time AS last_run_time, lr.trigger AS last_trigger
               FROM jobs j
               LEFT JOIN (
                   SELECT job_id, status, start_time, trigger,
                          ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY start_time DESC) AS rn
                   FROM job_runs
               ) lr ON lr.job_id = j.id AND lr.rn = 1
               WHERE j.enabled = 1
               {order_clause}"#
        )
    };

    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let rows = if has_filter {
                let pattern = format!("%{}%", filter.unwrap().to_lowercase());
                sqlx::query(&base_sql).bind(pattern).fetch_all(p).await?
            } else {
                sqlx::query(&base_sql).fetch_all(p).await?
            };
            Ok(rows
                .into_iter()
                .map(|r| DashboardJob {
                    id: r.get("id"),
                    name: r.get("name"),
                    schedule: r.get("schedule"),
                    resolved_schedule: r.get("resolved_schedule"),
                    job_type: r.get("job_type"),
                    timeout_secs: r.get("timeout_secs"),
                    last_status: r.get("last_status"),
                    last_run_time: r.get("last_run_time"),
                    last_trigger: r.get("last_trigger"),
                    enabled_override: r.try_get("enabled_override").ok().flatten(),
                })
                .collect())
        }
        PoolRef::Postgres(p) => {
            // Postgres uses $1 instead of ?1
            let pg_sql = if has_filter {
                format!(
                    r#"SELECT j.id, j.name, j.schedule, j.resolved_schedule, j.job_type, j.timeout_secs, j.enabled_override,
                              lr.status AS last_status, lr.start_time AS last_run_time, lr.trigger AS last_trigger
                       FROM jobs j
                       LEFT JOIN (
                           SELECT job_id, status, start_time, trigger,
                                  ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY start_time DESC) AS rn
                           FROM job_runs
                       ) lr ON lr.job_id = j.id AND lr.rn = 1
                       WHERE j.enabled = 1 AND LOWER(j.name) LIKE $1
                       {order_clause}"#
                )
            } else {
                format!(
                    r#"SELECT j.id, j.name, j.schedule, j.resolved_schedule, j.job_type, j.timeout_secs, j.enabled_override,
                              lr.status AS last_status, lr.start_time AS last_run_time, lr.trigger AS last_trigger
                       FROM jobs j
                       LEFT JOIN (
                           SELECT job_id, status, start_time, trigger,
                                  ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY start_time DESC) AS rn
                           FROM job_runs
                       ) lr ON lr.job_id = j.id AND lr.rn = 1
                       WHERE j.enabled = 1
                       {order_clause}"#
                )
            };
            let rows = if has_filter {
                let pattern = format!("%{}%", filter.unwrap().to_lowercase());
                sqlx::query(&pg_sql).bind(pattern).fetch_all(p).await?
            } else {
                sqlx::query(&pg_sql).fetch_all(p).await?
            };
            Ok(rows
                .into_iter()
                .map(|r| DashboardJob {
                    id: r.get("id"),
                    name: r.get("name"),
                    schedule: r.get("schedule"),
                    resolved_schedule: r.get("resolved_schedule"),
                    job_type: r.get("job_type"),
                    timeout_secs: r.get("timeout_secs"),
                    last_status: r.get("last_status"),
                    last_run_time: r.get("last_run_time"),
                    last_trigger: r.get("last_trigger"),
                    enabled_override: r.try_get("enabled_override").ok().flatten(),
                })
                .collect())
        }
    }
}

/// Sparkline cell for one terminal job run (Phase 13 OBS-03).
///
/// Returned by [`get_dashboard_job_sparks`] — the handler buckets these by
/// `job_id` and folds the last 20 per job into status-colored cells + a
/// success-rate badge on the dashboard's Recent column.
#[derive(Debug, Clone)]
pub struct DashboardSparkRow {
    pub job_id: i64,
    pub id: i64,
    pub job_run_number: i64,
    pub status: String,
    pub duration_ms: Option<i64>,
    pub start_time: String,
    pub rn: i64,
}

/// Fetch the last 20 terminal runs per job in a single SQL query (OBS-03).
///
/// "Terminal" excludes `'running'` — only runs with status in
/// `(success, failed, timeout, cancelled, stopped)` are returned. Rows are
/// ordered by `job_id ASC, rn ASC` so the handler can bucket and reverse once
/// for oldest-to-newest cell rendering.
///
/// No N+1 — one query covers every job; the caller buckets by `job_id`.
pub async fn get_dashboard_job_sparks(pool: &DbPool) -> anyhow::Result<Vec<DashboardSparkRow>> {
    let sql = r#"
        SELECT job_id, id, job_run_number, status, duration_ms, start_time, rn
        FROM (
            SELECT job_id, id, job_run_number, status, duration_ms, start_time,
                   ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id DESC) AS rn
            FROM job_runs
            WHERE status IN ('success','failed','timeout','cancelled','stopped')
        ) t
        WHERE rn <= 20
        ORDER BY job_id ASC, rn ASC
    "#;

    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let rows = sqlx::query(sql).fetch_all(p).await?;
            Ok(rows
                .into_iter()
                .map(|r| DashboardSparkRow {
                    job_id: r.get("job_id"),
                    id: r.get("id"),
                    job_run_number: r.get("job_run_number"),
                    status: r.get("status"),
                    duration_ms: r.get("duration_ms"),
                    start_time: r.get("start_time"),
                    rn: r.get("rn"),
                })
                .collect())
        }
        PoolRef::Postgres(p) => {
            let rows = sqlx::query(sql).fetch_all(p).await?;
            Ok(rows
                .into_iter()
                .map(|r| DashboardSparkRow {
                    job_id: r.get("job_id"),
                    id: r.get("id"),
                    job_run_number: r.get("job_run_number"),
                    status: r.get("status"),
                    duration_ms: r.get("duration_ms"),
                    start_time: r.get("start_time"),
                    rn: r.get("rn"),
                })
                .collect())
        }
    }
}

/// Fetch the last N successful durations for a job, newest first (Phase 13 OBS-04, OBS-05).
///
/// Returns ONLY rows where `status = 'success'` AND `duration_ms IS NOT NULL`
/// (D-20: strict-success only — mixing failure/timeout durations skews p95).
///
/// Returns raw `Vec<u64>` samples — aggregation happens in Rust via
/// `src/web/stats.rs::percentile`. The return type (`Vec<u64>`, not `f64` or
/// `Option<u64>`) is the type-level enforcement of OBS-05 structural parity:
/// no SQL-native percentile (`percentile_cont`, `percentile_disc`) is used on
/// either backend.
pub async fn get_recent_successful_durations(
    pool: &DbPool,
    job_id: i64,
    limit: i64,
) -> anyhow::Result<Vec<u64>> {
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let rows = sqlx::query(
                "SELECT duration_ms FROM job_runs
                 WHERE job_id = ?1
                   AND status = 'success'
                   AND duration_ms IS NOT NULL
                 ORDER BY id DESC
                 LIMIT ?2",
            )
            .bind(job_id)
            .bind(limit)
            .fetch_all(p)
            .await?;
            Ok(rows
                .into_iter()
                .map(|r| r.get::<i64, _>("duration_ms") as u64)
                .collect())
        }
        PoolRef::Postgres(p) => {
            let rows = sqlx::query(
                "SELECT duration_ms FROM job_runs
                 WHERE job_id = $1
                   AND status = 'success'
                   AND duration_ms IS NOT NULL
                 ORDER BY id DESC
                 LIMIT $2",
            )
            .bind(job_id)
            .bind(limit)
            .fetch_all(p)
            .await?;
            Ok(rows
                .into_iter()
                .map(|r| r.get::<i64, _>("duration_ms") as u64)
                .collect())
        }
    }
}

/// A terminal or in-flight run for the `/timeline` gantt view (Phase 13 OBS-01, OBS-02).
///
/// Rendered as one bar; `end_time` is `None` iff `status == "running"`.
/// `start_time` and `end_time` are stored as `TEXT` on both backends (RFC3339
/// or `"YYYY-MM-DD HH:MM:SS"` fallback — see `format_relative_past` parser).
#[derive(Debug, Clone)]
pub struct TimelineRun {
    pub run_id: i64,
    pub job_id: i64,
    pub job_name: String,
    pub job_run_number: i64,
    pub status: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub duration_ms: Option<i64>,
}

/// Fetch up to 10 000 runs from enabled jobs whose `start_time >= window_start`
/// (Phase 13 OBS-01, OBS-02).
///
/// Filter is locked on `start_time` (not `end_time`) so SQLite / Postgres can
/// use `idx_job_runs_start_time` (Research Open Question #1 resolution per
/// Plan Task 3 Assumption A2). Semantics: "runs that started in the last
/// 24h/7d." Runs that started before the window but ended inside it are
/// intentionally excluded — homelab-scale 24h+ runs are an edge case, and the
/// `start_time` filter lets the query planner use the shipped index.
///
/// `LIMIT 10000` is a hard SQL literal (never parameterized) per OBS-02.
/// `ORDER BY j.name ASC, jr.start_time ASC` yields deterministic per-job row
/// order on the timeline (alphabetical lanes, chronological bars within lane).
pub async fn get_timeline_runs(
    pool: &DbPool,
    window_start_rfc3339: &str,
) -> anyhow::Result<Vec<TimelineRun>> {
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let rows = sqlx::query(
                r#"SELECT jr.id AS run_id,
                          jr.job_id,
                          j.name AS job_name,
                          jr.job_run_number,
                          jr.status,
                          jr.start_time,
                          jr.end_time,
                          jr.duration_ms
                   FROM job_runs jr
                   JOIN jobs j ON j.id = jr.job_id
                   WHERE j.enabled = 1
                     AND jr.start_time >= ?1
                   ORDER BY j.name ASC, jr.start_time ASC
                   LIMIT 10000"#,
            )
            .bind(window_start_rfc3339)
            .fetch_all(p)
            .await?;
            Ok(rows
                .into_iter()
                .map(|r| TimelineRun {
                    run_id: r.get("run_id"),
                    job_id: r.get("job_id"),
                    job_name: r.get("job_name"),
                    job_run_number: r.get("job_run_number"),
                    status: r.get("status"),
                    start_time: r.get("start_time"),
                    end_time: r.get("end_time"),
                    duration_ms: r.get("duration_ms"),
                })
                .collect())
        }
        PoolRef::Postgres(p) => {
            // `jobs.enabled` is BIGINT on Postgres (see migrations/postgres; the
            // schema_parity test normalizes BIGINT ↔ INTEGER to a shared INT64
            // token). Compare to the integer literal `1`, NOT `true` —
            // Postgres will raise `operator does not exist: bigint = boolean`
            // on the latter, and EXPLAIN will surface the same error.
            // (Plan 13-06 Task 1 Rule-1 auto-fix: discovered by the Postgres
            // EXPLAIN harness added in tests/v13_timeline_explain.rs.)
            let rows = sqlx::query(
                r#"SELECT jr.id AS run_id,
                          jr.job_id,
                          j.name AS job_name,
                          jr.job_run_number,
                          jr.status,
                          jr.start_time,
                          jr.end_time,
                          jr.duration_ms
                   FROM job_runs jr
                   JOIN jobs j ON j.id = jr.job_id
                   WHERE j.enabled = 1
                     AND jr.start_time >= $1
                   ORDER BY j.name ASC, jr.start_time ASC
                   LIMIT 10000"#,
            )
            .bind(window_start_rfc3339)
            .fetch_all(p)
            .await?;
            Ok(rows
                .into_iter()
                .map(|r| TimelineRun {
                    run_id: r.get("run_id"),
                    job_id: r.get("job_id"),
                    job_name: r.get("job_name"),
                    job_run_number: r.get("job_run_number"),
                    status: r.get("status"),
                    start_time: r.get("start_time"),
                    end_time: r.get("end_time"),
                    duration_ms: r.get("duration_ms"),
                })
                .collect())
        }
    }
}

/// Fetch a single job by id.
pub async fn get_job_by_id(pool: &DbPool, id: i64) -> anyhow::Result<Option<DbJob>> {
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let row = sqlx::query_as::<_, SqliteDbJobRow>(
                "SELECT id, name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, enabled_override, timeout_secs, created_at, updated_at FROM jobs WHERE id = ?1",
            )
            .bind(id)
            .fetch_optional(p)
            .await?;
            Ok(row.map(|r| r.into()))
        }
        PoolRef::Postgres(p) => {
            let row = sqlx::query_as::<_, PgDbJobRow>(
                "SELECT id, name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, enabled_override, timeout_secs, created_at, updated_at FROM jobs WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(p)
            .await?;
            Ok(row.map(|r| r.into()))
        }
    }
}

/// Fetch all jobs whose `enabled_override` is non-NULL, alphabetical by name.
///
/// Phase 14 ERG-03 / D-10b: powers the Settings page "Currently Overridden"
/// section so operators can see (and per-row clear) every job that has been
/// bulk-toggled away from its config-side `enabled` flag.
pub async fn get_overridden_jobs(pool: &DbPool) -> anyhow::Result<Vec<DbJob>> {
    const SELECT: &str = "SELECT id, name, schedule, resolved_schedule, job_type, \
        config_json, config_hash, enabled, enabled_override, timeout_secs, \
        created_at, updated_at FROM jobs WHERE enabled_override IS NOT NULL \
        ORDER BY name ASC";
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let rows = sqlx::query_as::<_, SqliteDbJobRow>(SELECT)
                .fetch_all(p)
                .await?;
            Ok(rows.into_iter().map(|r| r.into()).collect())
        }
        PoolRef::Postgres(p) => {
            let rows = sqlx::query_as::<_, PgDbJobRow>(SELECT).fetch_all(p).await?;
            Ok(rows.into_iter().map(|r| r.into()).collect())
        }
    }
}

/// Update the resolved schedule for a job (used by @random re-roll).
pub async fn update_resolved_schedule(
    pool: &DbPool,
    job_id: i64,
    resolved: &str,
) -> anyhow::Result<()> {
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            sqlx::query("UPDATE jobs SET resolved_schedule = ?1 WHERE id = ?2")
                .bind(resolved)
                .bind(job_id)
                .execute(p)
                .await?;
        }
        PoolRef::Postgres(p) => {
            sqlx::query("UPDATE jobs SET resolved_schedule = $1 WHERE id = $2")
                .bind(resolved)
                .bind(job_id)
                .execute(p)
                .await?;
        }
    }
    Ok(())
}

/// Fetch paginated run history for a job, ordered by start_time DESC.
pub async fn get_run_history(
    pool: &DbPool,
    job_id: i64,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Paginated<DbRun>> {
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let count_row = sqlx::query("SELECT COUNT(*) as cnt FROM job_runs WHERE job_id = ?1")
                .bind(job_id)
                .fetch_one(p)
                .await?;
            let total: i64 = count_row.get("cnt");

            let rows = sqlx::query(
                "SELECT id, job_id, job_run_number, status, trigger, start_time, end_time, duration_ms, exit_code, error_message, image_digest, config_hash FROM job_runs WHERE job_id = ?1 ORDER BY start_time DESC LIMIT ?2 OFFSET ?3",
            )
            .bind(job_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(p)
            .await?;

            let items = rows
                .into_iter()
                .map(|r| DbRun {
                    id: r.get("id"),
                    job_id: r.get("job_id"),
                    job_run_number: r.get("job_run_number"),
                    status: r.get("status"),
                    trigger: r.get("trigger"),
                    start_time: r.get("start_time"),
                    end_time: r.get("end_time"),
                    duration_ms: r.get("duration_ms"),
                    exit_code: r.get("exit_code"),
                    error_message: r.get("error_message"),
                    image_digest: r.get("image_digest"), // Phase 16 FOUND-14
                    config_hash: r.get("config_hash"),   // Phase 16 FCTX-04
                })
                .collect();

            Ok(Paginated { items, total })
        }
        PoolRef::Postgres(p) => {
            let count_row = sqlx::query("SELECT COUNT(*) as cnt FROM job_runs WHERE job_id = $1")
                .bind(job_id)
                .fetch_one(p)
                .await?;
            let total: i64 = count_row.get("cnt");

            let rows = sqlx::query(
                "SELECT id, job_id, job_run_number, status, trigger, start_time, end_time, duration_ms, exit_code, error_message, image_digest, config_hash FROM job_runs WHERE job_id = $1 ORDER BY start_time DESC LIMIT $2 OFFSET $3",
            )
            .bind(job_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(p)
            .await?;

            let items = rows
                .into_iter()
                .map(|r| DbRun {
                    id: r.get("id"),
                    job_id: r.get("job_id"),
                    job_run_number: r.get("job_run_number"),
                    status: r.get("status"),
                    trigger: r.get("trigger"),
                    start_time: r.get("start_time"),
                    end_time: r.get("end_time"),
                    duration_ms: r.get("duration_ms"),
                    exit_code: r.get("exit_code"),
                    error_message: r.get("error_message"),
                    image_digest: r.get("image_digest"), // Phase 16 FOUND-14
                    config_hash: r.get("config_hash"),   // Phase 16 FCTX-04
                })
                .collect();

            Ok(Paginated { items, total })
        }
    }
}

/// Fetch a single run by id with its associated job name.
pub async fn get_run_by_id(pool: &DbPool, run_id: i64) -> anyhow::Result<Option<DbRunDetail>> {
    let sql_sqlite = r#"
        SELECT r.id, r.job_id, r.job_run_number, j.name AS job_name, r.status, r.trigger,
               r.start_time, r.end_time, r.duration_ms, r.exit_code, r.error_message,
               r.image_digest, r.config_hash
        FROM job_runs r
        JOIN jobs j ON j.id = r.job_id
        WHERE r.id = ?1
    "#;
    let sql_postgres = r#"
        SELECT r.id, r.job_id, r.job_run_number, j.name AS job_name, r.status, r.trigger,
               r.start_time, r.end_time, r.duration_ms, r.exit_code, r.error_message,
               r.image_digest, r.config_hash
        FROM job_runs r
        JOIN jobs j ON j.id = r.job_id
        WHERE r.id = $1
    "#;

    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let row = sqlx::query(sql_sqlite)
                .bind(run_id)
                .fetch_optional(p)
                .await?;
            Ok(row.map(|r| DbRunDetail {
                id: r.get("id"),
                job_id: r.get("job_id"),
                job_run_number: r.get("job_run_number"),
                job_name: r.get("job_name"),
                status: r.get("status"),
                trigger: r.get("trigger"),
                start_time: r.get("start_time"),
                end_time: r.get("end_time"),
                duration_ms: r.get("duration_ms"),
                exit_code: r.get("exit_code"),
                error_message: r.get("error_message"),
                image_digest: r.get("image_digest"), // Phase 16 FOUND-14
                config_hash: r.get("config_hash"),   // Phase 16 FCTX-04
            }))
        }
        PoolRef::Postgres(p) => {
            let row = sqlx::query(sql_postgres)
                .bind(run_id)
                .fetch_optional(p)
                .await?;
            Ok(row.map(|r| DbRunDetail {
                id: r.get("id"),
                job_id: r.get("job_id"),
                job_run_number: r.get("job_run_number"),
                job_name: r.get("job_name"),
                status: r.get("status"),
                trigger: r.get("trigger"),
                start_time: r.get("start_time"),
                end_time: r.get("end_time"),
                duration_ms: r.get("duration_ms"),
                exit_code: r.get("exit_code"),
                error_message: r.get("error_message"),
                image_digest: r.get("image_digest"), // Phase 16 FOUND-14
                config_hash: r.get("config_hash"),   // Phase 16 FCTX-04
            }))
        }
    }
}

/// Fetch paginated log lines for a run, ordered by id DESC (most recent first).
pub async fn get_log_lines(
    pool: &DbPool,
    run_id: i64,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Paginated<DbLogLine>> {
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let count_row = sqlx::query("SELECT COUNT(*) as cnt FROM job_logs WHERE run_id = ?1")
                .bind(run_id)
                .fetch_one(p)
                .await?;
            let total: i64 = count_row.get("cnt");

            let rows = sqlx::query(
                "SELECT id, stream, ts, line FROM job_logs WHERE run_id = ?1 ORDER BY id ASC LIMIT ?2 OFFSET ?3",
            )
            .bind(run_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(p)
            .await?;

            let items = rows
                .into_iter()
                .map(|r| DbLogLine {
                    id: r.get("id"),
                    stream: r.get("stream"),
                    ts: r.get("ts"),
                    line: r.get("line"),
                })
                .collect();

            Ok(Paginated { items, total })
        }
        PoolRef::Postgres(p) => {
            let count_row = sqlx::query("SELECT COUNT(*) as cnt FROM job_logs WHERE run_id = $1")
                .bind(run_id)
                .fetch_one(p)
                .await?;
            let total: i64 = count_row.get("cnt");

            let rows = sqlx::query(
                "SELECT id, stream, ts, line FROM job_logs WHERE run_id = $1 ORDER BY id ASC LIMIT $2 OFFSET $3",
            )
            .bind(run_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(p)
            .await?;

            let items = rows
                .into_iter()
                .map(|r| DbLogLine {
                    id: r.get("id"),
                    stream: r.get("stream"),
                    ts: r.get("ts"),
                    line: r.get("line"),
                })
                .collect();

            Ok(Paginated { items, total })
        }
    }
}

// ── Retention pruner queries (DB-08) ────────────────────────────────────

/// Delete a batch of job_logs rows where the associated run ended before the cutoff.
/// Returns the number of rows deleted. Deletes logs BEFORE runs (FK safety).
pub async fn delete_old_logs_batch(
    pool: &DbPool,
    cutoff: &str,
    batch_size: i64,
) -> Result<i64, sqlx::Error> {
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            let result = sqlx::query(
                "DELETE FROM job_logs WHERE rowid IN (
                    SELECT jl.rowid FROM job_logs jl
                    INNER JOIN job_runs jr ON jl.run_id = jr.id
                    WHERE jr.end_time IS NOT NULL AND jr.end_time < ?1
                    LIMIT ?2
                )",
            )
            .bind(cutoff)
            .bind(batch_size)
            .execute(p)
            .await?;
            Ok(result.rows_affected() as i64)
        }
        PoolRef::Postgres(p) => {
            let result = sqlx::query(
                "DELETE FROM job_logs WHERE id IN (
                    SELECT jl.id FROM job_logs jl
                    INNER JOIN job_runs jr ON jl.run_id = jr.id
                    WHERE jr.end_time IS NOT NULL AND jr.end_time < $1
                    LIMIT $2
                )",
            )
            .bind(cutoff)
            .bind(batch_size)
            .execute(p)
            .await?;
            Ok(result.rows_affected() as i64)
        }
    }
}

/// Delete a batch of job_runs rows that ended before the cutoff and have no remaining logs.
pub async fn delete_old_runs_batch(
    pool: &DbPool,
    cutoff: &str,
    batch_size: i64,
) -> Result<i64, sqlx::Error> {
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            let result = sqlx::query(
                "DELETE FROM job_runs WHERE rowid IN (
                    SELECT jr.rowid FROM job_runs jr
                    WHERE jr.end_time IS NOT NULL AND jr.end_time < ?1
                    AND NOT EXISTS (SELECT 1 FROM job_logs jl WHERE jl.run_id = jr.id)
                    LIMIT ?2
                )",
            )
            .bind(cutoff)
            .bind(batch_size)
            .execute(p)
            .await?;
            Ok(result.rows_affected() as i64)
        }
        PoolRef::Postgres(p) => {
            let result = sqlx::query(
                "DELETE FROM job_runs WHERE id IN (
                    SELECT jr.id FROM job_runs jr
                    WHERE jr.end_time IS NOT NULL AND jr.end_time < $1
                    AND NOT EXISTS (SELECT 1 FROM job_logs jl WHERE jl.run_id = jr.id)
                    LIMIT $2
                )",
            )
            .bind(cutoff)
            .bind(batch_size)
            .execute(p)
            .await?;
            Ok(result.rows_affected() as i64)
        }
    }
}

/// Issue WAL checkpoint on SQLite to reclaim space after large deletes.
/// No-op on Postgres (auto-vacuum handles it).
pub async fn wal_checkpoint(pool: &DbPool) -> Result<(), sqlx::Error> {
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                .execute(p)
                .await?;
            Ok(())
        }
        PoolRef::Postgres(_) => Ok(()),
    }
}

// ── Phase 11 backfill helpers (DB-09/10/11/12) ───────────────────────────

/// Phase 11 backfill batch: update up to `batch_size` rows where
/// `job_run_number IS NULL`, assigning a per-job sequential number via
/// `ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id ASC)` so the result
/// is deterministic and idempotent across partial-crash re-runs.
///
/// Returns the number of rows actually updated. Zero rows means the backfill
/// is complete (or un-progressable, which is the same thing since the
/// `WHERE job_run_number IS NULL` guard re-applies).
///
/// Uses the portable `UPDATE ... WHERE id IN (SELECT ... LIMIT N)` form per
/// RESEARCH Open Question Q3 (RESOLVED): simpler than `UPDATE ... FROM ...`
/// and works identically on SQLite 3.33+ and Postgres 9.x+.
pub async fn backfill_job_run_number_batch(pool: &DbPool, batch_size: i64) -> anyhow::Result<i64> {
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            // SQLite 3.33+ `UPDATE ... FROM` — matches the Postgres arm.
            //
            // Earlier drafts used a correlated scalar subquery form, but
            // SQLite's optimizer pushes the outer `WHERE s.id = job_runs.id`
            // equality INTO the inner subquery, which shrinks the `FROM
            // job_runs WHERE job_run_number IS NULL` scan to a single row
            // and causes ROW_NUMBER() to always evaluate to 1. The FROM-join
            // form materializes the ROW_NUMBER() result as a derived table
            // BEFORE the join, so partitioning is preserved correctly.
            //
            // The LEFT JOIN on `prev` adds a per-job offset = MAX(job_run_number)
            // of already-backfilled rows. Without it, a partial-crash restart
            // (second batch) would re-number from 1, colliding with the first
            // batch's assignments (T-V11-RUNNUM-08 resume test regression).
            let result = sqlx::query(
                "UPDATE job_runs \
                 SET job_run_number = s.rn + COALESCE(prev.max_filled, 0) \
                 FROM ( \
                    SELECT id, job_id, \
                           ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id ASC) AS rn \
                    FROM job_runs \
                    WHERE job_run_number IS NULL \
                 ) AS s \
                 LEFT JOIN ( \
                    SELECT job_id, MAX(job_run_number) AS max_filled \
                    FROM job_runs \
                    WHERE job_run_number IS NOT NULL \
                    GROUP BY job_id \
                 ) AS prev ON prev.job_id = s.job_id \
                 WHERE job_runs.id = s.id \
                   AND job_runs.id IN ( \
                        SELECT id FROM job_runs \
                        WHERE job_run_number IS NULL \
                        ORDER BY id ASC \
                        LIMIT ?1 \
                   )",
            )
            .bind(batch_size)
            .execute(p)
            .await?;
            Ok(result.rows_affected() as i64)
        }
        PoolRef::Postgres(p) => {
            let result = sqlx::query(
                "UPDATE job_runs \
                 SET job_run_number = s.rn + COALESCE(prev.max_filled, 0) \
                 FROM ( \
                    SELECT id, job_id, \
                           ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id ASC) AS rn \
                    FROM job_runs \
                    WHERE job_run_number IS NULL \
                 ) s \
                 LEFT JOIN ( \
                    SELECT job_id, MAX(job_run_number) AS max_filled \
                    FROM job_runs \
                    WHERE job_run_number IS NOT NULL \
                    GROUP BY job_id \
                 ) AS prev ON prev.job_id = s.job_id \
                 WHERE job_runs.id = s.id \
                   AND job_runs.id IN ( \
                        SELECT id FROM job_runs \
                        WHERE job_run_number IS NULL \
                        ORDER BY id ASC \
                        LIMIT $1 \
                   )",
            )
            .bind(batch_size)
            .execute(p)
            .await?;
            Ok(result.rows_affected() as i64)
        }
    }
}

/// Phase 11 post-backfill resync: set `jobs.next_run_number` to one more
/// than the current max `job_run_number` per job. Idempotent by construction.
pub async fn resync_next_run_number(pool: &DbPool) -> anyhow::Result<()> {
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            sqlx::query(
                "UPDATE jobs \
                 SET next_run_number = COALESCE( \
                    (SELECT MAX(job_run_number) + 1 \
                     FROM job_runs \
                     WHERE job_runs.job_id = jobs.id), \
                    1 \
                 )",
            )
            .execute(p)
            .await?;
        }
        PoolRef::Postgres(p) => {
            sqlx::query(
                "UPDATE jobs \
                 SET next_run_number = COALESCE( \
                    (SELECT MAX(job_run_number) + 1 \
                     FROM job_runs \
                     WHERE job_runs.job_id = jobs.id), \
                    1 \
                 )",
            )
            .execute(p)
            .await?;
        }
    }
    Ok(())
}

/// Phase 11 DB-15 startup assertion support: counts rows where
/// `job_run_number IS NULL`. Must return 0 after migrations complete.
pub async fn count_job_runs_with_null_run_number(pool: &DbPool) -> anyhow::Result<i64> {
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let n: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM job_runs WHERE job_run_number IS NULL")
                    .fetch_one(p)
                    .await?;
            Ok(n)
        }
        PoolRef::Postgres(p) => {
            let n: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM job_runs WHERE job_run_number IS NULL")
                    .fetch_one(p)
                    .await?;
            Ok(n)
        }
    }
}

/// Phase 11 sentinel check: returns true iff `_v11_backfill_done` exists and
/// contains a row. O(1) fast-path for the orchestrator's re-run short-circuit.
pub async fn v11_backfill_sentinel_exists(pool: &DbPool) -> anyhow::Result<bool> {
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let table_exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type = 'table' AND name = '_v11_backfill_done'",
            )
            .fetch_one(p)
            .await?;
            if table_exists == 0 {
                return Ok(false);
            }
            let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _v11_backfill_done")
                .fetch_one(p)
                .await?;
            Ok(row_count > 0)
        }
        PoolRef::Postgres(p) => {
            let table_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS ( \
                    SELECT 1 FROM information_schema.tables \
                    WHERE table_name = '_v11_backfill_done' \
                 )",
            )
            .fetch_one(p)
            .await?;
            if !table_exists {
                return Ok(false);
            }
            let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _v11_backfill_done")
                .fetch_one(p)
                .await?;
            Ok(row_count > 0)
        }
    }
}

/// Phase 11 sentinel write: ensures `_v11_backfill_done` exists with one
/// marker row. Idempotent (CREATE TABLE IF NOT EXISTS + INSERT guarded by
/// a single-row unique primary key).
pub async fn v11_backfill_sentinel_mark_done(pool: &DbPool) -> anyhow::Result<()> {
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            sqlx::query(
                "CREATE TABLE IF NOT EXISTS _v11_backfill_done ( \
                    id INTEGER PRIMARY KEY CHECK (id = 1), \
                    finished_at TEXT NOT NULL \
                 )",
            )
            .execute(p)
            .await?;
            sqlx::query(
                "INSERT OR IGNORE INTO _v11_backfill_done (id, finished_at) VALUES (1, ?1)",
            )
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(p)
            .await?;
        }
        PoolRef::Postgres(p) => {
            sqlx::query(
                "CREATE TABLE IF NOT EXISTS _v11_backfill_done ( \
                    id SMALLINT PRIMARY KEY CHECK (id = 1), \
                    finished_at TEXT NOT NULL \
                 )",
            )
            .execute(p)
            .await?;
            sqlx::query(
                "INSERT INTO _v11_backfill_done (id, finished_at) VALUES (1, $1) \
                 ON CONFLICT (id) DO NOTHING",
            )
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(p)
            .await?;
        }
    }
    Ok(())
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
        upsert_job(
            &pool,
            "keep",
            "* * * * *",
            "* * * * *",
            "command",
            "{}",
            "h1",
            60,
        )
        .await
        .unwrap();
        upsert_job(
            &pool,
            "remove",
            "* * * * *",
            "* * * * *",
            "command",
            "{}",
            "h2",
            60,
        )
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
        upsert_job(
            &pool,
            "a",
            "* * * * *",
            "* * * * *",
            "command",
            "{}",
            "h1",
            60,
        )
        .await
        .unwrap();
        upsert_job(
            &pool,
            "b",
            "* * * * *",
            "* * * * *",
            "command",
            "{}",
            "h2",
            60,
        )
        .await
        .unwrap();

        let disabled = disable_missing_jobs(&pool, &[]).await.unwrap();
        assert_eq!(disabled, 2);
        pool.close().await;
    }

    #[tokio::test]
    async fn get_enabled_jobs_filters_disabled() {
        let pool = setup_pool().await;
        upsert_job(
            &pool,
            "enabled1",
            "* * * * *",
            "* * * * *",
            "command",
            "{}",
            "h1",
            60,
        )
        .await
        .unwrap();
        upsert_job(
            &pool,
            "enabled2",
            "* * * * *",
            "* * * * *",
            "command",
            "{}",
            "h2",
            60,
        )
        .await
        .unwrap();
        upsert_job(
            &pool,
            "disabled",
            "* * * * *",
            "* * * * *",
            "command",
            "{}",
            "h3",
            60,
        )
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

    #[tokio::test]
    async fn insert_running_run_creates_row() {
        let pool = setup_pool().await;
        let job_id = upsert_job(
            &pool,
            "test",
            "* * * * *",
            "* * * * *",
            "command",
            "{}",
            "h1",
            60,
        )
        .await
        .unwrap();

        let run_id = insert_running_run(&pool, job_id, "scheduled", "testhash")
            .await
            .unwrap();
        assert!(run_id > 0);

        // Verify the row was created with correct fields.
        match pool.reader() {
            PoolRef::Sqlite(p) => {
                let row =
                    sqlx::query("SELECT status, trigger, start_time FROM job_runs WHERE id = ?1")
                        .bind(run_id)
                        .fetch_one(p)
                        .await
                        .unwrap();
                let status: String = row.get("status");
                let trigger: String = row.get("trigger");
                let start_time: String = row.get("start_time");
                assert_eq!(status, "running");
                assert_eq!(trigger, "scheduled");
                assert!(!start_time.is_empty());
            }
            _ => unreachable!(),
        }
        pool.close().await;
    }

    #[tokio::test]
    async fn finalize_run_updates_row() {
        let pool = setup_pool().await;
        let job_id = upsert_job(
            &pool,
            "test",
            "* * * * *",
            "* * * * *",
            "command",
            "{}",
            "h1",
            60,
        )
        .await
        .unwrap();
        let run_id = insert_running_run(&pool, job_id, "scheduled", "testhash")
            .await
            .unwrap();

        let start = tokio::time::Instant::now();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        finalize_run(&pool, run_id, "success", Some(0), start, None, None, None)
            .await
            .unwrap();

        match pool.reader() {
            PoolRef::Sqlite(p) => {
                let row = sqlx::query(
                    "SELECT status, exit_code, end_time, duration_ms FROM job_runs WHERE id = ?1",
                )
                .bind(run_id)
                .fetch_one(p)
                .await
                .unwrap();
                let status: String = row.get("status");
                let exit_code: Option<i32> = row.get("exit_code");
                let end_time: Option<String> = row.get("end_time");
                let duration_ms: Option<i64> = row.get("duration_ms");
                assert_eq!(status, "success");
                assert_eq!(exit_code, Some(0));
                assert!(end_time.is_some());
                assert!(duration_ms.unwrap() >= 5);
            }
            _ => unreachable!(),
        }
        pool.close().await;
    }

    #[tokio::test]
    async fn insert_log_batch_inserts_lines() {
        let pool = setup_pool().await;
        let job_id = upsert_job(
            &pool,
            "test",
            "* * * * *",
            "* * * * *",
            "command",
            "{}",
            "h1",
            60,
        )
        .await
        .unwrap();
        let run_id = insert_running_run(&pool, job_id, "scheduled", "testhash")
            .await
            .unwrap();

        let lines = vec![
            (
                "stdout".to_string(),
                "2026-01-01T00:00:00Z".to_string(),
                "line1".to_string(),
            ),
            (
                "stderr".to_string(),
                "2026-01-01T00:00:01Z".to_string(),
                "line2".to_string(),
            ),
        ];
        insert_log_batch(&pool, run_id, &lines).await.unwrap();

        match pool.reader() {
            PoolRef::Sqlite(p) => {
                let rows = sqlx::query(
                    "SELECT stream, ts, line FROM job_logs WHERE run_id = ?1 ORDER BY id",
                )
                .bind(run_id)
                .fetch_all(p)
                .await
                .unwrap();
                assert_eq!(rows.len(), 2);
                let s0: String = rows[0].get("stream");
                let l0: String = rows[0].get("line");
                assert_eq!(s0, "stdout");
                assert_eq!(l0, "line1");
                let s1: String = rows[1].get("stream");
                let l1: String = rows[1].get("line");
                assert_eq!(s1, "stderr");
                assert_eq!(l1, "line2");
            }
            _ => unreachable!(),
        }
        pool.close().await;
    }

    // ── Helper for dashboard/UI query tests ──────────────────────────────

    async fn create_job_with_runs(pool: &DbPool, name: &str, schedule: &str) -> i64 {
        upsert_job(
            pool,
            name,
            schedule,
            schedule,
            "command",
            &format!(r#"{{"command":"echo {name}"}}"#),
            &format!("hash-{name}"),
            3600,
        )
        .await
        .unwrap()
    }

    async fn insert_run(pool: &DbPool, job_id: i64, status: &str, trigger: &str) -> i64 {
        let run_id = insert_running_run(pool, job_id, trigger, "testhash")
            .await
            .unwrap();
        if status != "running" {
            let start = tokio::time::Instant::now();
            finalize_run(pool, run_id, status, Some(0), start, None, None, None)
                .await
                .unwrap();
        }
        run_id
    }

    async fn insert_logs(pool: &DbPool, run_id: i64, count: usize) {
        let lines: Vec<(String, String, String)> = (0..count)
            .map(|i| {
                (
                    "stdout".to_string(),
                    format!("2026-01-01T00:00:{:02}Z", i % 60),
                    format!("log line {i}"),
                )
            })
            .collect();
        insert_log_batch(pool, run_id, &lines).await.unwrap();
    }

    // ── Dashboard query tests ────────────────────────────────────────────

    #[tokio::test]
    async fn dashboard_jobs_returns_enabled_with_last_run() {
        let pool = setup_pool().await;
        let job_id = create_job_with_runs(&pool, "test-job", "*/5 * * * *").await;
        insert_run(&pool, job_id, "success", "scheduled").await;
        // Small delay then insert another run — this should be the "last" one
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        insert_run(&pool, job_id, "failed", "manual").await;

        let jobs = get_dashboard_jobs(&pool, None, "name", "asc")
            .await
            .unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].name, "test-job");
        assert_eq!(jobs[0].last_status.as_deref(), Some("failed"));
        assert_eq!(jobs[0].last_trigger.as_deref(), Some("manual"));
        assert!(jobs[0].last_run_time.is_some());
        pool.close().await;
    }

    #[tokio::test]
    async fn dashboard_jobs_no_runs_returns_null_status() {
        let pool = setup_pool().await;
        create_job_with_runs(&pool, "no-runs-job", "*/5 * * * *").await;

        let jobs = get_dashboard_jobs(&pool, None, "name", "asc")
            .await
            .unwrap();
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].last_status.is_none());
        assert!(jobs[0].last_run_time.is_none());
        assert!(jobs[0].last_trigger.is_none());
        pool.close().await;
    }

    #[tokio::test]
    async fn dashboard_jobs_filter_by_name() {
        let pool = setup_pool().await;
        create_job_with_runs(&pool, "test-backup", "*/5 * * * *").await;
        create_job_with_runs(&pool, "test-sync", "*/10 * * * *").await;
        create_job_with_runs(&pool, "deploy-app", "0 0 * * *").await;

        let jobs = get_dashboard_jobs(&pool, Some("test"), "name", "asc")
            .await
            .unwrap();
        assert_eq!(jobs.len(), 2);
        let names: Vec<&str> = jobs.iter().map(|j| j.name.as_str()).collect();
        assert!(names.contains(&"test-backup"));
        assert!(names.contains(&"test-sync"));
        assert!(!names.contains(&"deploy-app"));
        pool.close().await;
    }

    #[tokio::test]
    async fn dashboard_jobs_filter_case_insensitive() {
        let pool = setup_pool().await;
        create_job_with_runs(&pool, "TestJob", "*/5 * * * *").await;
        create_job_with_runs(&pool, "other", "*/10 * * * *").await;

        let jobs = get_dashboard_jobs(&pool, Some("TESTJOB"), "name", "asc")
            .await
            .unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].name, "TestJob");
        pool.close().await;
    }

    #[tokio::test]
    async fn dashboard_jobs_sort_name_desc() {
        let pool = setup_pool().await;
        create_job_with_runs(&pool, "alpha", "*/5 * * * *").await;
        create_job_with_runs(&pool, "zulu", "*/10 * * * *").await;
        create_job_with_runs(&pool, "mike", "0 0 * * *").await;

        let jobs = get_dashboard_jobs(&pool, None, "name", "desc")
            .await
            .unwrap();
        assert_eq!(jobs.len(), 3);
        assert_eq!(jobs[0].name, "zulu");
        assert_eq!(jobs[1].name, "mike");
        assert_eq!(jobs[2].name, "alpha");
        pool.close().await;
    }

    // ── Run history tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn run_history_paginated() {
        let pool = setup_pool().await;
        let job_id = create_job_with_runs(&pool, "paginated-job", "*/5 * * * *").await;
        for _ in 0..5 {
            insert_run(&pool, job_id, "success", "scheduled").await;
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }

        let page1 = get_run_history(&pool, job_id, 2, 0).await.unwrap();
        assert_eq!(page1.total, 5);
        assert_eq!(page1.items.len(), 2);
        // Ordered by start_time DESC — first item should be most recent
        assert!(page1.items[0].start_time >= page1.items[1].start_time);

        let page2 = get_run_history(&pool, job_id, 2, 2).await.unwrap();
        assert_eq!(page2.total, 5);
        assert_eq!(page2.items.len(), 2);

        let page3 = get_run_history(&pool, job_id, 2, 4).await.unwrap();
        assert_eq!(page3.total, 5);
        assert_eq!(page3.items.len(), 1);
        pool.close().await;
    }

    #[tokio::test]
    async fn run_history_returns_correct_total() {
        let pool = setup_pool().await;
        let job_id = create_job_with_runs(&pool, "count-job", "*/5 * * * *").await;
        for _ in 0..3 {
            insert_run(&pool, job_id, "success", "scheduled").await;
        }

        let result = get_run_history(&pool, job_id, 10, 0).await.unwrap();
        assert_eq!(result.total, 3);
        assert_eq!(result.items.len(), 3);
        pool.close().await;
    }

    // ── Log lines tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn log_lines_paginated_desc() {
        let pool = setup_pool().await;
        let job_id = create_job_with_runs(&pool, "log-job", "*/5 * * * *").await;
        let run_id = insert_run(&pool, job_id, "success", "scheduled").await;
        insert_logs(&pool, run_id, 10).await;

        let page1 = get_log_lines(&pool, run_id, 3, 0).await.unwrap();
        assert_eq!(page1.total, 10);
        assert_eq!(page1.items.len(), 3);
        // ORDER BY id ASC — first item has lowest id
        assert!(page1.items[0].id < page1.items[1].id);
        assert!(page1.items[1].id < page1.items[2].id);
        pool.close().await;
    }

    #[tokio::test]
    async fn log_lines_returns_correct_total() {
        let pool = setup_pool().await;
        let job_id = create_job_with_runs(&pool, "log-count-job", "*/5 * * * *").await;
        let run_id = insert_run(&pool, job_id, "success", "scheduled").await;
        insert_logs(&pool, run_id, 7).await;

        let result = get_log_lines(&pool, run_id, 100, 0).await.unwrap();
        assert_eq!(result.total, 7);
        assert_eq!(result.items.len(), 7);
        pool.close().await;
    }

    // ── get_job_by_id tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn get_job_by_id_returns_job() {
        let pool = setup_pool().await;
        let job_id = create_job_with_runs(&pool, "by-id-job", "*/5 * * * *").await;

        let job = get_job_by_id(&pool, job_id).await.unwrap();
        assert!(job.is_some());
        assert_eq!(job.unwrap().name, "by-id-job");

        let missing = get_job_by_id(&pool, 99999).await.unwrap();
        assert!(missing.is_none());
        pool.close().await;
    }

    // ── get_run_by_id tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn get_run_by_id_returns_run_with_job_name() {
        let pool = setup_pool().await;
        let job_id = create_job_with_runs(&pool, "detail-job", "*/5 * * * *").await;
        let run_id = insert_run(&pool, job_id, "success", "manual").await;

        let detail = get_run_by_id(&pool, run_id).await.unwrap();
        assert!(detail.is_some());
        let d = detail.unwrap();
        assert_eq!(d.job_name, "detail-job");
        assert_eq!(d.status, "success");
        assert_eq!(d.trigger, "manual");
        assert_eq!(d.job_id, job_id);

        let missing = get_run_by_id(&pool, 99999).await.unwrap();
        assert!(missing.is_none());
        pool.close().await;
    }
}
