---
phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
reviewed: 2026-05-01T00:00:00Z
depth: standard
files_reviewed: 27
files_reviewed_list:
  - docs/WEBHOOKS.md
  - justfile
  - migrations/postgres/20260502_000008_webhook_deliveries_add.up.sql
  - migrations/sqlite/20260502_000008_webhook_deliveries_add.up.sql
  - src/cli/run.rs
  - src/config/mod.rs
  - src/config/validate.rs
  - src/db/queries.rs
  - src/scheduler/retention.rs
  - src/scheduler/sync.rs
  - src/telemetry.rs
  - src/webhooks/dispatcher.rs
  - src/webhooks/mod.rs
  - src/webhooks/retry.rs
  - src/webhooks/worker.rs
  - tests/metrics_endpoint.rs
  - tests/scheduler_integration.rs
  - tests/v12_webhook_dlq.rs
  - tests/v12_webhook_drain.rs
  - tests/v12_webhook_failed_metric.rs
  - tests/v12_webhook_https_required.rs
  - tests/v12_webhook_metrics_family.rs
  - tests/v12_webhook_network_error_metric.rs
  - tests/v12_webhook_queue_drop.rs
  - tests/v12_webhook_retry.rs
  - tests/v12_webhook_retry_after.rs
  - tests/v12_webhook_retry_classification.rs
  - tests/v12_webhook_scheduler_unblocked.rs
  - tests/v12_webhook_success_metric.rs
findings:
  blocker: 3
  warning: 6
  total: 9
status: issues_found
---

# Phase 20: Code Review Report

**Reviewed:** 2026-05-01
**Depth:** standard
**Files Reviewed:** 27 (16 source + docs/justfile/migrations + 13 tests)
**Status:** issues_found

## Summary

The Phase 20 webhook posture work is largely well-structured: the
`RetryingDispatcher` composes cleanly over `HttpDispatcher` (D-21 invariant
preserved), the SSRF/HTTPS-required validator's classification logic is
sound and well-tested, the metric family rename is properly closed-enum, and
no plaintext secrets leak (HMAC keys live in `SecretString`, DLQ schema
intentionally omits payload/header/secret columns). The `cargo tree -i
openssl-sys` guard in `just openssl-check` is correctly anchored against
PASS-on-no-match (`grep -q .`), and the test surface is broad.

However, three correctness defects warrant blocking the rc.1 cut:

1. **Retention pruner is permanently breakable** by the new
   `webhook_deliveries` table. The new FK `run_id REFERENCES job_runs(id)`
   has no `ON DELETE CASCADE`, SQLite `foreign_keys(true)` is enabled
   (`src/db/mod.rs:72`), and the prune order deletes `job_runs` BEFORE
   `webhook_deliveries`. Any DLQ row referencing a now-old run will cause
   the runs-DELETE batch to fail with FK violation, which the pruner logs
   and `break`s out of — retention silently stops working.
