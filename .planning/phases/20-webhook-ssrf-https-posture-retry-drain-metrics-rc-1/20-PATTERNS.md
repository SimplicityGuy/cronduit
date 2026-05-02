# Phase 20: Webhook SSRF/HTTPS Posture + Retry/Drain + Metrics — rc.1 - Pattern Map

**Mapped:** 2026-05-01
**Files analyzed:** 17 (8 created, 9 modified, 1 docs/justfile each)
**Analogs found:** 17 / 17

## File Classification

### Files to be CREATED

| New File | Role | Data Flow | Closest Analog | Match Quality |
|----------|------|-----------|----------------|---------------|
| `src/webhooks/retry.rs` | service (newtype wrapper around dispatcher trait) | request-response (composes `WebhookDispatcher::deliver`) + event-driven (cancel-aware sleeps) | `src/webhooks/dispatcher.rs::HttpDispatcher` (lines 90-303) for the trait-impl shape; `src/scheduler/retention.rs::run_prune_cycle` (lines 32-41) for the cancel-aware `tokio::select!` loop | exact (composition + trait impl) |
| `migrations/sqlite/20260502_000008_webhook_deliveries_add.up.sql` | migration (additive table create) | persistence (DDL — no data flow) | `migrations/sqlite/20260410_000000_initial.up.sql` (lines 16-46 — `CREATE TABLE jobs` + `CREATE TABLE job_runs` + indexes) for the new-table shape; `migrations/sqlite/20260428_000006_config_hash_add.up.sql` for the additive-migration header comment style | exact (additive + new table; not ADD COLUMN) |
| `migrations/postgres/20260502_000008_webhook_deliveries_add.up.sql` | migration (additive table create — postgres mirror) | persistence (DDL) | `migrations/postgres/20260410_000000_initial.up.sql` (matching lines for jobs/job_runs) | exact |
| `tests/v12_webhook_retry.rs` | test (integration; wiremock + tokio::test) | request-response (POST → wiremock 5xx → assert retry chain) | `tests/v12_webhook_failed_metric.rs` (whole file) for wiremock + dispatcher.deliver + counter delta-assert; `tests/v12_webhook_success_metric.rs` for the seed_job_with_failed_run helper | exact (same wiremock + sqlite::memory pattern) |
| `tests/v12_webhook_retry_classification.rs` | test (integration) | request-response | `tests/v12_webhook_failed_metric.rs` | exact |
| `tests/v12_webhook_retry_after.rs` | test (integration; uses `tokio::time::pause` + `advance`) | request-response with controlled clock | `tests/v12_webhook_failed_metric.rs` + tokio::test docs (clock pause/advance in dispatcher tests) | role-match (no existing tokio::time::pause test in tree) |
| `tests/v12_webhook_drain.rs` | test (integration; cancel.cancel() + wiremock with delay) | event-driven shutdown | `tests/v12_webhook_queue_drop.rs` + `tests/v12_webhook_failed_metric.rs` | role-match |
| `tests/v12_webhook_dlq.rs` | test (integration; sqlx + wiremock; reads `webhook_deliveries` rows) | persistence-read after delivery-fail | `tests/v12_webhook_failed_metric.rs` (sqlx Row read) + `tests/v12_webhook_filter_position_explain.rs` (sqlx schema verification pattern) | exact |
| `tests/v12_webhook_https_required.rs` | test (integration; uses `cronduit::config::parse_and_validate`) | config validation (load-time) | existing `tests/` config-validation tests + `src/config/validate.rs::tests` (extend) | role-match |
| `tests/v12_webhook_metrics_family.rs` | test (integration; renders `/metrics` and asserts label set) | observability render | `tests/metrics_endpoint.rs` (lines 1-60 — describe+TYPE/HELP assertions) | exact |

### Files to be MODIFIED

| Modified File | Role | Change Type | Closest Analog (in same file) | Match Quality |
|---------------|------|-------------|------------------------------|---------------|
| `src/webhooks/dispatcher.rs` | service (trait impl) | populate WebhookError variants in match arms; replace flat counters with labeled family; add histogram around send().await | self lines 260-301 (existing match arms) + self line 262 metric call | exact (in-file edit) |
| `src/webhooks/worker.rs` | worker (mpsc consumer) | extend `tokio::select!` with 3rd arm for drain deadline; sample `rx.len()` gauge | self lines 50-95 (existing select!) + `src/scheduler/retention.rs:32-41` (2-arm cancel-aware select! pattern) | exact |
| `src/webhooks/mod.rs` | module barrel | add `pub mod retry;` + re-export | self existing `pub mod dispatcher;` + `pub use dispatcher::{HttpDispatcher, ...}` | exact |
| `src/config/mod.rs` | config struct | add `webhook_drain_grace: Duration` field with humantime_serde | self lines 44-45 (existing `shutdown_grace`) + line 65-67 (`default_shutdown_grace`) | exact (template field) |
| `src/config/validate.rs` | validator (load-time) | extend `check_webhook_url` with HTTPS-required + IP classification | self lines 385-417 (existing `check_webhook_url`) | exact (extend in place) |
| `src/scheduler/retention.rs` | scheduler (background pruner) | add Phase 4 batch-delete loop for `webhook_deliveries` | self lines 56-91 (Phase 1 logs delete) + lines 94-130 (Phase 2 runs delete) | exact (additive phase) |
| `src/db/queries.rs` | persistence (sqlx helper) | add `WebhookDlqRow` struct + `insert_webhook_dlq_row` + `delete_old_webhook_deliveries_batch` | self `delete_old_logs_batch` (line 1434) + `delete_old_runs_batch` (line 1474) for the DELETE batch helper; `get_failure_context` (line 681) for the dual-dialect pattern | exact |
| `src/telemetry.rs` | observability (recorder boot) | add 3 describes + zero-baselines + histogram bucket config | self lines 67-71 (`set_buckets_for_metric` for `_run_duration_seconds`) + lines 91-153 (describe + zero-baseline block) + lines 172-182 (status pre-seed loop) | exact |
| `src/cli/run.rs` | bin (wire-up) | wrap `HttpDispatcher` in `RetryingDispatcher::new(...)`; pass drain_grace; pre-seed per-job metric labels | self lines 286-302 (existing dispatcher build + spawn_worker wire-up) | exact |
| `docs/WEBHOOKS.md` | documentation | append 6 sections (retry, Retry-After, DLQ, drain, HTTPS, metrics) | self existing 10 sections from P19 (receiver examples + HMAC) | exact (extension) |
| `justfile` | tooling (UAT recipes) | add `uat-webhook-retry`, `uat-webhook-drain`, `uat-webhook-dlq-query`, `uat-webhook-https-required` | self lines 326-352 (`uat-webhook-mock` / `uat-webhook-fire` / `uat-webhook-verify`) | exact (recipe-calls-recipe pattern) |

