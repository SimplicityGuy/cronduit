---
phase: 19-webhook-hmac-signing-receiver-examples
verified: 2026-04-30T20:15:00Z
status: human_needed
score: 3/3 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 2/3
  gaps_closed:
    - "Each shipped receiver implements the Standard Webhooks v1 signing-string convention: HMAC over `${webhook-id}.${webhook-timestamp}.${body}` using the RAW timestamp header bytes (not a parsed-and-re-serialized integer)."
  gaps_remaining: []
  regressions: []
human_verification:
  - test: "U1 — Workspace builds clean (`just ci`)"
    expected: "Recipe exits 0; openssl-check empty across native + arm64-musl + amd64-musl"
    why_human: "Maintainer UAT per D-22 — Claude does not flip checkboxes (project memory `feedback_uat_user_validates.md`)."
  - test: "U2 — Rust fixture lock test green (`just nextest`)"
    expected: "`webhooks::dispatcher::tests::sign_v1_locks_interop_fixture` PASSED in nextest output."
    why_human: "Maintainer UAT per D-22; verifier confirmed `just test-unit` (257/257 unit tests passed) but maintainer must run the full nextest suite."
  - test: "U3 — Python verify-fixture recipe (now 5 variants)"
    expected: "`OK: all 5 fixture variants behave correctly`. Verifier confirmed locally."
    why_human: "Maintainer UAT per D-22; covers canonical pass + mutated-secret + mutated-body + drifted-timestamp + new BL-01 leading-zero regression."
  - test: "U4 — Go verify-fixture recipe (now 5 variants)"
    expected: "`OK: all 5 fixture variants behave correctly`. Verifier confirmed locally."
    why_human: "Maintainer UAT per D-22."
  - test: "U5 — Node verify-fixture recipe (now 5 variants)"
    expected: "`OK: all 5 fixture variants behave correctly`; no `RangeError` traceback. Verifier confirmed locally."
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
    expected: "All 3 cells (python/go/node) GREEN; final-step log contains `OK: all 5 fixture variants behave correctly`."
    why_human: "Read GHA Checks tab on the PR; cannot be verified locally without push."
---

# Phase 19: Webhook HMAC Signing + Receiver Examples Verification Report (Re-verification)

**Phase Goal:** Operators can verify webhook authenticity using HMAC-SHA256 and the Standard Webhooks signing-string convention; ship reference receiver examples that demonstrate constant-time HMAC compare.
**Verified:** 2026-04-30T20:15:00Z
**Status:** human_needed
**Re-verification:** Yes — after BL-01 + WR-01..WR-05 auto-fix landed (commits `2e7a8f8`, `f5823a8`, `a0a72fd`, `f421815`, `fc4917d`, `6fffa95`, `53d8adc`).

## Re-verification Summary

| Item | Previous | Current |
| ---- | -------- | ------- |
| Status | `gaps_found` | `human_needed` |
| Score | 2/3 | **3/3** |
| Open gaps | 1 (BL-01) | **0** |
| Regressions introduced | — | **0** |
| Human UAT items | 11 (unchecked per D-22) | 11 (unchecked per D-22) |

