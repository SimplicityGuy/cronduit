---
phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
plan: 03
subsystem: config
tags: [webhooks, ssrf, https, validation, ipv4-rfc1918, ipv6-ula, config-load]

# Dependency graph
requires:
  - phase: 20
    provides: Wave 0 stub `tests/v12_webhook_https_required.rs`; existing `check_webhook_url` parseability + scheme check (Phase 18 / WH-01)
  - phase: 18
    provides: `WebhookConfig` schema and `check_webhook_url` extension point at `src/config/validate.rs:385`
provides:
  - LOAD-time HTTPS-required validator extending `check_webhook_url` in place
  - `classify_http_destination` private helper using `url::Host` enum match (avoids Pitfall 4 host_str/IpAddr round-trip)
  - D-21 verbatim error message regression-locked at both unit and integration boundaries
  - Boot-time INFO log on http-allowed paths (`target = "cronduit.config"`, `classified_net` field)
  - 15 new in-module unit tests + 10 new integration tests covering accept/reject matrix
affects: [20-04 webhook scheduler/dispatcher work, 20-07 docs/WEBHOOKS.md forward-pointer for accepted-residual SSRF risk, 24-* THREAT_MODEL TM5]

# Tech tracking
tech-stack:
  added: []  # No new crates — D-38 invariant intact (cargo tree -i openssl-sys empty)
  patterns:
    - "url::Host enum match for IP/host classification (avoids host_str().parse::<IpAddr>() bracket round-trip)"
    - "stdlib helpers (Ipv4Addr::is_private, Ipv6Addr::is_unique_local) over hand-rolled bit patterns"
    - "INFO log at LOAD-time validator rejection-or-accept for operator visibility"

key-files:
  created: []
  modified:
    - src/config/validate.rs (+260 lines: new helper + extension + 15 unit tests)
    - tests/v12_webhook_https_required.rs (Wave 0 stub replaced; 10 integration tests)

key-decisions:
  - "IPv6 ULA classification uses stdlib Ipv6Addr::is_unique_local (RFC 4193 fc00::/7) rather than hand-rolled fd00::/8 — broader-than-spec, never rejects spec-allowed (RESEARCH §4.1). Operator-facing error message still cites fd00::/8 for clarity."
  - "Domain arm of classify_http_destination accepts ONLY literal `localhost` (case-insensitive); no DNS resolution at LOAD time per D-20."
  - "Classification helper kept private (not pub) — only check_webhook_url consumes it."

patterns-established:
  - "D-21 verbatim error wording regression-locked at TWO levels (in-module unit test + integration test) so any future drift in either layer surfaces independently."
  - "broader-than-spec ULA acceptance is regression-locked at both layers via fc00::1 cases — prevents accidental tightening to a hand-rolled fd00::/8 check."

requirements-completed: [WH-07, WH-08]

# Metrics
duration: 9m 19s
completed: 2026-05-01
---

# Phase 20 Plan 03: HTTPS-Required Webhook URL Validator Summary

**LOAD-time validator narrows webhook SSRF surface to {loopback, RFC1918, ULA, localhost} for HTTP; HTTPS unrestricted. Lands D-19/D-20/D-21 with stdlib helpers — zero new dependencies.**

## Performance

- **Duration:** 9 min 19 sec
- **Started:** 2026-05-01T20:05:19Z
- **Completed:** 2026-05-01T20:14:38Z
- **Tasks:** 2/2 (both `tdd="true"`)
- **Files modified:** 2

## Accomplishments