---

## Pattern Assignments

### `src/webhooks/retry.rs` (service, request-response + event-driven)

**Analog #1 (trait impl shape):** `src/webhooks/dispatcher.rs::HttpDispatcher` lines 75-118 + 158-303

**Imports + struct shape (dispatcher.rs lines 10-23, 90-118):**
```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
// (Phase 20 retry.rs adds: use tokio_util::sync::CancellationToken;
//                          use tokio::time::{sleep, Instant};)
use thiserror::Error;

use super::event::RunFinalized;
use crate::config::WebhookConfig;
use crate::db::DbPool;

pub struct HttpDispatcher {
    client: reqwest::Client,
    pool: DbPool,
    webhooks: Arc<HashMap<i64, WebhookConfig>>,
    cronduit_version: &'static str,
}

impl HttpDispatcher {
    #[allow(dead_code)] // Phase 18 Plan 05 wire-up consumes.
    pub fn new(
        pool: DbPool,
        webhooks: Arc<HashMap<i64, WebhookConfig>>,
    ) -> Result<Self, WebhookError> { /* ... */ }
}
```

**Trait impl skeleton (dispatcher.rs lines 158-260, condensed):**
```rust
#[async_trait]
impl WebhookDispatcher for HttpDispatcher {
    async fn deliver(&self, event: &RunFinalized) -> Result<(), WebhookError> {
        // 1. Look up per-job webhook config; absent => skip silently.
        let Some(cfg) = self.webhooks.get(&event.job_id) else {
            return Ok(());
        };
        // ... (more business logic) ...
        let response = req.body(body_bytes).send().await;

        // 11. Classify + record metrics.
        match response {
            Ok(resp) if resp.status().is_success() => { /* counter++ */ Ok(()) }
            Ok(resp) => { /* counter++ failed */ Ok(()) }
            Err(e) => { /* counter++ failed */ Ok(()) }
        }
    }
}
```

**Analog #2 (cancel-aware loop pattern):** `src/scheduler/retention.rs` lines 31-42

**Cancel-aware select! pattern (retention.rs lines 31-42):**
```rust
loop {
    tokio::select! {
        _ = interval.tick() => {
            run_prune_cycle(&pool, retention, &cancel).await;
        }
        _ = cancel.cancelled() => {
            tracing::info!(target: "cronduit.retention", "retention_pruner shutting down");
            break;
        }
    }
}
```

**Adaptation notes:**
- Newtype wrapper: `pub struct RetryingDispatcher<D: WebhookDispatcher> { inner: D, pool: DbPool, cancel: CancellationToken }` (D-05).
- `impl<D: WebhookDispatcher> WebhookDispatcher for RetryingDispatcher<D>` — composition (P18 D-21); NO trait expansion.
- The retry loop wraps `self.inner.deliver(event).await` and inspects the returned `WebhookError` variant (Phase 20 populates `HttpStatus(u16)`/`Network(String)`/`Timeout` in dispatcher.rs).
- Each retry sleep: `tokio::select! { _ = sleep(jittered) => continue, _ = self.cancel.cancelled() => break_with_dlq_shutdown_drain }` (D-03).
- DLQ-row write on terminal failure: call `crate::db::queries::insert_webhook_dlq_row(&self.pool, row).await` — log+continue on error per RESEARCH §4.8.
- Helpers as `pub(crate) fn`: `jitter(d: Duration) -> Duration` (D-02); `cap_for_slot(slot: usize, schedule: &[Duration]) -> Duration` (RESEARCH §4.7); `classify_response(...)` (D-06 table).
- Tests live in `#[cfg(test)] mod tests` at the bottom of the file (mirror dispatcher.rs lines 305-535).

---

### `src/webhooks/dispatcher.rs` (modify — populate variants + labeled metrics + histogram)

**Analog (in-file):** existing match arms at lines 260-301; existing metric calls at lines 262, 272, 285.

**Current match arm shape (dispatcher.rs lines 260-301):**
```rust
let response = req.body(body_bytes).send().await;

// 11. Classify + record metrics.
match response {
    Ok(resp) if resp.status().is_success() => {
        metrics::counter!("cronduit_webhook_delivery_sent_total").increment(1);
        tracing::debug!(/* ... */);
        Ok(())
    }
    Ok(resp) => {
        metrics::counter!("cronduit_webhook_delivery_failed_total").increment(1);
        let status = resp.status().as_u16();
        let body_preview = resp.text().await.unwrap_or_default();
        let truncated: String = body_preview.chars().take(200).collect();
        tracing::warn!(/* ... */, status, body_preview = %truncated, "webhook non-2xx");
        Ok(()) // Phase 18 posture per D-21; never propagate to worker
    }
    Err(e) => {
        metrics::counter!("cronduit_webhook_delivery_failed_total").increment(1);
        let kind = if e.is_timeout() { "timeout" } else if e.is_connect() { "connect" } else { "network" };
        tracing::warn!(/* ... */, kind, error = %e, "webhook network error");
        Ok(()) // Phase 18 posture per D-21
    }
}
```

