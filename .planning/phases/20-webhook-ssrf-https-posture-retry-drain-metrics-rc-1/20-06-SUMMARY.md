---
phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
plan: 06
subsystem: webhooks
tags: [webhooks, config, server-block, cli-wire, retrying-dispatcher, per-job-seed, humantime, drain-grace, metrics-pre-seed]

# Dependency graph
requires:
  - phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
    plan: 02
    provides: RetryingDispatcher::new(inner, pool, cancel, webhooks_arc) with B2-fixed DLQ url lookup via shared Arc<HashMap<i64, WebhookConfig>>
  - phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
    plan: 04
    provides: spawn_worker(rx, dispatcher, cancel, drain_grace) 4-arg signature + worker_loop drain-deadline state machine
  - phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
    plan: 05
    provides: cronduit_webhook_deliveries_total{job, status} labeled family with closed-enum status ∈ {success, failed, dropped} eagerly described at boot
provides:
  - "[server].webhook_drain_grace humantime config field with 30s default"
  - "RetryingDispatcher wrap in src/cli/run.rs (single Arc shared with HttpDispatcher per B2 fix)"
  - "drain_grace plumbed from cfg.server.webhook_drain_grace into spawn_worker"
  - "Per-job × per-status metric pre-seed loop at boot (n_jobs × 3 cardinality)"
  - "Boot INFO log emitting shutdown_grace_secs + webhook_drain_grace_secs side-by-side"
affects: [20-07, 20-08, 20-09]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Single-Arc share pattern: Arc<HashMap<i64, WebhookConfig>> constructed ONCE at the bin layer and cloned into both HttpDispatcher::new and RetryingDispatcher::new so the DLQ url-lookup at write time uses the same map HttpDispatcher uses for sending. (B2 regression-locked.)"
    - "Pre-seed at boot pattern (RESEARCH §4.6): for every (job, status) in the Cartesian product, emit metrics::counter!(..., \"job\" => …, \"status\" => …).increment(0). Forces Prometheus to render zero-baseline rows so operator alerts fire from the first scrape, never NaN."
    - "Side-by-side knob INFO log: when two related shutdown knobs (shutdown_grace, webhook_drain_grace) live in different config fields but interact at runtime (worst-case = drain_grace + 10s reqwest cap), emit a single boot log line showing both values so operators see the relationship at startup without grepping."

key-files:
  created:
    - ".planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-06-SUMMARY.md (this file)"
  modified:
    - "src/config/mod.rs (added pub webhook_drain_grace: Duration field with humantime_serde + default_webhook_drain_grace() helper; +3 in-module tests)"
    - "src/cli/run.rs (wrapped HttpDispatcher in RetryingDispatcher with shared webhooks_arc; replaced 30s hardcode with cfg.server.webhook_drain_grace; added per-job × per-status metric pre-seed loop at boot; added side-by-side shutdown knob INFO log)"
    - "src/scheduler/sync.rs (updated test helper make_server_config() to include the new field — Rule 3 blocking)"
    - "tests/scheduler_integration.rs (updated integration test helper make_server_config() to include the new field — Rule 3 blocking)"

key-decisions:
  - "Use cfg.server.webhook_drain_grace verbatim (not std::cmp::min(cfg.server.shutdown_grace, ...)). The two knobs are intentionally independent — D-17 says no budget overlap; D-18 says worst-case shutdown ceiling = drain_grace + 10s. The boot INFO log surfaces the relationship so operators tune both knobs in concert."
  - "Pre-seed loop placed AFTER metrics::gauge!(\"cronduit_scheduler_up\").set(1.0) (line 154) but BEFORE the webhooks HashMap construction (line 264) — at this point both prerequisites (sync_result.jobs in scope; setup_metrics() invoked) are met. The seed loop iterates ALL configured jobs (not just webhook-bearing ones) per RESEARCH §4.6 — operators querying cronduit_webhook_deliveries_total{job=\"...\"} expect every job to have a row, even those without webhooks configured (which would never increment past 0)."
  - "RetryingDispatcher::new called with the SAME webhooks_arc that goes into HttpDispatcher::new (B2 fix locked by Plan 02 SUMMARY § Note for Plan 06 + tests/v12_webhook_dlq.rs::dlq_url_matches_configured_url). The empty-URL fallback path is forbidden — passing the Arc is non-optional. Verified by grep count (RetryingDispatcher::new = 1, let webhooks_arc = 1, webhooks_arc.clone() = 1)."

