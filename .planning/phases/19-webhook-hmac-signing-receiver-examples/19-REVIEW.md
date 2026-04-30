---
phase: 19-webhook-hmac-signing-receiver-examples
reviewed: 2026-04-30T17:59:57Z
depth: standard
files_reviewed: 19
files_reviewed_list:
  - .github/workflows/ci.yml
  - README.md
  - docs/CONFIG.md
  - docs/WEBHOOKS.md
  - examples/cronduit.toml
  - examples/webhook-receivers/go/README.md
  - examples/webhook-receivers/go/receiver.go
  - examples/webhook-receivers/node/README.md
  - examples/webhook-receivers/node/receiver.js
  - examples/webhook-receivers/python/README.md
  - examples/webhook-receivers/python/receiver.py
  - justfile
  - src/webhooks/dispatcher.rs
  - tests/fixtures/webhook-v1/.gitattributes
  - tests/fixtures/webhook-v1/README.md
  - tests/fixtures/webhook-v1/expected-signature.txt
  - tests/fixtures/webhook-v1/payload.json
  - tests/fixtures/webhook-v1/secret.txt
  - tests/fixtures/webhook-v1/webhook-id.txt
  - tests/fixtures/webhook-v1/webhook-timestamp.txt
findings:
  blocker: 1
  warning: 6
  total: 7
status: issues_found
---

# Phase 19: Code Review Report

**Reviewed:** 2026-04-30T17:59:57Z
**Depth:** standard
**Files Reviewed:** 19
**Status:** issues_found

## Summary

Phase 19 ships a clean, well-documented set of three reference webhook receivers (Python, Go, Node) plus the locked Standard Webhooks v1 fixture. The security-critical primitives (constant-time HMAC compare, the Node length-guard pre-`timingSafeEqual`, no-trailing-newline fixture discipline, secret-as-bytes-never-trim) all land correctly. The CI matrix gates on the fixture from day one (no `continue-on-error`).

The most serious finding is a **cross-language interop divergence** in how the Python and Node receivers reconstruct the HMAC signing string — they use the *parsed integer* of the timestamp header, while the Go receiver and the cronduit Rust dispatcher use the *raw header bytes*. This passes the canonical fixture (where the integer round-trips byte-for-byte) but silently fails for any timestamp string that is not its own canonical decimal form (leading zeros, embedded whitespace, leading `+`, trailing junk that `parseInt` truncates, etc.). It is a latent interop bug that the locked fixture cannot catch.

Five additional defects are flagged as Warning: the Node receiver's `parseInt` accepts garbage suffixes that pass `Number.isFinite`, the Node `_sanitizeFixtureArg` claims to reject `..` segments but `path.normalize` collapses them silently, the Python receiver silently treats chunked-transfer bodies as empty, the receiver READMEs claim "logs to stdout" but Python/Node actually write to stderr, and there is minor documentation duplication between `docs/WEBHOOKS.md` and the receiver source-comment explanation of the length-guard rationale (this is mild, intentional, and probably fine — flagged for awareness not action).

The fixture, the dispatcher Rust unit test, the operator-facing `docs/WEBHOOKS.md`, and the README/CONFIG.md/example wiring are all in good shape and match the security expectations the phase set out to ship.

---

## Blockers

### BL-01: Python and Node receivers sign-string with parsed integer instead of raw timestamp header

**File:** `examples/webhook-receivers/python/receiver.py:67`, `examples/webhook-receivers/node/receiver.js:82`
**Issue:** The cronduit Rust dispatcher signs over the bytes of the timestamp header *as transmitted* (`webhook_ts.to_string()` — `src/webhooks/dispatcher.rs:244`). The Go receiver mirrors that contract exactly — it computes HMAC over `wid + "." + wts + "."` (raw `wts` header bytes). The Python and Node receivers, however, parse the header to an integer first and re-serialize via `f"{wid}.{ts}."` / `` `${wid}.${ts}.` `` — using the *parsed* integer, NOT the raw header.

For the canonical fixture (`webhook-timestamp.txt = 1735689600`) the two paths produce the same bytes, so the fixture-verify CI gate passes. But the contract diverges silently for any timestamp string that is not its own canonical decimal form. Demonstration:

```
'01735689600'   parsed=1735689600  Python/Node-style HMAC ≠ Go-style HMAC
'1735689600 '   parsed=1735689600  Python/Node-style HMAC ≠ Go-style HMAC (Python int() accepts trailing space)
' 1735689600'   parsed=1735689600  Python/Node-style HMAC ≠ Go-style HMAC
'+1735689600'   parsed=1735689600  Python/Node-style HMAC ≠ Go-style HMAC
'1735689600.0'  parsed=1735689600  Node-style HMAC ≠ Go-style HMAC (Node parseInt truncates)
```

Today this only matters if a future cronduit version changes how it serializes the timestamp, OR if an operator implements a non-cronduit signer that uses one of these formats. But the explicit goal of this phase is to lock the wire format across four runtimes (Rust, Python, Go, Node) — and the Rust↔Go contract is "raw header bytes" while the Rust↔Python and Rust↔Node contracts are accidentally "raw header IFF it round-trips through the language's int parser." That is an interop bug, even if the canonical fixture happens to satisfy it.

The Standard Webhooks v1 spec is also explicit on this point: the signing string is `${id}.${timestamp}.${body}` where `${timestamp}` is the raw header value, not a re-serialized integer.

**Fix:** Have all three receivers compute HMAC over the raw header bytes. Parse `ts` to int only for the drift check; never use it in the signing-string composition.

Python (`receiver.py:67`):
```python
# BEFORE
signing_str = f"{wid}.{ts}.".encode() + body_bytes

# AFTER — use the raw wts header bytes, not the parsed int
signing_str = f"{wid}.{wts}.".encode() + body_bytes
```

Node (`receiver.js:82`):
```javascript
// BEFORE
mac.update(`${wid}.${ts}.`);

// AFTER — use the raw wts header bytes, not the parsed int
mac.update(`${wid}.${wts}.`);
```

(Go is already correct at `receiver.go:78`: `mac.Write([]byte(wid + "." + wts + "."))`.)

Add a regression test to the fixture-verify recipes that mutates `webhook-timestamp.txt` to `01735689600` (semantically same int, different bytes) with a re-signed `expected-signature.txt`, and confirm all three receivers verify it identically. Or — a tighter test — sign a fixture with leading-zero timestamp using the Rust `sign_v1`, then confirm all three receivers accept it.

---

## Warnings

### WR-01: Node receiver accepts non-numeric timestamp suffixes via `Number.parseInt`

**File:** `examples/webhook-receivers/node/receiver.js:75-76,159-160`
**Issue:** `Number.parseInt(wts, 10)` followed by `Number.isFinite(ts)` does NOT validate that `wts` is a clean decimal string. `parseInt` truncates trailing junk (`"1735689600abc"` → `1735689600`), accepts whitespace, and accepts values larger than `Number.MAX_SAFE_INTEGER` (silently lossy). All such inputs pass `Number.isFinite`. Combined with BL-01's parsed-int signing-string, this means the Node receiver will *accept* a delivery whose timestamp header is `"1735689600abc"` if the sender computed HMAC using the parsed integer — but reject the same delivery if the sender (correctly per spec) computed HMAC over the raw header.

The Python receiver has a milder version of this issue: `int("1735689600abc")` raises `ValueError`, but `int(" 1735689600 ")` and `int("+1735689600")` are accepted with leading/trailing whitespace and explicit sign.

**Fix:** Validate that the timestamp is a clean unsigned decimal string before parsing. After fixing BL-01, this is also the easy place to enforce wire-format strictness:

Node:
```javascript
if (!/^\d+$/.test(wts)) return false;  // reject leading zeros if you want spec-strict; or just /^[1-9]\d*$/
const ts = Number.parseInt(wts, 10);
if (!Number.isFinite(ts) || ts > Number.MAX_SAFE_INTEGER) return false;
```

Python:
```python
if not wts.isascii() or not wts.isdigit():
    return False
ts = int(wts)
```

This isn't a security exploit on its own, but it is a parser-divergence vector that compounds BL-01.

---

### WR-02: Node `_sanitizeFixtureArg` documentation/behavior mismatch on `..` rejection

