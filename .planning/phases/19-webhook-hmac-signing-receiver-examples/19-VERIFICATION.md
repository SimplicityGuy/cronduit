---
phase: 19-webhook-hmac-signing-receiver-examples
verified: 2026-04-30T19:35:00Z
status: gaps_found
score: 2/3 must-haves verified
overrides_applied: 0
gaps:
  - truth: "Each shipped receiver implements the Standard Webhooks v1 signing-string convention: HMAC over `${webhook-id}.${webhook-timestamp}.${body}` using the RAW timestamp header bytes (not a parsed-and-re-serialized integer)."
    status: failed
    reason: "Python and Node receivers parse the timestamp header into an integer and reconstruct the signing string from the parsed int (`f\"{wid}.{ts}.\"` / `` `${wid}.${ts}.` ``), while cronduit (Rust) and the Go receiver use the raw header bytes. For the canonical fixture timestamp `1735689600`, the parsed int round-trips byte-equal to the raw header, so the locked CI fixture cannot detect the divergence. For any non-canonical decimal timestamp (`+1735689600`, ` 1735689600`, `01735689600`, etc.), Python/Node compute a DIFFERENT HMAC than Go/Rust. This violates the Standard Webhooks v1 spec, which is explicit that `${timestamp}` is the raw header value. The phase's stated goal — 'lock the wire format across four runtimes' — is not actually locked; it is locked only for the one canonical timestamp shape the fixture happens to use. Empirically reproduced: with the canonical fixture body+id+secret and timestamp `+1735689600`, Go-style HMAC = `FK4jn0iKNv1VBYiGE1dzyE0YFE8mFo+MBEn1qnfJt3U=`, Py-style HMAC = `uPZ22ISxvps6lH9Bmc8tTvCx0dH/OrDHZMBTRxEVgvo=` — divergent."
    artifacts:
      - path: "examples/webhook-receivers/python/receiver.py"
        issue: "Line 67: `signing_str = f\"{wid}.{ts}.\".encode() + body_bytes` — uses parsed int `ts` from `int(wts)` (line 62). Should use raw header `wts`."
      - path: "examples/webhook-receivers/node/receiver.js"
        issue: "Line 82: `mac.update(`${wid}.${ts}.`)` — uses parsed int `ts` from `Number.parseInt(wts, 10)` (line 75). Should use raw header `wts`."
      - path: "tests/fixtures/webhook-v1/webhook-timestamp.txt"
        issue: "Canonical timestamp `1735689600` is its own decimal canonical form, so the locked fixture cannot detect the parse-and-reformat divergence. Add a regression vector with a non-canonical decimal form (e.g., a sister fixture or recipe-internal mutation) once the receivers are fixed."
    missing:
      - "Python receiver: change line 67 from `f\"{wid}.{ts}.\".encode()` to `f\"{wid}.{wts}.\".encode()` — sign over raw header bytes."
      - "Node receiver: change line 82 from `mac.update(`${wid}.${ts}.`)` to `mac.update(`${wid}.${wts}.`)` — sign over raw header bytes."
      - "Add a tamper variant or sister fixture exercising a non-canonical-decimal timestamp (e.g., leading whitespace, leading `+`, leading zero) to lock the contract permanently."
