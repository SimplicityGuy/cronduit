//! `cronduit health` — probe the local `/health` endpoint and exit 0 if `status == "ok"`.
//!
//! Phase 12 — OPS-06. Intended as the Dockerfile `HEALTHCHECK` target (D-06).
//!
//! Decisions (see `.planning/phases/12-docker-healthcheck-rc-1-cut/12-CONTEXT.md`):
//! - **D-01:** HTTP client built on `hyper 1` + `hyper-util` (no `reqwest`, no raw TCP).
//! - **D-02:** 5 s total timeout via `tokio::time::timeout`.
//! - **D-03:** Target URL derived from the global `--bind` flag, defaulting to `127.0.0.1:8080`.
//! - **D-04:** Does NOT read `--config`; no TOML parsing in the health path.
//! - **D-05:** Exit `0` iff HTTP 200 AND body parses as JSON AND `body.status == "ok"`. Exit `1`
//!   on connect-refused, DNS failure, timeout, non-200, unparseable body, or `status != "ok"`.
//!
//! Skeleton lands in Plan 12-01; the hyper-util client + body parse + exit-code logic + 7
//! unit tests (per D-14) land in Plan 12-02.

use crate::cli::Cli;

/// Skeleton placeholder. Plan 12-02 replaces the body with the real hyper-util probe.
pub async fn execute(_cli: &Cli) -> anyhow::Result<i32> {
    // Phase 12 Plan 12-02 will implement:
    //   1. Build URL: format!("http://{}/health", cli.bind.as_deref().unwrap_or("127.0.0.1:8080"))
    //   2. Construct hyper-util client (HttpConnector + TokioExecutor + HTTP/1).
    //   3. tokio::time::timeout(5s, client.request(req)).await
    //   4. Check status() == 200 and JSON body.status == "ok".
    //   5. Return Ok(0) on success, Ok(1) (with tracing::error! to stderr) on any failure mode.
    //   6. #[cfg(test)] mod tests — 7 cases per VALIDATION.md (12-02-01..07).
    Ok(0)
}
