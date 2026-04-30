---
phase: 19-webhook-hmac-signing-receiver-examples
plan: 02
subsystem: webhooks
tags: [webhooks, hmac, sha256, python, receiver, standard-webhooks-v1, interop, stdlib]

# Dependency graph
requires:
  - phase: 19-webhook-hmac-signing-receiver-examples
    plan: 01
    provides: "tests/fixtures/webhook-v1/ — 5 fixture data files + sentinel anchor in justfile"
provides:
  - "examples/webhook-receivers/python/receiver.py — stdlib-only mini-server + verify_signature core + --verify-fixture mode"
  - "examples/webhook-receivers/python/README.md — per-receiver run-and-verify docs"
  - "justfile::uat-webhook-receiver-python — foreground UAT recipe (port 9991)"
  - "justfile::uat-webhook-receiver-python-verify-fixture — CI gate (canonical + 3 tamper variants)"
affects: [19-05-docs, 19-06-ci-matrix, 20-webhook-retries]

# Tech tracking
tech-stack:
  added: []  # Python stdlib only — no new Rust crates (D-24 satisfied; openssl-check still empty)
  patterns:
    - "Stdlib-only reference receiver: hmac + hashlib + base64 + http.server + socketserver — no pip install required"
    - "Recipe-driven tamper variants: canonical pass + 3 mutated copies (secret, body, timestamp) under mktemp -d so the in-tree fixture is never mutated"
    - "Drift-skip fixture-verify mode: HMAC + multi-token parse still run; ONLY the |now - ts| > 300s check is skipped (fixture timestamps are intentionally past, locked by Plan 01)"
    - "Multi-token signature parse: split on space, iterate v1, prefixes — forward-compat with Standard Webhooks v1.3+ multi-secret rotation"

key-files:
  created:
    - "examples/webhook-receivers/python/receiver.py (205 lines including docstring + per-section comments)"
    - "examples/webhook-receivers/python/README.md (61 lines, 8-section per-receiver page)"
    - ".planning/phases/19-webhook-hmac-signing-receiver-examples/19-02-SUMMARY.md (this file)"
  modified:
    - "justfile (appended 2 recipes after the # === Phase 19 webhook receivers === sentinel introduced by Plan 01)"

key-decisions:
  - "Receiver is 205 lines — wider than the plan's '~80-120 LOC' hint because the verbatim content blueprint provided in the plan's <action> section was inserted as-written (with docstring, per-section dividers, and full handler scaffolding). The plan's verify hint of 130-145 lines and the acceptance criteria do not enforce a strict line count; behavior is what matters."
  - "Python 3.14.4 was the host's `python3` — recipe and ast.parse both pass, and the receiver code uses zero 3.10+/3.12-only syntax (no walrus-in-comprehension, no PEP 695 type aliases). Any Python 3.8+ should work as the README claims."

patterns-established:
  - "examples/webhook-receivers/<lang>/receiver.<ext> + README.md (8 sections) + 2 just recipes per language — Plans 03 (Go) and 04 (Node) will mirror this exact shape"
  - "Tamper-variant orchestration in just recipes: canonical + 3 mutations under mktemp -d, exit 0 only if all 4 outcomes match expectations; CI-gateable from day one"

requirements-completed: [WH-04]

# Metrics
duration: ~3 min
completed: 2026-04-30
---

# Phase 19 Plan 02: Python Webhook Receiver Reference Summary

**Shipped a stdlib-only Python reference webhook receiver that verifies cronduit's Standard Webhooks v1 signatures using `hmac.compare_digest` (constant-time per WH-04), plus 2 just recipes (`uat-webhook-receiver-python` foreground + `uat-webhook-receiver-python-verify-fixture` CI gate with 4 tamper variants). The receiver maps verify outcomes to the D-12 retry contract status codes (400/401/200/503) and includes a `--verify-fixture <dir>` mode that drift-skips against the past-timestamped fixture from Plan 01.**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-04-30
- **Completed:** 2026-04-30
- **Tasks:** 3
- **Files created:** 2 (receiver.py + README.md)
- **Files modified:** 1 (justfile)

## Accomplishments

