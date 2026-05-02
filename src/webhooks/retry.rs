//! Phase 20 / WH-05: RetryingDispatcher — composes HttpDispatcher to add an
//! in-memory 3-attempt retry chain with full-jitter backoff, classification,
//! Retry-After honoring with cap, cancel-aware sleeps, and DLQ-row write on
//! terminal failure. The wrapped trait stays at two lines (P18 D-21).
//!
//! Composition: `RetryingDispatcher<D: WebhookDispatcher>` impls `WebhookDispatcher`
//! by wrapping `inner: D` and intercepting Err variants for retry decisions.
//! The worker (`src/webhooks/worker.rs`) calls `dispatcher.deliver(event).await`
//! exactly as before — it never learns retries exist.
//!
//! Decision references (CONTEXT.md):
//! - D-01: composition newtype wrapper (no trait expansion — P18 D-21 invariant)
//! - D-02: full-jitter `[0.8, 1.2)` factor on rand 0.10 global free function
//! - D-03: cancel-aware `tokio::select!` on every retry-sleep boundary
//! - D-04: in-memory only — no restart survival, SIGTERM-loss recorded as `shutdown_drain`
//! - D-06: classification map (200..=299 success, 408/429/5xx/Network/Timeout transient,
//!   4xx-other permanent)
//! - D-07: integer-seconds `Retry-After` only; HTTP-date form falls back to schedule + WARN
//! - D-08: cap math `cap_for_slot(slot, schedule) = schedule[slot+1] * 1.2`
//!   (last-slot fallback to `schedule[slot] * 1.2`)
//! - D-10/D-12: DLQ row's `url` column equals configured webhook URL; lookup at
//!   write time via `Arc<HashMap<i64, WebhookConfig>>`

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use tokio_util::sync::CancellationToken;

use super::dispatcher::{WebhookDispatcher, WebhookError};
use super::event::RunFinalized;
use crate::config::WebhookConfig;
use crate::db::DbPool;
use crate::db::queries::{self, WebhookDlqRow};

/// Closed enum of dlq_reason column values written into webhook_deliveries.
/// String conversions match the column values exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlqReason {
    Http4xx,
    Http5xx,
    Network,
    Timeout,
    ShutdownDrain,
}

impl DlqReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            DlqReason::Http4xx => "http_4xx",
            DlqReason::Http5xx => "http_5xx",
            DlqReason::Network => "network",
            DlqReason::Timeout => "timeout",
            DlqReason::ShutdownDrain => "shutdown_drain",
        }
    }
}

/// Result of `classify(&WebhookError)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    /// 4xx other than 408/429: stop, write DLQ row, no retry.
    Permanent(DlqReason),
    /// 5xx, 408, 429, network, timeout: retry per schedule (subject to Retry-After cap).
    Transient(DlqReason),
}

/// Per CONTEXT D-06: classification map for retry decisions.
///   - 200..=299 → unreachable in practice (success returns Ok before classify);
///     conservatively transient.
///   - 408 / 429 → transient (treated like 5xx for retry purposes).
///   - 4xx-other → permanent (`Http4xx`).
///   - 5xx → transient (`Http5xx`).
///   - 1xx / 3xx unexpected → conservative transient (`Network`).
///   - reqwest network error → transient (`Network`).
///   - reqwest timeout → transient (`Timeout`).
///   - InvalidUrl / DispatchFailed / SerializationFailed → permanent (`Network`)
///     so the chain short-circuits.
pub fn classify(err: &WebhookError) -> Classification {
    match err {
        WebhookError::HttpStatus { code, .. } => {
            let c = *code;
            if (200..=299).contains(&c) {
                // Unreachable in practice — success returns Ok(()) before classify is called.
                Classification::Transient(DlqReason::Network)
            } else if c == 408 || c == 429 {
                // Treated like 5xx for retry purposes (D-06).
                Classification::Transient(DlqReason::Http5xx)
            } else if (400..=499).contains(&c) {
                Classification::Permanent(DlqReason::Http4xx)
            } else if (500..=599).contains(&c) {
                Classification::Transient(DlqReason::Http5xx)
            } else {
                // 1xx / 3xx unexpected — conservative transient.
                Classification::Transient(DlqReason::Network)
            }
        }
        WebhookError::Network(_) => Classification::Transient(DlqReason::Network),
        WebhookError::Timeout => Classification::Transient(DlqReason::Timeout),
        WebhookError::InvalidUrl(_)
        | WebhookError::DispatchFailed(_)
        | WebhookError::SerializationFailed(_) => Classification::Permanent(DlqReason::Network),
    }
}