**The single previous-iteration gap (BL-01 — Python and Node signing over parsed-int timestamp instead of raw header bytes) is closed.** Both receivers now sign over `wts` (raw header bytes), matching the Rust dispatcher and Go receiver. A new regression variant (commit `f5823a8`) using `webhook-timestamp = "01735689600"` (same int, different bytes) is permanently locked into all three `just uat-webhook-receiver-{lang}-verify-fixture` recipes; if any future regression re-introduces parsed-int signing, the recipes will fail with a clear "BL-01 regression" error. The five WR-01..WR-05 warnings the fixer also addressed are noted in §Anti-Patterns Found.

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| #   | Truth                                                                                                                                                                                                                                                                                                                                                                                                                                  | Status     | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SC1 | Operator who configures `webhook.secret = "..."` sees `webhook-signature: v1,<base64-of-hmac>` where HMAC is over `webhook-id.webhook-timestamp.payload` raw bytes using SHA-256.                                                                                                                                                                                                                                                       | ✓ VERIFIED | `src/webhooks/dispatcher.rs:138-156` (`sign_v1` HMAC-SHA256 over `${id}.${ts}.${body}`); `:253` (`format!("v1,{sig}")`); Rust unit test `sign_v1_locks_interop_fixture` PASSED locally as part of `just test-unit` (257/257 passed); Phase 18's `sign_v1_known_fixture`, `signature_uses_standard_base64_alphabet`, `signature_value_is_v1_comma_b64` tests still in tree.                                                                                                                                                                |
| SC2 | Operator running shipped Python, Go, Node receiver examples successfully verifies signatures from a real cronduit delivery; each uses constant-time compare primitive (Python `hmac.compare_digest`, Go `hmac.Equal`, Node `crypto.timingSafeEqual`).                                                                                                                                                                                  | ✓ VERIFIED | All 3 receivers verify the canonical fixture (`OK: fixture verified` × 3 — confirmed locally). Each uses the spec'd constant-time primitive (Python: `receiver.py:88` `hmac.compare_digest`; Go: `receiver.go:91` `hmac.Equal`; Node: `receiver.js:114` `crypto.timingSafeEqual` after a length-equality guard at `:112`). **BL-01 closed:** Python (`receiver.py:76`) and Node (`receiver.js:95`) now sign over `wts` (raw header bytes), aligned with Go (`receiver.go:78`) and Rust (`dispatcher.rs:144`). Empirically reproduced (see Behavioral Spot-Checks #6/#7 below): a non-canonical leading-zero timestamp `01735689600` signed via raw bytes verifies in all three receivers (Python/Go/Node `OK`). Maintainer UAT (U6/U8) for the live cronduit→receiver flow remains in `19-HUMAN-UAT.md`. |
| SC3 | Operator reviewing receiver-example docs sees explicit note that v1.2 ships SHA-256 only (no algorithm-agility / multi-secret rotation cronduit-side; rotation is a receiver concern).                                                                                                                                                                                                                                                  | ✓ VERIFIED | `docs/WEBHOOKS.md` § "SHA-256 only" + § "Secret rotation" (receiver-side dual-secret guidance); per-receiver `README.md` files all carry the SHA-256-only note in their headers; `docs/CONFIG.md:594` back-link to `WEBHOOKS.md`; `README.md:176` pointer.                                                                                                                                                                                                                                                                                |

**Score:** **3/3 truths verified**

### Required Artifacts

