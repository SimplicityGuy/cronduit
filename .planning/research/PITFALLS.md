# Cronduit v1.2 — Pitfalls Research

**Dimension:** Integration Pitfalls (subsequent milestone — adding 5 features on top of shipped v1.1.0 codebase)
**Milestone:** v1.2 "Operator Integration & Insight"
**Researched:** 2026-04-25
**Confidence:** HIGH — every pitfall is grounded in a direct read of the shipped v1.1.0 source tree, the v1.1 pitfall log, the SEED-001 design decisions, and the existing `THREAT_MODEL.md`. Line numbers and file paths included where nontrivial.

> This document continues the v1.1 PITFALLS.md tone and depth: numbered, ranked Critical → Moderate → Minor per feature, with a phase-mapping summary at the bottom. **Numbering continues from the v1.1 series** — v1.1's last numbered pitfall stub was around #27 (counting Critical + Moderate + Minor entries in v1.1's nine feature sections), so v1.2 numbering starts at **Pitfall 28** and runs through **Pitfall 56** across the five v1.2 features plus a cross-feature section.
>
> v1.0 + v1.1 baseline pitfalls (docker socket = root-equivalent, log back-pressure, DST, SQLite write contention, schema parity, openssl cross-compile, secret-leak-into-errors, scheduler clock drift, `mark_run_orphaned WHERE status='running'` guard, preserved `.process_group(0)` instead of `kill_on_drop`) are NOT re-flagged here. Each v1.2 pitfall that re-enters one of those zones calls it out as a 🔗 regression risk.
>
> Five v1.2 features ranked by integration risk (highest first):
>
> 1. **Webhook notifications** — net-new outbound surface, retry policy, delivery worker isolation. Highest blast-radius expansion of v1.2.
> 2. **Custom Docker labels (SEED-001)** — design pre-locked, but operator-misuse pitfalls remain.
> 3. **Failure context (image-digest + config-hash deltas, streak math)** — depends on a per-run column the codebase does not yet record per-run (config_hash is at job-level only).
> 4. **Per-job exit-code histogram** — additive UI card, but cardinality + window-choice pitfalls are real.
> 5. **Job tagging / grouping** — UI-only filter chips, but normalization and filter UX have classic pitfalls.

---

## How to Read This File

Each feature section has the pitfalls ranked **Critical → Moderate → Minor** for that feature. Every pitfall has:

- **Where:** the file(s) and (where relevant) line numbers the pitfall touches
- **What goes wrong:** the failure mode in plain language
- **Why:** the specific code shape, ecosystem behavior, or operator-experience driver
- **When it manifests:** config load? first fire? long-running deployment? upgrade?
- **Prevention:** an actionable fix — ideally a named test case, a validator, or a doc
- **Phase hint:** which v1.2 phase should own the mitigation (rough; not binding)
- **Test case name (T-V12-…):** parallels the v1.1 `T-V11-…` convention

A pitfall marked 🔗 indicates it touches a hazard zone already mitigated in v1.0 or v1.1; the v1.2 fix must NOT re-open that pitfall.

---

## Feature 1 — Webhook Notifications

The single largest blast-radius expansion in v1.2: cronduit gains an outbound HTTP path. Pitfalls cluster around (a) keeping the scheduler isolated from the network, (b) flood control, (c) cryptographic signing done correctly the first time, (d) SSRF / HTTPS posture, and (e) payload schema stability so receivers don't break on v1.3+.

### Pitfall 28 — CRITICAL — Blocking the scheduler loop on outbound HTTP

**Where:** New `src/scheduler/webhook/` module + the call site inside the executor finalize path (currently `src/scheduler/run.rs::finalize_run` ≈L260–L295 — the same place that writes terminal status, fires `cronduit_runs_total{status}`, and removes the entry from `running_handles`).

**What goes wrong:** Naive shape: `finalize_run` does `reqwest::post(url).send().await` (or any blocking-await on an outbound HTTP call) inline. Failure modes:

1. **Slow receiver = stalled scheduler.** Operator's Slack receiver takes 30s to ack (network blip, receiver CPU-pinned). The scheduler loop is blocked for 30s — the next job's fire window is missed by 30s, every other in-flight job's `finalize_run` queues up behind, and the entire fire heap drifts.
2. **Receiver down = drift forever.** A persistently-failing webhook with 3 retries × 30s connect timeout = 90s of dead time per failed job. Five concurrent failed jobs = 7.5 minutes of scheduler stall.
3. **The healthcheck flips to `unhealthy`.** `/health` is served by the same axum runtime. If the scheduler loop is starved, axum tasks still respond, but `cronduit_scheduler_up` may flap if any scheduler-pulse signal is wired into the health logic.
4. **Cancellation token doesn't cancel HTTP.** The scheduler's graceful-shutdown `CancellationToken` does NOT propagate into a blocking outbound HTTP call unless the call is wrapped in `tokio::select! { _ = http; _ = cancel.cancelled() => ... }`. SIGTERM hangs for the full HTTP timeout.

**Why:** v1.0/v1.1 chose a hand-rolled scheduler loop precisely so cronduit owned the timing semantics (`@random` + `random_min_gap` + in-flight survival across reload). That ownership is a benefit only if the loop never blocks on something the operator's environment controls. Outbound HTTP is the canonical "operator-environment-controlled latency" surface.

**When it manifests:** First fire after a webhook is configured against a slow/down receiver. Visible as scheduler drift in `cronduit_scheduler_up` flapping, missed fires in `cronduit_runs_total`, or operator-reported "my 1-minute job runs every 90 seconds now."

**Prevention:**

