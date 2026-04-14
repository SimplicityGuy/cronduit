---
status: partial
phase: 09-ci-cd-improvements
source: [09-VERIFICATION.md]
started: 2026-04-13
updated: 2026-04-13
---

## Current Test

[awaiting human testing]

## Tests

### 1. cleanup-cache.yml fires on PR close
expected: Open a draft PR against main, then close it. `gh run list --workflow=cleanup-cache.yml` shows a run; log shows "Fetching list of cache keys for refs/pull/<N>/merge"; run exits 0 even if zero caches are found.
result: [pending]

### 2. cleanup-images.yml dispatches against live GHCR package
expected: `gh workflow run cleanup-images.yml`, then `gh run list --workflow=cleanup-images.yml`. Run succeeds (exit 0); log shows retention policy summary (keep-n-tagged:2, older-than:30days) applied against ghcr.io/<owner>/cronduit.
result: [pending]

### 3. CI cache hits on second push
expected: Open a PR and let CI run to completion; on second push, lint and test jobs log "Cache restored from key:..." (Swatinem/rust-cache@v2); compose-smoke logs "importing cache manifest" from the cronduit-ci-smoke scope; image job does NOT log a cache restore (expected per FOUND-12 deliberate gap).
result: [pending]

## Summary

total: 3
passed: 0
issues: 0
pending: 3
skipped: 0
blocked: 0

## Gaps