- `check_webhook_url` extended in place (no sibling function, per D-19) with HTTPS-required classification — adds ~115 lines including helper, INFO log path, and D-21 verbatim error.
- `classify_http_destination` private helper uses `url::Host` enum match: `Ipv4Addr::is_loopback || is_private` for v4; `Ipv6Addr::is_loopback || is_unique_local` for v6; `eq_ignore_ascii_case("localhost")` for domain arm. No manual bit patterns, no DNS at LOAD (D-20).
- 15 new in-module unit tests in `src/config/validate.rs::tests` covering accept (https-anywhere, http-localhost case-insensitive, http-localhost:port, all RFC1918 v4 corners, ::1, fd00::1, fc00::1) and reject (http-public, http-link-local-v4 169.254/16, http-public-DNS, http-public-v6 2001:db8::/32, scheme-other-than-http/https returns first error and short-circuits).
- 10 new integration tests in `tests/v12_webhook_https_required.rs` driving `cronduit::config::parse_and_validate` end-to-end (interpolate → toml → apply_defaults → validate) — regression-locks the D-21 verbatim wording AND the broader-than-spec fc00::/7 acceptance through the full pipeline.
- `cargo tree -i openssl-sys` empty (D-38 invariant intact); zero new external crates.

## Task Commits

Each task followed the TDD RED → GREEN cycle:

