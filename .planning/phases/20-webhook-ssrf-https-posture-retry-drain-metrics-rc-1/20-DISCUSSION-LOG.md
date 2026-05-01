# Phase 20: Webhook SSRF/HTTPS Posture + Retry/Drain + Metrics — rc.1 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-01
**Phase:** 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
**Areas discussed:** Retry mechanism shape, webhook_deliveries content, Drain semantics on SIGTERM, Retry classification edges

---

## Pre-discussion: Already-locked decisions (carried forward, not re-asked)

These were established by upstream artifacts (PROJECT.md, REQUIREMENTS.md, ROADMAP.md, prior CONTEXTs P15/P17/P18/P19) and surfaced to the user as "won't re-ask":

- Retry schedule: 3 attempts at `t=0/30s/300s` with `rand()*0.4+0.8` full-jitter (WH-05)
- HTTPS for non-loopback / non-RFC1918; HTTP only for `127/8`, `::1`, `10/8`, `172.16/12`, `192.168/16`, `fd00::/8` (WH-07)
- 30s `webhook_drain_grace` configurable on shutdown (WH-10)
- `cronduit_webhook_*` Prometheus family with closed-enum `status ∈ {success, failed, dropped}`, per-job histogram, queue-depth gauge (WH-11)
- 4xx-permanent / 5xx-transient classification (P19 D-12)
- `RetryingDispatcher` wraps `HttpDispatcher` via composition (P18 D-21)
- SSRF accepted-risk; THREAT_MODEL.md TM5 close-out belongs to Phase 24 (per ROADMAP)
- rc.1 cut follows existing `docs/release-rc.md` runbook from v1.1 P12 (no runbook changes)

User-facing question: "Which areas do you want to discuss for Phase 20?"
User selected: ALL FOUR areas (Retry mechanism shape, webhook_deliveries content, Drain semantics, Retry classification edges).

---

## Gray Area 1 — Retry mechanism shape

| Option | Description | Selected |
|--------|-------------|----------|
| In-memory async chain | RetryingDispatcher::deliver() awaits HttpDispatcher::deliver() and on transient failure does `tokio::time::sleep(jitter*30s).await` then retries. Each delivery's chain runs as a single tokio future scoped to the worker's cancel-token. ~100 LOC; chain lost on shutdown (drain handles graceful exit). | ✓ |
| DB-backed retry queue | First attempt synchronous; on transient failure INSERT into `webhook_deliveries` with `next_attempt_at`, `attempts`, payload bytes. Worker polls every 10s for due rows. Survives restart; adds polling cost + persisted-payload secret/PII surface; ~300 LOC. | |

**User's choice:** In-memory async chain (Recommended).
**Notes:** Locks `RetryingDispatcher` as a synchronous chain inside the worker task; `webhook_deliveries` table doesn't need scheduling state (`next_attempt_at`, payload bytes) — pure post-mortem audit. Mid-chain retries dropped on shutdown is acceptable given the drain budget. Implication: schema for Gray Area 2 collapses to DLQ-shape only.

---

## Gray Area 2 — `webhook_deliveries` table content

| Option | Description | Selected |
|--------|-------------|----------|
| DLQ-only | One row per delivery that exhausted all attempts. Schema: id, run_id, job_id, url, attempts, last_status, last_error, dlq_reason, first_attempt_at, last_attempt_at. NO payload bytes (avoids secret/PII on disk). Small table; pruner reuses log_retention. | ✓ |
| Per-attempt audit | One row per HTTP attempt (every retry). Schema adds attempt_number, status_code, error, attempted_at. Richer audit; larger table; questionable v1.2 value with no UI. | |

**User's choice:** DLQ-only (Recommended).
**Notes:** No payload bytes column locked — secret/PII hygiene. Closed-enum `dlq_reason` deferred to Gray Area 4 (since it depends on retry classification). The 4xx-permanent-also-DLQ question folded into Gray Area 4.

---

## Gray Area 3 — Drain semantics on SIGTERM

| Option | Description | Selected |
|--------|-------------|----------|
| Cancel sleeping retries; let in-flight HTTP finish | Cancel-token wakes the `tokio::select!` mid-sleep → chain breaks and writes a DLQ row with reason=`shutdown_drain`. HTTP request actively on the wire runs to completion (reqwest 10s cap). Queued events drained with first-attempt budget; remainder dropped at expiry. | ✓ |
| Hard-cancel everything at budget expiry | 30s budget for queue intake AND retry chains. At t=30s abort the worker task; in-flight HTTP cancelled mid-flight; sleeping retries dropped; queued events dropped. Simpler but contradicts success criterion 3 wording ("in-flight NOT cancelled"). | |

**User's choice:** Cancel sleeping retries; let in-flight HTTP finish (Recommended).
**Notes:** Locks the actual shutdown ceiling at `webhook_drain_grace + 10s` worst-case (drain budget + reqwest single-attempt timeout). Documented operator-facing in `docs/WEBHOOKS.md`. Mid-chain retries cancelled write `dlq_reason = "shutdown_drain"` (operator-queryable subset).