| Artifact                                                                                | Expected                                                                                            | Status     | Details                                                                                                                                                                  |
| --------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tests/fixtures/webhook-v1/secret.txt`                                                  | 37 bytes, no trailing newline                                                                       | ✓ VERIFIED | Unchanged since initial verification.                                                                                                                                    |
| `tests/fixtures/webhook-v1/webhook-id.txt`                                              | 26 bytes, no trailing newline                                                                       | ✓ VERIFIED | Unchanged.                                                                                                                                                                |
| `tests/fixtures/webhook-v1/webhook-timestamp.txt`                                       | 10 bytes, `1735689600`                                                                              | ✓ VERIFIED | Unchanged (canonical fixture intentionally untouched per Wave 1 contract; regression variant lives in temp dirs inside the recipes).                                    |
| `tests/fixtures/webhook-v1/payload.json`                                                | 349 bytes; canonical JSON                                                                            | ✓ VERIFIED | Unchanged.                                                                                                                                                                |
| `tests/fixtures/webhook-v1/expected-signature.txt`                                      | `v1,<base64>` 47 bytes                                                                              | ✓ VERIFIED | Unchanged.                                                                                                                                                                |
| `tests/fixtures/webhook-v1/.gitattributes` + `README.md`                                | EOL discipline + provenance                                                                          | ✓ VERIFIED | Unchanged.                                                                                                                                                                |
| `src/webhooks/dispatcher.rs::tests::sign_v1_locks_interop_fixture`                       | Rust in-module fixture lock test                                                                     | ✓ VERIFIED | Still passing (in-tree; `just test-unit` reports 257/257 unit tests green post-fix).                                                                                     |
| `src/webhooks/dispatcher.rs::tests::print_canonical_payload_bytes`                       | `#[ignore]` regen helper                                                                             | ✓ VERIFIED | Still present.                                                                                                                                                            |
| `examples/webhook-receivers/python/receiver.py`                                         | Stdlib HTTP server on :9991, `hmac.compare_digest`, `--verify-fixture` mode                          | ✓ VERIFIED | **BL-01 closed:** line 76 now reads `signing_str = f"{wid}.{wts}.".encode() + body_bytes`. WR-01 closed: strict unsigned-decimal validation at lines 65-66 + 170-172. WR-03 closed: chunked-transfer rejected at lines 129-147. |
| `examples/webhook-receivers/python/README.md`                                           | Run instructions + SHA-256-only note                                                                 | ✓ VERIFIED | WR-04 closed: now correctly says "logs to stderr" (commit `6fffa95`).                                                                                                    |
| `examples/webhook-receivers/go/receiver.go`                                             | Stdlib HTTP server on :9992, `hmac.Equal`, `--verify-fixture` mode                                   | ✓ VERIFIED | Already correct since initial verification (signed over raw bytes); unchanged.                                                                                            |
| `examples/webhook-receivers/go/README.md`                                               | Run instructions                                                                                    | ✓ VERIFIED | WR-04 closed: now says stderr.                                                                                                                                            |
| `examples/webhook-receivers/node/receiver.js`                                           | Stdlib HTTP server on :9993, `crypto.timingSafeEqual` + length-guard, `--verify-fixture` mode        | ✓ VERIFIED | **BL-01 closed:** line 95 now reads `mac.update(`${wid}.${wts}.`)`. Length-guard intact (`:112`). WR-01 closed: regex `/^\d+$/` validation at lines 80, 176. WR-02 closed: `..` checked on raw arg at lines 228-236 (commit `f421815`). WR-06 closed by WR-01 fix. |
| `examples/webhook-receivers/node/README.md`                                             | Run instructions                                                                                    | ✓ VERIFIED | WR-04 closed.                                                                                                                                                             |
| `docs/WEBHOOKS.md`                                                                      | Operator hub with mermaid diagrams + SHA-256-only callout + retry contract                           | ✓ VERIFIED | WR-05 closed: length-guard rationale slimmed to a "see receiver.js for why" link block (commit `53d8adc`); receiver source comment remains the source of truth.         |
| `docs/CONFIG.md` back-link, `README.md` pointer                                         | Cross-references                                                                                    | ✓ VERIFIED | Unchanged.                                                                                                                                                                |
| `examples/cronduit.toml::wh-example-receiver-{python,go,node}`                          | 3 commented-out job blocks (per D-05)                                                                | ✓ VERIFIED | Unchanged.                                                                                                                                                                |
| `justfile::uat-webhook-receiver-{python,go,node}` recipes                                | Foreground receiver runners                                                                          | ✓ VERIFIED | Unchanged.                                                                                                                                                                |
| `justfile::uat-webhook-receiver-{python,go,node}-verify-fixture` recipes                 | **5 variants now**: canonical + 3 tamper + BL-01 regression                                           | ✓ VERIFIED | Commit `f5823a8` added the 5th variant ("Wire-format strictness — non-canonical-decimal timestamp must STILL verify") to all 3 recipes (justfile lines 417-435, 493-511, 569-587). Verifier ran each recipe end-to-end; each prints `OK: all 5 fixture variants behave correctly`. |
| `.github/workflows/ci.yml::webhook-interop` matrix job                                   | Python/Go/Node matrix gating cross-language drift                                                    | ✓ VERIFIED | Unchanged. Will exercise the new 5th variant on the next CI run because it calls into the recipe by name.                                                                |
| `.planning/phases/19-webhook-hmac-signing-receiver-examples/19-HUMAN-UAT.md`             | 11 unchecked maintainer scenarios per D-22                                                          | ✓ VERIFIED | All 11 still ship `[ ] Maintainer-validated`; sign-off line blank. Verifier confirmed no checkboxes were flipped (per D-22 / `feedback_uat_user_validates.md`).            |

