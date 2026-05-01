# Phase 19: Webhook HMAC Signing + Receiver Examples — Pattern Map

**Mapped:** 2026-04-29
**Files analyzed:** 19 (NEW + MODIFY combined)
**Analogs found:** 17 / 19 (2 with role-match only — see "No Analog Found")

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `tests/fixtures/webhook-v1/secret.txt` | fixture (data) | file-I/O | `tests/fixtures/valid-minimal.toml` | role-match (different format) |
| `tests/fixtures/webhook-v1/webhook-id.txt` | fixture (data) | file-I/O | (sibling of above) | role-match |
| `tests/fixtures/webhook-v1/webhook-timestamp.txt` | fixture (data) | file-I/O | (sibling of above) | role-match |
| `tests/fixtures/webhook-v1/payload.json` | fixture (data) | file-I/O | (sibling of above) | role-match |
| `tests/fixtures/webhook-v1/expected-signature.txt` | fixture (data) | file-I/O | (sibling of above) | role-match |
| `tests/fixtures/webhook-v1/README.md` | docs (fixture-local) | file-I/O | `examples/cronduit.toml` header comments | partial-match |
| `src/webhooks/dispatcher.rs` (MODIFY) | Rust unit test (in-module) | request-response (HMAC) | `src/webhooks/dispatcher.rs::tests::sign_v1_known_fixture` (`:309-327`) | exact (extends same test family) |
| `examples/webhook-receivers/python/receiver.py` | example mini-server | request-response (HMAC verify) | `examples/webhook_mock_server.rs` | role-match (cross-language) |
| `examples/webhook-receivers/python/README.md` | docs (per-receiver) | file-I/O | `docs/QUICKSTART.md` (sectional intro pattern) | partial-match |
| `examples/webhook-receivers/go/receiver.go` | example mini-server | request-response (HMAC verify) | `examples/webhook_mock_server.rs` | role-match (cross-language) |
| `examples/webhook-receivers/go/README.md` | docs (per-receiver) | file-I/O | `docs/QUICKSTART.md` | partial-match |
| `examples/webhook-receivers/node/receiver.js` | example mini-server | request-response (HMAC verify) | `examples/webhook_mock_server.rs` | role-match (cross-language) |
| `examples/webhook-receivers/node/README.md` | docs (per-receiver) | file-I/O | `docs/QUICKSTART.md` | partial-match |
| `docs/WEBHOOKS.md` | operator hub doc | file-I/O | `docs/CONFIG.md` | exact (sibling hub-doc) |
| `docs/CONFIG.md` (MODIFY) | operator reference | file-I/O | self (back-link addition) | exact |
| `README.md` (MODIFY) | top-level pointer | file-I/O | self (one-line pointer) | exact |
| `examples/cronduit.toml` (MODIFY) | example config | file-I/O | existing `wh-example-signed` block (lines 240-245) | exact |
| `justfile` (MODIFY) | build/UAT recipes | request-response (CLI) | existing `uat-webhook-mock` / `uat-webhook-fire` / `uat-webhook-verify` (`:326`, `:337`, `:349`) | exact |
| `.github/workflows/ci.yml` (MODIFY) | CI matrix job | event-driven (CI) | existing `compose-smoke` matrix job (`:137-378`) | role-match (different toolchain) |
| `.planning/phases/19-webhook-hmac-signing-receiver-examples/19-HUMAN-UAT.md` | UAT artifact | file-I/O | `.planning/phases/18-webhook-payload-state-filter-coalescing/18-HUMAN-UAT.md` AND `.planning/phases/17-custom-docker-labels-seed-001/17-HUMAN-UAT.md` | exact |

## Pattern Assignments

### `tests/fixtures/webhook-v1/secret.txt` (and 4 sibling fixture data files) — fixture, file-I/O

**Analog:** `tests/fixtures/valid-minimal.toml` (role-match — data-only fixture file). For the **format** of plain plaintext-with-comment-header, the closest existing analog is the comment-block header pattern at the top of `examples/cronduit.toml`.

**No directly matching plaintext-fixture pattern exists in the codebase**, so the fixture format follows the RESEARCH.md guidance verbatim:

- Plaintext only — receivers read with stdlib file I/O (`open(...,'rb').read()` / `os.ReadFile` / `fs.readFileSync`)
- **NO trailing newline** in any data file (Pitfall 3) — write with `printf '%s' '<value>' > file.txt`
- One-line `#`-prefixed comment header in data files where format permits (NOT in `payload.json` — JSON has no comment syntax; comment header lives only in the `secret.txt` per RESEARCH § Pattern 5)
- Add `tests/fixtures/webhook-v1/.gitattributes` entry `* -text` to disable EOL normalization (Pitfall 3 mitigation)

**File-by-file content shape** (per RESEARCH.md §Standard Stack and §Recommended Project Structure):

```
secret.txt        → exactly the string `cronduit-test-fixture-secret-not-real` (no newline)
webhook-id.txt    → 26-char Crockford-base32 ULID, e.g. `01HXYZTESTFIXTURE0000000000` (no newline)
webhook-timestamp.txt → stable past Unix-seconds integer, e.g. `1700000000` (no newline)
payload.json      → output of `serde_json::to_vec(&WebhookPayload::build(canonical event))` byte-for-byte (no newline)
expected-signature.txt → `v1,<base64>` from `sign_v1` over the above 4 inputs (no newline)
```

**Key variation from the analog:** Existing `tests/fixtures/*.toml` are all TOML config fixtures consumed by Rust parser tests. The webhook-v1 fixture is consumed by **four different runtimes** (Rust, Python, Go, Node) — so the format is the most-portable shape (raw bytes / one-value-per-file / no parsing required).

---

### `tests/fixtures/webhook-v1/README.md` — fixture-local docs, file-I/O

**Analog:** Comment-header pattern at top of `examples/cronduit.toml` lines 200-211 (existing fixture-context warning). No exact analog README-for-fixture exists.

**Content shape** (per RESEARCH.md §Wave 0 Gaps):
1. **Origin** — how the fixture was generated (one-line: "generated by `cargo run --example generate_webhook_fixture` then committed verbatim").
2. **Warning** — `secret.txt` is **a test value only**; never reuse in production. Mirrors the warning in `examples/cronduit.toml:200-211`.
3. **Regen instructions** — point at `src/webhooks/dispatcher.rs::tests::sign_v1_locks_interop_fixture` and the (one-time) generator binary.
4. **No-trailing-newline rule** — call out Pitfall 3 verbatim: "All files in this directory have NO trailing newline. Edit with care."
5. **`.gitattributes` reference** — explain the `* -text` rule.

---