---

## Gray Area 4 — Retry classification edges

### Sub-question 4a: 429 / Retry-After handling

| Option | Description | Selected |
|--------|-------------|----------|
| Strict: 4xx=permanent, locked schedule | All 4xx (incl. 429) = permanent per P19 D-12. Locked t=0/30s/300s sacred. Retry-After ignored. Receivers wanting backoff use 5xx. Simplest. | |
| Refined: 408+429 transient; honor Retry-After cap | 408 + 429 reclassified as transient. Retry-After delta-seconds honored as `delay = max(locked_schedule[i], retry_after)` capped at next-attempt's worst-case. More HTTP-correct; adds parsing surface. | ✓ |
| Refined: 408+429 transient; ignore Retry-After | 408/429 transient (use locked schedule); Retry-After ignored (operator predictability). Compromise. | |

**User's choice:** Refined: 408+429 transient; honor Retry-After cap.
**Notes:** Retry-After delta-seconds form only (HTTP-date form NOT supported in v1.2; falls back to schedule + WARN log). Cap math: `delay = max(locked_schedule[next_attempt], retry_after_seconds)`, then `cap = locked_schedule[next_attempt+1] * 1.2`. For attempt 2: cap = 360s. For attempt 3: same cap (no slot 4).

### Sub-question 4b: 4xx-permanent → DLQ row?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — every non-2xx writes DLQ | DLQ closed enum: `http_4xx | http_5xx | network | timeout | shutdown_drain`. attempts column distinguishes (1 for permanent-fail, 3 for exhausted-transient, ≤3 for shutdown_drain). Operators SELECT all failed deliveries via single table. | ✓ |
| No — 4xx counts as `failed` metric only | DLQ table reserved for transient-exhausted (3 attempts hit). 4xx-permanent increments `failed` counter and logs WARN; no DLQ row. Smaller table; logs grep needed for permanent-fail history. | |

**User's choice:** Yes — every non-2xx writes a DLQ row (Recommended).
**Notes:** Locks the `dlq_reason` closed enum at 5 values; `attempts` column gives the DLQ-vs-permanent-fail distinction. Single SELECT covers operator's "what failed" question across all reasons.

---

## Claude's Discretion

User did not say "you decide" explicitly on any decision; the gray areas were all answered. Items left to researcher/planner judgment (documented in CONTEXT.md "Claude's Discretion" subsection):

- Migration filename + timestamp prefix (next free slot after `20260429_000007`)
- Internal struct names (planner picks; `RetryingDispatcher` is the public surface)
- Validator function name (`check_webhook_url_https_required` vs extending `check_webhook_url`)
- IPv6 ULA classification: `fc00::/7` (RFC-correct) vs `fd00::/8` (success-criterion-literal). Default narrower per WH-07; researcher may broaden with rationale.
- Histogram bucket choice for `cronduit_webhook_delivery_duration_seconds` (default `metrics` crate buckets)
- Retry-After cap math implementation (`next_attempt+1` worst-case vs hard ceiling `1.2× max(schedule)` — both operator-predictable)
- Whether to ship a new `src/webhooks/retry.rs` or extend `src/webhooks/dispatcher.rs` (currently 535 lines)
- Whether worker's `tokio::select!` extends in-place or transitions to a drain-only sub-loop on cancel-fire
- Final naming of the `uat-webhook-*` recipes (suggested floor in CONTEXT.md)
- Final naming of `tests/v12_webhook_*` integration tests (suggested floor in CONTEXT.md)

## Deferred Ideas

Captured in CONTEXT.md `<deferred>` section. Highlights:

- Webhook delivery UI / DLQ inspector panel — v1.3 candidate
- Durable webhook queue (restart-survives) — v1.3 candidate
- DB-backed retry queue (Gray Area 1 alternative) — explicitly rejected; revisit only on operator demand
- Per-attempt audit log (Gray Area 2 alternative) — explicitly rejected; revisit on operator demand
- HTTP-date form of `Retry-After` — falls back to schedule + WARN; add if a real receiver demands it
- Per-job `webhook.attempt_timeout` — P18 D-18 hard-coded 10s; no demand surfaced
- Concurrent delivery semaphore — P18 D-19 serial-within-worker stays
- SSRF allow/block-list filter (WH-08 accepted-risk) — v1.3 candidate
- Webhook URL DNS resolution at LOAD time — D-20 "no DNS at load"
- Per-job metric labels with reason granularity — closed enum on counter; reason in `dlq_reason` SQL column
- THREAT_MODEL.md TM5 full close-out — Phase 24 owns; P20 ships doc-pointer stub
- `release.yml` / `cliff.toml` / `release-rc.md` modifications for v1.2 — runbook reused verbatim
- `webhook_deliveries` retention beyond `log_retention` — D-14 reuses 90-day knob
- Renaming `cronduit_webhook_delivery_dropped_total` — kept as-is to preserve operator dashboards
