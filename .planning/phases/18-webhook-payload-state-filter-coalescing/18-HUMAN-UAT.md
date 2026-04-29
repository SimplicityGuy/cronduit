# Phase 18 Human UAT — Webhook Payload + State-Filter + Coalescing

> **Maintainer-validated only.** Per project memory `feedback_uat_user_validates.md`, Claude does NOT mark these scenarios passed — the maintainer runs each scenario and flips the `[ ]` to `[x]` themselves. Per `feedback_uat_use_just_commands.md`, every step references a `just` recipe — NEVER raw `curl`/`cargo`/`docker`.

## Prerequisites

| Prereq | Recipe | Notes |
|--------|--------|-------|
| Workspace builds clean | `just ci` | Full CI gate: fmt + clippy + openssl-check + nextest + schema-diff + image |
| rustls invariant holds | `just openssl-check` | `cargo tree -i openssl-sys` returns empty across native + arm64-musl + amd64-musl |
| Example config validates | `WEBHOOK_SECRET=test-secret-shh just check-config examples/cronduit.toml` | Set the env var in your shell before running `just dev` |

## Scenarios

### Scenario 1 — Signed delivery with default state filter

**What this proves:** WH-01 (config), WH-03 (3 headers + signed signature), WH-09 (16-field payload).

Steps:
1. In terminal A: `just uat-webhook-mock` (starts mock receiver on 127.0.0.1:9999)
2. In terminal B: `export WEBHOOK_SECRET=my-test-secret-shh`
3. In terminal B: `just dev` (cronduit runs against examples/cronduit.toml; the `wh-example-signed` job fires every minute and fails)
4. In terminal C: `just uat-webhook-fire wh-example-signed` (force an immediate run via the api-run-now helper)
5. In terminal C: `just uat-webhook-verify`

Expected output (terminal C):
- The log lines show `POST /signed HTTP/1.1`
- Headers include: `content-type: application/json`, `webhook-id: <26-char ULID>`, `webhook-timestamp: <10-digit Unix seconds>`, `webhook-signature: v1,<base64-with-padding-and-+-or-/>`
- The body (after the empty line in the log) is compact JSON containing all 16 fields: `payload_version`, `event_type`, `run_id`, `job_id`, `job_name`, `status`, `exit_code`, `started_at` (ending in `Z`), `finished_at`, `duration_ms`, `streak_position` (== 1 on the first delivery), `consecutive_failures` (== 1+), `image_digest` (`null` for command jobs), `config_hash`, `tags` (`[]`), `cronduit_version`

[ ] Maintainer-validated

### Scenario 2 — Unsigned delivery omits webhook-signature header

**What this proves:** WH-03 / D-05 — `unsigned = true` omits the signature header (cronduit extension to Standard Webhooks v1).

Steps:
1. With `just uat-webhook-mock` and `just dev` still running (from Scenario 1)…
2. `just uat-webhook-fire wh-example-unsigned`
3. `just uat-webhook-verify`

Expected: the `/unsigned` POST has `webhook-id` AND `webhook-timestamp` headers but NO `webhook-signature` header.

[ ] Maintainer-validated

### Scenario 3 — Default coalescing (fire_every = 1)

**What this proves:** WH-06 / D-12 / D-15 / D-16 — first-of-streak default behavior.

Steps:
1. With `just uat-webhook-mock` running, restart cronduit fresh (Ctrl-C terminal B, then `just dev` again)
2. `just uat-webhook-fire wh-example-signed` three times in a row (wait ~5 seconds between each)
3. `just uat-webhook-verify`

Expected: ONE `/signed` delivery (from the first fire), then NO further deliveries from `wh-example-signed` because the streak-position stays > 1 (until a `success` resets it). Compare to scenario 5 below.

[ ] Maintainer-validated

### Scenario 4 — State filter excludes success

**What this proves:** WH-01 / D-04 — `states = ["failed", "timeout"]` (default) excludes successful runs.

Steps:
1. Edit `examples/cronduit.toml` and add a passing job `wh-example-passing` with `command = "true"` and the SAME webhook URL (`http://127.0.0.1:9999/signed`)
2. Run `WEBHOOK_SECRET=test-secret-shh just check-config examples/cronduit.toml` — should pass
3. Run `just dev` for 5 minutes
4. `just uat-webhook-fire wh-example-passing`
5. `just uat-webhook-verify`

Expected: NO deliveries triggered by the `wh-example-passing` job (success is not in the default `["failed", "timeout"]` filter).

[ ] Maintainer-validated

### Scenario 5 — `fire_every = 0` legacy mode

**What this proves:** WH-06 / D-16 — `fire_every = 0` always fires.

Steps:
1. With `just uat-webhook-mock` and `just dev` still running…
2. `just uat-webhook-fire wh-example-fire-every-zero` three times in a row
3. `just uat-webhook-verify`

Expected: 3 deliveries to `/timeouts-only` IF any timeout occurred OR — if `wh-example-fire-every-zero`'s schedule produces failures every 3 min and you map a `command` that fails as timeout via a long-running false… (NOTE: the example config uses `command = "false"` which exits failed, NOT timeout. To exercise this scenario, the maintainer should temporarily change the example to `command = "sleep 3600"` and add `timeout = "5s"`.)

[ ] Maintainer-validated

### Scenario 6 — `${WEBHOOK_SECRET}` env-var interpolation

**What this proves:** WH-01 / D-03 — secret interpolation flows through to HMAC sign-site (Pitfall H also: empty secret rejected at LOAD).

Steps:
1. `unset WEBHOOK_SECRET`
2. Run `just check-config examples/cronduit.toml`
3. Verify it FAILS with a `MissingVar` error referencing `WEBHOOK_SECRET`
4. `export WEBHOOK_SECRET=""`
5. Run `just check-config examples/cronduit.toml`
6. Verify it FAILS with `webhook.secret resolved to an empty string` (Pitfall H — Plan 02 validator)
7. `export WEBHOOK_SECRET=actual-secret`
8. Run `just check-config examples/cronduit.toml`
9. Verify it PASSES

[ ] Maintainer-validated

### Scenario 7 — Metrics families

**What this proves:** Phase 15/18 telemetry — three webhook counters described from boot.

Steps:
1. With `just dev` running…
2. Run `just metrics-check`

Expected: stdout/grep includes:
- `# HELP cronduit_webhook_delivery_dropped_total ...`
- `# HELP cronduit_webhook_delivery_sent_total ...`
- `# HELP cronduit_webhook_delivery_failed_total ...`
- `# TYPE cronduit_webhook_delivery_<each>_total counter`
- All three counters present (with at least the zero-baseline value 0; sent/failed may be > 0 if scenarios above already fired)

[ ] Maintainer-validated

## Sign-off

All 7 scenarios above are maintainer-validated [ ] (flip to [x] when complete).

Cross-reference: every scenario above has an automated regression test in `tests/v12_webhook_*.rs` (Plans 03 + 05). The UAT scenarios re-prove the same behaviors against a real receiver — operator-side TLS, real network latency, real ${ENV_VAR} substitution.