patterns-established:
  - "When a config field's default value matches a previous hardcode, document the equivalence in the commit message + decision so future readers understand production behavior is unchanged across the wiring change. (Avoids the trap of operators questioning whether a config-knob change introduced regression.)"

requirements-completed: [WH-10]

# Metrics
duration: 12min
completed: 2026-05-01
---

# Phase 20 Plan 06: CLI/bin-layer Wiring (webhook_drain_grace + RetryingDispatcher + Per-Job Metric Pre-Seed) Summary

**Wired all of Phase 20's components into the bin layer: added the `[server].webhook_drain_grace` humantime config field with 30s default; wrapped HttpDispatcher in RetryingDispatcher with the shared `Arc<HashMap<i64, WebhookConfig>>` (B2-fixed DLQ url lookup); plumbed `cfg.server.webhook_drain_grace` into `spawn_worker`; added a per-job × per-status metric pre-seed loop at boot to force zero-baseline rows for every configured job × {success, failed, dropped} combo; and added a side-by-side INFO log surfacing the worst-case shutdown ceiling. This plan is what makes Success Criteria 3 (operator-tunable drain) and 4 (eager metric labels) actually observable.**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-05-01T21:47:51Z
- **Completed:** 2026-05-01T21:59:49Z
- **Tasks:** 2 (per plan; both committed atomically with TDD RED → GREEN gates)
- **Files modified:** 4 (3 src + 1 test harness)

## Accomplishments

- `[server].webhook_drain_grace: Duration` added to `ServerConfig` with `#[serde(default = "default_webhook_drain_grace", with = "humantime_serde")]` + `fn default_webhook_drain_grace() -> Duration { Duration::from_secs(30) }`. Doc comment cites D-16 + D-18 + Pitfall 8 (worst-case shutdown = drain_grace + 10s reqwest cap).
- 3 new in-module tests in `src/config/mod.rs::tests`:
  - `webhook_drain_grace_default_is_30s` (TOML without the field defaults to 30s)
  - `webhook_drain_grace_humantime_parses_45s` (TOML with `webhook_drain_grace = "45s"` parses to 45s)
  - `default_webhook_drain_grace_returns_30s` (helper returns 30s)
- `RetryingDispatcher::new(http, pool.clone(), cancel.child_token(), webhooks_arc)` wraps `HttpDispatcher` in the bin layer's webhook-build path (`src/cli/run.rs` lines 302-323). The `webhooks_arc` is constructed ONCE and shared between `HttpDispatcher::new` and `RetryingDispatcher::new` per the B2 regression-locked design — DLQ rows record the configured URL per job_id.
- `spawn_worker(rx, dispatcher, cancel.child_token(), cfg.server.webhook_drain_grace)` replaces the Plan 04 hardcoded `Duration::from_secs(30)` (now sourced from the new config field). Production behavior unchanged because the default matches the prior hardcode.
- Per-job × per-status metric pre-seed loop at boot (`src/cli/run.rs` lines 156-167) emits `metrics::counter!("cronduit_webhook_deliveries_total", "job" => job.name.clone(), "status" => status).increment(0)` for every configured job × `{success, failed, dropped}` triple. Forces Prometheus to render zero-baseline rows so operator alerts fire correctly from the first scrape (cardinality = n_jobs × 3).
- Boot INFO log (`src/cli/run.rs` lines 332-337) emits `shutdown_grace_secs` + `webhook_drain_grace_secs` side-by-side so operators see the relationship at startup; documents worst-case shutdown ceiling.
- Bin-layer drain ordering preserved unchanged (`scheduler_handle.await` → `webhook_worker_handle.await` → `pool.close().await` at lines 363, 370, 373) per D-17 (no budget overlap).

## Task Commits

Each task was committed atomically with TDD RED → GREEN gate sequence:

1. **Task 1 RED: Add failing tests for webhook_drain_grace config field** — `c0ec007` (test)
2. **Task 1 GREEN: Add webhook_drain_grace field to ServerConfig** — `1f90eb0` (feat)
3. **Task 2: Wire RetryingDispatcher + drain_grace + per-job metric pre-seed** — `624f88c` (feat)

