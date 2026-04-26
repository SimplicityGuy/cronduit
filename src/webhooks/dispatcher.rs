//! WebhookDispatcher trait — the seam P18's HttpDispatcher implements
//! against. Phase 15 ships only NoopDispatcher; the worker calls dispatcher
//! by trait object so swapping implementations in P18 requires zero changes
//! to the worker loop body.

use async_trait::async_trait;
use thiserror::Error;

use super::event::RunFinalized;

#[derive(Debug, Error)]
pub enum WebhookError {
    /// Stub variant for P18 to expand. P15 never produces this — the
    /// `NoopDispatcher` always returns `Ok(())`.
    #[error("webhook dispatch failed: {0}")]
    DispatchFailed(String),
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