**Adaptation notes (Phase 20):**
- Remove `#[allow(dead_code)]` on `WebhookError::HttpStatus`/`Network`/`Timeout`/`InvalidUrl` (lines 37-48).
- The `Ok(resp) =>` arm now returns `Err(WebhookError::HttpStatus(status))` instead of `Ok(())` (the wrapping `RetryingDispatcher` interprets the variant for retry classification). Per CONTEXT canonical_refs: "Phase 18 already pre-allowed [variants under #[allow(dead_code)]] — Phase 20 removes those allows by populating the variants in HttpDispatcher::deliver's match arms AND consuming them in RetryingDispatcher::deliver."
- The `Err(e) =>` arm splits on `e.is_timeout()` → `Err(WebhookError::Timeout)`; otherwise `Err(WebhookError::Network(format!("{e}")))`.
- Replace `metrics::counter!("cronduit_webhook_delivery_sent_total").increment(1)` with the labeled family call:
  ```rust
  metrics::counter!(
      "cronduit_webhook_deliveries_total",
      "job" => event.job_name.clone(),
      "status" => "success",
  ).increment(1);
  ```
- The pair labeled-family call for `failed` lives in `RetryingDispatcher` (terminal-failure boundary), NOT here (per-attempt is per-attempt; per-delivery is once at the chain end). Per CONTEXT: `_deliveries_total{status}` is per-delivery.
- Add histogram around the `req.body(body_bytes).send().await`:
  ```rust
  let attempt_start = tokio::time::Instant::now();
  let response = req.body(body_bytes).send().await;
  let dur = attempt_start.elapsed().as_secs_f64();
  metrics::histogram!(
      "cronduit_webhook_delivery_duration_seconds",
      "job" => event.job_name.clone(),
  ).record(dur);
  ```
- Histogram is per-attempt (NOT chain wall time) per D-24.

---

### `src/webhooks/worker.rs` (modify — third select! arm for drain budget + queue_depth gauge)

**Analog (in-file):** existing `worker_loop` lines 50-96.

**Current select! shape (worker.rs lines 55-95):**
```rust
loop {
    tokio::select! {
        // Bias toward draining events over checking cancel.
        biased;
        maybe_event = rx.recv() => {
            match maybe_event {
                Some(event) => {
                    if let Err(err) = dispatcher.deliver(&event).await {
                        tracing::warn!(/* ... */, error = %err, "webhook dispatch returned error");
                    }
                }
                None => {
                    tracing::info!(target: "cronduit.webhooks", "webhook worker exiting: channel closed");
                    break;
                }
            }
        }
        _ = cancel.cancelled() => {
            tracing::info!(
                target: "cronduit.webhooks",
                remaining = rx.len(),
                "webhook worker exiting: cancel token fired"
            );
            break;
        }
    }
}
```