### `src/webhooks/dispatcher.rs` (MODIFY — extend `#[cfg(test)] mod tests`) — Rust unit test, request-response (HMAC)

**Analog:** `src/webhooks/dispatcher.rs::tests::sign_v1_known_fixture` at `:309-327` (exact match — same module, same function family).

**Existing test family pattern** (lines 309-369, 3 tests):
```rust
#[test]
fn sign_v1_known_fixture() {
    let secret = SecretString::from("shh");
    let id = "01HZAFY0V1F1BS1F2H8GV4XG3R";
    let ts: i64 = 1_761_744_191;
    let body = br#"{"hello":"world"}"#;
    let sig = sign_v1(&secret, id, ts, body);

    // Compute expected by an independent HMAC over the SAME prefix+body.
    let prefix = format!("{id}.{ts}.");
    let mut mac = Hmac::<Sha256>::new_from_slice(b"shh").unwrap();
    mac.update(prefix.as_bytes());
    mac.update(body);
    let expected =
        base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());

    assert_eq!(sig, expected);
    assert!(!sig.is_empty());
}
```

**New test (`sign_v1_locks_interop_fixture`) — extension of this same family**, per RESEARCH.md §Code Examples Example 4:

```rust
#[test]
fn sign_v1_locks_interop_fixture() {
    // Fixture lives at tests/fixtures/webhook-v1/. include_bytes! embeds at
    // compile time so the test reads zero files at run time.
    // NB: `sign_v1` is `pub(crate)` — this test MUST live in this module.
    // An integration test under tests/ cannot see the function (Pitfall 1).
    let secret_raw = include_bytes!("../../tests/fixtures/webhook-v1/secret.txt");
    let webhook_id = include_str!("../../tests/fixtures/webhook-v1/webhook-id.txt");
    let webhook_ts_str = include_str!("../../tests/fixtures/webhook-v1/webhook-timestamp.txt");
    let payload = include_bytes!("../../tests/fixtures/webhook-v1/payload.json");
    let expected_full = include_str!("../../tests/fixtures/webhook-v1/expected-signature.txt");

    // Construct secret EXACTLY as production path does (SecretString wrap).
    // Fixture files have no trailing newline (Pitfall 3).
    let secret_str = std::str::from_utf8(secret_raw).expect("secret.txt is UTF-8");
    let secret = SecretString::from(secret_str);
    let webhook_ts: i64 = webhook_ts_str
        .trim_end()  // defensive: tolerate one trailing \n if a future editor adds it
        .parse()
        .expect("webhook-timestamp.txt is i64");

    let actual = sign_v1(&secret, webhook_id.trim_end(), webhook_ts, payload);
    let actual_full = format!("v1,{actual}");
    let expected_full = expected_full.trim_end();

    assert_eq!(
        actual_full, expected_full,
        "fixture interop signature drift — \
         either sign_v1 changed (regenerate fixture intentionally) \
         or fixture file was corrupted (don't regenerate; investigate)"
    );
}
```

