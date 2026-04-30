# Phase 19: Webhook HMAC Signing + Receiver Examples - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in `19-CONTEXT.md` — this log preserves the alternatives considered.

**Date:** 2026-04-29
**Phase:** 19-webhook-hmac-signing-receiver-examples
**Areas discussed:** Receiver form factor, Receiver layout & docs home, Receiver scope beyond signature verification, Interop CI verification

---

## Receiver form factor

| Option | Description | Selected |
|--------|-------------|----------|
| Runnable mini-servers (mirror Rust mock) | Each receiver is a self-contained HTTP server (stdlib only — no Flask/gin/Express) that listens on 127.0.0.1:PORT, verifies the signature with constant-time compare, logs the verdict, returns 200/401. Mirrors `examples/webhook_mock_server.rs`. Operator runs `python receiver.py` / `go run receiver.go` / `node receiver.js` and immediately validates against a real cronduit delivery. (Recommended) | ✓ |
| Snippet + verify function only | `verify_signature(secret, headers, body)` function with usage examples in a README. No HTTP server. Smaller surface, less language-specific code, but the operator can't run the example end-to-end out of the box. | |
| Both: snippet inside a runnable server | Each receiver file is a runnable mini-server, but the verify logic lives in a clearly-separated, copy-pasteable function (with docstring) at the top. Operators can run as-is OR copy the verify into their framework. | |

**User's choice:** Runnable mini-servers (mirror Rust mock)
**Notes:** Stdlib only confirmed (no third-party deps). Layout `examples/webhook-receivers/{python,go,node}/` selected via the question's preview. Captured in CONTEXT.md as **D-01..D-04**. Note that D-04 still pulls forward the "copy-pasteable verify function at the top of each file" idea from option 3 — operators get the runnable server AND the integration-friendly snippet without paying for a separate file.

---

## Receiver layout & docs home

| Option | Description | Selected |
|--------|-------------|----------|
| New docs/WEBHOOKS.md hub + per-receiver READMEs | Create `docs/WEBHOOKS.md` as the unified operator guide (wire format, header semantics, secret rotation guidance, SHA-256-only note, links to all 3 receiver examples). Each receiver dir gets a short README focused on that language's run-and-verify flow. `docs/CONFIG.md` webhook section gains a back-link. (Recommended — mirrors `docs/QUICKSTART.md`/`docs/CONFIG.md`/`docs/SPEC.md` hub pattern) | ✓ |
| Extend docs/CONFIG.md only | Expand the existing webhook section in `docs/CONFIG.md` with wire-format details, SHA-256 note, and receiver-example links. No new top-level doc. | |
| Per-receiver READMEs only + README.md section | No new doc; the project README gets a Webhooks section linking out to each receiver dir's README. | |

**User's choice:** New docs/WEBHOOKS.md hub + per-receiver READMEs
**Notes:** Captured in CONTEXT.md as **D-05..D-08**. The new doc sits alongside `docs/CONFIG.md`, `docs/QUICKSTART.md`, `docs/SPEC.md`. Each receiver dir gets its own focused README. CONFIG.md picks up a back-link only (no content duplication). README.md gets a one-line pointer.

---

## Receiver scope beyond signature verification

| Option | Description | Selected |
|--------|-------------|----------|
| Verify only (minimum WH-04) | Each receiver shows: parse 3 headers → compute HMAC → constant-time compare → 200 on match, 401 on mismatch. Anti-replay / idempotency / retry-friendly response codes are mentioned in `docs/WEBHOOKS.md` but NOT shipped as code. | |
| Verify + timestamp drift check + idempotency note | Verify + reject on >5 min drift (Standard Webhooks v1 anti-replay). Add a comment block explaining how to dedupe by `webhook-id`. | |
| Verify + drift + idempotency + retry-aware response codes (Recommended) | Everything in option 2 PLUS each receiver returns 4xx for permanent (signature mismatch → 401, malformed → 400) vs 5xx for transient (downstream DB unreachable → 503), so Phase 20's retry semantics work correctly. Idempotency is still a comment block. | ✓ |
| Verify + drift + working idempotency dedup | Everything in option 3 PLUS each receiver implements a working in-memory dedup set keyed by `webhook-id`. Closest to a real production receiver, but adds state management code that distracts from the HMAC focus. | |

