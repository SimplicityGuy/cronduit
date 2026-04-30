//! WebhookDispatcher trait + Phase 15 NoopDispatcher + Phase 18 HttpDispatcher.
//!
//! Phase 15 shipped the trait + NoopDispatcher; the worker calls dispatcher
//! by trait object so swapping implementations in P18 requires zero changes
//! to the worker loop body. Phase 18 (this file) lands `HttpDispatcher`
//! alongside `NoopDispatcher`, plus the `sign_v1` HMAC helper and the
//! `should_fire` pure decision function (D-16 coalesce). Wire-up to the
//! worker is Plan 05.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use secrecy::{ExposeSecret, SecretString};
use sha2::Sha256;
use thiserror::Error;

use super::event::RunFinalized;
use crate::config::WebhookConfig;
use crate::db::DbPool;

#[derive(Debug, Error)]
pub enum WebhookError {
    /// Generic dispatch failure. Phase 15 only emitted `Ok(())` from
    /// NoopDispatcher; Phase 18 keeps this as the primary log-side variant
    /// so the worker_loop's pattern-match stays simple.
    #[error("webhook dispatch failed: {0}")]
    DispatchFailed(String),
    // Phase 18 — explicit variants for richer Phase 20 retry decisioning.
    // For Phase 18 they are largely funneled through the existing
    // `DispatchFailed` log path so the worker_loop's pattern-match at
    // src/webhooks/worker.rs stays simple. Phase 20 RetryingDispatcher
    // surfaces these distinctly.
    #[allow(dead_code)] // Phase 20 RetryingDispatcher consumes.
    #[error("webhook HTTP non-2xx: status={0}")]
    HttpStatus(u16),
    #[allow(dead_code)] // Phase 20 RetryingDispatcher consumes.
    #[error("webhook network error: {0}")]
    Network(String),
    #[allow(dead_code)] // Phase 20 RetryingDispatcher consumes.
    #[error("webhook request timed out")]
    Timeout,
    #[allow(dead_code)] // Phase 20 RetryingDispatcher consumes.
    #[error("invalid webhook URL: {0}")]
    InvalidUrl(String),
    #[error("webhook payload serialization failed: {0}")]
    SerializationFailed(String),
}

#[async_trait]
pub trait WebhookDispatcher: Send + Sync {
    async fn deliver(&self, event: &RunFinalized) -> Result<(), WebhookError>;
}

/// Always-on default dispatcher. Logs at debug and returns `Ok(())`.
pub struct NoopDispatcher;

#[async_trait]
impl WebhookDispatcher for NoopDispatcher {
    async fn deliver(&self, event: &RunFinalized) -> Result<(), WebhookError> {
        tracing::debug!(
            target: "cronduit.webhooks",
            run_id = event.run_id,
            job_id = event.job_id,
            status = %event.status,
            "noop webhook dispatch"
        );
        Ok(())
    }
}

/// Phase 18 / WH-03 HttpDispatcher — implements `WebhookDispatcher` for real
/// HTTP delivery to operator-supplied URLs.
///
/// Owns:
/// - one shared `reqwest::Client` (connection-pooled per RESEARCH Pattern 6)
/// - `DbPool` reader for `get_failure_context` + `get_run_by_id` + filter-position
/// - `Arc<HashMap<i64, WebhookConfig>>` keyed by `job_id` — built once at
///   startup by `src/cli/run.rs` (Plan 05). Phase 18 ships static config
///   only; reload semantics are Phase 20.
///
/// D-19 concurrency posture: dispatcher serves a SINGLE worker task (per
/// WH-02) and issues HTTP requests serially within that task — one outbound
/// delivery at a time. No internal concurrency primitive (no semaphore,
/// no JoinSet, no spawn-per-delivery). Phase 20+ can wrap with a semaphore
/// via composition (D-21) without touching this impl.
pub struct HttpDispatcher {
    client: reqwest::Client,
    pool: DbPool,
    webhooks: Arc<HashMap<i64, WebhookConfig>>,
    cronduit_version: &'static str,
}

