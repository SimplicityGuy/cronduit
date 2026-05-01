#!/usr/bin/env node
/**
 * Phase 19 — Cronduit webhook receiver reference (Node, stdlib only).
 *
 * Listens on 127.0.0.1:9993 and verifies Standard Webhooks v1 signatures
 * using HMAC-SHA256 + constant-time compare (`crypto.timingSafeEqual`).
 * Mirrors the form factor of `examples/webhook_mock_server.rs` (Phase 18)
 * but upgrades the always-200 mock into a graded-status verifier per the
 * D-12 retry contract.
 *
 * USE ONLY for local maintainer UAT validation. Loopback-bound (127.0.0.1).
 * Never expose to the public internet. Production receivers should:
 *   - run behind a reverse proxy / TLS terminator
 *   - implement working idempotency dedup (this script ships a comment
 *     block only — see success branch)
 *
 * Run modes:
 *   node receiver.js                          HTTP server mode (default)
 *   node receiver.js --verify-fixture <dir>   CI/UAT fixture-verify mode
 *
 * CRITICAL Pitfall 2: `crypto.timingSafeEqual` THROWS RangeError on length
 * mismatch. Every call MUST be preceded by a length-equality guard
 * (`received.length !== expected.length` — see verify core below).
 * The length check is non-constant-time (fine — HMAC output is fixed-length;
 * length difference reveals zero secret material).
 *
 * Cronduit v1.2 ships SHA-256 only. No algorithm-agility (SHA-384/512/Ed25519).
 */

'use strict';

const crypto = require('crypto');
const fs = require('fs');
const http = require('http');
const path = require('path');

// --- Constants -------------------------------------------------------------
const PORT = 9993;
const LOG_PATH = '/tmp/cronduit-webhook-receiver-node.log';
const MAX_BODY_BYTES = 1 << 20; // 1 MiB; matches webhook_mock_server.rs cap
const MAX_TIMESTAMP_DRIFT_SECONDS = 300; // Standard Webhooks v1 default (D-11)

// Fixed allowlist of fixture filenames the --verify-fixture mode reads.
// Used to defend against path-traversal in the CLI argument (Pitfall: an
// operator could pass `--verify-fixture ../../etc`; we resolve and validate
// the directory but still restrict reads to this allowlist of bare names).
const FIXTURE_FILES = Object.freeze({
  secret: 'secret.txt',
  id: 'webhook-id.txt',
  ts: 'webhook-timestamp.txt',
  body: 'payload.json',
  sig: 'expected-signature.txt',
});

// --- Verify core (copy-pasteable) ------------------------------------------
/**
 * Constant-time HMAC-SHA256 verify per Standard Webhooks v1 / WH-04.
 * Signing string is `${webhook-id}.${webhook-timestamp}.${body}` over the
 * BYTE-EXACT body (NEVER JSON.parse + JSON.stringify — Pitfall 5).
 *
 * @param {Buffer} secret  - raw key bytes from fs.readFileSync; do NOT trim (Pitfall 3)
 * @param {Object} headers - Node lowercases incoming header names (req.headers)
 * @param {Buffer} body    - bytes received on the wire (Buffer.concat of chunks)
 * @returns {boolean}
 */
function verifySignature(secret, headers, body) {
  return _verifyWithDrift(secret, headers, body, true);
}

function _verifyWithDrift(secret, headers, body, checkDrift) {
  const wid = headers['webhook-id'];
  const wts = headers['webhook-timestamp'];
  const wsig = headers['webhook-signature'];
  if (!wid || !wts || !wsig) return false;
  // Strict unsigned-decimal validation per Standard Webhooks v1 wire format.
  // `Number.parseInt` truncates trailing junk (`"1735abc"` -> 1735), accepts
  // a leading "+", whitespace, and silently lossy values above MAX_SAFE_INTEGER
  // — all of which would diverge from the signing-string raw-bytes contract
  // (see BL-01) and the WR-01 review note.
  if (!/^\d+$/.test(wts)) return false;
  const ts = Number.parseInt(wts, 10);
  if (!Number.isFinite(ts) || ts > Number.MAX_SAFE_INTEGER) return false;
  if (checkDrift &&
      Math.abs(Math.floor(Date.now() / 1000) - ts) > MAX_TIMESTAMP_DRIFT_SECONDS) {
    return false;
  }
  const mac = crypto.createHmac('sha256', secret);
  // Sign over the RAW header bytes (`wts`), not the parsed integer (`ts`).
  // The cronduit Rust dispatcher and the Standard Webhooks v1 spec both
  // treat the timestamp portion of the signing string as the byte-exact
  // value of the `webhook-timestamp` header. Using the parsed int would
  // silently diverge for any non-canonical-decimal form (leading zeros,
  // leading `+`, embedded whitespace, trailing junk that parseInt
  // truncates, etc.). See review BL-01.
  mac.update(`${wid}.${wts}.`);
  mac.update(body);
  const expected = mac.digest(); // Buffer (32 bytes — HMAC-SHA256 output)
  // Multi-token parse per Standard Webhooks v1 (forward-compat with v1.3+).
  for (const tok of wsig.split(/\s+/)) {
    if (!tok.startsWith('v1,')) continue;
    let received;
    try {
      received = Buffer.from(tok.slice(3), 'base64');
    } catch {
      continue;
    }
    // CRITICAL length guard MANDATORY — `crypto.timingSafeEqual` throws
    // RangeError on length mismatch (Pitfall 2). The length check is
    // non-constant-time, which is fine: HMAC-SHA256 output is fixed
    // 32 bytes; a length difference can only come from a structurally
    // malformed signature and reveals zero secret material.
    if (received.length !== expected.length) continue;
    // constant-time compare per WH-04
    if (crypto.timingSafeEqual(expected, received)) return true;
  }
  return false;
}

