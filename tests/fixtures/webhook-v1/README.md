# Phase 19 Webhook Interop Fixture

Locked Standard Webhooks v1 wire-format test vector for cross-runtime interop.

Consumed by:
- `src/webhooks/dispatcher.rs::tests::sign_v1_locks_interop_fixture` (Rust)
- `examples/webhook-receivers/python/receiver.py --verify-fixture` (Plan 02)
- `examples/webhook-receivers/go/receiver.go --verify-fixture` (Plan 03)
- `examples/webhook-receivers/node/receiver.js --verify-fixture` (Plan 04)
- `just uat-webhook-receiver-{python,go,node}-verify-fixture` (CI gate)

## Files

| File | Purpose | Bytes |
|---|---|---|
| `secret.txt` | HMAC key (TEST VALUE — NEVER reuse in production) | 37 |
| `webhook-id.txt` | 26-char ULID for the canonical event | 26 |
| `webhook-timestamp.txt` | Unix-epoch seconds (2025-01-01T00:00:00Z) | 10 |
| `payload.json` | Compact JSON of `WebhookPayload::build` for the canonical event | variable |
| `expected-signature.txt` | `v1,<base64>` from `sign_v1` over the four inputs above | 47 |

## CRITICAL: No trailing newline

Every file in this directory MUST have NO trailing newline. Most shell idioms (`echo`, `cat <<EOF`, vim with `endofline`) add `\n`. Use `printf '%s' '<value>' > file.txt` to write without one. The `.gitattributes` file in this directory disables EOL normalization (`* -text`) so future commits cannot silently add newlines.

A trailing newline in `secret.txt` corrupts the HMAC: the sign-side reads `secret-not-real` (37 bytes), the receiver reads `secret-not-real\n` (38 bytes), and HMAC outputs diverge.

## Regenerating the fixture

The fixture is regenerated only when the wire format intentionally changes (e.g., a future `payload_version: "v2"` bump). To regenerate:

1. Update `WebhookPayload` if needed in `src/webhooks/payload.rs`.
2. Run the printer test: `cargo test -p cronduit --lib webhooks::dispatcher::tests::print_canonical_payload_bytes -- --nocapture --ignored`.
3. Capture the printed `payload.json` bytes and `expected-signature.txt` value, write them to disk with `printf '%s'` (no trailing newline).
4. Run `cargo nextest run -p cronduit -- webhooks::dispatcher::tests::sign_v1_locks_interop_fixture` and confirm green.
5. Commit the regenerated files with a clear "wire format v2 bump" message.

## Provenance

Canonical event: `RunFinalized { run_id: 42, job_id: 7, job_name: "backup-nightly", status: "failed", exit_code: Some(1), started_at: 2025-01-01T00:00:00Z, finished_at: 2025-01-01T00:00:01Z }` with `tags = []`, `image_digest = None`, `config_hash = None`, `filter_position = 1`, `cronduit_version = "1.2.0"`.
