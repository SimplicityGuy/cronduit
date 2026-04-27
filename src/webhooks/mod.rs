//! Webhook delivery worker (Phase 15 / WH-02).
//!
//! Bounded mpsc + dedicated tokio task pattern: the scheduler emits
//! `RunFinalized` events via `try_send` (NEVER `send().await`); the worker
//! consumes them and dispatches via the `WebhookDispatcher` trait. Phase 15
//! ships only `NoopDispatcher`; P18 swaps in `HttpDispatcher` against the
//! same trait.
//!
//! ```mermaid
//! flowchart LR
//!     SCHED[scheduler<br/>finalize_run] -->|try_send| CHAN[(mpsc bounded 1024)]
//!     CHAN --> WORKER[worker_loop<br/>tokio::select!]
//!     WORKER --> DISP[dyn WebhookDispatcher]
//!     SCHED -->|TrySendError::Full| METRIC[cronduit_webhook_delivery_dropped_total ++]
//! ```

pub mod dispatcher;
pub mod event;
pub mod worker;

pub use dispatcher::{NoopDispatcher, WebhookDispatcher, WebhookError};
pub use event::RunFinalized;
pub use worker::{CHANNEL_CAPACITY, channel, channel_with_capacity, spawn_worker};
