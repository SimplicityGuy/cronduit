# Requirements: Cronduit v1.1 — Operator Quality of Life

**Defined:** 2026-04-14
**Milestone:** v1.1 (subsequent milestone; v1.0.1 shipped 2026-04-14)
**Core Value:** One tool that both runs recurrent jobs reliably AND makes their state observable through a web UI.

> Source documents: `.planning/PROJECT.md` § Current Milestone (locked scope), `.planning/research/SUMMARY.md` (research synthesis with Architecture corrections), `.planning/research/STACK.md`, `.planning/research/FEATURES.md`, `.planning/research/ARCHITECTURE.md`, `.planning/research/PITFALLS.md`, `.planning/milestones/v1.0-REQUIREMENTS.md` (archived baseline for REQ-ID numbering continuity).

> **v1.0 requirements (FOUND-01..11, CONF-01..10, DB-01..08, SCHED-01..08, RAND-01..06, EXEC-01..06, DOCKER-01..10, RELOAD-01..07, UI-01..15, OPS-01..05) are all validated and archived in `.planning/milestones/v1.0-REQUIREMENTS.md`.** v1.1 continues numbering per category from where v1.0 left off and introduces two new categories: `OBS` (observability polish) and `ERG` (ergonomics).

## v1.1 Requirements

Every requirement below is a testable operator-visible behavior. Pitfall test-case identifiers (`T-V11-*` from `.planning/research/PITFALLS.md`) are referenced inline where the pitfall research surfaced a specific verification lock.

### Scheduler (SCHED)

Continuation from v1.0 SCHED-01..08.

- [ ] **SCHED-09**: Operator can stop a running job from the UI; the run finalizes with a new `stopped` status (distinct from `cancelled`, `failed`, `timeout`, and `success`). Single hard kill — no SIGTERM grace-period escalation. Works identically for command, script, and docker job types. `T-V11-STOP-09`, `T-V11-STOP-10`, `T-V11-STOP-11`.

- [ ] **SCHED-10**: The scheduler maintains a per-run control handle (`RunControl`) carrying a `CancellationToken` plus a `stop_reason: Arc<AtomicU8>` so the executor can distinguish operator-stop from shutdown-cancel and finalize with the correct status. `T-V11-STOP-01`, `T-V11-STOP-02`, `T-V11-STOP-03`.

- [ ] **SCHED-11**: A stop request for a run that has already completed naturally does NOT overwrite the natural-completion status in the database. The race between `SchedulerCmd::Stop` and `JoinNext` is covered by a deterministic test using `tokio::time::pause`. `T-V11-STOP-04`, `T-V11-STOP-05`, `T-V11-STOP-06`.

