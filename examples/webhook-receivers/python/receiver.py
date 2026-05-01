#!/usr/bin/env python3
"""Phase 19 — Cronduit webhook receiver reference (Python, stdlib only).

Listens on 127.0.0.1:9991 and verifies Standard Webhooks v1 signatures
using HMAC-SHA256 + constant-time compare (`hmac.compare_digest`). Mirrors
the form factor of `examples/webhook_mock_server.rs` (Phase 18) but
upgrades the always-200 mock into a graded-status verifier per the D-12
retry contract.

USE ONLY for local maintainer UAT validation. Loopback-bound (127.0.0.1).
Never expose to the public internet. Production receivers should:
  - swap HTTPServer for ThreadingHTTPServer (single-threaded by default
    — Pitfall 6)
  - implement working idempotency dedup (this script ships a comment
    block only — see success branch)
  - run behind a reverse proxy / TLS terminator

Run modes:
  python3 receiver.py                           HTTP server mode (default)
  python3 receiver.py --verify-fixture <dir>    CI/UAT fixture-verify mode

Cronduit v1.2 ships SHA-256 only. No algorithm-agility (SHA-384/512/Ed25519).
"""

import base64
import hashlib
import hmac
import http.server
import os
import socketserver
import sys
import time

# --- Constants -------------------------------------------------------------
PORT = 9991
LOG_PATH = "/tmp/cronduit-webhook-receiver-python.log"
MAX_BODY_BYTES = 1 << 20  # 1 MiB; matches webhook_mock_server.rs cap
MAX_TIMESTAMP_DRIFT_SECONDS = 300  # Standard Webhooks v1 default (D-11)


# --- Verify core (copy-pasteable) ------------------------------------------
def verify_signature(secret_bytes: bytes, headers, body_bytes: bytes) -> bool:
    """Constant-time HMAC-SHA256 verify per Standard Webhooks v1 / WH-04.

    Signing string is `${webhook-id}.${webhook-timestamp}.${body}` over the
    BYTE-EXACT body (NEVER json.loads + json.dumps — Pitfall 5).

    secret_bytes — raw key bytes (no .strip()/decode — Pitfall 3)
    headers — case-insensitive accessor (dict-like with `.get`)
    body_bytes — bytes received on the wire
    """
    return _verify_with_drift(secret_bytes, headers, body_bytes, check_drift=True)


def _verify_with_drift(secret_bytes, headers, body_bytes, *, check_drift):
    wid = headers.get('webhook-id')
    wts = headers.get('webhook-timestamp')
    wsig = headers.get('webhook-signature')
    if not (wid and wts and wsig):
        return False
    # Strict unsigned-decimal validation per Standard Webhooks v1 wire format.
    # Python's `int()` accepts leading/trailing whitespace and a leading "+",
    # which would parse OK but produce a header that disagrees with the
    # signing-string raw-bytes contract (see BL-01) and the WR-01 review note.
    if not (wts.isascii() and wts.isdigit()):
        return False
    ts = int(wts)
    if check_drift and abs(int(time.time()) - ts) > MAX_TIMESTAMP_DRIFT_SECONDS:
        return False
    # Sign over the RAW header bytes (`wts`), not the parsed integer (`ts`).
    # The cronduit Rust dispatcher and the Standard Webhooks v1 spec both
    # treat the timestamp portion of the signing string as the byte-exact
    # value of the `webhook-timestamp` header. Using the parsed int would
    # silently diverge for any non-canonical-decimal form (leading zeros,
    # leading `+`, surrounding whitespace, etc.). See review BL-01.
    signing_str = f"{wid}.{wts}.".encode() + body_bytes
    expected = hmac.new(secret_bytes, signing_str, hashlib.sha256).digest()
    # Multi-token parse per Standard Webhooks v1 (forward-compat with v1.3+
    # multi-secret rotation; cronduit currently emits one v1, token).
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


# --- Logging ---------------------------------------------------------------
def _log(line: str) -> None:
    sys.stderr.write(line + "\n")
    sys.stderr.flush()
    try:
        with open(LOG_PATH, "a") as f:
            f.write(line + "\n")
    except OSError:
        pass


