---
phase: 15
slug: foundation-preamble
status: verified
threats_open: 0
threats_total: 15
threats_closed: 15
asvs_level: 1
created: 2026-04-26
audited: 2026-04-26
---

# Security Audit — Phase 15: foundation-preamble

**Audited:** 2026-04-26
**ASVS Level:** 1
**block_on:** critical_only
**Threats Closed:** 15/15
**Threats Open:** 0/15

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| Supply-chain (`Cargo.toml` → transitive deps via crates.io) | `deny.toml` IS the boundary that gates which licenses, advisory-tracked CVEs, and duplicate versions enter the cronduit dep graph; cargo-deny runs on every PR | crate metadata, license SPDX strings, advisory IDs |
| CI-runtime (CI tool install vs cronduit binary) | cargo-deny is a CI tool installed fresh per-PR via `taiki-e/install-action@v2`; it is NEVER linked into the cronduit binary | (none — out-of-process tool) |
| In-process trait object boundary (`dyn WebhookDispatcher`) | Worker holds an `Arc<dyn WebhookDispatcher>`; today `NoopDispatcher`. Any future implementor (P18 `HttpDispatcher`) must satisfy `Send + Sync` and the `deliver(&self, &RunFinalized) -> Result<(), WebhookError>` contract | `RunFinalized` event struct (internal-only in P15) |
| Tokio mpsc channel (Sender ↔ Receiver) | One-way dataflow: scheduler is the sole producer (`finalize_run` step 7d); worker is the sole consumer | `RunFinalized` from validated DB columns |
| Bin-layer ↔ scheduler-runtime ↔ worker (lifetime hierarchy) | Bin owns the worker JoinHandle and parent cancel token; scheduler holds child cancel + Sender clones; worker holds Receiver + child cancel | cancellation signals + sender-drop signals |
| Test-process boundary | New `tests/v12_webhook_*.rs` integration crates exercise `cronduit::webhooks::*` public surface only; do NOT bypass runtime trust boundaries | (test fixtures only) |

---

## Threat Verification