**Adaptation notes (Phase 20 — RESEARCH §4.5 picked shape):**
- Function signature gains `drain_grace: Duration` parameter; `spawn_worker(...)` threads it through.
- State machine: `let mut drain_deadline: Option<tokio::time::Instant> = None;` initialized to `None` outside the loop.
- On first `cancel.cancelled()` fire: set `drain_deadline = Some(Instant::now() + drain_grace)`; log `webhook worker entering drain mode (budget: {drain_grace:?})`; do NOT break.
- Third arm (only active when `drain_deadline.is_some()`): `_ = sleep_until_or_pending(drain_deadline) => { /* drop remaining; increment _deliveries_total{status="dropped"} per event; break */ }`. Use a `match drain_deadline { Some(d) => tokio::time::sleep_until(d).left_future(), None => future::pending().right_future() }` shape OR reorganize so the third arm is added only inside a sub-loop entered on cancel-fire (research §4.5 leaves both forms acceptable).
- Sample queue depth on every `rx.recv()` boundary: `metrics::gauge!("cronduit_webhook_queue_depth").set(rx.len() as f64);` immediately before or after the recv branch.
- Drain-and-drop loop: use `rx.try_recv()` in a tight loop until empty, incrementing `cronduit_webhook_deliveries_total{job=??, status="dropped"}` per event AND the existing `cronduit_webhook_delivery_dropped_total` (P15) is **NOT** touched (per D-26 — it's the channel-saturation drop, distinct from drain drop).

---

### `src/config/mod.rs` (modify — add webhook_drain_grace field)

**Analog (in-file):** existing `shutdown_grace` field at lines 44-45 + default fn at lines 65-67.

**Current shape:**
```rust
#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_db_url")]
    pub database_url: SecretString,
    /// MANDATORY (D-19). No implicit host-timezone fallback.
    pub timezone: String,
    #[serde(default = "default_shutdown_grace", with = "humantime_serde")]
    pub shutdown_grace: Duration,
    #[serde(default = "default_log_retention", with = "humantime_serde")]
    pub log_retention: Duration,
    /// Enable file watcher for automatic config reload (D-10, RELOAD-03).
    /// Default: true. Disable with `watch_config = false` in `[server]`.
    #[serde(default = "default_watch_config")]
    pub watch_config: bool,
}

fn default_shutdown_grace() -> Duration {
    Duration::from_secs(30)
}
```

**Adaptation notes:**
- Add identical-shape field `webhook_drain_grace: Duration` with `#[serde(default = "default_webhook_drain_grace", with = "humantime_serde")]` (D-16).
- Add `fn default_webhook_drain_grace() -> Duration { Duration::from_secs(30) }`.
- Doc comment cites D-15/D-16 + RESEARCH §4.5 + worst-case ceiling = `drain_grace + 10s` (D-18).

---

### `src/config/validate.rs` (modify — extend check_webhook_url with HTTPS classification)

**Analog (in-file):** existing `check_webhook_url` lines 385-417 (RESEARCH §4.2 — extend in place).

**Current shape (lines 385-417):**
```rust
fn check_webhook_url(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(wh) = &job.webhook else {
        return;
    };
    match url::Url::parse(&wh.url) {
        Err(e) => {
            errors.push(ConfigError {
                file: path.into(),
                line: 0,
                col: 0,
                message: format!(
                    "[[jobs]] `{}`: webhook.url `{}` is not a valid URL: {}. \
                     Provide a fully-qualified URL like `https://hook.example.com/path`.",
                    job.name, wh.url, e
                ),
            });
        }
        Ok(parsed) => {
            let scheme = parsed.scheme();
            if scheme != "http" && scheme != "https" {
                errors.push(ConfigError { /* ... unsupported scheme ... */ });
            }
        }
    }
}
```

**Adaptation notes:**
- After the scheme check, if `scheme == "http"`:
  - Call `parsed.host()` returning `Option<url::Host<&str>>`.
  - For `Some(Host::Ipv4(v4))`: accept if `v4.is_loopback() || v4.is_private()`; else reject.
  - For `Some(Host::Ipv6(v6))`: accept if `v6.is_loopback() || v6.is_unique_local()` (RESEARCH §4.1 — Ipv6Addr::is_unique_local stable since Rust 1.84).
  - For `Some(Host::Domain(name))`: accept iff `name.eq_ignore_ascii_case("localhost")`; reject everything else (D-19/D-20 — no DNS resolution).
- Reject error message format mirrors D-21 verbatim:
  ```text
  [[jobs]] `{name}`: webhook.url `{url}` requires HTTPS for non-loopback / non-RFC1918 destinations. Use `https://` or one of the allowed local nets: 127/8, ::1, 10/8, 172.16/12, 192.168/16, fd00::/8.
  ```
- HTTP-allowed path emits `tracing::info!(target: "cronduit.config", url = %wh.url, classified_net = %classification, "webhook URL accepted on local net")` per D-19.
- The IPv4-loopback precedent in `src/cli/run.rs:337-342` (`is_loopback`) is the codebase's existing stdlib-IP-helper usage.

---

### `migrations/sqlite/20260502_000008_webhook_deliveries_add.up.sql` (create — DLQ table)

**Analog (sibling):** `migrations/sqlite/20260410_000000_initial.up.sql` lines 16-46 (full table-create shape).

**Sample table-create + indexes pattern (initial.up.sql lines 32-46):**
```sql
CREATE TABLE IF NOT EXISTS job_runs (
    id             INTEGER PRIMARY KEY,
    job_id         INTEGER NOT NULL REFERENCES jobs(id),
    status         TEXT    NOT NULL,
    trigger        TEXT    NOT NULL,
    start_time     TEXT    NOT NULL,
    end_time       TEXT,
    duration_ms    INTEGER,
    exit_code      INTEGER,
    container_id   TEXT,
    error_message  TEXT
);