### Key Link Verification

| From                                               | To                                                                  | Via                                                                                  | Status      | Details                                                                                                                                                                                                |
| -------------------------------------------------- | ------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `sign_v1_locks_interop_fixture`                    | `tests/fixtures/webhook-v1/{*.txt,payload.json}`                    | `include_bytes!`/`include_str!` at compile time                                      | ✓ WIRED     | Rust test still PASSED in `just test-unit`.                                                                                                                                                          |
| `sign_v1_locks_interop_fixture`                    | `sign_v1`                                                            | Direct in-module call                                                                | ✓ WIRED     | Test re-derives signature; assertion holds.                                                                                                                                                          |
| Python/Go/Node receivers                           | fixture files                                                        | `--verify-fixture <dir>` mode                                                        | ✓ WIRED     | All three print `OK: fixture verified` against canonical fixture (verifier confirmed locally).                                                                                                       |
| Cronduit `HttpDispatcher`                          | Outbound HTTP request                                                | `sign_v1` → `format!("v1,{sig}")` → `webhook-signature` header                       | ✓ WIRED     | `dispatcher.rs:252-253` unchanged.                                                                                                                                                                   |
| `webhook-interop` CI matrix                        | per-language verify-fixture recipes                                  | `just uat-webhook-receiver-${{ matrix.lang }}-verify-fixture`                        | ✓ WIRED     | Matrix shape unchanged; will execute the new 5-variant recipes on next CI run.                                                                                                                       |
| `docs/WEBHOOKS.md`                                 | per-receiver READMEs                                                 | Markdown links                                                                       | ✓ WIRED     | Section 9 links unchanged.                                                                                                                                                                          |
| **Python/Node receiver HMAC computation**          | **Standard Webhooks v1 spec wire format (raw timestamp bytes)**     | **`f"{wid}.{wts}."` / `` `${wid}.${wts}.` ``**                                       | **✓ WIRED** | **Closed:** Both receivers now use raw header bytes — spec-correct (commit `2e7a8f8`); locked by the BL-01 regression variant in all 3 verify-fixture recipes (commit `f5823a8`).                    |
| Go receiver HMAC computation                       | Standard Webhooks v1 spec wire format (raw timestamp bytes)         | `wid + "." + wts + "."`                                                              | ✓ WIRED     | Unchanged — already correct.                                                                                                                                                                          |

### Data-Flow Trace (Level 4)