impl HttpDispatcher {
    /// Construct an HttpDispatcher. Returns `WebhookError::DispatchFailed` if
    /// `reqwest::Client::build()` fails (TLS provider init, etc.).
    #[allow(dead_code)] // Phase 18 Plan 05 wire-up consumes.
    pub fn new(
        pool: DbPool,
        webhooks: Arc<HashMap<i64, WebhookConfig>>,
    ) -> Result<Self, WebhookError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10)) // D-18 hard-code
            .pool_idle_timeout(Some(Duration::from_secs(90))) // keep-alive across deliveries
            .user_agent(format!("cronduit/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| WebhookError::DispatchFailed(format!("reqwest client init: {e}")))?;
        Ok(Self {
            client,
            pool,
            webhooks,
            cronduit_version: env!("CARGO_PKG_VERSION"),
        })
    }
}

/// Coalesce decision per D-16. Pure + crate-public for testability.
///
/// `fire_every == 0` always fires; `fire_every == 1` is the first-of-stream
/// special case (filter_position == 1); `fire_every > 1` fires when
/// `filter_position % fire_every == 1` (1, N+1, 2N+1, ...).
/// Returns false on `fire_every < 0` (defensively — validator catches at LOAD).
pub(crate) fn should_fire(fire_every: i64, filter_position: i64) -> bool {
    match fire_every {
        0 => true,
        1 => filter_position == 1,
        n if n > 1 => filter_position > 0 && filter_position % n == 1,
        _ => false,
    }
}