CREATE INDEX IF NOT EXISTS idx_job_runs_job_id_start ON job_runs(job_id, start_time DESC);
CREATE INDEX IF NOT EXISTS idx_job_runs_start_time   ON job_runs(start_time);
```

**Header-comment shape from additive precedent (`20260428_000006_config_hash_add.up.sql` lines 1-23):**
```sql
-- Phase 16: job_runs.config_hash per-run column (FCTX-04).
-- ...
-- Pairs with migrations/postgres/20260428_000006_config_hash_add.up.sql.
-- Any structural change MUST land in both files in the same PR;
-- tests/schema_parity.rs::normalize_type collapses TEXT-family types to
-- TEXT, so this column passes parity with zero test edits (RESEARCH §E).
```

**Adaptation notes (Phase 20):**
- Filename starts new sequence: `20260502_000008_webhook_deliveries_add.up.sql` (timestamp from CONTEXT prefix; sequence after `_000007`).
- Header comment cites Phase 20 / WH-05 / D-10 / D-13; explicit pairing note with postgres mirror.
- Schema (verbatim from CONTEXT D-10):
  ```sql
  CREATE TABLE webhook_deliveries (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id           INTEGER NOT NULL,
    job_id           INTEGER NOT NULL,
    url              TEXT    NOT NULL,
    attempts         INTEGER NOT NULL,
    last_status      INTEGER,
    last_error       TEXT,
    dlq_reason       TEXT    NOT NULL,
    first_attempt_at TEXT    NOT NULL,
    last_attempt_at  TEXT    NOT NULL,
    FOREIGN KEY (run_id) REFERENCES job_runs(id),
    FOREIGN KEY (job_id) REFERENCES jobs(id)
  );
  CREATE INDEX idx_webhook_deliveries_last_attempt ON webhook_deliveries (last_attempt_at);
  ```
- Use `CREATE TABLE IF NOT EXISTS` + `CREATE INDEX IF NOT EXISTS` per the initial migration's idempotency precedent (lines 16, 30, 32, 45, 46).
- Postgres mirror swaps `INTEGER PRIMARY KEY AUTOINCREMENT` → `BIGSERIAL PRIMARY KEY` (PG dialect); other types unchanged (TEXT, INTEGER both portable).

---

### `src/scheduler/retention.rs` (modify — add Phase 4 webhook_deliveries delete loop)

**Analog (in-file):** existing Phase 1 (delete logs) lines 56-91 + Phase 2 (delete runs) lines 94-130.

**Phase 1 delete-batch loop shape (retention.rs lines 56-91):**
```rust
// Phase 1: Delete old log lines (children first for FK safety).
let mut total_logs_deleted: i64 = 0;
loop {
    if cancel.is_cancelled() {
        tracing::warn!(
            target: "cronduit.retention",
            logs_deleted = total_logs_deleted,
            "prune interrupted by shutdown"
        );
        return;
    }
    match queries::delete_old_logs_batch(pool, &cutoff_str, BATCH_SIZE).await {
        Ok(deleted) => {
            total_logs_deleted += deleted;
            if deleted > 0 {
                tracing::debug!(/* ... */ "prune_batch: logs");
            }
            if deleted < BATCH_SIZE {
                break;
            }
            tokio::time::sleep(BATCH_SLEEP).await;
        }
        Err(e) => {
            tracing::error!(/* ... */ "retention prune: failed to delete log batch");
            break;
        }
    }
}
```

**Adaptation notes:**
- Insert as Phase 4 AFTER Phase 2 (delete runs) and BEFORE Phase 3 (WAL checkpoint check) — so the WAL threshold sums all three deletes.
- Helper to call: `queries::delete_old_webhook_deliveries_batch(pool, &cutoff_str, BATCH_SIZE)` (added in `src/db/queries.rs`).
- Counter variable: `total_webhook_dlq_deleted: i64`; rolled into the WAL checkpoint sum (`total_deleted = total_logs_deleted + total_runs_deleted + total_webhook_dlq_deleted`).
- Final log line at line 149-154 expanded to include `webhook_dlq_deleted`.

---

### `src/db/queries.rs` (modify — add WebhookDlqRow + insert_webhook_dlq_row + delete_old_webhook_deliveries_batch)

**Analog #1 (struct shape):** `FailureContext` struct + `get_failure_context` (lines 635-707) for the dual-dialect query helper.

**Analog #2 (delete-batch shape):** `delete_old_logs_batch` lines 1434-1471.

**Delete-batch dual-dialect pattern (lines 1434-1471):**
```rust
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
            let result = sqlx::query(/* $1/$2 placeholders */)
                .bind(cutoff)
                .bind(batch_size)
                .execute(p)
                .await?;
            Ok(result.rows_affected() as i64)
        }
    }
}
```

**Adaptation notes:**
- New `pub struct WebhookDlqRow { pub run_id: i64, pub job_id: i64, pub url: String, pub attempts: i64, pub last_status: Option<i64>, pub last_error: Option<String>, pub dlq_reason: String, pub first_attempt_at: String, pub last_attempt_at: String }` — fields match the D-10 schema 1:1.
- New `pub async fn insert_webhook_dlq_row(pool: &DbPool, row: WebhookDlqRow) -> Result<(), sqlx::Error>` — dual-dialect via `match pool.writer()` (mirrors `delete_old_logs_batch`).
- `last_error` truncation to ≤500 chars happens at the call site in `retry.rs`, NOT here (per D-10).
- New `pub async fn delete_old_webhook_deliveries_batch(pool: &DbPool, cutoff: &str, batch_size: i64) -> Result<i64, sqlx::Error>` — exact shape of `delete_old_logs_batch` but DELETE FROM webhook_deliveries WHERE last_attempt_at < ?1 LIMIT ?2.

---

### `src/telemetry.rs` (modify — describe + zero-baseline new metrics + histogram buckets)

**Analog #1 (histogram buckets):** lines 67-71.

**Existing pattern (lines 67-71):**
```rust
let handle = PrometheusBuilder::new()
    .set_buckets_for_metric(
        Matcher::Full("cronduit_run_duration_seconds".to_string()),
        &[1.0, 5.0, 15.0, 30.0, 60.0, 300.0, 900.0, 1800.0, 3600.0],
    )
    .expect("valid bucket config")
    .install_recorder()
    .expect("metrics recorder not yet installed");
```

**Analog #2 (describe + zero-baseline):** lines 91-153 + 172-182 (status pre-seed loop).

**Status pre-seed loop pattern (lines 172-182):**
```rust
for status in [
    "success",
    "failed",
    "timeout",
    "cancelled",
    "error",
    "stopped",
] {
    metrics::counter!("cronduit_runs_total", "status" => status.to_string())
        .increment(0);
}
```

**Adaptation notes (Phase 20):**
- Chain a second `set_buckets_for_metric` for `cronduit_webhook_delivery_duration_seconds` with buckets `[0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]` per RESEARCH §4.4.
- Replace the P18 describes at lines 122-133 (`_sent_total` + `_failed_total`) with the new family describes:
  - `describe_counter!("cronduit_webhook_deliveries_total", "Total webhook deliveries by terminal outcome (success/failed/dropped). Replaces P18 _sent_total/_failed_total flat counters per Phase 20 D-22.")`.
  - `describe_histogram!("cronduit_webhook_delivery_duration_seconds", "Per-attempt HTTP duration in seconds, labeled by job. NOT chain wall time (D-24).")`.
  - `describe_gauge!("cronduit_webhook_queue_depth", "Current depth of the webhook delivery channel, sampled by the worker on each rx.recv() boundary (D-25).")`.
- Preserve the P15 `cronduit_webhook_delivery_dropped_total` describe + zero-baseline as-is per D-22/D-26 (it's the channel-saturation counter, distinct from `_deliveries_total{status="dropped"}` which is the drain-drop counter).
- Preserve P18 zero-baselines? **No** — the flat `_sent_total`/`_failed_total` counters are removed entirely (D-22). Operators with v1.1 dashboards must migrate to the labeled family; documented in `docs/WEBHOOKS.md` and rc.1 release notes.
- Status pre-seed loop for the new family:
  ```rust
  for status in ["success", "failed", "dropped"] {
      metrics::counter!("cronduit_webhook_deliveries_total", "status" => status.to_string())
          .increment(0);
  }
  ```
  (Per-job seeding lives in `src/cli/run.rs` after `sync_result.jobs` is in scope per RESEARCH §4.6.)
- Zero-baseline the gauge: `metrics::gauge!("cronduit_webhook_queue_depth").set(0.0);`.
- Zero-baseline the histogram: `metrics::histogram!("cronduit_webhook_delivery_duration_seconds").record(0.0);` (matches the existing `_run_duration_seconds` baseline at line 147).

---

### `src/cli/run.rs` (modify — wrap dispatcher in RetryingDispatcher + thread drain_grace + per-job seed)

**Analog (in-file):** existing dispatcher build + spawn_worker wire-up at lines 286-302.

**Current shape (lines 286-302):**
```rust
let dispatcher: std::sync::Arc<dyn crate::webhooks::WebhookDispatcher> = if webhooks.is_empty() {
    std::sync::Arc::new(crate::webhooks::NoopDispatcher)
} else {
    let http =
        crate::webhooks::HttpDispatcher::new(pool.clone(), std::sync::Arc::new(webhooks))
            .map_err(|e| anyhow::anyhow!("HttpDispatcher init failed: {e}"))?;
    std::sync::Arc::new(http)
};

