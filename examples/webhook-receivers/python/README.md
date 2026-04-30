# Cronduit Webhook Receiver — Python

A stdlib-only reference receiver demonstrating constant-time HMAC-SHA256
verification of cronduit's [Standard Webhooks v1](https://github.com/standard-webhooks/standard-webhooks/blob/main/spec/standard-webhooks.md)
deliveries (WH-04). Mirrors the form factor of
[`examples/webhook_mock_server.rs`](../../webhook_mock_server.rs) but upgrades
the always-200 mock into a graded-status verifier.

## Install

Stdlib only. No `pip install` required. Tested with Python 3.12+ (whatever
`ubuntu-latest` ships in 2026); any Python 3.8+ should work.

## Run

```bash
# Point WEBHOOK_SECRET_FILE at the secret file your cronduit instance signs with.
export WEBHOOK_SECRET_FILE=/path/to/your-webhook-secret.txt
python3 examples/webhook-receivers/python/receiver.py
```

The receiver listens on `http://127.0.0.1:9991/` and logs to stdout AND
`/tmp/cronduit-webhook-receiver-python.log`. Ctrl-C to stop.

For fixture-verify mode (CI / smoke):

```bash
python3 examples/webhook-receivers/python/receiver.py --verify-fixture tests/fixtures/webhook-v1
# prints "OK: fixture verified" and exits 0 on success.
```

## Expected log output

On a verified delivery from cronduit:

```
[python-receiver] listening on http://127.0.0.1:9991/  (log: /tmp/cronduit-webhook-receiver-python.log)
[python-receiver] verified webhook-id=01HZAFY0V1F1BS1F2H8GV4XG3R bytes=312
```

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| 401 from receiver | `WEBHOOK_SECRET_FILE` content does not match cronduit's `webhook.secret` | Confirm both sides reference the same secret bytes (no trailing newline — see Pitfall 3 in `tests/fixtures/webhook-v1/README.md`) |
| 400 missing required headers | cronduit dispatcher dropped a header OR delivery is not a Standard Webhooks v1 delivery | Confirm cronduit version >= 1.2.0 and `webhook = { ... }` config has `secret` set (not `unsigned = true`) |
| 400 timestamp drift > 5min | Receiver clock is wrong OR cronduit clock is wrong | `ntpdate` / `chrony`; verify both sides via `date` |
| 503 server misconfigured | `WEBHOOK_SECRET_FILE` env var is unset | Set the env var before launch |
| No deliveries arrive | cronduit's webhook coalescing collapses streaks (default `fire_every = 1`) | Set `webhook.fire_every = 0` to test legacy per-failure mode |

## SHA-256 only

Cronduit v1.2 ships SHA-256 only. No algorithm-agility (SHA-384/512/Ed25519)
— if your operator workflow requires those, file a v1.3 roadmap issue.

## See also

- [`docs/WEBHOOKS.md`](../../../docs/WEBHOOKS.md) — operator-facing hub doc (wire format, headers, retry contract, secret rotation)
- [`examples/webhook-receivers/go/README.md`](../go/README.md) — Go variant (port 9992)
- [`examples/webhook-receivers/node/README.md`](../node/README.md) — Node variant (port 9993)
- [`examples/webhook_mock_server.rs`](../../webhook_mock_server.rs) — Phase 18 Rust loopback mock (always-200 — for header/payload inspection only, no verify)
