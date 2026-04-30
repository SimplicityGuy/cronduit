---
phase: 19-webhook-hmac-signing-receiver-examples
plan: 01
subsystem: testing
tags: [webhooks, hmac, sha256, fixtures, interop, standard-webhooks-v1, rust, sign_v1]

# Dependency graph
requires:
  - phase: 18-webhook-mvp
    provides: "sign_v1 helper, WebhookPayload::build, 16-field v1 wire format"
provides:
  - "tests/fixtures/webhook-v1/ — 7-file shared interop fixture (secret, id, ts, payload, signature, README, .gitattributes)"
  - "src/webhooks/dispatcher.rs::tests::sign_v1_locks_interop_fixture — in-module byte-equality lock test (locks both payload AND signature; drift in either direction fails Rust CI)"
  - "src/webhooks/dispatcher.rs::tests::print_canonical_payload_bytes — #[ignore] regenerator helper for future intentional wire-format bumps"
  - "justfile sentinel '# === Phase 19 webhook receivers ===' — shared insertion point for Plans 19-02/03/04 (parallelism enabler)"
affects: [19-02-python-receiver, 19-03-go-receiver, 19-04-node-receiver, 19-05-docs, 19-06-ci-matrix]

# Tech tracking
tech-stack:
  added: []  # zero new Rust crates (D-24 satisfied — `cargo tree -i openssl-sys` empty across native + arm64-musl + amd64-musl)
  patterns:
    - "Shared interop fixture: a single canonical wire-format byte-vector consumed by Rust + 3 receiver runtimes"
    - "Byte-locking test: include_bytes!/include_str! at compile time so the test reads zero files at run time and works on any cwd"
    - "Pitfall 3 mitigation: `* -text` .gitattributes + manually-verified no-trailing-newline data files"
    - "Sentinel anchor in justfile: a comment-only hook that lets multiple plans append per-language recipes without colliding on edits"

key-files:
  created:
    - "tests/fixtures/webhook-v1/secret.txt"
    - "tests/fixtures/webhook-v1/webhook-id.txt"
    - "tests/fixtures/webhook-v1/webhook-timestamp.txt"
    - "tests/fixtures/webhook-v1/payload.json"
    - "tests/fixtures/webhook-v1/expected-signature.txt"
    - "tests/fixtures/webhook-v1/README.md"
    - "tests/fixtures/webhook-v1/.gitattributes"
  modified:
    - "src/webhooks/dispatcher.rs (added 2 tests in mod tests: sign_v1_locks_interop_fixture + print_canonical_payload_bytes)"
    - "justfile (added 3-line sentinel comment block after Phase 18 uat-webhook-verify recipe)"

key-decisions:
  - "Used 26-byte ULID `01HXYZTESTFIXTURE000000000` (true ULID length) instead of the plan's 27-byte literal — Rule 1 plan inconsistency fix; the must_haves frontmatter and acceptance criteria all said 26 bytes but the literal had 27 chars. Test code updated to match the 26-char form."
  - "Locked canonical event timestamps at 2025-01-01T00:00:00Z..+1s (stable past) so the fixture does not drift with the test clock"
  - "image_digest = None, config_hash = None for the canonical 'command archetype' event — receivers see real null serialization"
  - "Filter position fixed at 1, cronduit_version = '1.2.0' for the fixture event"

patterns-established:
  - "Compile-time-embedded fixtures: tests use `include_bytes!('../../tests/fixtures/.../payload.json')` so they read zero files at runtime"
  - "Re-derive both directions: lock tests should re-derive BOTH the payload AND the signature, so drift in serde output OR sign code fails the same test"
  - "Printer helper as #[ignore]: an in-module `#[ignore]` test that prints regeneration output to stdout, gated behind `--ignored --nocapture`, lets operators regenerate the fixture deterministically without an external script"

requirements-completed: [WH-04]

# Metrics
duration: ~10 min
completed: 2026-04-30
---

# Phase 19 Plan 01: Webhook v1 Interop Fixture Lock Summary

**Locked the Standard Webhooks v1 wire format on the cronduit side: 7-file shared interop fixture under `tests/fixtures/webhook-v1/` plus an in-module Rust unit test that re-derives both the canonical 16-field payload bytes AND the `v1,<base64>` signature and asserts byte-equality against the on-disk files. Drift in `sign_v1`, `WebhookPayload::build`, or any fixture file fails Rust CI before Plans 02/03/04 (Python/Go/Node receivers) can even run.**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-04-30 (worktree base d77d5d5)
- **Completed:** 2026-04-30
- **Tasks:** 3
- **Files created:** 7
- **Files modified:** 2

