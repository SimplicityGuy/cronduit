# Phase 15: Foundation Preamble - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-25
**Phase:** 15-foundation-preamble
**Areas discussed:** Worker P15 scope, RunFinalized event shape, Scheduler integration & wiring, Emit point, cargo-deny config location, `just deny` recipe, CI placement, Non-blocking mechanism, Bans strictness, Metric family scope, Plan sequencing

---

## Worker P15 scope

| Option | Description | Selected |
|--------|-------------|----------|
| Trait + no-op default | Define `WebhookDispatcher` trait + ship `NoopDispatcher` returning Ok after debug log; P18 swaps in HttpDispatcher. Locks the seam for P18+. | ✓ |
| Pure plumbing demo | Worker just logs at debug + handles drop-counter on overflow; no abstraction. P18 grows worker body in place. | |
| Stub HTTP against no-op | Wire `reqwest` now and POST a placeholder payload with no signing/retry. | |

**User's choice:** Trait + no-op default (Recommended)
**Notes:** Most testable; locks the seam P18+ implements against; trivial mock for tests. Pure plumbing rejected because P18 would have to refactor the worker body when HTTP arrives. Stub-HTTP rejected because it violates the P15 phase boundary (pulls TLS/payload questions in).

---

## RunFinalized event shape

| Option | Description | Selected |
|--------|-------------|----------|
| Self-contained minimum | `RunFinalized { run_id, job_id, job_name, status, exit_code, started_at, finished_at }`. Streak/digest/config_hash come from P16's get_failure_context() at delivery time. | ✓ |
| run_id only | Channel carries just `run_id`; worker SELECTs everything from job_runs + jobs at delivery time. | |
| Full WH-09 payload | Assemble complete payload (streak_position, image_digest, config_hash, tags) at emit time inside finalize_run. | |

**User's choice:** Self-contained minimum (Recommended)
**Notes:** Avoids N DB reads per delivery; doesn't couple worker to DB pool; doesn't lock schema before P16 lands config_hash/streak helpers. Stable struct against P16's schema work.

---

## Scheduler integration & wiring

| Option | Description | Selected |
|--------|-------------|----------|
| Always-on, always-spawned | `Scheduler.webhook_tx: mpsc::Sender<RunFinalized>` (not Option). Worker spawned at startup unconditionally with NoopDispatcher. finalize_run always calls try_send. | ✓ |
| Conditional spawn + Option<tx> | Inspect config; skip spawning worker when no jobs/defaults have webhooks; finalize_run guards on `if let Some(tx)`. | |
| Always-on but lazily started on first config sync | Worker is part of scheduler struct but spawn happens inside first apply_config call. | |

**User's choice:** Always-on, always-spawned (Recommended)
**Notes:** Symmetric (one code path); integration tests don't need a config-conditional shape; foundation phase exercises the channel even when no webhooks are configured. Conditional spawn rejected for adding two code paths to test for negligible runtime savings.

---

## Emit point

| Option | Description | Selected |
|--------|-------------|----------|
| After step 7c sentinel | Order: DB finalize → metrics → SSE sentinel → webhook emit. Webhook is informational; emit last. | ✓ |
| After step 7b metrics | Emit between metrics and SSE sentinel. | |
| Inside step 7 finalize_run | Co-locate with DB UPDATE return Ok. | |

**User's choice:** After step 7c sentinel (Recommended)
**Notes:** User-visible signals (DB row, metrics, log stream sentinel) ordered before any external side-effect attempt. A drop here is operationally less harmful than a drop before metrics or SSE.

---

## cargo-deny config location

| Option | Description | Selected |
|--------|-------------|----------|
| deny.toml at project root | Standard cargo-deny convention; auto-discovered without `--config`. | ✓ |
| .cargo/deny.toml | Tucks inside .cargo/. Less discoverable; cargo-deny works via `--config` but no upside. | |

**User's choice:** deny.toml at project root (Recommended)
**Notes:** Standard convention; visible alongside Cargo.toml; discoverable for OSS contributors.

---

## `just deny` recipe

| Option | Description | Selected |
|--------|-------------|----------|
| New `just deny` recipe | Mirrors `just clippy`, `just fmt-check`. Locks the project's just-only convention from Phase 1 D-10 / FOUND-12. | ✓ |
| Inline `cargo deny check` in CI step | Skips just recipe; CI runs cargo command directly. Violates the just-only convention. | |

**User's choice:** New `just deny` recipe (Recommended)
**Notes:** Project rule (Phase 1 D-10): every CI step invokes `just <recipe>`, never inline cargo. Recipe runs `cargo deny check advisories licenses bans`.

---

## CI placement

| Option | Description | Selected |
|--------|-------------|----------|
| New step in existing `lint` job | Lands after `just clippy` and `just openssl-check`; reuses toolchain checkout + rust-cache; install via `taiki-e/install-action@v2 with: tool: cargo-deny`. | ✓ |
| Separate `deny` job | New top-level job on its own runner; more PR-status visibility but adds ~30s cold-start + cache duplication on every PR. | |

**User's choice:** New step in existing `lint` job (Recommended)
**Notes:** No extra runner cold-start; PR check list grows by one row, not one job; reuses Swatinem/rust-cache@v2 setup.