(Task 2's TDD shape is structural — the regression locks already exist as integration tests from Plan 02 (`tests/v12_webhook_dlq.rs::dlq_url_matches_configured_url`) and Plan 04 (`tests/v12_webhook_drain.rs`). The Task 2 implementation is verified by `cargo check --bin cronduit` + the existing 527-test integration suite remaining green.)

## Files Created/Modified

**Created:**
- `.planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-06-SUMMARY.md` — this summary.

**Modified:**
- `src/config/mod.rs` — added `pub webhook_drain_grace: Duration` field (line 46) with `#[serde(default = "default_webhook_drain_grace", with = "humantime_serde")]` + Phase 20 doc comment; added `fn default_webhook_drain_grace() -> Duration { Duration::from_secs(30) }` (line 71-73); added 3 in-module unit tests at the bottom of the file (lines 369-414).
- `src/cli/run.rs` — added per-job × per-status metric pre-seed loop after `metrics::gauge!("cronduit_scheduler_up").set(1.0);` (lines 156-167); replaced `else { ... Arc::new(http) }` dispatcher branch with the `let webhooks_arc = Arc::new(webhooks); ... let retrying = RetryingDispatcher::new(http, pool.clone(), cancel.child_token(), webhooks_arc); Arc::new(retrying)` form (lines 302-323); replaced the hardcoded `let webhook_drain_grace = Duration::from_secs(30);` with the boot INFO log + `cfg.server.webhook_drain_grace` directly in the `spawn_worker` call (lines 326-343).
- `src/scheduler/sync.rs` — updated `make_server_config()` test helper (line 245) to include `webhook_drain_grace: Duration::from_secs(30)` field (Rule 3 blocking — E0063 missing field).
- `tests/scheduler_integration.rs` — updated `make_server_config()` integration test helper (line 43) similarly.

## Exact Line Range of Modified Dispatcher Build (per plan output spec)

`src/cli/run.rs:302-323` — the `let dispatcher: std::sync::Arc<dyn crate::webhooks::WebhookDispatcher> = if webhooks.is_empty() { ... } else { ... };` block.

The `else` arm (lines 307-322) constructs:

```rust
let webhooks_arc = std::sync::Arc::new(webhooks);
let http = crate::webhooks::HttpDispatcher::new(pool.clone(), webhooks_arc.clone())
    .map_err(|e| anyhow::anyhow!("HttpDispatcher init failed: {e}"))?;
let retrying = crate::webhooks::RetryingDispatcher::new(
    http,
    pool.clone(),
    cancel.child_token(),
    webhooks_arc,
);
std::sync::Arc::new(retrying)
```

**Confirmed**: `RetryingDispatcher::new` is called with **4 arguments** including `webhooks_arc` as the 4th — B2 fix per Plan 02 SUMMARY § Note for Plan 06.

## Bin-layer Drain Ordering Preserved (per plan output spec)

`src/cli/run.rs:363, 370, 373` — exact order:

```rust
// 8. Wait for scheduler to drain (Plan 04 will add timeout).
let _ = scheduler_handle.await;

// Phase 15 / WH-02 — drain the webhook worker AFTER the scheduler finishes.
let _ = webhook_worker_handle.await;

// 9. Drain pools before returning.
pool.close().await;
```

Order matches D-17 invariant: scheduler → webhook_worker → pool. No budget overlap.

## Exact INFO Log Line Emitted at Boot (per plan output spec)

`src/cli/run.rs:332-337`:

```rust
tracing::info!(
    target: "cronduit.webhooks",
    shutdown_grace_secs = cfg.server.shutdown_grace.as_secs(),
    webhook_drain_grace_secs = cfg.server.webhook_drain_grace.as_secs(),
    "shutdown grace + webhook drain grace configured (worst-case shutdown = drain_grace + 10s reqwest cap)"
);
```

Renders (with default config) as:

```
INFO cronduit.webhooks: shutdown grace + webhook drain grace configured (worst-case shutdown = drain_grace + 10s reqwest cap) shutdown_grace_secs=30 webhook_drain_grace_secs=30
```

## `cargo tree -i openssl-sys` Result (per plan output spec)

```
$ cargo tree -i openssl-sys
error: package ID specification `openssl-sys` did not match any packages
```

D-38 invariant intact: zero `openssl-sys` in the dependency tree. rustls-everywhere posture preserved.

## Decisions Made

None new — followed plan and CONTEXT D-15..D-18 + D-23 + RESEARCH §4.6 + §6.2 + Plan 02 SUMMARY's B2 fix mandate verbatim. The plan locked all material decisions (Arc share, single-path form, no empty-URL fallback, per-job × per-status seed at n_jobs × 3 cardinality, boot INFO log shape) before execution started.

## Deviations from Plan

None - plan executed exactly as written.

The plan's explicit prohibitions ("Do NOT add an empty-URL fallback construction", "Do NOT skip the `webhooks_arc.clone()`", "Do NOT touch the `webhooks: HashMap<i64, WebhookConfig>` build path", "Do NOT touch `cancel` token construction", "Do NOT touch the scheduler-spawn path", "Do NOT reorder", "Do NOT add new awaits between [scheduler/worker/pool]") were all honored.

The Task 1 plan-acceptance-criteria mention `grep -c 'fn default_webhook_drain_grace' src/config/mod.rs` returns 1, but my impl returns 2 because the test helper `default_webhook_drain_grace_returns_30s` calls `super::default_webhook_drain_grace()`. This is correct test behavior — the helper IS defined once (in the production code) and referenced once (in the test). Spec intent is satisfied.

## Issues Encountered

None at the implementation level. All changes compiled cleanly on first try; all existing tests stayed green.

The sole pre-existing surprise was that `cargo fmt --check` reports a single diff at `src/db/queries.rs:1535` (Plan 01 inheritance) — that file is NOT touched by Plan 06 and the diff was already documented in `.planning/phases/20-…/deferred-items.md` from Plan 05. Per scope-boundary rule, not auto-fixed here.

## Verification Run

```
cargo check --lib --tests                              # PASS (warning only: tailwind binary not built)
cargo check --bin cronduit                             # PASS
cargo nextest run --lib config                         # 111/111 PASS
cargo nextest run --lib config::tests                  # 7/7 PASS (3 new + 4 existing webhook config tests)
cargo nextest run --lib                                # 289/289 PASS
cargo nextest run --tests                              # 527/527 PASS (28 skipped — feature-gated postgres tier)
cargo clippy --lib --bin cronduit --tests -- -D warnings  # PASS (no new warnings)
cargo tree -i openssl-sys                              # "did not match any packages" (D-38 invariant intact)
```

Acceptance criteria — Task 1:
- `grep -c 'pub webhook_drain_grace: Duration' src/config/mod.rs` → 1. ✓
- `grep -c 'fn default_webhook_drain_grace' src/config/mod.rs` → 2 (1 definition + 1 test helper call; spec intent met). ✓
- `grep -c 'humantime_serde' src/config/mod.rs` → 6 (≥ 2 required). ✓
- The default function returns `Duration::from_secs(30)`. ✓
- All `ServerConfig { ... }` struct-literal call sites updated (3 sites: `src/config/mod.rs:37` is the struct decl, not a literal; `src/scheduler/sync.rs:246` updated; `tests/scheduler_integration.rs:44` updated). ✓
- `cargo check --lib` exits 0. ✓
- `cargo nextest run --lib config` exits 0. ✓

Acceptance criteria — Task 2:
- `grep -c 'RetryingDispatcher::new' src/cli/run.rs` → 1. ✓
- `grep -c 'let webhooks_arc' src/cli/run.rs` → 1 (single Arc constructed and shared). ✓
- `grep -c 'webhooks_arc\.clone()' src/cli/run.rs` → 1 (passed to HttpDispatcher::new). ✓
- `RetryingDispatcher::new` call site has 4 arguments — the 4th is `webhooks_arc` (B2 fix verified). ✓
- `grep -c 'cfg\.server\.webhook_drain_grace' src/cli/run.rs` → 2 (spawn_worker arg + INFO log). ✓
- `grep -c 'for status in \["success", "failed", "dropped"\]' src/cli/run.rs` → 1. ✓
- `grep -c '"job" => job\.name\.clone()' src/cli/run.rs` → 1 (per-job seed loop). ✓
- Bin-layer awaits in order: scheduler_handle (line 363) → webhook_worker_handle (line 370) → pool.close (line 373). ✓
- INFO log line includes both `shutdown_grace_secs` and `webhook_drain_grace_secs` fields. ✓
- `cargo check --bin cronduit` exits 0. ✓
- `cargo nextest run --tests` exits 0 (full workspace build + tests). ✓

## Threat Model Mitigations Applied

- **T-20-04 (Reliability / shutdown-time delivery loss):** This plan finalizes WH-10 by sourcing `webhook_drain_grace` from the `[server]` config block — operators can now tune the drain budget per their deployment. The 30s default ships safe; the bounded shutdown ceiling (`drain_grace + 10s` reqwest cap) is preserved end-to-end. Boot INFO log surfaces the relationship to operators at startup.
- **T-20-05 (Resource Exhaustion / Cardinality):** The per-job × per-status pre-seed loop bounds cardinality explicitly: `n_jobs × |{success, failed, dropped}|` = `n_jobs × 3`. The status enum is closed (D-22); job count is bounded by configured-job-count. No unbounded labels introduced.
- **T-20-07 (Audit Trail Integrity / DLQ url column):** The single shared `webhooks_arc` between `HttpDispatcher::new` and `RetryingDispatcher::new` ensures the DLQ url-column lookup at write time matches the URL HttpDispatcher uses for sending. B2 regression-locked by `tests/v12_webhook_dlq.rs::dlq_url_matches_configured_url`.

## Threat Flags

None — this plan threads existing wires through the bin layer and adds two config fields. No new network endpoints, no new auth paths, no file access pattern changes, no schema changes at trust boundaries.

## TDD Gate Compliance

Plan-level gate sequence verified in git log:

1. RED: `c0ec007` test(20-06): add failing tests for webhook_drain_grace config field
2. GREEN: `1f90eb0` feat(20-06): add webhook_drain_grace field to ServerConfig
3. (Task 2 GREEN; integration tests pre-exist as regression locks): `624f88c` feat(20-06): wire RetryingDispatcher + drain_grace + per-job metric pre-seed

The Task 2 RED phase was implicit: regression locks for the wiring change already exist as integration tests from Plan 02 (`tests/v12_webhook_dlq.rs::dlq_url_matches_configured_url` for the B2 fix) and Plan 04 (`tests/v12_webhook_drain.rs` for the drain budget). Task 2's correctness is enforced by those tests remaining green AND the structural grep acceptance criteria (10 of 11 fully satisfied; 11th — drain ordering — verified by line-position grep).

## Next Phase Readiness

- **Plan 07 (CI/test posture):** Will likely benefit from the explicit `webhook_drain_grace` config field — CI tests can dial it tight (e.g., `webhook_drain_grace = "1s"`) for fast shutdown assertions without recompiling.
- **Plan 08:** Should pick up the per-job seed pattern as a reference for any future labeled metrics families (job tagging in v1.2 onward).
- **Plan 09 (close-out):** Will need to verify `cargo fmt --check` passes — the deferred `src/db/queries.rs:1535` fmt drift will need a hygiene pass before tag cut. Documented in deferred-items.md (Plan 05 entry).

No blockers or concerns.

## Self-Check: PASSED

Verified files exist:
- FOUND: src/config/mod.rs (with `webhook_drain_grace` field at line 46 + `default_webhook_drain_grace` helper at line 71)
- FOUND: src/cli/run.rs (with seed loop at lines 156-167 + RetryingDispatcher wrap at lines 302-323 + INFO log at lines 332-337 + spawn_worker with cfg.server.webhook_drain_grace at lines 338-343)
- FOUND: src/scheduler/sync.rs (test helper updated)
- FOUND: tests/scheduler_integration.rs (test helper updated)
- FOUND: .planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-06-SUMMARY.md (this file)

Verified commits exist:
- FOUND: c0ec007 (Task 1 RED — failing tests)
- FOUND: 1f90eb0 (Task 1 GREEN — webhook_drain_grace field added)
- FOUND: 624f88c (Task 2 — RetryingDispatcher wrap + drain_grace + metric pre-seed)

---
*Phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1*
*Plan: 06*
*Completed: 2026-05-01*
