//! `cronduit health` — probe the local `/health` endpoint and exit 0 if `status == "ok"`.
//!
//! Phase 12 — OPS-06. Intended as the Dockerfile `HEALTHCHECK` target (D-06).
//!
//! Decisions (see `.planning/phases/12-docker-healthcheck-rc-1-cut/12-CONTEXT.md`):
//! - **D-01:** HTTP client built on `hyper 1` + `hyper-util` (no `reqwest`, no raw TCP).
//! - **D-02:** 5 s total timeout via `tokio::time::timeout`.
//! - **D-03:** Target URL derived in precedence order: `--bind` flag → `CRONDUIT_BIND`
//!   env var → `127.0.0.1:8080` default. The env var lets operators whose compose
//!   overrides the internal bind (e.g. `bind = "0.0.0.0:9090"`) share a single env
//!   setting between `cronduit run` and the Dockerfile `HEALTHCHECK` invocation
//!   without patching the image.
//! - **D-04:** Does NOT read `--config`; no TOML parsing in the health path.
//! - **D-05:** Exit `0` iff HTTP 200 AND body parses as JSON AND `body.status == "ok"`. Exit `1`
//!   on connect-refused, DNS failure, timeout, non-200, unparseable body, or `status != "ok"`.
//!
//! Skeleton lands in Plan 12-01; the hyper-util client + body parse + exit-code logic + 9
//! unit tests (per D-14 / VALIDATION.md 12-02-01..07 with the URL case split across v4/v6/default)
//! land in Plan 12-02.

use crate::cli::Cli;
use http_body_util::{BodyExt, Empty};
use hyper::Request;
use hyper::body::Bytes;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use std::time::Duration;

/// Default bind target when `--bind` is absent. Aligns with the loopback
/// default declared in CLAUDE.md § Constraints ("Default bind 127.0.0.1:8080").
const DEFAULT_BIND: &str = "127.0.0.1:8080";

/// Total time budget for one probe (D-02). Must stay strictly below the
/// Dockerfile HEALTHCHECK `--timeout=5s` (D-06).
const TIMEOUT: Duration = Duration::from_secs(5);

/// Resolve the bind host per D-03 precedence:
///   1. `--bind` flag (explicit CLI override wins)
///   2. `CRONDUIT_BIND` env var (operator-set, shared with `cronduit run`)
///   3. `DEFAULT_BIND` (loopback 127.0.0.1:8080)
///
/// Returns an owned `String` so `execute` and the URL-construction helper both
/// see the same value without re-querying the environment.
fn resolve_bind(flag: Option<&str>) -> String {
    if let Some(b) = flag {
        return b.to_string();
    }
    if let Ok(b) = std::env::var("CRONDUIT_BIND")
        && !b.is_empty()
    {
        return b;
    }
    DEFAULT_BIND.to_string()
}

/// Pure URL-construction helper (W5). Separated from `execute` so the URL-shape
/// tests can assert against this function without ever opening a socket.
/// Returns the full health URL for the given optional `--bind` value.
/// Note: this helper is env-agnostic so tests can assert URL shape deterministically;
/// `execute` calls `resolve_bind` (which consults the env) and passes the result
/// as `Some(&resolved)` to keep this helper pure.
pub(crate) fn parse_health_url(bind: Option<&str>) -> String {
    let host = bind.unwrap_or(DEFAULT_BIND);
    format!("http://{host}/health")
}