## Accomplishments

- 7 fixture files locked under `tests/fixtures/webhook-v1/` with verified no-trailing-newline byte counts (secret=37, webhook-id=26, webhook-timestamp=10, expected-signature=47, payload.json=349)
- In-module `sign_v1_locks_interop_fixture` test passes — re-derives BOTH the payload (via `WebhookPayload::build` + `serde_json::to_vec`) AND the signature (via `sign_v1`), asserts byte-equality against compile-time-embedded fixture files
- `#[ignore]` regenerator helper `print_canonical_payload_bytes` lets future maintainers intentionally bump the wire format without external tooling
- Sentinel anchor `# === Phase 19 webhook receivers ===` added to `justfile` so Plans 02/03/04 can run in parallel in wave 2 without justfile-edit collisions
- All 20 existing `webhooks::` unit tests stay green; `just openssl-check` returns empty across native + arm64-musl + amd64-musl (D-24 satisfied — zero new Rust crates)

## Task Commits

1. **Task 1: Create the 7 fixture files** — `0cc4027` (test)
2. **Task 2: Add lock test + printer helper, regenerate payload + signature** — `3a70cee` (test)
3. **Task 3: Insert sentinel anchor in justfile** — `83d983f` (chore)

_Note: Task 1 and Task 2 are both `test(...)` commits per TDD gate (Task 1 is the fixture-file skeleton — RED-equivalent for the Plan 19 wire-format lock; Task 2 wires the test that locks them — GREEN-equivalent)._ _Final metadata commit happens at orchestrator level._

## Files Created/Modified

### Created (7 fixture files)

- `tests/fixtures/webhook-v1/secret.txt` — 37 bytes, `cronduit-test-fixture-secret-not-real` (no trailing newline)
- `tests/fixtures/webhook-v1/webhook-id.txt` — 26 bytes, `01HXYZTESTFIXTURE000000000` (no trailing newline)
- `tests/fixtures/webhook-v1/webhook-timestamp.txt` — 10 bytes, `1735689600` (= 2025-01-01T00:00:00Z, no trailing newline)
- `tests/fixtures/webhook-v1/payload.json` — 349 bytes, compact-JSON of `WebhookPayload::build(canonical_event)` (no trailing newline). Full content: `{"payload_version":"v1","event_type":"run_finalized","run_id":42,"job_id":7,"job_name":"backup-nightly","status":"failed","exit_code":1,"started_at":"2025-01-01T00:00:00Z","finished_at":"2025-01-01T00:00:01Z","duration_ms":1000,"streak_position":1,"consecutive_failures":3,"image_digest":null,"config_hash":null,"tags":[],"cronduit_version":"1.2.0"}`
- `tests/fixtures/webhook-v1/expected-signature.txt` — 47 bytes, `v1,Gqa7PWQIieHzNE5/ccPk3IuJZsWhmgF5R0qVZJbLnig=` (no trailing newline)
- `tests/fixtures/webhook-v1/README.md` — fixture provenance + no-trailing-newline rule + regen workflow + canonical-event provenance
- `tests/fixtures/webhook-v1/.gitattributes` — `* -text` (disables EOL normalization for the directory)

### Modified

- `src/webhooks/dispatcher.rs` — appended 2 tests to existing `mod tests` block: `sign_v1_locks_interop_fixture` (the lock test) and `print_canonical_payload_bytes` (`#[ignore]` regenerator). Tests live in-module because `sign_v1` is `pub(crate)`.
- `justfile` — appended a 3-line sentinel comment block (`# === Phase 19 webhook receivers ===` plus 2 doc lines) between the existing Phase 18 `uat-webhook-verify` recipe and the dev-loop separator.

## Decisions Made

