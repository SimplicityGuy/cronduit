---
phase: 11
slug: per-job-run-numbers-log-ux-fixes
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-16
revised: 2026-04-16
---

# Phase 11 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Pre-populated from `11-RESEARCH.md § Validation Architecture`; planner refines per-plan during planning and executor fills the Per-Task Verification Map row-by-row.

> **Wave 0 status:** complete. Plan 11-00 (wave 0) landed all Wave-0 harness files as
> compiling `#[ignore]` stubs. Downstream plans remove `#[ignore]` and replace the
> vacuous `assert!(true)` bodies with real assertions.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` / `cargo nextest` (existing Rust test infra, no changes) |
| **Config file** | `Cargo.toml` (existing); no new `.cargo/config.toml` or `.nextest.toml` required |
| **Quick run command** | `cargo nextest run --lib v11 --no-fail-fast` (pattern-filter to Phase 11 tests) |
| **Full suite command** | `cargo nextest run --all-features --profile ci` (CI-equivalent) |
| **Integration command** | `cargo nextest run --features integration --test 'v11_*'` (testcontainers/Postgres + Docker-backed runs) |
| **Benchmark command** | `cargo test --release --test v11_log_dedupe_benchmark -- --nocapture` (T-V11-LOG-02 gate) |
| **Estimated runtime** | quick ~15s · full ~90s · integration ~180s · benchmark ~30s |

---

## Sampling Rate

- **After every task commit:** Run `cargo nextest run --lib v11 --no-fail-fast` (quick) — max 15s feedback.
- **After every plan wave:** Run `cargo nextest run --all-features --profile ci` (full) — max 90s feedback.
- **Before `/gsd-verify-work`:** Full suite + integration suite must be green (no `⚠️ flaky`, no `❌ red` in the map below).
- **Max feedback latency:** 15s (task) / 90s (wave) / 180s (integration).

---

## Per-Task Verification Map

Every Phase 11 requirement is locked to at least one test ID. The planner MUST map each plan's tasks to rows in this table; executor updates Status column.

| Task ID (tentative) | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------------------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 11-01-01 | 01 | 1 | DB-09, UI-20 | — | LogLine.id populated before broadcast (T-V11-LOG-01) | unit | `cargo nextest run --lib -E 'test(log_line_id_populated_before_broadcast)'` | ✅ W0 | ⬜ pending |
| 11-01-02 | 01 | 1 | UI-17, UI-18, UI-20 | — | p95 insert latency < 50ms for 64-line SQLite batch (T-V11-LOG-02) | benchmark | `cargo test --release --test v11_log_dedupe_benchmark p95_under_50ms` | ✅ W0 | ⬜ pending |
| 11-02-01 | 02 | 2 | DB-09, DB-10 | — | Migration file 1 (add nullable) creates `jobs.next_run_number` + `job_runs.job_run_number` on both backends (T-V11-RUNNUM-01) | migration | `cargo nextest run --test v11_runnum_migration migration_01_add_nullable_columns` | ✅ W0 | ⬜ pending |
| 11-02-02 | 02 | 2 | DB-10 | — | Migration file 1 idempotent (re-run is no-op on SQLite + Postgres) (T-V11-RUNNUM-04) | migration | `cargo nextest run --test v11_runnum_migration migration_01_idempotent` | ✅ W0 | ⬜ pending |
| 11-03-01 | 03 | 3 | DB-09, DB-12 | — | Migration file 2 chunked backfill (10k-row batches, INFO progress log) fills all NULLs (T-V11-RUNNUM-02) | migration | `cargo nextest run --test v11_runnum_migration migration_02_backfill_completes` | ✅ W0 | ⬜ pending |
| 11-03-02 | 03 | 3 | DB-12 | — | Backfill INFO log format: `batch={i}/{N} rows={done}/{total} pct={p:.1}% elapsed_ms={ms}` (T-V11-RUNNUM-07) | migration | `cargo nextest run --test v11_runnum_migration migration_02_logs_progress` | ✅ W0 | ⬜ pending |
| 11-03-03 | 03 | 3 | DB-10, DB-12 | — | Backfill idempotent after partial-crash; resumes via `WHERE job_run_number IS NULL` (T-V11-RUNNUM-05) | migration | `cargo nextest run --test v11_runnum_migration migration_02_resume_after_crash` | ✅ W0 | ⬜ pending |
| 11-03-04 | 03 | 3 | DB-11 | — | `jobs.next_run_number = MAX(job_run_number) + 1` after backfill (T-V11-RUNNUM-08) | migration | `cargo nextest run --test v11_runnum_migration migration_02_counter_reseed` | ✅ W0 | ⬜ pending |
| 11-03-05 | 03 | 3 | DB-09, DB-12 | — | Backfill uses `ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id ASC)` on both backends (T-V11-RUNNUM-09) | migration | `cargo nextest run --test v11_runnum_migration migration_02_row_number_order_by_id` | ✅ W0 | ⬜ pending |
| 11-04-01 | 04 | 4 | DB-10 | — | Migration file 3 (NOT NULL) applies on Postgres via `ALTER COLUMN ... SET NOT NULL` (T-V11-RUNNUM-03) | migration | `cargo nextest run --features integration --test v11_runnum_migration migration_03_postgres_not_null` | ✅ W0 | ⬜ pending |
| 11-04-02 | 04 | 4 | DB-10 | — | Migration file 3 SQLite 12-step rewrite preserves rows + indexes + foreign keys (T-V11-RUNNUM-06) | migration | `cargo nextest run --test v11_runnum_migration migration_03_sqlite_table_rewrite` | ✅ W0 | ⬜ pending |
| 11-04-03 | 04 | 4 | DB-10 | — | SQLite rewrite recreates `idx_job_runs_job_id_start` + `idx_job_runs_start_time` verbatim (T-V11-RUNNUM-06) | migration | `cargo nextest run --test v11_runnum_migration migration_03_sqlite_indexes_preserved` | ✅ W0 | ⬜ pending |
| 11-05-01 | 05 | 5 | DB-11 | — | `insert_running_run` uses two-statement transaction: UPDATE jobs SET next_run_number RETURNING + INSERT (T-V11-RUNNUM-10) | unit | `cargo nextest run --lib insert_running_run_uses_counter_transaction` | ✅ W0 | ⬜ pending |
| 11-05-02 | 05 | 5 | DB-11 | — | `next_run_number` increments monotonically per job under concurrent inserts (T-V11-RUNNUM-11) | concurrency | `cargo nextest run --lib insert_running_run_concurrent_monotonic` | ✅ W0 | ⬜ pending |
| 11-06-01 | 06 | 6 | UI-19 | — | `run_now` handler synchronously inserts `job_runs` row before returning HX-Refresh (T-V11-LOG-08) | integration | `cargo nextest run --test v11_run_now_sync_insert handler_inserts_before_response` | ✅ W0 | ⬜ pending |
| 11-06-02 | 06 | 6 | UI-19 | — | No "error getting logs" flash on immediate click-through after Run Now (T-V11-LOG-09) | e2e | `cargo nextest run --features integration --test v11_run_now_sync_insert no_transient_error` | ✅ W0 | ⬜ pending |
| 11-06-03 | 06 | 6 | UI-19 | — | `SchedulerCmd::RunNowWithRunId { job_id, run_id }` variant exists and scheduler reuses pre-inserted run_id | unit | `cargo nextest run --lib scheduler_cmd_run_now_with_run_id_variant` | ✅ W0 | ⬜ pending |
| 11-07-01 | 07 | 7 | UI-20 | — | `insert_log_batch` returns `Vec<i64>` of persisted log ids (T-V11-LOG-01) | unit | `cargo nextest run --lib insert_log_batch_returns_ids` | ✅ W0 | ⬜ pending |
| 11-07-02 | 07 | 7 | UI-20 | — | Batch transaction preserved — one fsync per batch, not per line (T-V11-LOG-02) | unit | `cargo nextest run --lib insert_log_batch_single_tx_per_batch` | ✅ W0 | ⬜ pending |
| 11-08-01 | 08 | 8 | UI-18 | — | SSE handler emits `id:` line per log_line event (T-V11-LOG-05) | integration | `cargo nextest run --test v11_sse_log_stream event_includes_id_field` | ✅ W0 | ⬜ pending |
| 11-08-02 | 08 | 8 | UI-17, UI-18 | — | Broadcast channel delivers monotonic ids per run (T-V11-LOG-06) | integration | `cargo nextest run --test v11_sse_log_stream ids_monotonic_per_run` | ✅ W0 | ⬜ pending |
| 11-09-01 | 09 | 9 | UI-17 | — | `queries::get_log_lines` returns lines with id ASC ordering (T-V11-BACK-01) — uses existing function at src/db/queries.rs:783 | unit | `cargo nextest run --lib get_recent_job_logs_chronological` | ✅ W0 | ⬜ pending |
| 11-09-02 | 09 | 9 | UI-17 | — | Run-detail page GET renders persisted lines inline + `data-max-id` on `#log-lines` | integration | `cargo nextest run --test v11_run_detail_page_load renders_static_backfill` | ✅ W0 | ⬜ pending |
| 11-10-01 | 10 | 9 | UI-17, UI-18 | — | Terminal `run_finished` SSE event fires before `drop(broadcast_tx)` in `finalize_run` (T-V11-LOG-07) | integration | `cargo nextest run --test v11_sse_terminal_event fires_before_broadcast_drop` | ✅ W0 | ⬜ pending |
| 11-10-02 | 10 | 9 | UI-17, UI-18 | — | `run_finished` event payload includes `run_id` + `final_status` (one of success/failed/timeout/stopped) | integration | `cargo nextest run --test v11_sse_terminal_event payload_shape` | ✅ W0 | ⬜ pending |
| 11-11-01 | 11 | 10 | UI-17, UI-18, UI-20 | — | Client-side dedupe: SSE events with `id <= maxId` dropped (T-V11-LOG-03) | autonomous | `cargo nextest run --test v11_log_dedupe_contract v11_dedupe_contract` (unit test replacing browser UAT; full visual UAT in Plan 11-12 Task 5) | ✅ W0 | ⬜ pending |
| 11-11-02 | 11 | 10 | UI-18 | — | Script references `dataset.maxId` + `preventDefault` + `htmx:sseBeforeMessage` (T-V11-LOG-04) | contract | `cargo nextest run --test v11_log_dedupe_contract script_references_dataset_maxid script_references_htmx_sse_hook` | ✅ W0 | ⬜ pending |
| 11-11-03 | 11 | 10 | UI-17 | — | Back-navigation to running-job detail renders persisted + live with zero gap/duplicate (T-V11-BACK-02) | browser | Manual UAT: consolidated into Plan 11-12 Task 5 |  | ⬜ pending |
| 11-12-01 | 12 | 11 | UI-16 | — | `run_history.html` renders `#{{ run.job_run_number }}` with `title="global id: {{ run.id }}"` | template | `cargo nextest run --test v11_log_dedupe_contract run_history_renders_run_number_and_title_attr` | ✅ W0 | ⬜ pending |
| 11-12-02 | 12 | 11 | UI-16 | — | `run_detail.html` header renders `Run #{{ run.job_run_number }}` + `(id {{ run.id }})` muted suffix | template | `cargo nextest run --test v11_run_detail_page_load header_renders_runnum_with_id_suffix` | ✅ W0 | ⬜ pending |
| 11-12-03 | 12 | 11 | UI-16, DB-13 | — | URL `/jobs/{job_id}/runs/{run_id}` continues to resolve by global `job_runs.id` (T-V11-RUNNUM-12/13) | integration | `cargo nextest run --test v11_run_detail_page_load permalink_by_global_id` | ✅ W0 | ⬜ pending |
| 11-13-01 | 13 | 12 | DB-09, DB-10 | — | `src/cli/run.rs` PANICS with clear message `SELECT COUNT(*) FROM job_runs WHERE job_run_number IS NULL > 0` — per D-15 locked wording (NOT anyhow::bail) | integration | `cargo nextest run --test v11_startup_assertion panics_when_null_rows_present` | ✅ W0 | ⬜ pending |
| 11-13-02 | 13 | 12 | — | — | HTTP listener binds AFTER backfill completes (strict two-phase startup) | integration | `cargo nextest run --test v11_startup_assertion listener_after_backfill` | ✅ W0 | ⬜ pending |
| 11-14-01 | 14 | 13 | all | — | Schema parity test `tests/schema_parity.rs` stays green with Phase 11 migrations | integration | `cargo nextest run --features integration --test schema_parity` | ✅ (exists) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