- `examples/webhook-receivers/python/receiver.py` — stdlib-only (`hmac`, `hashlib`, `base64`, `http.server`, `socketserver`, `os`, `sys`, `time`); 205 LOC including docstring + per-section dividers
- Constant-time HMAC-SHA256 verify via `hmac.compare_digest` with the `# constant-time compare per WH-04` comment immediately above the call
- `verify_signature(secret_bytes, headers, body_bytes) -> bool` is a copy-pasteable top-of-file function ready to drop into Flask/gin/Express receivers
- HTTP-mode status code mapping per D-12: 400 missing/malformed/drift, 401 mismatch, 200 success, 503 catch-all
- 1 MiB body cap before `rfile.read`; `Connection: close` framing on every response
- Multi-token signature parse (`v1,sigA v1,sigB`) — forward-compat with Standard Webhooks v1.3+ multi-secret rotation
- `--verify-fixture <dir>` mode reads the 5 fixture files and runs the same `_verify_with_drift` core that the HTTP path uses, with `check_drift=False` (HMAC + multi-token parse still run; ONLY drift is skipped)
- `examples/webhook-receivers/python/README.md` — 8-section per-receiver page (title, intro, install, run, expected log, troubleshooting table, verbatim SHA-256-only note, see-also cross-links)
- 2 new just recipes appended after the `# === Phase 19 webhook receivers ===` sentinel anchor (introduced by Plan 01 Task 3) so Plans 03 (Go) + 04 (Node) can append at the same anchor without colliding
- `just uat-webhook-receiver-python-verify-fixture` exercises canonical pass + 3 tamper variants (mutated-secret, mutated-body, mutated-timestamp) under `mktemp -d` and exits 0 only if all 4 outcomes are correct
- `just openssl-check` still empty across native + arm64-musl + amd64-musl (D-24 — Plan 02 added zero Rust crates)

## Task Commits

1. **Task 1: Create `examples/webhook-receivers/python/receiver.py`** — `ffb79f8` (feat)
2. **Task 2: Create `examples/webhook-receivers/python/README.md`** — `ab7ee93` (docs)
3. **Task 3: Add 2 just recipes for Python receiver UAT** — `8c8fdde` (feat)

_Final metadata commit happens at orchestrator level._

## Files Created/Modified

### Created

- `examples/webhook-receivers/python/receiver.py` — 205 lines, stdlib only, verify_signature top-of-file copy-pasteable core, HTTP server on 127.0.0.1:9991, `--verify-fixture` drift-skip mode, idempotency comment block in success branch, dual-log to stdout + `/tmp/cronduit-webhook-receiver-python.log`, header comment loudly warns "USE ONLY for local maintainer UAT".
- `examples/webhook-receivers/python/README.md` — 61 lines, 8 sections per PATTERNS.md spec: Title, Intro, Install (Stdlib only), Run (HTTP + fixture-verify), Expected log output, Troubleshooting table (5 rows: 401 / 400 missing / 400 drift / 503 misconfigured / coalescing), SHA-256-only note (verbatim), See also (docs/WEBHOOKS.md + sibling go/node + Phase 18 mock).

### Modified

- `justfile` — appended 2 recipes after the `# === Phase 19 webhook receivers ===` sentinel (line 354, introduced by Plan 01 Task 3): `uat-webhook-receiver-python` (foreground) + `uat-webhook-receiver-python-verify-fixture` (CI gate). Both carry `[group('uat')]` + `[doc('Phase 19 — ...')]` decorators for consistency with the Phase 18 family.

## 4 Tamper Variants Encoded in `uat-webhook-receiver-python-verify-fixture`

| Variant | Mutation | Expected outcome |
|---|---|---|
| 1. Canonical | None — original fixture | `OK: fixture verified` (exit 0) |
| 2. Mutated secret | `printf 'WRONG' > secret.txt` | HMAC mismatch — `_verify_with_drift` returns False, recipe sees non-zero exit, asserts FAIL outcome |
| 3. Mutated body | `sed 's/"v1"/"X1"/' payload.json` (changes `payload_version`) | HMAC over different body bytes mismatches `expected-signature.txt` |
| 4. Mutated timestamp | `printf '%s' "$(($(date +%s) - 600))" > webhook-timestamp.txt` | HMAC computed over `${id}.${NEW_TS}.${body}` mismatches the canonical signature pinned in `expected-signature.txt`. NB: drift detection itself is exercised by U6/U7/U8 live UAT scenarios, not this recipe. |

All 4 outcomes asserted by the recipe; an always-true `verify_signature` would pass canonical but fail tampers, catching both regression directions.

## Verification Receipts

- `python3 -c "import ast; ast.parse(open('examples/webhook-receivers/python/receiver.py').read())"` exit 0
- `python3 examples/webhook-receivers/python/receiver.py --verify-fixture tests/fixtures/webhook-v1` → `OK: fixture verified` (exit 0)
- `just uat-webhook-receiver-python-verify-fixture` → `OK: all 4 tamper variants behave correctly` (exit 0)
- `grep -q "hmac.compare_digest" examples/webhook-receivers/python/receiver.py` → match
- `grep -q "MAX_TIMESTAMP_DRIFT_SECONDS = 300" examples/webhook-receivers/python/receiver.py` → match
- `grep -q "constant-time compare per WH-04" examples/webhook-receivers/python/receiver.py` → match
- `grep -q "PORT = 9991" examples/webhook-receivers/python/receiver.py` → match
- `grep -q "loopback-only" examples/webhook-receivers/python/receiver.py` → match (Issue 7)
- `grep -q "D-09" examples/webhook-receivers/python/receiver.py` → match (Issue 7)
- `grep -E "^import (flask|requests|urllib3|cryptography)" examples/webhook-receivers/python/receiver.py` → empty (D-02 stdlib-only)
- `grep -E '(==|!=)\s*expected' examples/webhook-receivers/python/receiver.py` → empty (no plain `==` against expected)
- `grep -q "Cronduit v1.2 ships SHA-256 only" examples/webhook-receivers/python/README.md` → match (verbatim)
- `grep -q "WEBHOOK_SECRET_FILE" examples/webhook-receivers/python/README.md` → match
- `grep -q "127.0.0.1:9991" examples/webhook-receivers/python/README.md` → match
- `grep -q "verify-fixture tests/fixtures/webhook-v1" examples/webhook-receivers/python/README.md` → match
- `just --list 2>&1 | grep -c "uat-webhook-receiver-python"` → 2
- `just openssl-check` → `OK: no openssl-sys in dep tree (native + arm64-musl + amd64-musl)` (D-24)

