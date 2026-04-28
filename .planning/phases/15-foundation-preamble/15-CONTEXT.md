# Phase 15: Foundation Preamble - Context

**Gathered:** 2026-04-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Establish the v1.2 hygiene baseline and lock the webhook delivery worker isolation pattern before any payload/signing/posture work depends on it. Three independent deliverables in one phase:

1. **FOUND-15** — `Cargo.toml` version bumps from `1.1.0` to `1.2.0` on the very first v1.2 commit (mirrors Phase 10 D-12). `cronduit --version` reports `1.2.0` from the start of the v1.2 development window. rc tags use the semver pre-release format `v1.2.0-rc.1`, …, final ship is `v1.2.0`.
2. **FOUND-16** — `cargo-deny` CI preamble (advisories + licenses + duplicate-versions). License allowlist matches the v1.0/v1.1 posture (MIT/Apache-2.0/BSD-3-Clause/ISC/Unicode-DFS-2016 plus project-specific exceptions). Non-blocking on rc.1; promoted to blocking before final `v1.2.0` (Phase 24).
3. **WH-02** — New module `src/webhooks/mod.rs` owning a dedicated tokio task that consumes `RunFinalized` events from a bounded `tokio::sync::mpsc::channel(1024)`. The scheduler emits via `try_send` (NEVER `await tx.send()`); on full queue the event is dropped with a warn-level log + `cronduit_webhook_delivery_dropped_total` counter increment. The scheduler loop is never blocked by outbound HTTP.

**Out of scope for Phase 15** (deferred to downstream phases — do not creep): payload schema (P18 / WH-03 + WH-09), HMAC signing + receiver examples (P19 / WH-04), retry logic + jitter + dead-letter table (P20 / WH-05), URL validation / SSRF posture (P20 / WH-07 + WH-08), graceful-shutdown drain accounting (P20 / WH-10), the rest of the `cronduit_webhook_*` metric family (P20 / WH-11), state-filter + coalescing (P18 / WH-01 + WH-06), `webhooks` config schema parsing (P18 — only the channel-message struct is locked here).

</domain>

<decisions>
## Implementation Decisions

### Webhook worker scope (P15)

- **D-01:** **Trait-based dispatcher with no-op default.** Define a `WebhookDispatcher` trait inside `src/webhooks/mod.rs` (or `src/webhooks/dispatcher.rs` if the planner prefers a split):

  ```rust
  #[async_trait::async_trait]
  pub trait WebhookDispatcher: Send + Sync {
      async fn deliver(&self, event: &RunFinalized) -> Result<(), WebhookError>;
  }
  ```

  Ship a `NoopDispatcher` that returns `Ok(())` after a `tracing::debug!` log line. P18 swaps in `HttpDispatcher` against the same trait without touching the worker loop. This locks the seam P18 implements against and makes the worker trivially testable in P15 (mock dispatcher in tests).

  **Rationale:** Minimal P15 surface area while still locking the architectural seam. Pure plumbing (no abstraction) was rejected because P18 would have to refactor the worker body the moment HTTP arrives. Stub-HTTP-against-no-op was rejected because it pulls P18's TLS/payload questions into P15.

### `RunFinalized` channel-message contract

- **D-02:** **Self-contained minimum payload on the channel.** The struct passed through the bounded mpsc carries only what the worker needs to log/route without a DB round-trip:

  ```rust
  #[derive(Debug, Clone)]
  pub struct RunFinalized {
      pub run_id: i64,
      pub job_id: i64,
      pub job_name: String,
      pub status: String,         // "success" | "failed" | "timeout" | "stopped" | "cancelled" | "error"
      pub exit_code: Option<i32>,
      pub started_at: chrono::DateTime<chrono::Utc>,
      pub finished_at: chrono::DateTime<chrono::Utc>,
  }
  ```

  Streak metrics (`streak_position`, `consecutive_failures`), `image_digest`, `config_hash`, and `tags` come from P16's `get_failure_context(job_id)` helper inside the dispatcher at delivery time — they are NOT carried on the channel. This keeps the P15 message stable against P16's schema work and avoids forcing a struct rename later.

  **Rejected:** `run_id`-only (forces N DB reads per delivery and couples worker to DB pool); full WH-09 payload (locks schema before P16 lands `config_hash`/streak helpers — risky).