**Key variation:** Uses `include_bytes!` / `include_str!` (compile-time embed) instead of inline literal bytes (the existing test's `"shh"` / `b"hello,world"`). No new imports needed — every symbol (`SecretString`, `sign_v1`, `Hmac`, `Sha256`, `base64`) is already in scope from the existing tests.

**Pitfall enforcement (RESEARCH.md Pitfall 1):** This test MUST live in-module; do NOT create `tests/webhook_signature_fixture.rs` because `sign_v1` is `pub(crate)`.

---

### `examples/webhook-receivers/python/receiver.py` — example mini-server, request-response (HMAC verify)

**Analog:** `examples/webhook_mock_server.rs` (cross-language; same form factor: stdlib loopback HTTP receiver, `Connection: close`, dual-log, body-cap).

**Header-comment pattern** (lines 1-15 of `webhook_mock_server.rs`):
```rust
//! Phase 18 — webhook UAT mock receiver.
//!
//! Run via `just uat-webhook-mock`. Listens on 127.0.0.1:9999, logs every
//! request (method, path, all headers, body) to stdout AND to
//! /tmp/cronduit-webhook-mock.log. Returns 200 OK on every request.
//!
//! USE ONLY for local maintainer UAT validation per project memory
//! `feedback_uat_user_validates.md`. Never expose to the public internet.
//!
//! NOTE: This is a simple loopback receiver for manual UAT inspection only —
//! NOT a production-grade HTTP/1.1 implementation. The `Connection: close`
//! response header forces request-per-connection semantics so reqwest doesn't
//! reuse a stale TCP stream between deliveries.
```

**Constants block** (mirrors `webhook_mock_server.rs:21-22`):
```rust
const ADDR: &str = "127.0.0.1:9999";
const LOG_PATH: &str = "/tmp/cronduit-webhook-mock.log";
```

**Body-cap pattern** (lines 67-71):
```rust
// Safety cap: don't grow forever on a misbehaving client.
if buf.len() > 1_048_576 {
    // 1 MiB
    eprintln!("[webhook-mock] body too large from {peer}; dropping");
    return;
}
```

**Dual-log pattern** (lines 80-87):
```rust
eprintln!("{log_line}");
if let Ok(mut f) = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(LOG_PATH)
{
    let _ = f.write_all(log_line.as_bytes());
}
```

**`Connection: close` response framing** (line 91):
```rust
let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
```

**Verify-function shape (the new copy-pasteable core, ~15 LOC)** — per RESEARCH.md §Code Examples Example 1:

```python
import hmac, hashlib, base64, time

MAX_TIMESTAMP_DRIFT_SECONDS = 300  # Standard Webhooks v1 default

def verify_signature(secret_bytes: bytes, headers, body_bytes: bytes) -> bool:
    """Constant-time HMAC-SHA256 verify per Standard Webhooks v1 / WH-04."""
    wid = headers.get('webhook-id')
    wts = headers.get('webhook-timestamp')
    wsig = headers.get('webhook-signature')
    if not (wid and wts and wsig):
        return False
    try:
        ts = int(wts)
    except ValueError:
        return False
    if abs(int(time.time()) - ts) > MAX_TIMESTAMP_DRIFT_SECONDS:
        return False
    signing_str = f"{wid}.{ts}.".encode() + body_bytes
    expected = hmac.new(secret_bytes, signing_str, hashlib.sha256).digest()
    for tok in wsig.split(' '):
        if not tok.startswith('v1,'):
            continue
        try:
            received = base64.b64decode(tok[3:])
        except Exception:
            continue
        # constant-time compare per WH-04
        if hmac.compare_digest(expected, received):
            return True
    return False
```

**`--verify-fixture <dir>` mode** — per RESEARCH.md §Code Examples Example 5:

```python
def _verify_fixture_mode(fixture_dir):
    """Read 5 fixture files and run verify_signature exactly as the HTTP path does."""
    secret = open(os.path.join(fixture_dir, 'secret.txt'), 'rb').read()
    wid = open(os.path.join(fixture_dir, 'webhook-id.txt'), 'rb').read().decode()
    wts = open(os.path.join(fixture_dir, 'webhook-timestamp.txt'), 'rb').read().decode()
    body = open(os.path.join(fixture_dir, 'payload.json'), 'rb').read()
    wsig = open(os.path.join(fixture_dir, 'expected-signature.txt'), 'rb').read().decode()
    headers = {'webhook-id': wid, 'webhook-timestamp': wts, 'webhook-signature': wsig}
    if verify_signature_skip_drift(secret, headers, body):  # drift-skip per Open Q2
        print("OK: fixture verified")
        sys.exit(0)
    else:
        print("FAIL: fixture did NOT verify", file=sys.stderr)
        sys.exit(1)

if __name__ == '__main__':
    if len(sys.argv) >= 3 and sys.argv[1] == '--verify-fixture':
        _verify_fixture_mode(sys.argv[2])
    else:
        # ... HTTPServer setup + serve_forever() ...
```

**Variations from the analog:**
- Stdlib HTTP server replaces tokio `TcpListener` (Python `http.server.HTTPServer` + `BaseHTTPRequestHandler`).
- Returns **graded HTTP status** (200 / 400 / 401 / 503), NOT always-200 (per D-09 D-12 retry contract).
- Adds the `verify_signature` function the analog never had (the analog logs but does not verify).
- Adds the `--verify-fixture` CLI mode the analog never had (CI gate path).
- Different port: `9991` (Python) vs `9999` (analog).
- Log path: `/tmp/cronduit-webhook-receiver-python.log` (per-language, distinct from the analog).

---

### `examples/webhook-receivers/go/receiver.go` — example mini-server, request-response (HMAC verify)

**Analog:** `examples/webhook_mock_server.rs` (same patterns as Python receiver above — re-use the same header comment, body cap, `Connection: close`, dual-log, port shape).

**Verify-function shape** — per RESEARCH.md §Code Examples Example 2:

```go
package main

import (
    "crypto/hmac"
    "crypto/sha256"
    "encoding/base64"
    "net/http"
    "strconv"
    "strings"
    "time"
)

const MAX_TIMESTAMP_DRIFT_SECONDS = 300

func verifySignature(secret []byte, h http.Header, body []byte) bool {
    wid := h.Get("webhook-id")
    wts := h.Get("webhook-timestamp")
    wsig := h.Get("webhook-signature")
    if wid == "" || wts == "" || wsig == "" {
        return false
    }
    ts, err := strconv.ParseInt(wts, 10, 64)
    if err != nil {
        return false
    }
    delta := time.Now().Unix() - ts
    if delta < 0 {
        delta = -delta
    }
    if delta > MAX_TIMESTAMP_DRIFT_SECONDS {
        return false
    }
    mac := hmac.New(sha256.New, secret)
    mac.Write([]byte(wid + "." + wts + "."))
    mac.Write(body)
    expected := mac.Sum(nil)
    for _, tok := range strings.Fields(wsig) {
        if !strings.HasPrefix(tok, "v1,") {
            continue
        }
        received, err := base64.StdEncoding.DecodeString(tok[3:])
        if err != nil {
            continue
        }
        // constant-time compare per WH-04
        if hmac.Equal(expected, received) {
            return true
        }
    }
    return false
}
```

**HTTP server pattern (stdlib `net/http`):**
```go
func main() {
    http.HandleFunc("/", handler)
    log.Println("Listening on http://127.0.0.1:9992/")
    if err := http.ListenAndServe("127.0.0.1:9992", nil); err != nil {
        log.Fatal(err)
    }
}

func handler(w http.ResponseWriter, r *http.Request) {
    body, err := io.ReadAll(http.MaxBytesReader(w, r.Body, 1<<20)) // 1 MiB cap
    if err != nil { http.Error(w, "body too large", http.StatusBadRequest); return }
    secret, _ := os.ReadFile(os.Getenv("WEBHOOK_SECRET_FILE"))
    if !verifySignature(secret, r.Header, body) {
        // Map outcome to status per D-12 retry contract
        http.Error(w, "verify failed", http.StatusUnauthorized)
        return
    }
    w.Header().Set("Connection", "close")
    w.WriteHeader(http.StatusOK)
    w.Write([]byte("OK"))
}
```

**Variations from analog:** Port `9992`. Log path `/tmp/cronduit-webhook-receiver-go.log`. Uses `http.MaxBytesReader` for body cap (Go's idiomatic 1-MiB cap; replaces the analog's manual `buf.len() > 1_048_576` check). `--verify-fixture` mode same shape as Python (CLI flag check at the top of `main()`, branches before `ListenAndServe`).

---

### `examples/webhook-receivers/node/receiver.js` — example mini-server, request-response (HMAC verify)

**Analog:** `examples/webhook_mock_server.rs` (same patterns).

**Verify-function shape** — per RESEARCH.md §Code Examples Example 3:

```js
const crypto = require('crypto');

const MAX_TIMESTAMP_DRIFT_SECONDS = 300;

function verifySignature(secret /*Buffer*/, headers, body /*Buffer*/) {
  const wid = headers['webhook-id'];
  const wts = headers['webhook-timestamp'];
  const wsig = headers['webhook-signature'];
  if (!wid || !wts || !wsig) return false;
  const ts = Number.parseInt(wts, 10);
  if (!Number.isFinite(ts)) return false;
  if (Math.abs(Math.floor(Date.now() / 1000) - ts) > MAX_TIMESTAMP_DRIFT_SECONDS) return false;
  const mac = crypto.createHmac('sha256', secret);
  mac.update(`${wid}.${ts}.`);
  mac.update(body);
  const expected = mac.digest();
  for (const tok of wsig.split(/\s+/)) {
    if (!tok.startsWith('v1,')) continue;
    let received;
    try { received = Buffer.from(tok.slice(3), 'base64'); }
    catch { continue; }
    // length guard MANDATORY — timingSafeEqual throws on mismatch (Pitfall 2)
    if (received.length !== expected.length) continue;
    // constant-time compare per WH-04
    if (crypto.timingSafeEqual(expected, received)) return true;
  }
  return false;
}
```

**HTTP server pattern (stdlib `http`, body as `Buffer`):**
```js
const http = require('http');
http.createServer((req, res) => {
  const chunks = [];
  let total = 0;
  req.on('data', (c) => {
    total += c.length;
    if (total > 1 << 20) { res.writeHead(413); res.end(); req.destroy(); return; }
    chunks.push(c);
  });
  req.on('end', () => {
    const body = Buffer.concat(chunks);
    // ... read secret, verifySignature, map to status code per D-12 ...
    res.setHeader('Connection', 'close');
    res.writeHead(200); res.end('OK');
  });
}).listen(9993, '127.0.0.1');
```

**Variations from analog:** Port `9993`. Log path `/tmp/cronduit-webhook-receiver-node.log`. Body accumulation via `Buffer` chunks (`req.on('data')` + `Buffer.concat`) — **MUST NOT call `req.setEncoding('utf8')`** (Pitfall 5 — corrupts byte-exact body). Length guard before `crypto.timingSafeEqual` (Pitfall 2 — throws RangeError on mismatch).

---

### `examples/webhook-receivers/{python,go,node}/README.md` — per-receiver docs, file-I/O

**Analog:** `docs/QUICKSTART.md` lines 1-30 (sectional introduction pattern with prereqs / install / run / common-pitfalls structure). No exact per-language-receiver-readme analog exists.

**Reusable header pattern** (`docs/QUICKSTART.md:1-5`):
```markdown
# Cronduit Quickstart

This guide walks you from `git clone` to a running scheduled job in under ten minutes.
```

**Section shape** (each per-receiver README, ~50-100 lines):
1. **Title** — `# Cronduit Webhook Receiver — Python` (etc.)
2. **One-paragraph intro** — what this receiver demonstrates (constant-time HMAC verify per WH-04).
3. **Install** — none. Quote: "Stdlib only. No `pip install` / `npm install` / `go mod download` required."
4. **Run command** — single bash block (`python3 receiver.py`).
5. **Expected log output** — sample stdout for first delivery (verified vs reject).
6. **Troubleshooting** — short list: "signature mismatch usually = wrong secret"; "503 means uncaught exception — check stderr"; "`webhook-signature` header missing = config disabled HMAC".
7. **SHA-256-only note** — verbatim: "Cronduit v1.2 ships SHA-256 only. No algorithm-agility (SHA-384/512/Ed25519) — if your operator workflow requires those, file a v1.3 roadmap issue."
8. **See-also** — link back to `docs/WEBHOOKS.md` and the other 2 per-language READMEs.

**Variation from the analog:** QUICKSTART is a hub doc covering the full deployment path; per-receiver README is single-purpose (one runtime, one verify, one troubleshooting list). No mermaid diagrams (3 already live in `docs/WEBHOOKS.md`).

---

### `docs/WEBHOOKS.md` — operator hub doc, file-I/O

**Analog:** `docs/CONFIG.md` (sibling hub-doc; same heading hierarchy, same TOC structure, same `[CITED]`-style cross-references).

**Heading-hierarchy pattern from `docs/CONFIG.md`** (`grep -n "^##\|^### "` summarized):

```markdown
# Cronduit Configuration Reference

Cronduit is configured via a **single TOML file**. ...

This document is the complete reference for every field the config file accepts.
For a step-by-step walkthrough, start with [`QUICKSTART.md`](./QUICKSTART.md).
For the architectural picture, read [`SPEC.md`](./SPEC.md).

## Table of contents

1. [...]
2. [...]

## File structure
...

## `[server]` section
### `[server].bind`
### `[server].timezone`
...
```

**Section shape (matches D-06 — 10 sections, ~250-400 lines):**
1. **Overview** + Standard Webhooks v1 link (defer wire format to spec; don't paraphrase).
2. **Three required headers** (`webhook-id`, `webhook-timestamp`, `webhook-signature: v1,<base64>`).
3. **SHA-256-only note** (verbatim — locked for v1.2; algo-agility is explicit OUT scope).
4. **Secret rotation** (receiver-side dual-secret window; cronduit holds one secret per job).
5. **Constant-time compare** + per-language primitive table (Python `hmac.compare_digest` / Go `hmac.Equal` / Node `crypto.timingSafeEqual`) per Open Q3 short-table guidance.
6. **Anti-replay** — 5-minute drift constant (`MAX_TIMESTAMP_DRIFT_SECONDS = 300`).
7. **Idempotency** — dedupe by `webhook-id` (production guidance).
8. **Retry-aware response codes** — verbatim D-12 retry-contract table (4xx permanent, 5xx transient).
9. **Receiver examples** — links to all 3 per-language receivers.
10. **Loopback Rust mock pointer** — link to `examples/webhook_mock_server.rs`.

**Cross-reference back-links** — sibling pattern from `docs/CONFIG.md:5`:
```markdown
For a step-by-step walkthrough, start with [`QUICKSTART.md`](./QUICKSTART.md).
For the architectural picture, read [`SPEC.md`](./SPEC.md).
```

**Mermaid diagrams** — re-use the 3 mermaid diagrams already drafted in `19-RESEARCH.md` (System Architecture sequenceDiagram, Verify Decision Tree flowchart, Phase 20 Retry stateDiagram). **NO ASCII art** (D-19).

**Variation from the analog:** CONFIG.md is field-by-field reference (every TOML key has a `### `[[jobs]].fieldname` ` heading); WEBHOOKS.md is a flat 10-section operator-facing hub. CONFIG.md has no `mermaid` diagrams; WEBHOOKS.md has 3.

---

### `docs/CONFIG.md` (MODIFY — add back-link in webhook section) — operator reference, file-I/O

**Analog:** `docs/CONFIG.md:5` (the existing `For a step-by-step walkthrough...` cross-reference pattern).

**Existing cross-reference pattern** (line 5):
```markdown
This document is the complete reference for every field the config file accepts.
For a step-by-step walkthrough that ends with a running dashboard, start with
[`QUICKSTART.md`](./QUICKSTART.md). For the architectural picture, read
[`SPEC.md`](./SPEC.md).
```

**New back-link** — single-line addition under the existing webhook section (currently CONFIG.md has no `## Webhooks` heading per `grep -n "webhook"` returning empty; the back-link likely lands either as a new short subsection under `## `[[jobs]]` blocks` near the docker job docs OR as a new `## Webhooks (configuration reference)` section that points OUT to WEBHOOKS.md). Per D-07: **no content duplication; CONFIG.md stays focused on TOML field reference**.

**Suggested literal:**
```markdown
For receiver implementation guidance (Python, Go, Node) and the verify protocol,
see [`WEBHOOKS.md`](./WEBHOOKS.md).
```

**Variation:** First addition of cross-reference content into CONFIG.md beyond the top-of-doc QUICKSTART/SPEC pointers.

---

### `README.md` (MODIFY — add one-line pointer) — top-level pointer, file-I/O

**Analog:** `README.md:174-175` (existing `> **New to Cronduit?** Start with...` pointer pattern).

**Existing pointer pattern** (lines 174-175):
```markdown
> **New to Cronduit?** Start with **[docs/QUICKSTART.md](./docs/QUICKSTART.md)** for a zero-to-first-scheduled-job walkthrough.
> **Looking up a specific field?** The complete reference is in **[docs/CONFIG.md](./docs/CONFIG.md)**. The section below is a cheat sheet.
```

**New one-line pointer** (per D-08 — "no README sprawl"):
```markdown
> **Need to verify webhook deliveries?** Receiver examples (Python/Go/Node) and the verify protocol live in **[docs/WEBHOOKS.md](./docs/WEBHOOKS.md)**.
```

**Variation:** The README does not currently have a `## Webhooks` section (`grep -n "webhook"` is empty in README.md as of phase start) — the pointer either lands in `## Configuration` next to the existing two pointers, or as a new dedicated subsection. Planner picks based on README flow; the pattern is identical.

---

### `examples/cronduit.toml` (MODIFY — add 3 new `wh-example-receiver-*` jobs) — example config, file-I/O

**Analog:** Existing `wh-example-signed` block at lines 240-245 of `examples/cronduit.toml` (exact match — same job archetype, same fields).

**Existing pattern** (lines 240-245):
```toml
# Template A -- signed delivery, default state filter, default coalescing.
# The dispatcher fires on `failed` and `timeout` (default states); first
# of each new streak (default fire_every = 1).
#
# [[jobs]]
# name = "wh-example-signed"
# schedule = "* * * * *"
# command = "false"  # always fails -- exercises the failure firing path
# use_defaults = false
# timeout = "5m"
# webhook = { url = "http://127.0.0.1:9999/signed", secret = "${WEBHOOK_SECRET}" }
```

**New blocks** — per RESEARCH.md §Code Examples Example 7:
```toml
# --- Phase 19 receiver examples (loopback HMAC verification) ----------------
#
# These three jobs target the Python/Go/Node example receivers shipped at
# examples/webhook-receivers/{python,go,node}/. They are commented-out
# templates by default — uncomment to enable, and `export WEBHOOK_SECRET` first.
# See docs/WEBHOOKS.md for the receiver run instructions.
#
# [[jobs]]
# name = "wh-example-receiver-python"
# schedule = "* * * * *"
# command = "false"
# use_defaults = false
# timeout = "5m"
# webhook = { url = "http://127.0.0.1:9991/", secret = "${WEBHOOK_SECRET}" }
#
# [[jobs]]
# name = "wh-example-receiver-go"
# schedule = "* * * * *"
# command = "false"
# use_defaults = false
# timeout = "5m"
# webhook = { url = "http://127.0.0.1:9992/", secret = "${WEBHOOK_SECRET}" }
#
# [[jobs]]
# name = "wh-example-receiver-node"
# schedule = "* * * * *"
# command = "false"
# use_defaults = false
# timeout = "5m"
# webhook = { url = "http://127.0.0.1:9993/", secret = "${WEBHOOK_SECRET}" }
```

**Variations from analog:**
- `url` differs per language: 9991 (Python) / 9992 (Go) / 9993 (Node).
- All three blocks ship **commented-out** (matches Phase 18 D-05 — keep `docker compose up` smoke clean).
- Same `command = "false"`, `use_defaults = false`, `timeout = "5m"` shape verbatim — locked LBL-04 + Watchtower-inheritance precedent.
- New comment block at top introduces the trio (mirrors lines 226-234's introduction of the existing signed templates).

---

### `justfile` (MODIFY — append 6 new `uat-webhook-receiver-*` recipes) — build/UAT recipes, request-response (CLI)

**Analog:** Existing `uat-webhook-mock` (`:326`), `uat-webhook-fire` (`:337`), `uat-webhook-verify` (`:349`) recipe family. **Direct exact match** — Phase 19 recipes append to this same family.

**Existing `uat-webhook-mock` pattern** (lines 322-329):
```just
# Mock HTTP receiver on 127.0.0.1:9999 — logs every request to stdout AND
# /tmp/cronduit-webhook-mock.log. Use Ctrl-C to stop.
[group('uat')]
[doc('Phase 18 — start mock HTTP receiver on 127.0.0.1:9999 (logs requests)')]
uat-webhook-mock:
    @echo "Starting webhook mock on http://127.0.0.1:9999/  (log: /tmp/cronduit-webhook-mock.log)"
    @echo "Maintainer: Ctrl-C to stop. Run 'just uat-webhook-verify' in another terminal."
    cargo run --example webhook_mock_server
```

**Existing `uat-webhook-fire` recipe-calls-recipe pattern** (lines 335-342):
```just
[group('uat')]
[doc('Phase 18 — force Run Now on a webhook-configured job (operator-supplied JOB_NAME)')]
uat-webhook-fire JOB_NAME:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▶ UAT: triggering run for {{JOB_NAME}} — watch the receiver and the cronduit log"
    JOB_ID=$(just api-job-id "{{JOB_NAME}}")
    just api-run-now "$JOB_ID"
```

**New `uat-webhook-receiver-python` recipe** — mirrors `uat-webhook-mock` shape verbatim (per RESEARCH.md §Code Examples Example 8):
```just
[group('uat')]
[doc('Phase 19 — start Python receiver, fire wh-example-receiver-python, tail log')]
uat-webhook-receiver-python:
    @echo "Starting Python receiver on http://127.0.0.1:9991/"
    @echo "Maintainer: in another terminal, run 'just dev', then 'just uat-webhook-fire wh-example-receiver-python'."
    @echo "Watch this terminal for the 'verified' line. Ctrl-C to stop the receiver."
    python3 examples/webhook-receivers/python/receiver.py
```

**New `uat-webhook-receiver-python-verify-fixture` recipe** — recipe-internal tamper variants, per RESEARCH.md §Pattern 5:
```just
[group('uat')]
[doc('Phase 19 — verify Python receiver against fixture (canonical + 3 tamper variants)')]
uat-webhook-receiver-python-verify-fixture:
    #!/usr/bin/env bash
    set -euo pipefail
    FIX=tests/fixtures/webhook-v1
    cd examples/webhook-receivers/python

    # 1. Canonical — must verify
    python3 receiver.py --verify-fixture "$(realpath ../../../$FIX)" \
        || { echo "FAIL: canonical fixture did not verify"; exit 1; }

    # 2. Mutated secret — must FAIL
    BAD_SECRET=$(mktemp -d)
    cp ../../../$FIX/* "$BAD_SECRET"/
    printf 'WRONG' > "$BAD_SECRET"/secret.txt
    if python3 receiver.py --verify-fixture "$BAD_SECRET" 2>/dev/null; then
        echo "FAIL: mutated-secret variant verified — should have failed"; exit 1
    fi

    # 3. Mutated body — must FAIL
    BAD_BODY=$(mktemp -d)
    cp ../../../$FIX/* "$BAD_BODY"/
    sed -i 's/"v1"/"X1"/' "$BAD_BODY"/payload.json
    if python3 receiver.py --verify-fixture "$BAD_BODY" 2>/dev/null; then
        echo "FAIL: mutated-body variant verified — should have failed"; exit 1
    fi

    # 4. Drift > 5 min — must FAIL (re-sign with stale ts)
    BAD_TS=$(mktemp -d)
    cp ../../../$FIX/* "$BAD_TS"/
    echo $(($(date +%s) - 600)) > "$BAD_TS"/webhook-timestamp.txt
    NEW_SIG=$(python3 -c "import hmac,hashlib,base64; \
        s=open('$BAD_TS/secret.txt','rb').read(); \
        wid=open('$BAD_TS/webhook-id.txt','rb').read(); \
        ts=open('$BAD_TS/webhook-timestamp.txt','rb').read(); \
        body=open('$BAD_TS/payload.json','rb').read(); \
        m=hmac.new(s, wid+b'.'+ts+b'.'+body, hashlib.sha256); \
        print('v1,'+base64.b64encode(m.digest()).decode())")
    echo "$NEW_SIG" > "$BAD_TS"/expected-signature.txt
    if python3 receiver.py --verify-fixture "$BAD_TS" 2>/dev/null; then
        echo "FAIL: drift variant verified — should have failed"; exit 1
    fi

    echo "OK: all 4 tamper variants behave correctly"
```

**Variations from the analog:**
- 6 new recipes (3 real-cronduit + 3 verify-fixture) vs the analog's 3 (`mock`, `fire`, `verify`).
- Real-cronduit recipes are foreground (mirror `uat-webhook-mock`); verify-fixture recipes are non-interactive shell scripts using `#!/usr/bin/env bash` (mirror `uat-webhook-fire`'s recipe-internal-bash pattern).
- All Phase 19 recipes are `[group('uat')]` and `[doc('Phase 19 — ...')]` consistent with Phase 18 family.
- Go and Node verify-fixture recipes are byte-identical to Python except: `python3 receiver.py` → `go run receiver.go` / `node receiver.js`. Re-signing one-liner stays Python (it's available on every CI runner via `setup-python@v5` and is the cleanest stdlib option).

---

### `.github/workflows/ci.yml` (MODIFY — add `webhook-interop` matrix job) — CI matrix job, event-driven (CI)

**Analog:** Existing `compose-smoke` matrix job at `.github/workflows/ci.yml:137-378` (role-match — sibling top-level job using `strategy.matrix` over `ubuntu-latest`, delegates `run:` steps through `extractions/setup-just@v2`, scoped `permissions: contents: read`). For the **shape** of pure language-toolchain matrix (no Docker compose), the closer-fit existing precedent is the `test` job at `:66-108` (matrix over `arch`).

**Existing matrix-job pattern** (`test` job, lines 66-108 — top-level fields):
```yaml
test:
  name: test ${{ matrix.arch }}
  runs-on: ubuntu-latest
  timeout-minutes: 30
  permissions:
    contents: read
  strategy:
    fail-fast: false
    matrix:
      arch: [amd64, arm64]
  env:
    SQLX_OFFLINE: "true"
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - uses: Swatinem/rust-cache@v2
      with:
        key: ${{ matrix.arch }}
    - uses: extractions/setup-just@v2
    - uses: taiki-e/install-action@v2
      with:
        tool: nextest,cargo-zigbuild
    ...
    - run: just nextest
```

**Existing `extractions/setup-just@v2` pattern** (`lint` at `:34`, `test` at `:84`, `image` at `:123`):
```yaml
- uses: extractions/setup-just@v2
```

**New `webhook-interop` job** — per RESEARCH.md §Code Examples Example 6:
```yaml
  webhook-interop:
    name: webhook-interop (${{ matrix.lang }})
    runs-on: ubuntu-latest
    timeout-minutes: 10
    permissions:
      contents: read
    strategy:
      fail-fast: false
      matrix:
        lang: [python, go, node]
    steps:
      - uses: actions/checkout@v4
      - if: matrix.lang == 'python'
        uses: actions/setup-python@v5
        with:
          python-version: '3.x'
      - if: matrix.lang == 'go'
        uses: actions/setup-go@v5
        with:
          go-version: 'stable'
      - if: matrix.lang == 'node'
        uses: actions/setup-node@v4
        with:
          node-version: '20'
      - uses: extractions/setup-just@v2
      - run: just uat-webhook-receiver-${{ matrix.lang }}-verify-fixture
```

**Variations from the analog:**
- Adds 3 new conditional `setup-{python,go,node}` toolchain steps (gated on `matrix.lang`) — the existing matrix jobs only set up Rust toolchain.
- No `Swatinem/rust-cache` (no Rust compile in this job) — fastest possible matrix cell, <30s expected.
- No DB pre-pull (no testcontainers).
- New job is a **same-file sibling** (NOT a separate workflow file) per RESEARCH.md §Alternatives Considered — keeps PR-page CI summary in one place.
- **CI gate from day one** (NOT `continue-on-error: true` like the existing `cargo-deny` step at `:58`) — interop drift is more dangerous than dependency-policy drift.
- Job is independent of existing `lint` / `test` / `image` / `compose-smoke` jobs (no `needs:` dependency).

---

### `.planning/phases/19-webhook-hmac-signing-receiver-examples/19-HUMAN-UAT.md` — UAT artifact, file-I/O

**Analog:** `.planning/phases/18-webhook-payload-state-filter-coalescing/18-HUMAN-UAT.md` (exact — most recent webhook-domain UAT artifact, ships locked recipe-call structure). For overall scaffolding (preamble, prereqs table, `[ ] Maintainer-validated` checkboxes, sign-off section), `17-HUMAN-UAT.md` is the canonical newer-shape precedent (`-CHECKLIST` + `pass criteria` per item).

**Existing `18-HUMAN-UAT.md` preamble pattern** (lines 1-13):
```markdown
# Phase 18 Human UAT — Webhook Payload + State-Filter + Coalescing

> **Maintainer-validated only.** Per project memory `feedback_uat_user_validates.md`, Claude does NOT mark these scenarios passed — the maintainer runs each scenario and flips the `[ ]` to `[x]` themselves. Per `feedback_uat_use_just_commands.md`, every step references a `just` recipe — NEVER raw `curl`/`cargo`/`docker`.

## Prerequisites

| Prereq | Recipe | Notes |
|--------|--------|-------|
| Workspace builds clean | `just ci` | Full CI gate: fmt + clippy + openssl-check + nextest + schema-diff + image |
| rustls invariant holds | `just openssl-check` | `cargo tree -i openssl-sys` returns empty across native + arm64-musl + amd64-musl |
| Signed-delivery scenarios enabled | edit `examples/cronduit.toml` | Scenarios 1, 3, 5, 6 below... |
```

**Existing `17-HUMAN-UAT.md` section-shape pattern** (lines 12-21):
```markdown
- [x] **U1 — README labels subsection renders correctly on GitHub.**
  - **Recipe:** None — visual review of `README.md` after PR is open.
  - **Steps:**
    1. After the PR is opened, navigate to the PR's "Files changed" tab.
    2. Find the `README.md` diff and confirm the rendered preview shows the new `### Labels` subsection.
    ...
  - **Pass criteria:** Mermaid diagram renders; table renders; code blocks render. NO ASCII art anywhere (D-07).
```

**Existing `17-HUMAN-UAT.md` sign-off pattern** (lines 72-78):
```markdown
## After All Boxes Ticked

- The maintainer comments on the PR with `UAT passed` (or equivalent) once every box above is ticked.
- `gsd-execute-phase` (or the orchestrator) treats the phase as complete only after the human-validation comment lands.

**Validated by:** Maintainer (Robert) on 2026-04-29 — all 6 UAT items passed locally per D-09.
```

**Phase 19 UAT scenarios — based on D-21 (only `just`-callable surface) and the 6 new recipes:**
1. Workspace builds clean (`just ci`).
2. Rust fixture test green (`just nextest`) — guards the `sign_v1_locks_interop_fixture` test.
3. Python receiver runs end-to-end against real cronduit (`just uat-webhook-receiver-python` + `just uat-webhook-fire wh-example-receiver-python`).
4. Go receiver runs end-to-end (analogous).
5. Node receiver runs end-to-end (analogous).
6. Python receiver verify-fixture passes all 4 tamper variants (`just uat-webhook-receiver-python-verify-fixture`).
7. Go receiver verify-fixture passes (analogous).
8. Node receiver verify-fixture passes (analogous).
9. `docs/WEBHOOKS.md` renders cleanly on GitHub (mermaid diagrams render as SVG; D-12 retry table is a real markdown table; SHA-256-only callout is visible).
10. README pointer + CONFIG.md back-link both render.

**All checkboxes start `[ ] Maintainer-validated`** (D-22). Sign-off section blank until maintainer flips boxes.

**Variations from analog:**
- Phase 18's UAT was `[x]`-checked by maintainer post-merge with a "schedule-driven path" caveat; Phase 19's starts blank (the maintainer flips boxes as they validate).
- Phase 18's prereqs table includes `Signed-delivery scenarios enabled` (config edit step); Phase 19's prereqs table includes `Receiver toolchain available` (Python/Go/Node version check).
- Phase 19 scenarios are language-symmetric (3 receivers × 2 modes = 6 scenarios + 4 platform-wide); Phase 18's were behavior-asymmetric (signed/unsigned/coalescing/filter/secret each tested once).

---

## Shared Patterns

### Pattern: Stdlib HMAC Verify (cross-language)
**Source:** RESEARCH.md §Pattern 3 + §Code Examples 1-3
**Apply to:** All 3 receiver files (`receiver.py`, `receiver.go`, `receiver.js`)

**Per-language constant-time primitive table** (every receiver MUST use the documented stdlib primitive — never `==` on hex/base64):

| Language | Primitive | Length-guard required? |
|---|---|---|
| Python | `hmac.compare_digest(a, b)` | No (handles unequal length internally) |
| Go | `hmac.Equal(macA, macB)` | No (handles unequal length internally) |
| Node | `crypto.timingSafeEqual(bufA, bufB)` | **YES** — throws `RangeError` on mismatch (Pitfall 2) |

Each receiver's verify function carries a `# constant-time compare per WH-04` comment immediately above the primitive call.

---

### Pattern: `Connection: close` Response Discipline
**Source:** `examples/webhook_mock_server.rs:88-93`
**Apply to:** All 3 receiver files

```rust
// Connection: close → reqwest will NOT keep this socket alive,
// so the next delivery opens a fresh TCP stream. This avoids
// half-state issues if our reader breaks early.
let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
```

Each receiver MUST set `Connection: close` on every response (HTTP framing concern; mitigates reqwest stream reuse on cronduit's dispatcher per Phase 18 T-18-35).

---

### Pattern: 1-MiB Body Cap
**Source:** `examples/webhook_mock_server.rs:67-72`
**Apply to:** All 3 receiver files

```rust
// Safety cap: don't grow forever on a misbehaving client.
if buf.len() > 1_048_576 {
    eprintln!("[webhook-mock] body too large from {peer}; dropping");
    return;
}
```

Each receiver caps the request body at 1 MiB (matches the Rust mock):
- Go: `http.MaxBytesReader(w, r.Body, 1<<20)` (idiomatic).
- Python: check `Content-Length` header BEFORE `rfile.read()`.
- Node: cumulative `total += chunk.length` check inside `req.on('data')`.

---

### Pattern: Multi-token Signature Parsing
**Source:** RESEARCH.md §Pattern 2 (Standard Webhooks v1 spec compliance)
**Apply to:** All 3 receiver files

`webhook-signature` header is a **space-delimited list** of versioned signatures (`v1,sigA v1,sigB v1a,sigC`). Receivers parse all `v1,...` tokens and accept on first match. Forward-compat with v1.3+ multi-secret rotation. ~3 LOC per language; sample literal:

- Python: `for tok in wsig.split(' ')` then `if not tok.startswith('v1,'): continue`
- Go: `for _, tok := range strings.Fields(wsig)` then `if !strings.HasPrefix(tok, "v1,") { continue }`
- Node: `for (const tok of wsig.split(/\s+/))` then `if (!tok.startsWith('v1,')) continue`

---

### Pattern: 5-Minute Drift Constant
**Source:** RESEARCH.md §Pattern 4 (Standard Webhooks JS reference impl)
**Apply to:** All 3 receiver files + `docs/WEBHOOKS.md` § 6

```python
MAX_TIMESTAMP_DRIFT_SECONDS = 300  # Standard Webhooks v1 default
```

Hard-coded per D-11 (configurability not in scope). Each receiver compares `abs(now() - ts) > MAX_TIMESTAMP_DRIFT_SECONDS` and rejects 400 if outside.

**Drift-skip in fixture-verify mode:** the `--verify-fixture` CLI mode SKIPS the drift check (the fixture is a static known-good test vector, not a live event) per RESEARCH.md §Open Question 2. The HTTP path always enforces.

---

### Pattern: Retry-aware HTTP Status Codes
**Source:** D-12 + RESEARCH.md §Pattern 1 (D-09 / D-12 retry contract)
**Apply to:** All 3 receiver files + `docs/WEBHOOKS.md` § 8

Each receiver maps verify outcome → status verbatim per D-12:

| Outcome | HTTP | Cronduit Phase 20 |
|---|---|---|
| Missing/malformed headers | 400 | Permanent — drop, no retry |
| Drift > 5 min | 400 | Permanent — drop, no retry |
| HMAC mismatch | 401 | Permanent — drop, no retry |
| Verify success | 200 | Counter increment |
| Unexpected exception | 503 | Transient — Phase 20 retries |

This contract is verbatim in `docs/WEBHOOKS.md` § 8 — Phase 20's retry implementation MUST inherit unchanged.

---

### Pattern: Idempotency Comment Block
**Source:** RESEARCH.md §Specific Ideas (idempotency template)
**Apply to:** All 3 receiver files (verbatim, in success branch)

```python
# In production: dedupe by webhook-id to handle Phase 20 retries.
# E.g., short-TTL Set/Map (in-memory) or DB unique constraint on webhook-id.
# Cronduit may redeliver on transient receiver failures (5xx response → retry t=30s, t=300s).
# First successful 2xx terminates the retry chain.
```

Per D-10 — comment block only, NOT working code. Working dedup needs a TTL story; spec note suffices for v1.2.

---

### Pattern: Mermaid-only Diagrams
**Source:** Project memory `feedback_diagrams_mermaid.md` + D-19
**Apply to:** `docs/WEBHOOKS.md`, all per-receiver `README.md` (if any diagram is added), `19-HUMAN-UAT.md`

Every diagram is a mermaid code block. NO ASCII art. Three mermaid diagrams already drafted in `19-RESEARCH.md` (System Architecture sequenceDiagram, Verify Decision Tree flowchart, Phase 20 Retry stateDiagram) — re-use verbatim in `docs/WEBHOOKS.md`.

---

### Pattern: `[ ] Maintainer-validated` UAT Checkboxes
**Source:** Project memory `feedback_uat_user_validates.md` + D-22
**Apply to:** `19-HUMAN-UAT.md`

Every UAT scenario starts `[ ] Maintainer-validated`. Claude NEVER flips the box. Per the `17-HUMAN-UAT.md` precedent, the maintainer flips boxes after running the cited recipe and adds a one-line "Validated by: Maintainer (Robert) on YYYY-MM-DD" sign-off block at the bottom.

---

### Pattern: `just`-recipe-Only UAT Steps
**Source:** Project memory `feedback_uat_use_just_commands.md` + D-21
**Apply to:** `19-HUMAN-UAT.md`

Every UAT step references a named `just` recipe. NEVER raw `curl` / `cargo` / `docker` / URL. Phase 19 recipes available: `dev`, `check-config PATH`, `ci`, `openssl-check`, `nextest`, `uat-webhook-fire JOB_NAME`, `uat-webhook-receiver-{python,go,node}`, `uat-webhook-receiver-{python,go,node}-verify-fixture`. If a step requires anything outside this set (e.g., visual GitHub render review), state `Recipe: None — visual review` explicitly per the `17-HUMAN-UAT.md U1` precedent.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `tests/fixtures/webhook-v1/secret.txt` (and 4 sibling fixture data files) | fixture (data) | file-I/O | No plaintext non-config fixture exists in the repo. Existing `tests/fixtures/*.toml` are all TOML configs consumed by Rust parsers. The new fixture is consumed by **four runtimes** (Rust + Python + Go + Node) — so the format is the most-portable shape (raw bytes, one-value-per-file, no parsing). Planner uses RESEARCH.md §Recommended Project Structure + §Pitfall 3 verbatim for content shape. |
| `tests/fixtures/webhook-v1/README.md` | docs (fixture-local) | file-I/O | No README-per-fixture-directory pattern exists in the repo. Closest analog is the comment-header block at top of `examples/cronduit.toml:200-211`. Planner uses RESEARCH.md §Wave 0 Gaps + §Pitfall 3 (no-newline) for section structure. |

For both, the planner SHOULD reference RESEARCH.md directly (Pattern 5 / Pitfall 3 / Open Question 1) instead of synthesizing from a non-existent codebase analog.

---

## Metadata

**Analog search scope:** `examples/`, `src/webhooks/`, `docs/`, `tests/`, `tests/fixtures/`, `justfile`, `.github/workflows/`, `.planning/phases/{17,18}/`, `README.md`.

**Files scanned:**
- `examples/webhook_mock_server.rs` (110 lines, full read)
- `src/webhooks/dispatcher.rs:130-419` (290 lines, targeted read covering `sign_v1` + `tests` module)
- `examples/cronduit.toml:200-256` (57 lines, targeted read covering Phase 18 webhook templates)
- `justfile:260-470` (210 lines, targeted read covering `uat-*` recipe family)
- `.github/workflows/ci.yml` (full 378 lines)
- `docs/CONFIG.md:1-60`, `:300-420` (targeted: header pattern + section style)
- `docs/QUICKSTART.md:1-60` (header + section pattern)
- `docs/SPEC.md:1-50` (header pattern)
- `.planning/phases/18-webhook-payload-state-filter-coalescing/18-HUMAN-UAT.md` (full 126 lines)
- `.planning/phases/17-custom-docker-labels-seed-001/17-HUMAN-UAT.md` (full 78 lines)
- `README.md:170-225` (targeted: webhook-context section)

**Pattern extraction date:** 2026-04-29

**Key decision:** Cross-language receivers (Python/Go/Node) inherit the **form factor** of `examples/webhook_mock_server.rs` (header comment, body cap, dual-log, `Connection: close`, log path) and the **verify function** from RESEARCH.md §Code Examples 1-3 (which themselves cite stdlib docs via Context7). Planner does NOT need to invent new patterns — Phase 18's Rust mock is the form factor; Standard Webhooks v1 spec + per-language stdlib are the verify implementation. The 6 new `just` recipes copy `uat-webhook-mock` / `uat-webhook-fire` shape verbatim. The Rust fixture test extends `src/webhooks/dispatcher.rs::tests::sign_v1_known_fixture` family verbatim. The CI matrix job extends `.github/workflows/ci.yml`'s existing `compose-smoke` matrix shape. The UAT artifact extends `18-HUMAN-UAT.md`'s checkbox-and-recipe shape verbatim.