let (webhook_tx, webhook_rx) = crate::webhooks::channel();
let webhook_worker_handle =
    crate::webhooks::spawn_worker(webhook_rx, dispatcher, cancel.child_token());
```

**Adaptation notes (Phase 20):**
- Inside the `else` branch (when webhooks are configured), wrap `http` in `RetryingDispatcher`:
  ```rust
  let http = crate::webhooks::HttpDispatcher::new(pool.clone(), std::sync::Arc::new(webhooks))
      .map_err(|e| anyhow::anyhow!("HttpDispatcher init failed: {e}"))?;
  let retrying = crate::webhooks::RetryingDispatcher::new(http, pool.clone(), cancel.child_token());
  std::sync::Arc::new(retrying)
  ```
- `spawn_worker(...)` signature gains `drain_grace: Duration` (passed from `cfg.server.webhook_drain_grace`):
  ```rust
  let webhook_worker_handle =
      crate::webhooks::spawn_worker(webhook_rx, dispatcher, cancel.child_token(), cfg.server.webhook_drain_grace);
  ```
- Per-job metric pre-seed AFTER `sync_result.jobs` is in scope (RESEARCH §4.6):
  ```rust
  for job in &sync_result.jobs {
      for status in ["success", "failed", "dropped"] {
          metrics::counter!(
              "cronduit_webhook_deliveries_total",
              "job" => job.name.clone(),
              "status" => status,
          ).increment(0);
      }
  }
  ```
  This block can also be a helper `fn seed_per_job_metrics(jobs: &[DbJob])` either in `src/webhooks/mod.rs` or `src/telemetry.rs`; planner picks.

---

### `tests/v12_webhook_retry.rs` and family (create — wiremock + sqlite::memory)

**Analog:** `tests/v12_webhook_failed_metric.rs` (whole file) + `tests/v12_webhook_success_metric.rs` (`seed_job_with_failed_run`).

**Sample setup harness (failed_metric.rs lines 32-72):**
```rust
async fn setup_test_db() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    pool.migrate().await.expect("apply migrations");
    pool
}

async fn seed_job_with_failed_run(pool: &DbPool) -> (i64, i64) {
    let now = chrono::Utc::now().to_rfc3339();
    let p = match pool.writer() {
        PoolRef::Sqlite(p) => p,
        _ => panic!("sqlite-only test"),
    };
    let job_row = sqlx::query(
        "INSERT INTO jobs (name, schedule, resolved_schedule, job_type, config_json, config_hash, timeout_secs, created_at, updated_at) \
         VALUES ('failed-metric-job', '* * * * *', '* * * * *', 'command', '{}', 'seed-cfg', 60, ?1, ?1) RETURNING id",
    )
    .bind(&now)
    .fetch_one(p)
    .await
    .expect("seed job");
    let job_id: i64 = job_row.get("id");
    /* ... seed run ... */
    (job_id, run_id)
}
```

**Counter delta-assert pattern (success_metric.rs lines 76-132):**
```rust
#[tokio::test]
async fn webhook_success_metric_increments_sent_total() {
    let handle = setup_metrics();

    // Capture baselines BEFORE the test action — the OnceLock-backed
    // PrometheusHandle is shared with anything else in this test binary.
    let baseline_sent = read_counter(&handle.render(), "cronduit_webhook_delivery_sent_total");
    /* ... */

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let url = server.uri();
    /* ... build dispatcher + deliver ... */

    let final_sent = read_counter(&handle.render(), "cronduit_webhook_delivery_sent_total");
    assert_eq!(final_sent - baseline_sent, 1.0, /* ... */);
}
```

**Adaptation notes:**
- Each `tests/v12_webhook_*.rs` is a standalone integration test binary (Cargo's per-file convention). Ship the boilerplate `setup_test_db` + `seed_job_with_failed_run` + `read_counter` helpers per-file (or refactor into `tests/common/mod.rs` if shared — planner decides).
- `tests/v12_webhook_retry_after.rs` adds `tokio::time::pause()` + `advance(30s)` + `advance(270s)` to drive the deterministic retry clock; wiremock's `MockServer` is fine under paused-time because it spawns its own runtime.
- `tests/v12_webhook_drain.rs` constructs the worker directly (`spawn_worker(rx, dispatcher, cancel, Duration::from_secs(2))`), pushes events into `tx`, fires `cancel.cancel()`, and asserts the worker exits within `drain_grace + 10s + slack`.
- `tests/v12_webhook_dlq.rs` reads the `webhook_deliveries` table after delivery-fail using `sqlx::query("SELECT * FROM webhook_deliveries WHERE run_id = ?")` and asserts `dlq_reason` per scenario.
- `tests/v12_webhook_https_required.rs` uses `cronduit::config::parse_and_validate(toml_str)` → asserts `Err` for `http://example.com` and `Ok` for `http://192.168.1.1`. The validator-error matrix lives in `src/config/validate.rs::tests` (unit) AND repeats key cases here (integration for boot-time INFO log assertion).
- `tests/v12_webhook_metrics_family.rs` extends the `metrics_endpoint.rs` shape:

