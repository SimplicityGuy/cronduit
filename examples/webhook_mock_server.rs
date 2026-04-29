//! Phase 18 — webhook UAT mock receiver.
//!
//! Run via `just uat-webhook-mock`. Listens on 127.0.0.1:9999, logs every
//! request (method, path, all headers, body) to stdout AND to
//! /tmp/cronduit-webhook-mock.log. Returns 200 OK on every request.
//!
//! USE ONLY for local maintainer UAT validation per project memory
//! `feedback_uat_user_validates.md`. Never expose to the public internet.
//!
//! NOTE: This is a simple loopback receiver for manual UAT inspection only —
//! NOT a production-grade HTTP/1.1 implementation. The `Connection: close`
//! response header forces request-per-connection semantics so reqwest doesn't
//! reuse a stale TCP stream between deliveries. Reads run in a small loop
//! that breaks once headers + Content-Length body are received OR the
//! connection is closed by the client.

use std::io::Write;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const ADDR: &str = "127.0.0.1:9999";
const LOG_PATH: &str = "/tmp/cronduit-webhook-mock.log";

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind(ADDR).await?;
    eprintln!("[webhook-mock] listening on http://{ADDR}/  (log: {LOG_PATH})");
    loop {
        let (mut socket, peer) = listener.accept().await?;
        tokio::spawn(async move {
            // Read in a loop. Stop once we see "\r\n\r\n" AND have read
            // Content-Length bytes after it, OR the client closes.
            let mut buf: Vec<u8> = Vec::with_capacity(8192);
            let mut chunk = [0u8; 4096];
            let mut content_length: Option<usize> = None;
            let mut headers_end: Option<usize> = None;

            loop {
                let n = match socket.read(&mut chunk).await {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("[webhook-mock] read err from {peer}: {e}");
                        return;
                    }
                };
                buf.extend_from_slice(&chunk[..n]);

                if headers_end.is_none()
                    && let Some(idx) = find_header_end(&buf)
                {
                    headers_end = Some(idx);
                    content_length = parse_content_length(&buf[..idx]);
                }

                if let (Some(end), Some(len)) = (headers_end, content_length) {
                    if buf.len() >= end + 4 + len {
                        break; // headers + body fully received
                    }
                } else if let Some(end) = headers_end
                    && content_length.is_none()
                    && buf.len() >= end + 4
                {
                    break; // no Content-Length, no body expected
                }

                // Safety cap: don't grow forever on a misbehaving client.
                if buf.len() > 1_048_576 {
                    // 1 MiB
                    eprintln!("[webhook-mock] body too large from {peer}; dropping");
                    return;
                }
            }

            let raw = String::from_utf8_lossy(&buf).to_string();
            let log_line = format!(
                "----- {} from {peer} -----\n{raw}\n----- end -----\n",
                chrono::Utc::now().to_rfc3339()
            );
            eprintln!("{log_line}");
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(LOG_PATH)
            {
                let _ = f.write_all(log_line.as_bytes());
            }
            // Connection: close → reqwest will NOT keep this socket alive,
            // so the next delivery opens a fresh TCP stream. This avoids
            // half-state issues if our reader breaks early.
            let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
            let _ = socket.write_all(response).await;
            let _ = socket.shutdown().await;
        });
    }
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn parse_content_length(headers: &[u8]) -> Option<usize> {
    let s = std::str::from_utf8(headers).ok()?;
    for line in s.split("\r\n") {
        if let Some(rest) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            return rest.trim().parse::<usize>().ok();
        }
    }
    None
}