human_verification:
  - test: "U1 — Workspace builds clean (`just ci`)"
    expected: "Recipe exits 0; openssl-check empty across native + arm64-musl + amd64-musl"
    why_human: "Maintainer UAT per D-22 — Claude does not flip checkboxes (project memory `feedback_uat_user_validates.md`)."
  - test: "U2 — Rust fixture lock test green (`just nextest`)"
    expected: "`webhooks::dispatcher::tests::sign_v1_locks_interop_fixture` PASSED in nextest output."
    why_human: "Maintainer UAT per D-22; verifier confirmed locally but maintainer must run the full suite."
  - test: "U3 — Python verify-fixture recipe (4 tamper variants)"
    expected: "`OK: all 4 tamper variants behave correctly`"
    why_human: "Maintainer UAT per D-22; covers canonical pass + mutated-secret + mutated-body + drifted-timestamp."
  - test: "U4 — Go verify-fixture recipe (4 tamper variants)"
    expected: "`OK: all 4 tamper variants behave correctly`"
    why_human: "Maintainer UAT per D-22."
  - test: "U5 — Node verify-fixture recipe (4 tamper variants)"
    expected: "`OK: all 4 tamper variants behave correctly`; no `RangeError` traceback."
    why_human: "Maintainer UAT per D-22; also asserts Pitfall 2 length-guard holds."
  - test: "U6 — Python receiver end-to-end against real cronduit"
    expected: "Receiver logs `[python-receiver] verified webhook-id=<ULID> bytes=<N>`; cronduit log shows 200 from 127.0.0.1:9991."
    why_human: "Live multi-process scenario (3 terminals); cronduit fires real webhook; cannot be verified programmatically without launching `just dev`."
  - test: "U7 — Go receiver end-to-end against real cronduit"
    expected: "Receiver logs `[go-receiver] verified ...`; cronduit log shows 200 from 127.0.0.1:9992."
    why_human: "Live multi-process scenario."
  - test: "U8 — Node receiver end-to-end against real cronduit"
    expected: "Receiver logs `[node-receiver] verified ...`; cronduit log shows 200 from 127.0.0.1:9993."
    why_human: "Live multi-process scenario."
  - test: "U9 — `docs/WEBHOOKS.md` renders cleanly on GitHub"
    expected: "3 mermaid diagrams render as SVG; tables render; SHA-256-only callout visible; no ASCII-art box-drawing characters."
    why_human: "Visual GitHub-render review on the PR `Files changed` tab — GitHub's mermaid renderer cannot be invoked locally."
  - test: "U10 — README + CONFIG.md cross-references render"
    expected: "Both links resolve to `docs/WEBHOOKS.md`; clean Markdown rendering."
    why_human: "Visual GitHub-render review."
  - test: "U11 — `webhook-interop` CI matrix passes on the PR"
    expected: "All 3 cells (python/go/node) GREEN; final-step log contains `OK: all 4 tamper variants behave correctly`."
    why_human: "Read GHA Checks tab on the PR; cannot be verified locally without push."
---

# Phase 19: Webhook HMAC Signing + Receiver Examples Verification Report

**Phase Goal:** Operators can verify webhook authenticity using HMAC-SHA256 and the Standard Webhooks signing-string convention; ship reference receiver examples that demonstrate constant-time HMAC compare.
**Verified:** 2026-04-30T19:35:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| #   | Truth                                                                                                                                                                                                                                              | Status     | Evidence                                                                                                                                                                                                                                                                                                                                                                                                       |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SC1 | Operator who configures `webhook.secret = "..."` sees `webhook-signature: v1,<base64-of-hmac>` where HMAC is over `webhook-id.webhook-timestamp.payload` raw bytes using SHA-256.                                                                  | ✓ VERIFIED | `src/webhooks/dispatcher.rs:138-156` (`sign_v1` HMAC-SHA256 over `${id}.${ts}.${body}`); `:253` (`format!("v1,{sig}")`); Rust unit test `sign_v1_locks_interop_fixture` PASSED locally; Phase 18's `sign_v1_known_fixture`, `signature_uses_standard_base64_alphabet`, `signature_value_is_v1_comma_b64` tests still in tree.                                                                                  |
| SC2 | Operator running shipped Python, Go, Node receiver examples successfully verifies signatures from a real cronduit delivery; each uses constant-time compare primitive (Python `hmac.compare_digest`, Go `hmac.Equal`, Node `crypto.timingSafeEqual`). | ✗ FAILED | All 3 receivers verify the canonical fixture (`OK: fixture verified` × 3 — confirmed locally), and each uses the spec'd constant-time primitive (Python: `receiver.py:79`; Go: `receiver.go:91`; Node: `receiver.js:101`). However, BL-01 from `19-REVIEW.md` is a real spec-compliance defect: Python (`receiver.py:67`) and Node (`receiver.js:82`) sign over the parsed-int form of the timestamp, not the raw header bytes. Empirically reproduced: timestamp `+1735689600` produces divergent HMAC between Python/Node and Go/Rust. The phase's "lock the wire format across four runtimes" promise (CONTEXT D-13/D-14) is not actually locked — it is locked only for canonical decimal timestamps. Standard Webhooks v1 spec is explicit on this point. |
| SC3 | Operator reviewing receiver-example docs sees explicit note that v1.2 ships SHA-256 only (no algorithm-agility / multi-secret rotation cronduit-side; rotation is a receiver concern).                                                              | ✓ VERIFIED | `docs/WEBHOOKS.md` § "SHA-256 only" (line 93-95) + § "Secret rotation" (receiver-side dual-secret guidance); per-receiver `README.md` files all carry the SHA-256-only note in their headers; `docs/CONFIG.md:594` back-link to `WEBHOOKS.md`; `README.md:176` pointer.                                                                                                                                       |

