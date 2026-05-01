# Cronduit Webhook Receiver — Go

A stdlib-only reference receiver demonstrating constant-time HMAC-SHA256
verification of cronduit's [Standard Webhooks v1](https://github.com/standard-webhooks/standard-webhooks/blob/main/spec/standard-webhooks.md)
deliveries (WH-04). Mirrors the form factor of
[`examples/webhook_mock_server.rs`](../../webhook_mock_server.rs) but upgrades
the always-200 mock into a graded-status verifier.

## Install

Stdlib only. No `go mod download` required. Tested with Go 1.21+ (whatever
`ubuntu-latest` ships in 2026).

Single-file program — there is no `go.mod`. Run via `go run` directly.

## Run

```bash
# Point WEBHOOK_SECRET_FILE at the secret file your cronduit instance signs with.
export WEBHOOK_SECRET_FILE=/path/to/your-webhook-secret.txt
go run examples/webhook-receivers/go/receiver.go
```

The receiver listens on `http://127.0.0.1:9992/` and logs to stderr AND
`/tmp/cronduit-webhook-receiver-go.log`. Ctrl-C to stop.

(stderr is the conventional choice for log lines: stdout is reserved for
primary program output, and the receiver prints `OK`/`FAIL` lines to
stdout in `--verify-fixture` mode. If you want to capture logs with
shell redirection, use `go run receiver.go 2> log.txt`.)

For fixture-verify mode (CI / smoke):

```bash
go run examples/webhook-receivers/go/receiver.go --verify-fixture tests/fixtures/webhook-v1
# prints "OK: fixture verified" and exits 0 on success.
```

## Expected log output

On a verified delivery from cronduit:

```
[go-receiver] listening on http://127.0.0.1:9992/  (log: /tmp/cronduit-webhook-receiver-go.log)
[go-receiver] verified webhook-id=01HZAFY0V1F1BS1F2H8GV4XG3R bytes=312
```

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| 401 from receiver | `WEBHOOK_SECRET_FILE` content does not match cronduit's `webhook.secret` | Confirm both sides reference the same secret bytes (no trailing newline — see Pitfall 3 in `tests/fixtures/webhook-v1/README.md`) |
| 400 missing required headers | cronduit dispatcher dropped a header OR delivery is not a Standard Webhooks v1 delivery | Confirm cronduit version >= 1.2.0 and `webhook = { ... }` config has `secret` set (not `unsigned = true`) |
| 400 timestamp drift > 5min | Receiver clock is wrong OR cronduit clock is wrong | `ntpdate` / `chrony`; verify both sides via `date` |
| 503 server misconfigured | `WEBHOOK_SECRET_FILE` env var is unset | Set the env var before launch |
| `go vet` warnings | Go version too old (uses 1.21+ idioms) | Upgrade Go |

## SHA-256 only

Cronduit v1.2 ships SHA-256 only. No algorithm-agility (SHA-384/512/Ed25519)
— if your operator workflow requires those, file a v1.3 roadmap issue.

## See also

- [`docs/WEBHOOKS.md`](../../../docs/WEBHOOKS.md) — operator-facing hub doc
- [`examples/webhook-receivers/python/README.md`](../python/README.md) — Python variant (port 9991)
- [`examples/webhook-receivers/node/README.md`](../node/README.md) — Node variant (port 9993)
- [`examples/webhook_mock_server.rs`](../../webhook_mock_server.rs) — Phase 18 Rust loopback mock (always-200 — for header/payload inspection only)