---

## Non-blocking mechanism

| Option | Description | Selected |
|--------|-------------|----------|
| `continue-on-error: true` on the step | Single-line GHA flag; failure shows as yellow warning but doesn't fail the job. Promote to blocking by removing the flag in P24. | ✓ |
| Separate `deny.yml` workflow with continue-on-error | Standalone workflow file. More configurable but adds a second workflow to maintain. | |
| Conditional skip via commit message | Brittle; relies on a convention nobody enforces. | |

**User's choice:** `continue-on-error: true` on the step (Recommended)
**Notes:** Self-documenting; one-line removal in Phase 24's pre-final cleanup; no separate workflow file.

---

## Bans strictness on rc.1

| Option | Description | Selected |
|--------|-------------|----------|
| Warn-only via `multiple-versions = "warn"` | Cargo-deny logs duplicate-version output but bans subcommand returns success. Combined with step-level continue-on-error → two layers of "non-blocking initially". | ✓ |
| Hard `deny` with empty allowlist now | `multiple-versions = "deny"` from rc.1. Likely fails on common transitive duplicates. Forces allowlist curation effort into P15 — adds scope. | |

**User's choice:** Warn-only via `multiple-versions = "warn"` (Recommended)
**Notes:** Two layers of non-blocking gives planner-resilience: a future planner removing one of the two layers still leaves CI tolerant. Phase 24 promotes both.

---

## Metric family scope at boot

| Option | Description | Selected |
|--------|-------------|----------|
| Drop counter only | Eagerly describe + zero-baseline only `cronduit_webhook_delivery_dropped_total` in P15. Rest of family lands with WH-11 in P20. | ✓ |
| Full family eagerly described | Describe all four `cronduit_webhook_*` families at boot in P15, even though P15 only emits the dropped counter. | |
| Drop counter + queue_depth gauge | Describe drop counter AND queue_depth gauge (worker can update on every send/recv). Skip histogram and per-status counter until P20. | |

**User's choice:** Drop counter only (Recommended)
**Notes:** Keeps `/metrics` honest — describing a metric with no producer creates flat-zero series in operator dashboards built against rc.1 and erodes trust in the metric family. The drop counter is the only one P15 can emit values to.

---

## Plan sequencing within Phase 15

| Option | Description | Selected |
|--------|-------------|----------|
| Bump → deny → webhook worker | Plan 15-01: Cargo bump (mirrors Phase 10 D-12 — first commit of milestone). Plan 15-02: cargo-deny preamble. Plan 15-03..N: webhook worker scaffold. | ✓ |
| Bump → worker → deny | Worker before deny. Risk: deny would discover any new transitive dups introduced by worker work BEFORE the bans=warn posture is in place. | |
| Single combined plan / PR | All three deliverables in one large PR. Smaller phase ceremony but harder to revert any single piece. | |

**User's choice:** Bump → deny → webhook worker (Recommended)
**Notes:** Bump first is the project rule (Phase 10 D-12). Deny before worker means the worker PR is reviewed under the active cargo-deny check. Single combined PR rejected for poor revertability on review findings.

---

## Claude's Discretion

The following P15 choices were left to the planner:

- Crate decision: use `async-trait` (verify if already a transitive dep) for the trait shape, or use a non-trait `enum WebhookDispatcher { Noop, Http(...) }` and dispatch via match.
- Channel-receiver-loop shape inside `src/webhooks/mod.rs` (`tokio::select!` between `rx.recv()` and a shutdown `CancellationToken` is the obvious pattern).
- `WebhookError` enum shape in P15 — single `DispatchFailed(String)` variant or skip the enum and have NoopDispatcher return `Result<(), Infallible>`.
- Module split inside `src/webhooks/`: single `mod.rs` vs split into `dispatcher.rs` / `event.rs` / `worker.rs`.
- Whether `15-HUMAN-UAT.md` is needed for the drop-counter scenario specifically.
- Specific integration-test file names following the `vNN_<feature>_<scenario>.rs` convention.
- Specific `bans.skip = [...]` entries (empty is fine for rc.1).
- Whether `cronduit_webhook_delivery_dropped_total` description text mentions the future P20 labeled equivalent.

## Deferred Ideas

Captured in CONTEXT.md `<deferred>` section:

- Full `cronduit_webhook_*` metric family (`deliveries_total{status}`, `delivery_duration_seconds{job}`, `queue_depth`) → Phase 20 / WH-11
- `queue_depth` gauge in P15 (middle-ground option) → Phase 20 / WH-11
- Stub HTTP delivery against a no-op endpoint → Phase 18+
- Conditional worker spawn based on config → revisit if future profiling shows inert-worker overhead
- Webhook persistence to disk (durable queue) → REQUIREMENTS.md Future Requirements (v1.3+)
- `webhook_drain_grace = "30s"` graceful-shutdown drain accounting → Phase 20 / WH-10
- License allowlist exact crate enumeration → P15 Plan 15-02 implementation step
- Specific RustSec advisory ID allowlisting → P15 Plan 15-02 implementation step
- `15-HUMAN-UAT.md` scope → planner discretion