*"File Exists" column: ✅ W0 means Plan 11-00 landed the harness as a compiling stub; ✅ (exists) means test infra already present pre-Phase 11.*

---

## Wave 0 Requirements

New test files created in Wave 0 (Plan 11-00 landed these as compiling `#[ignore]` stubs):

- [x] `tests/v11_runnum_migration.rs` — integration harness for all three migration files (SQLite in-memory + Postgres testcontainer). Covers T-V11-RUNNUM-01..09.
- [x] `tests/v11_run_now_sync_insert.rs` — integration harness for the UI-19 race fix. Covers T-V11-LOG-08, T-V11-LOG-09.
- [x] `tests/v11_sse_log_stream.rs` — integration harness asserting SSE event shape + id monotonicity. Covers T-V11-LOG-05, T-V11-LOG-06.
- [x] `tests/v11_sse_terminal_event.rs` — integration harness for the `run_finished` terminal event. Covers T-V11-LOG-07.
- [x] `tests/v11_run_detail_page_load.rs` — integration harness for server-rendered backfill + `data-max-id`. Covers T-V11-BACK-01, URL-resolution tests.
- [x] `tests/v11_startup_assertion.rs` — integration harness for the NULL-count panic (D-15) + listener-after-backfill invariant.
- [x] `tests/v11_log_dedupe_benchmark.rs` — T-V11-LOG-02 gate: criterion-style benchmark or bespoke harness, 100 iterations × 64-line batches against in-memory SQLite, p95 threshold 50ms.
- [x] `tests/common/v11_fixtures.rs` — shared fixtures: `setup_sqlite_with_phase11_migrations`, `seed_test_job`, `seed_running_run`, `make_test_batch`, `seed_null_runs`.