# --- HTTP handler ----------------------------------------------------------
class _Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):  # silence default noisy access log
        pass

    def _respond(self, code: int, body: bytes = b"") -> None:
        self.send_response(code)
        self.send_header("Connection", "close")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if body:
            self.wfile.write(body)

    def do_POST(self):
        try:
            # loopback-only — secret presence checked after body read; justified per D-09 receiver responsibility model
            # 1. Body cap before read (Pitfall: don't grow forever).
            # Reject chunked transfers explicitly (RFC 7230 forbids
            # Transfer-Encoding + Content-Length together; cronduit's
            # reqwest-based dispatcher always sets Content-Length, but a
            # third-party signer using chunked encoding would otherwise
            # silently read 0 bytes and 401 with a misleading reason —
            # see review WR-03). Node and Go receivers handle chunked
            # transparently via streaming readers; Python's stdlib HTTP
            # server does not, so reject up-front with 400.
            if self.headers.get('transfer-encoding', '').lower() == 'chunked':
                _log("[python-receiver] chunked transfer not supported; rejecting 400")
                self._respond(400, b"chunked transfer not supported")
                return
            cl_str = self.headers.get('content-length')
            if cl_str is None:
                _log("[python-receiver] missing Content-Length; rejecting 411")
                self._respond(411, b"length required")
                return
            try:
                cl = int(cl_str)
            except ValueError:
                _log(f"[python-receiver] malformed Content-Length: {cl_str!r}; rejecting 400")
                self._respond(400, b"malformed content-length")
                return
            if cl < 0 or cl > MAX_BODY_BYTES:
                _log(f"[python-receiver] body length out of range ({cl}); rejecting 400")
                self._respond(400, b"body length out of range")
                return
            body_bytes = self.rfile.read(cl) if cl > 0 else b""

            # 2. Read secret from env-var path. Raw bytes, no strip (Pitfall 3).
            secret_path = os.environ.get('WEBHOOK_SECRET_FILE')
            if not secret_path:
                _log("[python-receiver] WEBHOOK_SECRET_FILE not set; rejecting 503")
                self._respond(503, b"server misconfigured")
                return
            with open(secret_path, 'rb') as f:
                secret_bytes = f.read()

            # 3. Map verify outcome to status per D-12 retry contract.
            # First do a precheck so we can return 400 vs 401 vs 503 correctly.
            wid = self.headers.get('webhook-id')
            wts = self.headers.get('webhook-timestamp')
            wsig = self.headers.get('webhook-signature')
            if not (wid and wts and wsig):
                self._respond(400, b"missing required headers")
                return
            # Strict unsigned-decimal validation per Standard Webhooks v1 wire
            # format. `int()` would accept whitespace and a leading "+" that
            # disagree with the signing-string raw-bytes contract (BL-01/WR-01).
            if not (wts.isascii() and wts.isdigit()):
                self._respond(400, b"malformed webhook-timestamp")
                return
            ts = int(wts)
            if abs(int(time.time()) - ts) > MAX_TIMESTAMP_DRIFT_SECONDS:
                self._respond(400, b"timestamp drift > 5min")
                return
            if not verify_signature(secret_bytes, self.headers, body_bytes):
                self._respond(401, b"hmac verify failed")
                return

            # 4. Verify success — log + 200.
            # In production: dedupe by webhook-id to handle Phase 20 retries.
            # E.g., short-TTL Set/Map (in-memory) or DB unique constraint on webhook-id.
            # Cronduit may redeliver on transient receiver failures (5xx response → retry t=30s, t=300s).
            # First successful 2xx terminates the retry chain.
            _log(f"[python-receiver] verified webhook-id={wid} bytes={len(body_bytes)}")
            self._respond(200, b"OK")
        except Exception as e:  # noqa: BLE001 — D-12 catch-all => 503 transient
            _log(f"[python-receiver] unexpected exception: {e!r}; rejecting 503")
            self._respond(503, b"unexpected exception")


# --- Fixture-verify mode ---------------------------------------------------
def _verify_fixture_mode(fixture_dir: str) -> int:
    """Read 5 fixture files; run verify_signature with drift-skip; exit 0/1.

    Drift-skip ONLY: HMAC + multi-token parse still run. Fixture timestamps
    are intentionally past (locked by Plan 01); the HTTP path correctly
    rejects them on drift, but a static test-vector verification is the
    whole point of this mode.
    """
    try:
        with open(os.path.join(fixture_dir, 'secret.txt'), 'rb') as f:
            secret = f.read()
        with open(os.path.join(fixture_dir, 'webhook-id.txt'), 'rb') as f:
            wid = f.read().decode()
        with open(os.path.join(fixture_dir, 'webhook-timestamp.txt'), 'rb') as f:
            wts = f.read().decode()
        with open(os.path.join(fixture_dir, 'payload.json'), 'rb') as f:
            body = f.read()
        with open(os.path.join(fixture_dir, 'expected-signature.txt'), 'rb') as f:
            wsig = f.read().decode()
    except OSError as e:
        print(f"FAIL: cannot read fixture: {e}", file=sys.stderr)
        return 1
    headers = {'webhook-id': wid, 'webhook-timestamp': wts, 'webhook-signature': wsig}
    if _verify_with_drift(secret, headers, body, check_drift=False):
        print("OK: fixture verified")
        return 0
    print("FAIL: fixture did NOT verify", file=sys.stderr)
    return 1


def main():
    if len(sys.argv) >= 3 and sys.argv[1] == '--verify-fixture':
        sys.exit(_verify_fixture_mode(sys.argv[2]))
    # HTTP mode.
    addr = ('127.0.0.1', PORT)
    with socketserver.TCPServer(addr, _Handler) as httpd:
        _log(f"[python-receiver] listening on http://{addr[0]}:{addr[1]}/  (log: {LOG_PATH})")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            _log("[python-receiver] shutting down on Ctrl-C")


if __name__ == '__main__':
    main()
