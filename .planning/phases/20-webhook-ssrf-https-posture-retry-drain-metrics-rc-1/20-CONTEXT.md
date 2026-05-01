# Phase 20: Webhook SSRF/HTTPS Posture + Retry/Drain + Metrics — rc.1 - Context

**Gathered:** 2026-05-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Lock the operational webhook posture on top of Phase 18's `HttpDispatcher` (signing, payload, coalescing) and Phase 19's receiver-interop fixture: HTTPS-required URL validation for non-loopback / non-RFC1918 destinations, an in-memory 3-attempt retry chain with full-jitter backoff, a DLQ-only `webhook_deliveries` audit table, a 30-second graceful drain on shutdown, and the full `cronduit_webhook_*` Prometheus metric family. Compose `RetryingDispatcher` over `HttpDispatcher` via the wrapper pattern locked in P18 D-21 — no trait expansion. The phase ends with the `v1.2.0-rc.1` tag cut following the existing `docs/release-rc.md` runbook (reused from v1.1 P12).

**In scope (Phase 20):**
- LOAD-time URL validator: `webhook.url` scheme MUST be `https://` for non-loopback / non-RFC1918 hosts; `http://` allowed only for `127.0.0.0/8`, `::1`, `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `fd00::/8`. Hostname-as-host (e.g., `localhost`, public DNS names) classified textually at LOAD; no DNS resolution at validate-time. (WH-07)
- `RetryingDispatcher<HttpDispatcher>` newtype wrapper implementing `WebhookDispatcher` — wraps the existing dispatcher trait; the worker stays oblivious to retry semantics (P18 D-21 composition). 3 attempts at `t=0`, `t=30s`, `t=300s` with each delay multiplied by `rand()*0.4 + 0.8` full-jitter. (WH-05)
- Retry classification: 4xx = permanent (drop after attempt 1); 5xx + network + reqwest-timeout = transient (retry); 408 + 429 reclassified as transient (despite being 4xx); `Retry-After` header on 429/503 honored as `delay = max(locked_schedule[i], retry_after_seconds)` capped at the next-attempt's worst-case (`schedule[i+1] * 1.2`).
- DLQ-only `webhook_deliveries` table — one row per delivery that failed to reach 2xx, regardless of attempt count. Closed-enum `dlq_reason ∈ {http_4xx, http_5xx, network, timeout, shutdown_drain}`. NO payload bytes on disk (secret/PII hygiene); receivers re-derive payload from `run_id` if needed. One-file additive migration per backend (sqlite + postgres parity). (WH-05)
- 30-second `webhook_drain_grace` on SIGTERM (configurable in `[server]` block alongside `shutdown_grace`). Worker drains queued events at full speed within the budget; sleeping retries cancel via cancel-token-aware `tokio::select!` and write a `shutdown_drain` DLQ row; in-flight HTTP requests are NOT cancelled (reqwest's 10s per-attempt timeout caps them). At budget expiry: remaining queued events dropped + `cronduit_webhook_deliveries_total{status="dropped"}` increment per event. (WH-10)
- `cronduit_webhook_*` Prometheus metric family eagerly described at boot (extending the P15/P18 partial family):
  - `cronduit_webhook_deliveries_total{job, status}` where `status ∈ {success, failed, dropped}` (closed enum) — replaces the P18 `_sent_total` / `_failed_total` flat counters with a labeled family; preserves the P15 `_dropped_total` (queue-saturation drops, distinct from delivery-pipeline drops).
  - `cronduit_webhook_delivery_duration_seconds{job}` — histogram of single HTTP attempt duration (NOT the whole retry chain wall time).
  - `cronduit_webhook_queue_depth` — gauge sampled by the worker on each `rx.recv()` boundary.
  - All eagerly described + zero-baselined at boot per existing `src/telemetry.rs` discipline (P15/P18 precedent). (WH-11)
- `THREAT_MODEL.md` Threat Model 5 (Webhook Outbound) — surfaced WORDS-ONLY in this phase; full close-out belongs to Phase 24 (per ROADMAP). Phase 20 ships a stub note in code/docs as needed; the canonical TM5 entry lands in P24's milestone close-out wave.
- `docs/WEBHOOKS.md` operator-doc extension: new sections covering retry schedule, DLQ table semantics + example SQL, drain-on-shutdown behavior, HTTPS/SSRF posture (locked allowlist + accepted-risk note for hostnames). Operator-actionable, not a paraphrase of the spec.
- `v1.2.0-rc.1` tag cut at phase end via existing `docs/release-rc.md` runbook (no runbook changes; the maintainer flow is identical to v1.1.0-rc.1). Pre-flight checklist gated by full v1.2 webhook block (Phases 15→20) merged to `main` + green CI + green compose-smoke.

**Out of scope (deferred to other phases):**
- THREAT_MODEL.md TM5 (Webhook Outbound) and TM6 (Operator-supplied Docker labels) — Phase 24 milestone close-out (per ROADMAP).
- DB-backed retry queue with `next_attempt_at` polling — explicit alternative considered and rejected; in-memory chain locked. Retries lost on restart are accepted (drain handles graceful exit).
- Per-attempt audit log (one row per retry attempt) — explicit alternative considered and rejected; DLQ-only table locked.
- Webhook UI (delivery status, replay/retry button on dashboard) — Phase 21 owns FCTX UI but webhooks deferred to v1.3.
- SSRF allow/block-list filter beyond the loopback+RFC1918 classification — explicit accepted-risk per WH-08; deferred to v1.3.
- Per-job webhook-delivery metric labels beyond `{job, status}` — closed enum only; reason granularity (4xx vs 5xx vs network) lives in the DLQ `dlq_reason` column, NOT in metric labels.
- Per-attempt timeout configurability — Phase 18 D-18 hard-coded 10s remains; no operator demand surfaced to date.
- Concurrent delivery semaphore (parallel HTTP within the worker task) — Phase 18 D-19 serial-within-worker remains; deferred to v1.3+ if load demands change.
- Cronduit-side multi-secret rotation — Phase 19 already documented this as receiver-side (dual-secret verify); not a Phase 20 surface.
- Webhook persistence across restart (durable queue) — Punted to v1.3 candidate list per PROJECT.md § Future Requirements.
- Algorithm-agility for HMAC — Phase 19 locked SHA-256-only.
- DLQ inspector UI / dashboard panel — `webhook_deliveries` is queryable via SQL only in v1.2; UI deferred to v1.3.

</domain>

<decisions>
## Implementation Decisions

### Retry Mechanism (Gray Area 1; WH-05)
- **D-01:** **In-memory async retry chain.** `RetryingDispatcher::deliver()` awaits `HttpDispatcher::deliver()` synchronously; on transient failure, calls `tokio::time::sleep(jittered_delay).await` inside a `tokio::select!` with the worker's cancel token, then retries. Each delivery's full chain runs as a single tokio future scoped to the worker task. NO `tokio::spawn` per delivery — preserves the P18 D-19 serial-within-worker invariant; concurrent retries deferred to v1.3+.
- **D-02:** Schedule + jitter literal: `delays = [0, 30s, 300s]`; before each post-attempt-1 sleep, multiply the base delay by `rand::random::<f64>() * 0.4 + 0.8` (uniform in `[0.8, 1.2)`). Use `rand 0.9` (already in tree per v1.1 hygiene bump).
- **D-03:** Mid-chain shutdown semantics: every retry sleep wraps in `tokio::select! { _ = sleep(d) => continue, _ = cancel.cancelled() => break }`. On cancel-break, the chain writes a DLQ row with `dlq_reason = "shutdown_drain"` and the actual `attempts` count (not always 3) before returning `Err`. The worker_loop then continues draining the next event until the 30s budget expires.
- **D-04:** **NO retry chain survives restart.** A delivery interrupted mid-chain by SIGTERM is lost (DLQ-recorded as `shutdown_drain`). Operators are expected to design idempotent receivers + accept best-effort delivery for v1.2; durable queue is a v1.3 candidate per PROJECT.md.
- **D-05:** `RetryingDispatcher` newtype: `pub struct RetryingDispatcher<D: WebhookDispatcher> { inner: D, pool: DbPool, cancel: CancellationToken }`. Owns its own pool clone for DLQ row writes; shares the worker's cancel token by clone. Implements `WebhookDispatcher` via composition (P18 D-21).

### Retry Classification & 429/Retry-After (Gray Area 4; WH-05)
- **D-06:** Classification map (carries forward P19 D-12 retry-respect contract; refined for 408/429):
  | HTTP / outcome | Classification | Behavior |
  |---|---|---|
  | 200..=299 | success | counter++, no retry |
  | 408 (Request Timeout) | transient | retry per schedule |
  | 429 (Too Many Requests) | transient | retry per schedule, honor `Retry-After` |
  | 4xx (other) | permanent | DLQ row `http_4xx`, no retry |
  | 5xx | transient | retry per schedule, honor `Retry-After` (rare) |
  | reqwest network error | transient | retry per schedule |
  | reqwest timeout (10s) | transient | retry per schedule |
- **D-07:** `Retry-After` header (HTTP RFC 7231 §7.1.3) is honored on 429 + 5xx responses. Parse as integer-seconds (delta-seconds form); HTTP-date form is NOT supported in v1.2 (operator predictability over correctness — log WARN if HTTP-date encountered, fall back to schedule).
- **D-08:** `Retry-After` interaction with locked schedule: `delay = max(locked_schedule[next_attempt], retry_after_seconds)`, then capped at the worst-case of the next-attempt's locked window — `cap = locked_schedule[next_attempt+1] * 1.2` to keep `Retry-After` from blowing past Phase 20's predictable budget. This is operator-visible behavior; document in `docs/WEBHOOKS.md`.
- **D-09:** Receivers wanting more aggressive backoff than the locked schedule should return 5xx+`Retry-After` (transient + delay hint); receivers explicitly rejecting a request should return 400-or-equivalent (permanent). Documented in `docs/WEBHOOKS.md` retry-respect contract section (extends Phase 19 D-12 table).

### `webhook_deliveries` DLQ Table (Gray Area 2; WH-05)
- **D-10:** **DLQ-only, no payload bytes.** Table records every delivery that failed to reach 2xx (regardless of attempt count); first attempt success → no row. Schema (sqlite shown; postgres mirrors with type adjustments):
  ```sql
  CREATE TABLE webhook_deliveries (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id           INTEGER NOT NULL,
    job_id           INTEGER NOT NULL,
    url              TEXT    NOT NULL,    -- as-configured (after env-var interp)
    attempts         INTEGER NOT NULL,    -- 1..=3 (1 if 4xx-permanent, 3 if exhausted-transient, ≤3 if shutdown_drain)
    last_status      INTEGER,             -- HTTP status code if any (NULL on network/timeout/shutdown)
    last_error       TEXT,                -- truncated reqwest error message (≤500 chars), NULL on http_4xx/5xx
    dlq_reason       TEXT    NOT NULL,    -- closed enum: http_4xx | http_5xx | network | timeout | shutdown_drain
    first_attempt_at TEXT    NOT NULL,    -- RFC3339 (project convention)
    last_attempt_at  TEXT    NOT NULL,    -- RFC3339
    FOREIGN KEY (run_id) REFERENCES job_runs(id),
    FOREIGN KEY (job_id) REFERENCES jobs(id)
  );
  CREATE INDEX idx_webhook_deliveries_last_attempt ON webhook_deliveries (last_attempt_at);
  ```
- **D-11:** Index strategy: single index on `last_attempt_at` (the dominant operator query — "what failed in the last hour"). NO index on `(job_id, last_attempt_at)` for v1.2 — most homelabs have <100 jobs and `webhook_deliveries` is bounded by failure rate; a sequential scan over a small table is cheaper than maintaining a second index. Researcher confirms via PITFALLS pass.
- **D-12:** **No payload column, no header column, no signature column.** Operator-supplied secrets in URLs (e.g., `https://webhook.example.com/?token=ABC` is not allowed by Standard Webhooks but operators do it) stay in `url` as-configured (already accepted risk; same as logs). NO body bytes, NO HMAC signature on disk. Audit story is "what failed when, how"; replay story is deferred to v1.3.
- **D-13:** Migration shape: ONE additive file per backend (`migrations/sqlite/2026XXXX_NNNNNN_webhook_deliveries_add.up.sql` + postgres mirror). No backfill (table starts empty). Tested at LOAD time on `cronduit run` first start post-merge (the existing migration runner picks it up automatically).
- **D-14:** Pruner integration: extend the existing daily retention pruner (`src/scheduler/retention.rs`) to also DELETE from `webhook_deliveries WHERE last_attempt_at < now() - log_retention` (reuses the v1.0 `log_retention = "90d"` knob; no new config). Researcher confirms pruner shape via P6 retention precedent.