1. **Task 1 RED — failing unit tests** — `0c91d46` (`test(20-03): add failing tests for HTTPS-required webhook validator`) — 6/6 new rejection tests fail without implementation.
2. **Task 1 GREEN — validator extension** — `df13328` (`feat(20-03): extend check_webhook_url with HTTPS-required classification`) — 55/55 config::validate unit tests pass (15 new + 40 existing).
3. **Task 2 — integration test population** — `0f03e1b` (`test(20-03): populate v12_webhook_https_required.rs LOAD-time integration tests`) — 10/10 integration tests pass against the already-landed validator (intentionally regression-locks the implementation at the integration boundary; no separate RED pass since Task 1's GREEN already implemented the behavior).

_Note: Task 2 is `tdd="true"` but its purpose is integration-level regression locking of behavior implemented in Task 1. The unit-level RED gate occurred in Task 1's commit `0c91d46`. The plan's task graph treats Task 2 as additive coverage at the public-entry-point boundary — no functional code lands in Task 2._

## Files Created/Modified

- **Modified** `src/config/validate.rs` — added `classify_http_destination` private helper (33 lines incl. doc comment) above `check_webhook_url`; extended the `Ok(parsed)` arm of `check_webhook_url` with the HTTPS-required branch (33 lines incl. INFO log + D-21 error); appended 15 unit tests (~150 lines) under existing `mod tests`. Total file size: 1363 → 1623 lines (+260).
- **Modified** `tests/v12_webhook_https_required.rs` — replaced 7-line Wave 0 stub (PHASE_MARKER constant) with full integration test body: helper functions + 10 `#[test]` cases. ~190 lines.

## Decisions Made

- **`classify_http_destination` is `fn`, not `pub fn`.** Only `check_webhook_url` calls it; keeping it private avoids leaking validator internals.
- **Scheme-mismatch ConfigError uses early `return`.** The new HTTPS-required block sits AFTER the existing scheme check; rather than nesting `if scheme == "http"` inside an `else`, the unsupported-scheme arm emits its error and `return`s, leaving the http branch as a flat sibling. This matches the existing flat-control-flow style of the validator's other arms.
- **`http_localhost_uppercase_accepted` test is intentional.** The `url` crate normalizes hostnames to lowercase during parsing, so `eq_ignore_ascii_case` is technically redundant for the `Domain` arm — but the test pins the case-insensitive contract so any future change to the helper that drops the case-fold (e.g., to a literal `==`) will break this test.
- **Public entry point used in integration test:** `cronduit::config::parse_and_validate(path: &Path) -> Result<ParsedConfig, Vec<ConfigError>>` (path-taking, not string-taking). Confirmed by reading `tests/config_parser.rs`, `tests/v12_labels_interpolation.rs`, and `tests/reload_inflight.rs`. The test helper writes the TOML to a `tempfile::NamedTempFile` and passes its path — same pattern as `v12_labels_interpolation.rs`.

## Deviations from Plan

None — plan executed exactly as written.

The only minor adaptation was an early `return` after the unsupported-scheme ConfigError so the HTTPS-required branch is reachable only for `http`/`https` schemes; this matches the spirit of the plan's `<action>` ("Keep the existing scheme-mismatch ConfigError intact and BEFORE this new block") while avoiding deeply nested control flow.

## Issues Encountered

- `cargo fmt -- src/config/validate.rs` (running fmt with a path argument) reformatted not only the targeted file but also picked up unrelated drift in `src/db/queries.rs` (a 4-line argument-list reformat on `insert_webhook_dlq_row`). The drift was NOT caused by this plan and is pre-existing in another agent's wave 1 scope. Reverted via `git checkout -- src/db/queries.rs` before each commit. No impact on the plan's deliverables.
- Pre-existing clippy warnings in `tests/v12_webhook_retry_after.rs` (Wave 0 stub owned by Plan 20-02) trigger when running `cargo clippy --all-targets`. Not in scope for this plan; intentionally left for Plan 20-02 owner to resolve.

## User Setup Required

None — pure config-validator change; no operator action needed.

## Verification Performed

| Check | Command | Result |
|-------|---------|--------|
| Library type-check | `cargo check --lib --tests` | exit 0 |
| Unit tests (config::validate) | `cargo nextest run --lib config::validate` | 55/55 PASS (15 new HTTPS-required + 40 pre-existing) |
| Integration tests | `cargo nextest run --test v12_webhook_https_required` | 10/10 PASS |
| TLS dependency invariant (D-38) | `cargo tree -i openssl-sys` | "did not match any packages" — empty as required |
| INFO log line present | `grep -c '"webhook URL accepted on local net"' src/config/validate.rs` | 1 |
| D-21 verbatim error in source | `grep -c 'requires HTTPS for non-loopback / non-RFC1918' src/config/validate.rs` | 1 |
| D-21 verbatim allowed-nets list | `grep -c '127/8, ::1, 10/8, 172.16/12, 192.168/16, fd00::/8' src/config/validate.rs` | 2 (definition + integration test) |
| Helper present exactly once | `grep -c 'fn classify_http_destination' src/config/validate.rs` | 1 |

## Key Numbers

- **Validator extension size:** ~70 lines of production code (33-line helper + 33-line extension + a few comments). Plan estimated ~30 lines; the extra came from doc comments on the helper and the integration-friendly multi-arm helper return shape.
- **`src/config/validate.rs` line delta:** +260 (1363 → 1623).
- **In-module unit tests added:** 15.
- **Integration tests added:** 10.
- **Public entry point used in integration test:** `cronduit::config::parse_and_validate(&Path)` — path-taking. The integration test writes TOML to a `tempfile::NamedTempFile` and passes its path. Important for future plans that need to drive the full LOAD pipeline.
- **IPv6 ULA decision:** spec-literal `fd00::/8` cited in operator-facing error message; broader `fc00::/7` actually accepted via `Ipv6Addr::is_unique_local` (RESEARCH §4.1: never rejects spec-allowed). Both behaviors regression-locked.

## Self-Check: PASSED

- File exists: `src/config/validate.rs` ✓
- File exists: `tests/v12_webhook_https_required.rs` ✓
- Commit `0c91d46` (test RED) present in `git log --all` ✓
- Commit `df13328` (feat GREEN) present in `git log --all` ✓
- Commit `0f03e1b` (test integration) present in `git log --all` ✓

## TDD Gate Compliance

This plan does NOT have plan-level `type: tdd` (it's `type: execute`), but both tasks are `tdd="true"`. Per-task gate check:

| Task | RED commit | GREEN commit | Notes |
|------|------------|--------------|-------|
| 1 | `0c91d46` (test) | `df13328` (feat) | Standard RED/GREEN cycle; 6/6 new rejection tests verified failing before implementation. |
| 2 | (n/a — integration regression-lock against Task 1's GREEN) | `0f03e1b` (test) | Task 2 is `tdd="true"` but is structurally a coverage-extension task: it adds integration-level regression locks to behavior already implemented in Task 1. No production code in Task 2. The plan's `<behavior>` and `<acceptance_criteria>` for Task 2 are pure assertion sets; running them against an empty stub would be meaningless because Task 1's implementation is the system under test. |