Additional harness files created by Plan 11-00 (referenced by downstream plans):

- [x] `tests/v11_log_id_plumbing.rs` — Plan 11-07 owns (broadcast_id_populated, insert_log_batch_returns_ids, insert_log_batch_single_tx_per_batch).
- [x] `tests/v11_runnum_counter.rs` — Plan 11-05 owns (runnum_starts_at_1, counter_transaction, concurrent_distinct, invariant).
- [x] `tests/v11_log_dedupe_contract.rs` — Plans 11-11 + 11-12 own (dataset.maxId, run_finished listener, htmx hook, data_max_id_rendered, run_history rendering, autonomous v11_dedupe_contract unit test).

Existing infrastructure leveraged:
- `tests/schema_parity.rs` — already green; Phase 11 migrations must keep it green.
- `tests/common/` — existing shared test module; Plan 11-00 registers v11_fixtures here.
- `sqlx-cli` offline `query!` macro checks via `cargo sqlx prepare` pre-commit.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| No transient "error getting logs" flash visible to the eye | UI-19 | Browser visual regression — no CLI tool catches sub-100ms flashes reliably | Start cronduit locally (`just dev` or `cargo run`). Open dashboard. Click "Run Now" on a job. Immediately click through to the run-detail page. Observe that the logs panel never flashes "error getting logs". Repeat 10x. |
| Chronological continuity across live→static transition | UI-17, UI-18, T-V11-BACK-02 | Requires a real running job whose stream is active while operator navigates | Start cronduit locally. Run a long (60s+) job via "Run Now". Navigate to run-detail while running. Navigate away (to dashboard), wait 5s, navigate back. Confirm: (a) previously-seen lines are present, (b) new lines arrive chronologically without gaps/duplicates, (c) when the run completes, the static partial renders without log jitter. |
| No duplicate lines on client-side dedupe | UI-18, T-V11-LOG-03 | Requires eyeballing rendered DOM under live stream | Run a job with chatty output (1000+ lines). Open devtools → Elements, watch `#log-lines`. Confirm no line appears twice. Confirm `data-max-id` increases monotonically between mutations. |
| `#42` hover tooltip displays global id | UI-16 | Hover-state browser behavior | Run-history partial: hover over any `#N` cell, confirm tooltip reads `global id: {integer}`. Keyboard-focus the cell (Tab), confirm accessible name is also read by screen reader. |
| Backfill INFO logs visible in `docker logs -f` during startup | DB-12 | Operator observability during a 30+s migration | Seed a SQLite DB with 100k `job_runs` rows (via test fixture). Start cronduit with that DB. `docker logs -f` should show ~10 INFO lines matching the format `batch=i/N rows=done/total pct=X.X% elapsed_ms=M`. |