**Score:** 2/3 truths verified

### Required Artifacts

| Artifact                                                                          | Expected                                                                                       | Status      | Details                                                                                                                                              |
| --------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tests/fixtures/webhook-v1/secret.txt`                                            | 37 bytes, no trailing newline                                                                  | ✓ VERIFIED  | `wc -c` = 37; content matches `cronduit-test-fixture-secret-not-real`.                                                                                |
| `tests/fixtures/webhook-v1/webhook-id.txt`                                        | 26 bytes, no trailing newline                                                                  | ✓ VERIFIED  | `wc -c` = 26; content `01HXYZTESTFIXTURE0000000000`.                                                                                                  |
| `tests/fixtures/webhook-v1/webhook-timestamp.txt`                                 | 10 bytes, `1735689600`                                                                         | ✓ VERIFIED  | `wc -c` = 10; canonical Unix epoch.                                                                                                                  |
| `tests/fixtures/webhook-v1/payload.json`                                          | Compact JSON of canonical `WebhookPayload::build`                                              | ✓ VERIFIED  | 349 bytes; Rust `sign_v1_locks_interop_fixture` re-derives byte-equal output.                                                                         |
| `tests/fixtures/webhook-v1/expected-signature.txt`                                | `v1,<base64>` 47 bytes                                                                         | ✓ VERIFIED  | 47 bytes; Rust lock test confirms equality with re-derived `sign_v1` output.                                                                          |
| `tests/fixtures/webhook-v1/.gitattributes`                                        | `* -text` to disable EOL normalization                                                         | ✓ VERIFIED  | 8 bytes; contains `* -text`.                                                                                                                          |
| `tests/fixtures/webhook-v1/README.md`                                             | Provenance + no-newline + regen guidance                                                       | ✓ VERIFIED  | 2539 bytes; contains "no trailing newline" and regen pointer.                                                                                         |
| `src/webhooks/dispatcher.rs::tests::sign_v1_locks_interop_fixture`                 | Rust in-module fixture lock test                                                               | ✓ VERIFIED  | `cargo nextest` PASSED locally; uses `include_bytes!`/`include_str!`.                                                                                  |
| `src/webhooks/dispatcher.rs::tests::print_canonical_payload_bytes`                | `#[ignore]` regen helper                                                                       | ✓ VERIFIED  | Found in dispatcher.rs.                                                                                                                              |
| `examples/webhook-receivers/python/receiver.py`                                   | Stdlib HTTP server on :9991, `hmac.compare_digest`, `--verify-fixture` mode                    | ⚠️ STUB-LIKE | Exists, runs, fixture verifies. BUT signs over parsed int (BL-01) — wire-format mismatch with cronduit/Go for non-canonical timestamps. Spec defect.    |
| `examples/webhook-receivers/python/README.md`                                     | Run instructions + SHA-256-only note                                                           | ✓ VERIFIED  | Present; minor cosmetic issue (claims "stdout" but logs to stderr — WR-04, non-blocking).                                                            |
| `examples/webhook-receivers/go/receiver.go`                                       | Stdlib HTTP server on :9992, `hmac.Equal`, `--verify-fixture` mode                             | ✓ VERIFIED  | Exists, runs, fixture verifies; signs over raw header bytes (`receiver.go:78` `wid + "." + wts + "."`) — spec-correct.                                 |
| `examples/webhook-receivers/go/README.md`                                         | Run instructions                                                                               | ✓ VERIFIED  | Present.                                                                                                                                              |
| `examples/webhook-receivers/node/receiver.js`                                     | Stdlib HTTP server on :9993, `crypto.timingSafeEqual` + length-guard, `--verify-fixture` mode  | ⚠️ STUB-LIKE | Exists, runs, fixture verifies. Length-guard correctly placed (`receiver.js:99`). BUT signs over parsed int (BL-01) — same spec defect as Python.       |
| `examples/webhook-receivers/node/README.md`                                       | Run instructions                                                                               | ✓ VERIFIED  | Present.                                                                                                                                              |
| `docs/WEBHOOKS.md`                                                                | Operator-facing hub with 10 sections, mermaid diagrams, SHA-256-only note, retry contract     | ✓ VERIFIED  | 288 lines; 6 mermaid blocks; § "SHA-256 only" present; § "Retry-aware response codes" with D-12 table; per-receiver links present.                    |
| `docs/CONFIG.md` back-link to `WEBHOOKS.md`                                       | One-line pointer in webhook section                                                            | ✓ VERIFIED  | `docs/CONFIG.md:594`.                                                                                                                                 |
| `README.md` pointer to `WEBHOOKS.md`                                              | One-line pointer                                                                               | ✓ VERIFIED  | `README.md:176`.                                                                                                                                      |
| `examples/cronduit.toml::wh-example-receiver-{python,go,node}`                    | 3 commented-out job blocks (per D-05)                                                          | ✓ VERIFIED  | Lines 266/274/282 present, commented-out as designed.                                                                                                |
| `justfile::uat-webhook-receiver-{python,go,node}` recipes                         | Foreground receiver runners                                                                    | ✓ VERIFIED  | Lines 367, 423, 479.                                                                                                                                  |
| `justfile::uat-webhook-receiver-{python,go,node}-verify-fixture` recipes          | 4-variant verify (canonical + 3 tamper)                                                        | ✓ VERIFIED  | Lines 377, 433, 489; recipes implement canonical + mutated-secret + mutated-body + mutated-timestamp.                                                 |
| `.github/workflows/ci.yml::webhook-interop` matrix job                            | Python/Go/Node matrix gating cross-language drift                                              | ✓ VERIFIED  | Lines 387-412; `matrix.lang: [python, go, node]`; no `continue-on-error: true`; hard gate.                                                            |
| `.planning/phases/19-webhook-hmac-signing-receiver-examples/19-HUMAN-UAT.md`      | 11 unchecked maintainer scenarios per D-22                                                     | ✓ VERIFIED  | All 11 ship `[ ] Maintainer-validated`; sign-off line blank.                                                                                          |