| Artifact                                                  | Data Variable                          | Source                                                                       | Produces Real Data                                                                                                                                                                                              | Status     |
| --------------------------------------------------------- | -------------------------------------- | ---------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- |
| Python `verify_signature` HMAC computation                | raw `wts` string                       | `headers.get('webhook-timestamp')` → `f"{wid}.{wts}.".encode()` (line 76)     | Yes — uses raw header bytes per Standard Webhooks v1 spec. Strict unsigned-decimal validator at line 65 rejects spec-illegal forms before HMAC.                                                                | ✓ FLOWING  |
| Node `_verifyWithDrift` HMAC computation                  | raw `wts` string                       | `headers['webhook-timestamp']` → `mac.update(`${wid}.${wts}.`)` (line 95)     | Yes — uses raw header bytes per Standard Webhooks v1 spec. Strict unsigned-decimal validator at line 80 rejects spec-illegal forms before HMAC.                                                                | ✓ FLOWING  |
| Go `verifyWithDrift` HMAC computation                     | raw `wts` string                       | `r.Header.Get("webhook-timestamp")` → `mac.Write([]byte(wid + "." + wts + "."))` | Yes — unchanged from initial verification.                                                                                                                                                                       | ✓ FLOWING  |
| Cronduit `HttpDispatcher::deliver`                        | `webhook_ts` i64                       | `chrono::Utc::now().timestamp()` → `webhook_ts.to_string()` for header + `format!("{}.{}.", id, webhook_ts)` for sign | Yes — header value is the canonical decimal of the i64 (via `to_string()`); `sign_v1` formats the same i64 via `{webhook_timestamp}` (Display). Cronduit emits canonical decimal; receivers now sign over raw bytes — contract is byte-aligned. | ✓ FLOWING  |
| BL-01 regression variant (justfile)                       | `NEW_WTS = "01735689600"`              | `printf '%s' "01735689600" > $TMP/webhook-timestamp.txt` + `openssl dgst -sha256 -hmac $SECRET` over `${id}.${NEW_WTS}.${body}` | Yes — all 3 receivers verify (`OK: fixture verified`); confirms raw-bytes contract holds across runtimes for non-canonical decimal forms.                                                                       | ✓ FLOWING  |

### Behavioral Spot-Checks

| #   | Behavior                                                                | Command                                                                                                                  | Result                                                | Status |
| --- | ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------- | ------ |
| 1   | Rust unit suite green (regression check)                                | `just test-unit`                                                                                                          | `test result: ok. 257 passed; 0 failed; 1 ignored`   | ✓ PASS |
| 2   | Python verify-fixture recipe (5 variants)                               | `just uat-webhook-receiver-python-verify-fixture`                                                                         | `OK: all 5 fixture variants behave correctly`        | ✓ PASS |
| 3   | Go verify-fixture recipe (5 variants)                                   | `just uat-webhook-receiver-go-verify-fixture`                                                                             | `OK: all 5 fixture variants behave correctly`        | ✓ PASS |
| 4   | Node verify-fixture recipe (5 variants)                                 | `just uat-webhook-receiver-node-verify-fixture`                                                                           | `OK: all 5 fixture variants behave correctly`        | ✓ PASS |
| 5   | Direct empirical proof of BL-01 raw vs parsed divergence at 01735689600 | Python script computing HMAC over raw `01735689600` vs parsed-int `1735689600`                                           | Two BYTE-DIFFERENT signatures (DLyaMt... vs Gqa7PW...) | ✓ PASS (divergence is real; receivers correctly use raw form) |
| 6   | All 3 receivers accept the BL-01 regression fixture (raw-bytes contract)| Hand-built `/tmp/bl01-noncan` with `webhook-timestamp = "01735689600"` and re-signed `expected-signature.txt`            | Python `OK`, Node `OK`, Go `OK`                       | ✓ PASS |
| 7   | Strict-validator behavior on `+1735689600` (parser strictness check)    | Hand-built `/tmp/bl01-plus` with `webhook-timestamp = "+1735689600"` and raw-bytes-signed `expected-signature.txt`        | Python `FAIL` (strict reject), Node `FAIL` (strict reject), Go `OK`. Per the verifier brief — Either outcome is acceptable: contract converges in Python/Node by REJECTING the spec-illegal header BEFORE HMAC. | ✓ PASS (intentional strictness divergence; not a contract divergence — Python/Node never compute a divergent HMAC for `+`-prefixed wts because they reject the header up-front) |

Spot-check #7 is worth noting: Python and Node now reject `+1735689600` via strict unsigned-decimal validation at the wts parser (`wts.isascii() and wts.isdigit()` / `/^\d+$/`) BEFORE any HMAC computation. Go accepts the `+` form (`strconv.ParseInt` allows leading `+`). This is a parser-strictness divergence between Go and Python/Node, NOT a wire-format divergence — the verifier brief explicitly accepts this outcome ("Either outcome is fixed — the contract just needs to converge"). For ALL valid (unsigned-decimal) timestamps the runtimes agree byte-for-byte; for the spec-illegal `+`-prefix Python/Node reject early, Go accepts. No HMAC ever diverges silently.