| Threat ID | Category | Disposition | Status | Evidence |
|-----------|----------|-------------|--------|----------|
| T-15-01-01 | Tampering | accept | CLOSED | `Cargo.toml:3` — `version = "1.2.0"`; D-16 (git tag = Cargo.toml version) and git history are the audit log. Public build metadata, no runtime trust impact. |
| T-15-02-01 | Tampering | mitigate | CLOSED | `deny.toml:32-38` — exactly 5 SPDX IDs (`MIT`, `Apache-2.0`, `BSD-3-Clause`, `ISC`, `Unicode-DFS-2016`) under `allow = [...]` |
| T-15-02-02 | Information Disclosure | mitigate | CLOSED | `deny.toml:25` — `ignore = []`; `.github/workflows/ci.yml:58` — step-level `continue-on-error: true` ensures advisories surface as warns visible on the PR check list |
| T-15-02-03 | Denial of Service | mitigate | CLOSED | `.github/workflows/ci.yml:58` — `continue-on-error: true` indented at step level (sibling of `run:` at line 57). No job-level `continue-on-error` matches anywhere in the lint job. Pitfall 5 averted. |
| T-15-02-04 | Tampering | mitigate | CLOSED | `deny.toml` — zero occurrences of deprecated cargo-deny v0.19.x keys (`default`, `unlicensed`, `copyleft`, `allow-osi-fsf-free`). Pitfall 4 averted. |
| T-15-02-05 | Tampering | mitigate | CLOSED | `deny.toml:57-64` — `[sources]` block: `unknown-registry = "warn"` (L61), `unknown-git = "warn"` (L62), `allow-registry = ["https://github.com/rust-lang/crates.io-index"]` (L63) |
| T-15-03-01 | Denial of Service | accept | CLOSED | Producer-side guard verified at `src/scheduler/run.rs:427` (`try_send` + drop). Worker `tokio::select!` with `biased;` arm priority at `src/webhooks/worker.rs:55-95`. Scheduler-survival enforced producer-side per design — slow dispatcher slows worker but cannot stall scheduler. |
| T-15-03-02 | Information Disclosure | mitigate | CLOSED | `src/telemetry.rs:111-117` — `describe_counter!("cronduit_webhook_delivery_dropped_total", ...)` paired with `src/telemetry.rs:133` — `metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(0)`. End-to-end verified by `tests/metrics_endpoint.rs:73-78` HELP/TYPE assertions. Pitfall 3 averted. |
| T-15-03-03 | Tampering | mitigate | CLOSED | `src/webhooks/dispatcher.rs:19` (`#[async_trait]` on trait declaration) and `src/webhooks/dispatcher.rs:27` (on `NoopDispatcher` impl block). 2 occurrences confirmed. Pitfall 2 (native dyn async-fn-in-trait object-safety failure on Rust 1.94.1) averted. |
| T-15-04-01 | Denial of Service | mitigate | CLOSED | `src/scheduler/run.rs:427` — `webhook_tx.try_send(event)`. Recursive grep across `src/` returns ZERO matches for `webhook_tx.send(` or `webhook_tx.send_blocking`. Only string hit (`tests/v12_webhook_scheduler_unblocked.rs:7`) is a `//!` doc comment describing the prohibited pattern. Pitfall 1/28 averted at producer side. |
| T-15-04-02 | Tampering | mitigate | CLOSED | `src/scheduler/run.rs` — exactly one `// 7d.` (L404, NEW webhook emit) and one `// 7e.` (L451, renumbered cleanup). Pitfall 6 averted. |
| T-15-04-03 | Denial of Service | mitigate | CLOSED (with WR-01 follow-up) | `src/cli/run.rs:275,282` — `scheduler_handle.await` (L275) STRICTLY before `webhook_worker_handle.await` (L282). Await-ordering mitigation as scoped in PLAN-04 is verified. **Note:** Phase 15 code review surfaced WR-01 — both worker and scheduler share `cancel.child_token()` derivation, so the worker's cancel arm can fire the same instant as the scheduler's, allowing the worker to exit while `finalize_run` still runs during the grace period. This is a SHUTDOWN-TIME signal coordination issue distinct from T-15-04-03's await-ordering scope; tracked as a Phase 24 hardening candidate (does not invalidate the verified mitigation under `block_on: critical_only`). |
| T-15-04-04 | Information Disclosure | accept | CLOSED | Coexistence documented in `15-CONTEXT.md` § Specifics: P15 unlabeled counter = "queue saturated event count" (worker-side); P20 labeled `cronduit_webhook_deliveries_total{status="dropped"}` = "delivery outcome count" (HTTP-side). Help text on the describe call at `src/telemetry.rs:111-117` forward-references P20 / WH-11. |
| T-15-05-01 | Denial of Service | mitigate | CLOSED | `tests/v12_webhook_queue_drop.rs:140,146` — `cancel.cancel(); drop(_worker_handle);`; `tests/v12_webhook_scheduler_unblocked.rs:119,120` — same pair. Cancel-arm fire interrupts `StalledDispatcher`'s 60s sleep at the next select! check; test process exits in well under 30s without leaking the worker. |
| T-15-05-02 | Tampering | accept | CLOSED | Drop counter delta + scheduler drift are operator-observable surfaces per ROADMAP § Phase 15 Success Criteria #3 and #4. The test thresholds (≥10 drops, < 1s drift, < 5ms per emit) match operator-observable acceptance criteria. |

---

## Accepted Risks Log