2. **`Retry-After` is silently ignored on the FIRST inter-attempt sleep.**
   The implemented `cap_for_slot(prev_slot)` semantics (where
   `prev_slot = next_attempt - 1`) caps the sleep before attempt 2 at
   `schedule[1] * 1.2 = 36s`, but `docs/WEBHOOKS.md` § "Retry-After header
   handling" explicitly documents the cap as `schedule[next_attempt + 1]
   * 1.2 = 360s`. A receiver returning `Retry-After: 350` between
   attempts 1 and 2 is silently truncated to 36s. The code, the tests, and
   the doc all disagree; the worked-table in the doc reflects the
   intended semantics.
3. **5-arm match in `RetryingDispatcher::deliver` mishandles
   `WebhookError::HttpStatus.last_error`.** When the chain exhausts
   transient 5xx attempts, `last_status` is set on every attempt but
   `last_error` is set to `None`, so the DLQ row stores
   `last_error = NULL` for an `http_5xx` outcome. Operators querying the
   DLQ for "what went wrong" lose the receiver's body-preview signal that
   `HttpDispatcher` already log-warned about.

Other findings cover docs/code drift, an unused-when-no-webhook metric
seed loop, missing column documentation in the migrations, and a couple
of test-quality issues.

## Critical Issues

### BL-01: Retention pruner FK-violates on `webhook_deliveries.run_id` after rc.1 ships

**File:** `src/scheduler/retention.rs:93-172`, `src/db/queries.rs:1474-1511`,
`migrations/sqlite/20260502_000008_webhook_deliveries_add.up.sql:30-31`,
`migrations/postgres/20260502_000008_webhook_deliveries_add.up.sql:30-31`,
`src/db/mod.rs:72`

**Issue:** The new `webhook_deliveries` table declares
`FOREIGN KEY (run_id) REFERENCES job_runs(id)` with no `ON DELETE CASCADE`.
SQLite has `foreign_keys(true)` set globally on every pool connection
(`src/db/mod.rs:72`); Postgres enforces FKs by default. The retention pruner
runs in this order:

1. Phase 1 — delete `job_logs` where `created_at < cutoff` (runs to completion)
2. Phase 2 — delete `job_runs` where `end_time < cutoff AND NOT EXISTS (logs)` (`src/db/queries.rs:1481-1493`)
3. Phase 4 — delete `webhook_deliveries` where `last_attempt_at < cutoff` (`src/scheduler/retention.rs:138-172`)

Phase 2's `NOT EXISTS` clause checks **only `job_logs`** — not
`webhook_deliveries`. As soon as ANY DLQ row references an old `run_id`,
the Phase 2 DELETE statement aborts with an FK-constraint violation, the
pruner's match arm logs `error = %e, "retention prune: failed to delete
run batch"` and breaks the loop (`src/scheduler/retention.rs:121-127`). The
24-hour interval re-runs and hits the same wall every cycle — **retention
is silently and permanently broken** the moment a webhook delivery fails.
This is the core scenario this feature was designed for.

**Fix:** Either (a) reverse the prune order so `webhook_deliveries` is
deleted BEFORE `job_runs` AND extend the runs DELETE's `NOT EXISTS` to
also cover `webhook_deliveries`, or (b) add `ON DELETE CASCADE` to the
FK in both migrations and an `ON DELETE NO ACTION` test, or (c) drop the
FK constraint and treat `run_id` as a soft pointer (DLQ rows survive run
deletion). Option (a) preserves cascade-by-policy clarity; option (b) is
the Postgres-idiomatic fix; option (c) matches the existing
"audit-table-only" framing of the schema.

```sql
-- Option (b), SQLite migration:
CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id           INTEGER NOT NULL,
    job_id           INTEGER NOT NULL,
    ...
    FOREIGN KEY (run_id) REFERENCES job_runs(id) ON DELETE CASCADE,
    FOREIGN KEY (job_id) REFERENCES jobs(id)     ON DELETE CASCADE
);
```

Add a regression test that seeds a job_run + webhook_deliveries row,
sets the runs end_time to before cutoff, calls
`delete_old_runs_batch`, and asserts no FK violation. Currently no test
in the suite exercises retention against a populated DLQ, so the suite
shipped this rc.1 candidate green.

### BL-02: `compute_sleep_delay` cap is one schedule-slot too small — first-attempt `Retry-After` silently capped at 36s

**File:** `src/webhooks/retry.rs:166-180`, contradicting
`docs/WEBHOOKS.md:368-388` and `src/webhooks/retry.rs:124-127`'s docstring

**Issue:** The doc and the function-level comment for `cap_for_slot`
both state:

> For a sleep that precedes attempt index `slot`, the cap is
> `schedule[slot+1] * 1.2`; if no `slot+1` exists (last attempt), reuse
> the previous slot's cap (`schedule[slot] * 1.2`).

`docs/WEBHOOKS.md` § "Retry-After header handling" is explicit:

```
delay = max(locked_schedule[next_attempt], retry_after_seconds)
delay = min(delay, schedule[next_attempt + 1] * 1.2)   # cap
# last attempt: cap = schedule[next_attempt] * 1.2
```

| Pre-attempt slot | Slot delay | `Retry-After` floor | `Retry-After` cap |
|------------------|------------|---------------------|-------------------|
| Before attempt 2 | 30s × jitter | 30s | `300s × 1.2 = 360s` |
| Before attempt 3 | 300s × jitter | 300s | `300s × 1.2 = 360s` (last-slot fallback) |

The implementation:

```rust
let prev_slot = next_attempt.saturating_sub(1);
let cap = cap_for_slot(prev_slot, schedule);
```