### Drain Semantics on SIGTERM (Gray Area 3; WH-10)
- **D-15:** **Soft-cancel + active-drain.** On `cancel.cancelled()`:
  1. Worker continues draining `rx.recv()` for up to `webhook_drain_grace` (30s default).
  2. Each pulled event runs through the FULL `RetryingDispatcher::deliver()`, but every retry sleep is cancel-aware (D-03) — so any new retries scheduled mid-chain bail out immediately and DLQ as `shutdown_drain`.
  3. In-flight HTTP requests on the wire run to completion (capped by reqwest's existing 10s per-attempt timeout — P18 D-18). NOT cancelled (per success criterion 3 wording).
  4. At budget expiry: any events still in the channel are drained-and-dropped one by one with `cronduit_webhook_deliveries_total{status="dropped"}` increment per event + WARN log per event; channel is closed; worker_loop exits.
- **D-16:** Configuration knob: `webhook_drain_grace` lives in the existing `[server]` block alongside `shutdown_grace`, parsed via `humantime_serde`. Default `"30s"`. Documented in `docs/CONFIG.md` § Server.
- **D-17:** Interaction with `shutdown_grace` (the scheduler's existing wait-for-running-jobs budget): the two are independent. `shutdown_grace` covers in-flight job execution (Phase 1/2 invariant); `webhook_drain_grace` covers the webhook worker's queue drain AFTER the scheduler finishes. The bin layer's existing `let _ = scheduler_handle.await;` followed by `let _ = webhook_worker_handle.await;` ordering preserves this — webhook worker doesn't start draining until the scheduler is done. NO budget overlap.
- **D-18:** **In-flight HTTP attempt is allowed to outlast the drain budget** (capped by reqwest's 10s timeout). Worst case: SIGTERM fires at t=0; an HTTP request was sent at t=29.9s; the request can run until t=39.9s (10s reqwest cap). Documented in `docs/WEBHOOKS.md` so operators understand the actual shutdown ceiling = `webhook_drain_grace + 10s` worst-case (rare).

### URL Validation (WH-07)
- **D-19:** LOAD-time validator extends the existing P18 `check_webhook_url` (`src/config/validate.rs:385`). After parse + scheme check, if scheme is `http`:
  - Parse `parsed.host()` as an `IpAddr` if possible; if it parses, classify against the locked allowlist:
    - IPv4: `127.0.0.0/8`, `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`
    - IPv6: `::1`, `fc00::/7` (this is the canonical RFC 4193 ULA range — `fd00::/8` is a strict subset; the success criterion lists `fd00::/8` but the broader RFC range is what's meaningful. Researcher confirms: spec aligns with `fd00::/8` literal per WH-07; if `fc00::/7` is more semantically correct, Phase 20 plan can call this out and ship the broader allow.)
  - If the host is NOT a parseable IP (i.e., it's a hostname), classify by special-cased name: `localhost` accepted; everything else (any DNS name) → REJECT with HTTPS-required error pointing at the offending URL.
  - HTTP allowed → emit a startup INFO log naming the URL + classified-net (helps operators spot misconfiguration); HTTPS always allowed silently.
- **D-20:** **No DNS resolution at LOAD time.** The validator is textual/IP-classification-only. Hostnames that resolve to public IPs but appear `localhost`-shaped (e.g., a `127.0.0.1` -aliased hostname in `/etc/hosts`) are accepted; this is documented as accepted residual SSRF risk in `docs/WEBHOOKS.md` (mirrors the WH-08 SSRF accepted-risk posture).
- **D-21:** Validator name: `check_webhook_url_https_required` (or extend the existing `check_webhook_url` — researcher decides). Error message format mirrors P17 LBL precedent: `[[jobs]] \`{name}\`: webhook.url \`{url}\` requires HTTPS for non-loopback / non-RFC1918 destinations. Use \`https://\` or one of the allowed local nets: 127/8, ::1, 10/8, 172.16/12, 192.168/16, fd00::/8.`

### Metrics Family (WH-11)
- **D-22:** **Replace the P18 flat counters with the labeled family.** Phase 18 shipped `cronduit_webhook_delivery_sent_total` (no labels) + `cronduit_webhook_delivery_failed_total` (no labels). Phase 20 unifies these as `cronduit_webhook_deliveries_total{job, status}` with `status ∈ {success, failed, dropped}`. The P15 `cronduit_webhook_delivery_dropped_total` (queue-saturation drops) STAYS as a separate flat counter — it's a distinct semantic event (channel-side drop, not delivery-pipeline drop) and renaming it would break operators' Prometheus dashboards.
- **D-23:** Migration plan for the rename:
  - Phase 18 flat counters (`_sent_total`, `_failed_total`) become the labeled family `_deliveries_total{status="success"|"failed"}`.
  - The P15 `_delivery_dropped_total` stays as-is; the labeled family adds `status="dropped"` for events drained-and-dropped at shutdown (D-15) — distinct from queue-saturation drops which still increment `_delivery_dropped_total`.
  - Update `src/telemetry.rs` to describe + zero-baseline the labeled family at boot for all three status values per job-known-at-boot. Researcher confirms whether `metrics-exporter-prometheus` lets us pre-seed `{job, status}` rows by job name (we have the names from `sync_result.jobs` at startup) — preferred. Otherwise, omit the per-job seed and accept that `{job, status}` rows materialize on first observation.
- **D-24:** `cronduit_webhook_delivery_duration_seconds{job}` — histogram of single HTTP attempt duration, NOT chain wall time. Recorded in `HttpDispatcher::deliver` via `tokio::time::Instant::now()` deltas. Bucket choice: research/planner decides — start with the metrics crate's default histogram buckets unless evidence shows otherwise. Eagerly described per existing pattern.
- **D-25:** `cronduit_webhook_queue_depth` — gauge sampled by the worker on each `rx.recv()` boundary using `mpsc::Receiver::len()`. NO additional polling task. Eagerly described + zero-baselined at boot.
- **D-26:** Dropped-counter semantic split (operator-visible distinction):
  - `cronduit_webhook_delivery_dropped_total` — channel-saturation drops (P15, scheduler-side `try_send` failure). Increments on `TrySendError::Full`. Indicates dispatcher CANNOT keep up.
  - `cronduit_webhook_deliveries_total{status="dropped"}` — drain-on-shutdown drops (P20 D-15). Increments per event drained-and-dropped at SIGTERM budget expiry. Indicates SHUTDOWN-time loss.
  Both documented in `docs/WEBHOOKS.md` so operators set the right alert.

### `docs/WEBHOOKS.md` Extension (operator-doc surface)
- **D-27:** Phase 19 created `docs/WEBHOOKS.md` (10 sections; receiver examples + HMAC + Standard Webhooks v1 link). Phase 20 ADDS new sections:
  1. **Retry schedule** — locked t=0/30s/300s + jitter math + 4xx/5xx/408/429 classification table
  2. **Retry-After header handling** — what cronduit honors (D-07/D-08 cap math)
  3. **DLQ table** — schema + operator example SQL queries (`SELECT * FROM webhook_deliveries WHERE last_attempt_at > datetime('now', '-1 hour')`)
  4. **Drain on shutdown** — D-15/D-18 semantics + actual ceiling = `drain_grace + 10s`
  5. **HTTPS / SSRF posture** — D-19 allowlist + accepted-risk note for hostnames + pointer to `THREAT_MODEL.md` TM5 (stub in P20; full in P24)
  6. **Metrics family** — D-22 rename note + dropped-counter split
  Maintainer-facing edits land in the same PR as the code (no follow-up PR for docs).

### rc.1 Tag Cut (Release Engineering)
- **D-28:** **Re-use the existing `docs/release-rc.md` runbook.** No runbook changes; the v1.1.0-rc.1 → v1.2.0-rc.1 path is identical. Cargo.toml is already at `1.2.0` (P15). Pre-flight checklist: all P15-P20 PRs merged to `main` + green CI + green compose-smoke + git-cliff release-notes preview.
- **D-29:** Tag command: `git tag -a -s v1.2.0-rc.1 -m "v1.2.0-rc.1 — webhook block (P15..P20)"`. The `:latest` GHCR tag stays at `v1.1.0` (the `release.yml` patch from P12 D-10 enforces this automatically — `:latest` is gated to skip on tags containing a hyphen). The rolling `:rc` tag updates to `v1.2.0-rc.1` on push.
- **D-30:** Phase 20 does NOT modify `release.yml`, `cliff.toml`, or `docs/release-rc.md`. Any maintainer-discovered runbook gap discovered during the rc.1 cut is folded into a hotfix PR before tagging (mirrors v1.1 P12 discipline).
- **D-31:** GitHub Release notes: `git-cliff` output is authoritative (per v1.1 P12 D-12). Phase 20 does NOT hand-edit the release body post-publish.

### Universal Project Constraints (carried forward)

> The decisions below are **[informational]** — repo-wide process constraints honored by absence (mermaid-only diagrams, PR-only branch state, maintainer-validated UAT, just-recipe UAT). They are not phase-implementation tasks.

- **D-32:** [informational] All changes land via PR on a feature branch. No direct commits to `main`. Working branch: `phase-20-webhook-posture-rc1` (or similar; planner picks). The frontmatter STATE.md update from earlier this session ride-alongs on this branch's first commit (per session note).
- **D-33:** [informational] Diagrams in any Phase 20 artifact (PLAN, SUMMARY, README, code comments, `docs/WEBHOOKS.md`) are mermaid. No ASCII art. (Carries forward project memory `feedback_diagrams_mermaid.md`.)
- **D-34:** UAT recipes use existing `just` commands. New `just` recipes for Phase 20 UAT (e.g., `uat-webhook-retry`, `uat-webhook-drain`, `uat-webhook-dlq-query`) follow the P18/P19 `recipe-calls-recipe` pattern.
- **D-35:** [informational] Maintainer validates UAT — Claude does NOT mark UAT passed from its own runs (project memory `feedback_uat_user_validates.md`).
- **D-36:** [informational] Tag and version match — `Cargo.toml` is at `1.2.0`; the rc tag is `v1.2.0-rc.1`. Per project memory `feedback_tag_release_version_match.md` the in-source version stays unsuffixed; `-rc.1` is the tag-only suffix.
- **D-37:** [informational] No UI surface in Phase 20. `webhook_deliveries` is SQL+metrics-only in v1.2 (UI deferred to v1.3 per ROADMAP). ROADMAP marks Phase 20 `UI hint: no` (implicit).
- **D-38:** [informational] Cronduit-side rustls invariant unchanged — `cargo tree -i openssl-sys` must remain empty. Phase 20 adds zero new TLS-touching crates. The reqwest 0.13 `rustls` feature (P18) covers all HTTP needs.

### Claude's Discretion
- Exact migration filenames + timestamp prefix (Phase 20 starts a new sequence after `20260429_000007`).
- Internal struct names (`RetryingDispatcher` is the public name; field names planner picks).
- Exact validator function name (`check_webhook_url_https_required` vs extending `check_webhook_url`) — researcher decides based on existing function shape.
- IPv6 ULA classification: `fc00::/7` (RFC-correct, broader) vs `fd00::/8` (success-criterion-literal, narrower). Researcher checks the existing IPv4 `is_private` impl precedent and aligns; success criterion 1 mentions `fd00::/8` so default is the narrower form. Plan calls this out explicitly.
- Histogram bucket choice for `cronduit_webhook_delivery_duration_seconds` — researcher recommends; default to `metrics` crate defaults absent evidence.
- Whether `Retry-After` cap math uses `next_attempt+1` worst-case (D-08) or a simpler hard ceiling like `1.2× max(schedule)` — planner picks; either is operator-predictable.
- The split between LOAD-time validator extension and a NEW `src/config/validate.rs` function — researcher decides based on `validate.rs` size + existing function-decomposition pattern.
- Which `just` recipes to add for UAT (suggested floor: `uat-webhook-retry`, `uat-webhook-drain`, `uat-webhook-dlq-query`, `uat-webhook-https-required`); planner finalizes the set against `20-HUMAN-UAT.md` scenarios.
- Whether to ship a top-level `src/webhooks/retry.rs` for `RetryingDispatcher` or extend `dispatcher.rs` — researcher decides based on file-size of `dispatcher.rs` (currently 535 lines).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level locks
- `.planning/PROJECT.md` — core value, locked v1.2 webhook decisions (HMAC SHA-256 only, retry shape, payload schema, reload survival, drain), Tech Stack constraints (rustls everywhere, mermaid diagrams, PR-only workflow, just-recipe UAT)
- `.planning/REQUIREMENTS.md` § Webhooks — `WH-05`, `WH-07`, `WH-08`, `WH-10`, `WH-11` are Phase 20's requirements; `WH-01`/`WH-03`/`WH-04`/`WH-06`/`WH-09` (Phases 18-19) are upstream prerequisites already met
- `.planning/STATE.md` § Accumulated Context > Decisions — v1.2 webhook decisions inherited from research/requirements (retry t=0/30s/300s, drain 30s, HTTPS-non-loopback/RFC1918, etc.)
- `.planning/ROADMAP.md` § Phase 20 — goal + 5 success criteria + dependency on Phase 19 + rc.1 cut commitment
- `./CLAUDE.md` — project conventions, locked tech stack, mermaid-only, PR-only workflow, GSD enforcement

### Standard Webhooks v1 spec (READ DIRECTLY — do NOT paraphrase)
- `https://github.com/standard-webhooks/standard-webhooks/blob/main/spec/standard-webhooks.md` — wire format spec; Phase 20 doesn't touch the wire format (P18/P19 own it) but the retry-respect contract is consistent with the spec's recommendations.

### HTTP RFCs (referenced by retry classification)
- RFC 7231 §7.1.3 — `Retry-After` header semantics. Phase 20 honors integer-seconds form only; HTTP-date form NOT supported in v1.2 (D-07).
- RFC 1918 — IPv4 private address space (10/8, 172.16/12, 192.168/16). Used by D-19 allowlist.
- RFC 4193 — IPv6 ULA (`fc00::/7`). Success criterion uses `fd00::/8` (narrower); D-19 + Claude's discretion notes the choice.

### Phase 18 (the cronduit signing + dispatcher side — already implemented)
- `.planning/phases/18-webhook-payload-state-filter-coalescing/18-CONTEXT.md` — D-17/D-18/D-19/D-20/D-21 (single-attempt posture, 10s timeout, serial-within-worker, reqwest+rustls, retry hook point); D-25/D-26 (UAT discipline)
- `.planning/phases/18-webhook-payload-state-filter-coalescing/VERIFICATION.md` — Phase 18 dispatcher verification record
- `src/webhooks/dispatcher.rs:90-118` — `HttpDispatcher::new` (Phase 20 wraps this in `RetryingDispatcher`)
- `src/webhooks/dispatcher.rs:158-303` — `impl WebhookDispatcher for HttpDispatcher` (the inner attempt; Phase 20 must NOT modify the trait — composition only)
- `src/webhooks/dispatcher.rs:25-51` — `WebhookError` variants (already lists `HttpStatus`, `Network`, `Timeout` as `#[allow(dead_code)] // Phase 20 RetryingDispatcher consumes` — Phase 20 removes the dead-code allows)
- `src/webhooks/worker.rs:50-96` — `worker_loop` + `tokio::select!` cancel/recv pattern (Phase 20 extends this with the drain budget; current code already exits on cancel + on channel close, which is the correct base behavior)

### Phase 19 (the receiver-interop + retry-respect contract)
- `.planning/phases/19-webhook-hmac-signing-receiver-examples/19-CONTEXT.md` — D-12 retry-respect contract table (4xx-permanent / 5xx-transient); D-09 5-minute drift window for receivers (anti-replay context); D-15 webhook-interop CI matrix
- `.planning/phases/19-webhook-hmac-signing-receiver-examples/19-VERIFICATION.md` — Phase 19 sign-side + receiver-interop verification record
- `docs/WEBHOOKS.md` — operator hub doc shipped in P19; Phase 20 EXTENDS with retry/DLQ/drain/HTTPS sections (D-27)

### Phase 15 (foundation worker + drop counter)
- `.planning/phases/15-foundation-preamble/15-CONTEXT.md` — webhook worker scaffolding decisions (bounded mpsc(1024), `try_send` from scheduler, dedicated worker task, `cronduit_webhook_delivery_dropped_total` counter)
- `src/webhooks/worker.rs` — `CHANNEL_CAPACITY = 1024`, `spawn_worker`, `worker_loop` (Phase 20 extends with drain budget logic)
- `src/scheduler/run.rs:445-452` — scheduler emits `RunFinalized` via `try_send`; on `Err(TrySendError::Full)` increments `cronduit_webhook_delivery_dropped_total` (Phase 20 keeps this counter as-is per D-26)

### Phase 16 (config_hash + retention pruner pattern)
- `src/scheduler/retention.rs` — daily retention pruner (Phase 20 extends to also DELETE from `webhook_deliveries` per D-14; researcher confirms shape)
- `migrations/sqlite/20260428_000006_config_hash_add.up.sql` + postgres mirror — one-file additive migration precedent (Phase 20 mirrors for `webhook_deliveries`)

### Phase 17 (LBL precedent for validators + just-recipe UAT)
- `.planning/phases/17-custom-docker-labels-seed-001/17-CONTEXT.md` — D-04 README structure; D-08 just-recipe UAT pattern; CR-01 truth on whole-file env-var interpolation
- `src/config/validate.rs:385-417` — existing `check_webhook_url` (Phase 20 extends OR adds a sibling per D-21)

### Phase 12 / 12.1 (rc.1 release-engineering precedent — v1.1)
- `.planning/milestones/v1.1-phases/12-docker-healthcheck-rc-1-cut/12-CONTEXT.md` — D-10/D-11/D-12/D-13 rc-cut decisions (release.yml `:latest` gating, runbook structure, git-cliff authoritative, maintainer-not-workflow_dispatch trust anchor)
- `docs/release-rc.md` — runbook itself; reused verbatim (D-28). NO modifications in Phase 20.
- `.github/workflows/release.yml` — `:latest` gated to skip on tags containing `-` (per P12 D-10); Phase 20 does NOT modify this file (D-30).

### Existing Cronduit infra to reuse
- `Cargo.toml` — `reqwest 0.13` (rustls + json features, P18), `tokio` (full), `chrono`, `tracing`, `serde`, `metrics`, `metrics-exporter-prometheus`, `humantime-serde`, `secrecy`, `hmac`, `sha2`, `base64`, `ulid`, `rand` (0.9 per v1.1 hygiene), `sqlx` (sqlite + postgres + rustls features) all present. Phase 20 adds ZERO new external crates. `cargo tree -i openssl-sys` must remain empty (D-38).
- `src/telemetry.rs:91-153` — eager describe + zero-baseline pattern. Phase 20 EXTENDS this with `cronduit_webhook_deliveries_total{job, status}`, `_delivery_duration_seconds{job}`, `_queue_depth` per D-23/D-24/D-25.
- `src/cli/run.rs:299-328` — webhook worker spawn + drain ordering (scheduler awaits before worker). Phase 20 wraps the dispatcher in `RetryingDispatcher::new(http, pool.clone(), cancel.child_token())` between lines 297 and 299.
- `src/config/mod.rs:42-46` — `[server]` block + `shutdown_grace` field (`humantime_serde`). Phase 20 ADDS `webhook_drain_grace` field same shape.
- `src/db/queries.rs` — DLQ insert helper joins this module (Phase 20's planner names the function: e.g., `insert_webhook_dlq_row`).
- `migrations/{sqlite,postgres}/` — one-file additive migration pattern. Phase 20 ships ONE pair of files for the `webhook_deliveries` table.
- `justfile` — existing `uat-webhook-*` family (P18/P19) for recipe-naming consistency. Phase 20 adds `uat-webhook-retry`, `uat-webhook-drain`, `uat-webhook-dlq-query`, `uat-webhook-https-required` (planner finalizes).
- `tests/v12_webhook_*` — existing P15/P18 integration test layout. Phase 20 adds `tests/v12_webhook_retry.rs`, `tests/v12_webhook_drain.rs`, `tests/v12_webhook_dlq.rs`, `tests/v12_webhook_https_required.rs` (planner finalizes naming).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`HttpDispatcher` from Phase 18** (`src/webhooks/dispatcher.rs:90-303`): the single-attempt deliverer Phase 20 wraps. Phase 18 already pre-allowed the `WebhookError::HttpStatus`/`Network`/`Timeout`/`InvalidUrl` variants under `#[allow(dead_code)] // Phase 20 RetryingDispatcher consumes` — Phase 20 removes those allows by populating the variants in `HttpDispatcher::deliver`'s `match response { ... }` block AND consuming them in `RetryingDispatcher::deliver` for classification (D-06).
- **`WebhookDispatcher` trait** (`src/webhooks/dispatcher.rs:53-56`): two-line trait. `RetryingDispatcher` impls it the same way `HttpDispatcher` does — composition without trait expansion (P18 D-21).
- **`worker_loop` cancel/recv `tokio::select!`** (`src/webhooks/worker.rs:50-96`): the existing pattern Phase 20 extends. Add a third arm for the drain budget OR change the cancel arm's behavior to enter a "drain-and-drop" sub-loop. Researcher decides; both shapes are idiomatic.
- **`mpsc::Receiver::len()`**: stdlib method (sampled in D-25 for queue-depth gauge).
- **Existing `try_send` + drop counter at `src/scheduler/run.rs:445-452`**: NOT touched by Phase 20 — that counter is the channel-saturation drop (P15), distinct from the drain-on-shutdown drop (D-26).
- **`secrecy::SecretString` wrapper** (used in P18 for `webhook.secret`): NOT touched by Phase 20 — the dispatcher already exposes the secret only at `sign_v1` call boundaries.
- **`humantime_serde`** for `webhook_drain_grace = "30s"` (existing pattern at `src/config/mod.rs:44`).
- **`url::Url` + `parsed.host()` -> `IpAddr` parse**: stdlib; Phase 20 D-19 leverages this for the HTTPS-required validator (no new crate).
- **`rand::random::<f64>()`**: existing crate for jitter math (D-02).
- **`reqwest::Response::headers()` + `Retry-After` parse**: stdlib `Duration::from_secs(s.parse::<u64>()?)`; no new crate.
- **`metrics::counter!` / `histogram!` / `gauge!` macros**: existing pattern in `src/telemetry.rs` + dispatchers; Phase 20 adds the `{job, status}` labeled family per D-22.

### Established Patterns
- **`[defaults]` + per-job override + `use_defaults = false`**: Phase 20 doesn't add per-job retry/drain config — both are global (`[server].webhook_drain_grace`); the locked retry schedule is intentionally NOT operator-tunable (predictability).
- **LOAD-time validators with `ConfigError { line: 0, col: 0 }`**: D-19/D-21 follow the P17/P18 precedent.
- **One-file additive migration** (precedent: P16's `job_runs.config_hash` add): D-13 mirrors for `webhook_deliveries`.
- **`#[ignore]` integration tests** for HTTP path tests: Phase 20's retry/drain integration tests follow this pattern (run with `--features integration` / nextest filter).
- **Eager metric description + zero-baseline at boot** (`src/telemetry.rs` lines 91-153): D-23 extends.
- **CI matrix `linux/{amd64,arm64} × {SQLite, Postgres}`**: Phase 20 doesn't add a new matrix axis — the new tests run on the existing `webhook-interop` job's adjacent matrix legs (Postgres + SQLite already covered).

### Integration Points
- **Where `RetryingDispatcher` wires in**: `src/cli/run.rs` lines 286-301 — after building `HttpDispatcher`, wrap in `RetryingDispatcher::new(http, pool.clone(), cancel.child_token())` THEN spawn the worker with the wrapped dispatcher. The worker doesn't see the wrapping (composition).
- **Where the URL validator hooks in**: `src/config/validate.rs::run_all_checks` per-job loop (existing P18 location for `check_webhook_url`); D-19 either extends the existing function or adds a sibling.
- **Where the DLQ insert lives**: `src/db/queries.rs` (mirrors P16's `get_failure_context` insert-helper shape). Function signature: `pub async fn insert_webhook_dlq_row(pool: &DbPool, row: WebhookDlqRow) -> sqlx::Result<()>`.
- **Where the drain budget is enforced**: `src/webhooks/worker.rs::worker_loop` — extend the existing `tokio::select!` with a `let drain_deadline = tokio::time::Instant::now() + drain_grace; tokio::select! { _ = sleep_until(drain_deadline) => ... }` arm OR transition to a drain-only sub-loop on cancel-fire. Researcher decides.
- **Where the metrics-family upgrade lands**: `src/telemetry.rs` (describe + zero-baseline) + `src/webhooks/dispatcher.rs` (the `metrics::counter!("cronduit_webhook_deliveries_total", "job" => job_name, "status" => "success").increment(1)` calls). The `_sent_total` and `_failed_total` calls in `dispatcher.rs:262, :272, :285` are REMOVED in favor of the labeled family.
- **Where the THREAT_MODEL.md TM5 stub lands**: a forward pointer in `docs/WEBHOOKS.md` (per D-27 §5) — full TM5 is Phase 24's deliverable.
- **Where the rc.1 tag cut runs**: maintainer-local terminal, NOT CI (per `docs/release-rc.md` D-13). Phase 20 does NOT spawn a tagging script.

</code_context>

<specifics>
## Specific Ideas

- **Retry chain runs in the worker task, NOT in a spawned sub-task.** Preserves P18 D-19 serial-within-worker. A long retry chain (e.g., 5xx + 5xx → wait 300s) blocks the next event from being dequeued — but the channel buffers up to 1024 events and the P15 drop counter catches sustained backpressure. Operators getting `_dropped_total{}` increments are the signal to either fix their receiver or investigate.
- **`Retry-After` cap math** (D-08): for attempt-2 (the t=30s slot), `cap = locked_schedule[3] * 1.2 = 300 * 1.2 = 360s`; for attempt-3 (t=300s), `cap = 360s` as well (no slot 4). Documented operator-facing in `docs/WEBHOOKS.md`.
- **DLQ `dlq_reason` for `shutdown_drain`** is distinct from the actual last attempt status — it means "drain budget cut the chain short," not "attempt N got <classification>." Operator queries that filter on `dlq_reason = 'shutdown_drain'` find the SIGTERM-loss subset specifically.
- **In-memory chain + cancel token** means the chain's `tokio::select!` on every sleep is the SINGLE shutdown-aware primitive. Don't add a second cancel check inside the HTTP request — reqwest's 10s timeout is the cap, and the success criterion explicitly forbids cancelling in-flight requests.
- **Validator error format** (mirroring Phase 17/18 LBL+webhook precedent):
  ```
  [[jobs]] `backup-nightly`: webhook.url `http://example.com` requires HTTPS for non-loopback / non-RFC1918 destinations. Use `https://` or one of the allowed local nets: 127/8, ::1, 10/8, 172.16/12, 192.168/16, fd00::/8.
  ```
- **Boot-time INFO log for HTTP-allowed URLs** (D-19): `"webhook URL http://192.168.1.10/hook accepted (RFC1918 192.168.0.0/16)"`. Helps operators confirm the validator's classification matches their intent.
- **Worst-case shutdown ceiling = `webhook_drain_grace + 10s`** (D-18). For `webhook_drain_grace = "30s"`, that's 40s total — a few seconds longer than `shutdown_grace` defaults in many homelabs. Documented; operators with strict shutdown budgets should set both knobs in concert.
- **rc.1 release notes will aggregate Phase 15+16+17+18+19+20 commits** — git-cliff handles this from conventional-commit messages. The pre-flight checklist's `git cliff --unreleased --tag v1.2.0-rc.1` step catches any mis-categorized commit BEFORE tagging.
- **`webhook_deliveries` is queryable but not displayed** in v1.2 — operators run `sqlite3 cronduit.db 'SELECT * FROM webhook_deliveries WHERE last_attempt_at > datetime("now", "-1 hour")'` (or postgres equivalent). v1.3 candidate: a dashboard panel.

</specifics>

<deferred>
## Deferred Ideas

- **Webhook delivery UI** (delivery status, replay/retry button on dashboard, DLQ inspector panel): NOT in scope; v1.3 candidate per ROADMAP. Operators query `webhook_deliveries` via SQL in v1.2.
- **Durable webhook queue** (retries survive restart): explicit deferred per PROJECT.md § Future Requirements > v1.3. Phase 20 in-memory chain is best-effort; restart loses mid-chain retries.
- **DB-backed retry queue with `next_attempt_at` polling**: explicit alternative considered + rejected (Gray Area 1, D-01). Revisit only if operator demand for restart-survival surfaces.
- **Per-attempt audit log** (one row per HTTP attempt vs DLQ-only): explicit alternative considered + rejected (Gray Area 2, D-10). Revisit if operators ask for per-attempt diagnostics.
- **HTTP-date form of `Retry-After`** (D-07): not supported in v1.2; integer-seconds form only. Add if a real receiver demands it.
- **Per-job `webhook.attempt_timeout` configurability**: P18 D-18 hard-coded 10s; deferred to "if operator demand emerges." None has emerged at Phase 20 kickoff.
- **Concurrent delivery semaphore** (parallel HTTP within worker): P18 D-19 serial-within-worker stays. Deferred to v1.3+ if load demands.
- **SSRF allow/block-list filter** beyond loopback+RFC1918 classification: WH-08 explicit accepted-risk; v1.3 candidate per PROJECT.md § Future Requirements.
- **Webhook URL DNS resolution at LOAD time**: D-20 explicit "no DNS at load." Hostname-resolves-to-public-IP residual risk documented in `docs/WEBHOOKS.md`. Revisit if accepted-risk posture shifts.
- **Per-job metric labels with reason granularity**: closed enum `{success, failed, dropped}` only on the metrics counter. Reason granularity (4xx vs 5xx vs network vs timeout) lives in `webhook_deliveries.dlq_reason` (SQL-only). Revisit if Prometheus query patterns demand label-side reason.
- **THREAT_MODEL.md TM5 (Webhook Outbound) full close-out**: Phase 24 owns this per ROADMAP. Phase 20 ships a doc-pointer stub only.
- **`release.yml` / `cliff.toml` / `release-rc.md` modifications** for v1.2-specific rc behavior: the v1.1 P12 runbook is reused verbatim per D-28/D-30. Any maintainer-discovered runbook gap during the rc.1 cut is a hotfix PR before tagging.
- **`webhook_deliveries` retention beyond `log_retention`**: D-14 reuses the existing 90-day knob. Revisit if operators want the DLQ table on a different cadence.
- **Renaming `cronduit_webhook_delivery_dropped_total`**: explicit kept-as-is per D-22 to preserve operator dashboards. The new labeled family adds `status="dropped"` for shutdown drops; the legacy counter stays for queue-saturation drops.

</deferred>

---

*Phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1*
*Context gathered: 2026-05-01*
