# Phase 10: Stop-a-Running-Job + Hygiene Preamble - Context

**Gathered:** 2026-04-15
**Status:** Ready for planning

<domain>
## Phase Boundary

Deliver a Stop button that kills any in-flight run (command / script / docker) from the web UI and finalizes the run with a new `stopped` terminal status that is distinct from `success`, `failed`, `timeout`, and `cancelled`. The scheduler grows a per-run `RunControl` (cancellation token + stop-reason atomic) so executors can distinguish operator-stop from graceful-shutdown cancel. The hygiene preamble lands in the same phase: `Cargo.toml` version bumps from `1.0.1` to `1.1.0` on the very first v1.1 commit, and the `rand` crate is bumped from `0.8` to `0.9.x` (not `0.10`).

Out of scope for Phase 10: SIGTERM-to-SIGKILL escalation / grace periods, authentication gating on Stop, any webhook or notification plumbing, and the per-job `stop_grace_period` field. All of those are deferred to v1.2 additively.

</domain>

<decisions>
## Implementation Decisions

### Scheduler Architecture

- **D-01:** **Merge `active_runs` into a single `HashMap<i64, RunEntry>` map.** `RunEntry { broadcast_tx: tokio::sync::broadcast::Sender<LogLine>, control: RunControl }`. This supersedes the "keep `running_handles` separate" option that the research SUMMARY flagged as an open question. Planner mechanically updates every existing `active_runs` call site (`src/scheduler/mod.rs`, `src/scheduler/run.rs`, the SSE log handler in `src/web/handlers/`) so a single lock acquisition is sufficient per run, insert/remove is atomic per run boundary, and the two concerns cannot drift. The larger diff is accepted in exchange for one authoritative per-run record. The race tests T-V11-STOP-04..06 MUST cover the merged lifecycle — specifically, that `join_next()` removes the `RunEntry` atomically before the scheduler loop processes the next `SchedulerCmd::Stop { run_id }`.

- **D-09 (carried from research):** `src/scheduler/control.rs` is a new ~60 LOC module holding `RunControl { cancel: CancellationToken, stop_reason: Arc<AtomicU8> }` and a `StopReason` enum with at minimum `Operator` and `Shutdown` variants. Executors (`command.rs`, `script.rs`, `docker.rs`) read the atomic after `cancel.cancelled()` fires and return `RunStatus::Stopped` or `RunStatus::Shutdown` accordingly. No other `StopReason` variants are introduced in v1.1 — keep the atomic minimal.

### Status Surface

- **D-02:** **Add a new `--cd-status-stopped` design-system token** (neutral slate/gray family; specific hex to be picked by planner to harmonize with the terminal-green brand) along with a matching `--cd-status-stopped-bg` pair and a `.cd-badge--stopped` CSS class in `assets/static/app.css`. Update the Status Colors table in `design/DESIGN_SYSTEM.md` in the same commit so the design system stays the source of truth. Rationale: `stopped` is operator-interrupt, not a failure — it must be excluded from the failure denominator (Phase 13 sparkline + success-rate badge) and visually distinct from both `cd-status-error` (failure) and `cd-status-disabled` (config-level pause, which Phase 14 uses for bulk disable). Matches GitHub Actions' treatment of `cancelled` runs visually.

- **D-10 (carried from roadmap):** Add `RunStatus::Stopped` to `src/scheduler/command.rs` enum. `finalize_run` in `src/scheduler/run.rs` maps it to the `"stopped"` database string. The `classify_failure_reason` helper in `run.rs` does NOT classify `stopped` as a failure reason — `stopped` runs never increment `cronduit_run_failures_total`. The `cronduit_runs_total{status}` counter gains the new `"stopped"` label value.

### UI Placement