/// Multiply a base delay by uniform `[0.8, 1.2)`. Per CONTEXT D-02 verbatim;
/// `rand::random::<f64>()` works in rand 0.10 (the global free function is
/// preserved across the 0.9 → 0.10 transition).
///
/// `pub` (not `pub(crate)`) so integration tests can call it directly per
/// the W2 visibility fix.
pub fn jitter(base: Duration) -> Duration {
    let factor = rand::random::<f64>() * 0.4 + 0.8;
    base.mul_f64(factor)
}

/// Per CONTEXT D-08 + RESEARCH §4.7. For a sleep that precedes attempt index
/// `slot`, the cap is `schedule[slot+1] * 1.2`; if no `slot+1` exists (last
/// attempt), reuse the previous slot's cap (`schedule[slot] * 1.2`).
///
/// `pub` so integration tests can call it directly.
pub fn cap_for_slot(slot: usize, schedule: &[Duration]) -> Duration {
    let base = schedule.get(slot + 1).copied().unwrap_or(schedule[slot]);
    base.mul_f64(1.2)
}

/// Parse the `Retry-After` header from a HeaderMap. Integer-seconds form only
/// (CONTEXT D-07); HTTP-date form returns None and emits a WARN. Used by
/// HttpDispatcher to populate the `retry_after` field of
/// `WebhookError::HttpStatus` before consuming the response body.
///
/// `pub` so integration tests can call it directly.
pub fn parse_retry_after_from_response(
    headers: &reqwest::header::HeaderMap,
    url: &str,
    status: u16,
) -> Option<Duration> {
    let header = headers.get(reqwest::header::RETRY_AFTER)?;
    let s = header.to_str().ok()?;
    match s.trim().parse::<u64>() {
        Ok(secs) => Some(Duration::from_secs(secs)),
        Err(_) => {
            tracing::warn!(
                target: "cronduit.webhooks",
                url = %url,
                status,
                retry_after = %s,
                "Retry-After header present but not integer-seconds (HTTP-date form not supported in v1.2); falling back to schedule"
            );
            None
        }
    }
}

/// Compute sleep delay before the attempt at index `next_attempt`, given the
/// last error's `retry_after` hint. Per CONTEXT.md D-08 (verbatim locked
/// contract: `cap = locked_schedule[next_attempt+1] * 1.2`):
///   - If `retry_after` is `None` → `jitter(schedule[next_attempt])`.
///   - If `retry_after` is `Some(ra)` →
///     `min(cap_for_slot(next_attempt, schedule), max(jitter(schedule[next_attempt]), ra))`.
///
/// The cap uses `next_attempt` because `cap_for_slot(slot)` is defined as
/// `schedule[slot+1] * 1.2` (with a last-slot fallback of `schedule[slot] * 1.2`).
/// For the sleep before attempt index `next_attempt`, D-08's contract is
/// "the next attempt's worst-case window times 1.2," which is exactly
/// `schedule[next_attempt+1] * 1.2 == cap_for_slot(next_attempt)`.
///
/// Phase 20 BL-02 history: a prior version passed `next_attempt - 1` here,
/// producing a 36s cap for the first inter-attempt sleep instead of the
/// documented 360s. The bug surfaced when a receiver returned
/// `Retry-After: 350` and the chain capped to 36s. Fixed per the locked D-08
/// contract; do not regress.
fn compute_sleep_delay(
    next_attempt: usize,
    schedule: &[Duration],
    retry_after: Option<Duration>,
) -> Duration {
    let base = jitter(schedule[next_attempt]);
    match retry_after {
        None => base,
        Some(ra) => {
            // BL-02 fix: pass `next_attempt` (not `next_attempt - 1`). D-08
            // locked formula: cap = schedule[next_attempt + 1] * 1.2,
            // which is exactly cap_for_slot(next_attempt) by definition.
            let cap = cap_for_slot(next_attempt, schedule);
            std::cmp::min(cap, std::cmp::max(base, ra))
        }
    }
}

