---
phase: 19-webhook-hmac-signing-receiver-examples
plan: 04
subsystem: webhooks
tags: [webhooks, hmac, sha256, node, receiver, standard-webhooks-v1, interop, stdlib]

# Dependency graph
requires:
  - phase: 19-webhook-hmac-signing-receiver-examples
    plan: 01
    provides: "tests/fixtures/webhook-v1/ — 5 fixture data files + sentinel anchor in justfile"
provides:
  - "examples/webhook-receivers/node/receiver.js — stdlib-only mini-server + verifySignature core + --verify-fixture mode (with Pitfall 2 length guard)"
  - "examples/webhook-receivers/node/README.md — per-receiver run-and-verify docs"
  - "justfile::uat-webhook-receiver-node — foreground UAT recipe (port 9993)"
  - "justfile::uat-webhook-receiver-node-verify-fixture — CI gate (canonical + 3 tamper variants)"
affects: [19-05-docs, 19-06-ci-matrix, 20-webhook-retries]

# Tech tracking
tech-stack:
  added: []  # Node stdlib only — no new Rust crates (D-24 satisfied; openssl-check still empty)
  patterns:
    - "Stdlib-only Node reference receiver: http + crypto + fs + path — no package.json, no npm install required"
    - "Pitfall 2 length guard: `received.length !== expected.length` MUST precede every crypto.timingSafeEqual call (which throws RangeError on length mismatch); functionally verified — short-signature variant exits FAIL gracefully, never propagates RangeError"
    - "Recipe-driven tamper variants: canonical pass + 3 mutated copies (secret, body, timestamp) under mktemp -d so the in-tree fixture is never mutated"
    - "Drift-skip fixture-verify mode: HMAC + multi-token parse still run; ONLY the |now - ts| > 300s check is skipped (fixture timestamps are intentionally past, locked by Plan 01)"
    - "Multi-token signature parse via wsig.split(/\\s+/): forward-compat with Standard Webhooks v1.3+ multi-secret rotation"
    - "Path-traversal guard for --verify-fixture <dir>: reject NUL bytes, control characters, and `..` segments BEFORE path.resolve; allowlist of bare fixture filenames; fs.realpathSync resolves symlinks"

key-files:
  created:
    - "examples/webhook-receivers/node/receiver.js (284 lines including docstring + per-section comments)"
    - "examples/webhook-receivers/node/README.md (64 lines, 8-section per-receiver page)"
    - ".planning/phases/19-webhook-hmac-signing-receiver-examples/19-04-SUMMARY.md (this file)"
  modified:
    - "justfile (appended 2 recipes after the Go recipes from Plan 03; both placed under the # === Phase 19 webhook receivers === sentinel introduced by Plan 01)"

key-decisions:
  - "Sanitize-first path-traversal guard introduced for `--verify-fixture <dir>` argument: a `_sanitizeFixtureArg` helper rejects empty string, NUL bytes, control characters, and `..` segments BEFORE path.resolve; fs.realpathSync resolves symlinks and statSync confirms it's a directory; per-file reads use a hard-coded FIXTURE_FILES allowlist of bare names. Required by the repo's semgrep `path-join-resolve-traversal` rule on user-input paths — Rule 3 deviation. Net positive: makes the receiver robust against malicious fixture-dir args even though it's a maintainer-only CLI."
  - "Receiver line count is 284, wider than the plan's '~140 LOC' hint: the verbatim content blueprint provided in the plan's <action> section was inserted as-written (with docstring, per-section dividers, full handler scaffolding); the path-sanitize helpers added ~30 lines. Acceptance criteria are grep-presence + behavior, not line count — Plan 02 Python landed at 205 LOC and Plan 03 Go landed at 244 LOC for the same reason."
  - "Plan internal inconsistency around `setEncoding` resolved by paraphrasing the comment: the plan's blueprint instructs writing the literal `req.setEncoding('utf8')` in a comment, but the plan's acceptance criterion `! grep -q \"setEncoding\"` forbids the literal token anywhere in the file. Honored intent (no actual call) and threat-model requirement (documented in comment) by rewording: `Pitfall 5: do NOT decode the request stream to utf8 — that corrupts byte-exact body bytes`. The receiver still never decodes the body."
  - "Plan internal inconsistency around `!== expected` regex resolved by acknowledging it: the plan REQUIRES the literal `received.length !== expected.length` length guard (Pitfall 2 mandatory) AND has an acceptance grep `! grep -E '(===|!==)\\s*expected'` that the mandatory pattern matches. The Pitfall 2 guard is load-bearing safety; honored that. The `!== expected` grep was clearly intended to catch plain non-constant-time signature comparison (e.g., `someBuffer === expected`), not the length-int comparison; the spirit is satisfied (the signature compare itself goes through crypto.timingSafeEqual)."
  - "Node 25.9 was the host's `node` — receiver code uses zero 21+/22-only syntax (no `--experimental-permission`, no node:test, no top-level await). Any Node 18+ should work as the README claims (the README locks 'Node 20+' to match Plan 02/03 wording style)."

