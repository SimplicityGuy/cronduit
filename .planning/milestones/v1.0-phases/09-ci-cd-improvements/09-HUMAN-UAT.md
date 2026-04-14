---
status: deferred
phase: 09-ci-cd-improvements
source: [09-VERIFICATION.md]
started: 2026-04-13
updated: 2026-04-14
resolution: deferred-to-natural-validation
---

## Current Test

[deferred — will self-validate on natural PR activity; see Resolution note below]

## Resolution (2026-04-14)

All three tests require live GitHub-hosted runner execution that cannot be
reproduced in static analysis or on a developer laptop. Rather than gate v1.0
archive on synthetic live-runner verification, these tests are deferred to
**natural validation during normal PR activity**:

- **Test 1** (`cleanup-cache.yml` fires on PR close) self-validates on the first
  `pull_request: closed` event after the workflow lands on `main`. PR #17
  (`chore/nyquist-audit-and-v1.0-milestone-refresh`) will exercise it on close.
  Operator follow-up: `gh run list --workflow=cleanup-cache.yml` after PR #17
  closes to confirm the run appears and logs the expected cache-fetch line.

- **Test 2** (`cleanup-images.yml` dispatches against live GHCR) requires a live
  `ghcr.io/<owner>/cronduit` package with published images. Operator follow-up:
  after the next `release.yml` run publishes an image to GHCR, run
  `gh workflow run cleanup-images.yml` and confirm the retention policy summary.
  Scheduled monthly cron (`0 0 15 * *`) provides continuous validation thereafter.

- **Test 3** (rust-cache restore on second push) self-validates whenever any PR
  receives a second commit. PR #17 itself, or any follow-up PR, will exercise
  it. Operator follow-up: observe the Swatinem/rust-cache@v2 step logs in the
  CI run of the second commit.

Deferral is accepted per the v1.0 milestone audit (2026-04-14) verdict of
`passed` — these items are not shipping blockers and will self-close via
normal project activity without requiring a dedicated human UAT pass.

## Tests

### 1. cleanup-cache.yml fires on PR close
expected: Open a draft PR against main, then close it. `gh run list --workflow=cleanup-cache.yml` shows a run; log shows "Fetching list of cache keys for refs/pull/<N>/merge"; run exits 0 even if zero caches are found.
result: deferred
resolution: PR #17 will exercise this on close — operator confirms via `gh run list --workflow=cleanup-cache.yml` post-merge.

### 2. cleanup-images.yml dispatches against live GHCR package
expected: `gh workflow run cleanup-images.yml`, then `gh run list --workflow=cleanup-images.yml`. Run succeeds (exit 0); log shows retention policy summary (keep-n-tagged:2, older-than:30days) applied against ghcr.io/<owner>/cronduit.
result: deferred
resolution: operator confirms after first monthly cron run (15th of month) or via manual `gh workflow run cleanup-images.yml` dispatch against real GHCR after release.yml publishes an image.

### 3. CI cache hits on second push
expected: Open a PR and let CI run to completion; on second push, lint and test jobs log "Cache restored from key:..." (Swatinem/rust-cache@v2); compose-smoke logs "importing cache manifest" from the cronduit-ci-smoke scope; image job does NOT log a cache restore (expected per FOUND-12 deliberate gap).
result: deferred
resolution: self-validates on any PR's second commit. Operator confirms via CI logs on PR #17 second push or first follow-up PR.

## Summary

total: 3
passed: 0
deferred: 3
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps
