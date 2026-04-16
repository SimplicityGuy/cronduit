# Phase 11: Per-Job Run Numbers + Log UX Fixes - Context

**Gathered:** 2026-04-16
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 11 delivers two operator-visible fixes for cronduit's run/log surfaces:

1. **Per-job run numbering.** Every `job_runs` row carries a `job_run_number` stable from insert time. Run-history shows `#1, #2, #3, ...` per job instead of the global `job_runs.id`. Existing deployments are migrated in place via a three-file, idempotent, chunked backfill. Global `job_runs.id` remains the URL key so permalinks and Prometheus alert annotations keep working (DB-09..13, UI-16).
2. **Run-detail log UX.** Navigating back to a running job's detail page renders the accumulated log lines from the DB, then attaches the live SSE stream with zero gaps and zero duplicates across the live→static transition. The transient "error getting logs" flash on immediate click-through after "Run Now" is eliminated (UI-17..20).

Phase closes at the end of Phase 11 proper; the `v1.1.0-rc.1` tag cut happens in Phase 12 alongside the healthcheck work.

**Out of scope (deferred to other phases):**
- Rekeying permalinks by `job_run_number` (DB-13 explicitly keeps global id as the URL key)
- HTMX 4.x upgrade (defer — `sse-swap` breaking change)
- Bulk enable/disable (Phase 14)

</domain>

<decisions>
## Implementation Decisions

### Log Dedupe Mechanism (the explicit ROADMAP decision gate)

- **D-01:** **Option A chosen: insert-then-broadcast with `RETURNING id`.** Each per-line `INSERT INTO job_logs (run_id, stream, ts, line)` in `src/db/queries.rs:insert_log_batch` gains `RETURNING id`. Ids are collected into a `Vec<i64>` while still inside the existing batch transaction; after `tx.commit()` they are zipped with the lines and broadcast via `broadcast_tx.send(LogLine { id: Some(i), ... })`. **No new schema column.** `LogLine` gains the `id: Option<i64>` field that UI-20 already names.
- **D-02:** **T-V11-LOG-02 benchmark is the first plan of Phase 11 — a gated spike.** It asserts p95 insert latency < 50ms for a 64-line batch against in-memory SQLite on the CI runner. If the benchmark fails, the phase flips to Option B (monotonic `seq: u64` column in `LogLine` + nullable `seq` column on `job_logs`) before any other Phase 11 work lands. Same de-risking pattern Phase 10 used for the Stop spike (Phase 10 D-14).
- **D-03:** **Per-line `RETURNING id` inside the existing batch tx** — do NOT use multi-row `INSERT ... VALUES ... RETURNING id`, and do NOT drop the batch transaction for one-at-a-time inserts. Keeps the batching throughput property (one fsync per batch, not per line) that the current `DEFAULT_BATCH_SIZE = 64` design assumes.

### Per-Job `#N` Display

- **D-04:** **Run-history partial renders `#42` bare, with `title="global id: {job_runs.id}"` on the row.** File: `templates/partials/run_history.html`. Keeps the narrow column uncluttered and avoids fighting the "Stop button must stay compact" compactness promise from Phase 10 D-04. Hover/keyboard-focus exposes the global id for operators who need it.
- **D-05:** **Run-detail page header renders `Run #42` primary + `(id 1234)` muted suffix** in the left side of the header row at `templates/pages/run_detail.html:15-18`. The diagnostic surface shows both values inline because this is where operators copy-paste into issue reports. The Stop button continues to occupy the right-side action slot (Phase 10 D-03) — primary title length remains short enough to fit.
- **D-06:** **No special treatment for in-flight `running` rows in history.** `#N` is stable from insert time (DB-11 locks `jobs.next_run_number` counter semantics). The `running` badge from Phase 10 D-02 is the single source of truth for run state — do not duplicate "running" into the run-number cell.
- **D-07:** **Backfill numbering uses `ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id ASC)`.** Stable, deterministic, monotonic by construction. After backfill: `UPDATE jobs SET next_run_number = COALESCE((SELECT MAX(job_run_number) FROM job_runs WHERE job_runs.job_id = jobs.id), 0) + 1`. Post-backfill inserts continue the sequence without gaps and without duplicates.

### Page-Load → Live-SSE Attachment on Run-Detail

- **D-08:** **Server-side render the initial backfill inline in the page template.** The `GET /jobs/{job_id}/runs/{run_id}` handler (`src/web/handlers/run_detail.rs`) reads the last N lines from `job_logs` ordered by `id ASC` and passes them to the template. `templates/partials/static_log_viewer.html` (or its replacement) emits the lines **and** sets `data-max-id="{last_id}"` on `#log-lines`. One HTTP response — no skeleton flash, no second round-trip.
- **D-09:** **Client-side dedupe: `data-max-id` + SSE event.id comparison.** HTMX SSE swap handler is augmented (via a small inline script or `hx-ext`) to parse `event.lastEventId` (or `event.data.id`), drop the frame if `id <= parseInt(container.dataset.maxId)`, and otherwise update `dataset.maxId` to the new max after append. No Set-based dedupe — the single broadcast channel already guarantees ordering; belt-and-suspenders isn't warranted.
- **D-10:** **Terminal `run_finished` SSE event.** When `finalize_run` runs and the broadcast channel is about to close, the SSE handler (`src/web/handlers/sse.rs`) emits a final `event: run_finished\ndata: {"run_id": N}\n\n` frame before closing the stream. Client listens for `run_finished`, calls `htmx.trigger('#log-lines', 'refresh')` which fires a `hx-get` on the final static partial. Any SSE frames that were still buffered client-side get dropped by the D-09 dedupe because their `id <= new max`. Scroll position is preserved because only `#log-lines` swaps, not the whole page.
- **D-11:** **Do NOT reuse `HX-Refresh: true` for the live→static transition.** `HX-Refresh` reloads the whole page and loses scroll/selection state. Keep `HX-Refresh` for the Stop button and Run Now handlers (Phase 10 D-08 compatibility).

### Backfill Startup Ergonomics

- **D-12:** **HTTP listener binds AFTER the migration backfill completes.** Startup is a strict two-phase: migrate → spawn scheduler + bind listener. No half-state, no "503 during backfill" handler. Docker's healthcheck sees the container as "starting" because the TCP port isn't open — this is exactly what Phase 12's `HEALTHCHECK --start-period=60s` accommodates.
- **D-13:** **Backfill progress surfaced via INFO log lines only.** One line per 10k-row batch. Shape: `INFO cronduit.migrate: job_run_number backfill: batch={i}/{N} rows={done}/{total} pct={p:.1}% elapsed_ms={ms}`. No progress metric, no progress file — the log line is sufficient for `docker logs -f` and any log-scrape alerting an operator already has set up.
- **D-14:** **Fail-fast on backfill error.** The backfill migration crashes the process on any error, naming the failing batch and the rows-done count. The three-file migration shape (DB-10) guarantees the crash is recoverable: the nullable column is already present, partial rows are filled, the rest are NULL; the backfill re-runs idempotently on restart with `WHERE job_run_number IS NULL`. No in-process retry loop — restart policies belong to the container orchestrator.
- **D-15:** **Assertion before scheduler spawn.** In `main.rs` (or wherever the scheduler is spawned), add a post-migration assertion: `SELECT COUNT(*) FROM job_runs WHERE job_run_number IS NULL` must equal 0. Panic with a clear message if not. In production this can never fire (D-12 + D-14 enforce it); in tests it locks the sequence against future regressions. Covered by T-V11-RUNNUM-01/02/03.

### Claude's Discretion

- The exact muted style tokens for the `(id 1234)` suffix on run-detail (D-05). Use an existing design-system muted-text token — do not add new CSS variables.
- Whether the `run_finished` SSE event is dispatched via the existing broadcast channel or a dedicated oneshot — pick whichever keeps `sse.rs` simplest.
- Whether the client-side dedupe script is inlined in `run_detail.html` or lives in `assets/static/app.js`. Inline is simpler for a ~10 LOC handler; extract only if it grows.
- Exact `N` for the initial backfill line count on page-load (suggested: 500 or match the existing `log_viewer.html` first-page size; reuse the "Load older lines" button — `templates/partials/log_viewer.html:1-8` — so operators can scroll further back).
- Per-line vs batch-commit granularity inside `insert_log_batch` — planner preserves the batch tx (D-03) and decides whether `RETURNING id` is collected in one `Vec<i64>` or streamed via `fetch_all`.

### Folded Todos

None — no pending todos matched Phase 11 (cross-reference returned 0 matches).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (researcher, planner, executor) MUST read these before planning or implementing.**

### Phase 11 scope and requirements
- `.planning/ROADMAP.md` § "Phase 11: Per-Job Run Numbers + Log UX Fixes" — phase goal, success criteria, decision gate, locked design decisions
- `.planning/REQUIREMENTS.md` § DB-09..DB-13 — per-job numbering migration contract
- `.planning/REQUIREMENTS.md` § UI-16..UI-20 — display and log-UX acceptance criteria
- `.planning/REQUIREMENTS.md` § Traceability — T-V11-RUNNUM-01..13, T-V11-LOG-01..09, T-V11-BACK-01..02 test ids

### Carried decisions from earlier phases
- `.planning/phases/10-stop-a-running-job-hygiene-preamble/10-CONTEXT.md` § D-01, D-02, D-10, D-11 — `RunEntry` / `broadcast_tx` merge, `stopped` status semantics, `HX-Refresh + toast` response pattern

### Project-level constraints
- `/Users/Robert/Code/public/cronduit/CLAUDE.md` § "Constraints" — Tech stack lock (sqlx, askama_web 0.15, bollard, croner), rustls, TOML config, terminal-green design system, mermaid-only diagrams, PR-only landing
- `design/DESIGN_SYSTEM.md` — existing muted-text tokens, `cd-status-*` families (stopped, error, disabled) from Phase 10 D-02
- `THREAT_MODEL.md` — security posture; Phase 11 adds no new attack surface but the run-detail page is already auth-unprotected in v1

### Code integration points
- `src/scheduler/log_pipeline.rs:22` — `LogLine` struct; Phase 11 adds `id: Option<i64>`
- `src/scheduler/run.rs:100,359` — broadcast channel setup + `broadcast_tx.send(line.clone())` call-site
- `src/db/queries.rs:370-410` — `insert_log_batch` — change per-line INSERT to `RETURNING id`
- `src/db/queries.rs:286-310` — `insert_running_run` — already uses `RETURNING id`; confirms pattern compiles on both backends
- `src/web/handlers/sse.rs:38,93` — SSE subscribe + `format_log_line_html`; Phase 11 adds `run_finished` terminal event
- `src/web/handlers/run_detail.rs` — GET handler that must server-render the initial backfill (D-08)
- `src/web/handlers/api.rs:20-82` — `run_now` handler; Phase 11 must synchronously insert `job_runs` row here (UI-19 locked fix)
- `templates/pages/run_detail.html:15-18, 83-84` — header row (Run #N layout) and `sse-connect` wiring
- `templates/partials/run_history.html` — per-row `#N` display
- `templates/partials/log_viewer.html, static_log_viewer.html` — backfill render, `data-max-id` site
- `migrations/sqlite/, migrations/postgres/` — add three new migration files per backend for DB-09/10

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`insert_running_run`** (`src/db/queries.rs:286`) already uses `INSERT ... RETURNING id` on both SQLite (`?1..?3`) and Postgres (`$1..$3`) branches — D-01 adopts the exact same pattern for log inserts.
- **`RunEntry` + `broadcast_tx`** (merged in Phase 10 D-01) — SSE handler already subscribes via `state.active_runs.lock().entry.broadcast_tx.subscribe()`. Phase 11 does not change the subscribe site; only augments the `LogLine` payload.
- **HTMX SSE wiring** — `templates/pages/run_detail.html:83-84` already has `hx-ext="sse" sse-connect=... sse-swap="log_line"`. Extending to recognize a `run_finished` event is one extra listener, not a rewrite.
- **`HX-Refresh + toast` response pattern** (`src/web/handlers/api.rs:60-78`) — used by Run Now and Stop. Phase 11 does NOT reuse `HX-Refresh` for the live→static transition (D-11) — uses `hx-get` + `sse-close` instead to preserve scroll.
- **Schema parity test** (mentioned in `migrations/sqlite/20260410_000000_initial.up.sql` header: "tests/schema_parity.rs (Plan 05) MUST remain green") — Phase 11 migrations must keep this test passing on both backends.
- **Head-drop log channel** (`src/scheduler/log_pipeline.rs`) — drops oldest line on backpressure. Compatible with Option A because the drop happens before persistence; dropped lines never reach the broadcast and therefore never need an id.

### Established Patterns
- **Separate read/write pools** (project CLAUDE.md) — Phase 11 backfill writes use the writer pool only; page-load reads use the reader pool. Standard split.
- **Badge + status tokens** in `assets/static/app.css` — `cd-status-running`, `cd-status-stopped` (Phase 10), `cd-status-error`, `cd-status-disabled`. Phase 11 adds no new status tokens.
- **Template partial composition** — page template includes static_log_viewer which includes log_viewer. D-08 keeps this composition and server-renders into the innermost partial.

### Integration Points
- **Run-history partial** (`templates/partials/run_history.html`) — single call-site for `#N` rendering (D-04).
- **Run-detail page header** (`templates/pages/run_detail.html:15-18`) — Phase 10 already placed the Stop button on the right; D-05 lands `Run #N (id X)` on the left of the same row.
- **Scheduler `finalize_run`** (`src/scheduler/run.rs:finalize_run`) — site where the `run_finished` SSE event is triggered (D-10).
- **`main.rs` startup path** — sequence is migrate → assert → spawn scheduler → bind listener (D-12, D-15). Backfill is blocking.

</code_context>

<specifics>
## Specific Ideas

- **ROADMAP wording to follow literally:** "Log dedupe is id-based, client-side: `data-max-id` on the static partial, SSE listener drops events with `id <= max_backfill_id`." — this is a direct lift into D-09.
- **ROADMAP wording to follow literally:** "The transient 'error getting logs' race is fixed by inserting the `job_runs` row on the API handler thread before returning the response, not asynchronously in the scheduler loop." — drives `run_now` handler shape.
- **Symmetry with Phase 10:** the Stop-race handler already uses "silence is success" (Phase 10 D-07). D-11 preserves this: the live→static transition has no toast.

</specifics>

<deferred>
## Deferred Ideas

- **Rekeying URLs by `job_run_number`** — REQUIREMENTS.md explicitly lists this as deferred; DB-13 locks global id as the URL key.
- **HTMX 4.x upgrade** — REQUIREMENTS.md explicitly lists this as deferred (breaks `sse-swap`).
- **`/healthz` with a 'starting' state during backfill** — not needed for v1.1 (D-12 blocks the listener). Revisit if future migrations run long enough to frustrate operators.
- **Auto-retry backfill within the process** — explicitly rejected (D-14); retry policy belongs to docker/compose.
- **Belt-and-suspenders Set-based dedupe** — rejected (D-09); single broadcast channel already guarantees ordering.
- **Progress file at `/tmp/cronduit-migrate-progress.json`** — rejected as over-engineered for v1.1.
- **One-shot backfill metric counter** — only useful if the listener binds during backfill; since D-12 blocks the listener, this tells operators nothing the logs don't already say.

</deferred>

---

*Phase: 11-per-job-run-numbers-log-ux-fixes*
*Context gathered: 2026-04-16*