passes `next_attempt - 1` as `slot` to `cap_for_slot`, so the resulting
cap for `next_attempt = 1` (sleep before attempt 2) is
`cap_for_slot(0) = schedule[1] * 1.2 = 30s * 1.2 = 36s` — **NOT** the
documented `360s`. A real receiver returning `Retry-After: 350` between
attempts 1 and 2 (a textbook "wait 5 minutes before retry" pattern) is
silently truncated to ~36s. The unit test
`compute_sleep_delay_caps_retry_after_at_slot_cap` and the integration
test `receiver_429_with_retry_after_9999_is_capped` both assert the
36s/360s behavior — they regression-lock the buggy semantics rather
than the documented contract.

The last-attempt cap (next_attempt = 2) coincidentally lands on
schedule[2]*1.2 = 360s, so only the FIRST inter-attempt cap is wrong.

**Fix:** Either fix the code to match the docs, OR fix the docs to match
the code and add a SECURITY note that the cap is intentionally tight
(can't be adjusted by a misbehaving receiver). Since D-21 locks
verbatim error messages and D-22 documents the new metric family
publicly, the correct path is almost certainly to fix the code:

```rust
fn compute_sleep_delay(
    next_attempt: usize,
    schedule: &[Duration],
    retry_after: Option<Duration>,
) -> Duration {
    let base = jitter(schedule[next_attempt]);
    match retry_after {
        None => base,
        Some(ra) => {
            // cap_for_slot(slot) returns schedule[slot+1]*1.2 with
            // last-slot fallback. For sleep BEFORE attempt `next_attempt`,
            // the cap should be schedule[next_attempt+1]*1.2 — i.e.,
            // pass next_attempt directly, NOT next_attempt-1.
            let cap = cap_for_slot(next_attempt, schedule);
            std::cmp::min(cap, std::cmp::max(base, ra))
        }
    }
}
```

Then update tests to assert the documented behavior:
- `compute_sleep_delay(1, ..., Some(9999s))` → cap at 360s (was 36s)
- `compute_sleep_delay(2, ..., Some(9999s))` → cap at 360s (was 360s — unchanged)
- `receiver_429_with_retry_after_9999_is_capped` total elapsed becomes
  ~360 + 360 = 720s (currently asserts ≤ 450s).

If the locked semantics ARE the 36s/360s behavior (e.g., to bound
total chain wall time more aggressively), the doc must be updated and
the design rationale recorded — but that contradicts the worked-table
explicitly published to operators.

### BL-03: DLQ row stores `last_error = NULL` for HTTP-5xx exhaustion, losing the receiver body preview

**File:** `src/webhooks/retry.rs:359-364`, contradicting the dispatcher
log at `src/webhooks/dispatcher.rs:300-308`

**Issue:** When `HttpDispatcher::deliver` returns
`WebhookError::HttpStatus { code, retry_after }` for a non-2xx response,
it has already read the response body and emitted a WARN log with
`body_preview = %truncated` (200 chars). The `RetryingDispatcher::deliver`
loop then captures per-variant fields:

```rust
WebhookError::HttpStatus { code, retry_after } => {
    last_status = Some(*code as i64);
    last_error = None;
    last_retry_after = *retry_after;
}
```

`last_error` is hard-coded `None` for the HTTP-status variant. After
3-attempt 5xx exhaustion the DLQ row is written with `last_status =
500` (good) but `last_error = NULL` — operators querying
`SELECT * FROM webhook_deliveries WHERE dlq_reason = 'http_5xx'` see no
information about WHAT the 5xx body said, even though `HttpDispatcher`
already extracted and logged a 200-char preview.

The `WebhookError::HttpStatus` enum variant carries no `body` field, so
the body preview is genuinely lost between dispatcher and DLQ-writer.
This is a real loss of the audit-table's diagnostic value: the WARN log
has the body preview, but when an operator wants the full picture they
read both the log AND the DLQ — and the DLQ is silent on what the
receiver actually said.

**Fix:** Add a `body_preview: Option<String>` field to
`WebhookError::HttpStatus` (or a sibling field), populate it in
`HttpDispatcher` from the same `truncated` string the warn-log already
uses, and propagate into `last_error` in
`RetryingDispatcher::deliver`:

```rust
// src/webhooks/dispatcher.rs
#[error("webhook HTTP non-2xx: status={code}")]
HttpStatus {
    code: u16,
    retry_after: Option<std::time::Duration>,
    body_preview: Option<String>,
},

// at line 300:
let body_preview = resp.text().await.unwrap_or_default();
let truncated: String = body_preview.chars().take(200).collect();
// ... existing warn log ...
Err(WebhookError::HttpStatus {
    code,
    retry_after,
    body_preview: Some(truncated),
})

// src/webhooks/retry.rs around line 359:
WebhookError::HttpStatus { code, retry_after, body_preview } => {
    last_status = Some(*code as i64);
    last_error = body_preview.as_deref().map(truncate_error);
    last_retry_after = *retry_after;
}
```

The truncate_error helper at retry.rs:185 already enforces 500 chars,
so we get defense-in-depth on the size cap.

## Warnings

### WR-01: Webhook metric per-job seed loop runs even with zero webhooks configured, inflating cardinality

**File:** `src/cli/run.rs:161-170`

**Issue:** The Phase 20 / WH-11 closed-enum seed loop runs unconditionally:

```rust
for job in &sync_result.jobs {
    for status in ["success", "failed", "dropped"] {
        metrics::counter!(
            "cronduit_webhook_deliveries_total",
            "job" => job.name.clone(),
            "status" => status,
        )
        .increment(0);
    }
}
```

This fires AFTER the `webhooks: HashMap<i64, WebhookConfig>` is built
(line 280-300) but does NOT consult that map. For an operator running 50
jobs of which only 2 have webhooks configured, the `/metrics` endpoint
will surface 50 × 3 = 150 zero-baselined rows for the closed-enum family
even though 48 of those jobs can never produce a delivery. Cardinality
is bounded as the doc claims, but the surface is misleading: an operator
checking dashboards sees `cronduit_webhook_deliveries_total{job="cleanup-tmp",status="dropped"} 0`
for a job that has no webhook configured.

**Fix:** Gate the seed loop on the `webhooks` map (which IS in scope at
the call site if reordered):

```rust
// move the webhooks-map construction (lines 280-300) up to before
// the seed loop, then:
for job in &sync_result.jobs {
    if !webhooks.contains_key(&job.id) {
        continue;
    }
    for status in ["success", "failed", "dropped"] {
        metrics::counter!(
            "cronduit_webhook_deliveries_total",
            "job" => job.name.clone(),
            "status" => status,
        )
        .increment(0);
    }
}
```

Note the comment at telemetry.rs:223-237 already pre-seeds the
status-only dimension (no job label) at boot — that family stays
operator-visible from /metrics whether or not any job has webhooks.

### WR-02: `default_db_url()` reads `DATABASE_URL` env var without a SecretString round-trip

**File:** `src/config/mod.rs:65-72`

**Issue:** `default_db_url()` is the serde default for
`server.database_url`:

```rust
fn default_db_url() -> SecretString {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://./cronduit.db?mode=rwc".to_string());
    SecretString::from(url)
}
```

The intermediate `String` (which can contain Postgres credentials like
`postgres://user:pass@host/db`) lives on the stack until wrapped. Not
catastrophic, but the security posture documented elsewhere in the
codebase ("No plaintext secrets in the config file; interpolate from
env, wrap in a SecretString type") implies same-call-site wrapping. The
intermediate `String` is `Drop` (zero-on-drop is not done by std), so a
heap snapshot during the call could leak the credential.

**Fix:** Use `secrecy`'s `SecretString::new` directly from the env var
or accept this is below the threat model and add a doc comment
explaining why it's acceptable for an env-var fast path.

### WR-03: `last_class.unwrap_or(DlqReason::Network)` papers over a logic invariant rather than asserting it

**File:** `src/webhooks/retry.rs:407`

**Issue:** After the retry loop:

```rust
let reason = last_class.unwrap_or(DlqReason::Network);
```

`last_class` can only be `None` if the loop body never ran the `Err(_)`
arm — but if the loop never errored, control flow returns `Ok(())` from
the `Ok(())` arm at line 354, never reaching this code. The
`unwrap_or` therefore can never fire, but the choice of `Network` as
the "should be unreachable" sentinel is potentially misleading: a
future refactor that adds an early `break` outside the match could
silently land here and write `dlq_reason='network'` for a logically
different failure.

**Fix:** Replace with an `expect` that names the invariant:

```rust
let reason = last_class.expect(
    "last_class is set on every Err arm of the retry loop; \
     reaching the terminal-failure tail with last_class=None \
     means a refactor broke the invariant"
);
```

Or, refactor `deliver` so the terminal-failure tail receives `reason`
through the type system (e.g., return early from the match arms with a
labelled break that carries the DlqReason).

### WR-04: `drain_budget_expiry_drops_remaining_queued_events` test asserts only `delta >= 0.0`, exercising no actual invariant

**File:** `tests/v12_webhook_drain.rs:382-393`

**Issue:** The test concedes the drop count is "racy under biased;
recv-first locked design" and asserts only that the counter delta is
non-negative — which is trivially true for an unsigned counter family.
The header comment (lines 232-270) documents the racy-test problem in
detail and points at SUMMARY.md § Deviations.

This means: **the production code path that actually performs the drain
drops + closed-enum increments is not regression-locked by an
integration test.** A future refactor that removes the `try_recv` loop
in worker.rs or swaps the closed-enum `"dropped"` literal would ship
green.

The earlier in-flight test (`in_flight_request_runs_to_completion_during_drain`,
lines 170-231) does cover the "in-flight HTTP not cancelled" invariant
correctly, so the worst case isn't total absence of coverage — but the
drop-counter increment path is genuinely untested at the integration
level.

**Fix:** Move the drop-counter assertion into a unit-style test in
`src/webhooks/worker.rs` that constructs the worker with a controlled
sender, sends N events, fires cancel, advances paused-clock past the
drain deadline, and asserts the counter increments by N. Tokio's
paused-clock interacts with `Instant::now()` boundaries deterministically,
which sidesteps the multi-thread runtime racing problem the integration
test hit. Alternatively, replace the `delta >= 0.0` assertion with
`delta > 0.0` AND inflate the test's pusher window so at least one
drop is observed deterministically (the test's STEP-2 design comment
already speculates on this; pin the worst case rather than the best).

### WR-05: Retention pruner numbers prune phases as 1, 2, 4 (no Phase 3) — easy to mis-read as a missing phase

**File:** `src/scheduler/retention.rs:55, 93, 132, 174`

**Issue:** The retention pruner's phase comments use `Phase 1` (logs),
`Phase 2` (runs), `Phase 4` (webhook DLQ), `Phase 3` (WAL checkpoint
inline at 174). Reader expects sequential numbering; the out-of-order
labels suggest a missing Phase 3 (e.g., a deletion that was elided).
Per the in-line comment, "Phase 3" is the WAL checkpoint that always
ran AFTER runs deletion in the pre-Phase-20 code, and the new
webhook-DLQ phase was inserted as "Phase 4" to keep it lexically last
without renumbering. The result is a confusing read.

**Fix:** Renumber to 1/2/3/4 (logs, runs, dlq, wal-checkpoint) and
update each comment.

### WR-06: Migration `last_status INTEGER` allows out-of-band signed values; no CHECK constraint on dlq_reason or sane HTTP status range

**File:** `migrations/sqlite/20260502_000008_webhook_deliveries_add.up.sql:25-27`,
`migrations/postgres/20260502_000008_webhook_deliveries_add.up.sql:25-27`

**Issue:** The `dlq_reason` column is documented as a closed enum
(`http_4xx | http_5xx | network | timeout | shutdown_drain`) but has no
DB-level CHECK constraint. The matching code path
(`DlqReason::as_str` in `src/webhooks/retry.rs:50-58`) is the only
guard. A future direct INSERT (a maintenance script, a UI-side
update, etc.) could write any string and downstream operators would
not know to detect it. Similarly, `last_status INTEGER` allows
negative values; HTTP status codes are `100..=599`.

The schema comment block (lines 1-18) states "Closed enum: ..." but the
schema does not enforce it.

**Fix:** Add CHECK constraints in both migration files:

```sql
-- SQLite
last_status      INTEGER CHECK (last_status IS NULL OR (last_status BETWEEN 100 AND 599)),
dlq_reason       TEXT NOT NULL CHECK (dlq_reason IN
                     ('http_4xx', 'http_5xx', 'network', 'timeout', 'shutdown_drain')),

-- Postgres: identical syntax works.
```

This is consistent with similar constraints elsewhere in the
project (none currently exist on `job_runs.status`, but if Phase 20
is locking a closed-enum contract it should be DB-enforced).

---

_Reviewed: 2026-05-01_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