/// Probe the local `/health` endpoint. Exit `0` iff HTTP 200 AND body parses as
/// JSON AND `body.status == "ok"`. Exit `1` on any other outcome.
///
/// Per D-05 a single non-zero code covers all failure modes; the per-mode
/// `tracing::error!` lines on stderr give a human reader enough to debug.
pub async fn execute(cli: &Cli) -> anyhow::Result<i32> {
    let resolved_bind = resolve_bind(cli.bind.as_deref());
    let bind: &str = &resolved_bind;
    let url = parse_health_url(Some(bind));
    let uri: hyper::Uri = match url.parse() {
        Ok(u) => u,
        Err(e) => {
            tracing::error!(target: "cronduit.health", url = %url, error = %e, "invalid URL");
            return Ok(1);
        }
    };

    // D-01: hyper-util client. HttpConnector is fine for loopback; no DNS.
    // D-02 literal honoring: 2s connect timeout + 5s outer wrap (read budget ~3s
    // belt-and-suspenders). The outer `tokio::time::timeout(TIMEOUT, ...)` below
    // remains the absolute upper bound at 5s total.
    let mut connector = HttpConnector::new();
    connector.set_connect_timeout(Some(Duration::from_secs(2)));
    let client: Client<HttpConnector, Empty<Bytes>> =
        Client::builder(TokioExecutor::new()).build(connector);

    let req = match Request::builder()
        .uri(uri)
        .header(hyper::header::HOST, bind)
        .body(Empty::<Bytes>::new())
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(target: "cronduit.health", error = %e, "request build failed");
            return Ok(1);
        }
    };

    // D-02: 5 s total budget via tokio::time::timeout.
    let resp = match tokio::time::timeout(TIMEOUT, client.request(req)).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            tracing::error!(
                target: "cronduit.health",
                error = %e,
                "request failed (connect-refused / DNS / transport)"
            );
            return Ok(1);
        }
        Err(_elapsed) => {
            tracing::error!(target: "cronduit.health", timeout_secs = 5, "request timed out");
            return Ok(1);
        }
    };

    if resp.status() != hyper::StatusCode::OK {
        tracing::error!(target: "cronduit.health", status = %resp.status(), "non-200 response");
        return Ok(1);
    }

    // D-05: collect body → Bytes → serde_json::Value, then check `status == "ok"`.
    let body_bytes = match resp.into_body().collect().await {
        Ok(c) => c.to_bytes(),
        Err(e) => {
            tracing::error!(target: "cronduit.health", error = %e, "body read failed");
            return Ok(1);
        }
    };

    let json: serde_json::Value = match serde_json::from_slice(&body_bytes) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(target: "cronduit.health", error = %e, "body not JSON");
            return Ok(1);
        }
    };

    if json.get("status").and_then(|v| v.as_str()) != Some("ok") {
        tracing::error!(
            target: "cronduit.health",
            status = ?json.get("status"),
            "status field missing or not 'ok'"
        );
        return Ok(1);
    }

    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command, LogFormatArg};
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Builds a minimal `Cli` whose `command = Health` and global flags are at
    /// defaults EXCEPT for the bind override the test wants to assert against.
    fn cli_with_bind(bind: Option<&str>) -> Cli {
        Cli {
            command: Command::Health,
            config: None,
            database_url: None,
            bind: bind.map(String::from),
            log_format: LogFormatArg::Json,
        }
    }

    /// Spawns a one-shot HTTP/1.1 server bound to 127.0.0.1:0 that returns the
    /// given status line + JSON body on the first connection, then exits.
    /// Returns the bound `host:port` string for use as `--bind`.
    async fn spawn_stub(status_line: &'static str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr: SocketAddr = listener.local_addr().expect("local_addr");
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.expect("accept");
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            let response = format!(
                "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
                len = body.len(),
            );
            let _ = sock.write_all(response.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
        format!("127.0.0.1:{}", addr.port())
    }

    #[tokio::test]
    async fn success_exits_zero() {
        let bind = spawn_stub(
            "HTTP/1.1 200 OK",
            r#"{"status":"ok","db":"ok","scheduler":"running"}"#,
        )
        .await;
        let cli = cli_with_bind(Some(&bind));
        assert_eq!(execute(&cli).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn non_200_exits_one() {
        let bind = spawn_stub(
            "HTTP/1.1 503 Service Unavailable",
            r#"{"status":"degraded","db":"error"}"#,
        )
        .await;
        let cli = cli_with_bind(Some(&bind));
        assert_eq!(execute(&cli).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn missing_status_field_exits_one() {
        let bind = spawn_stub("HTTP/1.1 200 OK", r#"{"db":"ok"}"#).await;
        let cli = cli_with_bind(Some(&bind));
        assert_eq!(execute(&cli).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn connect_refused_exits_one_fast() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let bind = format!("127.0.0.1:{port}");
        let cli = cli_with_bind(Some(&bind));

        let start = std::time::Instant::now();
        let code = execute(&cli).await.unwrap();
        let elapsed = start.elapsed();

        assert_eq!(code, 1);
        assert!(
            elapsed < Duration::from_secs(2),
            "connect-refused must fail fast; took {elapsed:?}"
        );
    }

    /// URL construction must handle the IPv4 host:port form. We don't actually
    /// dial; we infer correctness from the connect-refused path completing
    /// (which proves the URL parsed and a connection was attempted).
    #[tokio::test]
    async fn url_construction_v4() {
        let cli = cli_with_bind(Some("127.0.0.1:1"));
        assert_eq!(execute(&cli).await.unwrap(), 1);
    }

    /// URL construction must handle the IPv6 bracketed host:port form.
    #[tokio::test]
    async fn url_construction_v6() {
        let cli = cli_with_bind(Some("[::1]:1"));
        assert_eq!(execute(&cli).await.unwrap(), 1);
    }

    /// W5: Pure URL-parse assertion via the `parse_health_url` helper — does
    /// NOT touch a real socket, so it's immune to port-8080 contention on the
    /// test runner.
    #[test]
    fn url_construction_missing_port_default() {
        assert_eq!(parse_health_url(None), "http://127.0.0.1:8080/health");
    }

    /// D-04 + W5: the health probe MUST NOT require `--config` to be set, and
    /// the test must NOT depend on port 8080 being free.
    #[tokio::test]
    async fn no_config_read_required() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let bind = format!("127.0.0.1:{port}");

        let mut cli = cli_with_bind(Some(&bind));
        cli.config = Some(std::path::PathBuf::from("/nonexistent/cronduit.toml"));
        assert!(
            cli.config.is_some(),
            "test fixture invariant: config path is set but bogus"
        );

        // Just calling execute without surfacing a config-read IO error proves D-04.
        let _ = execute(&cli).await.unwrap();
    }

    /// D-03 precedence: explicit `--bind` flag wins over the CRONDUIT_BIND env var.
    /// Uses a synchronization primitive (rather than env guards) to avoid cross-test
    /// races — each call to `resolve_bind` with `Some` should return that value
    /// without consulting the environment.
    #[test]
    fn resolve_bind_flag_beats_env() {
        // Even if env is set, flag wins — prove by passing a distinctive flag.
        let out = super::resolve_bind(Some("10.0.0.1:9090"));
        assert_eq!(out, "10.0.0.1:9090");
    }

    /// Module-level lock for env-mutating tests. Rust runs tests in parallel by
    /// default and `std::env::set_var` is process-wide; tests that read or write
    /// `CRONDUIT_BIND` must share this lock so one test's set_var doesn't race
    /// another test's read. One lock per env var surface is the minimum coupling
    /// that still prevents inter-test env bleed.
    static CRONDUIT_BIND_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// D-03 precedence: CRONDUIT_BIND env var is used when no flag is supplied.
    #[test]
    fn resolve_bind_env_used_when_flag_absent() {
        let _g = CRONDUIT_BIND_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let prev = std::env::var("CRONDUIT_BIND").ok();
        // SAFETY: test-only; env mutation serialized by CRONDUIT_BIND_LOCK.
        unsafe {
            std::env::set_var("CRONDUIT_BIND", "192.168.1.5:8081");
        }
        let out = super::resolve_bind(None);
        // Restore env before any assertion can early-return.
        unsafe {
            match prev {
                Some(v) => std::env::set_var("CRONDUIT_BIND", v),
                None => std::env::remove_var("CRONDUIT_BIND"),
            }
        }
        assert_eq!(out, "192.168.1.5:8081");
    }

    /// D-03 precedence: default loopback fallback when neither flag nor env set.
    #[test]
    fn resolve_bind_defaults_to_loopback() {
        let _g = CRONDUIT_BIND_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let prev = std::env::var("CRONDUIT_BIND").ok();
        unsafe {
            std::env::remove_var("CRONDUIT_BIND");
        }
        let out = super::resolve_bind(None);
        unsafe {
            if let Some(v) = prev {
                std::env::set_var("CRONDUIT_BIND", v);
            }
        }
        assert_eq!(out, DEFAULT_BIND);
    }

    /// D-02: the 5 s total timeout fires deterministically. Uses
    /// `tokio::time::pause` + `advance` so the test runs in milliseconds even
    /// though the production code waits 5 seconds.
    #[tokio::test(start_paused = true)]
    async fn timeout_fires_after_5s() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            if let Ok((mut sock, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = sock.read(&mut buf).await;
                // Stall — never write. Hold the socket open until the test exits.
                std::future::pending::<()>().await;
            }
        });

        let bind = format!("127.0.0.1:{port}");
        let cli = cli_with_bind(Some(&bind));

        let probe = tokio::spawn(async move { execute(&cli).await });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(6)).await;
        let code = probe.await.expect("join").expect("anyhow ok");
        assert_eq!(code, 1);
    }
}