**Note on browser UAT consolidation:** Plan 11-11 previously had its own browser-UAT
checkpoint. The revised plan set moves that UAT into Plan 11-12 Task 5 (which already
has a browser UAT) — Plan 11-12 is where `data-max-id` is emitted into the template,
so Plan 11-11's dedupe behavior is first meaningfully verifiable there. Consolidation
avoids double-walking an operator through the same flow.

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: Wave 0 (Plan 11-00) landed every harness before Wave 1 starts
- [x] Wave 0 covers all previously-❌ MISSING references (10 harness files landed — 7 required + 3 additional)
- [x] No watch-mode flags (`cargo watch` is dev-only; CI never invokes watch mode)
- [x] Feedback latency < 15s (quick suite), < 90s (full), < 180s (integration)
- [x] `nyquist_compliant: true` set in frontmatter (Plan 11-00 landed Wave 0)

**Approval:** approved 2026-04-16 (revision; Plan 11-00 landed Wave 0)

---

## Notes for the Planner

- The T-V11-LOG-02 benchmark (`11-01-02`) is the **gated decision point**: if p95 ≥ 50ms, Phase 11 flips from Option A (RETURNING id) to Option B (monotonic `seq: u64` column). Plan 11-01 replaces the Wave-0 stub with the real body and records the result in SUMMARY.md.
- Wave 0 test files were created in a single "setup" plan (Plan 11-00) — every subsequent plan's test task simply removes `#[ignore]` and replaces the vacuous `assert!(true)` with real assertions.
- The browser-UAT row (`11-11-03`) is marked `browser` test type and is consolidated into Plan 11-12 Task 5 per the revision.
- The test ID column in the map is tentative — the planner refines to match actual plan numbers and adjusts Wave assignments as needed. The requirement→test-ID linkage is the invariant.
- Wave numbering revised: Plan 11-07 bumped to wave 6 (file conflict with Plan 11-06 resolved by dep + wave shift). Plans 11-08/09/10/11/12/13/14 each shift up by one wave accordingly.
- Iter-2 cascade: Plan 11-05 bumped to wave 5 (must be strictly > 11-04 @ wave 4). Plans 11-06→6, 11-07→7, 11-08→8, 11-09→8, 11-10→9, 11-11→10, 11-12→11, 11-13→12, 11-14→13 shifted accordingly.
</content>
</invoke>