### Requirements Coverage

| Requirement | Source Plan          | Description                                                                                                                                                          | Status        | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ----------- | -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| WH-04       | 19-01 .. 19-06       | HMAC algorithm is SHA-256 only in v1.2; cronduit ships receiver examples in Python/Go/Node demonstrating constant-time HMAC compare (NOT `==` on hex-decoded bytes). | ✓ SATISFIED  | SHA-256-only enforced in code (`sign_v1` uses `Hmac::<Sha256>`) and documented in `docs/WEBHOOKS.md` § "SHA-256 only". All 3 receivers use spec'd constant-time primitives (`hmac.compare_digest` / `hmac.Equal` / `crypto.timingSafeEqual` with length guard). **BL-01 closed:** Python and Node now sign over raw header bytes per Standard Webhooks v1 spec; locked by the 5th `wire-format strictness` variant in all 3 verify-fixture recipes. WH-04 status in `.planning/REQUIREMENTS.md` table at line 178 still reads `Pending` — the maintainer flips that to `Validated` after running the 11 D-22 UAT scenarios. |

No requirement IDs from `.planning/REQUIREMENTS.md` Phase 19 mapping are unaccounted for. WH-04 remains the sole requirement assigned to Phase 19 (REQUIREMENTS.md line 178). No orphaned requirements.

### Anti-Patterns Found

| File                                              | Line(s)        | Pattern                                                                                          | Severity                | Impact                                                                                                                                                                                            |
| ------------------------------------------------- | -------------- | ------------------------------------------------------------------------------------------------ | ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `examples/webhook-receivers/python/receiver.py`   | 76 (was 67)    | ~~Sign-string composed from PARSED int (BL-01)~~                                                  | ✓ FIXED                  | Commit `2e7a8f8` — now uses raw `wts` header bytes; spec-correct.                                                                                                                                |
| `examples/webhook-receivers/node/receiver.js`     | 95 (was 82)    | ~~Sign-string composed from PARSED int (BL-01)~~                                                  | ✓ FIXED                  | Commit `2e7a8f8` — now uses raw `wts` header bytes; spec-correct.                                                                                                                                |
| `examples/webhook-receivers/node/receiver.js`     | 80, 176 (WR-01)| ~~`Number.parseInt` accepts non-numeric suffixes / `+`-prefix / whitespace~~                      | ✓ FIXED                  | Commit `a0a72fd` — strict `/^\d+$/` validator + `Number.MAX_SAFE_INTEGER` rejection at both call sites. WR-06 closed by same fix.                                                              |
| `examples/webhook-receivers/python/receiver.py`   | 65, 170 (WR-01)| ~~Python `int()` accepts whitespace / leading `+`~~                                              | ✓ FIXED                  | Commit `a0a72fd` — `wts.isascii() and wts.isdigit()` validator at both call sites.                                                                                                              |
| `examples/webhook-receivers/node/receiver.js`     | 228-236 (WR-02)| ~~`_sanitizeFixtureArg` claims `..` rejection but `path.normalize` collapses embedded `..` first~~| ✓ FIXED                  | Commit `f421815` — `..` check now happens BEFORE `path.normalize`, splitter switched to `/[\\/]/` for cross-platform correctness. Manual 7-pattern test in fix report confirms behavior.       |
| `examples/webhook-receivers/python/receiver.py`   | 117-148 (WR-03)| ~~Treats chunked-transfer bodies as empty~~                                                       | ✓ FIXED                  | Commit `fc4917d` — explicit chunked rejection (400), missing-Content-Length (411), malformed (400), out-of-range (400).                                                                          |
| `examples/webhook-receivers/{python,node,go}/README.md` | (multiple)| ~~Claim "logs to stdout" but receivers actually write to stderr~~                                  | ✓ FIXED                  | Commit `6fffa95` — all 3 READMEs corrected to "logs to stderr"; brief explanation of why stderr is the right choice added.                                                                       |
| `docs/WEBHOOKS.md`                                | (length-guard) | ~~Length-guard rationale duplicated across 3 locations~~                                          | ✓ FIXED                  | Commit `53d8adc` — hub doc slimmed to 7-line link block pointing at receiver.js (single source of truth).                                                                                        |
| Standard scan                                     | —              | TODO/FIXME/placeholder/empty-impl                                                                  | None                    | No anti-patterns from the standard scan in any Phase 19 file.                                                                                                                                    |