| Threat ID | Rationale |
|-----------|-----------|
| T-15-01-01 | `Cargo.toml [package].version` is public build metadata; git history is the audit log. D-16 enforces git tag = Cargo.toml version for every v1.2 tag. No runtime trust impact. |
| T-15-03-01 | The webhook worker's `tokio::select!` `biased;` arm gives `rx.recv()` priority but cannot prevent a dispatcher implementation that blocks indefinitely from slowing the worker. The scheduler-survival contract is enforced at the **producer** side (`try_send` + drop) per CONTEXT.md design intent — not at the consumer side. A misbehaving P18 dispatcher impl would slow webhook delivery (queue fills → drops → counter increments → operator alerts) but cannot stall the scheduler. |
| T-15-04-04 | The unlabeled `cronduit_webhook_delivery_dropped_total` counter intentionally coexists with the future P20 labeled family (`cronduit_webhook_deliveries_total{status="dropped"}`). They measure semantically distinct things: P15 measures queue-saturation events at the worker boundary (Pitfall 1 visibility); P20 measures HTTP delivery outcomes at the network boundary. CONTEXT.md § Specifics documents the divergence; the describe-counter help text forward-references P20 / WH-11. |
| T-15-05-02 | The test thresholds (≥10 drops with capacity 4 + 20 events; < 1s scheduler drift across 5 ticks; < 5ms per `try_send` emit) are operator-observable contracts derived from ROADMAP § Phase 15 Success Criteria. They do not assert internal scheduler implementation details. |

---

## Unregistered Threat Flags

None. SUMMARY files (15-01..05) all explicitly note "No threat flags to record." No new attack surface appeared during implementation that lacks a threat-register mapping.

---

## Notes

**WR-01 follow-up (Phase 24 candidate):** The Phase 15 code review (`15-REVIEW.md` WR-01) flagged that `cancel.child_token()` derivation is shared between the scheduler and the worker, so when SIGTERM fires `cancel.cancel()`, the worker's `cancel.cancelled()` arm is ready at the same instant the scheduler's is. The `biased;` directive in `worker.rs` does NOT prevent this: as soon as the channel goes empty for a moment during the scheduler's grace-period drain, the cancel arm wins. Events that the still-draining scheduler emits afterward hit either a worker that already exited (drop with no counter increment) or a closed channel (`TrySendError::Closed` error log per drained run). This is distinct from T-15-04-03's narrowly-scoped await-ordering mitigation; the threat-register-declared mitigation is verified, but operators should track WR-01 as a Phase 24 cancel-token-hierarchy refactor candidate before the final v1.2.0 ship. Recommended fix: give the worker its OWN cancel token derived later in the shutdown sequence, or use sender-drop (close the mpsc) as the only worker-shutdown signal.

**T-15-04-04 P20 label-family relationship:** The unlabeled drop counter shipped in P15 will NOT be removed when P20 lands the labeled family. They serve different purposes (saturation event vs delivery outcome). Operators alerting on either will continue to see consistent surfaces. If this changes during P20 implementation, this Notes section should be updated to record the deprecation path.

**rc.1 cargo-deny posture (T-15-02-01, T-15-02-02):** The first run of `just deny` surfaces 1 advisory (`RUSTSEC-2026-0104` rustls-webpki CRL panic — not exploitable in cronduit's code path) and 5 transitive license findings (`Unicode-3.0`, `Zlib`, `CC0-1.0`, `CDLA-Permissive-2.0`, dual-licensed combos from `icu_*` crates). All are surfaced as warn-level CI status (yellow on PR check list) without blocking, per the rc.1 posture documented in 15-02-PLAN.md. Phase 24 promotes both `continue-on-error: true` removal AND `bans.multiple-versions = "deny"` flip; these advisory items must be resolved (or explicitly accepted in `[advisories].ignore`) before the gate-flip.

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-04-26 | 15 | 15 | 0 | gsd-security-auditor (initial) |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter
- [x] Phase 24 follow-ups recorded (WR-01 cancel-hierarchy refactor; advisory/license resolution before gate-flip)

**Approval:** verified 2026-04-26