/// Truncate `last_error` to <= MAX chars at the call site (per CONTEXT D-10).
/// `WebhookDlqRow` itself does not enforce the limit so the helper stays one
/// place that is easy to audit.
fn truncate_error(s: &str) -> String {
    const MAX: usize = 500;
    if s.chars().count() <= MAX {
        s.to_string()
    } else {
        s.chars().take(MAX).collect()
    }
}

/// RetryingDispatcher — Phase 20 / WH-05 composition wrapper around any
/// `WebhookDispatcher` (in practice `HttpDispatcher`). Adds the 3-attempt
/// in-memory retry chain (D-01..D-12).
///
/// `webhooks: Arc<HashMap<i64, WebhookConfig>>` is required for D-10/D-12 —
/// the DLQ row's `url` column is looked up by `event.job_id` at write time.
/// The Arc is shared with `HttpDispatcher` so both dispatchers see the same
/// config table.
pub struct RetryingDispatcher<D: WebhookDispatcher> {
    inner: D,
    pool: DbPool,
    cancel: CancellationToken,
    webhooks: Arc<HashMap<i64, WebhookConfig>>,
    schedule: [Duration; 3],
}

impl<D: WebhookDispatcher> RetryingDispatcher<D> {
    /// Construct a RetryingDispatcher.
    ///
    /// - `inner` — the wrapped dispatcher (typically `HttpDispatcher`).
    /// - `pool` — DB pool used for the DLQ INSERT path.
    /// - `cancel` — cancellation token; cancelled mid-sleep writes a
    ///   `shutdown_drain` DLQ row and returns `Err(DispatchFailed("shutdown drain"))`.
    /// - `webhooks` — shared per-job webhook config map; used at DLQ-write time
    ///   to populate the `url` column (D-10/D-12).
    #[allow(dead_code)] // Plan 06 wires this into the worker startup path.
    pub fn new(
        inner: D,
        pool: DbPool,
        cancel: CancellationToken,
        webhooks: Arc<HashMap<i64, WebhookConfig>>,
    ) -> Self {
        Self {
            inner,
            pool,
            cancel,
            webhooks,
            schedule: [
                Duration::ZERO,           // attempt 1: t=0
                Duration::from_secs(30),  // attempt 2: t=30s
                Duration::from_secs(300), // attempt 3: t=300s
            ],
        }
    }