All 7 review findings (BL-01 + WR-01..WR-06) confirmed FIXED in the codebase. WR-06 was subsumed by the WR-01 strict-validator fix per the review's own analysis (same root cause).

### Human Verification Required

11 maintainer UAT scenarios in `19-HUMAN-UAT.md` deliberately ship unchecked per D-22 (project memory `feedback_uat_user_validates.md`). **Verifier confirmed no checkboxes were flipped during this re-verification.**

Highlights from `19-HUMAN-UAT.md`:

- **U1-U2** — Workspace builds clean (`just ci`) + Rust fixture lock test green (`just nextest`). Verifier confirmed `just test-unit` (257/257 unit tests passed); maintainer must run the full nextest pipeline.
- **U3-U5** — Per-language verify-fixture recipes (now **5 variants** including the BL-01 regression). Verifier confirmed all three locally.
- **U6-U8** — Live cronduit deliveries to each receiver (3 terminals each; cannot be automated).
- **U9-U10** — GitHub-render review of `docs/WEBHOOKS.md` mermaid diagrams + README/CONFIG cross-references.
- **U11** — `webhook-interop` CI matrix passes on the PR (3 cells GREEN).

**Note on U3/U4/U5 wording:** the recipes now print `OK: all 5 fixture variants behave correctly` (was `4 tamper variants`). The current `19-HUMAN-UAT.md` text still reads "4 tamper variants" in the U3/U4/U5 pass-criteria lines; this is a doc-text drift the maintainer may want to update post-merge, but it does NOT block UAT — the maintainer running the recipe will see the new "5 fixture variants" line and recognize the BL-01 regression vector landed (and the fix report calls this out explicitly).

### Gaps Summary

**The phase goal is fully achieved at the code level. All 3 ROADMAP success criteria are verified, all 7 review findings (BL-01 + WR-01..WR-06) are fixed, the BL-01 regression vector is permanently locked into the per-language verify-fixture recipes, the Rust unit suite remains green (257/257), and zero regressions were introduced by the auto-fix iteration.**

What remains is the maintainer-validated UAT layer per D-22:

1. **U1-U2** — `just ci` + `just nextest` end-to-end pass (locally confirmed via `just test-unit`).
2. **U3-U5** — 3 verify-fixture recipes printing `OK: all 5 fixture variants behave correctly` (locally confirmed by verifier).
3. **U6-U8** — 3 live cronduit→receiver flows (cannot be automated).
4. **U9-U10** — GitHub-render review of mermaid diagrams + cross-reference links.
5. **U11** — `webhook-interop` CI matrix GREEN on the PR.

Per project memory `feedback_uat_user_validates.md` and D-22, Claude does NOT flip these checkboxes. The 11 unchecked items are the spec, not a gap. Status `human_needed` is the correct terminal state for this phase pre-UAT-completion.

After all 11 UAT items are flipped by the maintainer, `.planning/REQUIREMENTS.md:178` (`WH-04 | 19 | Pending`) gets flipped to `Validated`, `.planning/STATE.md` records the close-out, and the ROADMAP "completed" check at line 48 is reaffirmed (already checked in the Wave 3 close-out at commit `c9d41b2`).

---

_Verified: 2026-04-30T20:15:00Z_
_Verifier: Claude (gsd-verifier)_
_Iteration: 2 (re-verification after auto-fix)_
