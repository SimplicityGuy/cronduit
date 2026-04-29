//! Filter-matching stream-position helper (Phase 18 / WH-06).
//!
//! Returns the position of the current run within the consecutive-match
//! streak defined by the operator's `webhook.states` filter. Counts back
//! from the most-recent run, stopping at the first non-match OR the first
//! `success` (whichever is more recent). Mirrors the dual-SQL CTE shape
//! of `src/db/queries.rs::get_failure_context` so both queries hit the
//! same `idx_job_runs_job_id_start (job_id, start_time DESC)` index.
//!
//! Position semantics (D-15):
//! - Position 1 == this is the FIRST matching run since the previous
//!   non-match. With default `fire_every = 1`, this is the only position
//!   that triggers a delivery.
//! - Position N == there have been N consecutive matching runs since
//!   the last non-match.
//!
//! D-15 SUCCESS SENTINEL: The SQL CASE expression hard-codes
//! `WHEN status = 'success' THEN 0` BEFORE the IN-list check. This makes
//! a `success` run ALWAYS a streak break — even if the operator
//! misconfigures `states` to include `"success"`. Phase 18 contract says
//! success resets the filter-matching stream regardless of operator
//! filter; the sentinel enforces that at the SQL level so a single
//! integration path covers all operator misconfigurations.
//!
//! Pitfall I — variable-length operator-supplied `states` slice is
//! pre-padded to 6 placeholders by repeating the last entry. Duplicates
//! collapse harmlessly inside SQL `IN (...)`.

use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::db::DbPool;
use crate::db::queries::PoolRef;

/// SQLite — uses `?N` parameters and TEXT timestamp comparison via
/// lexicographic ordering (RFC3339 `Z`-suffix is order-stable; epoch
/// sentinel at COALESCE).
///
/// D-15 success sentinel hard-coded in the CASE expression: `WHEN
/// status='success' THEN 0` runs BEFORE the IN-list check, so success is
/// never counted as a match regardless of the operator's `states` filter.
pub(crate) const SQL_SQLITE: &str = r#"
    WITH ordered AS (
        SELECT id, status, start_time
          FROM job_runs
         WHERE job_id = ?1
           AND start_time <= ?2
         ORDER BY start_time DESC
    ),
    marked AS (
        SELECT id, status, start_time,
               CASE
                   WHEN status = 'success' THEN 0
                   WHEN status IN (?3, ?4, ?5, ?6, ?7, ?8) THEN 1
                   ELSE 0
               END AS is_match
          FROM ordered
    ),
    first_break AS (
        SELECT MAX(start_time) AS break_time
          FROM marked
         WHERE is_match = 0
    )
    SELECT COUNT(*) AS pos
      FROM marked
     WHERE is_match = 1
       AND start_time > COALESCE(
             (SELECT break_time FROM first_break),
             '1970-01-01T00:00:00Z'
           )
"#;

/// Postgres mirror — `$N` parameters; otherwise identical CTE shape.
///
/// D-15 success sentinel mirrored on this branch (CASE WHEN
/// status='success' THEN 0 before the IN-list check).
pub(crate) const SQL_POSTGRES: &str = r#"
    WITH ordered AS (
        SELECT id, status, start_time
          FROM job_runs
         WHERE job_id = $1
           AND start_time <= $2
         ORDER BY start_time DESC
    ),
    marked AS (
        SELECT id, status, start_time,
               CASE
                   WHEN status = 'success' THEN 0
                   WHEN status IN ($3, $4, $5, $6, $7, $8) THEN 1
                   ELSE 0
               END AS is_match
          FROM ordered
    ),
    first_break AS (
        SELECT MAX(start_time) AS break_time
          FROM marked
         WHERE is_match = 0
    )
    SELECT COUNT(*)::BIGINT AS pos
      FROM marked
     WHERE is_match = 1
       AND start_time > COALESCE(
             (SELECT break_time FROM first_break),
             '1970-01-01T00:00:00Z'
           )
"#;

/// Returns 6 strings — pre-padded by repeating the last entry. Caller's
/// `states` slice is operator-supplied (validated against the closed enum
/// in Plan 02), so duplicates collapse harmlessly inside SQL `IN (...)`.
/// Pitfall I.
fn pad_states_to_6(states: &[String]) -> [String; 6] {
    debug_assert!(!states.is_empty(), "validator guarantees states non-empty");
    let last = states
        .last()
        .cloned()
        .unwrap_or_else(|| "failed".to_string());
    let mut padded: [String; 6] = [
        last.clone(),
        last.clone(),
        last.clone(),
        last.clone(),
        last.clone(),
        last,
    ];
    for (i, s) in states.iter().take(6).enumerate() {
        padded[i] = s.clone();
    }
    padded
}