- **D-03:** **Stop button on the run-detail page sits in the right-side page-action slot** of the `Run #N` header row (currently empty in `templates/pages/run_detail.html:15-18`). Reads as a deliberate page-level command and is visually separated from the metadata card's status badge. Appears only when the run is in `status = 'running'`, gated by the same `is_running` template variable the live log section uses.

- **D-04:** **In the run-history partial (`templates/partials/run_history.html`), Stop renders as a compact text button** labeled "Stop" (accessible by default, no aria-label gymnastics required). Appears only on rows where `status = 'running'`. Keep it as small as possible column-wise so it does not force other columns to reflow.

- **D-05:** **Button visual weight: neutral outline**. Inherit text color; hover tints toward the new `--cd-status-stopped` token. Do NOT pre-color with `cd-status-error` — consistent with the "stopped is not a failure" semantic in D-02. Matches the non-alarming Stop affordance used by peer tools (GitHub Actions, Buildkite, Jenkins).

### Stop Feedback UX

- **D-06:** **Success toast text:** `"Stopped: <job name>"`. Symmetric with the existing Run Now handler toast (`"Run queued: <job name>"`, `src/web/handlers/api.rs:65`) so operators can build muscle memory across the two commands. Use `HxEvent::new_with_data("showToast", ...)` with `"level": "info"` to match Run Now exactly.

- **D-07:** **Race case** (Stop arrives after the run finalized naturally): the handler is a no-op and returns `HX-Refresh: true` with NO toast. The refreshed page shows the real natural terminal status (`success` / `failed` / `timeout` / whatever), and that IS the message. Rationale: showing an info/warning toast for a condition that isn't an error creates noise; showing nothing lets reality speak. This aligns with "silence is success" UI principles and avoids educating operators on an internal race they don't need to reason about.

- **D-08:** **No optimistic badge swap.** Click → `POST /api/runs/{run_id}/stop` → server returns `HX-Refresh: true` → page reloads → badge renders from DB. Same pattern as Run Now. The race case makes client-side optimism fragile (the badge could briefly flash `stopped` only to be overwritten by the natural status), and the cost of a tiny delay is worth the consistency and simplicity.

### API Surface (carried from roadmap / SCHED-14)

- **D-11:** `POST /api/runs/{run_id}/stop`, CSRF-gated using the same `csrf::validate_csrf` pattern as Run Now (`src/web/handlers/api.rs:32-40`). Request form matches `CsrfForm`. Sends a new `SchedulerCmd::Stop { run_id: i64 }` variant through the existing scheduler mpsc channel. No confirmation dialog. Handler contract:
  - If the scheduler channel send succeeds AND the run was still running → toast `"Stopped: <job name>"` + `HX-Refresh: true`.
  - If the scheduler channel send succeeds BUT the run had already finalized (race case) → `HX-Refresh: true`, no toast. Planner decides whether the handler can distinguish this locally (e.g. by checking the DB before send) or whether the scheduler replies via oneshot.
  - If the scheduler channel is closed (shutting down) → `503 Service Unavailable`, same as Run Now.

### Hygiene Preamble (carried from FOUND-12, FOUND-13)

- **D-12:** **Cargo.toml version bumps from `1.0.1` to `1.1.0` as the very first commit of Phase 10.** This guarantees `cronduit --version` reports `1.1.0` from the start of the v1.1 development window — no drift between the in-flight milestone version and the binary. Planner sequences this as plan 10-01 (or similar) before any Stop work lands.

- **D-13:** **`rand` bump from `0.8` to `0.9.x`** (NOT `0.10`, to avoid the `gen → random` trait rename churn). Call sites are the `@random` slot picker and CSRF token generation. Migration is mechanical. Land in a separate plan from the `Cargo.toml` version bump if it simplifies review, but both must be committed before the Stop spike begins so the hygiene baseline is clean.

### Testing (carried from research)

- **D-14:** **Stop spike is the first Stop-related plan.** Validate `RunControl` + `StopReason::Operator` round-trip on all three executors (command / script / docker) as a short spike before committing the full implementation, the API handler, or the UI. This is the highest-risk feature in v1.1 and the spike de-risks the executor wiring before bigger template / CSS work lands.