**Reference (metrics_endpoint.rs lines 23-60):**
```rust
let body = handle.render();

assert!(
    body.contains("# HELP cronduit_scheduler_up"),
    "missing HELP for cronduit_scheduler_up; body: {body}"
);
assert!(
    body.contains("# TYPE cronduit_scheduler_up gauge"),
    "missing TYPE for cronduit_scheduler_up; body: {body}"
);
```

Phase 20 adds analogous `# HELP` / `# TYPE` assertions for `cronduit_webhook_deliveries_total`, `cronduit_webhook_delivery_duration_seconds`, `cronduit_webhook_queue_depth`. Also assert per-job seed: `body.contains(r#"cronduit_webhook_deliveries_total{job="<seed-job>",status="success"} 0"#)`.

---

### `justfile` (modify — add 4 UAT recipes)

**Analog (in-file):** existing `uat-webhook-mock` / `uat-webhook-fire` / `uat-webhook-verify` recipes lines 325-352.

**Existing recipe-calls-recipe pattern (lines 335-342):**
```just
[group('uat')]
[doc('Phase 18 — force Run Now on a webhook-configured job (operator-supplied JOB_NAME)')]
uat-webhook-fire JOB_NAME:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▶ UAT: triggering run for {{JOB_NAME}} — watch the receiver and the cronduit log"
    JOB_ID=$(just api-job-id "{{JOB_NAME}}")
    just api-run-now "$JOB_ID"
```

**Tail-the-log shape (lines 348-352):**
```just
[group('uat')]
[doc('Phase 18 — print last 30 lines of webhook mock log for maintainer hand-validation')]
uat-webhook-verify:
    @echo "Last 30 lines from /tmp/cronduit-webhook-mock.log:"
    @echo "Maintainer: confirm headers (...), 16-field body, signature format v1,<base64>."
    @tail -n 30 /tmp/cronduit-webhook-mock.log 2>/dev/null || echo "(log empty; ensure 'just uat-webhook-mock' is running and 'just uat-webhook-fire <JOB>' was triggered)"
```

**Adaptation notes (Phase 20):**
- `uat-webhook-retry` — composes `uat-webhook-mock` (mock returning 500) + `uat-webhook-fire <JOB>` + `uat-webhook-verify` to count 3 attempts in the mock log. Identical shape to `uat-webhook-fire`.
- `uat-webhook-drain` — manual SIGTERM during in-flight delivery; references `just dev` + manual Ctrl-C. Documents the worst-case `drain_grace + 10s` ceiling.
- `uat-webhook-dlq-query` — runs `sqlite3 cronduit.dev.db 'SELECT * FROM webhook_deliveries WHERE last_attempt_at > datetime("now", "-1 hour")'`. Mirrors `uat-fctx-bugfix-spot-check` (existing sqlite3 caller per CONTEXT canonical_refs) — planner verifies that recipe's exact shape.
- `uat-webhook-https-required` — `cargo run --bin cronduit -- check examples/bad-webhook-url.toml` and asserts non-zero exit. Body is short bash; uses `[group('uat')]` + `[doc('...')]` attributes consistent with P18/P19 recipes.

---

### `docs/WEBHOOKS.md` (modify — append 6 sections)

**Analog (in-file):** existing 10 sections from P19 (HMAC + Standard Webhooks v1 + receiver examples). Phase 20 EXTENDS — does not restructure.

**Adaptation notes:**
- New sections per D-27 (in order):
  1. **Retry schedule** — the `[0, 30s, 300s]` table + jitter math + mermaid diagram of the 3-attempt chain (mermaid only, no ASCII per project memory).
  2. **Retry-After header handling** — D-07 integer-seconds-only + D-08 cap math worked example (the table from RESEARCH §4.7).
  3. **DLQ table** — schema + sample queries:
     ```sql
     SELECT * FROM webhook_deliveries
       WHERE last_attempt_at > datetime('now', '-1 hour')
       ORDER BY last_attempt_at DESC;
     ```
     + closed-enum `dlq_reason` table.
  4. **Drain on shutdown** — D-15/D-18 semantics, mermaid sequence diagram of SIGTERM → drain → drop, worst-case ceiling formula.
  5. **HTTPS / SSRF posture** — D-19 allowlist table (127/8, ::1, 10/8, 172.16/12, 192.168/16, fd00::/8), accepted-risk note for hostnames + DNS-resolution disclaimer (D-20), forward-pointer to `THREAT_MODEL.md` TM5 (P24).
  6. **Metrics family** — the new `_deliveries_total{job, status}` table + dropped-counter split (P15 channel-saturation vs P20 drain-drop) + histogram buckets `[0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]` + sample `histogram_quantile(0.95, ...)` PromQL.