// --- Logging ---------------------------------------------------------------
function logLine(line) {
  console.error(line);
  try {
    fs.appendFileSync(LOG_PATH, line + '\n');
  } catch {
    /* swallow log-file errors; stderr is the source of truth */
  }
}

// --- HTTP handler ----------------------------------------------------------
function handleRequest(req, res) {
  res.setHeader('Connection', 'close');
  if (req.method !== 'POST') {
    res.writeHead(405); res.end('method not allowed'); return;
  }

  // loopback-only — secret presence checked after body read; justified per D-09 receiver responsibility model
  // Body accumulation as raw Buffer chunks. Pitfall 5: do NOT decode the
  // request stream to utf8 — that corrupts byte-exact body bytes for any
  // non-ASCII payload and breaks HMAC verification. Buffer chunks only.
  const chunks = [];
  let total = 0;
  req.on('data', (chunk) => {
    total += chunk.length;
    if (total > MAX_BODY_BYTES) {
      logLine(`[node-receiver] body too large (${total} bytes); rejecting 413`);
      res.writeHead(413); res.end('body too large');
      req.destroy();
      return;
    }
    chunks.push(chunk);
  });

  req.on('end', () => {
    try {
      const body = Buffer.concat(chunks);

      // 1. Read secret from env-var path. Raw bytes (Buffer); no trim (Pitfall 3).
      const secretPath = process.env.WEBHOOK_SECRET_FILE;
      if (!secretPath) {
        logLine('[node-receiver] WEBHOOK_SECRET_FILE not set; rejecting 503');
        res.writeHead(503); res.end('server misconfigured'); return;
      }
      const secret = fs.readFileSync(secretPath);

      // 2. Map verify outcome to status per D-12 retry contract.
      const wid = req.headers['webhook-id'];
      const wts = req.headers['webhook-timestamp'];
      const wsig = req.headers['webhook-signature'];
      if (!wid || !wts || !wsig) {
        res.writeHead(400); res.end('missing required headers'); return;
      }
      // Strict unsigned-decimal validation per Standard Webhooks v1 wire
      // format. `parseInt` would accept whitespace, leading "+", and trailing
      // junk that disagree with the signing-string raw-bytes contract
      // (BL-01/WR-01).
      if (!/^\d+$/.test(wts)) {
        res.writeHead(400); res.end('malformed webhook-timestamp'); return;
      }
      const ts = Number.parseInt(wts, 10);
      if (!Number.isFinite(ts) || ts > Number.MAX_SAFE_INTEGER) {
        res.writeHead(400); res.end('malformed webhook-timestamp'); return;
      }
      if (Math.abs(Math.floor(Date.now() / 1000) - ts) > MAX_TIMESTAMP_DRIFT_SECONDS) {
        res.writeHead(400); res.end('timestamp drift > 5min'); return;
      }
      if (!verifySignature(secret, req.headers, body)) {
        res.writeHead(401); res.end('hmac verify failed'); return;
      }

      // 3. Verify success — log + 200.
      // In production: dedupe by webhook-id to handle Phase 20 retries.
      // E.g., short-TTL Set/Map (in-memory) or DB unique constraint on webhook-id.
      // Cronduit may redeliver on transient receiver failures (5xx response → retry t=30s, t=300s).
      // First successful 2xx terminates the retry chain.
      logLine(`[node-receiver] verified webhook-id=${wid} bytes=${body.length}`);
      res.writeHead(200); res.end('OK');
    } catch (e) {
      logLine(`[node-receiver] unexpected exception: ${e && e.stack || e}; rejecting 503`);
      try { res.writeHead(503); res.end('unexpected exception'); } catch { /* response already sent */ }
    }
  });

  req.on('error', (e) => {
    logLine(`[node-receiver] req error: ${e}`);
  });
}