    /// Write one DLQ row capturing the terminal-failure context for this run.
    /// Per RESEARCH §4.8: a failed INSERT logs at WARN and is NOT promoted to
    /// a worker crash — the DLQ is the audit trail; the metric is the source
    /// of truth.
    async fn write_dlq(
        &self,
        event: &RunFinalized,
        attempts: i64,
        last_status: Option<i64>,
        last_error: Option<String>,
        reason: DlqReason,
        first_attempt_at: &str,
    ) {
        // D-10/D-12: url is the configured webhook URL for this job_id.
        // The unwrap_or_default fallback handles the should-not-happen race
        // where the job's webhook config was removed mid-run; in that case
        // log WARN and persist an empty string so the row still inserts.
        let url = match self.webhooks.get(&event.job_id) {
            Some(cfg) => cfg.url.clone(),
            None => {
                tracing::warn!(
                    target: "cronduit.webhooks",
                    run_id = event.run_id,
                    job_id = event.job_id,
                    "webhook config not found at DLQ-write time; storing empty url"
                );
                String::new()
            }
        };

        let row = WebhookDlqRow {
            run_id: event.run_id,
            job_id: event.job_id,
            url,
            attempts,
            last_status,
            last_error,
            dlq_reason: reason.as_str().to_string(),
            first_attempt_at: first_attempt_at.to_string(),
            last_attempt_at: Utc::now().to_rfc3339(),
        };

        if let Err(e) = queries::insert_webhook_dlq_row(&self.pool, row).await {
            tracing::warn!(
                target: "cronduit.webhooks",
                run_id = event.run_id,
                job_id = event.job_id,
                dlq_reason = reason.as_str(),
                error = %e,
                "DLQ insert failed; metric still incremented"
            );
        }
    }
}