/// Standard Webhooks v1 HMAC-SHA256 sign helper. Produces the base64-STANDARD
/// (with `=` padding) string for `${webhook-id}.${webhook-timestamp}.${body}`.
/// `body` is the EXACT bytes that will be sent (Pitfall B mitigation).
pub(crate) fn sign_v1(
    secret: &SecretString,
    webhook_id: &str,
    webhook_timestamp: i64,
    body: &[u8],
) -> String {
    let prefix = format!("{webhook_id}.{webhook_timestamp}.");
    // hmac 0.13: `new_from_slice` is on `KeyInit`. Importing `KeyInit` at
    // the top of the file makes `Hmac::<Sha256>::new_from_slice` resolve
    // through the trait. The plan's `<Hmac<Sha256> as Mac>::new_from_slice`
    // form fails to compile under hmac 0.13 — `new_from_slice` is NOT on
    // `Mac` — so we use the prelude-resolved form (Rule 3 fix).
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.expose_secret().as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(prefix.as_bytes());
    mac.update(body);
    let bytes = mac.finalize().into_bytes();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[async_trait]
impl WebhookDispatcher for HttpDispatcher {
    async fn deliver(&self, event: &RunFinalized) -> Result<(), WebhookError> {
        // 1. Look up per-job webhook config; absent => skip silently.
        let Some(cfg) = self.webhooks.get(&event.job_id) else {
            tracing::debug!(
                target: "cronduit.webhooks",
                run_id = event.run_id, job_id = event.job_id,
                "skip delivery: no webhook configured for job"
            );
            return Ok(());
        };

        // 2. Filter by states; non-match => skip silently.
        if !cfg.states.iter().any(|s| s == &event.status) {
            tracing::debug!(
                target: "cronduit.webhooks",
                run_id = event.run_id, job_name = %event.job_name,
                status = %event.status,
                "skip delivery: status not in configured states"
            );
            return Ok(());
        }

        // 3. Compute filter-matching stream position.
        let filter_position = crate::webhooks::coalesce::filter_position(
            &self.pool,
            event.job_id,
            &event.started_at,
            &cfg.states,
        )
        .await
        .map_err(|e| WebhookError::DispatchFailed(format!("filter_position: {e}")))?;

        // 4. Coalesce decision (D-16).
        if !should_fire(cfg.fire_every, filter_position) {
            tracing::debug!(
                target: "cronduit.webhooks",
                run_id = event.run_id, job_name = %event.job_name,
                filter_position, fire_every = cfg.fire_every,
                "skip delivery: coalesced"
            );
            return Ok(());
        }

        // 5. Read failure context (Phase 16 helper).
        let fctx = crate::db::queries::get_failure_context(&self.pool, event.job_id)
            .await
            .map_err(|e| WebhookError::DispatchFailed(format!("get_failure_context: {e}")))?;

        // 6. Read current-run metadata for image_digest + config_hash. The
        //    plan references `get_run_detail`; the actual project helper is
        //    `get_run_by_id` returning `Option<DbRunDetail>` (Rule 3 fix:
        //    use the existing function and surface a clear error if the row
        //    is missing — should be impossible at this point because the
        //    scheduler just inserted the finalize row).
        let run_detail = crate::db::queries::get_run_by_id(&self.pool, event.run_id)
            .await
            .map_err(|e| WebhookError::DispatchFailed(format!("get_run_by_id: {e}")))?
            .ok_or_else(|| {
                WebhookError::DispatchFailed(format!(
                    "run {} disappeared between finalize and webhook dispatch",
                    event.run_id
                ))
            })?;

        // 7. Build payload + serialize ONCE into Vec<u8> (Pitfall B).
        let payload = crate::webhooks::payload::WebhookPayload::build(
            event,
            &fctx,
            &run_detail,
            filter_position,
            self.cronduit_version,
        );
        let body_bytes = serde_json::to_vec(&payload)
            .map_err(|e| WebhookError::SerializationFailed(format!("payload to_vec: {e}")))?;

        // 8. Build headers — Standard Webhooks v1 spec (D-09, D-11).
        let webhook_id = ulid::Ulid::new().to_string();
        let webhook_ts = chrono::Utc::now().timestamp(); // 10-digit Unix seconds (Pitfall D)

        let mut req = self
            .client
            .post(&cfg.url)
            .header("content-type", "application/json") // D-11
            .header("webhook-id", &webhook_id)
            .header("webhook-timestamp", webhook_ts.to_string());

        // 9. Sign IF not unsigned (D-05).
        if !cfg.unsigned {
            let secret = cfg
                .secret
                .as_ref()
                .expect("validator guarantees secret is Some when unsigned == false");
            let sig = sign_v1(secret, &webhook_id, webhook_ts, &body_bytes);
            req = req.header("webhook-signature", format!("v1,{sig}"));
        }

        // 10. Send the EXACT body_bytes (Pitfall B — same Vec<u8> we signed).
        let response = req.body(body_bytes).send().await;

        // 11. Classify + record metrics.
        match response {
            Ok(resp) if resp.status().is_success() => {
                metrics::counter!("cronduit_webhook_delivery_sent_total").increment(1);
                tracing::debug!(
                    target: "cronduit.webhooks",
                    run_id = event.run_id, job_name = %event.job_name,
                    url = %cfg.url, status = resp.status().as_u16(),
                    "webhook delivered"
                );
                Ok(())
            }
            Ok(resp) => {
                metrics::counter!("cronduit_webhook_delivery_failed_total").increment(1);
                let status = resp.status().as_u16();
                let body_preview = resp.text().await.unwrap_or_default();
                let truncated: String = body_preview.chars().take(200).collect();
                tracing::warn!(
                    target: "cronduit.webhooks",
                    run_id = event.run_id, job_name = %event.job_name,
                    url = %cfg.url, status, body_preview = %truncated,
                    "webhook non-2xx"
                );
                Ok(()) // Phase 18 posture per D-21; never propagate to worker
            }
            Err(e) => {
                metrics::counter!("cronduit_webhook_delivery_failed_total").increment(1);
                let kind = if e.is_timeout() {
                    "timeout"
                } else if e.is_connect() {
                    "connect"
                } else {
                    "network"
                };
                tracing::warn!(
                    target: "cronduit.webhooks",
                    run_id = event.run_id, job_name = %event.job_name,
                    url = %cfg.url, kind, error = %e,
                    "webhook network error"
                );
                Ok(()) // Phase 18 posture per D-21
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_v1_known_fixture() {
        let secret = SecretString::from("shh");
        let id = "01HZAFY0V1F1BS1F2H8GV4XG3R";
        let ts: i64 = 1_761_744_191;
        let body = br#"{"hello":"world"}"#;
        let sig = sign_v1(&secret, id, ts, body);

        // Compute expected by an independent HMAC over the SAME prefix+body.
        let prefix = format!("{id}.{ts}.");
        let mut mac = Hmac::<Sha256>::new_from_slice(b"shh").unwrap();
        mac.update(prefix.as_bytes());
        mac.update(body);
        let expected =
            base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());

        assert_eq!(sig, expected);
        assert!(!sig.is_empty());
    }

    #[test]
    fn signature_uses_standard_base64_alphabet() {
        // 200 random invocations — none should contain URL-safe chars.
        // rand 0.10 API: `rand::rng()` + `Rng` trait + `fill_bytes`.
        use rand::Rng;
        let mut rng = rand::rng();
        for _ in 0..200 {
            let mut body = [0u8; 64];
            rng.fill_bytes(&mut body);
            let sig = sign_v1(
                &SecretString::from("k"),
                &ulid::Ulid::new().to_string(),
                chrono::Utc::now().timestamp(),
                &body,
            );
            assert!(
                !sig.contains('-'),
                "URL-safe `-` in signature (Pitfall E): {sig}"
            );
            assert!(
                !sig.contains('_'),
                "URL-safe `_` in signature (Pitfall E): {sig}"
            );
            assert!(
                sig.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
            );
        }
    }

    #[test]
    fn signature_value_is_v1_comma_b64() {
        let sig = sign_v1(&SecretString::from("k"), "id", 1_761_744_191, b"body");
        let header_value = format!("v1,{sig}");
        assert!(header_value.starts_with("v1,"));
        // Decode the suffix; should produce 32 bytes (HMAC-SHA256 output length).
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&header_value["v1,".len()..])
            .unwrap();
        assert_eq!(bytes.len(), 32, "HMAC-SHA256 output is 32 bytes");
    }

    #[test]
    fn webhook_id_is_26char_ulid() {
        let id = ulid::Ulid::new().to_string();
        assert_eq!(id.len(), 26, "ULID string form is 26 chars");
        // Crockford base32 — no I, L, O, U.
        for c in id.chars() {
            assert!(c.is_ascii_alphanumeric());
        }
    }

    #[test]
    fn webhook_timestamp_is_10digit_seconds() {
        let ts = chrono::Utc::now().timestamp();
        assert_eq!(
            ts.to_string().len(),
            10,
            "Unix seconds — Pitfall D forbids millis (13 digits)"
        );
    }

    #[test]
    fn coalesce_decision_matrix() {
        // (fire_every, filter_position) -> expected
        for &(fe, pos, expected) in &[
            (0, 1, true),
            (0, 5, true),
            (0, 100, true),
            (1, 1, true),
            (1, 2, false),
            (1, 5, false),
            (3, 1, true),
            (3, 2, false),
            (3, 3, false),
            (3, 4, true),
            (3, 7, true),
            (5, 1, true),
            (5, 6, true),
            (5, 11, true),
            (5, 2, false),
            (1, 0, false), // defensive: position 0 means current didn't match
        ] {
            assert_eq!(
                should_fire(fe, pos),
                expected,
                "should_fire({fe}, {pos}) expected {expected}"
            );
        }
    }
}