/// Position of `(job_id, current_start)` within the consecutive-match
/// streak defined by `states`. Counts back from `current_start`, stopping
/// at the first non-matching run OR the first `success` (D-15).
///
/// Returns `0` when the current run itself is not a match (e.g., dispatcher
/// is consulted on a status outside `states`).
#[allow(dead_code)] // Phase 18 dispatcher (Plan 04) consumes.
pub async fn filter_position(
    pool: &DbPool,
    job_id: i64,
    current_start: &DateTime<Utc>,
    states: &[String],
) -> anyhow::Result<i64> {
    let p = pad_states_to_6(states);
    // RFC3339 with Z suffix matches existing job_runs.start_time storage shape.
    let ts = current_start.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    match pool.reader() {
        PoolRef::Sqlite(pool) => {
            let row = sqlx::query(SQL_SQLITE)
                .bind(job_id)
                .bind(&ts)
                .bind(&p[0])
                .bind(&p[1])
                .bind(&p[2])
                .bind(&p[3])
                .bind(&p[4])
                .bind(&p[5])
                .fetch_one(pool)
                .await?;
            let pos: i64 = row.try_get("pos")?;
            Ok(pos)
        }
        PoolRef::Postgres(pool) => {
            let row = sqlx::query(SQL_POSTGRES)
                .bind(job_id)
                .bind(&ts)
                .bind(&p[0])
                .bind(&p[1])
                .bind(&p[2])
                .bind(&p[3])
                .bind(&p[4])
                .bind(&p[5])
                .fetch_one(pool)
                .await?;
            let pos: i64 = row.try_get("pos")?;
            Ok(pos)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::queries;
    use chrono::Utc;

    /// Set up an in-memory SQLite DbPool with full project migrations
    /// applied, then upsert a job and seed `job_runs` with the given
    /// (status, start_time) tuples. Returns the (pool, job_id).
    async fn setup_sqlite_with_runs(
        statuses: &[(&str, &str)], // (status, start_time RFC3339)
    ) -> (DbPool, i64) {
        let pool = DbPool::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory");
        pool.migrate().await.expect("migrate");

        let job_id = queries::upsert_job(
            &pool,
            "filter-position-test",
            "*/5 * * * *",
            "*/5 * * * *",
            "command",
            r#"{"command":"echo fp"}"#,
            "hash-fp",
            3600,
        )
        .await
        .expect("upsert job");

        let pool_ref = match pool.writer() {
            PoolRef::Sqlite(p) => p,
            _ => panic!("setup_sqlite_with_runs requires SQLite pool"),
        };
        let insert_sql = "INSERT INTO job_runs \
            (job_id, job_run_number, status, trigger, start_time, image_digest, config_hash) \
            VALUES (?, ?, ?, 'manual', ?, NULL, 'seed-hash')";
        for (n, (status, start_time)) in statuses.iter().enumerate() {
            sqlx::query(insert_sql)
                .bind(job_id)
                .bind((n + 1) as i64)
                .bind(*status)
                .bind(*start_time)
                .execute(pool_ref)
                .await
                .expect("insert run");
        }

        (pool, job_id)
    }

    #[tokio::test]
    async fn filter_position_basic_streak() {
        // 3 consecutive failures, current = the latest, states = ["failed"] -> 3.
        let (pool, job_id) = setup_sqlite_with_runs(&[
            ("failed", "2026-04-29T10:00:00Z"),
            ("failed", "2026-04-29T11:00:00Z"),
            ("failed", "2026-04-29T12:00:00Z"),
        ])
        .await;
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-29T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let p = filter_position(&pool, job_id, &now, &["failed".to_string()])
            .await
            .unwrap();
        assert_eq!(p, 3, "3 consecutive failures should yield position 3");
    }

    #[tokio::test]
    async fn filter_position_stops_at_success() {
        // success then 2 failures; states = ["failed"]; current = latest.
        // Success acts as a streak break -> position 2.
        let (pool, job_id) = setup_sqlite_with_runs(&[
            ("success", "2026-04-29T10:00:00Z"),
            ("failed", "2026-04-29T11:00:00Z"),
            ("failed", "2026-04-29T12:00:00Z"),
        ])
        .await;
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-29T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let p = filter_position(&pool, job_id, &now, &["failed".to_string()])
            .await
            .unwrap();
        assert_eq!(
            p, 2,
            "success acts as a non-match break; 2 failures since success"
        );
    }

    #[tokio::test]
    async fn filter_position_d13_scenario() {
        // D-13: failed -> timeout with states = ["timeout"] -> position 1
        // (failed is a non-match for the timeout-only filter, so it breaks).
        let (pool, job_id) = setup_sqlite_with_runs(&[
            ("failed", "2026-04-29T11:00:00Z"),
            ("timeout", "2026-04-29T12:00:00Z"),
        ])
        .await;
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-29T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let p = filter_position(&pool, job_id, &now, &["timeout".to_string()])
            .await
            .unwrap();
        assert_eq!(
            p, 1,
            "D-13: timeout-only filter, failed is a non-match, timeout is position 1"
        );
    }

    /// D-15 sentinel regression — even if the operator includes "success"
    /// in `states`, the SQL CASE expression's `WHEN status='success' THEN 0`
    /// branch (which runs BEFORE the IN-list check) makes success a streak
    /// break. Without the sentinel guard, this test returns 3; with it,
    /// returns 2.
    #[tokio::test]
    async fn filter_position_treats_success_as_break_even_when_in_states() {
        let (pool, job_id) = setup_sqlite_with_runs(&[
            ("success", "2026-04-29T10:00:00Z"),
            ("failed", "2026-04-29T11:00:00Z"),
            ("failed", "2026-04-29T12:00:00Z"),
        ])
        .await;
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-29T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        // Operator includes "success" in states -- but the SQL sentinel
        // pre-empts that and treats success as a streak break anyway.
        let p = filter_position(
            &pool,
            job_id,
            &now,
            &["success".to_string(), "failed".to_string()],
        )
        .await
        .unwrap();
        assert_eq!(
            p, 2,
            "D-15: success is ALWAYS a streak break, even when the operator's states includes \"success\". \
             Without the SQL CASE WHEN status='success' THEN 0 sentinel, this would return 3."
        );
    }
}
