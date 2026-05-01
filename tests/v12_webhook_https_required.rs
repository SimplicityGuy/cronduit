//! Phase 20 / WH-07: LOAD-time HTTPS-required validator integration tests.
//!
//! Drives `cronduit::config::parse_and_validate` (the public entry point used
//! across the test suite — see also tests/config_parser.rs and
//! tests/v12_labels_interpolation.rs) with TOML configs whose `webhook.url`
//! exercises the accept/reject matrix specified in 20-03-PLAN:
//!
//!   * https://example.com/hook                  → accepted (any host)
//!   * http://example.com/hook                   → rejected (D-21 wording)
//!   * http://localhost/hook                     → accepted (literal host)
//!   * http://127.0.0.1/hook                     → accepted (loopback v4)
//!   * http://10.0.0.1/hook                      → accepted (RFC1918)
//!   * http://172.16.0.1/hook                    → accepted (RFC1918)
//!   * http://192.168.1.1/hook                   → accepted (RFC1918)
//!   * http://[::1]/hook                         → accepted (loopback v6)
//!   * http://[fd00::1]/hook                     → accepted (success-criterion-literal ULA)
//!   * http://[fc00::1]/hook                     → accepted (broader than spec — RESEARCH §4.1)
//!   * http://198.51.100.1/hook (TEST-NET-2)     → rejected (public v4)
//!   * http://example.org/hook                   → rejected (no DNS at LOAD — D-20)
//!
//! Pin the D-21 verbatim error wording so any future drift in
//! `check_webhook_url`'s message surfaces here as a regression.

use std::io::Write;

use cronduit::config::parse_and_validate;
use tempfile::NamedTempFile;

/// Build a minimal valid TOML config with the given webhook URL on a single
/// command job. Mirrors the `parse_and_validate` pattern used in
/// `tests/v12_labels_interpolation.rs` (write to tempfile, then parse via the
/// full pipeline: interpolate → toml → apply_defaults → validate).
fn make_config_toml(url: &str) -> String {
    format!(
        r#"
[server]
timezone = "UTC"
bind = "127.0.0.1:8080"

[[jobs]]
name = "test-job"
schedule = "* * * * *"
command = "echo hi"
webhook = {{ url = "{url}", states = ["failed"], secret = "shh" }}
"#
    )
}

/// Write the TOML to a tempfile and run `parse_and_validate` end-to-end.
/// Returns the raw `Result` so callers can inspect both success and the full
/// `Vec<ConfigError>` on failure.
fn parse_with_webhook_url(
    url: &str,
) -> Result<cronduit::config::ParsedConfig, Vec<cronduit::config::ConfigError>> {
    let toml_text = make_config_toml(url);
    let mut tmp = NamedTempFile::new().expect("tempfile created");
    tmp.write_all(toml_text.as_bytes()).expect("toml written");
    parse_and_validate(tmp.path())
}

/// Concatenate every `ConfigError.message` so substring assertions can match
/// against the error vec without caring about ordering.
fn errors_joined(errors: &[cronduit::config::ConfigError]) -> String {
    errors
        .iter()
        .map(|e| e.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn https_anywhere_accepted() {
    let result = parse_with_webhook_url("https://example.com/hook");
    assert!(
        result.is_ok(),
        "https://example.com should be accepted; got {:?}",
        result.err()
    );
}

#[test]
fn http_public_rejected_with_d21_message() {
    let errors = parse_with_webhook_url("http://example.com/hook")
        .expect_err("http://example.com should be rejected");
    let msg = errors_joined(&errors);
    assert!(
        msg.contains("requires HTTPS for non-loopback / non-RFC1918"),
        "D-21 wording missing; got: {msg}"
    );
    assert!(
        msg.contains("127/8, ::1, 10/8, 172.16/12, 192.168/16, fd00::/8"),
        "D-21 verbatim allowed-nets list missing; got: {msg}"
    );
}

#[test]
fn http_localhost_accepted() {
    let result = parse_with_webhook_url("http://localhost/hook");
    assert!(
        result.is_ok(),
        "http://localhost should be accepted; got {:?}",
        result.err()
    );
}

#[test]
fn http_loopback_v4_accepted() {
    let result = parse_with_webhook_url("http://127.0.0.1/hook");
    assert!(
        result.is_ok(),
        "http://127.0.0.1 should be accepted; got {:?}",
        result.err()
    );
}

#[test]
fn http_rfc1918_v4_accepted() {
    for url in [
        "http://10.0.0.1/hook",
        "http://172.16.0.1/hook",
        "http://192.168.1.1/hook",
    ] {
        let result = parse_with_webhook_url(url);
        assert!(
            result.is_ok(),
            "URL {url} should be accepted; got {:?}",
            result.err()
        );
    }
}

#[test]
fn http_loopback_v6_accepted() {
    let result = parse_with_webhook_url("http://[::1]/hook");
    assert!(
        result.is_ok(),
        "http://[::1] should be accepted; got {:?}",
        result.err()
    );
}

#[test]
fn http_ula_fd_accepted_per_success_criterion() {
    // fd00::/8 — the success-criterion-literal subset.
    let result = parse_with_webhook_url("http://[fd00::1]/hook");
    assert!(
        result.is_ok(),
        "http://[fd00::1] should be accepted; got {:?}",
        result.err()
    );
}

#[test]
fn http_ula_fc_accepted_broader_per_research() {
    // fc00::/8 — broader RFC 4193 range covered by Ipv6Addr::is_unique_local
    // (RESEARCH §4.1: broader than spec, never rejects spec-allowed). Pinned
    // here so any future tightening to a hand-rolled fd00::/8 check breaks
    // this regression-lock.
    let result = parse_with_webhook_url("http://[fc00::1]/hook");
    assert!(
        result.is_ok(),
        "http://[fc00::1] should be accepted; got {:?}",
        result.err()
    );
}

#[test]
fn http_public_v4_rejected() {
    // 198.51.100.0/24 — TEST-NET-2 (RFC 5737). Public but unallocated.
    let errors = parse_with_webhook_url("http://198.51.100.1/hook")
        .expect_err("http://198.51.100.1 should be rejected");
    let msg = errors_joined(&errors);
    assert!(
        msg.contains("requires HTTPS for non-loopback"),
        "D-21 wording missing; got: {msg}"
    );
}

#[test]
fn http_public_dns_rejected_no_dns_resolution_per_d20() {
    // example.org actually resolves to a public IP, but D-20 forbids DNS at
    // LOAD time. The validator must reject any non-`localhost` hostname that
    // is not parseable as an IpAddr.
    let errors = parse_with_webhook_url("http://example.org/hook")
        .expect_err("http://example.org should be rejected (no DNS at LOAD per D-20)");
    let msg = errors_joined(&errors);
    assert!(
        msg.contains("requires HTTPS for non-loopback"),
        "D-21 wording missing; got: {msg}"
    );
}