// --- Fixture-verify mode ---------------------------------------------------
// Sanitize the user-supplied directory BEFORE it reaches path.resolve:
// reject any string containing '..' segments, NUL bytes, or non-printable
// characters. Then resolve to an absolute path, require it to exist as a
// real directory (fs.realpathSync resolves symlinks), and only then read
// files from a fixed allowlist of bare filenames. Bare names are
// hard-coded constants (NOT user input), so the joined paths cannot escape
// the resolved directory.
function _sanitizeFixtureArg(rawArg) {
  if (typeof rawArg !== 'string' || rawArg.length === 0) {
    throw new Error('fixture directory argument is empty');
  }
  if (rawArg.length > 4096) {
    throw new Error('fixture directory argument too long');
  }
  // Reject NUL bytes and non-printable characters.
  // eslint-disable-next-line no-control-regex
  if (/[\x00-\x1f\x7f]/.test(rawArg)) {
    throw new Error('fixture directory argument contains control characters');
  }
  // Reject parent-directory traversal segments.
  const normalized = path.normalize(rawArg);
  const segments = normalized.split(path.sep);
  if (segments.includes('..')) {
    throw new Error('fixture directory argument contains parent traversal');
  }
  return normalized;
}

function _resolveFixtureDir(rawArg) {
  const sanitized = _sanitizeFixtureArg(rawArg);
  // nosemgrep: javascript.lang.security.audit.path-traversal.path-join-resolve-traversal.path-join-resolve-traversal
  const abs = path.resolve(sanitized);
  const real = fs.realpathSync(abs);
  const stat = fs.statSync(real);
  if (!stat.isDirectory()) {
    throw new Error(`fixture path is not a directory: ${real}`);
  }
  return real;
}

function _readFixtureFile(safeDir, allowlistedName) {
  // safeDir is the realpath of an existing directory (validated by
  // _sanitizeFixtureArg + fs.realpathSync); allowlistedName is a hard-coded
  // constant from FIXTURE_FILES (NOT user input).
  // nosemgrep: javascript.lang.security.audit.path-traversal.path-join-resolve-traversal.path-join-resolve-traversal
  const full = path.join(safeDir, allowlistedName);
  return fs.readFileSync(full);
}

function verifyFixtureMode(fixtureDir) {
  let safeDir;
  try {
    safeDir = _resolveFixtureDir(fixtureDir);
  } catch (e) {
    console.error(`FAIL: cannot read fixture: ${e.message || e}`);
    return 1;
  }
  try {
    const secret = _readFixtureFile(safeDir, FIXTURE_FILES.secret);
    const wid = _readFixtureFile(safeDir, FIXTURE_FILES.id).toString('utf8');
    const wts = _readFixtureFile(safeDir, FIXTURE_FILES.ts).toString('utf8');
    const body = _readFixtureFile(safeDir, FIXTURE_FILES.body);
    const wsig = _readFixtureFile(safeDir, FIXTURE_FILES.sig).toString('utf8');
    const headers = {
      'webhook-id': wid,
      'webhook-timestamp': wts,
      'webhook-signature': wsig,
    };
    if (_verifyWithDrift(secret, headers, body, false)) {
      console.log('OK: fixture verified');
      return 0;
    }
    console.error('FAIL: fixture did NOT verify');
    return 1;
  } catch (e) {
    console.error(`FAIL: cannot read fixture: ${e}`);
    return 1;
  }
}

function main() {
  if (process.argv.length >= 4 && process.argv[2] === '--verify-fixture') {
    process.exit(verifyFixtureMode(process.argv[3]));
  }
  // Cleartext HTTP is intentional: loopback-bound (127.0.0.1:9993) reference
  // receiver for local maintainer UAT only — see header docstring's
  // "USE ONLY for local maintainer UAT validation" warning. Production
  // receivers run behind a reverse proxy / TLS terminator (D-09).
  // nosemgrep: problem-based-packs.insecure-transport.js-node.using-http-server.using-http-server
  const server = http.createServer(handleRequest);
  server.listen(PORT, '127.0.0.1', () => {
    logLine(`[node-receiver] listening on http://127.0.0.1:${PORT}/  (log: ${LOG_PATH})`);
  });
}

main();