**File:** `examples/webhook-receivers/node/receiver.js:208-213`
**Issue:** The function comment (lines 208-209) and the error message ("contains parent traversal") imply that any string containing a `..` segment is rejected. The implementation calls `path.normalize` *first* and then splits on `path.sep` and looks for `..`. But `path.normalize` collapses embedded `..` segments — `path.normalize('foo/../etc')` returns `'etc'`, and `path.normalize('/foo/../etc/passwd')` returns `'/etc/passwd'`. So:

```
'foo/../etc'             -> normalize -> 'etc'             ACCEPTED (comment claims rejected)
'/foo/../etc/passwd'     -> normalize -> '/etc/passwd'     ACCEPTED
'../etc/passwd'          -> normalize -> '../etc/passwd'   REJECTED (works as documented)
```

This is **not** a security bug because `_readFixtureFile` only joins a hard-coded allowlist of 5 bare filenames against the resolved directory, so the operator can only redirect to a different-but-valid fixture directory. They cannot read arbitrary files. It IS a documentation/code mismatch that suggests defense-in-depth that is not actually present, and a future maintainer who relies on the claim while extending the receiver might introduce a real path-traversal.

On Windows (not a Cronduit deployment target, but the receivers ship as cross-platform reference code) `path.sep` is `\` and the check would not find `..` segments split by `/`, but normalize would convert them — adding more confusion.

**Fix:** Either make the comment match the behavior, or make the behavior match the comment. The simplest fix is to check `..` against the *raw* argument before normalize:

```javascript
function _sanitizeFixtureArg(rawArg) {
  if (typeof rawArg !== 'string' || rawArg.length === 0) {
    throw new Error('fixture directory argument is empty');
  }
  if (rawArg.length > 4096) {
    throw new Error('fixture directory argument too long');
  }
  if (/[\x00-\x1f\x7f]/.test(rawArg)) {
    throw new Error('fixture directory argument contains control characters');
  }
  // Reject parent-traversal segments in the RAW argument, before normalize.
  // path.normalize collapses embedded `..` (e.g. 'foo/../etc' -> 'etc'),
  // which would silently slip past a post-normalize check.
  if (rawArg.split(/[\\/]/).includes('..')) {
    throw new Error('fixture directory argument contains parent traversal');
  }
  return path.normalize(rawArg);
}
```

---

### WR-03: Python receiver silently treats chunked-transfer bodies as empty

**File:** `examples/webhook-receivers/python/receiver.py:112,117`
**Issue:** `cl = int(self.headers.get('content-length') or 0)` and `body_bytes = self.rfile.read(cl) if cl > 0 else b""`. If a client sends `Transfer-Encoding: chunked` with no Content-Length (RFC 7230 explicitly disallows both being present, and chunked is the canonical way to send a body without knowing the length up-front), the receiver silently reads zero bytes and then HMAC-rejects the (legit but un-bodied-as-far-as-the-receiver-knows) delivery with 401.

Cronduit's `reqwest`-based `HttpDispatcher` always sets Content-Length, so this is not a real interop bug today. But the receiver is documented as a reference implementation for "Standard Webhooks v1" — and a third-party signer using a different HTTP client that switches to chunked above some threshold would silently 401 here. The Node receiver handles this correctly (it accumulates `data` events from the request stream regardless of Content-Length); the Go receiver also handles this correctly (`io.ReadAll(http.MaxBytesReader(w, r.Body, ...))`).

**Fix:** Either explicitly reject chunked transfers with a 400, or read the body using `BaseHTTPRequestHandler`'s wfile in a chunked-aware way. The simplest spec-correct fix is to reject:

```python
if self.headers.get('transfer-encoding', '').lower() == 'chunked':
    self._respond(400, b"chunked transfer not supported")
    return
cl_str = self.headers.get('content-length')
if cl_str is None:
    self._respond(411, b"length required")
    return
try:
    cl = int(cl_str)
except ValueError:
    self._respond(400, b"malformed content-length")
    return
if cl < 0 or cl > MAX_BODY_BYTES:
    self._respond(400, b"body length out of range")
    return