- All diagrams in code blocks tagged ` ```mermaid ` (project memory `feedback_diagrams_mermaid.md`).
- No code changes; same PR as the source code (D-27 last sentence).

---

## Shared Patterns

### Cancel-aware async loop (worker drain + retry-sleep)

**Source #1:** `src/scheduler/retention.rs` lines 31-42 (2-arm select! with cancel arm)
**Source #2:** `src/webhooks/worker.rs` lines 55-95 (existing 2-arm: recv + cancel)
**Apply to:** `src/webhooks/retry.rs` (cancel-aware `tokio::time::sleep` per D-03), `src/webhooks/worker.rs` (3rd arm for drain deadline per RESEARCH §4.5).

```rust
loop {
    tokio::select! {
        biased;
        _ = work_branch() => { /* ... */ }
        _ = cancel.cancelled() => {
            tracing::info!(/* ... */);
            break;  // or transition into drain mode
        }
    }
}
```

### Dual-dialect sqlx query helper (sqlite + postgres)

**Source:** `src/db/queries.rs::delete_old_logs_batch` lines 1434-1471 (paired `match pool.writer() { Sqlite => ... Postgres => ... }`).
**Apply to:** `insert_webhook_dlq_row` and `delete_old_webhook_deliveries_batch` in `src/db/queries.rs`.

```rust
pub async fn helper(pool: &DbPool, ...) -> Result<T, sqlx::Error> {
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            sqlx::query("SQLITE SQL with ?1 ?2 placeholders")
                .bind(...)
                .execute(p).await?;
            /* ... */
        }
        PoolRef::Postgres(p) => {
            sqlx::query("POSTGRES SQL with $1 $2 placeholders")
                .bind(...)
                .execute(p).await?;
            /* ... */
        }
    }
}
```

### Eager describe + zero-baseline metric registration

**Source:** `src/telemetry.rs` lines 91-153 + 172-182.
**Apply to:** Phase 20 telemetry extension for `_deliveries_total` (status pre-seed loop) + `_delivery_duration_seconds` (zero-baseline record) + `_queue_depth` (zero-baseline set).

```rust
// 1. Describe (HELP/TYPE).
metrics::describe_counter!("name", "doc string");

// 2. Force registration (zero-baseline).
metrics::counter!("name").increment(0);

// 3. (Optional) Pre-seed labeled rows.
for value in CLOSED_ENUM {
    metrics::counter!("name", "label" => value.to_string()).increment(0);
}
```

### Counter delta-assert in integration tests

**Source:** `tests/v12_webhook_success_metric.rs` lines 76-132 + `tests/v12_webhook_failed_metric.rs`.
**Apply to:** All Phase 20 metric-touching integration tests.

```rust
let handle = setup_metrics();
let baseline = read_counter(&handle.render(), "metric_name");
/* ... action ... */
let final_value = read_counter(&handle.render(), "metric_name");
assert_eq!(final_value - baseline, expected_delta);
```

(Required because the OnceLock-backed PrometheusHandle is process-global; absolute-value asserts cross-pollute between tests in the same binary.)

### LOAD-time validator with ConfigError { line: 0, col: 0 }

**Source:** `src/config/validate.rs::check_webhook_url` lines 385-417 (P18) + `check_webhook_block_completeness` lines 425+.
**Apply to:** Phase 20 HTTPS-required extension of `check_webhook_url`.

```rust
errors.push(ConfigError {
    file: path.into(),
    line: 0,
    col: 0,
    message: format!(
        "[[jobs]] `{}`: webhook.url `{}` requires HTTPS for non-loopback / non-RFC1918 destinations. \
         Use `https://` or one of the allowed local nets: 127/8, ::1, 10/8, 172.16/12, 192.168/16, fd00::/8.",
        job.name, wh.url
    ),
});
```

### Additive migration with paired sqlite/postgres files

**Source:** `migrations/sqlite/20260428_000006_config_hash_add.up.sql` + postgres mirror.
**Apply to:** `migrations/{sqlite,postgres}/20260502_000008_webhook_deliveries_add.up.sql`.

- Header comment cites phase + requirement ID + paired-file note.
- `CREATE TABLE IF NOT EXISTS` + `CREATE INDEX IF NOT EXISTS` for sqlite idempotency.
- `tests/schema_parity.rs::normalize_type` collapses TEXT-family — Phase 20 column types (TEXT + INTEGER + INTEGER nullable + TEXT nullable) all pass parity.

---

## No Analog Found

All Phase 20 files have a close codebase analog (the codebase is mature for v1.2). No "no analog" rows.

---

## Metadata

**Analog search scope:** `src/webhooks/`, `src/scheduler/`, `src/config/`, `src/db/`, `src/cli/`, `src/telemetry.rs`, `migrations/{sqlite,postgres}/`, `tests/v12_webhook_*.rs`, `tests/metrics_endpoint.rs`, `justfile` (lines 285-360).

**Files scanned:** 14 (5 src files read in full, 4 migration files inspected, 4 test files inspected, 1 justfile section inspected).

**Pattern extraction date:** 2026-05-01.

**Cross-cutting reminders for the planner consumer:**
- Phase 20 adds **ZERO** new external crates (Cargo.toml unchanged per D-38).
- `cargo tree -i openssl-sys` MUST remain empty (rustls-everywhere invariant).
- All diagrams in PLAN/SUMMARY/README/code comments/`docs/WEBHOOKS.md` are **mermaid** (project memory `feedback_diagrams_mermaid.md`).
- `Cargo.toml` version stays at `1.2.0`; the `-rc.1` is a tag-only suffix (project memory `feedback_tag_release_version_match.md`).
- UAT recipes use existing `just` commands; no ad-hoc `cargo`/`docker`/URL invocations (project memory `feedback_uat_use_just_commands.md`).
- Maintainer validates UAT — Claude does NOT mark UAT passed (project memory `feedback_uat_user_validates.md`).
