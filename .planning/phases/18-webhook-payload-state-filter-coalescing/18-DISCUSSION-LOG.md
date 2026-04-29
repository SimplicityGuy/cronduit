# Phase 18: Webhook Payload + State-Filter + Coalescing - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-29
**Phase:** 18-webhook-payload-state-filter-coalescing
**Areas discussed:** HMAC secret source, Payload field formats, State-filter + coalescing edge cases, HttpDispatcher posture

---

## Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| HMAC secret source | Per-webhook in TOML / single global / per-webhook+defaults override | ✓ |
| Payload field formats + tags/empty handling | Timestamps, streak_position type, image_digest on non-docker, tags pre-Phase-22, cronduit_version source | ✓ |
| State-filter + coalescing edge cases | Allowed values, empty-array semantics, neutral-status streak behavior, filter × streak interaction | ✓ |
| HttpDispatcher posture for Phase 18 | Single attempt vs retry, timeout, concurrency, failure handling | ✓ |

**User's choice:** All four.

---

## Area 1: HMAC Secret Source

### Where the signing key lives

| Option | Description | Selected |
|--------|-------------|----------|
| Per-webhook + defaults override | `webhook = { url, states, secret }` per-job AND in `[defaults]` with `use_defaults = false`; `${ENV_VAR}` interpolation; SecretString wrap. Mirrors LBL pattern. | ✓ |
| Single global secret | One `CRONDUIT_WEBHOOK_SECRET` env var (or `[server].webhook_secret`) signs everything | |
| Per-webhook only (no defaults) | `webhook.secret` is per-job mandatory; no `[defaults].webhook.secret`. Forces explicit per-job. | |

**User's choice:** Per-webhook + defaults override.
**Notes:** Locks D-01..D-03 in CONTEXT.md. Each receiver gets its own key; rotation per-receiver; matches existing config patterns exactly.

### Missing-secret behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Reject at config-LOAD | Validator hard-errors if `webhook` block lacks `secret` | |
| Allow unsigned with `unsigned = true` opt-in | Default: `secret` required. Operator sets `unsigned = true` for receivers like Slack/Discord | ✓ |
| Allow unsigned silently if secret omitted | Missing `secret` = unsigned delivery + startup WARN | |

**User's choice:** Allow unsigned with `unsigned = true` opt-in.
**Notes:** Locks D-04 (validator: `secret` xor `unsigned = true`) and D-05 (omit `webhook-signature` header on unsigned). Spec-compliant by default; explicit opt-out for receivers that don't verify.

---

## Area 2: Payload Field Formats

### Approve all 5 defaults?

| Option | Description | Selected |
|--------|-------------|----------|
| Approve all 5 | Lock the timestamps/streak_position/image_digest/tags/cronduit_version table as recommended | ✓ |
| Revisit timestamps | Discuss RFC3339 vs Unix epoch | |
| Revisit streak_position type | Discuss integer vs string enum | |
| Revisit null-vs-omit policy | Discuss whether absent fields should be `null` or omitted | |

**User's choice:** Approve all 5.
**Notes:** Locks D-06..D-08 in CONTEXT.md. RFC3339 timestamps; integer `streak_position`; `null` for missing fields (schema stability); `[]` for `tags` until Phase 22; `env!("CARGO_PKG_VERSION")` for `cronduit_version`.

---

## Area 3: State-Filter + Coalescing Edge Cases

### Approve top 5 + discuss the tricky one?

| Option | Description | Selected |
|--------|-------------|----------|
| Approve top 5 + discuss filter × streak interaction | Lock allowed values / empty-array / default / streak-counting / neutral-status; unpack the filter × streak interaction separately | ✓ |
| Approve all 6 | Take the strict-literal reading of the last row | |
| Revisit streak-break behavior | Discuss whether `cancelled`/`stopped` should reset the streak | |
| Revisit `states` default | Discuss whether `["failed", "timeout"]` is right | |

**User's choice:** Approve top 5 + discuss filter × streak interaction.
**Notes:** Locks D-09..D-11 + D-14 partial. The 6th sub-decision moved to its own follow-up below.

### Filter × streak interaction follow-up

Setup: `failed → timeout`, `states = ["timeout"]`, default `fire_every = 1`. Should the timeout fire?

| Option | Description | Selected |
|--------|-------------|----------|
| Coalesce within filter-matching stream | Filter defines the stream; coalescing applies to it. Timeout IS streak_position 1 in the filter stream, fires. Payload's `streak_position` reflects filter-stream position; `consecutive_failures` stays unified P16 count. Needs Rust-side wrapper. | ✓ |
| Strict literal: filter ∧ unified streak_position == 1 | Use P16's `streak_position` as-is. Surprising for narrow filters. | |
| Per-state streaks (extend P16) | Each state has its own streak. Most precise; biggest scope creep into Phase 16. | |

**User's choice:** Coalesce within filter-matching stream.
**Notes:** Locks D-12..D-16 in CONTEXT.md. The dispatcher computes filter-matching position Rust-side; P16's `get_failure_context` stays untouched.

---

## Area 4: HttpDispatcher Posture

### Approve all 7 defaults?

| Option | Description | Selected |
|--------|-------------|----------|
| Approve all 7 | Single attempt; 10s timeout; serial within worker task; reqwest with rustls-tls; Phase 20 retry hook via wrapper trait; compact JSON; HMAC over raw body bytes before headers | ✓ |
| Revisit per-request timeout | Discuss 10s vs operator-configurable | |
| Revisit concurrency model | Discuss serial vs N-concurrent (semaphore) | |
| Revisit failure handling | Discuss whether to write to `webhook_deliveries` immediately (Phase 20 scope creep) | |

**User's choice:** Approve all 7.
**Notes:** Locks D-17..D-22 in CONTEXT.md. Composability via `RetryingDispatcher` wrapper for Phase 20 keeps the trait clean.

---

## Claude's Discretion

- ULID vs UUID v7 for `webhook-id` (both spec-acceptable; researcher confirms crate availability)
- Internal struct shapes, function signatures, test file names (follow existing patterns)
- Exact metric names beyond `cronduit_webhook_delivery_failed_total` (follow `cronduit_*` family conventions)
- Validator error wording (follow Phase 17 LBL precedent)
- Migration filenames if any are needed (none expected — `webhook_deliveries` is Phase 20)

## Deferred Ideas

- Multi-webhook per job (one URL for failures, one for successes, fan-out) — future
- Webhook UI (status, last-delivery, replay button) — Phase 21 candidate
- SSRF allow/blocklist URL filter — explicit accepted-risk per WH-08; v1.3
- HMAC algorithm-agility (SHA-384/512/Ed25519) — v1.3+ if receiver ecosystem demands
- Pluggable signature schemes (e.g., GitHub-style `x-hub-signature`) — out of scope, Standard Webhooks v1 locked
- Per-attempt timeout configurability — Phase 20 if needed
- Concurrent delivery semaphore — v1.3+ if load demands
- `webhook_deliveries` dead-letter table — Phase 20 (WH-05)