### Scheduler integration & wiring

- **D-03:** **Always-on, always-spawned worker.** `Scheduler` (or `SchedulerLoop`) gains a non-`Option` field `webhook_tx: tokio::sync::mpsc::Sender<RunFinalized>`. The worker task is spawned at scheduler startup unconditionally with the `NoopDispatcher`. `finalize_run` always calls `webhook_tx.try_send(...)`. When no webhooks are configured, events flow through the channel and the no-op dispatcher drains them at debug level.

  **Rationale:** Symmetric (one code path, not two), easy to reason about, integration tests don't need a config-conditional shape, and the foundation phase exercises the channel even when `[defaults].webhook` and per-job `webhook = ...` are both absent — which is the rc.1 reality. Conditional spawn was rejected for adding two code paths to test for negligible runtime savings.

- **D-04:** **Drop semantics.** When `try_send` returns `TrySendError::Full`:
  1. Log at `warn` level with target `cronduit.webhooks` and structured fields `run_id`, `job_id`, `status`, plus a static message about queue saturation.
  2. Increment `cronduit_webhook_delivery_dropped_total` (closed-cardinality, no labels in P15 — labels arrive in P20 / WH-11).
  3. Do NOT block the calling task. The scheduler-loop survival contract is the load-bearing reason this counter exists.

  When `try_send` returns `TrySendError::Closed` (worker has gone away — should not happen unless the scheduler is shutting down): log at `error` level once per occurrence (not rate-limited in P15; P20's drain machinery will refine this).

- **D-05:** **Emit point inside `finalize_run` (`src/scheduler/run.rs`).** Place the `try_send` call AFTER the existing step 7c sentinel broadcast (`__run_finished__` log frame to live SSE subscribers). Final order:
  1. Log writer drained (existing step 6)
  2. DB UPDATE via `finalize_run(...)` (existing step 7)
  3. Prometheus counter/histogram increments (existing step 7b)
  4. Broadcast `__run_finished__` sentinel to SSE subscribers (existing step 7c)
  5. **NEW:** `webhook_tx.try_send(RunFinalized { ... })` with the drop-handling from D-04

  **Rationale:** Webhook is strictly informational — emitting last keeps user-visible signals (DB row, metrics, log stream sentinel) ordered before any external side-effect attempt. A drop here is operationally less harmful than a drop before metrics or SSE.

### cargo-deny CI integration

- **D-06:** **`deny.toml` lives at project root** (peer to `Cargo.toml` and `justfile`). Standard cargo-deny convention; `cargo deny check` auto-discovers without `--config`. Discoverable for OSS contributors browsing the repo.

- **D-07:** **New `just deny` recipe in `justfile`.** Runs `cargo deny check advisories licenses bans` (single invocation; `bans` covers duplicate-versions). Mirrors `just clippy`, `just fmt-check`, `just openssl-check`. Locks the project's "every CI step invokes `just <recipe>` exclusively" rule (Phase 1 D-10 / FOUND-12) — no inline `cargo` commands in CI yaml.

- **D-08:** **CI placement: new step inside the existing `lint` job in `.github/workflows/ci.yml`.** Step lands after `just openssl-check` and `just grep-no-percentile-cont`, before the test matrix runs. cargo-deny is installed via `taiki-e/install-action@v2` with `tool: cargo-deny` (already used for nextest). Reuses the same toolchain checkout + `Swatinem/rust-cache@v2` setup; no extra runner cold-start. Separate `deny:` job was rejected for the cold-start tax with no operator-visible benefit (PR check list still grows by one row).

- **D-09:** **Non-blocking via `continue-on-error: true` on the cargo-deny CI step.** Single-line GHA flag; the step still reports failure as a yellow warning in the PR check list but does not fail the `lint` job overall. Promotion to blocking (Phase 24, before final `v1.2.0`) is a one-line removal of the flag. Self-documenting; no separate workflow file.

- **D-10:** **`bans.multiple-versions = "warn"` in deny.toml on rc.1.** Cargo-deny logs duplicate-version findings but the bans subcommand returns success. Combined with D-09's step-level `continue-on-error`, this gives two layers of "non-blocking initially" so a transient `cargo update` change cannot redden CI in v1.2 hands. Phase 24 promotes to `multiple-versions = "deny"` with a curated `skip = [...]` allowlist for transitive duplicates we can't fix (e.g., windows-sys families). Hard-deny-from-rc.1 was rejected because it would force the allowlist curation into P15, expanding the foundation phase's scope.

  **License allowlist:** Inherits exactly from v1.0/v1.1 posture — `MIT`, `Apache-2.0`, `BSD-3-Clause`, `ISC`, `Unicode-DFS-2016`. If any current dependency surfaces a license outside this set during P15 implementation, document the addition in deny.toml with a comment pointing at the dep + crates.io rationale.

  **Advisories:** `cargo deny check advisories` runs against the RustSec advisory DB (the default). No allowlisted advisory IDs in rc.1; any active advisory will surface as a warn-level finding via D-09.

### Webhook metric family scope at boot

- **D-11:** **Eagerly describe + zero-baseline only `cronduit_webhook_delivery_dropped_total` in P15.** Add to `src/telemetry.rs` alongside the existing v1.0 OPS-02 family (described at boot via `metrics::describe_counter!` then `metrics::counter!(...).increment(0)` to force registry registration; matches the existing `cronduit_runs_total` pattern at `src/telemetry.rs:99-125`). The remaining `cronduit_webhook_*` families (`deliveries_total{status}`, `delivery_duration_seconds`, `queue_depth`) land in P20 alongside their actual emit sites in WH-11.

  **Rationale:** Keep `/metrics` honest — describing a metric that has no producer would create a flat-zero series in operator dashboards built against rc.1 and erode trust in the metric family. The drop counter is the only one we can actually emit values to in P15.

  Drop counter has zero labels in P15 (single global counter). Per-job labeling lands with WH-11 in P20.

### Plan sequencing within Phase 15

- **D-12:** **Atomic plan order: bump → deny → webhook worker.** Plans land in three separate atomic commits:
  - **Plan 15-01:** `Cargo.toml` `1.1.0` → `1.2.0` bump. Single-line change. Mirrors Phase 10 D-12 — the very first commit of the v1.2 milestone, guaranteeing `cronduit --version` reports `1.2.0` from the start of the development window. No drift between in-flight milestone version and the binary.
  - **Plan 15-02:** cargo-deny preamble. Adds `deny.toml`, `just deny` recipe, and the new `lint`-job step in `ci.yml` with `continue-on-error: true`. Lands BEFORE the worker work so the worker PR is reviewed under the new `cargo-deny check` posture (false-alarm-tolerant via D-09 + D-10).
  - **Plan 15-03..N:** webhook worker scaffold. Splits naturally into: (a) module skeleton + `RunFinalized` struct + `WebhookDispatcher` trait + `NoopDispatcher`; (b) bounded `mpsc` channel + worker spawn + drain loop; (c) scheduler wiring (`webhook_tx` field, `try_send` in `finalize_run` step 7c); (d) drop counter + telemetry registration; (e) integration tests covering try_send-success, try_send-full-drops-with-counter, and worker-shutdown-on-channel-close. Planner has discretion on plan-count and grouping within this list.

  **Rationale:** Bump first is the project rule (Phase 10 D-12). Deny before worker means the worker PR is reviewed under the active cargo-deny check — any new transitive duplicates introduced by `reqwest` foundations (none expected in P15, but prudent against future scaffolding) surface as warns in PR checks instead of hidden landmines. Single combined PR was rejected for poor revertability on review findings.

### Project-rule reaffirmations (carried from prior phases — restated for downstream agents)

- **D-13:** All changes land via PR on a feature branch. No direct commits to `main`. (Project rule, REQUIREMENTS.md and PROJECT.md.)
- **D-14:** All diagrams in any artifact (planning docs, README, PR descriptions, code comments) are mermaid code blocks. No ASCII art. (Project rule.)
- **D-15:** All UAT steps reference an existing `just` recipe. No ad-hoc `cargo`/`docker`/curl-URLs in UAT step text. (Project rule.)
- **D-16:** The git tag and `Cargo.toml` `version` field always match. Prefer full three-part semver (`v1.2.0`, `v1.2.0-rc.1`). Plan 15-01 enforces this on the first v1.2 commit. (Project rule.)
- **D-17:** UAT items in `15-HUMAN-UAT.md` (if produced by the planner) are validated by the maintainer running them locally — never marked passed from Claude's own runs. (Project rule.)

### Claude's Discretion

- Exact crate name choice: `async-trait` already in `Cargo.toml`? If not, planner decides whether to add it for D-01 or to use a non-trait `enum WebhookDispatcher { Noop, Http(HttpDispatcher) }` shape and dispatch via `match`. Both are acceptable; the trait shape is conventional for Rust and easier for tests, but the enum shape avoids one new dep. Verify before adding.
- Exact channel-receiver-loop shape inside `src/webhooks/mod.rs` — `tokio::select!` between `rx.recv()` and a shutdown `CancellationToken` is the obvious pattern; planner picks. Drain semantics on `recv()` returning `None` are: log info, exit task cleanly.
- The `WebhookError` enum's variants in P15 — planner can stub a single `WebhookError::DispatchFailed(String)` for P18 to expand, or skip the enum entirely for P15 and have `NoopDispatcher::deliver` return `Result<(), Infallible>`. Either is fine; the trait signature is what locks the seam.
- Name of the module split inside `src/webhooks/`: `mod.rs` may grow to host the trait, struct, and worker — or planner may split into `dispatcher.rs` (trait + Noop), `event.rs` (`RunFinalized` struct), and `worker.rs` (channel + spawn) from day one. Phase 15 has discretion either way; downstream phases will naturally split as P18+ adds payload/HMAC/HTTP.
- Whether `15-HUMAN-UAT.md` is needed at all. Phase 15's three deliverables are CI-observable: `cronduit --version`, the cargo-deny PR check, and a synthetic-load integration test for the drop counter. A maintainer UAT runbook is worthwhile for the drop-counter overflow scenario specifically (success criterion 4) since it's race-y to write as a unit test. Planner decides scope.
- Specific names for the integration tests covering the drop counter, channel saturation, and scheduler-not-blocked guarantees (e.g., `tests/v12_webhook_queue_drop.rs`, `tests/v12_webhook_scheduler_unblocked.rs`) — planner picks per the project's existing test-file naming convention.
- Whether to add a `bans.skip = [...]` allowlist entry for any specific transitive duplicate during plan-15-02 or to leave the allowlist empty and accept the warn-only output. Empty is fine for rc.1 per D-10.
- Whether the `cronduit_webhook_delivery_dropped_total` counter description text in `telemetry.rs` mentions the future `dropped` label value of `cronduit_webhook_deliveries_total{status}` (P20). Planner picks; either is fine.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level

- `.planning/PROJECT.md` — v1.2 milestone intent (§ Current Milestone), locked tech-stack constraints (§ Constraints), v1.2 inherited decisions (§ Key Decisions). FOUND-15 + FOUND-16 + WH-02 are the canonical requirement IDs Phase 15 satisfies.
- `.planning/REQUIREMENTS.md` § Foundation (FOUND) — FOUND-15 (Cargo bump), FOUND-16 (cargo-deny preamble + license allowlist + non-blocking-then-blocking posture). § Webhooks (WH) — WH-02 (delivery worker shape: bounded mpsc(1024), try_send, drop counter, scheduler-unblocked guarantee). T-V12-WH-03 + T-V12-WH-04 are the verification locks for the scheduler-survival contract.
- `.planning/ROADMAP.md` § "Phase 15: Foundation Preamble" — goal, success criteria (4 operator-observable behaviors), depends-on (v1.1.0 shipped), v1.2 build-order graph (P15 must land before P16/P17/P18/P22).
- `.planning/STATE.md` § Accumulated Context → Decisions — v1.2 inherited decisions (worker shape, mpsc(1024), try_send, drop semantics, cargo-deny posture).
- `.planning/MILESTONES.md` — historical milestone shape; v1.2 entry is empty until Phase 24 close-out.

### Research

- `.planning/research/SUMMARY.md` — Executive summary, integration map, build order, open questions resolved at requirements step. The "Webhook isolation" architectural lynch-pin paragraph is the canonical motivation for WH-02. § Build Order locks "P15 (webhook delivery worker) must land before P16/P17/P18".
- `.planning/research/STACK.md` — `reqwest 0.12.28` rustls-only + `hmac 0.13` are the two new crates. P15 introduces NEITHER (those land in P19/P20). P15 stays on the existing dep set; only `tokio::sync::mpsc` (already a dep) is required for the channel. **Verify before plan: confirm `async-trait` is or isn't already a transitive dep.**
- `.planning/research/ARCHITECTURE.md` — § Webhook delivery isolation (the load-bearing analysis). § "v1.2 modules / surfaces" lists `src/webhooks/mod.rs` as new + the integration sites (scheduler emit, finalize_run hook). NOTE: this doc references the run.rs:277 bug — that's Phase 16 territory, NOT Phase 15.
- `.planning/research/PITFALLS.md` Pitfall 28 (blocking the scheduler loop on outbound HTTP) — prevention is the bounded `mpsc(1024)` + `try_send` pattern locked here. Pitfall 38 (retry thundering herd) is downstream (P20). § T-V12-WH-03 and T-V12-WH-04 are the test-case identifiers for scheduler survival under queue saturation.
- `.planning/research/FEATURES.md` § Webhook notifications — feature landscape and Standard Webhooks v1 spec adherence as the differentiator. P15 ships none of the spec-visible surface; that's P18+. Useful for downstream-phase orientation only.

### Phase 10 precedent (the v1.1 hygiene preamble — same shape as Phase 15's bump + deny)

- `.planning/milestones/v1.1-phases/10-stop-a-running-job-hygiene-preamble/10-CONTEXT.md` § Hygiene Preamble (D-12, D-13) — pattern for "Cargo.toml bump as the very first commit" and clean separation of hygiene plans from feature plans within the same phase. Phase 15's D-12 (plan order) is a structural copy.

### Source files the phase touches

- `Cargo.toml` — `version` field at L3 (`1.1.0` → `1.2.0`). Plan 15-01 only.
- `Cargo.toml` — `[dependencies]` block. Plan 15-03+ may add `async-trait` if not already present (D-01 / Claude's Discretion bullet).
- `justfile` — new `deny:` recipe (peer to `clippy:`, `fmt-check:`, `openssl-check:`). Plan 15-02.
- `deny.toml` — NEW file at project root. Plan 15-02. License allowlist matches v1.0/v1.1 posture (MIT/Apache-2.0/BSD-3-Clause/ISC/Unicode-DFS-2016). `bans.multiple-versions = "warn"` initially.
- `.github/workflows/ci.yml` — `lint` job. Plan 15-02 adds two steps: `taiki-e/install-action@v2 with: tool: cargo-deny` then `run: just deny` with `continue-on-error: true`. Lands after `just grep-no-percentile-cont` (currently last step in `lint` at L46).
- `src/lib.rs` — register the new `webhooks` module (`pub mod webhooks;`). Plan 15-03.
- `src/webhooks/mod.rs` — NEW module. Holds (or re-exports from sibling files) `RunFinalized`, `WebhookDispatcher` trait, `NoopDispatcher`, `WebhookError`, the worker entry-point function. Plan 15-03+.
- `src/scheduler/mod.rs` — `Scheduler` / `SchedulerLoop` struct gains a `webhook_tx: tokio::sync::mpsc::Sender<crate::webhooks::RunFinalized>` field (D-03). Worker is spawned at scheduler `new()` / startup with the `NoopDispatcher`. Plan 15-03+.
- `src/scheduler/run.rs` — `finalize_run` after step 7c (currently L355+ comment block) gains the `webhook_tx.try_send(RunFinalized { ... })` call with `TrySendError::Full` → `warn!` log + counter increment, `TrySendError::Closed` → `error!` log. (D-04, D-05).
- `src/telemetry.rs` — between L107 (existing `cronduit_run_failures_total` describe block) and L126 (existing zero-baseline registrations), add `metrics::describe_counter!("cronduit_webhook_delivery_dropped_total", "...")` and `metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(0)` (D-11). Match the existing eager-describe + zero-baseline pattern verbatim.
- `tests/v12_webhook_queue_drop.rs` — NEW integration test (Plan 15-03+). Saturates the bounded channel with > 1024 events; asserts `cronduit_webhook_delivery_dropped_total` increments and the scheduler loop continues firing. T-V12-WH-04 verification.
- `tests/v12_webhook_scheduler_unblocked.rs` — NEW integration test (Plan 15-03+). Stalls a hypothetical receiver for 60s (here: a slow `NoopDispatcher` variant); asserts subsequent scheduled jobs across the fleet still fire on time (no scheduler drift > 1s). T-V12-WH-03 verification.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **Eager-describe + zero-baseline metric registration pattern** (`src/telemetry.rs:75-126`): five v1.0 cronduit metric families already follow this pattern with explanatory comment block. D-11's drop counter slots into the existing list with no architectural change. Comment block at L75-90 explains the metrics-exporter-prometheus 0.18 quirk that requires both `describe_*` and a zero-valued macro call to register the family in `/metrics` body. Follow it.
- **`tokio::sync::mpsc` as the scheduler-loop messaging fabric** (`src/scheduler/mod.rs`, `src/scheduler/cmd.rs`): existing `SchedulerCmd` channel for `RunNow`/`Reload`/`Reroll`/`Stop`. The webhook_tx is a separate, dedicated channel — NOT a new `SchedulerCmd` variant — because the dataflow is one-way (scheduler emits, worker consumes; no scheduler-side reply expected). Pattern is well-trodden in this codebase.
- **`finalize_run` lifecycle steps** (`src/scheduler/run.rs:300-360`): existing comment-numbered steps 6 / 7 / 7b / 7c with explicit ordering rationale (Phase 11 D-10 `__run_finished__` sentinel must broadcast BEFORE the broadcast sender is dropped). D-05 extends the sequence with a step-7d webhook emit. The existing step-numbering comment style is the convention to follow.
- **Closed-cardinality counter increment idiom** (`src/scheduler/run.rs:345-352`): `metrics::counter!("cronduit_runs_total", "job" => job.name.clone(), "status" => status_str.to_string()).increment(1)` is the established pattern. P15's drop counter is the simpler unlabeled variant (`metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(1)`) — no labels, no per-job dimension.
- **`tracing::warn!` with target + structured fields** (`src/scheduler/run.rs` and elsewhere): existing pattern for cronduit warn-level output is `tracing::warn!(target: "cronduit.run", run_id, error = %e, "...")`. D-04's drop log uses `target: "cronduit.webhooks"` and the same structured-field convention.
- **CI `lint` job + `taiki-e/install-action@v2`** (`.github/workflows/ci.yml:22-46` + `:73-75`): the latter is already used to install nextest in the test job. D-08 reuses it for cargo-deny inside the `lint` job. No new GHA action types, no version-pinning research needed.
- **`continue-on-error` step-level flag** — standard GHA primitive; nothing in the existing workflow uses it but the pattern is well-known and reversible in one line per D-09.
- **`just <recipe>` exclusivity in CI** (`ci.yml` L30-46): every `run:` step invokes `just <name>` only. D-07's new `just deny` recipe slots in transparently.
- **License allowlist baseline** — v1.0/v1.1 ran without cargo-deny but the repo's `LICENSE` and the cumulative dep tree have only ever surfaced MIT, Apache-2.0, BSD-3-Clause, ISC, and Unicode-DFS-2016. D-10's allowlist is the empirically-observed set.

### Established Patterns

- **mpsc → background task → side-effect**: existing log-pipeline (`src/scheduler/log_pipeline.rs`) and SSE log broadcast follow this exact shape — bounded channel, dedicated task, scheduler never awaits the consumer. The webhook worker is a structural copy of this idiom.
- **Module-per-feature inside `src/`**: `src/scheduler/`, `src/config/`, `src/db/`, `src/web/`, `src/cli/`. Adding `src/webhooks/` matches the convention; D-01's split into `mod.rs`/`event.rs`/`dispatcher.rs`/`worker.rs` (if planner picks it) follows what `src/scheduler/` already does.
- **Telemetry registration centralized in `src/telemetry.rs`**: every metric description + zero-baseline lives in one function. D-11 adds the drop counter there, NOT inside `src/webhooks/mod.rs` — keeps the `/metrics` HELP/TYPE table in one greppable place.
- **Integration tests in `tests/` with the `vNN_<feature>_<scenario>.rs` naming**: `tests/v11_bulk_toggle.rs`, `tests/v13_timeline_explain.rs`, `tests/dashboard_jobs_pg.rs`. P15's tests follow `v12_webhook_*` per the convention.
- **`testcontainers` integration for any DB or Docker work**, but P15's webhook worker is in-process only — no testcontainers needed; the queue saturation test uses a plain `tokio::test` + a stalled `Dispatcher` mock.
- **Just recipes are dependency-free**: `just deny` does not need to depend on `just install-targets` or any other recipe; cargo-deny self-discovers the workspace. Mirrors `just clippy`.

### Integration Points

- **`Scheduler::new` / `SchedulerLoop::new`**: where the worker task is spawned and `webhook_tx` is wired into the scheduler struct. Uses the existing `tokio::spawn` + `tokio_util::CancellationToken` shutdown pattern from v1.0/v1.1.
- **`finalize_run` step 7c → step 7d** in `src/scheduler/run.rs`: the single new emit site. No other call sites — `finalize_run` is the chokepoint for terminal-status DB writes (D-05).
- **`src/telemetry.rs` describe-block + zero-baseline pair**: where `cronduit_webhook_delivery_dropped_total` registers (D-11).
- **CI `lint` job in `ci.yml`**: where the cargo-deny step lands (D-08); cargo-deny is installed via taiki-e/install-action and invoked via `just deny`.
- **`Cargo.toml:3`**: the `version = "1.1.0"` line (Plan 15-01).
- **`justfile` recipes section**: new `deny:` recipe peer to `clippy:` (Plan 15-02).
- **Project root `deny.toml`**: NEW file (Plan 15-02).
- **No DB schema changes in P15.** Migrations are P16's concern (FCTX-04 / WH-09 enrichment columns). Webhook persistence to disk across restart is explicitly Future Requirements (v1.3+) per REQUIREMENTS.md § Future Requirements.

</code_context>

<specifics>
## Specific Ideas

- **Symmetry with Phase 10 D-12 is load-bearing for plan ordering.** The Cargo.toml bump must land as the very first commit of v1.2 — not the second, not bundled with cargo-deny, not bundled with the worker. The reason is operator-observable: any bug discovered between commit-1 and the next rc tag is reported against `cronduit --version = 1.2.0`, not `1.1.0`. The traceability cost of dual versions during a development window is real (we hit it informally during v1.1 before D-12 was codified).
- **The `WebhookDispatcher` trait is the seam P18 sees.** Planner: do NOT in P18 swap the trait for a different shape (e.g., closure type, generic parameter on the worker) just because HTTP is "more complex". The trait is the contract; HTTP fills it in.
- **The `RunFinalized` struct is the channel-message contract, not the wire payload.** The wire payload is WH-09 (Phase 18). These are two different things. Keep them separate in `src/webhooks/`: `event.rs` (channel-message struct, this phase) vs `payload.rs` (wire-format serialization, Phase 18).
- **"Non-blocking initially" is two layers, not one.** Both `bans.multiple-versions = "warn"` (config-side, D-10) AND `continue-on-error: true` (CI-side, D-09) are in effect. The redundancy is intentional: if a future planner removes one without removing the other, CI still tolerates findings. Phase 24's "promote to blocking" must remove BOTH layers.
- **Drop counter has zero labels in P15.** This is intentional. WH-11 (Phase 20) will add the full family with `{job, status}` labels, and `cronduit_webhook_deliveries_total{status="dropped"}` will become the labeled equivalent of the P15 unlabeled drop counter. The two CAN coexist on `/metrics` during the P15→P20 window without confusion: the P15 counter is the global "queue saturated" event count; the P20 counter is the per-job, per-status delivery outcome count. They are not redundant — the P15 counter fires when the queue is full (worker side), the P20 counter fires when a delivery completes (HTTP side). A planner refactor in P20 that subsumes the P15 counter into the labeled family is acceptable but NOT required.
- **The integration tests verifying scheduler-survival under queue saturation (T-V12-WH-03 + T-V12-WH-04) are non-negotiable.** They are the executable form of WH-02's load-bearing claim. Planner: the test surface is the load-bearing contract — write them.

</specifics>

<deferred>
## Deferred Ideas

- **Full `cronduit_webhook_*` metric family** (`deliveries_total{status}`, `delivery_duration_seconds{job}`, `queue_depth`) — Phase 20 / WH-11. Came up during the metric-family-scope discussion; explicitly held back to keep `/metrics` honest.
- **`queue_depth` gauge in P15** — middle-ground option from the metric discussion. Real but small win; held back so that all `cronduit_webhook_*` family additions land together in P20 with consistent labeling.
- **Stub HTTP delivery against a no-op endpoint** — pulls payload + TLS questions into P15. Held back to P18+.
- **Conditional worker spawn based on config** — adds a second code path without runtime savings worth the test burden. Always-on shape (D-03) is the foundation; if a future profiling pass shows the inert worker is non-trivial, P21+ can revisit.
- **Webhook persistence to disk across restart (durable queue)** — already in REQUIREMENTS.md § Future Requirements as a v1.3 candidate. v1.2 is best-effort delivery only.
- **`webhook_drain_grace = "30s"` graceful-shutdown drain accounting** — Phase 20 / WH-10. P15's worker has a token-cancellation shutdown but no grace-period bookkeeping yet.
- **License allowlist exact crate enumeration** — D-10 says "MIT/Apache-2.0/BSD-3-Clause/ISC/Unicode-DFS-2016 plus project-specific exceptions". Enumerating which specific deps justify which allowlisted license is a P15 plan-step concern (Plan 15-02) but not a discussion-step concern.
- **Allowlisting specific RustSec advisory IDs** — none in rc.1 per D-10. If a P15 implementation surfaces a pre-existing advisory hit (e.g., on `rustls` or `time` due to known unaffected paths), Plan 15-02 may add a deny.toml `[advisories.ignore]` entry with a comment pointing at the analysis. This is a runtime-of-plan decision, not a discussion-step decision.
- **Bulk-stop / bulk-RunNow webhook semantics** — out of v1.2 entirely; webhooks fire per-run only. Tag-based filtering is `[defaults].webhook` + per-job `webhook` config; not new surface area in v1.2.
- **`15-HUMAN-UAT.md` scope** — Claude's-discretion item. Planner picks whether the drop-counter overflow scenario warrants a maintainer runbook entry beyond the integration test.

</deferred>

---

*Phase: 15-foundation-preamble*
*Context gathered: 2026-04-25*