## Decisions Made

- **Receiver line count is 205, wider than the plan's `~80-120 LOC` hint:** The plan's `<action>` section provided a verbatim content blueprint to write byte-for-byte; that blueprint includes a long module docstring, three section divider comments, and a full handler scaffold. The acceptance criteria do not enforce a strict line count, only grep-presence + behavior. The receiver still satisfies D-04's intent (the copy-pasteable `verify_signature` core stays small and clearly separated at the top of the file).
- **Drift-skip in `--verify-fixture` mode is encoded as a `check_drift` keyword-only parameter on a sibling private `_verify_with_drift` function**, not as a duplicate of `verify_signature`. The public `verify_signature` always checks drift (HTTP path); fixture-verify mode calls the private form with `check_drift=False`. HMAC + multi-token parse + length guards still run.
- **Tamper-variant 4 is "Mutated timestamp" (Issue 8 rename), not "drift>5min":** Because fixture-verify mode skips the drift check, a stale-timestamp variant proves *nothing* in fixture-verify mode unless the HMAC is also affected. Mutating `webhook-timestamp.txt` while leaving `expected-signature.txt` as the canonical sig produces an HMAC mismatch for the new `(id, NEW_TS, body)` tuple — caught by the constant-time compare. The recipe comments explain this clearly.

## Deviations from Plan

None — plan executed exactly as written, with one observation:

- The acceptance-criteria grep `grep -B1 "^uat-webhook-receiver-python:" justfile | grep -q "\[group('uat')\]"` is overly narrow because `[doc(...)]` sits between `[group(...)]` and the recipe target line; `grep -B1` only catches `[doc]`. I verified the semantic intent with `grep -B2` instead (both decorators are present and confirmed). This is not a deviation in the produced artifact — both recipes carry both decorators, exactly as the plan specifies. Future plan refinement could change the grep to `grep -B2` for clarity.

## Issues Encountered

None — all 3 tasks executed cleanly. The pre-existing fixture (Plan 01) verified correctly on the first run of the canonical-path; all 3 tamper mutations failed verification as expected; the live `just` recipe exited 0 with the expected `OK: all 4 tamper variants behave correctly`.

## Threat Flags

None — Plan 02 mitigations match the threat register exactly:
- T-19-05 (Spoofing): HMAC verify with constant-time compare implemented
- T-19-06 (Tampering): body read via `rfile.read(content_length)`; no `json.loads/dumps` round-trip
- T-19-07 (Information Disclosure): `hmac.compare_digest` is the constant-time primitive (commented above the call)
- T-19-08 (Replay): drift check `abs(now - ts) > 300` in HTTP path; explicitly skipped only in `--verify-fixture` mode (HMAC always checked)
- T-19-09 (DoS): 1 MiB body cap before `rfile.read`
- T-19-10 (Elevation): catch-all `except Exception` returns 503 transient

No new security surface introduced beyond what the threat model already covers.

## Next Phase Readiness

- **Plans 03 (Go) and 04 (Node) are unblocked.** Each has an in-tree reference shape to mirror:
  - File layout: `examples/webhook-receivers/<lang>/{receiver.<ext>, README.md}`
  - Port allocation: 9991 (Python, taken), 9992 (Go), 9993 (Node)
  - Recipe shape: `uat-webhook-receiver-<lang>` (foreground) + `uat-webhook-receiver-<lang>-verify-fixture` (CI gate, canonical + 3 tamper variants)
  - The `verify_signature` interface (secret_bytes, headers, body_bytes -> bool) is the language-portable reference design
- **Plan 05 (docs)** can now reference `examples/webhook-receivers/python/` as a real in-tree path for the `docs/WEBHOOKS.md` "Receiver examples" section
- **Plan 06 (CI matrix)** can wire `just uat-webhook-receiver-python-verify-fixture` into the GHA `webhook-interop` job — the recipe is already a clean CI gate

## Self-Check: PASSED

- `examples/webhook-receivers/python/receiver.py` exists (verified `[ -f ... ]`)
- `examples/webhook-receivers/python/README.md` exists
- Commits `ffb79f8`, `ab7ee93`, `8c8fdde` are present in `git log --oneline -5` (verified)
- `just uat-webhook-receiver-python-verify-fixture` exits 0 with the expected message (verified above)
- `just openssl-check` returns the expected OK string (verified above)
