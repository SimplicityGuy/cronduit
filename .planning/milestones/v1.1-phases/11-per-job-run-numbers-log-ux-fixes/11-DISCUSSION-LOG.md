# Phase 11: Per-Job Run Numbers + Log UX Fixes - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-16
**Phase:** 11-per-job-run-numbers-log-ux-fixes
**Areas discussed:** Log Dedupe Mechanism, Per-Job #N Display, Page-load / SSE Ordering, Backfill Startup Ergonomics

---

## Log Dedupe Mechanism

### Q1: Which dedupe mechanism for the log pipeline?

| Option | Description | Selected |
|--------|-------------|----------|
| Option A: insert-then-broadcast w/ RETURNING id (Recommended) | Use existing job_logs.id as dedupe key; INSERT … RETURNING id per line (or batch-RETURNING) then broadcast. No schema change. Gated by p95 < 50ms benchmark. | ✓ |
| Option B: monotonic seq: u64 column | AtomicU64 counter in log_pipeline.rs; add nullable seq column to job_logs. Client dedupes on seq. Adds a migration file but latency-free. | |

**User's choice:** Option A (Recommended)
**Notes:** Roadmap's recommended path. Benchmark gates the decision — if it fails, flip to Option B before any other Phase 11 work lands.

### Q2: How should T-V11-LOG-02 (p95 < 50ms / 64-line SQLite batch) be enforced?

| Option | Description | Selected |
|--------|-------------|----------|
| First plan in the phase, spike — ship only if benchmark passes (Recommended) | Sequence as plan 11-01. Mirrors Phase 10's Stop-spike pattern. | ✓ |
| Run in CI on every plan, fail job on regression | Permanent CI gate. More robust long-term; doesn't de-risk upfront. | |
| Run once locally, record result in PLAN.md, no CI gate | Cheapest; accepts regression risk. | |

**User's choice:** First plan in the phase, spike
**Notes:** Explicit de-risking — same discipline as Phase 10 D-14.

### Q3: If Option A is picked, which INSERT-to-broadcast path?

| Option | Description | Selected |
|--------|-------------|----------|
| Per-line INSERT … RETURNING id inside existing batch tx (Recommended) | Keep batch tx; change each INSERT to RETURNING id and collect into Vec<i64>. One fsync per batch. | ✓ |
| Multi-row INSERT … VALUES … RETURNING id | Single SQL per batch; faster on Postgres, less idiomatic on SQLite sqlx query!. | |
| One-at-a-time outside any tx, broadcast immediately | Simplest code, highest write cost. Would definitely blow 50ms budget. | |

**User's choice:** Per-line INSERT … RETURNING id inside existing batch tx
**Notes:** Preserves existing batching throughput; minimal diff from current code.

---

## Per-Job #N Display

### Q1: How should #N render in the run-history partial (per-row)?

| Option | Description | Selected |
|--------|-------------|----------|
| `#42` — bare, global id in hover tooltip (Recommended) | `title="global id: 1234"` attribute. Minimal noise; URL still carries id for copy. | ✓ |
| `#42 (id:1234)` muted inline | Always visible two-piece label. Clutters narrow columns. | |
| `#42` main column + global id as separate column | Explicit columnar split; adds horizontal cost. | |

**User's choice:** `#42` bare + hover tooltip
**Notes:** Keeps the history row tight; matches "silence is success" discipline from Phase 10.

### Q2: How should #N render on the run-detail page header?

| Option | Description | Selected |
|--------|-------------|----------|
| `Run #42` + `(id 1234)` muted suffix (Recommended) | Diagnostic surface; both values visible for issue-report copy-paste. Fits alongside Stop button. | ✓ |
| `Run #42` only, global id via breadcrumb/URL only | Cleanest title; forces URL hunting. | |
| Same as row (bare #42 + hover) | Consistency with history row; hover less discoverable on full-width page. | |

**User's choice:** `Run #42` + muted `(id 1234)` suffix
**Notes:** This is the page where global id matters most — show both.

### Q3: Special treatment for running rows?

| Option | Description | Selected |
|--------|-------------|----------|
| No special treatment — `#N` stable from insert (Recommended) | DB-11 locks jobs.next_run_number at insert; running badge already carries state. | ✓ |
| `#42 (running)` prefix or styling | Redundant with status badge. | |

**User's choice:** No special treatment
**Notes:** Single source of truth for run state is the status badge.

### Q4: Backfill numbering ORDER?

| Option | Description | Selected |
|--------|-------------|----------|
| ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id ASC) (Recommended) | Stable, deterministic, monotonic by construction. | ✓ |
| ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY start_time ASC) | More "human" but depends on start_time trustworthiness. | |

**User's choice:** ORDER BY id ASC
**Notes:** id is monotonic; start_time is not guaranteed to be.

---

## Page-Load / SSE Ordering

### Q1: Where should the initial log backfill on page-load happen?