- **D-15:** **Race test T-V11-STOP-04 uses `tokio::time::pause` and runs 1000 iterations.** Non-negotiable per research; the Stop feature does NOT ship until this test is in place. Covers the Stop-vs-natural-completion race for the merged `RunEntry` lifecycle decided in D-01.

- **D-16:** **Orphan reconciliation test lock** (T-V11-STOP-12..14): `mark_run_orphaned` in `src/scheduler/docker_orphan.rs` already has the `WHERE status = 'running'` guard on both SQLite (L120) and Postgres (L131) branches. Add three tests that fail if either guard is removed. No design work; pure regression lock.

- **D-17:** **Preserve the `.process_group(0)` + `libc::kill(-pid, SIGKILL)` pattern** in `command.rs:203` and `script.rs:89`. Do NOT adopt `kill_on_drop(true)` — it would orphan shell-pipeline grandchildren (Research Correction #1). Add tests T-V11-STOP-07 and T-V11-STOP-08 that lock this in.

### Claude's Discretion

- Specific hex values for `--cd-status-stopped` and `--cd-status-stopped-bg` (planner picks in a neutral slate/gray family that harmonizes with the terminal-green brand and keeps contrast ratios compliant with the existing badge pattern).
- Whether plans 10-01 (Cargo.toml bump) and 10-02 (`rand` bump) are a single plan or two (commit granularity).
- Where exactly to plug the `SchedulerCmd::Stop` match arm into the `tokio::select!` loop in `src/scheduler/mod.rs`, and the internal helper shape for "look up the RunEntry, set stop_reason, call cancel" as an atomic operation.
- Whether the stop-reason check in the race-case branch happens in the web handler (DB read) or in the scheduler (oneshot reply) — pick whichever is simpler to test deterministically.
- Icon choice for the Stop button (if any — a square/stop glyph is conventional; pure-text is also acceptable).
- Keyboard shortcut affordance for Stop on the run-detail page — left to planner discretion; no explicit requirement either way.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level

- `.planning/PROJECT.md` — v1.1 milestone intent, locked constraints (Rust, bollard, sqlx, Tailwind, HTMX), threat model posture. § Key Decisions has the rationale for every locked tech-stack choice.
- `.planning/REQUIREMENTS.md` §Scheduler — SCHED-09..14 (stop a running job requirements, including the T-V11-STOP-NN test IDs) and FOUND-12..13 (rand + Cargo.toml hygiene).
- `.planning/ROADMAP.md` § "Phase 10: Stop-a-Running-Job + Hygiene Preamble" — goal, depends-on, success criteria, risk notes.
- `.planning/STATE.md` § Accumulated Context → Decisions — v1.1 scoping decisions (Shape A, rc cadence, phase numbering).

### Research

- `.planning/research/SUMMARY.md` — overall v1.1 architecture + Research-Phase Corrections (sections 1–4, especially #1 `kill_on_drop` stale claim and #4 `mark_run_orphaned` guard). § Open Questions #2 is the merge-vs-separate map question that D-01 resolves.
- `.planning/research/ARCHITECTURE.md` — scheduler loop structure and executor integration points. Note §3.1 "new `running_handles` map" is superseded by D-01's merged `RunEntry`.
- `.planning/research/FEATURES.md` § Stop a running job — peer comparison, design rationale, execution model. NOTE: the acceptance criterion in this doc that mentions `kill_on_drop(true)` is explicitly stale per Correction #1 in SUMMARY.md.
- `.planning/research/PITFALLS.md` — race failure modes, stop_reason atomic rationale, corrections to ARCHITECTURE.md.
- `.planning/research/STACK.md` — dependency version matrix (including the `rand` family).

### Source files the phase touches

- `src/scheduler/mod.rs` — scheduler struct (L53–56), `active_runs` type, `tokio::select!` loop where `SchedulerCmd::Stop` match arm lands.
- `src/scheduler/cmd.rs` — `SchedulerCmd` enum; add the `Stop { run_id: i64 }` variant here.
- `src/scheduler/control.rs` — NEW file, ~60 LOC, holds `RunControl` + `StopReason`.
- `src/scheduler/run.rs` — `finalize_run` status-to-string map (L238–244), broadcast-sender removal (L276), `classify_failure_reason` (L298). Both need `Stopped` awareness.
- `src/scheduler/command.rs` — `RunStatus` enum (L14–27), add `Stopped` variant; `.process_group(0)` spawn pattern preserved (Research Correction #1).
- `src/scheduler/script.rs` — same as command.rs.
- `src/scheduler/docker.rs` — docker executor; cancel branch must return `RunStatus::Stopped` when `stop_reason == Operator`, and the bollard `docker kill -s KILL` path must run before finalize.
- `src/scheduler/docker_orphan.rs` — L112, L120, L131 — test lock for the `status = 'running'` guard (D-16).
- `src/web/handlers/api.rs` — new `stop_run` handler modeled on `run_now` (L26–80).
- `src/web/mod.rs` — new route at L79 neighborhood: `POST /api/runs/{id}/stop`.
- `src/web/csrf.rs` — existing validator reused.
- `templates/pages/run_detail.html` L15–18 (header action slot), L26 (status badge context), L65 (`is_running` conditional already exists).
- `templates/partials/run_history.html` — per-row Stop cell for running rows.
- `assets/static/app.css` — add `.cd-badge--stopped` + referenced CSS vars.
- `design/DESIGN_SYSTEM.md` Status Colors table (§L54–66 in the current file) — add the `stopped` row.
- `Cargo.toml` version field + `rand` dep (D-12, D-13).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`SchedulerCmd` enum** (`src/scheduler/cmd.rs`): existing `RunNow`, `Reload`, `Reroll` variants. Adding `Stop { run_id }` is a one-line structural extension; the mpsc channel, axum state, and handler idioms all already exist.
- **`run_now` handler** (`src/web/handlers/api.rs:26-80`): canonical CSRF-gated scheduler-command handler with HX-Trigger toast + HX-Refresh. New `stop_run` handler should be a near-clone structurally.
- **`csrf::validate_csrf`** (`src/web/csrf.rs`) + `CsrfForm` struct: reused as-is.
- **`RunStatus` enum** (`src/scheduler/command.rs:16`): already holds `Success`, `Failed`, `Timeout`, `Shutdown`, `Error`. Adding `Stopped` is additive. The `finalize_run` string mapping in `run.rs:238` is the single place to thread it through.
- **`cd-badge` CSS class family** + existing 4 status tokens in `design/DESIGN_SYSTEM.md`: pattern is already established; adding a fifth token + badge modifier is a one-line-per-file addition in three files (CSS, design doc, any template that renders badges).
- **`is_running` template variable** in `run_detail.html`: already gates the live SSE log pane. Reuse it to gate the Stop button.
- **`HxResponseTrigger::normal([event])`** + `HxEvent::new_with_data("showToast", json!(...))`: existing toast pattern — copy verbatim.
- **`active_runs` map** (`src/scheduler/mod.rs:56`): already present; D-01 extends it to `RunEntry` rather than creating a parallel structure.
- **`docker_orphan.rs::mark_run_orphaned` guard** (L112, L120, L131): already correct; just needs a test lock (no design work).

### Established Patterns

- **mpsc channel → scheduler commands**: web handlers do NOT manipulate scheduler state directly. They send a `SchedulerCmd` variant and rely on the scheduler loop to act. Stop follows this pattern.
- **CSRF + `HX-Refresh` + toast**: all state-changing web actions follow this triad. Stop inherits all three.
- **Finalize-once database writes**: executors own the finalize. The web/API layer never writes `job_runs.status` directly. Stop respects this — the scheduler cancels the executor and the executor finalizes with `RunStatus::Stopped`.
- **`.process_group(0)` + `libc::kill(-pid, SIGKILL)`**: the v1.0 process-group kill pattern — preserved, not replaced (Research Correction #1).
- **`WHERE status = 'running'` guards on lifecycle writes** (e.g. `docker_orphan.rs`): pattern that prevents finalized rows from being clobbered. Same pattern the race tests T-V11-STOP-04..06 will exercise.
- **HTMX partial re-render idiom** — `run_history.html` partial already targeted by polling from the job-detail page. A Stop that succeeds returns `HX-Refresh: true` which re-renders the whole page (same as Run Now); per-row HTMX swap is not currently used for state-changing actions anywhere in the app.

### Integration Points

- **Scheduler mpsc channel**: `state.cmd_tx.send(SchedulerCmd::Stop { run_id }).await` — web handler entry point.
- **`tokio::select!` loop arms in `src/scheduler/mod.rs`**: a new arm handles `SchedulerCmd::Stop { run_id }` by looking up the `RunEntry` and calling `entry.control.stop(StopReason::Operator)` (or equivalent).
- **Executor cancel branch**: each of `command.rs`, `script.rs`, `docker.rs` already has a `cancel.cancelled()` branch that returns `RunStatus::Shutdown`. That branch gains a `match stop_reason.load()` read to decide between `Shutdown` and `Stopped`.
- **SSE log viewer**: continues to read `broadcast_tx` from the `RunEntry`; D-01's merge keeps it working with a `.control` sibling field.
- **Orphan reconciliation on startup**: unchanged behavior; the test lock (D-16) ensures the `stopped` status cannot be clobbered by a future refactor.

</code_context>

<specifics>
## Specific Ideas

- **Symmetry with Run Now is load-bearing** — D-06 and D-08 are both driven by the operator experience of "these two buttons should feel the same." Planner should avoid inventing a new toast idiom, a new response-header convention, or a new visual-weight pattern for Stop specifically. If the Stop handler needs to behave differently from Run Now structurally, that's a flag worth re-raising.
- **"Silent refresh on race" (D-07) is explicitly a design principle, not an oversight** — the planner/reviewer should not later add an info toast for the race case "for completeness." The user decided the refreshed state speaks for itself.
- **The neutral outline + hover-tint pattern (D-05)** is the affordance language. Do not escalate to red on hover even if a design iteration suggests it.
- **Spike-first (D-14)** means the Stop spike plan should not also try to land the UI, the CSS token, or the race test in the same commit sequence. Spike = executor-side proof of life on all three executors, that's it. UI and tests follow in subsequent plans.

</specifics>

<deferred>
## Deferred Ideas

- **Graceful SIGTERM-to-SIGKILL escalation + per-job `stop_grace_period`**: v1.2 additive feature. Mentioned in research; explicitly excluded from v1.1.
- **Authentication gating on Stop**: the v1 trusted-LAN posture in `THREAT_MODEL.md` covers it. Revisit in v2 when auth lands.
- **Webhook / chain notification on stop**: v1.2 webhooks will include `stopped` as a transition. Out of scope here.
- **Keyboard shortcut for Stop on run-detail page**: not requested by the user; planner has discretion but no requirement.
- **Per-row optimistic UI with rollback** (rejected in D-08): if a future phase genuinely needs snappier feedback, it would be a cross-app HTMX pattern change, not a Phase 10 scope creep.
- **Stop-all / bulk-stop from the dashboard**: not requested; would be additive to the Phase 14 bulk-disable ergonomics. Operators who want to kill a running job can use the per-run Stop button.

</deferred>

---

*Phase: 10-stop-a-running-job-hygiene-preamble*
*Context gathered: 2026-04-15*