- [ ] **SCHED-12**: Command and script executors continue using the shipped `.process_group(0)` + `libc::kill(-pid, SIGKILL)` pattern for kill; `kill_on_drop(true)` is NOT adopted because it would orphan shell-pipeline grandchildren. (Research correction #1; see SUMMARY.md § Research-Phase Corrections.) `T-V11-STOP-07`, `T-V11-STOP-08`.

- [ ] **SCHED-13**: `mark_run_orphaned` at restart does NOT overwrite rows already finalized to `stopped` / `success` / `failed` / `timeout`. The existing `WHERE status = 'running'` guard in `docker_orphan.rs` is locked in by test so future refactors cannot drop it. (Research correction #4.) `T-V11-STOP-12`, `T-V11-STOP-13`, `T-V11-STOP-14`.

- [ ] **SCHED-14**: A "Stop" button appears on the run detail page and in the job-detail run-history partial only when a run is in `status = 'running'`. Clicking sends a CSRF-gated `POST /api/runs/{run_id}/stop`. No confirmation dialog (consistent with "Run Now" which has none).

### Persistence (DB)

Continuation from v1.0 DB-01..08.

- [ ] **DB-09**: Every row in `job_runs` carries a per-job sequential number (`job_run_number`) assigned at insert time. Numbering starts at 1 for each job. Existing rows are backfilled on upgrade via an idempotent migration that runs strictly before the scheduler loop starts. `T-V11-RUNNUM-01`, `T-V11-RUNNUM-02`, `T-V11-RUNNUM-03`.

- [ ] **DB-10**: The migration to add `job_run_number` ships as **three** separate migration files (add-column nullable → backfill → add NOT-NULL constraint), never combined, because a partial-failure recovery from a single combined migration is unrecoverable. On SQLite the NOT-NULL step uses the 12-step table-rewrite pattern. Indexes are recreated verbatim. `T-V11-RUNNUM-04`, `T-V11-RUNNUM-05`, `T-V11-RUNNUM-06`.

- [ ] **DB-11**: Per-job numbering uses a dedicated counter column (`jobs.next_run_number`) incremented in a two-statement transaction on insert, NOT a subquery that computes `MAX(job_run_number) + 1`. This design works identically on SQLite and Postgres without dialect-specific locking. `T-V11-RUNNUM-10`, `T-V11-RUNNUM-11`.

- [ ] **DB-12**: The backfill migration chunks work into 10k-row batches and logs progress at INFO level so an operator running against a large `job_runs` table (100k+ rows) can see the migration making progress instead of a silent 30+ second stall. Dockerfile `HEALTHCHECK` `--start-period` is tuned to accommodate a reasonable upper bound. `T-V11-RUNNUM-07`, `T-V11-RUNNUM-08`, `T-V11-RUNNUM-09`.

- [ ] **DB-13**: `job_runs.id` remains the canonical URL key. The `/jobs/{job_id}/runs/{run_id}` route continues to accept the global `id`; `job_run_number` is display-only and does NOT break existing permalinks. `T-V11-RUNNUM-12`, `T-V11-RUNNUM-13`.

- [ ] **DB-14**: A new nullable `jobs.enabled_override` column (tri-state: NULL = follow config, 0 = force disabled, 1 = force enabled) supports bulk enable/disable without breaking config-source-of-truth semantics. `upsert_job` does NOT touch this column in its `ON CONFLICT DO UPDATE` SET clause. `disable_missing_jobs` clears the override when removing a job that has left the config file. `T-V11-BULK-01`.

### Web UI (UI)

Continuation from v1.0 UI-01..15.

- [ ] **UI-16**: The job run-history partial and run-detail breadcrumb show the per-job run number (`#42`) as the primary identifier instead of the global `job_runs.id`. Global id remains visible as a secondary hint for troubleshooting.

- [ ] **UI-17**: Navigating back to a running-job detail page renders the log lines already persisted to the database, then attaches to the live SSE stream without losing or duplicating any line across the live-to-static transition. `T-V11-BACK-01`, `T-V11-BACK-02`.

- [ ] **UI-18**: Log lines on the run-detail page remain in chronological order (by id, not by wall-clock timestamp) across the live-to-static transition that fires when a run finishes. Buffered SSE frames arriving after the static partial swap are dropped client-side via an id-based dedupe (`data-max-id` + listener check). `T-V11-LOG-03`, `T-V11-LOG-04`.

- [ ] **UI-19**: The transient "error getting logs" message that briefly renders on the run-detail page immediately after a Run Now click is eliminated. Root cause: the run row is inserted on the API handler thread (before returning the response to the client) instead of asynchronously in the scheduler loop. `T-V11-LOG-08`, `T-V11-LOG-09`.

- [ ] **UI-20**: The `LogLine` broadcast order is fixed so that the `id: Option<i64>` field required by UI-17 and UI-18 is populated before the broadcast is sent. Phase plan picks Option A (insert-then-broadcast with `RETURNING id`) or Option B (monotonic `seq` column) before writing the log-backfill implementation plan; the choice is recorded in the phase's PLAN.md. `T-V11-LOG-01`, `T-V11-LOG-02`.

### Observability (OBS) — new category

- [ ] **OBS-01**: A new `/timeline` page shows a cross-job gantt-style run timeline for the last 24h (default) or 7d (toggle). Each run renders as a horizontal bar color-coded by terminal status (`success`/`failed`/`timeout`/`cancelled`/`stopped`/`running`), using the existing `--cd-status-*` CSS variables from the v1.0 design system. No JS framework, no canvas, no WASM — inline server-rendered HTML + CSS grid only. Hidden/disabled jobs do not appear in the timeline.

- [ ] **OBS-02**: The timeline handler executes a single SQL query (not N+1) to fetch the window's runs, bounded by a hard `LIMIT 10000` to protect against pathological windows. `EXPLAIN QUERY PLAN` confirms the existing `idx_job_runs_start_time` index is used on both SQLite and Postgres. `T-V11-TIME-01`, `T-V11-TIME-02`. Timestamps are rendered in the operator's configured server timezone (from `[server].timezone`) to match the rest of the UI; `T-V11-TIME-04`.

- [ ] **OBS-03**: Each dashboard job card shows a success-rate badge and a 20-run column sparkline. Below a minimum sample threshold of N=5 terminal runs, the rate is rendered as `—` (dash), not a fake number. `stopped` runs are excluded from the denominator so operator-initiated stops do not skew the success rate. Zero-run jobs never crash the view. `T-V11-SPARK-01`, `T-V11-SPARK-02`, `T-V11-SPARK-03`, `T-V11-SPARK-04`.

- [ ] **OBS-04**: The job detail page shows duration trend as `p50: Xs` and `p95: Ys` computed over the last 100 successful runs. Percentile computation happens in Rust via a `src/web/stats.rs::percentile(samples, q)` helper with tests covering empty / single-element / minimum-sample-size edge cases. Below a minimum threshold of N=20 samples the values render as `—` instead of meaningless numbers. `T-V11-DUR-01`, `T-V11-DUR-02`, `T-V11-DUR-03`, `T-V11-DUR-04`.

- [ ] **OBS-05**: SQL-native percentile functions (`percentile_cont`) are NOT used even on Postgres. The structural-parity constraint from v1.0 requires the same code path to work on both SQLite and Postgres, and Rust-side computation satisfies this cleanly.

### Ergonomics (ERG) — new category

- [ ] **ERG-01**: The dashboard supports multi-select of jobs via checkboxes and a "Disable selected" / "Enable selected" action bar. Submitting the action fires a CSRF-gated `POST /api/jobs/bulk-toggle` handler that updates `jobs.enabled_override` for every selected job and then fires `SchedulerCmd::Reload` so the scheduler rebuilds its heap without the newly-disabled jobs.

- [ ] **ERG-02**: Bulk disable does NOT terminate running jobs — running instances complete naturally. The success toast communicates this explicitly (e.g. `"3 jobs disabled; 2 currently-running jobs will complete"`). Operators who want to kill a running job use the Stop button from SCHED-14.

- [ ] **ERG-03**: The settings page shows a "Currently overridden" section listing every job whose `enabled_override` is non-null, so operators can see at a glance which jobs have been manually disabled vs. which are simply absent from the config file. Without this, a v1.1 operator could bulk-disable five jobs, forget, and have backups silently not run for months.

- [ ] **ERG-04**: A reload (SIGHUP / API / file-watch) does NOT reset `enabled_override`. A job that is present in the config file AND has `enabled_override = 0` stays disabled. A job that is absent from the config (e.g. removed by the operator) has its `enabled_override` cleared at the same time as `enabled` is set to 0, so re-adding it to the config later produces the expected "fresh" behavior. `T-V11-BULK-01` locks this invariant.

### Operational (OPS)

Continuation from v1.0 OPS-01..05.

- [x] **OPS-06**: A new `cronduit health` CLI subcommand performs a local HTTP GET against `/health`, parses the JSON response, and exits 0 only if `status == "ok"`. It fails fast on connection-refused (no retry; the Docker healthcheck has its own retry policy) and reads the bind address from either a `--bind` flag or defaults to `http://127.0.0.1:8080`.

- [x] **OPS-07**: The Dockerfile ships with a `HEALTHCHECK CMD ["/cronduit", "health"]` directive using conservative defaults (`--interval=30s --timeout=5s --start-period=60s --retries=3`), so `docker compose up` reports `healthy` out of the box without any compose-file healthcheck stanza. Operators who write their own `healthcheck:` in compose continue to work (compose overrides Dockerfile). `T-V11-HEALTH-01`, `T-V11-HEALTH-02`.

- [x] **OPS-08**: The root cause of the reported `(unhealthy)` symptom (busybox `wget --spider` in alpine:3 misparses axum's chunked responses) is reproduced in a test environment before the fix is declared complete. If the reproduction shows a different root cause, this requirement is re-scoped; the `cronduit health` subcommand fix path is correct regardless because it removes the entire busybox wget dependency from the healthcheck path.

- [ ] **OPS-09** (Phase 12.1): GHCR `:latest` tag ONLY tracks the latest released non-rc stable version (e.g. `v1.0.1`, later `v1.1.0`). rc tag pushes (`vX.Y.Z-rc.N`) MUST NOT move `:latest`. Main-branch builds MUST NOT move `:latest`. The pre-existing `:latest` divergence from the v1.0.1 retag is corrected by a one-shot maintainer-run `docker buildx imagetools create` so `:latest` digest == `:1.0.1` digest as of the phase close.

- [ ] **OPS-10** (Phase 12.1): Every push to `main` triggers a multi-arch (amd64+arm64) build and publishes `ghcr.io/simplicityguy/cronduit:main` pointing at the freshly-built image. Operators who want bleeding-edge main builds pin `:main`; operators who want latest-released pin `:latest`. Documented in README.

### Foundation (FOUND) — hygiene

Continuation from v1.0 FOUND-01..11.

- [ ] **FOUND-12**: `rand` crate is bumped from `"0.8"` to `"0.9.x"` (stale by two majors, no CVE, hygiene only). Call sites — the `@random` slot picker and CSRF token generation — are migrated mechanically. `rand 0.10` is NOT adopted in v1.1 to avoid the `gen` → `random` trait rename churn.

- [ ] **FOUND-13**: `Cargo.toml` version is bumped from `1.0.1` to `1.1.0` on the first v1.1 commit. This ensures `cronduit --version` always reports the milestone-in-progress even between rc cuts. rc tags use the semver pre-release format `v1.1.0-rc.1`, `v1.1.0-rc.2`, etc. (dot before `rc.N`). The final ship is `v1.1.0`.

## Future Requirements

Deferred to a future milestone; NOT in v1.1 scope. Duplicated from `.planning/PROJECT.md` § Future Requirements for traceability.

### v1.2 — Feature expansion (tentative)
- Webhook notifications on job state transitions
- Job concurrency limits and queuing (deep scheduler-core change)
- Failure clustering / "what changed" context on run detail
- Per-job exit-code histogram on job detail page
- Cross-run log search across retention window
- Job tagging / grouping

### v1.3 — Operational ergonomics deepening (tentative)
- Snooze a job for a duration (auto-re-enable)
- Run history filters (status, date range, exit code) and sortable columns

### v1.4 — UX polish (tentative)
- Job duplicate-as-snippet (UI emits TOML snippet)
- Fuzzy job search

## Out of Scope

Explicit boundaries; NOT in v1.1 or v1.2. Duplicated from PROJECT.md § Out of Scope for explicitness.

- **Web UI authentication** — deferred to v2. v1.x still assumes loopback / trusted LAN / reverse-proxy fronting. The new Stop button DOES widen the blast radius for anyone with UI access (they can now terminate any running job), so the v1.1 ship should add a one-line note to `THREAT_MODEL.md` explicitly enumerating Stop. No design work.
- **Multi-node / distributed scheduling** — single-node only.
- **User management / RBAC** — single-operator tool.
- **Workflow DAGs / job dependencies** — jobs are independent.
- **Email notifications** — operators can layer email on top of a future webhook via bridges.
- **Ad-hoc one-shot runs not defined in the config** — config remains source of truth.
- **Importer for existing ofelia configs** — users rewrite.
- **SPA / React frontend** — server-rendered HTML only.
- **Graceful SIGTERM to SIGKILL escalation on Stop** — v1.1 single hard kill; a future `stop_grace_period` per-job field can be added in v1.2 additively without breaking existing calls.
- **Confirmation dialog for Stop** — consistent with "Run Now" having none. Toast-only.
- **SQL-native percentile functions** — see OBS-05.
- **`kill_on_drop(true)` pattern** — see SCHED-12 and Research Correction #1.
- **Rekeying URLs by `job_run_number`** — see DB-13.
- **HTMX 4.x upgrade** — 4.x removes `sse-swap`, a breaking change for the v1.0 SSE log pattern. Defer to a dedicated upgrade phase after v1.1 ships.

## Traceability

| REQ-ID   | Phase    | Status  |
| -------- | -------- | ------- |
| SCHED-09 | Phase 10 | Pending |
| SCHED-10 | Phase 10 | Pending |
| SCHED-11 | Phase 10 | Pending |
| SCHED-12 | Phase 10 | Pending |
| SCHED-13 | Phase 10 | Pending |
| SCHED-14 | Phase 10 | Pending |
| DB-09    | Phase 11 | Pending |
| DB-10    | Phase 11 | Pending |
| DB-11    | Phase 11 | Pending |
| DB-12    | Phase 11 | Pending |
| DB-13    | Phase 11 | Pending |
| DB-14    | Phase 14 | Pending |
| UI-16    | Phase 11 | Pending |
| UI-17    | Phase 11 | Pending |
| UI-18    | Phase 11 | Pending |
| UI-19    | Phase 11 | Pending |
| UI-20    | Phase 11 | Pending |
| OBS-01   | Phase 13 | Pending |
| OBS-02   | Phase 13 | Pending |
| OBS-03   | Phase 13 | Pending |
| OBS-04   | Phase 13 | Pending |
| OBS-05   | Phase 13 | Pending |
| ERG-01   | Phase 14 | Pending |
| ERG-02   | Phase 14 | Pending |
| ERG-03   | Phase 14 | Pending |
| ERG-04   | Phase 14 | Pending |
| OPS-06   | Phase 12   | Done    |
| OPS-07   | Phase 12   | Done    |
| OPS-08   | Phase 12   | Done    |
| OPS-09   | Phase 12.1 | Pending |
| OPS-10   | Phase 12.1 | Pending |
| FOUND-12 | Phase 10   | Pending |
| FOUND-13 | Phase 10   | Pending |

**Total:** 33 requirements across 7 categories, mapped to 5 phases + 1 inserted phase (10–14 + 12.1). OPS-06..08 complete; rest pending implementation.

### Phase → requirement rollup

| Phase | Requirements (count)                                           | rc target                         |
| ----- | -------------------------------------------------------------- | --------------------------------- |
| 10    | SCHED-09..14, FOUND-12..13 (8)                                 | `v1.1.0-rc.1`                     |
| 11    | DB-09..13, UI-16..20 (10)                                      | `v1.1.0-rc.1`                     |
| 12    | OPS-06..08 (3)                                                 | `v1.1.0-rc.1` ◀ (tag cut)         |
| 12.1  | OPS-09..10 (2) _(INSERTED)_                                    | prereq for Phase 13 rc.2 cut      |
| 13    | OBS-01..05 (5)                                                 | `v1.1.0-rc.2` ◀ (tag cut)         |
| 14    | ERG-01..04, DB-14 (5)                                          | `v1.1.0-rc.3` ◀ + `v1.1.0`        |

---

*Defined: 2026-04-14 — milestone kickoff, after the 4-dimension research pass. The Pitfalls research surfaced four corrections to the Architecture research (see `.planning/research/SUMMARY.md` § Research-Phase Corrections); those corrections are incorporated into the requirement language above. Traceability populated 2026-04-14 during roadmap creation — all 31 requirements mapped to Phase 10–14.*