| Option | Description | Selected |
|--------|-------------|----------|
| Server-side render in page template with data-max-id (Recommended) | One HTTP response; no flash, no extra round-trip. data-max-id set inline. | ✓ |
| Render empty, HTMX hx-trigger='load' GET static partial, then sse-connect | Two round-trips; potential FOUC. | |
| Connect SSE first; server holds frames, flushes backfill as synthetic events | SSE-only; slower first paint; complex cutover reasoning. | |

**User's choice:** Server-side render with data-max-id
**Notes:** Standard HTMX pattern; cheapest and most predictable.

### Q2: Client-side dedupe rule?

| Option | Description | Selected |
|--------|-------------|----------|
| data-max-id on #log-lines; drop if event.id <= maxId (Recommended) | Matches ROADMAP locked decision. Update maxId after each accept. | ✓ |
| Same + Set<id> of recent N ids | Belt-and-suspenders; extra memory; unnecessary given broadcast channel ordering. | |

**User's choice:** data-max-id + event.id comparison
**Notes:** Roadmap already locks this; no extra Set needed.

### Q3: How does page handle live→static transition on finish?

| Option | Description | Selected |
|--------|-------------|----------|
| Terminal 'run_finished' SSE event; client tears down SSE, hx-get final static partial (Recommended) | Explicit terminal; clean dedupe; preserves scroll. | ✓ |
| No terminal event; rely on SSE channel close + HX-Refresh from stop/finalize | HX-Refresh too heavy; loses scroll. | |
| Server appends terminal marker as pseudo log_line | Overloads event type; clients without recognition show garbage. | |

**User's choice:** Terminal run_finished event + hx-get
**Notes:** Standard idiomatic HTMX SSE pattern; keeps scroll/selection.

---

## Backfill Startup Ergonomics

### Q1: When should the HTTP listener bind?

| Option | Description | Selected |
|--------|-------------|----------|
| AFTER backfill completes — blocking startup (Recommended) | Two-phase: migrate → bind. Docker healthcheck sees 'starting'; pairs with Phase 12 --start-period=60s. | ✓ |
| BEFORE backfill — /healthz returns 503 until done | Lets operators curl during backfill; more moving parts, every route must decide behavior. | |

**User's choice:** AFTER backfill completes
**Notes:** No half-state; simpler mental model.

### Q2: How should backfill progress be surfaced beyond INFO logs?

| Option | Description | Selected |
|--------|-------------|----------|
| INFO logs only — one line per 10k-row batch with pct (Recommended) | Sufficient for docker logs -f and Prometheus log-scrape. | ✓ |
| INFO + one-shot 'backfill complete' metric counter | Only works if listener binds during backfill — contradicts Q1 answer. | |
| INFO + progress file at /tmp/cronduit-migrate-progress.json | Filesystem coupling; over-engineered for v1.1. | |

**User's choice:** INFO logs only
**Notes:** Pairs with Q1 answer; no half-state surfaces.

### Q3: What happens if backfill fails partway through?

| Option | Description | Selected |
|--------|-------------|----------|
| Fail fast — crash with clear error; operator re-runs (Recommended) | Three-file migration guarantees recoverability; WHERE job_run_number IS NULL resumes. | ✓ |
| Auto-retry N times in-process before giving up | Hides transient issues; orchestrator restart policies are the right place. | |

**User's choice:** Fail fast; operator re-runs (idempotent)
**Notes:** Restart policies belong to container orchestrator, not cronduit itself.

### Q4: How do we lock the 'scheduler starts AFTER backfill' guarantee in tests?

| Option | Description | Selected |
|--------|-------------|----------|
| Sequence test in main.rs setup: assertion on NULL count before scheduler spawn (Recommended) | Cheap; T-V11-RUNNUM-01/02/03 exercise this. | ✓ |
| Integration test that injects NULL row mid-migration and expects crash | More thorough; requires test-only hooks. Not worth surface complexity. | |

**User's choice:** Assertion before scheduler spawn
**Notes:** Simple invariant check; test suite locks it.

---

## Claude's Discretion

Areas where the planner has flexibility (from CONTEXT.md § Claude's Discretion):
- Exact muted style tokens for `(id 1234)` suffix (use existing design-system muted-text token)
- Dispatch path for `run_finished` SSE event (broadcast channel vs oneshot)
- Client-side dedupe script location (inline in run_detail.html vs assets/static/app.js)
- Initial backfill line count N on page-load (suggested: 500 or match existing log_viewer.html first-page size)
- Per-line vs batch-commit granularity inside insert_log_batch (preserve batch tx; fetch_all vs Vec collection)

## Deferred Ideas

- Rekeying URLs by job_run_number (REQUIREMENTS.md explicitly defers; DB-13 locks global id as URL key)
- HTMX 4.x upgrade (breaks sse-swap)
- /healthz 'starting' state during backfill
- Auto-retry backfill within process
- Belt-and-suspenders Set-based dedupe
- Progress file at /tmp
- One-shot backfill metric counter