### Key Link Verification

| From                                                     | To                                                                  | Via                                                                               | Status      | Details                                                                                              |
| -------------------------------------------------------- | ------------------------------------------------------------------- | --------------------------------------------------------------------------------- | ----------- | ----------------------------------------------------------------------------------------------------- |
| `sign_v1_locks_interop_fixture`                          | `tests/fixtures/webhook-v1/{*.txt,payload.json}`                    | `include_bytes!`/`include_str!` at compile time                                   | ✓ WIRED     | Rust test PASSED locally.                                                                             |
| `sign_v1_locks_interop_fixture`                          | `sign_v1`                                                            | Direct in-module call                                                             | ✓ WIRED     | Test re-derives signature; assertion holds.                                                          |
| Python receiver                                          | fixture files                                                        | `--verify-fixture <dir>` mode                                                     | ✓ WIRED     | `OK: fixture verified` confirmed locally.                                                             |
| Go receiver                                              | fixture files                                                        | `--verify-fixture <dir>` mode                                                     | ✓ WIRED     | `OK: fixture verified` confirmed locally.                                                             |
| Node receiver                                            | fixture files                                                        | `--verify-fixture <dir>` mode                                                     | ✓ WIRED     | `OK: fixture verified` confirmed locally.                                                             |
| Cronduit `HttpDispatcher`                                | Outbound HTTP request                                                | `sign_v1` → `format!("v1,{sig}")` → `webhook-signature` header                    | ✓ WIRED     | `dispatcher.rs:252-253`.                                                                              |
| `webhook-interop` CI matrix                              | per-language verify-fixture recipes                                  | `just uat-webhook-receiver-${{ matrix.lang }}-verify-fixture`                     | ✓ WIRED     | `ci.yml:412` matches `justfile:377/433/489`.                                                          |
| `docs/WEBHOOKS.md`                                       | per-receiver READMEs                                                 | Markdown links                                                                    | ✓ WIRED     | Section 9 "Receiver examples" links to all 3 README.md files.                                         |
| Python/Node receiver HMAC computation                    | Standard Webhooks v1 spec wire format (raw timestamp bytes)         | `f"{wid}.{ts}."` / `` `${wid}.${ts}.` ``                                          | ✗ NOT_WIRED | Spec contract is "raw header bytes"; receivers use parsed int. Diverges for non-canonical timestamps. |
| Go receiver HMAC computation                             | Standard Webhooks v1 spec wire format (raw timestamp bytes)         | `wid + "." + wts + "."`                                                           | ✓ WIRED     | Uses raw header bytes — spec-correct.                                                                 |