**User's choice:** Verify + drift + idempotency + retry-aware response codes
**Notes:** Captured in CONTEXT.md as **D-09..D-12**. The 4xx-vs-5xx contract (D-12 table) is locked here AND seeds Phase 20 — Phase 20's retry implementation MUST honor the 4xx-permanent / 5xx-transient split. 5-minute drift window is hard-coded with a clearly-named constant (D-11). Idempotency stays a verbatim comment block across all 3 receivers (D-10) — adding TTL state would distract from the HMAC focus.

---

## Interop CI verification

| Option | Description | Selected |
|--------|-------------|----------|
| Per-language CI matrix (verify against shared fixture) — Recommended | Add a new `webhook-interop` GHA job (matrix: python-3.x, go-1.2x, node-20.x) that runs each receiver's verify function against a checked-in fixture. Cronduit's `sign_v1` also verifies against the same fixture in a Rust test. If anyone breaks the wire format, ALL languages fail in CI. | ✓ |
| Rust-side fixture only + maintainer UAT for receivers | Cronduit's `sign_v1` verifies against a checked-in fixture (locks the cronduit side forever). The 3 receivers are validated by the maintainer running `just uat-webhook-receiver-{lang}-verify-fixture` once during Phase 19 UAT — not in CI. | |
| Hand-validation only (Phase 18 pattern) | Mirror Phase 18: maintainer runs each receiver against a real cronduit delivery via a `just uat-webhook-receiver-{lang}` recipe and flips checkboxes in `19-HUMAN-UAT.md`. No checked-in fixture; no CI gate. | |

**User's choice:** Per-language CI matrix (verify against shared fixture)
**Notes:** Captured in CONTEXT.md as **D-13..D-17**. CI gate from day one (NOT warn-only — interop drift is more dangerous than dependency policy drift). Tamper variants are part of each verify recipe (D-17) so an always-true `verify_signature` cannot pass CI. Rust unit test re-derives the fixture signature from `sign_v1` so cronduit's signing side is also locked. Six new `uat-webhook-receiver-*` recipes (3 real-delivery + 3 fixture-verify; D-16 table). Per-receiver ports avoid collision with Phase 18's mock on 9999.

---

## Claude's Discretion

Areas where the user did not lock specifics — Claude has flexibility here during planning/implementation, captured in CONTEXT.md § Claude's Discretion (D-01..D-17 notes):

- Exact filenames inside each receiver dir (`receiver.py` vs `verify.py` vs `server.py` — language convention)
- Exact `just` recipe body shape for fixture-verify (CLI flag in receiver script vs sibling test harness file)
- Exact section ordering inside `docs/WEBHOOKS.md` beyond the 10 sections in D-06
- Whether the per-language CI matrix lives as a new top-level job in `ci.yml` or as a separate workflow file
- Exact ULID value in `webhook-id.txt` and exact timestamp in `webhook-timestamp.txt` (any stable past values; documented in fixture README)
- Exact secret string in `secret.txt` — must be obviously-test (e.g., `cronduit-test-fixture-secret-not-real`) with a comment header warning operators
- Whether to add 3 new `wh-example-receiver-{python,go,node}` example jobs (each pointed at a different receiver port) OR reuse Phase 18's existing `wh-example-signed` and override the URL via env-var per recipe — researcher decides simpler shape

---

## Deferred Ideas

Captured in CONTEXT.md § Deferred Ideas — items raised during discussion but NOT in Phase 19 scope:

- Working idempotency dedup in receivers (in-memory Set with TTL, DB unique constraint) — comment-block only in v1.2
- Configurable timestamp-drift window — hard-coded at 5 minutes (Standard Webhooks v1 default)
- Frameworks examples (Flask, gin, Express) — stdlib-only keeps the run-without-install promise
- Webhook UI on dashboard (delivery status, replay/retry button) — Phase 21 candidate or v1.3
- Algorithm-agility for HMAC (SHA-384/512/Ed25519) — locked OUT for v1.2
- Pluggable signature schemes (GitHub-style `x-hub-signature`) — Standard Webhooks v1 is the locked wire format
- Cronduit-side multi-secret rotation window — rotation is a receiver-side concern (dual-secret verify)
- More than 3 receiver languages (Ruby, Java, .NET, Rust client-side) — v1.3+ if community demand
- Per-receiver Docker images — v1.3 if multi-receiver fan-out staging becomes a need
- Standard Webhooks reference test vectors interop — opportunistic; researcher checks during plan phase
