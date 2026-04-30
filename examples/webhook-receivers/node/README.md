# Cronduit Webhook Receiver — Node

A stdlib-only reference receiver demonstrating constant-time HMAC-SHA256
verification of cronduit's [Standard Webhooks v1](https://github.com/standard-webhooks/standard-webhooks/blob/main/spec/standard-webhooks.md)
deliveries (WH-04). Mirrors the form factor of
[`examples/webhook_mock_server.rs`](../../webhook_mock_server.rs) but upgrades
the always-200 mock into a graded-status verifier.

## Install

Stdlib only. No `npm install` required. Tested with Node 20+ (whatever
`ubuntu-latest` ships in 2026).

Single-file program — there is no `package.json`. Run via `node` directly.

## Run

```bash
# Point WEBHOOK_SECRET_FILE at the secret file your cronduit instance signs with.
export WEBHOOK_SECRET_FILE=/path/to/your-webhook-secret.txt
node examples/webhook-receivers/node/receiver.js
```

The receiver listens on `http://127.0.0.1:9993/` and logs to stdout AND
`/tmp/cronduit-webhook-receiver-node.log`. Ctrl-C to stop.

For fixture-verify mode (CI / smoke):

```bash
node examples/webhook-receivers/node/receiver.js --verify-fixture tests/fixtures/webhook-v1
# prints "OK: fixture verified" and exits 0 on success.
```

## Expected log output

On a verified delivery from cronduit:

```
[node-receiver] listening on http://127.0.0.1:9993/  (log: /tmp/cronduit-webhook-receiver-node.log)
[node-receiver] verified webhook-id=01HZAFY0V1F1BS1F2H8GV4XG3R bytes=312
```

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| 401 from receiver | `WEBHOOK_SECRET_FILE` content does not match cronduit's `webhook.secret` | Confirm both sides reference the same secret bytes (no trailing newline — see Pitfall 3 in `tests/fixtures/webhook-v1/README.md`) |
| 400 missing required headers | cronduit dispatcher dropped a header OR delivery is not a Standard Webhooks v1 delivery | Confirm cronduit version >= 1.2.0 and `webhook = { ... }` config has `secret` set (not `unsigned = true`) |
| 400 timestamp drift > 5min | Receiver clock is wrong OR cronduit clock is wrong | `ntpdate` / `chrony`; verify both sides via `date` |
| 503 with `RangeError: Input buffers must have the same byte length` | Pitfall 2 — `crypto.timingSafeEqual` throws on length mismatch. The shipped receiver guards against this. If you forked the verify function and dropped the `received.length !== expected.length` guard, restore it. | Add the length guard back BEFORE every `timingSafeEqual` call |
| 503 server misconfigured | `WEBHOOK_SECRET_FILE` env var is unset | Set the env var before launch |
| Body bytes look corrupted | Receiver code decodes the request stream to utf8 (Pitfall 5) | Remove that decode; collect raw `Buffer` chunks and `Buffer.concat` them |

## SHA-256 only

Cronduit v1.2 ships SHA-256 only. No algorithm-agility (SHA-384/512/Ed25519)
— if your operator workflow requires those, file a v1.3 roadmap issue.

## See also

- [`docs/WEBHOOKS.md`](../../../docs/WEBHOOKS.md) — operator-facing hub doc
- [`examples/webhook-receivers/python/README.md`](../python/README.md) — Python variant (port 9991)
- [`examples/webhook-receivers/go/README.md`](../go/README.md) — Go variant (port 9992)
- [`examples/webhook_mock_server.rs`](../../webhook_mock_server.rs) — Phase 18 Rust loopback mock (always-200 — for header/payload inspection only)