- **26-byte ULID instead of 27-byte literal:** the plan's `must_haves`, acceptance criteria, and README all said 26 bytes for `webhook-id.txt`, but the explicit literal `01HXYZTESTFIXTURE0000000000` was 27 chars. Resolved by using a real 26-char ULID (`01HXYZTESTFIXTURE000000000`) and updating the test code's literal to match. Both file content and test literal byte-match each other; HMAC verifies.
- **Locked timestamps at 2025-01-01T00:00:00Z..+1s:** stable past timestamps so the fixture does not drift with the test clock.
- **Command-archetype event:** `image_digest = None, config_hash = None` so receivers see real `null` serialization (proves the `Option<String>` -> `null` round-trip works).
- **Plan 01 adds the sentinel only, no per-language recipe:** keeps Plan 01 trivially mergeable and lets Plans 02/03/04 each append a per-language receiver block in parallel.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] webhook-id.txt byte count inconsistency**
- **Found during:** Task 1 (creating fixture files)
- **Issue:** The plan's literal `01HXYZTESTFIXTURE0000000000` is 27 chars, but the plan stated/asserted webhook-id.txt should be 26 bytes (a real ULID length). The test code in the plan also used the same 27-char literal. Inconsistent specification.
- **Fix:** Adjusted both the on-disk file and the test code's literal to a true 26-char ULID `01HXYZTESTFIXTURE000000000` (one fewer trailing zero). The on-disk file and test code's `let webhook_id = "01HXYZTESTFIXTURE000000000"` byte-match, so the HMAC computation is consistent and the lock test passes.
- **Files modified:** tests/fixtures/webhook-v1/webhook-id.txt (Task 1), src/webhooks/dispatcher.rs (Task 2)
- **Verification:** `wc -c tests/fixtures/webhook-v1/webhook-id.txt` returns 26; `cargo test sign_v1_locks_interop_fixture` passes.
- **Committed in:** 0cc4027 (Task 1) + 3a70cee (Task 2)

---

**Total deviations:** 1 auto-fixed (1 bug — plan internal inconsistency)
**Impact on plan:** No scope change. The locked literal still byte-matches across the fixture file and the test code; the lock test passes; Plans 02/03/04 will read the same 26-byte ULID from disk via stdlib file-read.

## Issues Encountered

None — all 3 tasks executed cleanly. The TDD-style flow was: (1) write the fixture skeleton with placeholders for payload/signature, (2) add the printer helper test and regenerate the placeholders to real bytes, (3) verify the lock test stays green and the full webhook suite is unaffected.

## Verification Receipts

- `cargo test --lib -- webhooks::dispatcher::tests::sign_v1_locks_interop_fixture` → **1 passed**
- `cargo test --lib -- webhooks::` → **20 passed, 0 failed, 1 ignored** (the printer helper)
- `just openssl-check` → **OK: no openssl-sys in dep tree (native + arm64-musl + amd64-musl)**
- `wc -c tests/fixtures/webhook-v1/secret.txt` → **37**
- `wc -c tests/fixtures/webhook-v1/webhook-id.txt` → **26**
- `wc -c tests/fixtures/webhook-v1/webhook-timestamp.txt` → **10**
- `wc -c tests/fixtures/webhook-v1/expected-signature.txt` → **47**
- `wc -c tests/fixtures/webhook-v1/payload.json` → **349**
- `tail -c 1` last byte for each data file: `6c` (secret 'l'), `30` (webhook-id '0'), `30` (webhook-timestamp '0'), `3d` (expected-signature '='), `7d` (payload.json '}') — none equal `0a` (Pitfall 3 verified)
- `grep -c "=== Phase 19 webhook receivers ===" justfile` → **1**
- `just --list` → exit 0, no `uat-webhook-receiver-*` recipes yet (Plans 02/03/04 will add them)

## Next Phase Readiness

- **Plans 02/03/04 (Python/Go/Node receivers) are unblocked.** Each can read the 5 fixture data files (secret/id/ts/payload/signature) via their stdlib's verbatim file-read primitive, recompute HMAC-SHA256, and assert byte-equality with `expected-signature.txt`.
- **Plan 05 (docs) and Plan 06 (CI matrix)** can reference this fixture directory directly.
- **Sentinel in justfile** lets Plans 02/03/04 each append `uat-webhook-receiver-<lang>` + `uat-webhook-receiver-<lang>-verify-fixture` recipes in parallel.

## Self-Check: PASSED

All 7 fixture files exist on disk; `src/webhooks/dispatcher.rs` and `justfile` were modified as documented. Commits `0cc4027`, `3a70cee`, `83d983f` are present in `git log --oneline --all`. The lock test passes; the full webhook suite passes; `just openssl-check` returns empty.