### Data-Flow Trace (Level 4)

| Artifact                                                  | Data Variable                          | Source                                                                       | Produces Real Data                                                                                                                                                                                              | Status     |
| --------------------------------------------------------- | -------------------------------------- | ---------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- |
| `tests/fixtures/webhook-v1/payload.json`                  | canonical 16-field payload            | `WebhookPayload::build` → `serde_json::to_vec`                                | Yes — derived from real Phase 18 payload schema; 349 bytes; Rust test re-derives byte-equal.                                                                                                                  | ✓ FLOWING  |
| `tests/fixtures/webhook-v1/expected-signature.txt`        | `v1,<base64>` HMAC                    | `sign_v1(secret, id, ts, body)` (cronduit's exact production helper)         | Yes — derived from cronduit's actual signer; 47 bytes; Rust test re-derives equal.                                                                                                                            | ✓ FLOWING  |
| Python `verify_signature` HMAC computation                | parsed `ts` integer                    | `int(wts)` line 62 → `f"{wid}.{ts}."` line 67                                | DIVERGES from spec for non-canonical decimal timestamps; only matches Go/Rust by accident when raw header is its own canonical decimal form (which the canonical fixture happens to be).                       | ⚠️ STATIC  |
| Node `verify_signature` HMAC computation                  | parsed `ts` integer                    | `Number.parseInt(wts, 10)` line 75 → `mac.update(`${wid}.${ts}.`)` line 82    | Same divergence as Python; additionally `parseInt` truncates trailing junk (WR-01) and accepts `+`-prefix.                                                                                                    | ⚠️ STATIC  |
| Go `verifyWithDrift` HMAC computation                     | raw `wts` string                       | `r.Header.Get("webhook-timestamp")` → `mac.Write([]byte(wid + "." + wts + "."))` | Yes — uses raw header bytes per Standard Webhooks v1 spec.                                                                                                                                                     | ✓ FLOWING  |
| Cronduit `HttpDispatcher::deliver`                        | `webhook_ts` i64                       | `chrono::Utc::now().timestamp()` → `webhook_ts.to_string()` for header + `format!("{}.{}.", id, webhook_ts)` for sign | Yes — header value is the canonical decimal of the i64 (via `to_string()`); `sign_v1` formats the same i64 via `{webhook_timestamp}` (Display). Cronduit always emits canonical decimal, so BL-01 latent. | ✓ FLOWING  |

### Behavioral Spot-Checks

| Behavior                                                                | Command                                                                                                                  | Result                                          | Status |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------- | ------ |
| Rust fixture lock test passes                                           | `cargo nextest run -p cronduit --lib -- webhooks::dispatcher::tests::sign_v1_locks_interop_fixture`                       | `1 passed`                                      | ✓ PASS |
| Python receiver verifies canonical fixture                              | `python3 examples/webhook-receivers/python/receiver.py --verify-fixture tests/fixtures/webhook-v1`                       | `OK: fixture verified`                          | ✓ PASS |
| Go receiver verifies canonical fixture                                  | `go run examples/webhook-receivers/go/receiver.go --verify-fixture tests/fixtures/webhook-v1`                            | `OK: fixture verified`                          | ✓ PASS |
| Node receiver verifies canonical fixture                                | `node examples/webhook-receivers/node/receiver.js --verify-fixture tests/fixtures/webhook-v1`                            | `OK: fixture verified`                          | ✓ PASS |
| `cargo tree -i openssl-sys` empty (D-24 rustls invariant)               | `cargo tree -i openssl-sys`                                                                                              | `package ID specification ... did not match`   | ✓ PASS |
| BL-01 divergence empirically reproducible for non-canonical timestamps  | Python script: HMAC over `+1735689600` (raw) vs `1735689600` (parsed) for canonical body+id+secret                       | Two different base64 HMACs (FK4j... vs uPZ2...) | ✗ FAIL |

### Requirements Coverage

| Requirement | Source Plan          | Description                                                                                                                                                          | Status                  | Evidence                                                                                                                                                                                                                                                                                          |
| ----------- | -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| WH-04       | 19-01 .. 19-06       | HMAC algorithm is SHA-256 only in v1.2; cronduit ships receiver examples in Python/Go/Node demonstrating constant-time HMAC compare (NOT `==` on hex-decoded bytes). | ⚠️ PARTIALLY SATISFIED | SHA-256-only enforced in code (`sign_v1` uses `Hmac::<Sha256>`) and documented in `docs/WEBHOOKS.md` § "SHA-256 only". All 3 receivers use spec'd constant-time primitives (`hmac.compare_digest` / `hmac.Equal` / `crypto.timingSafeEqual` with length guard). However, Python and Node receivers do NOT correctly implement the Standard Webhooks v1 signing-string contract (BL-01). The constant-time-compare clause is met; the wire-format-correctness clause is broken for non-canonical timestamps.  |

No requirement IDs from `.planning/REQUIREMENTS.md` Phase 19 mapping are unaccounted for. WH-04 is the sole requirement assigned to Phase 19 per the Traceability table (line 178).

### Anti-Patterns Found

| File                                              | Line(s)                | Pattern                                                                                          | Severity   | Impact                                                                                                                                                                                                                                                |
| ------------------------------------------------- | ---------------------- | ------------------------------------------------------------------------------------------------ | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `examples/webhook-receivers/python/receiver.py`   | 67                     | Sign-string composed from PARSED int, not raw header bytes (BL-01)                                | 🛑 Blocker | Spec violation; latent interop bug masked by canonical-decimal fixture.                                                                                                                                                                              |
| `examples/webhook-receivers/node/receiver.js`     | 82                     | Sign-string composed from PARSED int, not raw header bytes (BL-01)                                | 🛑 Blocker | Same as above.                                                                                                                                                                                                                                       |
| `examples/webhook-receivers/node/receiver.js`     | 75-76, 159-160         | `Number.parseInt` accepts non-numeric suffixes / `+`-prefix / whitespace passing `Number.isFinite` (WR-01) | ⚠️ Warning | Compounds BL-01: receiver accepts spec-illegal headers, then HMAC-rejects with confusing 401 message.                                                                                                                                                  |
| `examples/webhook-receivers/node/receiver.js`     | 208-213                | `_sanitizeFixtureArg` claims `..` rejection but `path.normalize` collapses embedded `..` first (WR-02) | ⚠️ Warning | Documentation/code mismatch; not a security bug because `_readFixtureFile` uses a hard-coded allowlist of bare names. Future maintainer extending the receiver could rely on the (false) claim.                                                       |
| `examples/webhook-receivers/python/receiver.py`   | 112, 117               | Treats chunked-transfer bodies as empty (no `Transfer-Encoding: chunked` handling) (WR-03)        | ⚠️ Warning | Cronduit's reqwest dispatcher always sets Content-Length, so no real interop bug today. A non-cronduit signer using chunked encoding would silently 401.                                                                                              |
| `examples/webhook-receivers/python/README.md`     | 22 (and Node README:24, Go README:25) | Claim "logs to stdout" but receivers actually write to stderr (WR-04)                  | ℹ️ Info    | Cosmetic tripwire for operators redirecting via `>`. Non-blocking.                                                                                                                                                                                    |
| `docs/WEBHOOKS.md` + `node/receiver.js` + `node/README.md` | 135-145, 21-26/94-99, 50 | Length-guard rationale duplicated across 3 locations (WR-05)                                | ℹ️ Info    | Intentional defense-in-depth; flagged for awareness only.                                                                                                                                                                                              |

All 7 review findings (BL-01 + WR-01..WR-06) confirmed present in the codebase. No anti-patterns from the standard scan (TODO/FIXME/placeholder/empty-impl) found in Phase 19 files.

### Human Verification Required

11 maintainer UAT scenarios in `19-HUMAN-UAT.md` deliberately ship unchecked per D-22 (project memory `feedback_uat_user_validates.md`). Verifier MUST NOT flip them. Detailed list in YAML frontmatter `human_verification:` field. Highlights:

- **U1-U2** — Workspace builds clean + Rust fixture lock test green (verifier confirmed locally).
- **U3-U5** — Per-language verify-fixture recipes pass canonical + 3 tamper variants (verifier confirmed canonical pass; tamper variants per-recipe internal — not separately spot-checked).
- **U6-U8** — Live cronduit deliveries to each receiver (3 terminals each; cannot be automated).
- **U9-U10** — GitHub-render review of `docs/WEBHOOKS.md` mermaid diagrams + README/CONFIG cross-references.
- **U11** — `webhook-interop` CI matrix passes on the PR (3 cells GREEN).

### Gaps Summary

The phase ships a substantial, well-engineered set of artifacts: a locked Standard Webhooks v1 wire-format fixture, a Rust unit test that re-derives the fixture from the canonical payload + signer, three stdlib reference receivers, a per-language CI matrix that gates on the fixture from day one, an operator-facing `docs/WEBHOOKS.md` hub with mermaid diagrams, README + CONFIG.md cross-references, 6 just recipes (3 foreground + 3 verify-fixture), and 11 maintainer UAT scenarios. All 4 runtimes (Rust, Python, Go, Node) verify the canonical fixture identically.

**One real defect (BL-01)** prevents the phase from fully achieving its stated "lock the wire format across four runtimes" goal:

- **Python (`receiver.py:67`) and Node (`receiver.js:82`) compute the HMAC over the parsed-integer form of the `webhook-timestamp` header, while cronduit's Rust signer (`dispatcher.rs:144`) and the Go receiver (`receiver.go:78`) use the raw header bytes.** The Standard Webhooks v1 specification is explicit that the signing-string `${id}.${timestamp}.${body}` uses `${timestamp}` as the raw header value.

- For the canonical fixture timestamp `1735689600`, the parsed-int round-trips byte-equal to the raw header, so the locked CI fixture cannot detect the divergence. The verifier empirically reproduced the divergence using non-canonical decimal forms (`+1735689600`, `01735689600`, leading whitespace, etc.) — the Python/Node-style HMAC is BYTE-DIFFERENT from the Go/Rust-style HMAC for these inputs.

- **Real-world impact today is bounded:** cronduit always emits a canonical decimal i64 timestamp via `chrono::Utc::now().timestamp().to_string()`, so a real cronduit→receiver delivery cannot trigger the divergence. The bug surfaces only if (a) cronduit's serialization changes, (b) a non-cronduit signer using a non-canonical decimal form interacts with these receivers, or (c) the Standard Webhooks fixture/test-vectors evolve to include non-canonical timestamps.

- **Why this is gaps_found and not human_needed:** The phase explicitly chose to "lock the wire format across all four runtimes" via the CI fixture (CONTEXT D-13/D-14, plan 19-01 must_have truths). BL-01 reveals that the lock is only canonical-shape-dependent, not contract-dependent — the locked fixture cannot detect the parse-and-reformat divergence. This is a code-only fix, small scope, no user choice required.

**Suggested fix scope:**
1. `examples/webhook-receivers/python/receiver.py:67` — change `f"{wid}.{ts}.".encode()` to `f"{wid}.{wts}.".encode()`.
2. `examples/webhook-receivers/node/receiver.js:82` — change `mac.update(`${wid}.${ts}.`)` to `mac.update(`${wid}.${wts}.`)`.
3. Add a regression vector to the verify-fixture recipes (or as a sister fixture) using a non-canonical decimal timestamp, so the spec contract is locked permanently.
4. Optional but recommended: tighten timestamp-string validators per WR-01 (require `^\d+$` pre-parse) — this also closes the tangentially-related issue that the receivers accept spec-illegal headers and then HMAC-reject with a confusing message.

**This looks intentional?** No — the review flagged BL-01 as a Blocker, and there is no documentation in any phase artifact (CONTEXT, RESEARCH, PATTERNS, plans 02/04, summaries) acknowledging the parsed-int approach as a deliberate trade-off. If the maintainer chooses to accept BL-01 as a known limitation (since canonical timestamps make it inert today), they may add an `overrides:` entry to this VERIFICATION.md frontmatter:

```yaml
overrides:
  - must_have: "Each shipped receiver implements the Standard Webhooks v1 signing-string convention: HMAC over `${webhook-id}.${webhook-timestamp}.${body}` using the RAW timestamp header bytes (not a parsed-and-re-serialized integer)."
    reason: "BL-01 deferred — cronduit always emits canonical decimal timestamps; divergence is latent. Track in BACKLOG.md for v1.3 and verify when Phase 20 lands."
    accepted_by: "<Maintainer name>"
    accepted_at: "<ISO timestamp>"
```

The 11 maintainer UAT items remain unchecked per D-22 regardless of the BL-01 disposition — the maintainer must run them post-fix (or post-override) and flip the boxes themselves.

---

_Verified: 2026-04-30T19:35:00Z_
_Verifier: Claude (gsd-verifier)_