#[async_trait]
impl<D: WebhookDispatcher> WebhookDispatcher for RetryingDispatcher<D> {
    async fn deliver(&self, event: &RunFinalized) -> Result<(), WebhookError> {
        let first_attempt_at = Utc::now().to_rfc3339();
        let mut last_class: Option<DlqReason> = None;
        let mut last_status: Option<i64> = None;
        let mut last_error: Option<String> = None;
        let mut last_retry_after: Option<Duration> = None;
        let mut attempts: i64 = 0;

        for i in 0..self.schedule.len() {
            // Sleep BEFORE attempt 2 and 3 (i==1 and i==2). Attempt 1 is t=0.
            if i > 0 {
                let delay = compute_sleep_delay(i, &self.schedule, last_retry_after);
                tokio::select! {
                    biased;
                    _ = tokio::time::sleep(delay) => {}
                    _ = self.cancel.cancelled() => {
                        // D-03: cancel-fire mid-sleep → DLQ shutdown_drain row.
                        // attempts so far reflect actual count (NOT always 3).
                        self.write_dlq(
                            event,
                            attempts,
                            last_status,
                            last_error.clone(),
                            DlqReason::ShutdownDrain,
                            &first_attempt_at,
                        ).await;
                        // Phase 20 / WH-11 / D-22: per-delivery terminal-failure
                        // counter (closed-enum status="failed") at the chain-
                        // terminal boundary. ShutdownDrain is a terminal failure
                        // outcome from the metric's perspective — operators
                        // disambiguate via dlq_reason in the SQL audit table.
                        metrics::counter!(
                            "cronduit_webhook_deliveries_total",
                            "job" => event.job_name.clone(),
                            "status" => "failed",
                        )
                        .increment(1);
                        return Err(WebhookError::DispatchFailed("shutdown drain".into()));
                    }
                }
            }

            attempts = (i + 1) as i64;
            // Per Pitfall 1 + RESEARCH §6.5: the inner deliver is NOT in a
            // tokio::select! — in-flight HTTP requests run to completion
            // (capped by reqwest's existing 10s per-attempt timeout, P18 D-18).
            // Cancel checks happen ONLY on the sleep-between-attempts boundary.
            match self.inner.deliver(event).await {
                Ok(()) => {
                    // Phase 20 / WH-11 / D-22: per-delivery success counter at
                    // the chain-success boundary (NOT per-attempt). Closed-enum
                    // status="success" — never a runtime string.
                    metrics::counter!(
                        "cronduit_webhook_deliveries_total",
                        "job" => event.job_name.clone(),
                        "status" => "success",
                    )
                    .increment(1);
                    return Ok(());
                }
                Err(e) => {
                    let cls = classify(&e);
                    // Capture per-variant fields BEFORE matching takes ownership.
                    match &e {
                        WebhookError::HttpStatus {
                            code,
                            retry_after,
                            body_preview,
                        } => {
                            last_status = Some(*code as i64);
                            // Phase 20 / WH-05 / BL-03: persist the truncated body
                            // preview into webhook_deliveries.last_error so http_5xx
                            // DLQ rows carry diagnostic value (CONTEXT D-10).
                            // truncate_error caps at 500 chars; HttpDispatcher already
                            // capped at 200 — the second pass is defense-in-depth.
                            last_error = body_preview.as_ref().map(|s| truncate_error(s));
                            last_retry_after = *retry_after;
                        }
                        WebhookError::Timeout => {
                            last_status = None;
                            last_error = Some(truncate_error("timeout"));
                            last_retry_after = None;
                        }
                        WebhookError::Network(msg) => {
                            last_status = None;
                            last_error = Some(truncate_error(msg));
                            last_retry_after = None;
                        }
                        WebhookError::InvalidUrl(msg) => {
                            last_status = None;
                            last_error = Some(truncate_error(msg));
                            last_retry_after = None;
                        }
                        WebhookError::DispatchFailed(msg) => {
                            last_status = None;
                            last_error = Some(truncate_error(msg));
                            last_retry_after = None;
                        }
                        WebhookError::SerializationFailed(msg) => {
                            last_status = None;
                            last_error = Some(truncate_error(msg));
                            last_retry_after = None;
                        }
                    }
                    last_class = Some(match cls {
                        Classification::Permanent(r) | Classification::Transient(r) => r,
                    });

                    if matches!(cls, Classification::Permanent(_)) {
                        break; // 4xx-permanent — no retry
                    }
                    if i == self.schedule.len() - 1 {
                        break; // exhausted
                    }
                    // else: continue loop; next iteration sleeps + retries
                }
            }
        }

        // Terminal failure — write one DLQ row with the last classification.
        let reason = last_class.unwrap_or(DlqReason::Network);
        self.write_dlq(
            event,
            attempts,
            last_status,
            last_error,
            reason,
            &first_attempt_at,
        )
        .await;
        // Phase 20 / WH-11 / D-22: per-delivery terminal-failure counter at the
        // chain-terminal boundary (4xx-permanent OR retry-exhausted-transient).
        // Closed-enum status="failed" — reason granularity (4xx vs 5xx vs
        // network vs timeout) lives in webhook_deliveries.dlq_reason, NOT in
        // metric labels (T-20-05 cardinality mitigation).
        metrics::counter!(
            "cronduit_webhook_deliveries_total",
            "job" => event.job_name.clone(),
            "status" => "failed",
        )
        .increment(1);
        Err(WebhookError::DispatchFailed("retry exhausted".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_in_range() {
        let base = Duration::from_secs(30);
        for _ in 0..1000 {
            let j = jitter(base);
            let factor = j.as_secs_f64() / base.as_secs_f64();
            assert!(
                (0.8..1.2).contains(&factor),
                "jitter factor {factor} out of [0.8, 1.2)"
            );
        }
    }

    #[test]
    fn cap_for_slot_math() {
        let schedule = [
            Duration::ZERO,
            Duration::from_secs(30),
            Duration::from_secs(300),
        ];
        // slot 0 → schedule[1] = 30s × 1.2 = 36s
        assert_eq!(cap_for_slot(0, &schedule), Duration::from_secs_f64(36.0));
        // slot 1 → schedule[2] = 300s × 1.2 = 360s
        assert_eq!(cap_for_slot(1, &schedule), Duration::from_secs_f64(360.0));
        // slot 2 → no slot 3, fallback to schedule[2] × 1.2 = 360s
        assert_eq!(cap_for_slot(2, &schedule), Duration::from_secs_f64(360.0));
    }

    #[test]
    fn classify_response_table() {
        // Helper to build HttpStatus { code, retry_after: None }
        let st = |c: u16| WebhookError::HttpStatus {
            code: c,
            retry_after: None,
            body_preview: None,
        };
        // 4xx-other → permanent http_4xx
        assert_eq!(
            classify(&st(404)),
            Classification::Permanent(DlqReason::Http4xx)
        );
        assert_eq!(
            classify(&st(403)),
            Classification::Permanent(DlqReason::Http4xx)
        );
        assert_eq!(
            classify(&st(401)),
            Classification::Permanent(DlqReason::Http4xx)
        );
        // 408/429 → transient (treated like 5xx)
        assert!(matches!(classify(&st(408)), Classification::Transient(_)));
        assert!(matches!(classify(&st(429)), Classification::Transient(_)));
        // 5xx → transient http_5xx
        assert_eq!(
            classify(&st(500)),
            Classification::Transient(DlqReason::Http5xx)
        );
        assert_eq!(
            classify(&st(502)),
            Classification::Transient(DlqReason::Http5xx)
        );
        assert_eq!(
            classify(&st(503)),
            Classification::Transient(DlqReason::Http5xx)
        );
        // network/timeout → transient
        assert_eq!(
            classify(&WebhookError::Network("conn refused".into())),
            Classification::Transient(DlqReason::Network)
        );
        assert_eq!(
            classify(&WebhookError::Timeout),
            Classification::Transient(DlqReason::Timeout)
        );
    }

    #[test]
    fn classify_invalid_url_is_permanent() {
        let err = WebhookError::InvalidUrl("not a url".into());
        assert!(matches!(classify(&err), Classification::Permanent(_)));
    }

    #[test]
    fn compute_sleep_delay_no_retry_after_uses_jitter() {
        let schedule = [
            Duration::ZERO,
            Duration::from_secs(30),
            Duration::from_secs(300),
        ];
        // No Retry-After → roughly schedule[1] (with jitter 0.8..1.2)
        let d = compute_sleep_delay(1, &schedule, None);
        let lo = Duration::from_secs_f64(24.0);
        let hi = Duration::from_secs_f64(36.0);
        assert!(
            d >= lo && d < hi,
            "no-retry-after delay should be ~30s ± 20%; got {d:?}"
        );
    }

    #[test]
    fn compute_sleep_delay_caps_retry_after_at_slot_cap() {
        // BL-02 regression lock: per CONTEXT D-08, cap = schedule[next_attempt+1]*1.2.
        // For next_attempt=1 the cap is schedule[2]*1.2 = 360s (NOT schedule[1]*1.2 = 36s
        // as the previous buggy implementation produced). This test asserts the
        // POST-FIX semantics; if it fails with `left: 36s, right: 360s` the BL-02
        // off-by-one has regressed.
        let schedule = [
            Duration::ZERO,
            Duration::from_secs(30),
            Duration::from_secs(300),
        ];
        // For sleep before attempt 1 (next_attempt=1):
        //   cap = cap_for_slot(1) = schedule[2]*1.2 = 360s. Retry-After:9999 → 360s.
        let d = compute_sleep_delay(1, &schedule, Some(Duration::from_secs(9999)));
        assert_eq!(
            d,
            Duration::from_secs_f64(360.0),
            "Retry-After: 9999 must be capped at cap_for_slot(next_attempt=1) = 360s per D-08"
        );
        // For sleep before attempt 2 (next_attempt=2):
        //   cap = cap_for_slot(2) = last-slot fallback schedule[2]*1.2 = 360s.
        let d = compute_sleep_delay(2, &schedule, Some(Duration::from_secs(9999)));
        assert_eq!(
            d,
            Duration::from_secs_f64(360.0),
            "Retry-After: 9999 must be capped at cap_for_slot(next_attempt=2) last-slot fallback = 360s"
        );
    }

    #[test]
    fn compute_sleep_delay_first_sleep_uses_attempt_2_cap_per_d08() {
        // BL-02 D-08 lock: a receiver returning Retry-After: 350 between attempts 1
        // and 2 must have the 350s honored end-to-end (350 < cap_for_slot(1) = 360).
        // Pre-fix code capped at cap_for_slot(0) = 36s, silently truncating 350 → 36.
        let schedule = [
            Duration::ZERO,
            Duration::from_secs(30),
            Duration::from_secs(300),
        ];
        let d = compute_sleep_delay(1, &schedule, Some(Duration::from_secs(350)));
        assert_eq!(
            d,
            Duration::from_secs(350),
            "Retry-After:350 between attempts 1 and 2 must be honored exactly \
             (within cap_for_slot(1)=360, above jitter floor schedule[1]*0.8=24s). \
             If this asserts 36s, BL-02 has regressed."
        );
    }

    #[test]
    fn compute_sleep_delay_honors_retry_after_within_cap() {
        let schedule = [
            Duration::ZERO,
            Duration::from_secs(30),
            Duration::from_secs(300),
        ];
        // For sleep before attempt 2 (next_attempt=2), prev_slot=1, cap=360s.
        // Retry-After: 350s exceeds the jitter floor of schedule[2]*0.8 = 240s
        // and is within cap, so the result must be exactly 350s.
        let d = compute_sleep_delay(2, &schedule, Some(Duration::from_secs(350)));
        assert_eq!(
            d,
            Duration::from_secs(350),
            "Retry-After: 350 (within cap_for_slot(1)=360 and > jitter floor) must be honored exactly"
        );
    }

    #[test]
    fn parse_retry_after_integer_seconds() {
        use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};
        let mut h = HeaderMap::new();
        h.insert(RETRY_AFTER, HeaderValue::from_static("60"));
        assert_eq!(
            parse_retry_after_from_response(&h, "http://test/", 429),
            Some(Duration::from_secs(60))
        );
        // Zero is a valid integer-seconds form.
        let mut h = HeaderMap::new();
        h.insert(RETRY_AFTER, HeaderValue::from_static("0"));
        assert_eq!(
            parse_retry_after_from_response(&h, "http://test/", 429),
            Some(Duration::from_secs(0))
        );
    }

    #[test]
    fn parse_retry_after_http_date_returns_none() {
        use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};
        let mut h = HeaderMap::new();
        h.insert(
            RETRY_AFTER,
            HeaderValue::from_static("Wed, 01 May 2026 12:00:00 GMT"),
        );
        assert_eq!(
            parse_retry_after_from_response(&h, "http://test/", 429),
            None
        );
    }

    #[test]
    fn parse_retry_after_negative_returns_none() {
        use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};
        let mut h = HeaderMap::new();
        h.insert(RETRY_AFTER, HeaderValue::from_static("-5"));
        assert_eq!(
            parse_retry_after_from_response(&h, "http://test/", 429),
            None
        );
    }

    #[test]
    fn parse_retry_after_missing_returns_none() {
        use reqwest::header::HeaderMap;
        let h = HeaderMap::new();
        assert_eq!(
            parse_retry_after_from_response(&h, "http://test/", 200),
            None
        );
    }

    #[test]
    fn dlq_reason_string_values() {
        assert_eq!(DlqReason::Http4xx.as_str(), "http_4xx");
        assert_eq!(DlqReason::Http5xx.as_str(), "http_5xx");
        assert_eq!(DlqReason::Network.as_str(), "network");
        assert_eq!(DlqReason::Timeout.as_str(), "timeout");
        assert_eq!(DlqReason::ShutdownDrain.as_str(), "shutdown_drain");
    }

    #[test]
    fn truncate_error_under_limit_unchanged() {
        let s = "short error";
        assert_eq!(truncate_error(s), s);
    }

    #[test]
    fn truncate_error_over_limit_is_500_chars() {
        let s = "x".repeat(1000);
        let t = truncate_error(&s);
        assert_eq!(t.chars().count(), 500);
    }
}