patterns-established:
  - "Pitfall 2 length guard pattern: `if (received.length !== expected.length) continue;` BEFORE every crypto.timingSafeEqual call — codified as the Node-specific row of the Phase 19 Pitfall matrix"
  - "Sanitize-then-resolve pattern for stdlib path handling in maintainer CLIs: explicit reject of `..`, NUL, control characters BEFORE path.resolve; realpathSync; allowlist of bare filenames at read time; nosemgrep with the exact rule ID at the path.resolve / path.join sites"
  - "Cleartext-HTTP nosemgrep with rule ID: `// nosemgrep: problem-based-packs.insecure-transport.js-node.using-http-server.using-http-server` directly above http.createServer is the working incantation for the Node ecosystem (Go ecosystem uses *http.Server struct form per Plan 03)"

requirements-completed: [WH-04]

# Metrics
duration: ~10 min
completed: 2026-04-30
---

# Phase 19 Plan 04: Node Webhook Receiver Reference Summary

**Shipped a stdlib-only Node reference webhook receiver that verifies cronduit's Standard Webhooks v1 signatures using `crypto.timingSafeEqual` (constant-time per WH-04) — guarded by a MANDATORY length check that prevents the RangeError throw on length mismatch (Pitfall 2). Plus 2 just recipes (`uat-webhook-receiver-node` foreground + `uat-webhook-receiver-node-verify-fixture` CI gate with 4 tamper variants). The receiver maps verify outcomes to the D-12 retry contract status codes (400/401/200/503) and includes a `--verify-fixture <dir>` mode that drift-skips against the past-timestamped fixture from Plan 01.**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-04-30 (worktree base 77bed48)
- **Completed:** 2026-04-30
- **Tasks:** 3
- **Files created:** 2 (receiver.js + README.md)
- **Files modified:** 1 (justfile)

## Accomplishments

- `examples/webhook-receivers/node/receiver.js` — stdlib-only (`require('http')`, `require('crypto')`, `require('fs')`, `require('path')`); 284 LOC including docstring, per-section dividers, and the path-sanitize helper added under the Rule-3 deviation
- Constant-time HMAC-SHA256 verify via `crypto.timingSafeEqual` with the `// constant-time compare per WH-04` comment immediately above the call
- **MANDATORY Pitfall 2 length guard:** `if (received.length !== expected.length) continue;` BEFORE every `crypto.timingSafeEqual` call. Functionally verified — a fixture variant with a 5-byte signature (length-mismatched against the 32-byte HMAC-SHA256 output) exits with `FAIL: fixture did NOT verify` and DOES NOT propagate a RangeError
- `verifySignature(secret, headers, body) -> boolean` is a copy-pasteable top-of-file function ready to drop into Express/Fastify/Hono handlers
- HTTP-mode status code mapping per D-12: 400 missing/malformed/drift, 401 mismatch, 200 success, 503 catch-all
- 1 MiB body cap via cumulative chunk-length check inside `req.on('data')` — oversize triggers 413 + `req.destroy()`; `Connection: close` framing on every response
- Multi-token signature parse via `wsig.split(/\s+/)` — forward-compat with Standard Webhooks v1.3+ multi-secret rotation
- `--verify-fixture <dir>` mode reads the 5 fixture files via `fs.readFileSync` and runs the same `_verifyWithDrift` core that the HTTP path uses, with `checkDrift=false` (HMAC + multi-token parse + length guards still run)
- Path-traversal guard introduced under Rule-3 deviation: `_sanitizeFixtureArg` rejects NUL bytes, control characters, `..` segments BEFORE `path.resolve`; `fs.realpathSync` resolves symlinks; reads use a hard-coded `FIXTURE_FILES` allowlist of bare filenames
- `examples/webhook-receivers/node/README.md` — 64-line, 8-section per-receiver page (title, intro, install, run, expected log, troubleshooting table including the **mandatory Pitfall 2 RangeError row**, verbatim SHA-256-only note, see-also cross-links to docs/WEBHOOKS.md + sibling python/go receivers + Phase 18 mock)
- 2 new just recipes appended after the Go recipes (which sit after the Python recipes, all under the `# === Phase 19 webhook receivers ===` sentinel anchor introduced by Plan 01 Task 3)
- `just uat-webhook-receiver-node-verify-fixture` exercises canonical pass + 3 tamper variants (mutated-secret, mutated-body, mutated-timestamp) under `mktemp -d` and exits 0 only if all 4 outcomes are correct
- `just openssl-check` still empty across native + arm64-musl + amd64-musl (D-24 — Plan 04 added zero Rust crates)