```

This is loopback-only reference code, so the severity is bounded. Worth fixing for the "matches Node and Go behavior" interop polish.

---

### WR-04: Receiver READMEs claim "logs to stdout" but receivers actually write to stderr

**File:** `examples/webhook-receivers/python/README.md:22`, `examples/webhook-receivers/node/README.md:24`
**Issue:** Both Python and Node READMEs claim "The receiver listens on `http://127.0.0.1:9991/` and logs to stdout AND `/tmp/cronduit-webhook-receiver-python.log`." — but the Python implementation writes to `sys.stderr` (`receiver.py:86`) and the Node implementation writes to `console.error` (`receiver.js:108`), which is also stderr. The Go README has the same wording (`go/README.md:25`) but the Go implementation uses `log.Println` (line 101), which the `log` package defaults to stderr.

This is a small but real tripwire for an operator who runs the receiver via `python3 receiver.py > log.txt` expecting a log file: they get an empty file plus a `/tmp/...` file, plus stderr scrolling on the terminal, and waste 2 minutes wondering why redirection failed.

**Fix:** Change "logs to stdout" → "logs to stderr" in all three READMEs. (Or change the implementations to use stdout — but stderr is the conventional choice for log lines in tools like this, since stdout is reserved for primary program output, and the receivers do print "OK"/"FAIL" to stdout for the fixture-verify mode.)

---

### WR-05: Documentation duplication: length-guard rationale appears in three places

**File:** `docs/WEBHOOKS.md:135-145`, `examples/webhook-receivers/node/receiver.js:21-26,94-99`, `examples/webhook-receivers/node/README.md:50`
**Issue:** The phase brief asks that "receivers are the single source of truth — docs should reference, not copy-paste." The Node length-guard rationale ("`crypto.timingSafeEqual` throws RangeError on length mismatch; HMAC-SHA256 output is fixed-length so the length-mismatch reveals zero secret material") appears in three places:

1. `docs/WEBHOOKS.md` § Constant-time compare (lines 135-145, including the table footnote)
2. `examples/webhook-receivers/node/receiver.js` header docstring (lines 21-26) AND inline comment (lines 94-99)
3. `examples/webhook-receivers/node/README.md` Troubleshooting (line 50)

The triple-redundancy is *intentional* and *probably fine* — the audience of a security-critical inline comment is different from the docs-hub reader, and the rationale is short. But it is a place where future drift can introduce divergence (one location gets edited, the other two go stale). Lower-severity consideration than WR-01..WR-04.

**Fix:** Optional. If you want the receivers to be the single source of truth, replace `docs/WEBHOOKS.md` lines 138-145 with a sentence pointing readers at the inline comment in `receiver.js`. Alternatively, leave as-is and accept the duplication as defense-in-depth (each reader path encounters the warning once); just be aware it exists during future doc edits.

---

### WR-06: Node receiver rejects `parseInt`-passing-but-not-finite values inconsistently with Python

**File:** `examples/webhook-receivers/node/receiver.js:74-76`
**Issue:** Node's `Number.parseInt('', 10)` returns `NaN`, which `Number.isFinite` correctly rejects. But `Number.parseInt('  ', 10)` also returns `NaN`. Meanwhile `wts = '   '` would have already been rejected by the `if (!wid || !wts || !wsig) return false` check on line 74 because `'   '` is truthy. Thus a header `webhook-timestamp:    ` (spaces) is accepted as truthy, parsed to NaN, and correctly rejected by `Number.isFinite`. Behavior matches Python's `int('   ')` raising and being rejected. Fine.

But the Python `int()` accepts `int('+1735689600')` and `int(' 1735689600 ')` (returns `1735689600`); Node `parseInt('+1735689600', 10)` also returns `1735689600`. Both then skip past `Number.isFinite` / `ValueError` checks and proceed to the BL-01-described divergent signing string. This is fundamentally the same root cause as BL-01 + WR-01 but worth surfacing as its own warning because the validation gates are weak across both languages: a header with a `+` sign or whitespace passes drift check, then HMAC-rejects (correctly! — but for the wrong reason: not "spec-illegal header format" but "parsed int doesn't match raw"). Operators debugging a 401 will not know to suspect their timestamp formatter.

**Fix:** Tighten both validators to require an unsigned-decimal-only timestamp string before parsing. Same fix as WR-01 applies — once the validator is strict, this divergence narrows.

---

_Reviewed: 2026-04-30T17:59:57Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