1. **Bounded `mpsc` channel + dedicated delivery worker.** `finalize_run` calls `webhook_tx.try_send(WebhookJob { run_id, terminal_status, payload })` and **always returns immediately**. The `try_send` is non-blocking; on full channel it logs a warn + increments a `cronduit_webhook_dropped_total{reason="queue_full"}` counter and continues. Bounded capacity (recommendation: 1024). The scheduler loop never awaits a network call.
2. **Delivery worker is a separate `tokio::spawn` task** in `src/scheduler/webhook/worker.rs` owning its own `hyper-util` client (already a transitive dep — see `Cargo.toml` L27–L28). The worker receives `WebhookJob`, looks up the operator URL/secret/state-filter, applies retry policy, and exits cleanly when its `CancellationToken` fires.
3. **Scheduler-shutdown integration.** On `SIGTERM`, the scheduler's graceful drain (`with_graceful_shutdown` + the existing double-signal SIGINT/SIGTERM state machine, v1.0 Phase 2) waits up to a configurable `webhook_drain_timeout` (default 10s) for the queue to flush; remaining queued webhooks are dropped (counted in `cronduit_webhook_dropped_total{reason="shutdown_drain"}`). Document this as expected behavior — webhooks are best-effort across restart.
4. **Survive scheduler reload.** `SchedulerCmd::Reload` re-reads webhook URLs/secrets per job; the worker continues running across reload (it's owned by `AppState`, not by `SchedulerLoop`). The bounded channel persists across reload — in-flight queued webhooks are NOT dropped on reload. Lock with a test.
5. **Decouple from `metrics-exporter-prometheus`.** Add a new gauge `cronduit_webhook_queue_depth` (no labels) and counter `cronduit_webhook_dropped_total{reason}` (closed enum: `queue_full`, `shutdown_drain`, `state_filter_excluded`, `disabled_at_load`). Both eagerly described at boot per the v1.0 bounded-cardinality discipline.

**Phase hint:** Phase 15 (webhook delivery worker — foundation for the entire feature). Lock the isolation pattern in the very first PR; every subsequent webhook PR depends on the worker existing.

**Test cases:**
- **T-V12-WH-01:** Stall a mock receiver for 60s; fire 5 webhooks; assert scheduler fires the next 5 scheduled jobs on time (no drift > 1s).
- **T-V12-WH-02:** Mock receiver returns 500 forever; fire a webhook; assert the worker performs 3 attempts, then drops with `delivery_failed`; assert scheduler unaffected.
- **T-V12-WH-03:** Fill the bounded channel to capacity; fire one more; assert `cronduit_webhook_dropped_total{reason="queue_full"}` increments and the scheduler unaffected.
- **T-V12-WH-04:** Send SIGTERM during in-flight delivery; assert process exits within `webhook_drain_timeout + 5s`; assert remaining queued webhooks counted as `shutdown_drain`.
- **T-V12-WH-05:** Trigger `SchedulerCmd::Reload` while a delivery is in-flight; assert delivery completes (worker survives reload).

**Severity:** CRITICAL — breaking this breaks the entire core promise ("execute jobs on time").

---

### Pitfall 29 — CRITICAL — Webhook flooding (no per-URL rate limit, no streak coalescing)

**Where:** Same delivery worker. Triggered by jobs that fail repeatedly on a tight schedule.

**What goes wrong:** Operator has a job `* * * * *` (every minute) with `webhook_states = ["failed"]`. The job starts failing at 03:00. By 03:30, the operator's PagerDuty/Slack receiver has been hit 30 times for the *same* underlying problem. Operator gets paged 30 times overnight; ignores future cronduit alerts entirely; cronduit becomes a known-bad-signal source in the operator's broader observability stack. **The whole point of webhooks is undermined by their own success.**

Variant: the operator's receiver itself rate-limits (Slack incoming-webhooks: ~1/sec per webhook URL; PagerDuty Events API: ~120/min per service key). Hitting the receiver-side rate limit produces 429s, retries fire, and the cronduit retry storm makes the receiver-side problem worse.

**Why:** No flood control is the default behavior of any "fire on terminal status" notifier unless explicitly designed in. v1.2 inherits the default unless we lock the policy now.

**When it manifests:** Within minutes of any fast-cadence (≤5min) job entering a sustained failure state. **Universal first-use experience** — every new operator will hit this on day 1 if not mitigated.

**Prevention (v1.2 default — lock at requirements time):**

1. **"First failure of new streak" semantics.** The default state-filter is **edge-triggered**, not level-triggered:
   - `webhook_states = ["failed"]` fires on the **transition** from non-failed→failed, AND on every Nth subsequent failure (default N=10), AND on the recovery transition failed→success.
   - Equivalent: cronduit emits at most ⌈runs/N⌉ + 2 webhooks during a sustained failure, regardless of run cadence.
   - The streak boundary is computed from `job_runs` queries (the same data the failure-context feature uses — see Feature 3 below). This creates a phase ordering: failure-context queries should land **before or alongside** webhook delivery.
2. **Operator override.** Per-job: `webhook_coalesce = "every"` to fire on every failure (legacy / loud setups), `"streak_first"` (default), `"streak_first_and_recovery"` (explicit), `"streak_first_every_n"` with `n` configurable.
3. **Per-URL rate limit (defense in depth).** Worker enforces a token bucket per destination URL: default 10 deliveries/min/URL. Drops over the bucket count toward `cronduit_webhook_dropped_total{reason="rate_limited"}`. Rationale: even with edge-triggered semantics, a misconfigured fleet (50 jobs all sharing one webhook URL all failing simultaneously) can flood; the per-URL bucket caps the worst case.
4. **Document the default loudly in the README.** "Cronduit fires webhooks on failure-streak boundaries, not every failure. Set `webhook_coalesce = 'every'` if you want the legacy noisy behavior." This is a discoverability item — operators searching for "cronduit only sent one alert" must find this immediately.
5. **State coalescing is a webhook-feature concern, NOT a scheduler concern.** The scheduler still runs the job every minute; the executor still writes a `failed` row every minute; metrics still increment. Only the webhook delivery is coalesced. This keeps the observability surface honest while taming the notification surface.

**Phase hint:** Phase 16 (webhook payload + state-filter logic). Land coalescing semantics in the same PR as the state-filter so they can't drift.

**Test cases:**
- **T-V12-WH-06:** Job fails 60 consecutive times; assert `streak_first` mode produces exactly 1 + ⌊60/10⌋ = 7 webhooks, with the first carrying `streak_position: "first_failure"` and intermediates carrying `streak_position: "ongoing"`.
- **T-V12-WH-07:** Job fails 5x then succeeds; assert exactly 2 webhooks (first failure, recovery).
- **T-V12-WH-08:** 50 jobs share one webhook URL; all fail at the same minute; assert per-URL rate limit drops the overflow with `rate_limited` reason; first 10 deliveries succeed.
- **T-V12-WH-09:** `webhook_coalesce = "every"` reproduces the noisy legacy behavior (1 webhook per failed run); assert no surprise drop.

**Severity:** CRITICAL — silently produces a worse operator experience than no webhooks at all.

---

### Pitfall 30 — CRITICAL — HMAC verification timing-attack vulnerability (in operator-written receivers)

**Where:** Documentation + the example receiver shipped in `examples/webhook-receiver.{py,rs}` (new). HMAC signing itself is in `src/scheduler/webhook/sign.rs` (new); we use `hmac` + `sha2` (already standard rust crypto, no new heavy deps; `sha2` is transitively pulled today via `sqlx`'s SCRAM auth).

**What goes wrong:** The cronduit-side HMAC implementation will be correct (we control it). The pitfall is on the **operator side** — anyone who reads our docs and writes a receiver may use `if signature == expected:` (Python, Go, JS, anywhere) which is timing-attack-vulnerable. A network-adjacent attacker can iterate through signature byte values, measuring response latency, and recover the HMAC byte-by-byte.

This is a documentation pitfall: cronduit ships the bug if its docs ship a vulnerable example.

**Why:** Constant-time comparison is a footgun across nearly every language. `hmac.compare_digest` (Python), `crypto.timingSafeEqual` (Node), `subtle.ConstantTimeCompare` (Go), `hmac::digest::CtOutput::eq` (Rust) all need to be used explicitly; `==` on bytes/strings is the natural-feeling-and-wrong choice everywhere.

**When it manifests:** The day an attacker with LAN access discovers cronduit (recall: v1 ships unauthenticated; the LAN-attacker model is the documented baseline). Long-running deployment risk.

**Prevention:**

1. **Cronduit-side signing (LOCK):**
   - Algorithm: HMAC-SHA256.
   - Header format: `X-Cronduit-Signature: sha256=<hex>` (matches GitHub webhooks convention — operators already know this shape).
   - **Signed payload = `<timestamp>.<request_body>`** (timestamp included to prevent replay; same shape as Stripe). `timestamp` = unix seconds, also sent as `X-Cronduit-Timestamp: <seconds>` header. Receivers SHOULD reject if `|now - timestamp| > 5min`.
   - **Sign body bytes only, NOT the URL.** URL signing is unnecessary (the URL is operator-known) and breaks proxy-rewriting setups.
   - Secret is opaque to cronduit (operator-owned random string, ≥32 bytes recommended). Cronduit interpolates from env-var via the existing `${ENV_VAR}` + `SecretString` pattern from v1.0 (no plaintext in TOML).
2. **Documentation MUST ship copy-paste-ready receiver examples in Python + Go + Node** that use the language's constant-time compare:
   - Python: `hmac.compare_digest(expected.encode(), signature.encode())`
   - Go: `subtle.ConstantTimeCompare([]byte(expected), []byte(signature)) == 1`
   - Node: `crypto.timingSafeEqual(Buffer.from(expected), Buffer.from(signature))`
   - Rust: `subtle::ConstantTimeEq::ct_eq(&expected, &signature).into()` or `hmac::Hmac::<Sha256>::verify_slice`.
3. **Each example MUST include a one-line comment explaining why the constant-time compare is mandatory.** Operators copy-pasting will leave the comment in; the next reader of the receiver code learns the rule for free.
4. **Secret rotation story.** Document the rotation procedure: (a) operator adds a new env-var, (b) receiver accepts both old + new signatures during the rotation window, (c) operator updates cronduit config to reference the new env-var, reload, (d) operator removes the old env-var. **Cronduit signs with one secret at a time — no dual-signing.** Keeps the cronduit code small; the multi-secret window lives on the receiver side. (Could revisit if operators ask for cronduit-side dual-signing in v1.3.)
5. **Test the cronduit-side signing against a known-vector** so future refactors of the signer can't drift silently.

**Phase hint:** Phase 17 (HMAC signing). Document examples in the same PR as the signer.

**Test cases:**
- **T-V12-WH-10:** Known-vector HMAC test: payload `'{"foo":"bar"}'`, timestamp `1700000000`, secret `"test-secret"` → expected signature locked as a constant in the test (recompute once at test-write time, never change).
- **T-V12-WH-11:** Verify `X-Cronduit-Timestamp` is present and within 60s of the actual delivery time on every request.
- **T-V12-WH-12:** Doc-coverage test (or PR checklist item): every receiver example file in `examples/webhook-receiver-*/` references the language's constant-time compare function.
- **T-V12-WH-13:** Receiver examples reject a request with a tampered body (signature mismatch).
- **T-V12-WH-14:** Receiver examples reject a request whose `X-Cronduit-Timestamp` is older than 5 minutes (replay window).

**Severity:** CRITICAL — operator-implemented receiver vulnerabilities are still cronduit's reputation problem.

---

### Pitfall 31 — CRITICAL — SSRF: webhook URL = arbitrary internal HTTP request

**Where:** `src/scheduler/webhook/worker.rs` outbound HTTP. The webhook URL comes from operator config (TOML) — but in the v1 unauthenticated UI threat model, "operator config" includes "anyone with LAN access who reads the config endpoint or has tampered with the file" (the Config Tamper threat — `THREAT_MODEL.md` § "Threat Model 3").

**What goes wrong:** An operator (or an attacker who's gained config access) sets `webhook_url = "http://192.168.1.1/admin/factory-reset"`, or `"http://169.254.169.254/latest/meta-data/iam/"` (cloud metadata endpoint), or `"http://localhost:6379/FLUSHALL\r\n"` (Redis CRLF injection — already mitigated by HTTP client's URL parser, but check), or `"http://internal-jenkins/job/build-prod/build"`. Cronduit's webhook delivery worker dutifully fires the request from within the homelab/VPC trust zone, hitting internal services that would never accept the connection from outside.

This is classic SSRF. Cronduit becomes a **proxy for arbitrary intra-network HTTP requests**.

**Why:** v1 ships unauthenticated UI by design. The Config Tamper model (`THREAT_MODEL.md` T-T1) acknowledges that an attacker with shell access can inject jobs. v1.2 webhooks expand the blast radius from "jobs that run shell/docker on the host" to "arbitrary HTTP requests to any address the cronduit network can reach." For homelab single-operator the practical risk is low; for shared-infra and cloud (where 169.254.169.254 is sensitive) it is meaningful.

**When it manifests:** Latent — the day an attacker compromises the LAN or the config file, the SSRF surface is theirs.

**Prevention (v1.2 stance — must be explicit, not assumed):**

1. **Default behavior: NO SSRF filter.** Consistent with v1's "loopback default + trusted LAN" posture. Adding a filter implies a security boundary that v1 explicitly declines to enforce.
2. **`THREAT_MODEL.md` MUST gain a new threat model entry** for "Threat Model 5: Webhook Outbound" enumerating:
   - Webhook URLs are operator-controlled and unfiltered.
   - The blast radius widens to "any HTTP-reachable address from the cronduit network namespace" (including cloud metadata, internal admin UIs, intra-homelab services).
   - Mitigation: same as the rest of v1's web UI — keep cronduit on loopback / trusted LAN, or front with reverse proxy + auth.
   - Recommendation: operators on cloud should explicitly block 169.254.0.0/16 at the cronduit container's network policy.
3. **Optional v1.2 hardening** (recommended, low cost): a startup-time **warn on suspicious webhook URLs** without blocking:
   - URL host is in `127.0.0.0/8`, `169.254.0.0/16`, or `::1` → log `WARN target="cronduit.webhook.suspicious_url"` with the offending job.
   - URL scheme is `http://` (not `https://`) AND host is not in RFC1918 / loopback → also WARN (see Pitfall 32).
   - **Do NOT block.** v1 doesn't block; v1 informs.
4. **Future hardening (v1.3+ candidate, NOT v1.2):** a `webhook.allow_loopback`, `webhook.allow_rfc1918`, `webhook.allowlist_hosts` config knob. Out of scope for v1.2 — adding a half-built filter is worse than no filter.
5. **`cronduit check` MUST surface the same WARN as the runtime startup** so operators validating config see the suspicious URLs before deployment.

**Phase hint:** Phase 18 (THREAT_MODEL.md update + startup warnings). Pair with Pitfall 32 (HTTPS posture).

**Test cases:**
- **T-V12-WH-15:** Configure a webhook URL pointing at `169.254.169.254`; assert startup logs a WARN with `target="cronduit.webhook.suspicious_url"`.
- **T-V12-WH-16:** Configure `webhook_url = "http://internal-admin/"`; assert delivery succeeds (no block, only warn) — locks the v1.2 "inform-don't-block" posture against accidental future filters.
- **T-V12-WH-17:** `cronduit check` against a config with a 127.0.0.1 webhook URL emits the same warning text as runtime startup.
- **T-V12-WH-18:** `THREAT_MODEL.md` contains a "Threat Model 5: Webhook Outbound" section with the four bullet points above (audit-driven test).

**Severity:** CRITICAL — failing to document this as an accepted trade-off at v1.2 ship time is worse than the technical risk itself; the threat model gap is the bug.

---

### Pitfall 32 — CRITICAL — HTTP-vs-HTTPS posture (forced HTTPS? loopback exception?)

**Where:** Same.

**What goes wrong:** Operator sets `webhook_url = "http://my-receiver.example.com/hook"`. The HMAC-signed body is now sent in cleartext over the LAN/internet. A passive observer (Wi-Fi sniffer, ISP, intermediate router) sees the entire payload — which includes job names, run IDs, exit codes, and (depending on payload schema) potentially the failure error message verbatim. Even with HMAC signing, the body is **not confidential**, only authenticated.

Conversely: operator's receiver runs on `http://localhost:8080/hook` or `http://192.168.1.5:8080/hook` (homelab common case). Forcing HTTPS would block the most common homelab pattern.

**Why:** Same tension as Pitfall 31. Cronduit's posture is "trust the LAN, document the trade-off." Forcing HTTPS by default would break legitimate homelab use; allowing HTTP everywhere silently weakens the security story.

**When it manifests:** Configuration time (operator picks a URL). Latent unless monitored.

**Prevention (v1.2 stance — lock at requirements):**

1. **Allow HTTP and HTTPS at the URL level.** No scheme enforcement. Consistent with the SSRF stance.
2. **Startup warn (the same WARN as Pitfall 31):**
   - URL scheme is `http://` AND host is **not** loopback (`127.0.0.0/8`, `::1`) AND **not** RFC1918 (`10/8`, `172.16/12`, `192.168/16`) → `WARN target="cronduit.webhook.suspicious_url"` with reason `"http_to_public_address"`.
   - URL scheme is `http://` AND host **is** loopback or RFC1918 → silent (this is the homelab default, no warn).
   - URL scheme is `https://` → silent.
3. **Document the policy in the webhook README section.** Three-line decision table:
   - `https://anywhere` → quiet, recommended.
   - `http://localhost`, `http://192.168.x.x`, `http://10.x.x.x` → quiet, common homelab.
   - `http://public.example.com` → WARN at startup (still works).
4. **TLS implementation: rustls-only.** v1.0's "no openssl-sys in `cargo tree -i`" invariant must be preserved. The `hyper-util` client in cronduit health (Phase 12) already uses rustls; reuse the same client builder for the webhook worker. Lock with the existing CI guard.

**Phase hint:** Phase 18 (with Pitfall 31).

**Test cases:**
- **T-V12-WH-19:** `https://example.com/hook` → no warn.
- **T-V12-WH-20:** `http://192.168.1.5/hook` → no warn (RFC1918 silent).
- **T-V12-WH-21:** `http://example.com/hook` → WARN with `reason="http_to_public_address"`.
- **T-V12-WH-22:** `cargo tree -i openssl-sys` empty after webhook worker addition (regression lock against pulling reqwest with default-features-on).

**Severity:** CRITICAL (posture clarity required at ship; technical implementation is small).

---

### Pitfall 33 — CRITICAL — Webhook payload schema must declare a version field on day 1

**Where:** New `src/scheduler/webhook/payload.rs`. The JSON shape is the public contract operator-built receivers will depend on for years.

**What goes wrong:** Cronduit ships v1.2 with payload:

```json
{ "job": "backup", "status": "failed", "exit_code": 1, "duration_ms": 5234 }
```

Operator builds a Slack receiver against it. v1.3 wants to add `image_digest` and `streak_position`. Receiver continues to work (tolerant JSON parsing — additive). v1.4 wants to rename `status` to `terminal_status` to disambiguate from the running-status concept. Now every operator receiver breaks. v1.4 maintainer hesitates to make the change because "we never declared a version"; the schema ossifies; breaking changes pile up; eventually a v2.0 must ship with `payload_v2` URL-keyed AND `payload_v1` for back-compat. **All of this is preventable by shipping a `payload_version` field on day one.**

**Why:** Public webhooks are an API. Every long-lived API regrets not versioning from day 1. (Stripe, GitHub, Slack — all have learned this lesson.)

**When it manifests:** v1.3 or v1.4, when the first additive-but-renaming change is proposed.

**Prevention:**

1. **Lock the v1.2 payload as v1:**
   ```json
   {
     "payload_version": "v1",
     "cronduit_version": "1.2.0",
     "delivered_at": "2026-04-25T12:34:56Z",
     "job": {
       "name": "backup-postgres",
       "tags": ["backup", "weekly"]
     },
     "run": {
       "id": 42,
       "job_run_number": 3,
       "trigger": "scheduled",
       "start_time": "2026-04-25T12:34:00Z",
       "end_time": "2026-04-25T12:34:56Z",
       "duration_ms": 5234,
       "status": "failed",
       "exit_code": 1,
       "error_message": "container exited with status 1",
       "image_digest": "sha256:abc123...",
       "streak_position": "first_failure"
     },
     "context": {
       "consecutive_failures": 1,
       "last_success_run_id": 41,
       "last_success_at": "2026-04-25T11:34:56Z"
     }
   }
   ```
2. **Future schema rules (document at ship time):**
   - **Additive changes** keep `payload_version: "v1"`. Operators tolerate unknown fields (standard JSON parsing).
   - **Breaking changes** (rename, type change, semantic shift) bump to `"v2"` and the v1 shape is preserved on a parallel code path for at least one full milestone.
   - The `payload_version` field is a **string** ("v1", "v2"), not a number — easier to grep, less error-prone in semver-style comparisons.
3. **Document the contract in the README** under a `## Webhooks` section with the full v1 schema as a code block. The `payload_version` field is bullet point 1.
4. **Anti-feature: NO Jinja-style payload templating.** Operators may want `payload_template = "${run.status} for ${job.name}"`. Refuse: every templating engine is a footgun (XSS, injection, surprise type coercion), and "let the receiver shape its own messages from the canonical payload" is the right separation of concerns. Document this as out-of-scope in the v1.2 requirements doc and link from the webhook README.

**Phase hint:** Phase 16 (payload schema). Lock the schema before any receiver implementation in tests.

**Test cases:**
- **T-V12-WH-23:** Snapshot test of the v1 payload shape: load a fixture run, render the payload, assert byte-for-byte equality with a checked-in `tests/fixtures/webhook_payload_v1.json`. Future schema additions update the fixture in the same PR; future schema breaks fail the test loudly.
- **T-V12-WH-24:** `payload_version` field is present and equal to `"v1"` on every webhook delivery.
- **T-V12-WH-25:** Anti-feature test (or doc audit): grep `examples/cronduit.toml` for "payload_template" → must not appear.

**Severity:** CRITICAL — preventable now, expensive forever after.

---

### Pitfall 34 — CRITICAL — Retry storm without jitter

**Where:** Webhook delivery worker retry policy (3 attempts, exponential backoff per the v1.2 milestone scope).

**What goes wrong:** Naive exponential backoff: attempts at +1s, +5s, +25s. 50 jobs all fire at the top of the minute, all webhooks fail (receiver overloaded). Without jitter, all 50 retries fire simultaneously at +1s — *making the receiver-side overload worse*. At +5s, all 50 fire again. Thundering herd against the operator's webhook receiver, which would have recovered if the load had been spread.

**Why:** This is the canonical "thundering herd against a struggling backend" pattern. Every retry library that doesn't include jitter by default is a footgun.

**When it manifests:** Any sustained outage of the operator's webhook receiver. Operator reports "my Slack receiver crashed and now cronduit is the reason it can't recover."

**Prevention:**

1. **Jitter from day 1, not as a post-incident patch.**
2. **Algorithm: full jitter** (the AWS architecture-blog recommendation):
   - Attempt N back-off = `random(0, base * 2^N)` where base = 1 second, N ∈ {0, 1, 2}.
   - Bounded total wait < 4s + 8s + 16s = 28s worst case across 3 retries.
3. **Use `rand 0.9`** (already in the dep tree from v1.1's bump). `rand::thread_rng().gen_range(0..max)`.
4. **Document the policy in the README**: "Cronduit retries failed webhooks 3 times with full-jitter exponential backoff (max 28s total). After 3 failed attempts, the webhook is dropped — cronduit does NOT queue webhooks across restart."
5. **`cronduit_webhook_attempts_total{outcome}`** counter (`outcome ∈ {success, retry, exhausted}`) so operators can dashboard their retry rate.

**Phase hint:** Phase 15 (in the same PR as the worker).

**Test cases:**
- **T-V12-WH-26:** Stress-test 100 simultaneous failures against a counting mock receiver; assert delivery times are spread across the backoff windows (not clustered at exact second boundaries).
- **T-V12-WH-27:** Single-failure test: assert 3 attempts then exhaustion; assert `cronduit_webhook_attempts_total{outcome="exhausted"}` increments by 1.
- **T-V12-WH-28:** Recovery test: receiver returns 500 once then 200 on retry; assert delivery succeeds in 2 attempts.

**Severity:** CRITICAL.

---

### Pitfall 35 — MODERATE — Webhook delivery success ≠ end-to-end notification success

**Where:** Documentation. The webhook delivery returns 200 OK; cronduit logs success; operator never sees the alert because the receiver dropped the message internally (Slack rate limit, PagerDuty deduplication, receiver-side bug).

**What goes wrong:** Operator runs incident post-mortem: "why didn't I get paged?" Cronduit logs say `webhook_delivered: true, status_code: 200`. Operator assumes cronduit failed. Cronduit didn't fail — the receiver swallowed the alert.

**Why:** Cronduit's contract terminates at the HTTP response code. End-to-end delivery (the human getting paged) is the receiver's responsibility. Without explicit documentation, operators conflate the two.

**When it manifests:** First incident where the operator expected a webhook page that didn't materialize.

**Prevention:**

1. **README `## Webhooks` § "What cronduit guarantees" — explicit promise:**
   - **Promised:** the HTTP request was made; the response code was N; the body was the payload above; the headers included `X-Cronduit-Signature` and `X-Cronduit-Timestamp`.
   - **NOT promised:** the operator's receiver delivered the alert; the operator saw it; rate limits / dedup / receiver bugs are out of scope.
2. **Guidance in the troubleshooting section:** "If you didn't get paged, check (a) cronduit's `cronduit_webhook_attempts_total{outcome}` metric — did the delivery succeed at the HTTP layer? (b) your receiver's logs — did it process the message? (c) your downstream paging system."
3. **Threat model addition (with Pitfall 31):** add to "Threat Model 5: Webhook Outbound" — "cronduit's delivery contract is HTTP-level; end-to-end delivery is the receiver's responsibility."

**Phase hint:** Phase 19 (webhook docs + threat model finalization).

**Test cases:** N/A — documentation. Phase plan checklist item.

**Severity:** MODERATE.

---

### Pitfall 36 — MODERATE — Webhook drop on shutdown is silent unless surfaced

**Where:** Same delivery worker.

**What goes wrong:** Operator runs `docker compose down`; cronduit drains for `webhook_drain_timeout` (10s); 3 webhooks are still queued; they're dropped. The operator never sees them. Worse: the operator might ASSUME cronduit either delivered them or persisted them to retry on next boot. **Cronduit does neither** — and that needs to be obvious.

**Why:** Webhook deliveries are not persisted to the DB (deliberate — adds a write path that competes with the log pipeline + retention pruner). v1.2 ships best-effort delivery only.

**When it manifests:** Restart / upgrade / crash during a webhook burst.

**Prevention:**

1. **`cronduit_webhook_dropped_total{reason="shutdown_drain"}`** is logged at INFO at shutdown with the count: `"webhook drain: delivered N, dropped M to shutdown_drain"`.
2. **README section "Webhook delivery is best-effort":** "Cronduit does not persist webhook deliveries to disk. Webhooks queued at shutdown / crash are lost. If you need at-least-once webhook delivery, your receiver should be the source of truth (poll cronduit's `/api/runs` instead of relying on the push)."
3. **Future v1.3 candidate:** persist webhook queue to disk. Out of scope for v1.2 — explicitly note in the requirements doc.

**Phase hint:** Phase 19.

**Test cases:**
- **T-V12-WH-29:** Send SIGTERM with N=5 queued webhooks against a mock receiver that takes 100s; assert log line `"webhook drain: delivered X, dropped Y to shutdown_drain"` with X+Y=5.

**Severity:** MODERATE.

---

### Pitfall 37 — MODERATE — Per-job HMAC secret rotation requires reload

**Where:** Same.

**What goes wrong:** Operator rotates the env var that backs `webhook_hmac_secret = "${WEBHOOK_SECRET}"`. The new secret is in the env; the running cronduit process holds the old `SecretString` from its last config-load. Until SIGHUP / file-watch / `POST /api/reload` fires, cronduit signs with the old secret. The receiver (which now expects the new secret) rejects the signatures. Webhooks silently fail until the operator reloads.

**Why:** v1.0's `${ENV_VAR}` interpolation happens at config-load time, not per-request. Same pattern as DB credentials, image registry secrets, etc. Operators may not realize webhook secrets follow the same rule.

**When it manifests:** Secret rotation procedure.

**Prevention:**

1. **Document in the README "Secret rotation" section:**
   - "Secrets are interpolated at config-load time. After updating an env var, run `kill -HUP $(pidof cronduit)` or `POST /api/reload` to pick up the new value."
   - "Cronduit signs with one secret at a time. During rotation, your receiver SHOULD accept both old and new signatures for a brief window (see receiver examples)."
2. **`cronduit health`** does NOT verify webhook secrets (out of scope for healthcheck — consistent with v1.1's "health = HTTP availability" stance).
3. **Optional v1.3 candidate:** per-secret cache invalidation on env-var change. Out of scope for v1.2.

**Phase hint:** Phase 19.

**Test cases:**
- **T-V12-WH-30:** Change the env var backing a webhook secret; fire a webhook; assert it's signed with the OLD secret (regression lock for the documented behavior). Then reload; fire another; assert it's signed with the NEW secret.

**Severity:** MODERATE.

---

### Pitfall 38 — MINOR — Webhook URL gets logged at ERROR on transport failure (secret leak risk)

**Where:** `tracing::error!` calls inside the webhook worker.

**What goes wrong:** Naive logging: `tracing::error!(url = %webhook.url, "webhook delivery failed: {e}")`. If the operator embedded a credential in the URL (`https://user:pass@receiver/hook` — bad practice but possible), the credential lands in cronduit's logs. v1.0's `strip_db_credentials` (T-I1) handled this for DB URLs; webhook URLs need the same treatment.

**Why:** URL-embedded credentials are a footgun; cronduit must defend against operator self-pwn.

**When it manifests:** Any error path that logs the webhook URL. Long-running deployment risk.

**Prevention:**

1. **`strip_url_credentials(url: &str) -> String`** helper in `src/util/url.rs` (or extend the existing `strip_db_credentials` in `src/db/mod.rs` with a generic version). Strips `userinfo` from the URL before logging.
2. **Lock with a code-search test:** every `tracing::error!` / `tracing::warn!` mention of a webhook URL must go through `strip_url_credentials`. Same shape as the v1.0 `strip_db_credentials` lock.
3. **`SecretString` for webhook URLs?** Probably overkill — the URL itself isn't secret, only the userinfo portion. Strip rather than wrap.

**Phase hint:** Phase 17 (alongside HMAC signing — both are "secret-handling" PRs).

**Test cases:**
- **T-V12-WH-31:** Log capture test: configure `webhook_url = "https://user:pass@example.com/hook"`; trigger a delivery error; assert the captured log does NOT contain `pass`.

**Severity:** MINOR (footgun defense, not a known live bug).

---

## Feature 2 — Custom Docker Labels (SEED-001)

The design is mostly locked at seed time (merge semantics, reserved namespace, type-gating). Pitfalls below are about ENFORCEMENT — making sure the locked design actually executes correctly under all the operator-misuse paths.

### Pitfall 39 — CRITICAL — `cronduit.*` reserved-namespace validator must run at config-load, not runtime

**Where:** New validator in `src/config/validate.rs` (parallels `check_cmd_only_on_docker_jobs` at `validate.rs:89`); call site in `validate_jobs` per-job loop (`validate.rs:22`); seed point at `src/scheduler/docker.rs` L146–L149 where `cronduit.run_id` and `cronduit.job_name` are inserted.

**What goes wrong:** Operator config:
```toml
[[jobs]]
name = "evil"
type = "docker"
labels = { "cronduit.run_id" = "999999" }
```

If the validator runs only at runtime (when the executor builds the Bollard `Config::labels` HashMap), the label collision is detected per-fire — too late, and the executor either (a) lets the operator value win and **breaks orphan reconciliation** (`src/scheduler/docker_orphan.rs:48` reads `cronduit.run_id` to map back to the DB row; an operator-supplied collision makes reconciliation update the wrong row OR fail to update any row), or (b) cronduit's value wins and the operator silently loses their label without an error (the SEED-001-locked behavior is "fail validation," not "silently override").

**Why it's CRITICAL not just MODERATE:** orphan reconciliation is the safety net that prevents `job_runs` rows from being stuck in `status='running'` forever after a crash. The `mark_run_orphaned WHERE status='running'` guard (v1.1 Research Correction #4, locked by `tests/docker_orphan_guard.rs`) protects against the *opposite* race; an operator-injected `cronduit.run_id="999999"` could mark the wrong row as orphaned during reconciliation, which DOES match `WHERE status='running'` if row 999999 happens to be a real running run. That's a data-integrity bug.

**When it manifests:** Config-load — IF the validator is in the right place. Otherwise: at first fire of the misconfigured job, with downstream reconciliation corruption.

**Prevention:**

1. **Validator runs in `validate_jobs` BEFORE any executor sees the job.** Same call site as `check_cmd_only_on_docker_jobs`. Job upsert into the DB happens AFTER `validate_jobs` returns OK.
2. **Validation is case-insensitive on the prefix check, AND trims whitespace.** Operators could write `Cronduit.run_id`, `cronduit.run_id `, `cronduit.run_id\t`. The Docker labels API itself is case-sensitive on label keys, but the *intent* of "cronduit.* is reserved" must be defended against accidental case variation. Concretely:
   ```rust
   let key_lower = key.trim().to_lowercase();
   if key_lower.starts_with("cronduit.") {
       return Err(...);
   }
   ```
   Note: this rejects `Cronduit.foo` as well; that's deliberate. If an operator legitimately needs `Cronduit.foo` (some other tool's namespace that happens to capitalize), they can use `cronduit_foo.bar` or any other prefix.
3. **Trailing-whitespace edge case:** TOML deserialization of `labels = { "cronduit.foo " = "x" }` — verify whether `serde-toml` strips trailing whitespace on map keys (it should NOT, as TOML keys are explicit). Test it directly. Update validator to handle.
4. **Reserved key list MUST be the source of truth** — store as a `const RESERVED_LABEL_PREFIXES: &[&str] = &["cronduit."]` in a single module so both the validator and the runtime label-builder reference it. Future additions (`cronduit-internal.*`?) update one place.
5. **Validator runs from `cronduit check`** so operators catch the error before deployment.

**Phase hint:** Phase 20 (Custom Docker labels — the SEED-001 implementation).

**Test cases:**
- **T-V12-LBL-01:** `labels = { "cronduit.run_id" = "x" }` on a docker job → `cronduit check` exits non-zero with a GCC-style error pointing at the offending key.
- **T-V12-LBL-02:** `labels = { "Cronduit.foo" = "x" }` (capitalized) → same rejection.
- **T-V12-LBL-03:** `labels = { "cronduit.foo " = "x" }` (trailing space) → same rejection.
- **T-V12-LBL-04:** `labels = { "my.cronduit.foo" = "x" }` (cronduit. is NOT a prefix here) → ACCEPTED. Validator must check `starts_with`, not `contains`.
- **T-V12-LBL-05:** Runtime regression: even if validator drift introduces a bypass, the runtime `Config::labels` builder MUST still refuse to insert operator labels with the reserved prefix (defense in depth — a `tracing::error!` + skip the label, do NOT panic).

**Severity:** CRITICAL.

---

### Pitfall 40 — CRITICAL — Type-gate validator must reject `labels` on command/script jobs

**Where:** Same validator module. Parallels v1.0.1's `check_cmd_only_on_docker_jobs` validator (the patch shipped after v1.0.0 to prevent `cmd = ["..."]` on a non-docker job from silently being ignored — same pattern, same risk class).

**What goes wrong:**
```toml
[[jobs]]
name = "backup"
type = "script"
script = "..."
labels = { "team" = "platform" }
```
Operator expects... what, exactly? There's no container; the labels have nowhere to go. If cronduit silently ignores the labels, the operator believes their `team=platform` cost-allocation tag is in place — but `docker ps --filter label=team=platform` returns nothing. **Silent semantic drop.** v1.0.1 hit the exact same shape with `cmd` on non-docker jobs and shipped a validator; v1.2 must do the same for `labels`.

**Why:** Same root cause as v1.0.1's `cmd` pitfall — TOML schema permissiveness lets fields land anywhere; the type-gate validator is the only guard.

**When it manifests:** Config load — IF the validator exists. Otherwise: silent forever.

**Prevention:**

1. **Validator in `validate_jobs` enforces:** `labels` is non-empty → `type` must be `"docker"`. Other types: `labels` must be unset OR empty map.
2. **Error message format matches v1.0.1's `cmd` validator** exactly (operators have already learned the shape). E.g., `error: 'labels' is only valid for jobs with type = "docker", but job 'backup' has type = "script"`.
3. **Validator runs BEFORE upsert.** Same enforcement timing as Pitfall 39.

**Phase hint:** Phase 20.

**Test cases:**
- **T-V12-LBL-06:** `type = "command"` + non-empty `labels` → reject at config-load.
- **T-V12-LBL-07:** `type = "script"` + non-empty `labels` → reject.
- **T-V12-LBL-08:** `type = "docker"` + non-empty `labels` → accept.
- **T-V12-LBL-09:** `type = "command"` + empty `labels = {}` → accept (trivial).

**Severity:** CRITICAL (data-integrity / silent-misconfig — exactly the class of bug v1.0.1 already shipped a fix for; v1.2 must not regress the principle).

---

### Pitfall 41 — MODERATE — Label value size limits + total label-set size

**Where:** Same validator + runtime safety net.

**What goes wrong:** Operator does `labels = { "annotations" = "<100KB JSON blob>" }`. Docker has practical (not strict in spec) limits on label values; in practice:
- Docker daemon accepts up to ~256KB total per container's `inspect` payload, including labels.
- Some downstream tooling (Watchtower, some Traefik versions) chokes on individual label values >4KB.
- The operator's intent is almost certainly an error, not a deliberate giant value.

If cronduit accepts arbitrarily large values:
1. Bollard's `Config::labels` HashMap insertion proceeds.
2. `create_container` may fail with a poorly-mapped error from the daemon — or succeed but produce a container the operator's other tooling can't read.
3. The cronduit DB stores the resolved config (`config_json`) including the giant blob — bloating the `jobs` table.
4. The error surfaces at first fire, far from the operator's edit.

**Why:** Docker label limits are not standardized but practical; cronduit should defend at config-load with a conservative cap.

**When it manifests:** Config edit by an operator pasting in a large block. Or by an operator using `${ENV_VAR}` interpolation on a large env var.

**Prevention:**

1. **Per-label cap at config-load:** value size ≤ 4096 bytes (UTF-8 byte length, not chars). Reject above with a clear error pointing at the offending key.
2. **Total label-set cap per job:** sum of all (key + value) lengths ≤ 32 KB. Reject above.
3. **Runtime defense-in-depth:** if a value somehow slipped past validation (e.g., env-var interpolation after validator? Check.), the runtime label-builder logs `WARN target="cronduit.labels.oversized"` and SKIPS the label; container creation proceeds with the rest.
4. **Bollard error mapping:** capture the daemon's error response on `create_container` failures involving labels and surface as `RunStatus::Error` with a clear message — don't let it become a generic "create container failed."

**Phase hint:** Phase 20.

**Test cases:**
- **T-V12-LBL-10:** `labels = { "x" = "<4097-byte string>" }` → reject at config-load.
- **T-V12-LBL-11:** 10 labels each 4 KB → 40 KB total → reject (above 32 KB total cap).
- **T-V12-LBL-12:** Config with `${BIG_ENV}` where `BIG_ENV=<5KB>` at runtime → if interpolation happens before validation, rejected; if after, runtime WARN + skip. Document which.

**Severity:** MODERATE.

---

### Pitfall 42 — MODERATE — `[defaults] + per-job` merge: insertion order must be deterministic and tested

**Where:** `src/config/defaults.rs::apply_defaults` (line 112 per SEED-001) — extended for the labels case, parallel to existing override fields.

**What goes wrong:** Bollard's `Config::labels` is `Option<HashMap<String, String>>`. HashMap iteration order in Rust is randomized per-run (hash DoS protection). If anything in cronduit's path or in downstream tooling depends on insertion order:
- Test snapshots of `Config::labels` JSON serialization will be flaky.
- Some daemon-side tools that hash-stamp labels may produce different stamps run-to-run.

In practice: Docker labels are unordered semantically. The risk is in **tests** (snapshot tests of the JSON sent to bollard) and in **cronduit's own logging** (label dumps in error messages that operators grep).

**Why:** HashMap iteration randomization is correct behavior, but tests and logs both implicitly assume order.

**When it manifests:** CI flake on snapshot tests; operator reports "the order keeps changing in the logs."

**Prevention:**

1. **Use `BTreeMap<String, String>` for the merged label set** (keys sorted alphabetically, deterministic order). Bollard's `Config::labels` is `HashMap<String, String>` so the final `into()` conversion is unavoidable, but the merge step itself uses `BTreeMap` for determinism.
2. **Snapshot tests sort keys before comparison:** any test that checks the resolved labels for a job sorts the keys explicitly before assert.
3. **Log output sorts labels:** `tracing::info!` lines that include label dumps use a sorted iterator.
4. **Override priority lock:** the merge is `defaults ∪ per-job`, with per-job winning on key collision. Document and lock with tests parallel to `apply_defaults_use_defaults_false_disables_merge` (`defaults.rs:316`).
5. **`use_defaults = false`** REPLACES the entire defaults map (per SEED-001 lock) — test the replace semantics with an explicit check that defaults labels are absent in the resolved config.

**Phase hint:** Phase 20.

**Test cases:**
- **T-V12-LBL-13:** `defaults.labels = { "team" = "ops", "env" = "prod" }`, `[[jobs]] labels = { "team" = "platform" }`, `use_defaults = true` → resolved labels = `{ "team" = "platform", "env" = "prod" }`.
- **T-V12-LBL-14:** Same setup with `use_defaults = false` → resolved labels = `{ "team" = "platform" }` only.
- **T-V12-LBL-15:** Snapshot test of resolved labels for a job with 5 labels asserts sorted-key-order JSON output (deterministic).

**Severity:** MODERATE.

---

### Pitfall 43 — MINOR — Label keys with TOML-special characters (dots, dashes, slashes)

**Where:** TOML parser + label-key validation.

**What goes wrong:** `labels = { "com.centurylinklabs.watchtower.enable" = "false" }` — TOML allows quoted keys with dots, but if the operator forgets quotes (`com.centurylinklabs.watchtower.enable = "false"`), TOML parses it as a NESTED TABLE, not a single key. The error message can be cryptic.

**Why:** TOML's bare-key vs quoted-key rules are subtle.

**When it manifests:** Operator edits config to add a Traefik/Watchtower label and forgets quotes.

**Prevention:**

1. **Examples in `examples/cronduit.toml`** show quoted keys: `"com.centurylinklabs.watchtower.enable" = "false"`. Operators copy-paste from examples.
2. **README `## Configuration` § "Custom Docker labels"** explicitly notes: "Label keys with dots must be quoted. The TOML parser otherwise treats them as nested tables."
3. **`cronduit check` error message** for a config that accidentally produced a nested table where labels were expected — surface a hint: `"hint: label keys with dots must be quoted"` if a `labels.com` table is detected anywhere.

**Phase hint:** Phase 20 (with examples).

**Test cases:**
- **T-V12-LBL-16:** Config with quoted dot-key → parsed correctly.
- **T-V12-LBL-17:** Config with unquoted dot-key under `labels` → `cronduit check` emits the hint message.

**Severity:** MINOR.

---

## Feature 3 — Failure Context (image-digest + config-hash + streak)

**Largest schema-impact feature in v1.2** because it adds a per-run column (`image_digest`) that doesn't exist today, AND it surfaces a config-hash semantic gap: `jobs.config_hash` is per-JOB only, not per-run, so "config changed between two successful runs" is invisible without a schema change.

### Pitfall 44 — CRITICAL — `config_hash` is recorded per-JOB, not per-RUN — hot-reload is invisible

**Where:** `migrations/sqlite/20260410_000000_initial.up.sql` L23 — `jobs.config_hash`. There is NO `job_runs.config_hash` column. Verified via grep — `config_hash` appears only in `jobs` schema and in `src/config/hash.rs` (compute), never on `job_runs`.

**What goes wrong:** The v1.2 failure-context feature wants to render: "config changed since last successful run." Naive implementation queries `jobs.config_hash` and compares to... what? The previous run's config_hash isn't recorded. Two failure modes:

1. **Hot reload between two successful runs is invisible.** Job runs at 12:00 with hash A → success. Operator hot-reloads with a new value → hash B. Job runs at 13:00 with hash B → success. UI shows "no config change since last success" (wrong — config DID change between the two runs).
2. **Reload mid-failure-streak collapses the streak signal.** Job fails at 12:00 (hash A). Operator changes the config to fix it (hash B). Job runs at 12:01 with hash B → success. UI shows "1 failure, recovered" but doesn't tell the operator that the recovery was due to a config change, not a flake. Important diagnostic context is lost.

**Why:** `jobs.config_hash` was introduced in v1.0 for the upsert-on-change path (`sync_config_to_db` uses it to detect "did this job's config change between reloads"). It was never wired into `job_runs` because no v1.0/v1.1 feature needed per-run config provenance. v1.2 is the first feature that does.

**When it manifests:** Any deployment with config reloads. The longer the deployment, the more reloads, the more invisible config-change events.

**Prevention:**

1. **New column: `job_runs.config_hash TEXT NULL`** (nullable for backfill compatibility). Three-file migration per backend, parallel to v1.1's `job_run_number` migration:
   - `20260501_000001_job_runs_config_hash_add.up.sql` — ADD COLUMN nullable.
   - `20260502_000002_job_runs_config_hash_backfill.up.sql` — `UPDATE job_runs SET config_hash = (SELECT j.config_hash FROM jobs j WHERE j.id = job_runs.job_id)` — best-effort backfill from the *current* job's hash. Document that backfilled values represent "the config_hash at backfill time," not "at run time" — backfilled rows should be visually distinguished in the UI ("config history not available before upgrade").
   - **Skip the NOT NULL step.** Old runs may legitimately lack `config_hash` (if backfill was skipped or partial); keep nullable forever. UI shows "—" for null. Same pattern as v1.1's image-digest column will use (see Pitfall 45).
2. **Write site:** `insert_running_run` in `src/db/queries.rs` (≈L286–L313) takes `config_hash: &str` from the resolved job config at fire time. The hash is captured BEFORE the executor spawns, so even if a reload happens mid-fire, the row reflects the config that the run was based on.
3. **UI rendering:** failure-context card on run detail compares `this_run.config_hash` to `last_success_run.config_hash` — same hash = "no config change since last success"; different hash = "config changed since last success" with a link or hover-text showing the high-level diff (or just "config changed at $reload_time" if we can't render diff easily).
4. **Test the backfill carefully** — large DB scenarios (≥100k runs) take time, parallel to v1.1's `job_run_number` backfill. Re-use the chunked-backfill pattern from v1.1 (`UPDATE … WHERE config_hash IS NULL LIMIT 10000` loop with INFO progress logs; see v1.1 Pitfall 5.3).

**Phase hint:** Phase 21 (failure context — schema + queries). The migration must land BEFORE the UI card in the same milestone.

**Test cases:**
- **T-V12-FCTX-01:** Migration: empty DB → all migrations run → `job_runs.config_hash` exists nullable.
- **T-V12-FCTX-02:** Backfill: seed 1000 runs across 3 jobs; run backfill migration; assert all 1000 rows have `config_hash` set to their job's current hash.
- **T-V12-FCTX-03:** Write site: fire a run → assert `job_runs.config_hash` matches the resolved job's `config_hash` at fire time.
- **T-V12-FCTX-04:** Reload between fires: fire run A, reload config (changing hash), fire run B → assert run A and run B have different `config_hash` values.
- **T-V12-FCTX-05:** UI: render failure-context card with `last_success.config_hash != this_run.config_hash` → text says "config changed since last success."
- **T-V12-FCTX-06:** Backfilled-row UI: render a row whose `config_hash` was set by backfill (use a marker or just "—" if uncertain) → distinguishable from real per-run captures.

**Severity:** CRITICAL — without this, the "config changed since last success" signal is fundamentally unreliable.

---

### Pitfall 45 — CRITICAL — Image-digest capture: which call yields the digest of the container that ACTUALLY ran?

**Where:** `src/scheduler/docker.rs` L240–L250 currently captures `image_digest` via `inspect_container(&container_id)` AFTER `start_container` (the existing v1.0 capture from `DockerExecResult.image_digest`). The captured field is `info.image` — confirmed at `docker.rs:241`. Per-run write site for v1.2 will be in `finalize_run` or earlier.

**What goes wrong (subtle):** Operator pulls `nginx:latest` Tuesday morning → digest A locally. Tuesday-night cron pulls `nginx:latest` again → digest B. Wednesday morning's run starts: `ensure_image` sees the image is already present (cache hit at digest B), runs the container, `inspect_container` returns digest B. Cronduit reports "ran at digest B." Correct.

But the failure-context card asks: "what was the digest at the LAST successful run?" — and that data point comes from `job_runs.image_digest` of the previous row, which was captured at THAT run's `inspect_container` call. So if Tuesday morning's run reported A and Wednesday morning's reported B, the failure-context card correctly shows "image changed from A to B between runs" — even though the operator just sees "nginx:latest" both times.

**Where it goes wrong:**

1. **`inspect_container` returns the IMAGE the container was created from**, not the image the daemon currently has at that tag. So if the cache evolves between create and inspect, you still capture the create-time digest. Good — but only if cronduit captures BEFORE the container is removed. v1.0's existing capture happens after `start_container` but BEFORE `remove_container`. Lock that ordering.
2. **`info.image` field** — verify the bollard 0.20 ContainerInspectResponse schema. The field is the image-by-id (not by-name) — i.e., the SHA256 digest cronduit needs. `image_id` is the alternate field (older bollard versions). Confirm in the test.
3. **Pre-flight pull race:** if `ensure_image` (`docker_pull.rs`) returns one digest but the daemon caches a different one between pull and `create_container`, you have a digest mismatch. v1.0's pre-flight returns its own digest (`docker.rs:129–144` `_image_digest` is captured but unused — note the `_` prefix). v1.2 should USE that pre-flight digest as the authoritative value, not the post-create inspect, because pre-flight is closer to the actual pull event.
4. **For images with `image = "alpine:latest"` and no registry digest at all** (rare but possible — operator built locally), `info.image` is the local content-hash sha256 (still a valid sha256:... string, just not from a registry). UI must accept any sha256 prefix and not break on "looks weird."

**Why:** Multiple capture sites with subtly different timings; the right one is "what did `docker create` use," and that's `inspect_container.info.image` if invoked after create.

**When it manifests:** Any environment with a moving tag (`nginx:latest`, `node:lts`) and a sustained job. Probably immediate on first deployment.

**Prevention:**

1. **Authoritative capture site:** the existing `inspect_container(&container_id)` post-`start_container` at `docker.rs:240`. Plumb the captured digest through to `job_runs.image_digest` via the executor's return path (`DockerExecResult.image_digest` already exists — wire it to the DB write).
2. **Schema:** `job_runs.image_digest TEXT NULL` (nullable; command/script jobs have no digest, write NULL).
3. **Format:** raw sha256 string from bollard (e.g., `"sha256:abc123..."`). Don't reformat or truncate at write time. UI may truncate to 12-char short form for display.
4. **Migration shape** (parallel to Pitfall 44):
   - File 1: ADD COLUMN nullable.
   - File 2: NO backfill — old rows stay NULL. Document: "Image digests are captured for runs after upgrade; pre-upgrade runs show '—' in the failure-context card."
   - File 3: NOT NULL step is **NEVER added.** Keep nullable forever — command/script jobs legitimately lack a digest.
5. **UI:** when a row's `image_digest` is NULL, render "—". When two consecutive successful docker runs have different digests, render "image changed: <short-A> → <short-B>" in the failure-context card.

**Phase hint:** Phase 21 (with config-hash; same migration block).

**Test cases:**
- **T-V12-FCTX-07:** Docker job → run → inspect `job_runs.image_digest` is non-null and starts with `sha256:`.
- **T-V12-FCTX-08:** Command job → run → `job_runs.image_digest` is NULL.
- **T-V12-FCTX-09:** Same docker job + image but different daemon pulls → digest changes captured correctly across runs (testcontainers integration test).
- **T-V12-FCTX-10:** Locally-built image (`docker build -t local-app .`, no registry) → cronduit captures the local content-hash sha256 without error; UI renders without crashing.
- **T-V12-FCTX-11:** Backfill is NOT performed; pre-upgrade rows have `image_digest = NULL`; UI shows "—".

**Severity:** CRITICAL.

---

### Pitfall 46 — MODERATE — Streak math + last-success queries: query plan + retention interaction

**Where:** New `src/db/queries.rs::get_failure_context(run_id) -> FailureContext` returning `{ first_failure_ts, consecutive_failures, last_success_run_id, last_success_at, image_digest_delta, config_hash_delta }`.

**What goes wrong (multiple sub-cases):**

1. **Naive streak query is O(N).** "How many consecutive failures ending at run X?" → `SELECT * FROM job_runs WHERE job_id = $1 AND start_time <= $2 ORDER BY start_time DESC LIMIT 1000` and walk in Rust until status='success'. For a job that succeeds every minute, the streak is always 1 — wasted query. For a backup job that runs daily and has been failing for 90 days = 90 rows. Fine. But for a 1-minute job that's been failing for an hour = 60 rows. Still fine. Edge case: a daily job that's been failing forever (3+ years of retention) = 1095+ rows. Still fine but the query plan must use the index.
2. **`idx_job_runs_job_id_start` index** (from v1.0 initial migration L46) covers `(job_id, start_time DESC)` — perfect for this. Verify with `EXPLAIN QUERY PLAN`.
3. **Last-success query** is similar: `SELECT * FROM job_runs WHERE job_id = $1 AND status = 'success' AND start_time < $2 ORDER BY start_time DESC LIMIT 1`. Same index.
4. **Retention interaction:** if retention has pruned old rows, `last_success` may legitimately be unknown (last success was 95 days ago, retention is 90 days). UI must render "—" or "older than retention" rather than crash.
5. **Failure-context for a command/script job** has no `image_digest` field; the streak math still works.

**Why:** Standard "walk back from this row" queries. The risk is in the query plan and the retention edge case, not the math itself.

**When it manifests:** First render of the failure-context card on any run-detail page.

**Prevention:**

1. **Two queries (not one big one):**
   ```sql
   -- Streak (count consecutive non-success runs ending at this one)
   SELECT id, status, start_time FROM job_runs
   WHERE job_id = $1 AND start_time <= $2
   ORDER BY start_time DESC
   LIMIT 1000;
   ```
   In Rust, walk the result and count until first non-failure (or 1000-row safety cap). Document the cap; in practice no real streak should reach 1000.

   ```sql
   -- Last success
   SELECT id, start_time, image_digest, config_hash FROM job_runs
   WHERE job_id = $1 AND status = 'success' AND start_time < $2
   ORDER BY start_time DESC
   LIMIT 1;
   ```
2. **`EXPLAIN QUERY PLAN` lock** in tests for both queries — must use `idx_job_runs_job_id_start`. CI regression on both SQLite and Postgres.
3. **Null-safe rendering:** if `last_success` query returns 0 rows (retention pruned, or no successes ever), UI renders "—" with hover text "no successful runs in retention window."
4. **Same query is used by webhooks (Pitfall 29 streak coalescing)** — extract into a shared helper. See cross-feature Pitfall 54.

**Phase hint:** Phase 21 (queries before UI).

**Test cases:**
- **T-V12-FCTX-12:** `get_failure_context` against a job with 5 consecutive failures → returns `consecutive_failures = 5`, correct first-failure timestamp.
- **T-V12-FCTX-13:** Job with 0 successes ever → `last_success` is None; UI renders "—".
- **T-V12-FCTX-14:** Retention pruned: simulate via DELETE; assert query is graceful.
- **T-V12-FCTX-15:** EXPLAIN: both queries use `idx_job_runs_job_id_start` on SQLite + Postgres.

**Severity:** MODERATE.

---

### Pitfall 47 — MODERATE — Backfill of `image_digest` post-upgrade (decision: don't)

**Where:** Migration `20260502_000002_job_runs_image_digest_*.up.sql`.

**What goes wrong:** Operator upgrades from v1.1.0 to v1.2.0 with 100k existing `job_runs` rows, none of which have `image_digest`. Tempting to backfill by hitting `inspect_container` for old rows... but the containers are long gone (especially if `delete = true`, which is the default). Backfill is impossible for completed runs.

If we attempt backfill (e.g., setting `image_digest = "unknown"` or copying from `jobs.config_json` parsed image-name), we're writing wrong-or-meaningless data. Worse: the UI then has to distinguish "real digest" from "backfill placeholder," adding complexity.

**Why:** Past container state is lost. There's no source of truth for old rows.

**When it manifests:** Upgrade time.

**Prevention:**

1. **Decision (lock):** NO backfill for `image_digest`. Old rows stay NULL forever.
2. **UI shows "—" for NULL `image_digest`** in the failure-context card. Document in the README v1.2 release notes: "Image-digest tracking begins at v1.2.0 upgrade. Pre-upgrade runs show '—'."
3. **Same approach is used for `config_hash` if backfill is risky** — but Pitfall 44 recommends a conservative backfill (current job hash) for config_hash because at least the `jobs.config_hash` column has a current value to copy. Image digest has no such fallback.
4. **Document the divergent backfill stance** between the two columns clearly in the migration files' comment headers.

**Phase hint:** Phase 21 (migration design — lock the no-backfill decision).

**Test cases:**
- **T-V12-FCTX-16:** Migration migrates 1000 v1.1-style rows; assert all 1000 still have `image_digest = NULL` after migration completes.
- **T-V12-FCTX-17:** Render failure-context card for a pre-upgrade row; assert "—" rendered, no crash.

**Severity:** MODERATE.

---

## Feature 4 — Per-Job Exit-Code Histogram

Smallest schema-impact feature (no new columns; queries existing `job_runs.exit_code`). Pitfalls cluster around (a) cardinality control, (b) null handling for stopped/timeout, (c) window choice.

### Pitfall 48 — CRITICAL — Exit-code cardinality explosion

**Where:** New `src/db/queries.rs::get_exit_code_histogram(job_id, window) -> Vec<(ExitCodeBucket, u32)>` and the UI card renderer.

**What goes wrong:** Exit code is `i32`. A misbehaving program can return any value (0..127, 128..255, also negative depending on shell semantics). A job whose script does `exit $RANDOM % 256` produces ~256 distinct exit codes over time. Naive histogram = 256 buckets. UI card becomes a wall of single-bar columns. Worse: if cronduit also exposes the histogram via `/metrics` (it shouldn't, see below), Prometheus cardinality blows up — exact case v1.0 explicitly designed against (FOUND-04: bounded-cardinality labels).

**Why:** Exit codes have no enforced range. Cronduit is at the mercy of operator code.

**When it manifests:** Any deployment with a job whose exit-code distribution is wide. Visible immediately on first card render for that job.

**Prevention:**

1. **Bucket strategy (LOCK):**

   | Bucket | Range | Semantic | Color |
   |--------|-------|----------|-------|
   | `success` | 0 | success | green |
   | `1` | 1 | catch-all error | red |
   | `2` | 2 | usage / mis-invocation | red |
   | `3-9` | 3..=9 | small custom error codes | red |
   | `10-126` | 10..=126 | other custom codes | red |
   | `127` | 127 | "command not found" (shell convention) | red |
   | `128-143` | 128..=143 | killed by signal (128 + signal_num for 1..=15) | orange (`stopped`-like) |
   | `144-254` | 144..=254 | other / app-defined | red |
   | `255` | 255 | sentinel "exit code -1" or generic | red |
   | `null` | NULL | timeout / stopped (no exit code) | distinct color |

2. **No `/metrics` exposure of the histogram.** UI card only. Rationale: Prometheus cardinality discipline (v1.0 lock) — `cronduit_runs_total{job, status}` already covers the broad-strokes count; per-bucket exit-code counters would add `~10 buckets × ~50 jobs = 500 series` for limited operational benefit.
3. **Window choice (LOCK):** last 100 ALL runs (NOT just successful). Exit codes are most useful for diagnosing failures, so failed/stopped/timeout rows must be in the window. Distinct from v1.1's p50/p95 card which uses last 100 SUCCESSFUL.
4. **Minimum sample threshold:** N ≥ 10 to render the card; below, render "—" with a tooltip "histogram needs at least 10 runs." Parallel to v1.1's sparkline `MIN_SAMPLES_FOR_RATE` constant; pull both into a single `src/web/stats.rs::MIN_SAMPLES_*` namespace.
5. **Bucket constants in code, not magic numbers.** A `const EXIT_CODE_BUCKETS: &[ExitCodeBucket]` that the histogram query and the UI card both consume. Future bucket changes update one place.

**Phase hint:** Phase 22 (exit-code histogram).

**Test cases:**
- **T-V12-EXIT-01:** 100 runs spread randomly across exit codes 0..255; assert histogram returns exactly 10 buckets (per the table above).
- **T-V12-EXIT-02:** Job with all `exit_code = 137` (SIGKILL'd) → renders in `128-143` bucket (orange).
- **T-V12-EXIT-03:** Job with `status = 'timeout'` (exit_code NULL) → counted in `null` bucket.
- **T-V12-EXIT-04:** Job with 5 runs → card shows "—" (below threshold).
- **T-V12-EXIT-05:** `/metrics` does NOT include exit-code histogram families (regression lock — `cronduit_exit_code_*` must not appear).

**Severity:** CRITICAL (cardinality discipline is a v1.0 invariant).

---

### Pitfall 49 — MODERATE — Null exit codes (timeout, stopped, error) need explicit bucket UX

**Where:** Same.

**What goes wrong:** Operator looks at the histogram for a job that has 30 `success` runs, 10 `failed (exit 1)` runs, and 5 `timeout` runs. If `null` exit codes are silently excluded (returning Some-only values), the histogram total is 40 — operator wonders "why doesn't this match my run count of 45?"

If `null` rows are bucketed as "0" or "—" without distinct visual treatment, they're confusable with success.

**Why:** `status = 'timeout'` and `status = 'stopped'` (v1.1) and `status = 'error'` rows have `exit_code IS NULL` because the process was killed before exit. They're a real and operationally-distinct outcome.

**When it manifests:** First render of a histogram for a job with any timeout/stopped runs.

**Prevention:**

1. **`null` bucket from Pitfall 48 is rendered as a distinct visual column** (e.g., diagonal-stripe pattern, distinct color from both green and red — purple/yellow/cyan). Hover text: "killed (no exit code): timeout, stopped, or error".
2. **Tooltip on the bucket** breaks down by status: `"timeout: 3, stopped: 1, error: 1"` so operators can dig deeper.
3. **Total at the top of the card:** "Last 100 runs, 5 with no exit code." Avoids the "where did 5 runs go?" surprise.

**Phase hint:** Phase 22.

**Test cases:**
- **T-V12-EXIT-06:** Card with 3 timeout + 1 stopped + 1 error runs → null bucket count = 5; tooltip breakdown shows "timeout: 3, stopped: 1, error: 1".

**Severity:** MODERATE.

---

### Pitfall 50 — MINOR — Window-edge effects on exit-code distribution

**Where:** Same.

**What goes wrong:** Window is "last 100 ALL runs." For a job that runs every minute, that's 100 minutes of history. For a daily backup, that's 100 days. Operator looking at the same UI card learns very different things from very different time windows. Without labeling, the card is misleading.

**Why:** Inherent to fixed-N rolling windows.

**When it manifests:** First reading of the card by an operator unfamiliar with the window.

**Prevention:**

1. **Card title** explicitly says "Last 100 runs" — no ambiguity.
2. **Hover/tooltip** shows the actual time span: "spanning 2026-04-22 to 2026-04-25" so operators can map to wall-clock.
3. **Empty-window edge:** if the job has no runs at all in retention, render "—" with "no runs available." Same UX as Pitfall 48 minimum-sample.

**Phase hint:** Phase 22.

**Test cases:**
- **T-V12-EXIT-07:** Card title contains "Last 100 runs" exactly (string assertion).
- **T-V12-EXIT-08:** Tooltip includes the actual time span.

**Severity:** MINOR.

---

## Feature 5 — Job Tagging / Grouping

UI-only feature (per v1.2 scope: tags do NOT affect webhooks, search, or metrics). Pitfalls cluster around normalization, charset, and filter UX.

### Pitfall 51 — MODERATE — Tag normalization (case + whitespace) collapses

**Where:** New `tags: Option<Vec<String>>` field on `JobConfig`. Normalization happens in `src/config/validate.rs` or a dedicated `src/config/tags.rs`.

**What goes wrong:**
```toml
[[jobs]]
name = "backup-1"
tags = ["Backup", "Daily"]

[[jobs]]
name = "backup-2"
tags = ["backup", "daily "]  # trailing space, lowercase

[[jobs]]
name = "backup-3"
tags = ["BACKUP", "DAILY"]
```

If cronduit treats these as 6 distinct tags, the dashboard filter chip bar renders 6 chips: `Backup`, `backup`, `BACKUP`, `Daily`, `daily `, `DAILY`. Operator clicks `Backup` and sees only 1 job. Confusing, immediate UX failure.

**Why:** Hand-written config + multiple operators editing → case-and-whitespace drift is inevitable.

**When it manifests:** Config edit by a second operator (or the same operator on a different day). Visible immediately on dashboard render.

**Prevention (LOCK at requirements):**

1. **Normalization at config-load:**
   ```rust
   fn normalize_tag(raw: &str) -> String {
       raw.trim().to_lowercase()
   }
   ```
   Apply during validation. Store the normalized value in the resolved job.
2. **Canonical form: lowercase + trimmed.** Document explicitly in the README and the requirements doc.
3. **Empty-after-normalize is rejected:** `tags = [""]` or `tags = ["   "]` → config-load error: `"empty tag in job 'backup'"`.
4. **Alternative considered + rejected:** preserve original case for display, normalize for filter matching. Rejected because it adds two-form complexity (display vs match) and operators get confused when "Backup" displays but `?tag=backup` filters — the URL form must match what they see. Lowercase-everywhere is simpler and more predictable.
5. **Lock with a config-load test** that asserts the normalized form is stored.

**Phase hint:** Phase 23 (tagging — TOML schema + normalization).

**Test cases:**
- **T-V12-TAG-01:** Job 1 with `tags = ["Backup", "Daily"]`, job 2 with `tags = ["backup", "daily "]` → both jobs resolve to `tags = ["backup", "daily"]`.
- **T-V12-TAG-02:** `tags = [""]` → config-load error.
- **T-V12-TAG-03:** `tags = ["  "]` (whitespace-only) → config-load error after trim.

**Severity:** MODERATE.

---

### Pitfall 52 — MODERATE — Tag charset + length validation

**Where:** Same validator.

**What goes wrong:**
```toml
tags = ["my very long tag with spaces and 🎉 emojis"]
```
Or:
```toml
tags = ["tag\nwith\nnewlines"]
```
Or:
```toml
tags = ["<script>alert(1)</script>"]
```

Three concerns:
1. **Display:** spaces/newlines/emojis render in the chip bar. Newlines break layout. Emojis are fine technically but inconsistent across browsers.
2. **URL safety:** `?tag=my+very+long+tag+with+spaces` works (URL encoding) but is ugly to share.
3. **XSS:** if the tag is rendered without HTML-escaping anywhere (e.g., in a URL `<a href="?tag=...">`), the `<script>` form is an XSS vector. **Cronduit's askama_web auto-escapes by default** (HIGH confidence — askama compile-time enforces this), so this is mitigated, but it's worth a regression-lock test.

**Why:** Tags are operator-controlled strings rendered into HTML and URLs.

**When it manifests:** Operator edit with a "fancy" tag.

**Prevention:**

1. **Charset (LOCK at requirements — restrictive but practical):** `^[a-z0-9][a-z0-9_-]{0,30}$` post-normalization.
   - Lowercase letters, digits, hyphen, underscore.
   - First character must be alphanumeric (no leading hyphen — avoids `--something` which looks like a CLI flag).
   - Length 1–31 characters.
   - No spaces, no dots, no emojis, no slashes, no special chars.
2. **Reject at config-load** with a clear error: `"tag 'my long tag' contains invalid characters (allowed: a-z, 0-9, hyphen, underscore; max 31 chars)"`.
3. **Trim + lowercase first** (Pitfall 51), THEN charset-validate.
4. **Defense-in-depth template escaping:** every template render of a tag uses askama's auto-escape (default). Lock with a test: `{{ tag }}` in templates, never `{{ tag|safe }}`.
5. **Document the rule in the README** under `## Tagging`.

**Phase hint:** Phase 23.

**Test cases:**
- **T-V12-TAG-04:** Valid tags (`backup`, `daily`, `prod-east`, `cost_center_42`) → accepted.
- **T-V12-TAG-05:** Invalid tags (`my tag`, `tag.with.dot`, `🎉`, `<script>`, `-leadinghyphen`, `verylongtagnamethatexceedsthirtyonechars`) → rejected at config-load.
- **T-V12-TAG-06:** XSS regression: tag like `<b>x</b>` (impossible after charset validation, but lock the template behavior) — render in template, assert literal text appears in HTML, not parsed `<b>` tag.

**Severity:** MODERATE.

---

### Pitfall 53 — MODERATE — Filter UX: AND vs OR + URL state + empty-tag-set jobs

**Where:** Dashboard handler `src/web/handlers/dashboard.rs` — the filter logic. URL parsing in axum extractors.

**What goes wrong (multiple sub-cases — lock at requirements):**

1. **Multiple selected tags: AND or OR?**
   - **AND** (`?tag=backup&tag=weekly` → jobs with BOTH backup AND weekly): more useful for "show me my weekly backups." Smaller result set. Standard Jira/GitHub-issues behavior.
   - **OR** (jobs with ANY of backup, weekly): "show me anything tagged backup or weekly." Bigger result set, less useful for narrowing.
   - **Recommendation: AND.** Matches existing operator intuition from issue trackers; narrowing is the more common need than broadening; if operator wants OR they can clear and re-pick one tag.

2. **URL state format:** `?tag=backup&tag=weekly` (repeated query param) is the most universally-parsed shape across HTTP frameworks. Avoid `?tags=backup,weekly` (comma-split is fine but axum-extra needs config). Avoid cookies (not shareable).

3. **Empty-tag-set jobs (`tags = []` or unset):** when ANY tag filter is active, jobs without tags are HIDDEN. Rationale: operator clicks `backup` → expects to see only backup-tagged jobs; an untagged "backup-postgres" job not having the tag is the operator's config bug to fix, not cronduit's bug to surface. **Document in the dashboard "0 jobs match this filter" empty state with hint: "If a job is missing here, it may not have the matching tag."**

4. **Filter persistence:** URL only. No cookie, no localStorage. Bookmarkable, shareable, no client-state magic.

5. **"Clear filters" link:** must be clearly present when any filter is active.

**Why:** Filter UX is the entire feature surface. Getting it wrong makes the feature feel broken.

**When it manifests:** First operator interaction with filters.

**Prevention:**

1. **AND semantics + repeated `?tag=` param + URL-only state + hidden-untagged on filter active.** Lock all four.
2. **Document on the dashboard page itself:** small icon next to the chip bar with a hover-tooltip: "Filters combine with AND. Click a chip to add/remove."
3. **Test the parse + render path end-to-end.**

**Phase hint:** Phase 24 (tagging UI — filter chips + URL state).

**Test cases:**
- **T-V12-TAG-07:** `?tag=backup&tag=weekly` → only jobs with BOTH tags shown.
- **T-V12-TAG-08:** `?tag=backup` only → only jobs with `backup` shown; jobs with no tags are hidden.
- **T-V12-TAG-09:** No filter → all jobs shown (including untagged).
- **T-V12-TAG-10:** "Clear filters" link clears the URL state to no params.
- **T-V12-TAG-11:** Bookmark a filtered URL → reload → filter restored.

**Severity:** MODERATE.

---

## Cross-Feature Pitfalls

### Pitfall 54 — CRITICAL — `[defaults] + per-job override + use_defaults = false` shared helper for webhooks AND labels

**Where:** Webhooks (Pitfalls 28–38) and Custom Docker Labels (Pitfalls 39–43) both replicate the same `[defaults] + per-job + use_defaults = false` override pattern that v1.0 introduced for `image`, `cmd`, `env`, etc. and v1.1 didn't extend.

**What goes wrong:** Each feature implements the override logic independently. Three months later, a bug fix in the webhook override logic (e.g., "use_defaults = false should also clear webhook_states") doesn't propagate to the labels override. Drift accumulates.

OR: a shared helper is extracted but its behavior isn't tested for both features — a refactor that breaks one's contract silently breaks the other.

**Why:** Code duplication with high structural similarity is the textbook breeding ground for drift.

**When it manifests:** Second feature's bug fix, or third feature's addition (v1.3+).

**Prevention:**

1. **Extract `apply_defaults_for_field<T>(defaults: Option<&T>, per_job: Option<&T>, use_defaults: bool, merge: F) -> Option<T>` into `src/config/defaults.rs`** as a generic helper. v1.0's existing `apply_defaults` (`defaults.rs:112`) becomes one of the callers.
2. **Test matrix locked across both features:**

   | Case | Defaults | Per-job | use_defaults | Expected |
   |------|----------|---------|--------------|----------|
   | Both unset | None | None | true | None |
   | Defaults only | Some(D) | None | true | Some(D) |
   | Per-job only, use_defaults=true | None | Some(P) | true | Some(P) |
   | Both, use_defaults=true | Some(D) | Some(P) | true | Some(merge(D, P)) |
   | Both, use_defaults=false | Some(D) | Some(P) | false | Some(P) |
   | Defaults only, use_defaults=false | Some(D) | None | false | None |

3. **Test the matrix for each consumer feature:**
   - Webhooks: `webhook_url`, `webhook_states`, `webhook_hmac_secret` — three independent applications.
   - Labels: `labels` — one application.
   - Future: any new override field re-uses the helper + adds a row to its consumer's test.
4. **Each consumer feature gets its own test file** (`tests/v12_webhook_overrides.rs`, `tests/v12_label_overrides.rs`) running the matrix. Helper has unit tests in `defaults.rs`.

**Phase hint:** Phase 15 OR Phase 20, whichever lands first. The helper is extracted in that phase; the second feature consumes the existing helper.

**Test cases:**
- **T-V12-XCUT-01:** Helper unit tests for the 6-row matrix above.
- **T-V12-XCUT-02:** Webhook consumer test runs the matrix for each of the three webhook fields.
- **T-V12-XCUT-03:** Label consumer test runs the matrix for `labels`.

**Severity:** CRITICAL (foundation for two features; bug-once-affects-twice).

---

### Pitfall 55 — CRITICAL — Phase ordering: failure-context queries land BEFORE webhook payload

**Where:** Phase plan ordering. Webhook payload (Pitfall 33) includes `streak_position` and `consecutive_failures` (failure context). Webhook coalescing (Pitfall 29) uses streak math. Both depend on the failure-context queries (Pitfall 46).

**What goes wrong:** If webhooks land in Phase 15–19 and failure-context lands in Phase 21, the webhook payload either:
- Ships with `streak_position: "unknown"` placeholder, requiring a second pass to fill it in (and a v1 → v1 schema change which violates Pitfall 33's promise) — or
- Webhooks block on failure-context completing first, blowing the iterative-rc cadence.

**Why:** Cross-feature data dependency wasn't surfaced in the v1.2 milestone scope; the natural phase ordering (webhooks first because they're the "headline" feature) inverts the dependency.

**When it manifests:** During roadmap construction OR during Phase 16 implementation when the engineer realizes streak data isn't available.

**Prevention (PHASE ORDERING — must be in the roadmap):**

1. **Phase 15 — Webhook delivery worker (foundation, isolation pattern, no payload yet).**
2. **Phase 21 — Failure-context schema + queries (config_hash + image_digest columns, streak query helper).**
   - **Land EARLIER if possible** — make this Phase 16 or split it.
3. **Phase 16 — Webhook payload (depends on Phase 21's streak helper).**
4. **Subsequent phases** can land in any order.
5. **Alternative ordering (also viable):** rename the phases so failure-context schema comes immediately after the webhook delivery worker. Requirements doc must lock the dependency.

**Phase hint:** N/A — this IS the phase-ordering pitfall. Roadmap step needs to know.

**Test cases:**
- **T-V12-XCUT-04:** Webhook payload integration test (which depends on the failure-context helper) is in the test suite for Phase 16, not Phase 15. Build-order check: `cargo build` in the Phase 16 commit MUST pass — if it fails, the schema dependency was forgotten.

**Severity:** CRITICAL (process/ordering, not code).

---

### Pitfall 56 — CRITICAL — `THREAT_MODEL.md` MUST gain a Threat Model 5 + Threat Model 6 entry

**Where:** `/Users/Robert/Code/public/cronduit/THREAT_MODEL.md` — currently 4 threat models (Docker Socket, Untrusted Client, Config Tamper, Malicious Image). v1.1 added "Stop button blast radius" and "Bulk toggle blast radius" inline within Threat Model 2 (Untrusted Client). v1.2 needs more than inline mentions — webhooks introduce a brand-new outbound surface.

**What goes wrong:** v1.2 ships without explicit threat-model treatment of:
1. **Outbound webhooks** (SSRF via webhook URL — Pitfall 31; HTTPS posture — Pitfall 32; HMAC verification on receiver side — Pitfall 30).
2. **Custom Docker labels** (operator-supplied data flowing into Docker daemon API; reserved-namespace abuse — Pitfall 39 — could corrupt orphan reconciliation).

External users reading the threat model think v1.2 is no different from v1.1 in security posture. Audit fails. Operators relying on the threat model for risk assessment get a stale picture.

**Why:** The threat model is the operator's contract for "what cronduit defends against, what it doesn't." Every blast-radius expansion needs a doc update.

**When it manifests:** First external security review post-v1.2.

**Prevention (LOCK as a Phase 26 close-out gate):**

1. **New Threat Model 5: Webhook Outbound** with the structure of the existing 4:
   - **Threat:** cronduit becomes an SSRF proxy / outbound HTTP source for whoever controls the config.
   - **Attack vector:** operator config / config-file tamper (T-T1) defines `webhook_url` pointing at internal services or cloud metadata; cronduit fires the request.
   - **Mitigations:** loopback-default UI; startup WARN on suspicious URLs (loopback+non-LAN HTTP, RFC1918 metadata addresses); HMAC signing of payload with timestamp anti-replay; rustls-only TLS (no openssl-sys regression).
   - **Residual risk:** any URL the cronduit container can reach is reachable. Operator must use network controls (firewall, network policies) to restrict cronduit's egress.
   - **Recommendations:** front the UI with reverse proxy + auth; deny-list cloud metadata endpoints (`169.254.169.254`) at the container's egress; consider separate cronduit instance for prod-vs-dev networks.
2. **New Threat Model 6: Operator-supplied Docker labels** (smaller surface but explicit):
   - **Threat:** operator supplies labels under `cronduit.*` reserved namespace, corrupting orphan reconciliation.
   - **Attack vector:** config edit (intentional or malicious).
   - **Mitigations:** validator at config-load (Pitfall 39); runtime defense-in-depth refuses to overwrite cronduit's own labels.
   - **Residual risk:** labels are otherwise unrestricted; an attacker with config write can label cronduit-spawned containers however they want, which can confuse operator tooling (Watchtower, etc.) but not corrupt cronduit state.
   - **Recommendations:** read-only config mount.
3. **STRIDE table updates:**
   - New row T-S3 (Spoofing): "Attacker forges webhook payload" → mitigated by HMAC signing; out-of-scope: receiver-side verification (operator's responsibility).
   - New row T-T4 (Tampering): "Attacker injects label collision into `cronduit.*` namespace" → mitigated by validator.
   - New row T-I4 (Information disclosure): "Webhook URL embeds credentials in `userinfo`" → mitigated by `strip_url_credentials` (Pitfall 38).
   - New row T-D4 (DoS): "Webhook receiver outage stalls scheduler loop" → mitigated by bounded mpsc + delivery worker isolation (Pitfall 28).
4. **Inline mention in Threat Model 2 (Untrusted Client):** "Webhook configuration (v1.2+ blast radius): an attacker with Web UI access can [in v1.3+ when the UI gains write access to webhook URLs; for v1.2 this is config-file-only] direct cronduit to fire HTTP requests at any address. Pair with Threat Model 5."
   Note: v1.2's webhook URL is config-file-only; the UI does NOT expose write access to webhook URLs. This narrows the v1.2 blast radius — call out explicitly so v1.3 planners see the constraint.

**Phase hint:** Phase 26 (milestone close-out — threat model finalization).

**Test cases:**
- **T-V12-XCUT-05:** Audit: `THREAT_MODEL.md` contains "Threat Model 5: Webhook Outbound" and "Threat Model 6: Operator-supplied Docker labels" sections.
- **T-V12-XCUT-06:** Audit: STRIDE table contains rows T-S3, T-T4, T-I4, T-D4.
- **T-V12-XCUT-07:** README links to the new threat-model sections from the security overview.

**Severity:** CRITICAL (audit gate; non-technical but ship-blocking).

---

## "Looks Done But Isn't" Checklist (v1.2)

Quick smoke list for the executor + UAT phases. Each item is a real failure mode where the feature looks complete in the unit tests but isn't.

- [ ] Webhook delivery worker survives `SchedulerCmd::Reload` without dropping in-flight queued webhooks (Pitfall 28).
- [ ] Webhook coalescing default is `streak_first` and it's tested under sustained-failure cadence (Pitfall 29).
- [ ] HMAC signing test uses a known-vector that future refactors must reproduce byte-for-byte (Pitfall 30).
- [ ] Receiver examples (Python, Go, Node) all use language-native constant-time compare with explanatory comments (Pitfall 30).
- [ ] `THREAT_MODEL.md` has Threat Model 5 + Threat Model 6 + STRIDE updates (Pitfall 56).
- [ ] `payload_version: "v1"` is on every webhook delivery and locked in a snapshot test (Pitfall 33).
- [ ] Webhook retry uses full jitter, not zero jitter (Pitfall 34).
- [ ] `cargo tree -i openssl-sys` empty after webhook + HTTP-client additions (Pitfall 32).
- [ ] `cronduit.*` reserved-namespace validator is case-insensitive on the prefix and trims whitespace; runs at `cronduit check` (Pitfall 39).
- [ ] Type-gated `labels` validator rejects command/script jobs with non-empty labels at config-load (Pitfall 40).
- [ ] `job_runs.config_hash` AND `job_runs.image_digest` columns exist nullable; UI renders "—" for NULLs (Pitfalls 44, 45).
- [ ] Config-hash backfill DOES happen (current job hash); image-digest backfill does NOT (Pitfall 47).
- [ ] Exit-code histogram uses bucket strategy; `null` bucket distinct from green/red; minimum N=10 enforced (Pitfalls 48, 49).
- [ ] No exit-code `/metrics` cardinality leak (Pitfall 48).
- [ ] Tags lowercase + trimmed at config-load; charset validator rejects spaces/special chars (Pitfalls 51, 52).
- [ ] Dashboard tag filter is AND; URL state via repeated `?tag=`; untagged jobs hidden when filter active (Pitfall 53).
- [ ] `apply_defaults_for_field` shared helper has 6-row matrix test; webhooks and labels both consume it (Pitfall 54).
- [ ] Failure-context schema migration lands BEFORE webhook payload includes streak data (Pitfall 55).

---

## Phase-Specific Warnings (for the Roadmapper)

| Phase (suggested) | Feature block | Likely Pitfalls | Mitigation |
|-------------------|---------------|-----------------|------------|
| Phase 15 | Webhook delivery worker (foundation) | 28, 34, 38, 54 | Lock isolation pattern + jitter + url-credential stripping in the very first PR. Extract `apply_defaults_for_field` helper here OR in Phase 20 (whichever lands first). |
| Phase 16 | Webhook payload + state-filter + coalescing | 29, 33, 35 | Lock `payload_version: "v1"` snapshot test. Lock `streak_first` default. Document the delivery-vs-notification contract. **DEPENDS on Phase 21 for streak helper.** |
| Phase 17 | HMAC signing + receiver examples | 30, 37 | Known-vector test. Ship Python + Go + Node receivers with constant-time compare. Document secret rotation. |
| Phase 18 | SSRF + HTTPS posture + threat model placeholder | 31, 32 | No filter; startup WARN; document loopback/RFC1918 silent + public HTTP warn. |
| Phase 19 | Webhook docs + drain accounting | 35, 36, 37 | README `## Webhooks` complete; drain log line; secret rotation procedure. |
| Phase 20 | Custom Docker labels (SEED-001) | 39, 40, 41, 42, 43, 54 | Both validators (reserved + type-gate). BTreeMap for determinism. Cap label sizes. Quoted-key examples. **Extract `apply_defaults_for_field` here if not in Phase 15.** |
| Phase 21 | Failure context schema + queries | 44, 45, 46, 47, 55 | Two-column nullable migration. Config-hash backfill from current job hash; image-digest no backfill. EXPLAIN-locked queries. **MUST land before Phase 16's payload work consumes streak data.** |
| Phase 22 | Failure-context UI + exit-code histogram | 48, 49, 50 | Bucket constants in code. Null bucket distinct color. N≥10 minimum sample. No `/metrics` cardinality leak. |
| Phase 23 | Job tagging — TOML schema + normalization | 51, 52 | Lowercase + trim at load. Charset regex. Reject empty/whitespace-only. |
| Phase 24 | Job tagging — dashboard filter chips + URL state | 53 | AND semantics. Repeated `?tag=`. Untagged hidden on filter active. |
| Phase 25 | rc cuts + iterative UAT | (all UAT-surfaced) | Reuse v1.1's rc.N → fix-PR cadence. Each rc surfaces a gap; fix in-cycle. |
| Phase 26 | Milestone close-out: THREAT_MODEL.md, README, docs | 56 | New Threat Model 5 + 6. STRIDE updates. README cross-links. Receiver examples published. |

**Sequencing summary:** the dependency graph forces this rough ordering:

```
Phase 15 (worker isolation) ──┬──> Phase 16 (payload, depends on 21)
                              │
Phase 21 (failure-context schema) ──┴──> Phase 16
                                    └──> Phase 22 (UI consumes failure-context queries)

Phase 17 (HMAC) — independent, can run parallel to 18, 19, 20

Phase 20 (labels) — independent (modulo helper extraction with 15)

Phase 23 (tag schema) ──> Phase 24 (tag UI)

Phase 26 (THREAT_MODEL.md, docs) — close-out, runs last
```

---

## Sources

**Codebase (read directly during research):**

- `/Users/Robert/Code/public/cronduit/src/scheduler/docker.rs` (entire file — label-build site at L146–L171, image-digest capture at L240–L250, cancel/operator-stop branches at L341–L406)
- `/Users/Robert/Code/public/cronduit/src/scheduler/docker_orphan.rs` (entire file — `mark_run_orphaned WHERE status='running'` guard at L120, L131, justifies `cronduit.*` reserved namespace)
- `/Users/Robert/Code/public/cronduit/migrations/sqlite/20260410_000000_initial.up.sql` (L23 `jobs.config_hash`; L46 `idx_job_runs_job_id_start`; verifies `job_runs.config_hash` does NOT exist — drives Pitfall 44)
- `/Users/Robert/Code/public/cronduit/migrations/sqlite/{20260416..20260422}_*.up.sql` (v1.1 migration shapes — three-file pattern referenced for v1.2 parallel migrations)
- `/Users/Robert/Code/public/cronduit/THREAT_MODEL.md` (entire file — basis for Threat Model 5 + 6 additions; Stop + Bulk-toggle blast-radius mentions at L113, L115 are the tone reference)
- `/Users/Robert/Code/public/cronduit/Cargo.toml` (L27–L28 — `hyper-util` already present; reuse for webhook delivery worker, no new heavy HTTP client dep needed)
- `/Users/Robert/Code/public/cronduit/.planning/seeds/SEED-001-custom-docker-labels.md` (entire file — design pre-locked; Pitfalls 39–43 enforce the lock)
- `/Users/Robert/Code/public/cronduit/.planning/PROJECT.md` (v1.2 milestone scope confirmation)
- `/Users/Robert/Code/public/cronduit/.planning/MILESTONES.md` (v1.0 + v1.1 history; v1.0.1 `cmd`-on-non-docker validator pitfall, the precedent for Pitfall 40)
- `/Users/Robert/Code/public/cronduit/.planning/milestones/v1.1-research/PITFALLS.md` (tone, depth, ranking convention; v1.1 numbering ends ~#27 → v1.2 starts at #28)
- `/Users/Robert/Code/public/cronduit/.planning/milestones/v1.0-research/PITFALLS.md` (baseline pitfalls referenced as "v1.0-P#N" where re-entered)
- `/Users/Robert/Code/public/cronduit/src/config/hash.rs` (verifies `compute_config_hash` is per-job and currently not wired into `job_runs`)

**Ecosystem references:**

- Stripe webhooks signing convention: timestamp + body, header `Stripe-Signature: t=<ts>,v1=<sig>` — basis for the `X-Cronduit-Signature` + `X-Cronduit-Timestamp` shape in Pitfall 30.
- GitHub webhooks: `X-Hub-Signature-256: sha256=<hex>` header convention.
- AWS architecture blog "Exponential Backoff and Jitter" (full jitter recommendation) — basis for Pitfall 34 algorithm choice.
- moby#8441 (auto_remove vs wait_container race) — referenced at v1.0 Pitfall #3, still load-bearing for the image-digest capture site.
- SQLite docs "Making Other Kinds Of Table Schema Changes" — table-rewrite pattern, reused for any v1.2 NOT NULL tightening if needed (none planned).

---

## Confidence

| Area | Level | Basis |
|------|-------|-------|
| Webhook isolation pattern | HIGH | Standard tokio mpsc + spawn-worker pattern; v1.0/v1.1 already use bounded channels (log pipeline, log-broadcast capacity 256). Direct read of scheduler loop confirms existing isolation surface. |
| Webhook flooding default | HIGH | Streak-edge-trigger is the operationally-correct default; rationale grounded in operator-experience pitfalls observed in similar tools. |
| HMAC pitfalls | HIGH | Constant-time compare is canonical Web-Cryptography 101; Stripe / GitHub signing conventions verified. |
| SSRF + HTTPS posture | HIGH | Aligned with v1's documented "trusted-LAN" stance (THREAT_MODEL.md Threat Model 2); no breaking change. |
| Payload schema versioning | HIGH | API-design canonical wisdom; deferred-versioning-cost is well-documented across Stripe/GitHub/Slack/Twilio post-mortems. |
| `cronduit.*` reserved namespace | HIGH | Validated against `docker_orphan.rs` reading `cronduit.run_id` → exact data-integrity risk if operator overrides. |
| Type-gate validator | HIGH | Direct precedent in v1.0.1 `cmd`-on-non-docker validator. |
| `job_runs.config_hash` gap | HIGH | Grep-verified: `config_hash` exists only on `jobs` table (`migrations/sqlite/20260410_000000_initial.up.sql:23`), nowhere on `job_runs`. |
| Image-digest capture site | HIGH | Direct read of `docker.rs:240–250`; `DockerExecResult.image_digest` already populated, just needs persistence. |
| Exit-code bucket strategy | MEDIUM-HIGH | Buckets reflect Unix shell-exit-code conventions (128+signal, 127 = command-not-found); decision-not-risk choice. |
| Tag normalization rules | HIGH | Standard URL-friendly slug conventions; matches GitHub Issues / GitLab labels. |
| Filter UX defaults (AND, hidden untagged) | HIGH | Issue-tracker convention (Jira, GitHub Issues, Linear all default to AND on multi-tag). |
| Phase-ordering dependency | HIGH | Direct trace of webhook payload (Pitfall 33) → streak helper (Pitfall 46) dependency. |
| Threat model gap | HIGH | Direct read of `THREAT_MODEL.md`; no current treatment of outbound HTTP. |