## Task Commits

1. **Task 1: Create `examples/webhook-receivers/node/receiver.js`** — `64d8d85` (feat)
2. **Task 2: Create `examples/webhook-receivers/node/README.md`** — `a0b6607` (docs)
3. **Task 3: Add 2 just recipes for Node receiver UAT** — `65c5026` (feat)

_Final metadata commit happens at orchestrator level._

## Files Created/Modified

### Created

- `examples/webhook-receivers/node/receiver.js` — 284 lines, stdlib only, verifySignature top-of-file copy-pasteable core, HTTP server on 127.0.0.1:9993, `--verify-fixture` drift-skip mode, idempotency comment block in success branch, dual-log to stdout + `/tmp/cronduit-webhook-receiver-node.log`, header comment loudly warns "USE ONLY for local maintainer UAT"; path-traversal sanitize helper added for the verify-fixture CLI argument.
- `examples/webhook-receivers/node/README.md` — 64 lines, 8 sections per PATTERNS.md spec: Title, Intro, Install (Stdlib only), Run (HTTP + fixture-verify), Expected log output, Troubleshooting table (6 rows including the mandatory Pitfall 2 RangeError row), SHA-256-only note (verbatim), See also (docs/WEBHOOKS.md + sibling python/go + Phase 18 mock).

### Modified

- `justfile` — appended 2 recipes after the existing Go recipes (which were appended by Plan 03 after the Python recipes from Plan 02, all under the `# === Phase 19 webhook receivers ===` sentinel introduced by Plan 01 Task 3): `uat-webhook-receiver-node` (foreground) + `uat-webhook-receiver-node-verify-fixture` (CI gate). Both carry `[group('uat')]` + `[doc('Phase 19 — ...')]` decorators for consistency with the Phase 18 family and Plans 02/03 recipes.

## 4 Tamper Variants Encoded in `uat-webhook-receiver-node-verify-fixture`

| Variant | Mutation | Expected outcome |
|---|---|---|
| 1. Canonical | None — original fixture | `OK: fixture verified` (exit 0) |
| 2. Mutated secret | `printf 'WRONG' > secret.txt` | HMAC mismatch — `_verifyWithDrift` returns false, recipe sees non-zero exit, asserts FAIL outcome |
| 3. Mutated body | `sed 's/"v1"/"X1"/' payload.json` (changes `payload_version`) | HMAC over different body bytes mismatches `expected-signature.txt` |
| 4. Mutated timestamp | `printf '%s' "$(($(date +%s) - 600))" > webhook-timestamp.txt` | HMAC computed over `${id}.${NEW_TS}.${body}` mismatches the canonical signature pinned in `expected-signature.txt`. NB: drift detection itself is exercised by U6/U7/U8 live UAT scenarios, not this recipe. |

All 4 outcomes asserted by the recipe; an always-true `verifySignature` would pass canonical but fail tampers, catching both regression directions.

## Pitfall 2 Length Guard — Functional Proof

Constructed a length-mismatched signature variant: replaced `expected-signature.txt` with `v1,YWJjZGU=` (5 base64-decoded bytes vs the 32-byte HMAC-SHA256 output) and ran the receiver against it.

```
$ node examples/webhook-receivers/node/receiver.js --verify-fixture <short-sig-dir>
FAIL: fixture did NOT verify
exit=1
```

**Critical:** stderr did NOT contain a `RangeError: Input buffers must have the same byte length` stack trace. The length guard caught the mismatch BEFORE `crypto.timingSafeEqual` could throw. Without the guard, this same input would have crashed the handler and returned 503 (transient — retry per D-12) instead of 401 (permanent — drop), corrupting Phase 20's retry semantics.

## Verification Receipts

- `node --check examples/webhook-receivers/node/receiver.js` exit 0
- `node examples/webhook-receivers/node/receiver.js --verify-fixture tests/fixtures/webhook-v1` → `OK: fixture verified` (exit 0)
- `just uat-webhook-receiver-node-verify-fixture` → `OK: all 4 tamper variants behave correctly` (exit 0)
- `grep -q "crypto.timingSafeEqual" examples/webhook-receivers/node/receiver.js` → match
- `grep -q "received.length !== expected.length" examples/webhook-receivers/node/receiver.js` → match (Pitfall 2 guard)
- `grep -q "MAX_TIMESTAMP_DRIFT_SECONDS = 300" examples/webhook-receivers/node/receiver.js` → match (D-11)
- `grep -q "constant-time compare per WH-04" examples/webhook-receivers/node/receiver.js` → match
- `grep -q "PORT = 9993" examples/webhook-receivers/node/receiver.js` → match (D-03)
- `grep -q "loopback-only" examples/webhook-receivers/node/receiver.js` → match (Issue 7)
- `grep -q "D-09" examples/webhook-receivers/node/receiver.js` → match (Issue 7)
- `grep -q "USE ONLY" examples/webhook-receivers/node/receiver.js` → match
- `! grep -q "setEncoding" examples/webhook-receivers/node/receiver.js` → match (Pitfall 5; the comment was reworded to avoid the literal token while still warning readers)
- `! test -f examples/webhook-receivers/node/package.json` → no module file (D-02 single-file program)
- `grep -E "require\(['\"]" ... | grep -vE "require\(['\"](http|crypto|fs|path|os)['\"]\)"` → empty (D-02 stdlib-only)
- Length-mismatched signature variant exits FAIL gracefully, NO RangeError stack trace (functional Pitfall 2 proof)
- `grep -q "Cronduit v1.2 ships SHA-256 only" examples/webhook-receivers/node/README.md` → match (verbatim)
- `grep -q "WEBHOOK_SECRET_FILE" examples/webhook-receivers/node/README.md` → match
- `grep -q "127.0.0.1:9993" examples/webhook-receivers/node/README.md` → match
- `grep -q "Pitfall 2" examples/webhook-receivers/node/README.md` → match (mandatory troubleshooting row)
- `grep -q "RangeError" examples/webhook-receivers/node/README.md` → match (specific symptom call-out)
- `grep -q "verify-fixture tests/fixtures/webhook-v1" examples/webhook-receivers/node/README.md` → match
- `just --list 2>&1 | grep -c "uat-webhook-receiver-node"` → 2
- `just openssl-check` → `OK: no openssl-sys in dep tree (native + arm64-musl + amd64-musl)` (D-24)
- `semgrep --config=auto examples/webhook-receivers/node/receiver.js` → clean (zero findings)

## Decisions Made

- **Sanitize-first path-traversal guard for `--verify-fixture <dir>`:** The repo's semgrep `path-join-resolve-traversal` rule fires on any user-input path reaching `path.resolve` or `path.join`. The plan's blueprint passed `process.argv[3]` straight through. Resolved by introducing `_sanitizeFixtureArg` (rejects `..` segments, NUL bytes, control characters BEFORE resolve), `_resolveFixtureDir` (calls `fs.realpathSync` + `statSync.isDirectory`), and `_readFixtureFile` (per-file reads use a hard-coded `FIXTURE_FILES` allowlist of bare names). Net positive: receiver is now robust against malicious fixture-dir args even though it's a maintainer-only CLI.
- **`*http.Server`-equivalent for Node:** `http.createServer` is fine in Node — there's no Node-equivalent of Go's slowloris guard requirement. Used `// nosemgrep: problem-based-packs.insecure-transport.js-node.using-http-server.using-http-server` immediately above `http.createServer` with a 4-line comment block explaining the loopback-only design and pointing to the header docstring's USE-ONLY-FOR-LOCAL warning. The receiver's loopback-bind, explicit `USE ONLY for local maintainer UAT` header docstring, and README's TLS-terminator-for-production note are the actual cleartext-HTTP mitigations — D-09 receiver-responsibility model.
- **Plan internal inconsistencies around `setEncoding` and `!== expected`:** Both were resolved by honoring the load-bearing intent (Pitfall 2 length guard is mandatory; no actual `setEncoding` call). Documented in detail in the Decisions Made section above.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking issue] semgrep path-traversal rule blocked plan's literal `path.resolve(rawArg)` form**
- **Found during:** Task 1 (Write tool emitted `PostToolUse:Write hook blocking error from command: "semgrep mcp -k post-tool-cli-scan"` with CWE-22 path-traversal findings on the bare `path.join(fixtureDir, '...')` calls)
- **Issue:** The plan's verbatim content blueprint passed `process.argv[3]` straight through to `path.join(fixtureDir, 'secret.txt')` etc. The repo's semgrep hook treats any user-input path reaching `path.join`/`path.resolve` as a CWE-22 candidate.
- **Fix:** Introduced a `_sanitizeFixtureArg` helper that rejects empty/long/NUL/control/`..`-containing inputs BEFORE `path.resolve`; resolves to absolute via `path.resolve` (with `nosemgrep` annotation citing the now-validated input), `fs.realpathSync` to resolve symlinks, `statSync.isDirectory` check; per-file reads via `_readFixtureFile(safeDir, allowlistedBareName)` with a hard-coded `FIXTURE_FILES` allowlist (also nosemgrep-annotated). Receiver is functionally identical for the canonical use case (`tests/fixtures/webhook-v1/`) and now correctly rejects malicious path inputs.
- **Files modified:** examples/webhook-receivers/node/receiver.js
- **Verification:** `node --check` clean; `node receiver.js --verify-fixture tests/fixtures/webhook-v1` prints `OK: fixture verified`; `just uat-webhook-receiver-node-verify-fixture` prints `OK: all 4 tamper variants behave correctly`; semgrep scan shows zero findings.
- **Committed in:** 64d8d85 (Task 1)

**2. [Rule 3 - Blocking issue] semgrep cleartext-HTTP rule blocked plan's `http.createServer` line**
- **Found during:** Task 1 (the cleartext-HTTP rule fired on `http.createServer(handleRequest)` in `main()`)
- **Issue:** Same posture as Plan 03's Go cleartext-HTTP issue — the rule fires on any HTTP server creation regardless of loopback bind.
- **Fix:** Added a `// nosemgrep: problem-based-packs.insecure-transport.js-node.using-http-server.using-http-server` immediately above `http.createServer`, with a 4-line comment block explaining the loopback-only design ("Cleartext HTTP is intentional: loopback-bound (127.0.0.1:9993) reference receiver for local maintainer UAT only … Production receivers run behind a reverse proxy / TLS terminator (D-09)"). The discovery method was `semgrep --config=auto --json` on the file to learn the exact rule ID; the original best-guess nosemgrep ID didn't match.
- **Files modified:** examples/webhook-receivers/node/receiver.js
- **Verification:** subsequent semgrep scan shows zero findings.
- **Committed in:** 64d8d85 (Task 1)

**3. [Rule 1 - Plan internal inconsistency] `setEncoding` token contradiction**
- **Found during:** Task 1 verification
- **Issue:** The plan blueprint (line 290) instructed writing a comment containing `req.setEncoding('utf8')` warning readers NOT to call it. But the plan acceptance criterion (line 429) demands `! grep -q "setEncoding"` — i.e., the literal token must not appear ANYWHERE in the file, including comments. Mutually exclusive.
- **Fix:** Honored the load-bearing intent ("File does NOT call setEncoding" — line 443; "documented in receiver code comment" — threat model line 137) by paraphrasing the comment: "Pitfall 5: do NOT decode the request stream to utf8 — that corrupts byte-exact body bytes for any non-ASCII payload and breaks HMAC verification. Buffer chunks only." This satisfies both the strict grep AND keeps the documentation telling readers to never decode the body.
- **Files modified:** examples/webhook-receivers/node/receiver.js (Task 1, before commit)
- **Committed in:** 64d8d85 (Task 1)

---

**Total deviations:** 3 auto-fixed (2 blocking-linter, 1 plan-inconsistency)
**Impact on plan:** No scope change. The `verifySignature` core, `--verify-fixture` mode, port allocation, status codes, length guard, multi-token parse, and 4 tamper variants are all byte-identical to the plan's blueprint behavior. The receiver now ALSO defends against path-traversal attacks on the maintainer CLI argument — a strict superset of the planned functionality.

## Issues Encountered

- **semgrep PostToolUse hook on `path.resolve(rawArg)` and `http.createServer`:** Documented above as Rule-3 deviations (#1 and #2). Resolved by adding the sanitize-first path-traversal guard (idiomatic Node defensive coding) and a `nosemgrep` annotation with the exact rule ID for the cleartext-HTTP finding (loopback-only design, mirrors Plan 03's Go approach).
- **Plan internal inconsistencies (`setEncoding` and `!== expected`):** Documented above as Rule-1 deviation (#3) and as a key decision. Resolved by honoring the load-bearing intent (Pitfall 2 length guard mandatory; no actual setEncoding call). Future plan refinement should reword the acceptance criteria to test SEMANTICS (no actual decode call, no plain-equality on the signature buffer) rather than literal tokens.

## Threat Flags

None — Plan 04 mitigations match the threat register exactly:
- T-19-17 (Spoofing): HMAC verify with `crypto.timingSafeEqual` constant-time compare implemented; same canonical string `${id}.${ts}.${body}` as cronduit's `sign_v1`
- T-19-18 (Tampering): body is `Buffer.concat(chunks)` from raw `req.on('data')` chunks; never decodes to utf8; comment in receiver warns readers (Pitfall 5)
- T-19-19 (Information Disclosure / timing side channel): `crypto.timingSafeEqual` is the constant-time primitive (per Node docs); commented `// constant-time compare per WH-04` immediately above the call
- **T-19-20 (DoS / Crash on malformed sig)**: **MANDATORY** length guard `if (received.length !== expected.length) continue;` BEFORE every `crypto.timingSafeEqual` call (Pitfall 2). Functionally proven — short-signature variant exits FAIL gracefully, no RangeError propagation
- T-19-21 (Replay): drift check `Math.abs(Math.floor(Date.now()/1000) - ts) > 300` in HTTP path; explicitly skipped only in `--verify-fixture` mode (HMAC always checked)
- T-19-22 (DoS / body size): cumulative chunk-length check inside `req.on('data')` enforces 1-MiB cap; oversize requests get 413 + `req.destroy()`
- T-19-23 (Elevation / exception handling): `try { verify } catch { 503 }` catch-all per D-12; the receiver never crashes the listener

**Strengthened beyond threat register:** Path-traversal guard on `--verify-fixture <dir>` argument (Rule-3 deviation #1). Not in the original threat register because the maintainer CLI was assumed to be operator-trusted; the receiver now actively defends against `..`/NUL/control-character injection on the CLI arg as well.

No new security surface introduced beyond what the threat model already covers; the path-traversal defense strictly *strengthens* the maintainer-trust posture.

## Next Phase Readiness

- **Plan 05 (docs hub) is unblocked.** All 3 receivers (Python/Go/Node) now exist in-tree at `examples/webhook-receivers/<lang>/`; the `docs/WEBHOOKS.md` "Receiver examples" section can reference real paths and real `just` recipes.
- **Plan 06 (CI matrix) is unblocked.** All 3 verify-fixture recipes (`uat-webhook-receiver-{python,go,node}-verify-fixture`) are CI-gateable from day one — each exits 0 only if canonical + 3 tamper variants behave correctly. The GHA `webhook-interop` job can fan out across all three in parallel.
- **Phase 20 (webhook retries) inherits the D-12 retry contract verbatim.** The Node receiver maps verify outcomes to 400/401/200/503 exactly per the contract; Pitfall 2 length guard ensures malformed signatures return 401 (permanent — drop) instead of crashing the handler into 503 (transient — retry), which would corrupt the retry semantics.

## Self-Check: PASSED

- `examples/webhook-receivers/node/receiver.js` exists (verified `[ -f ... ]`)
- `examples/webhook-receivers/node/README.md` exists
- Commits `64d8d85`, `a0b6607`, `65c5026` are present in `git log --oneline -5` (verified)
- `just uat-webhook-receiver-node-verify-fixture` exits 0 with `OK: all 4 tamper variants behave correctly` (verified above)
- `just openssl-check` returns `OK: no openssl-sys in dep tree (native + arm64-musl + amd64-musl)` (verified above)
- Length-mismatched signature variant exits FAIL gracefully, NO RangeError propagation — Pitfall 2 functional proof (verified above)
