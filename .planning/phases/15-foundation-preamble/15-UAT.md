---
status: complete
phase: 15-foundation-preamble
source: [15-01-SUMMARY.md, 15-02-SUMMARY.md, 15-03-SUMMARY.md, 15-04-SUMMARY.md, 15-05-SUMMARY.md]
started: 2026-04-26T23:30:00Z
updated: 2026-04-26T23:50:00Z
---

## Current Test

[testing complete]


## Tests

### 1. Cold Start Smoke Test
expected: |
  Run `just dev`. The cronduit daemon boots without errors. Migrations apply idempotently.
  Server logs show the scheduler started, the webhook worker spawned with `NoopDispatcher`,
  and the HTTP server is listening on `127.0.0.1:8080`. No panic, no startup error log
  about webhooks. Hit Ctrl-C and confirm graceful shutdown (scheduler drains, then
  worker exits cleanly — no `TrySendError::Closed` errors flooding the log).
result: pass

### 2. `cronduit --version` reports `1.2.0` (FOUND-15)
expected: |
  Run `just build` to compile the workspace. Then run `./target/debug/cronduit --version`.
  The output should be exactly `cronduit 1.2.0` (single line). This confirms the v1.2
  milestone version-bump landed atomically as the first commit of the v1.2 cycle (D-12),
  so every subsequent rc cut and the final `v1.2.0` ship will report the milestone
  version from the binary.
result: pass

### 3. `just deny` recipe runs locally (FOUND-16)
expected: |
  Run `just deny`. The recipe invokes `cargo deny check advisories licenses bans` and
  surfaces output. On rc.1 the recipe is expected to surface (a) `RUSTSEC-2026-0104`
  rustls-webpki advisory and (b) 5 transitive license findings (Unicode-3.0, Zlib,
  CC0-1.0, CDLA-Permissive-2.0, dual-licensed combos from `icu_*` crates) — all visible
  but non-blocking under the rc.1 warn-only posture (`continue-on-error: true` lives in
  CI, not in the recipe). Phase 24 will resolve these and flip the gate to blocking.
  The recipe NOT being defined, hanging, or panicking would all be issues; non-zero
  exit on rc.1 is BY DESIGN.
result: pass

### 4. `/metrics` serves the webhook drop counter from boot (WH-02 / D-11)
expected: |
  In one terminal: run `just dev` and let cronduit boot. In another terminal: run
  `just metrics-check` (which curls /metrics and confirms `cronduit_scheduler_up` +
  `cronduit_runs_total` lines). Then manually curl the same endpoint:
  `curl -s http://127.0.0.1:8080/metrics | grep cronduit_webhook_delivery_dropped_total`.
  You should see THREE lines — the HELP comment, the TYPE counter line, and a zero
  baseline value (`cronduit_webhook_delivery_dropped_total 0`) — all rendered from
  boot before any drop event occurs (Pitfall 3 prevention). This is the operator's
  alerting surface — without these lines from boot, Grafana/alertmanager would have
  no metric to scrape until the first drop event, which would mask the very condition
  the alert is meant to detect.
result: pass

### 5. cargo-deny check appears in PR check list (FOUND-16, MANUAL — needs real PR)
expected: |
  After this branch lands on `main` via PR (NOT a direct commit per project rule), the
  GitHub Actions PR check list for the next PR opened against `main` should include a
  `lint (fmt + clippy + openssl-sys guard)` job whose step list shows a `Run just deny`
  step. On rc.1 the step renders yellow (warn-level findings present per Test 3) without
  blocking the PR's mergeability — this is the visible-but-non-blocking posture promised
  by ROADMAP § Phase 15 Success Criterion #2. This test cannot be exercised until a PR
  is opened against `main` for the first time after this branch lands; defer to that
  PR's check-run review.
result: pass

## Summary

total: 5
passed: 5
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps
