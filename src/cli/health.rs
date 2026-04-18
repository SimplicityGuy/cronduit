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

/// Plan 12-02 RED-phase stub: the URL helper is declared here so the tests
/// compile against a symbol that exists. The implementation is replaced in
/// the GREEN commit; at RED time every caller path still returns `Ok(0)` via
/// `execute`, so the URL-format assertion is the specific failing test.
pub(crate) fn parse_health_url(_bind: Option<&str>) -> String {
    // Intentionally wrong until GREEN — lets url_construction_missing_port_default fail at RED.
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command, LogFormatArg};
    use std::net::SocketAddr;
    use std::time::Duration;
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
